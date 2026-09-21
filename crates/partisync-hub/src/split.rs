//! 动态分裂协议与崩溃恢复（SPEC M3-WP01 §3；M3-WP02 裁定 3 收敛后语义）。
//!
//! keyspace 收敛后所有分区共享 `m-child`，分裂为 **元数据-only 操作**：
//! 不再物理搬移键，只切分路由表与分区行数计数。协议不变量收缩为：
//! - 分裂 = 单次原子批：next-pid 计数器 + 新分区行（直接 active）；
//! - 崩溃窗口仅一处：批未提交 → open 恢复见 splitting 行重放收尾
//!   （幂等：重放即把同一行再置 active）；已提交 → 无 splitting 残留。
//!
//! 物理搬移随 keyspace 收敛移除（共享 keyspace 内 insert+remove 同键 =
//! 净删除，见 M3-WP02 裁定 3 推导）；WP02-T05 起搬移语义由 raft 日志
//! 复制承接（SPEC §6）。

use fjall::OwnedWriteBatch;
use std::ops::Bound;

use crate::entry_plane::HubError;
use crate::router::{
    partition_count, partition_meta_key, META_NEXT_PID, STATUS_ACTIVE, STATUS_SPLITTING,
};
use crate::tree_plane::TreePlane;

/// 崩溃注入（failpoint，测试专用，WP01-T05 崩溃一致性验收；WP02-T02 收敛后
/// 收缩为两布防点）：布防点命中计数到达阈值即 panic，模拟进程死亡后由
/// open 恢复路径收尾。
///
/// 布防表为 **thread-local**：分裂协议与恢复全部运行在调用线程（v0.1 单写者），
/// 同线程布防/命中天然配对；并行测试互不可见，无需互斥。未布防时每次命中
/// 仅一次线程局部表查询——分裂协议低频（默认 4M 行/次），成本可忽略；
/// 除本文件协议点外禁止在其他路径布防。
pub mod failpoint {
    use std::cell::RefCell;
    use std::collections::HashMap;

    /// 布防表（进程共享形态）。
    #[derive(Default)]
    struct ArmedTable {
        inner: std::collections::HashMap<&'static str, usize>,
    }

    impl ArmedTable {
        fn arm(&mut self, point: &'static str, hits: usize) {
            self.inner.insert(point, hits);
        }
        fn disarm(&mut self, point: &'static str) {
            self.inner.remove(point);
        }
        /// 返回 Some(fire) 表示本表已布防该点；None = 未布防（回落 thread-local）。
        fn hit(&mut self, point: &str) -> Option<bool> {
            let remain = self.inner.get_mut(point)?;
            *remain = remain.saturating_sub(1);
            let fire = *remain == 0;
            if fire {
                self.inner.remove(point);
            }
            Some(fire)
        }
    }

    /// 进程内共享布防表（组拓扑 failpoint 矩阵：命中发生在组内 runtime 线程）。
    static GLOBAL_ARMED: std::sync::LazyLock<std::sync::Mutex<ArmedTable>> =
        std::sync::LazyLock::new(|| {
            std::sync::Mutex::new(ArmedTable {
                inner: std::collections::HashMap::new(),
            })
        });

    thread_local! {
        static ARMED: RefCell<HashMap<&'static str, usize>> = RefCell::new(HashMap::new());
    }

    /// 注入点：分裂元数据原子批（next-pid + splitting 分区行）提交之后、
    /// 收尾前——最恶劣窗口（meta 先行已生效，目标分区未收尾）。
    pub const PT_META_COMMITTED: &str = "split::meta_committed";
    /// 注入点：分裂收尾批（splitting → active）提交之后、路由内存态更新前。
    /// 收敛后收尾与登记合批时该点在 fresh 分裂中不命中——恢复路径
    /// （`recover_splits` 置 active 批提交后）仍会命中。
    pub const PT_MOVES_DONE: &str = "split::moves_done";
    /// 注入点：恢复路径收尾批提交后——复用「搬移批」语义槽位（收敛后无
    /// 物理搬移，恢复重放的最后一个原子步即收尾批）。
    pub const PT_MOVE_BATCH: &str = "split::move_batch";

    /// 布防：本线程 `point` 第 `panic_on_hit` 次命中时 panic（1 = 首次命中即崩）。
    pub fn arm(point: &'static str, panic_on_hit: usize) {
        ARMED.with(|armed| {
            armed.borrow_mut().insert(point, panic_on_hit.max(1));
        });
    }

    /// 跨线程布防（组拓扑矩阵用）：命中计数进程内共享。
    pub fn arm_global(point: &'static str, panic_on_hit: usize) {
        GLOBAL_ARMED
            .lock()
            .expect("global armed lock")
            .arm(point, panic_on_hit.max(1));
    }

    /// 撤除指定全局布防（测试收尾）。
    pub fn disarm_global(point: &'static str) {
        GLOBAL_ARMED
            .lock()
            .expect("global armed lock")
            .disarm(point);
    }

    /// 撤除本线程全部布防。
    pub fn disarm_all() {
        ARMED.with(|armed| armed.borrow_mut().clear());
    }

