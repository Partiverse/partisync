//! ChunkPlan（SPEC M2-WP05 契约 §3）：块清单差分 + 路由式执行。
//!
//! 核心算法（调研方案 §5.8「泛化 delta」）：
//!   from = 源端持有块清单（升序）；to = 目标端持有块清单（升序）
//!   Need  = from - to    （目标缺，需推送）
//!   Extra = to - from    （目标多余，v1 保留——回收归 WP08）
//!   Have  = from ∩ to    （双方都有，跳过）
//!
//! 退化路径：from 或 to 的 chunk_root 为空字符串 ⇒ 整文件传送（Need = from 全部）。

use std::collections::BTreeSet;

use partisync_core::error::{PartisyError, Severity};

/// 块在 ChunkPlan 中的角色。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkRole {
    /// 目标端缺此块，需推送
    Need,
    /// 双方都有，跳过
    Have,
    /// 目标端多余（v1 保留，不主动回收）
    Extra,
}

/// 单条块的角色分类结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifiedChunk {
    pub hash: String,
    pub role: ChunkRole,
}

/// ChunkPlan 整体输出。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChunkPlan {
    pub from_root: String,
    pub to_root: String,
    pub classified: Vec<ClassifiedChunk>,
    /// 任一侧 chunk_root 缺失 ⇒ 退化路径生效（from 全部块视为 Need）
    pub degraded: bool,
}

impl ChunkPlan {
    #[must_use]
    pub fn need_count(&self) -> usize {
        self.classified
            .iter()
            .filter(|c| c.role == ChunkRole::Need)
            .count()
    }
    #[must_use]
    pub fn have_count(&self) -> usize {
        self.classified
            .iter()
            .filter(|c| c.role == ChunkRole::Have)
            .count()
    }
    #[must_use]
    pub fn extra_count(&self) -> usize {
        self.classified
            .iter()
            .filter(|c| c.role == ChunkRole::Extra)
            .count()
    }
}

/// 计算 ChunkPlan。
///
/// # Panics
/// 无（输入非法时不 panic）。
#[must_use]
pub fn plan_chunks(from: (&str, &[String]), to: (&str, &[String])) -> ChunkPlan {
    let (from_root, from_blocks) = from;
    let (to_root, to_blocks) = to;
    let degraded = from_root.is_empty() || to_root.is_empty();
    let from_set: BTreeSet<&String> = from_blocks.iter().collect();
    let to_set: BTreeSet<&String> = to_blocks.iter().collect();
    let mut classified = Vec::with_capacity(from_blocks.len() + to_blocks.len());
    // from 视角
    for h in from_blocks {
        let role = if degraded || !to_set.contains(h) {
            ChunkRole::Need
        } else {
            ChunkRole::Have
        };
        classified.push(ClassifiedChunk {
            hash: h.clone(),
            role,
        });
    }
    // to 独占 = Extra
    for h in to_blocks {
        if !from_set.contains(h) {
            classified.push(ClassifiedChunk {
                hash: h.clone(),
                role: ChunkRole::Extra,
            });
        }
    }
    // 排序输出（确定性——便于测试与 diff）
    classified.sort_by(|a, b| a.hash.cmp(&b.hash));
    ChunkPlan {
        from_root: from_root.to_string(),
        to_root: to_root.to_string(),
        classified,
        degraded,
    }
}

/// Plan 执行统计。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanStats {
    pub pushed: u64,
    pub skipped: u64,
}

/// 路由 ChunkPlan 到 sender 回调——回调一次代表一次块推送。
/// sender 闭包由调用方提供（iroh-blobs 流 / S3 MPU 部分 / hub QUIC 等）。
///
/// # Errors
/// sender 闭包返回 Err → 整批拒绝（Fatal——不允许半推，调用方可重试）。
pub async fn execute_plan<F>(plan: &ChunkPlan, mut sender: F) -> Result<PlanStats, PartisyError>
where
    F: FnMut(&str) -> Result<(), PartisyError>,
{
    let mut stats = PlanStats::default();
    for c in &plan.classified {
        match c.role {
            ChunkRole::Need => {
                sender(&c.hash).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("ChunkPlan sender 失败 (hash={}): {e}", c.hash).into()),
                })?;
                stats.pushed += 1;
            }
            ChunkRole::Have | ChunkRole::Extra => {
                stats.skipped += 1;
            }
        }
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(arr: &[&str]) -> Vec<String> {
        arr.iter().map(|x| (*x).to_string()).collect()
    }

    #[test]
    fn plan_basic_diff() {
        let from_blocks = s(&["H1", "H2", "H3"]);
        let to_blocks = s(&["H2", "H3", "H4"]);
        let plan = plan_chunks(("R1", from_blocks.as_slice()), ("R2", to_blocks.as_slice()));
        assert!(!plan.degraded);
        assert_eq!(plan.need_count(), 1); // H1
        assert_eq!(plan.have_count(), 2); // H2, H3
        assert_eq!(plan.extra_count(), 1); // H4
        let needs: Vec<_> = plan
            .classified
            .iter()
            .filter(|c| c.role == ChunkRole::Need)
            .map(|c| c.hash.as_str())
            .collect();
        assert_eq!(needs, vec!["H1"]);
    }

    #[test]
    fn plan_degraded_when_root_missing() {
        let from_blocks = s(&["H1", "H2"]);
        let to_blocks = s(&["H2"]);
        let plan = plan_chunks(("", from_blocks.as_slice()), ("R", to_blocks.as_slice()));
        assert!(plan.degraded);
        // 退化：from 全部 Need，to 独占（来自 to 而 from 无）= Extra
        assert_eq!(plan.need_count(), 2);
        assert_eq!(plan.have_count(), 0);
        assert_eq!(plan.extra_count(), 0); // 退化路径下 from 的全量 Need
    }

    #[test]
    fn plan_disjoint_from_to() {
        let from_blocks = s(&["HA", "HB"]);
        let to_blocks = s(&["HC", "HD"]);
        let plan = plan_chunks(("R1", from_blocks.as_slice()), ("R2", to_blocks.as_slice()));
        assert_eq!(plan.need_count(), 2);
        assert_eq!(plan.have_count(), 0);
        assert_eq!(plan.extra_count(), 2);
    }

    #[tokio::test]
    async fn execute_plan_calls_sender_for_need_only() {
        let from_blocks = s(&["H1", "H2", "H3"]);
        let to_blocks = s(&["H2"]);
        let plan = plan_chunks(("R1", from_blocks.as_slice()), ("R2", to_blocks.as_slice()));
        let mut calls: Vec<String> = Vec::new();
        let stats = execute_plan(&plan, |h| {
            calls.push(h.to_string());
            Ok(())
        })
        .await
        .unwrap();
        assert_eq!(stats.pushed, 2); // H1, H3
        assert_eq!(stats.skipped, 1); // H2 (Have)
        assert_eq!(calls.len(), 2);
        assert!(calls.contains(&"H1".to_string()));
        assert!(calls.contains(&"H3".to_string()));
    }

    #[tokio::test]
    async fn execute_plan_propagates_sender_error() {
        let from_blocks = s(&["H1"]);
        let to_blocks: Vec<String> = vec![];
        let plan = plan_chunks(("R1", from_blocks.as_slice()), ("R2", to_blocks.as_slice()));
        let err = execute_plan(&plan, |_| -> Result<(), PartisyError> {
            Err(PartisyError {
                severity: Severity::Fatal,
                source: Some("simulated network drop".into()),
            })
        })
        .await
        .unwrap_err();
        assert!(err.to_string().contains("simulated"));
    }
}
