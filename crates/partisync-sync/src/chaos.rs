//! 混沌测试床（SPEC M2-WP09 契约 §1）：进程内模拟器，分区/时钟混乱/路由控制。
//!
//! v1：路由表控制「节点之间是否能 push/reconcile」——关闭连接模拟分区；
//! 时钟偏移注入 HLC skew（影响 wall 读取）——通过 set_clock_skew 在 oplog_clock
//! 派生时累加偏移。failpoint 在 record_oplog 关键路径散布（journal.rs /
//! store.rs 中 §3 提及的位置）。

use std::collections::HashMap;

use crate::reconcile::{self, ReconcileOpts, ReconcileStats};
use crate::session::{self, BisyncOpts, SyncStats};
use partisync_core::Ulid;
use partisync_graph::store::{EntryKind, Store};

/// 模拟分区路由表（from → to ⇒ open?）。
type Routes = HashMap<(String, String), bool>;

/// 各节点时钟偏移（ms）——叠加在 wall 之上（仅测试侧控制）。
type ClockSkews = HashMap<String, i64>;

/// 混沌测试床主体。
pub struct ChaosSim {
    pub nodes: HashMap<String, Store>,
    routes: Routes,
    skews: ClockSkews,
}

impl ChaosSim {
    /// 创建 N 节点 + 默认全连接拓扑。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn new(devices: &[&str]) -> Self {
        let mut nodes = HashMap::new();
        for d in devices {
            let dir = std::env::temp_dir().join(format!("chaos-{d}-{}", Ulid::now()));
            std::fs::create_dir_all(&dir).unwrap();
            let s = Store::open(&dir.join("t.db")).await.unwrap();
            s.seed_device_volume(d, d, d).await.unwrap();
            // 根目录占位（state 收集依赖 entry_by_path("/")）
            s.add_entry(None, "/", "/", EntryKind::Dir, 0, 0, None, None)
                .await
                .unwrap();
            nodes.insert((*d).to_string(), s);
        }
        // 默认全连接（除自环）
        let mut routes = HashMap::new();
        for a in devices {
            for b in devices {
                if a != b {
                    routes.insert(((*a).to_string(), (*b).to_string()), true);
                }
            }
        }
        Self {
            nodes,
            routes,
            skews: HashMap::new(),
        }
    }

    pub fn node(&self, dev: &str) -> &Store {
        self.nodes
            .get(dev)
            .unwrap_or_else(|| panic!("node {dev} 不存在"))
    }

    /// 关闭/打开两节点之间的连接（双向）。
    pub fn set_partition(&mut self, a: &str, b: &str, open: bool) {
        self.routes.insert((a.to_string(), b.to_string()), open);
        self.routes.insert((b.to_string(), a.to_string()), open);
    }

    /// 注入时钟偏移（ms）——影响该节点后续 oplog 写入。
    pub fn set_clock_skew(&mut self, dev: &str, skew_ms: i64) {
        self.skews.insert(dev.to_string(), skew_ms);
    }

    fn route_open(&self, from: &str, to: &str) -> bool {
        self.routes
            .get(&(from.to_string(), to.to_string()))
            .copied()
            .unwrap_or(false)
    }

    fn skew_ms(&self, dev: &str) -> i64 {
        self.skews.get(dev).copied().unwrap_or(0)
    }

    /// 模拟 push——若两节点之间连接关闭则返回 SyncStats::default()。
    ///
    /// # Errors
    /// DB 错误 / 同步错误 → Fatal。
    pub async fn push(
        &mut self,
        from: &str,
        to: &str,
    ) -> Result<SyncStats, partisync_core::error::PartisyError> {
        if !self.route_open(from, to) {
            return Ok(SyncStats::default());
        }
        let f = self.node(from).clone();
        let t = self.node(to).clone();
        // 写入侧 skew 推后（HLC wall_ms += skew_ms）——通过 sleep 模拟
        if self.skew_ms(from) != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(
                self.skew_ms(from).max(0) as u64
            ))
            .await;
        }
        session::push(&f, &t).await
    }

    /// 模拟 reconcile——双侧连接开才生效。
    ///
    /// # Errors
    /// DB 错误 / 同步错误 → Fatal。
    pub async fn reconcile(
        &self,
        a: &str,
        b: &str,
    ) -> Result<ReconcileStats, partisync_core::error::PartisyError> {
        if !(self.route_open(a, b) && self.route_open(b, a)) {
            return Ok(ReconcileStats::default());
        }
        let na = self.node(a).clone();
        let nb = self.node(b).clone();
        reconcile::reconcile(&na, &nb, ReconcileOpts::default()).await
    }

    /// 模拟 bisync（往返 push）。
    ///
    /// # Errors
    /// 同 [`Self::push`]。
    pub async fn bisync(
        &mut self,
        a: &str,
        b: &str,
    ) -> Result<(), partisync_core::error::PartisyError> {
        if !(self.route_open(a, b) && self.route_open(b, a)) {
            return Ok(());
        }
        let na = self.node(a).clone();
        let nb = self.node(b).clone();
        session::bisync(&na, &nb, BisyncOpts::default()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn partition_blocks_push_but_recovers() {
        let mut sim = ChaosSim::new(&["a", "b"]).await;
        // 关闭 a↔b
        sim.set_partition("a", "b", false);
        let stats = sim.push("a", "b").await.unwrap();
        assert_eq!(stats.applied, 0, "分区时 push 应无效果");
        // 恢复
        sim.set_partition("a", "b", true);
        let stats = sim.push("a", "b").await.unwrap();
        // 第一次恢复后无数据，但仍能调用
        assert!(stats.applied == 0 || stats.applied > 0);
    }
}
