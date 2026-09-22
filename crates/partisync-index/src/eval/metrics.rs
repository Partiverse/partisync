//! IR 评估指标（自写，零第三方依赖）
//!
//! 实现三种标准 IR 指标：
//! - **Recall@K**：top-K 中命中的相关文档比例
//! - **MRR**（Mean Reciprocal Rank）：第一个相关文档的倒数排名平均
//! - **nDCG@K**（normalized Discounted Cumulative Gain）：基于 relevance 等级的排序质量
//!
//! 参考：
//! - Manning et al., *Introduction to Information Retrieval*, 2008
//! - Järvelin & Kekäläinen, *Cumulated gain-based evaluation of IR techniques*, 2002

use std::collections::HashMap;

/// 单查询的指标计算结果。
#[derive(Debug, Clone, PartialEq)]
pub struct PerQueryMetrics {
    /// Recall@K（top-K 命中的相关文档 / 全部相关文档）
    pub recall_at_k: f64,
    /// MRR（第一个相关文档排名的倒数；无相关 = 0）
    pub mrr: f64,
    /// nDCG@K（0..1；越接近 1 越好）
    pub ndcg_at_k: f64,
    /// K 值
    pub k: usize,
}

impl PerQueryMetrics {
    /// 计算给定查询的指标。
    ///
    /// `retrieved` 是按相关度降序排列的 content_id 列表（前 K 个）。
    /// `relevant` 是 `content_id → relevance` 映射（relevance > 0 即相关）。
    /// relevance 0 = 不相关（不计入 qrels 但也不计入命中）。
    ///
    /// 仅 `relevance > 0` 的文档被视为相关；relevance = 1/2/3 贡献 nDCG 计算权重。
    #[must_use]
    pub fn compute(retrieved: &[String], relevant: &HashMap<String, i32>, k: usize) -> Self {
        let k = k.min(retrieved.len());
        let top_k = &retrieved[..k];

        // ── Recall@K
        let total_relevant = relevant.values().filter(|&&r| r > 0).count();
        let hits_in_top_k = top_k
            .iter()
            .filter(|cid| relevant.get(*cid).is_some_and(|&r| r > 0))
            .count();
        let recall_at_k = if total_relevant > 0 {
            hits_in_top_k as f64 / total_relevant as f64
        } else {
            0.0
        };

        // ── MRR
        let mrr = top_k
            .iter()
            .position(|cid| relevant.get(cid).is_some_and(|&r| r > 0))
            .map(|pos| 1.0 / (pos as f64 + 1.0))
            .unwrap_or(0.0);

        // ── nDCG@K
        let dcg: f64 = top_k
            .iter()
            .enumerate()
            .map(|(idx, cid)| {
                let rel = relevant.get(cid).copied().unwrap_or(0).max(0) as f64;
                // rank = idx+1 (1-based); discount = log2(rank + 1)
                let discount = ((idx + 2) as f64).log2();
                if rel > 0.0 {
                    (2.0_f64.powf(rel) - 1.0) / discount
                } else {
                    0.0
                }
            })
            .sum();

        // IDCG：完美排序（relevance 降序）
        let mut ideal_relevances: Vec<i32> =
            relevant.values().copied().filter(|&r| r > 0).collect();
        ideal_relevances.sort_by(|a, b| b.cmp(a));
        ideal_relevances.truncate(k);

        let idcg: f64 = ideal_relevances
            .iter()
            .enumerate()
            .map(|(idx, &rel)| {
                let rel = rel.max(0) as f64;
                let discount = ((idx + 2) as f64).log2();
                if rel > 0.0 {
                    (2.0_f64.powf(rel) - 1.0) / discount
                } else {
                    0.0
                }
            })
            .sum();

        let ndcg_at_k = if idcg > 0.0 { dcg / idcg } else { 0.0 };

        Self {
            recall_at_k,
            mrr,
            ndcg_at_k,
            k,
        }
    }
}

/// 指标枚举（用于聚合报告）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Metric {
    RecallAtK,
    Mrr,
    NdcgAtK,
}