    /// 命中计数；到达布防阈值则注入崩溃（先撤自身布防再 panic）。
    pub(crate) fn hit(point: &'static str) {
        // 全局布防优先（跨线程命中——raft SM apply 运行在组内 runtime 线程，
        // 与布防线程不同，T07 矩阵用）；未布防再查 thread-local（WP01 直连形态）。
        if let Some(fire) = GLOBAL_ARMED.lock().expect("global armed lock").hit(point) {
            if fire {
                panic!("failpoint: injected crash at {point} (global)");
            }
            return;
        }
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
    ///
    /// 收敛后分裂为纯元数据：一次原子批提交「splitting 分区行」即完成
    /// 键的归属切换（共享 keyspace 下 [median, next) 的键天然就位），
    /// 随后收尾批置 active + 路由内存态修正计数。
    pub(crate) fn split_partition(&self, idx: usize) -> Result<(), HubError> {
        let (src_id, start, next_start) = {
            let router = self.router.read().expect("router lock");
            let p = &router.partitions()[idx];
            (
                p.id,
                p.start.clone(),
                router.partitions().get(idx + 1).map(|n| n.start.clone()),
            )
        };

        // 1. 选键序中位边界（真实行数计；内存计数仅做触发近似）。
        //    共享 keyspace：扫本分区逻辑区间 [start, next_start)。
        let keys: Vec<Vec<u8>> = self
            .data
            .range((
                Bound::<Vec<u8>>::Included(start.clone()),
                match &next_start {
                    None => Bound::Unbounded,
                    Some(e) => Bound::Excluded(e.clone()),
                },
            ))
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
        let moved = (n - n / 2) as u64;

        // 2. 元数据先行：原子批写 next-pid 计数器 + 新分区行（splitting 态）
        let new_pid = {
            let cur = self
                .meta
                .get(crate::ksconv::meta_key(META_NEXT_PID))
                .map_err(HubError::from)?
                .map_or(1u64, |v| {
                    let mut b = [0u8; 8];
                    b.copy_from_slice(&v);
                    u64::from_be_bytes(b)
                });
            let mut batch = OwnedWriteBatch::with_capacity(self.db.clone(), 2);
            batch.insert(
                &self.meta,
                crate::ksconv::meta_key(META_NEXT_PID),
                (cur + 1).to_be_bytes(),
            );
            let mut row = Vec::with_capacity(9);
            row.extend_from_slice(&cur.to_be_bytes());
            row.push(STATUS_SPLITTING);
            batch.insert(&self.meta, partition_meta_key(&median), row);
            batch.commit().map_err(HubError::from)?;
            cur
        };
        failpoint::hit(failpoint::PT_META_COMMITTED);
        // 不变量：新 pid 不得与既有分区冲突（计数器错位将退化为同分区自切）
        assert_ne!(
            new_pid, src_id,
            "split allocated pid {new_pid} colliding with source"
        );

        // 3. 收尾：同一分区行置 active（原子批；崩溃即由恢复路径重放）
        let mut batch = OwnedWriteBatch::with_capacity(self.db.clone(), 1);
        let mut row = Vec::with_capacity(9);
        row.extend_from_slice(&new_pid.to_be_bytes());
        row.push(STATUS_ACTIVE);
        batch.insert(&self.meta, partition_meta_key(&median), row);
        batch.commit().map_err(HubError::from)?;
        // 收尾批已提交、路由内存态未更新——崩溃窗口（覆盖 WP01「搬移完成/
        // 置 active 前后」语义的收敛形态：键已归属新分区，仅内存计数滞后）
        failpoint::hit(failpoint::PT_MOVE_BATCH);
        failpoint::hit(failpoint::PT_MOVES_DONE);

        // 4. 内存路由更新：新分区插到源之后，计数按键序中位修正
        self.keyspaces
            .write()
            .expect("keyspaces lock")
            .insert(new_pid, self.data.clone());
        let mut router = self.router.write().expect("router lock");
        router.insert_split(
            idx,
            crate::router::Partition {
                start: median,
                id: new_pid,
                splitting: false,
            },
            n as u64,
            moved,
        );
        Ok(())
    }

    /// open 恢复：收尾所有 splitting 分区（meta 行已在 Router::load 读入）。
    ///
    /// 收敛后无物理搬移——恢复 = 把 splitting 行置 active（幂等重放）
    /// + 路由内存态清标记 + 全量重算计数。
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
            let mut batch = OwnedWriteBatch::with_capacity(self.db.clone(), 1);
            let mut row = Vec::with_capacity(9);
            row.extend_from_slice(&pid.to_be_bytes());
            row.push(STATUS_ACTIVE);
            batch.insert(&self.meta, partition_meta_key(&start), row);
            batch.commit().map_err(HubError::from)?;
            failpoint::hit(failpoint::PT_MOVE_BATCH);
            failpoint::hit(failpoint::PT_MOVES_DONE);
            self.router
                .write()
                .expect("router lock")
                .clear_splitting(idx);
        }
        // 恢复后计数重算（splitting 期间 bump 归源分区，区间已变）
        self.recount_all();
        Ok(())
    }

    /// 按分区逻辑区间在共享 `m-child` 上重算各行数。
    fn recount_all(&self) {
        let mut router = self.router.write().expect("router lock");
        let parts = router.partitions().to_vec();
        let counts: Vec<u64> = parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let end = parts.get(i + 1).map(|n| n.start.as_slice());
                partition_count(&self.data, &p.start, end).unwrap_or(0)
            })
            .collect();
        router.replace_counts(counts);
    }
}

/// meta 分区行键（兼容导出——收敛后为 `[0x02, 0x01] + start_key`）。
#[must_use]
pub fn partition_meta_key_compat(start: &[u8]) -> Vec<u8> {
    partition_meta_key(start)
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
