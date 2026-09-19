//! M2-WP05 验收测试（SPEC 验收标准）：plan_chunks 三类块 + 退化路径 + 路由统计。

use partisync_transfer::chunk_plan::{execute_plan, plan_chunks, ChunkRole};

fn blocks(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

#[test]
fn need_have_extra_disjoint_classification() {
    let from = blocks(&["H1", "H2", "H3"]);
    let to = blocks(&["H2", "H3", "H4"]);
    let plan = plan_chunks(("R1", &from), ("R2", &to));
    assert!(!plan.degraded);
    let need: Vec<_> = plan
        .classified
        .iter()
        .filter(|c| c.role == ChunkRole::Need)
        .map(|c| c.hash.clone())
        .collect();
    let have: Vec<_> = plan
        .classified
        .iter()
        .filter(|c| c.role == ChunkRole::Have)
        .map(|c| c.hash.clone())
        .collect();
    let extra: Vec<_> = plan
        .classified
        .iter()
        .filter(|c| c.role == ChunkRole::Extra)
        .map(|c| c.hash.clone())
        .collect();
    assert_eq!(need, vec!["H1".to_string()]);
    assert_eq!(have, vec!["H2".to_string(), "H3".to_string()]);
    assert_eq!(extra, vec!["H4".to_string()]);
}

#[test]
fn plan_degrades_when_either_root_empty() {
    // 退化：from 全部 Need；to 独占视为 Have（不在 from 中）；退化标记退化。
    let from = blocks(&["H1", "H2"]);
    let to = blocks(&["H2"]);
    let plan = plan_chunks(("", &from), ("R", &to));
    assert!(plan.degraded);
    assert_eq!(plan.need_count(), 2, "退化：from 全部 Need");
    assert_eq!(plan.have_count(), 0);
    assert_eq!(
        plan.extra_count(),
        0,
        "退化：to 独占在退化路径下不上报 Extra"
    );
}

#[test]
fn equal_chunk_roots_implies_full_overlap() {
    let blocks = blocks(&["H1", "H2", "H3"]);
    let plan = plan_chunks(("R", &blocks), ("R", &blocks));
    assert_eq!(plan.need_count(), 0);
    assert_eq!(plan.have_count(), 3);
    assert_eq!(plan.extra_count(), 0);
}

#[tokio::test]
async fn execute_plan_routes_only_need_to_sender() {
    let from = blocks(&["H1", "H2", "H3"]);
    let to = blocks(&["H2"]);
    let plan = plan_chunks(("R1", &from), ("R2", &to));
    let mut pushed: Vec<String> = Vec::new();
    let stats = execute_plan(&plan, |h| {
        pushed.push(h.to_string());
        Ok(())
    })
    .await
    .unwrap();
    assert_eq!(stats.pushed, 2, "Need=H1, H3");
    assert_eq!(stats.skipped, 1, "Have=H2");
    assert!(pushed.contains(&"H1".to_string()));
    assert!(pushed.contains(&"H3".to_string()));
    assert!(!pushed.contains(&"H2".to_string()));
}

#[tokio::test]
async fn execute_plan_atomic_failure_on_sender_error() {
    let from = blocks(&["H1", "H2", "H3"]);
    let to: Vec<String> = vec![];
    let plan = plan_chunks(("R1", &from), ("R2", &to));
    let mut n_calls = 0;
    let err = execute_plan(&plan, |_| {
        n_calls += 1;
        if n_calls == 2 {
            Err(partisync_core::error::PartisyError {
                severity: partisync_core::error::Severity::Fatal,
                source: Some("second block fails".into()),
            })
        } else {
            Ok(())
        }
    })
    .await
    .unwrap_err();
    assert!(err.to_string().contains("second block fails"));
    assert_eq!(
        n_calls, 2,
        "首块成功、第二块失败后立即中止（不继续推送第三块）"
    );
}

#[tokio::test]
async fn empty_from_zero_need_zero_skipped() {
    let from: Vec<String> = vec![];
    let to = blocks(&["H1"]);
    let plan = plan_chunks(("R1", &from), ("R2", &to));
    let stats = execute_plan(&plan, |_| Ok(())).await.unwrap();
    assert_eq!(stats.pushed, 0);
    assert_eq!(stats.skipped, 1, "to 独占 = Extra");
}
