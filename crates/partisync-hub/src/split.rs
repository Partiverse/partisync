//! 动态分裂协议与崩溃恢复（SPEC M3-WP01 §3）。
//!
//! 协议不变量：**分区元数据先行 + 搬移原子批**——任一步崩溃后：
//! - meta 原子批要么整体生效要么不生效（新分区行存在 ⇔ splitting 态存在）；
//! - 搬移批每批原子（insert 目标 + remove 源同批），键恒在且仅在一侧；
//! - 恢复方向确定：见 splitting 分区行 → 键序前驱为源 → 重放搬移至清空 →
//!   置 active。半分裂状态不外泄（单写者串行 + open 先恢复后服务）。

use fjall::{Keyspace, OwnedWriteBatch};

use crate::entry_plane::HubError;
use crate::router::{META_NEXT_PID, STATUS_ACTIVE, STATUS_SPLITTING};
use crate::tree_plane::{TreePlane, MOVE_BATCH};

impl TreePlane {
    /// put 路径入口：行数超阈值时对新分裂（fresh）执行。
    pub(crate) fn split_partition(&self, idx: usize) -> Result<(), HubError> {
        let (src_id, next_start) = {
            let router = self.router.read().expect("router lock");
            let p = &router.partitions()[idx];
            (
                p.id,
                router.partitions().get(idx + 1).map(|n| n.start.clone()),
            )
        };
        let src = self
            .keyspaces
            .read()
            .expect("keyspaces lock")
            .get(&src_id)
            .cloned()
            .ok_or(HubError::PartitionMissing(src_id))?;

        // 1. 选键序中位边界（真实行数计；内存计数仅做触发近似）
        let keys: Vec<Vec<u8>> = src
            .iter()
            .filter_map(|g| g.key().ok().map(|k| k.to_vec()))
            .collect();
        let n = keys.len();
        if n < 2 {
            self.router
                .write()
                .expect("router lock")
                .set_count(idx, n as u64);
            return Ok(());
        }
        let median = keys[n / 2].clone();

        // 2. 元数据先行：原子批写 next-pid 计数器 + 新分区行（splitting 态）
        let new_pid = {
            let cur = self
                .meta
                .get(META_NEXT_PID)
                .map_err(HubError::from)?
                .map_or(1u64, |v| {
                    let mut b = [0u8; 8];
                    b.copy_from_slice(&v);
                    u64::from_be_bytes(b)
                });
            let mut batch = OwnedWriteBatch::with_capacity(self.db.clone(), 2);
            batch.insert(&self.meta, META_NEXT_PID, (cur + 1).to_be_bytes());
            let mut row = Vec::with_capacity(9);
            row.extend_from_slice(&cur.to_be_bytes());
            row.push(STATUS_SPLITTING);
            batch.insert(&self.meta, partition_meta_key(&median), row);
            batch.commit().map_err(HubError::from)?;
            cur
        };
        let target = self
            .db
            .keyspace(
                &format!("t-p{new_pid}"),
                fjall::KeyspaceCreateOptions::default,
            )
            .map_err(HubError::from)?;
        // 不变量：新 pid 不得与既有分区冲突（计数器错位将退化为同库自我搬移）
        assert_ne!(
            new_pid, src_id,
            "split allocated pid {new_pid} colliding with source"
        );

        // 3. 搬移 [median, next_start)：每批原子（insert 目标 + remove 源同批）
        let moved = self.move_keys(&src, &target, &median, next_start.as_deref())?;

        // 4. 收尾：置 active → 路由生效（源分区行不变、键已搬空）
        let mut batch = OwnedWriteBatch::with_capacity(self.db.clone(), 1);
        let mut row = Vec::with_capacity(9);
        row.extend_from_slice(&new_pid.to_be_bytes());
        row.push(STATUS_ACTIVE);
        batch.insert(&self.meta, partition_meta_key(&median), row);
        batch.commit().map_err(HubError::from)?;

        // 5. 内存路由更新：新分区插到源之后，计数按搬移实况修正
        self.keyspaces
            .write()
            .expect("keyspaces lock")
            .insert(new_pid, target);
        let src_count = self
            .keyspaces
            .read()
            .expect("keyspaces lock")
            .get(&src_id)
            .map_or(0, |ks| ks.iter().count() as u64);
        let mut router = self.router.write().expect("router lock");
        router.insert_split(
            idx,
            crate::router::Partition {
                start: median,
                id: new_pid,
                splitting: false,
            },
            src_count + moved,
            moved,
        );
        Ok(())
    }

