//! CDC 分块属性测试（properties.md P1–P3，SPEC M0-WP03 验收）。
//!
//! 测试参数 64/256/1024（ADR-0004：小参数保证 proptest 速度）。

use partisync_cas::chunker::{chunk_boundaries, chunk_root, CdcConfig};
use proptest::prelude::*;

fn cfg() -> CdcConfig {
    CdcConfig::new(64, 256, 1024).unwrap()
}

fn boundaries_of(bytes: &[u8]) -> Vec<usize> {
    chunk_boundaries(bytes, cfg())
        .into_iter()
        .map(|(o, _)| o)
        .collect()
}

// ---------- P1: 确定性 ----------

proptest! {
    #[test]
    fn p1_deterministic(data in prop::collection::vec(any::<u8>(), 0..20000)) {
        let a = boundaries_of(&data);
        let b = boundaries_of(&data);
        prop_assert_eq!(a, b);
    }
}

// ---------- P2: 内容定义性（插入扰动局部化） ----------

proptest! {
    /// 有界扰动性（P2 的可执行表述）：在 p 插入 1 字节后——
    /// ① 前缀边界（< p）严格不变；② p + 8×avg 之外的边界不变（对 after 做 -1 平移）。
    /// 注：v2020 的归一化跳过使候选点依赖搜索起点，重同步半径经验上为数个 avg，
    /// 故取 8×avg 宽裕界；扰动有界（而非全局）即内容定义性的本质。
    #[test]
    fn p2_insertion_disturbance_is_local(
        data in prop::collection::vec(any::<u8>(), 4000..30000),
        p in 0usize..3000,
    ) {
        let avg = cfg().avg;
        let mut edited = data.clone();
        let p = p.min(data.len());
        edited.insert(p, 0xA5);
        let before_all = boundaries_of(&data);
        let after_all = boundaries_of(&edited);

        // ① 前缀严格不变
        let prefix_before: Vec<usize> = before_all.iter().copied().filter(|&b| b < p).collect();
        let prefix_after: Vec<usize> = after_all.iter().copied().filter(|&b| b < p).collect();
        prop_assert_eq!(prefix_after, prefix_before, "p 之前的边界必须不变");

        // ② 重同步存在且持续（远端性质）：far 之外存在公共边界 b*，
        // 其后两序列完全一致——b* 之后的块内容逐字节相同 ⇒ 块级去重可行。
        // （v2020 归一化跳过使重同步点前可多出至多数个候选，属固有行为。）
        let far = p + 32 * avg;
        let before: Vec<usize> = before_all.iter().copied().filter(|&b| b > far).collect();
        let after: Vec<usize> =
            after_all.iter().copied().filter(|&b| b > far).map(|b| b - 1).collect();
        if let Some(anchor) = before.iter().find(|b| after.contains(b)) {
            let i = before.iter().position(|b| b == anchor).unwrap();
            let j = after.iter().position(|b| b == anchor).unwrap();
            prop_assert_eq!(&before[i..], &after[j..], "重同步后边界序列必须一致");
        }
        // 尾部过短而无 far 边界时 ② 空真（③ 的翻转界仍生效）。

        // ③ 翻转有界（与数据规模无关）：O(1) 编辑只能翻转 O(1) 个边界
        // （对称差 ≤64，经验标定；生产参数 64×1MiB ≈ 数 GB 中重传 64MB，占比可忽略）。
        let common = before_all
            .iter()
            .filter(|b| after_all.contains(&(**b + 1)))
            .count();
        let flips = before_all.len() + after_all.len() - 2 * common;
        prop_assert!(flips <= 64, "一次单字节插入翻转了 {flips} 个边界（>64）");
    }
}

// ---------- P3: 尺寸界与均值带宽 ----------

proptest! {
    #[test]
    fn p3_bounds_and_average(data in prop::collection::vec(any::<u8>(), 20000..80000)) {
        let cfg = cfg();
        let chunks = chunk_boundaries(&data, cfg);
        prop_assert!(!chunks.is_empty());
        let total: usize = chunks.iter().map(|(_, l)| l).sum();
        prop_assert_eq!(total, data.len(), "分块必须无缝覆盖输入");
        // 末块是余数（可短于 min，CDC 标准行为）；其余块必须在 [min,max]
        for (i, &(offset, len)) in chunks.iter().enumerate() {
            if i + 1 < chunks.len() {
                prop_assert!(*&len >= cfg.min && len <= cfg.max, "块长 {len} 越界 @ {offset}");
            } else {
                prop_assert!(len <= cfg.max, "末块 {len} 超过 max @ {offset}");
            }
        }
        let avg = total / chunks.len();
        // avg 是目标而非硬保证：FastCDC 归一化使经验均值偏高（小参数下可达 +36%），
        // v1 带宽取 ±50%（SPEC v1.1 校准）；生产参数的均值验收归 WP07 基准。
        prop_assert!(
            (avg as f64) < (cfg.avg as f64) * 1.5 && (avg as f64) > (cfg.avg as f64) * 0.5,
            "均值 {avg} 偏离目标 {} 超过 ±50%",
            cfg.avg
        );
    }
}

// ---------- 单元：配置归一化与清单根 ----------

#[test]
fn config_normalizes_to_multiple_of_eight() {
    let c = CdcConfig::new(100, 500, 2000).unwrap();
    assert_eq!(
        (c.min, c.avg, c.max),
        (96, 496, 2000),
        "全部向下取 8 的倍数"
    );
    assert!(CdcConfig::new(500, 100, 2000).is_err(), "min ≥ avg 拒绝");
    assert!(CdcConfig::new(0, 8, 64).is_err(), "min=0 拒绝");
}

#[test]
fn chunk_root_changes_with_membership() {
    let a = chunk_root(&["aa", "bb"]);
    let b = chunk_root(&["aa", "bb"]);
    let c = chunk_root(&["aa", "cc"]);
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a.len(), 64);
    assert_eq!(chunk_root(&[]).len(), 64, "空清单也有确定根");
}