impl Metric {
    /// 名称字符串（用于报告 JSON）。
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RecallAtK => "recall@K",
            Self::Mrr => "mrr",
            Self::NdcgAtK => "ndcg@K",
        }
    }

    /// 取值。
    #[must_use]
    pub fn value(self, m: &PerQueryMetrics) -> f64 {
        match self {
            Self::RecallAtK => m.recall_at_k,
            Self::Mrr => m.mrr,
            Self::NdcgAtK => m.ndcg_at_k,
        }
    }
}

/// 聚合指标报告（所有查询平均）。
#[derive(Debug, Clone)]
pub struct MetricReport {
    /// 模式名（如 "bm25_only"）
    pub mode: String,
    /// 查询数
    pub num_queries: usize,
    /// Recall@K 平均
    pub mean_recall_at_k: f64,
    /// MRR 平均
    pub mean_mrr: f64,
    /// nDCG@K 平均
    pub mean_ndcg_at_k: f64,
    /// K 值
    pub k: usize,
    /// 每查询详情
    pub per_query: Vec<(String, PerQueryMetrics)>,
}

impl MetricReport {
    /// 从 per-query 列表聚合（平均）。
    #[must_use]
    pub fn aggregate(mode: String, k: usize, per_query: Vec<(String, PerQueryMetrics)>) -> Self {
        let n = per_query.len();
        if n == 0 {
            return Self {
                mode,
                num_queries: 0,
                mean_recall_at_k: 0.0,
                mean_mrr: 0.0,
                mean_ndcg_at_k: 0.0,
                k,
                per_query,
            };
        }
        let sum_r: f64 = per_query.iter().map(|(_, m)| m.recall_at_k).sum();
        let sum_m: f64 = per_query.iter().map(|(_, m)| m.mrr).sum();
        let sum_n: f64 = per_query.iter().map(|(_, m)| m.ndcg_at_k).sum();
        let n_f = n as f64;
        Self {
            mode,
            num_queries: n,
            mean_recall_at_k: sum_r / n_f,
            mean_mrr: sum_m / n_f,
            mean_ndcg_at_k: sum_n / n_f,
            k,
            per_query,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_ranking() {
        // 完美排序：top-3 全命中且顺序正确
        let retrieved = vec!["a".into(), "b".into(), "c".into()];
        let mut relevant = HashMap::new();
        relevant.insert("a".into(), 3);
        relevant.insert("b".into(), 2);
        relevant.insert("c".into(), 1);
        let m = PerQueryMetrics::compute(&retrieved, &relevant, 3);
        assert!((m.recall_at_k - 1.0).abs() < 1e-9);
        assert!((m.mrr - 1.0).abs() < 1e-9); // 第一个就是相关
        assert!((m.ndcg_at_k - 1.0).abs() < 1e-9); // 完美排序
    }

    #[test]
    fn first_relevant_at_position_2() {
        // 第一个相关在 rank 2
        let retrieved = vec!["x".into(), "a".into(), "b".into()];
        let mut relevant = HashMap::new();
        relevant.insert("a".into(), 3);
        let m = PerQueryMetrics::compute(&retrieved, &relevant, 3);
        assert!((m.mrr - 0.5).abs() < 1e-9); // 1/2
    }

    #[test]
    fn no_relevant() {
        let retrieved = vec!["x".into(), "y".into()];
        let relevant: HashMap<String, i32> = HashMap::new();
        let m = PerQueryMetrics::compute(&retrieved, &relevant, 2);
        assert_eq!(m.recall_at_k, 0.0);
        assert_eq!(m.mrr, 0.0);
        assert_eq!(m.ndcg_at_k, 0.0);
    }

    #[test]
    fn partial_relevance_levels() {
        // 完美排序档位 3,2 → nDCG 应为 1
        // 实际排序 2,3 → nDCG 略低
        let mut relevant = HashMap::new();
        relevant.insert("a".into(), 3);
        relevant.insert("b".into(), 2);
        let retrieved_perfect = vec!["a".into(), "b".into()];
        let m1 = PerQueryMetrics::compute(&retrieved_perfect, &relevant, 2);
        assert!((m1.ndcg_at_k - 1.0).abs() < 1e-9);

        let retrieved_swapped = vec!["b".into(), "a".into()];
        let m2 = PerQueryMetrics::compute(&retrieved_swapped, &relevant, 2);
        assert!(m2.ndcg_at_k < m1.ndcg_at_k);
        assert!(m2.ndcg_at_k > 0.0);
    }
}