    /// open 恢复：收尾所有 splitting 分区（meta 行已在 Router::load 读入）。
    ///
    /// 恢复路径与 fresh 分裂的区别：pid/分区行已存在，只重放搬移 + 置 active。
    pub(crate) fn recover_splits(&self) -> Result<(), HubError> {
        let targets: Vec<(usize, u64, Vec<u8>)> = {
            let router = self.router.read().expect("router lock");
            router
                .partitions()
                .iter()
                .enumerate()
                .filter(|(_, p)| p.splitting)
                .map(|(i, p)| (i, p.id, p.start.clone()))
                .collect()
        };
        for (idx, pid, start) in targets {
            let (src_ks, next_start) = {
                let router = self.router.read().expect("router lock");
                let prev = router.partitions()[idx - 1].id;
                let next_start = router.partitions().get(idx + 1).map(|n| n.start.clone());
                let src_ks = self
                    .keyspaces
                    .read()
                    .expect("keyspaces lock")
                    .get(&prev)
                    .cloned()
                    .ok_or(HubError::PartitionMissing(prev))?;
                (src_ks, next_start)
            };
            let target = self
                .keyspaces
                .read()
                .expect("keyspaces lock")
                .get(&pid)
                .cloned()
                .ok_or(HubError::PartitionMissing(pid))?;
            self.move_keys(&src_ks, &target, &start, next_start.as_deref())?;

            let mut batch = OwnedWriteBatch::with_capacity(self.db.clone(), 1);
            let mut row = Vec::with_capacity(9);
            row.extend_from_slice(&pid.to_be_bytes());
            row.push(STATUS_ACTIVE);
            batch.insert(&self.meta, partition_meta_key(&start), row);
            batch.commit().map_err(HubError::from)?;
            self.router
                .write()
                .expect("router lock")
                .clear_splitting(idx);
        }
        // 恢复搬移改变了各分区驻留数——全量重算
        self.recount_all();
        Ok(())
    }

    /// 搬移 `[start, end)`（end=None 即无界），返回搬移行数；每批原子。
    fn move_keys(
        &self,
        src: &Keyspace,
        target: &Keyspace,
        start: &[u8],
        end: Option<&[u8]>,
    ) -> Result<u64, HubError> {
        let mut moved: u64 = 0;
        loop {
            let it = match end {
                None => src.range(start.to_vec()..),
                Some(e) => src.range(start.to_vec()..e.to_vec()),
            };
            let mut chunk = Vec::with_capacity(MOVE_BATCH);
            for guard in it {
                let (k, v) = guard.into_inner().map_err(HubError::from)?;
                chunk.push((k.to_vec(), v.to_vec()));
                if chunk.len() >= MOVE_BATCH {
                    break;
                }
            }
            if chunk.is_empty() {
                return Ok(moved);
            }
            let mut batch = OwnedWriteBatch::with_capacity(self.db.clone(), chunk.len() * 2);
            for (k, v) in &chunk {
                batch.insert(target, k.as_slice(), v.as_slice());
                batch.remove(src, k.as_slice());
            }
            batch.commit().map_err(HubError::from)?;
            moved += chunk.len() as u64;
        }
    }

    fn recount_all(&self) {
        let keyspaces = self.keyspaces.read().expect("keyspaces lock");
        let mut router = self.router.write().expect("router lock");
        let counts: Vec<u64> = router
            .partitions()
            .iter()
            .map(|p| {
                keyspaces
                    .get(&p.id)
                    .map_or(0, |ks| ks.iter().count() as u64)
            })
            .collect();
        router.replace_counts(counts);
    }
}

/// meta 分区行键 = 前缀字节 + start_key。
#[must_use]
pub fn partition_meta_key(start: &[u8]) -> Vec<u8> {
    let mut k = Vec::with_capacity(1 + start.len());
    k.push(crate::router::META_PARTITION_PREFIX);
    k.extend_from_slice(start);
    k
}
