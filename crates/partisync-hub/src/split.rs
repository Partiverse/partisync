//! 动态分裂协议与崩溃恢复（SPEC M3-WP01 §3）。
//!
//! 协议不变量：**分区元数据先行 + 搬移原子批**——任一步崩溃后：
//! - meta 原子批要么整体生效要么不生效（新分区行存在 ⇔ splitting 态存在）；
//! - 搬移批每批原子（insert 目标 + remove 源同批），键恒在且仅在一侧；
//! - 恢复方向确定：见 splitting 分区行 → 键序前驱为源 → 重放搬移至清空 →
//!   置 active。半分裂状态不外泄（单写者串行 + open 先恢复后服务）。

use fjall::{Keyspace, OwnedWriteBatch};
use std::ops::Bound;

use crate::entry_plane::HubError;
use crate::router::{META_NEXT_PID, STATUS_ACTIVE, STATUS_SPLITTING};
use crate::tree_plane::{TreePlane, MOVE_BATCH};

/// 崩溃注入（failpoint，测试专用，WP01-T05 崩溃一致性验收）：布防点命中
/// 计数到达阈值即 panic，模拟进程死亡后由 open 恢复路径收尾。
///
/// 布防表为 **thread-local**：分裂协议与恢复全部运行在调用线程（v0.1 单写者），
/// 同线程布防/命中天然配对；并行测试互不可见，无需互斥。未布防时每次命中
/// 仅一次线程局部表查询——分裂协议低频（默认 4M 行/次），成本可忽略；
/// 除本文件三处协议点外禁止在其他路径布防。
pub mod failpoint {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static ARMED: RefCell<HashMap<&'static str, usize>> = RefCell::new(HashMap::new());
    }

    /// 注入点：分裂协议「元数据先行」原子批提交之后、目标 keyspace 创建前。
    pub const PT_META_COMMITTED: &str = "split::meta_committed";
    /// 注入点：搬移批提交之后（含恢复重放——两条路径共用 move_keys）。
    pub const PT_MOVE_BATCH: &str = "split::move_batch";
    /// 注入点：搬移完成之后、置 active 原子批提交之前。
    pub const PT_MOVES_DONE: &str = "split::moves_done";

    /// 布防：本线程 `point` 第 `panic_on_hit` 次命中时 panic（1 = 首次命中即崩）。
    pub fn arm(point: &'static str, panic_on_hit: usize) {
        ARMED.with(|armed| {
            armed.borrow_mut().insert(point, panic_on_hit.max(1));
        });
    }

    /// 撤除本线程全部布防。
    pub fn disarm_all() {
        ARMED.with(|armed| armed.borrow_mut().clear());
    }

    /// 命中计数；到达布防阈值则注入崩溃（先撤自身布防再 panic）。
    pub(crate) fn hit(point: &'static str) {
        let fire = ARMED.with(|armed| {
            let mut armed = armed.borrow_mut();
            match armed.get_mut(point) {
                Some(remain) => {
                    *remain = remain.saturating_sub(1);
                    let fire = *remain == 0;
                    if fire {
                        armed.remove(point);
                    }
                    fire
                }
                None => false,
            }
        });
        if fire {
            panic!("failpoint: injected crash at {point}");
        }
    }
}

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
        failpoint::hit(failpoint::PT_META_COMMITTED);
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
        failpoint::hit(failpoint::PT_MOVES_DONE);

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
    ///
    /// 搬移游标自批尾续扫（排除上批末键）——总扫描 O(M)。v0.1 每批从 `start`
    /// 重开迭代器为 O(M²/B)：10⁷ 实测单次分裂停顿 ~100-200s 且随搬移量
    /// 超线性增长（M3-WP01-T06 基准报告，优化留痕），修正后分裂为秒级。
    /// 崩溃语义不变：批仍原子，恢复重放幂等（已搬走的键不在源中，扫描跳过）。
    fn move_keys(
        &self,
        src: &Keyspace,
        target: &Keyspace,
        start: &[u8],
        end: Option<&[u8]>,
    ) -> Result<u64, HubError> {
        let mut moved: u64 = 0;
        let mut lo = Bound::Included(start.to_vec());
        loop {
            let hi: Bound<Vec<u8>> = match end {
                None => Bound::Unbounded,
                Some(e) => Bound::Excluded(e.to_vec()),
            };
            let mut chunk = Vec::with_capacity(MOVE_BATCH);
            for guard in src.range((lo.clone(), hi)) {
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
            failpoint::hit(failpoint::PT_MOVE_BATCH);
            moved += chunk.len() as u64;
            lo = Bound::Excluded(chunk[chunk.len() - 1].0.clone());
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

#[cfg(test)]
mod tests {
    use super::failpoint;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// 单测试串行覆盖布防/命中/撤防三种形态（同一测试线程内顺序执行）。
    #[test]
    fn failpoint_arm_hit_disarm() {
        // 未布防命中：直通无害
        failpoint::hit("split::unit-unarmed");
        // 阈值计数：第 2 次命中注入崩溃
        failpoint::arm("split::unit-armed", 2);
        failpoint::hit("split::unit-armed");
        let fired = catch_unwind(AssertUnwindSafe(|| failpoint::hit("split::unit-armed")));
        assert!(fired.is_err(), "armed hit must panic on threshold");
        // 撤防后命中：直通
        failpoint::arm("split::unit-disarmed", 1);
        failpoint::disarm_all();
        failpoint::hit("split::unit-disarmed");
    }
}
