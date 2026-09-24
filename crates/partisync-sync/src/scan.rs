//! 分布式扫描调度器（SPEC M5-WP03）：目录前缀即分片、worker 池自取队列、
//! 分片账本断点恢复。
//!
//! 设计（裁定 1-8）：`ListSource`（清单源，列单层）+ [`ScanScheduler`]
//! （并行调度）+ `EntrySink`（批次消费）三件套；`impl ListSource for
//! Provider` 落本 crate（trait 本地，孤儿规则合规；sync→provider 为能力
//! 层→领域层向下依赖）。分片 = 目录前缀，worker 列单层后子目录作为新
//! 分片入队——自适应分裂即 BFS 展开本身（裁定 2），负载均衡 = 队列自取
//! （裁定 3）。断点恢复 = 分片粒度账本（裁定 4）：单层 list 为原子操作，
//! 分片即最小恢复单元。
//!
//! 与 `graph::remote_index` 的差异（裁定 6）：并行分片无全局字典序——
//! sink 须接受乱序批次（graph 写入面为幂等 upsert，乱序安全）；断点语义
//! 由分片账本承接，取代路径划界。

use std::future::Future;

use partisync_core::error::PartisyError;
use partisync_provider::Provider;

/// 单层清单项（`ListSource::list_dir` 返回；路径为 provider 内部全路径，
/// 无前导 `/`——`ProviderEntry` 同口径）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedNode {
    /// provider 内部全路径（相对 bucket/root）。
    pub path: String,
    /// 目录否。
    pub is_dir: bool,
    /// 字节大小（目录为 0）。
    pub size: u64,
    /// mtime ns（后端不提供为 0）。
    pub mtime_ns: u64,
}

impl ListedNode {
    /// 由 [`partisync_provider::ProviderEntry`] 升格（字段一一对应）。
    #[must_use]
    pub fn from_provider(e: partisync_provider::ProviderEntry) -> Self {
        Self {
            path: e.path,
            is_dir: e.is_dir,
            size: e.size,
            mtime_ns: e.mtime_ns,
        }
    }

    /// 直接父目录前缀（`""` = 根）；顶层项返回 `""`。
    #[must_use]
    pub fn parent_dir(&self) -> &str {
        match self.path.rfind('/') {
            Some(i) => &self.path[..i],
            None => "",
        }
    }
}

/// 清单源：列目录单层（裁定 1/8——层级语义由实现方吸收，调度器不直触
/// OpenDAL）。`dir = ""` 表示根。
pub trait ListSource: Send + Sync {
    /// 列 `dir` 单层子项。
    ///
    /// # Errors
    /// 源实现透传（Provider = OpenDAL 错误分类）。
    fn list_dir(
        &self,
        dir: &str,
    ) -> impl Future<Output = Result<Vec<ListedNode>, PartisyError>> + Send;
}

impl ListSource for Provider {
    async fn list_dir(&self, dir: &str) -> Result<Vec<ListedNode>, PartisyError> {
        Ok(self
            .list_children(dir)
            .await?
            .into_iter()
            .map(ListedNode::from_provider)
            .collect())
    }
}

/// 批次消费面（裁定 6）：按分片目录接收该层文件项；实现方须幂等
/// （并行 worker 乱序交付，且崩溃恢复会重放未落账本的分片）。
pub trait EntrySink: Send + Sync {
    /// 应用一批同目录文件项。
    ///
    /// # Errors
    /// 失败即 fail-fast（裁定 5：池立即停机）。
    fn apply(
        &self,
        dir: &str,
        entries: &[ListedNode],
    ) -> impl Future<Output = Result<(), PartisyError>> + Send;
}

/// 分片状态（账本持久态；在途分片不入账本——crash 后按 pending 重扫，
/// 裁定 4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardStatus {
    /// 待处理。
    Pending,
    /// 完成（账本记录）。
    Done,
    /// 失败（重试 1 次后；错误文本随行，恢复时重入队）。
    Failed {
        /// 首个错误摘要。
        error: String,
    },
}

/// 扫描累计统计（原子累积；`snapshot()` 供回调与 CLI 输出）。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ScanStats {
    /// 已完成分片数。
    pub shards_done: u64,
    /// 失败分片数（重试耗尽）。
    pub shards_failed: u64,
    /// 已交付 sink 的文件条目数。
    pub entries: u64,
    /// 发现的目录（子分片）数。
    pub dirs: u64,
}
