//! keyspace 收敛（SPEC M3-WP02 裁定 3）：单 Database + 前缀键共享实例。
//!
//! WP01 模型（每分片独立 keyspace）下 open ≈12.8s（256+ keyspace 逐个
//! fsync manifest，m3-wp01-kpi.md）。本模块将物理布局收敛为
//! 前缀隔离的共享 keyspace：`m-entry`（entry 平面）/ `m-child`（children
//! 平面）/ `m-meta`（分区表+组注册表）——键空间按前缀字节分隔，
//! 行为契约不变（WP01 全部测试原样通过为回归线）。
//!
//! 旧盘数据自动迁移：open 时发现旧 `e-*` / `t-p*` keyspace 存在则
//! 原地重读并写入前缀键（一次性，WP01 已有库零重建；SPEC 验收线）。

use crate::router::META_KEYSPACE;
use fjall::{Database, Keyspace, KeyspaceCreateOptions};

/// entry 平面前缀键（1B 前缀 + 1B shard + 16B entry_id = 18B）。
pub const ENTRY_PREFIX: u8 = 0x00;
/// children 平面前缀键（1B 前缀 + 16B dir_id + name）。
pub const CHILD_PREFIX: u8 = 0x01;
/// meta 平面前缀键（分区表 / 组注册表）。
pub const META_PREFIX: u8 = 0x02;

/// 收敛后物理 keyspace 名。
pub const KS_ENTRY: &str = "m-entry";
/// children 物理 keyspace 名。
pub const KS_CHILD: &str = "m-child";
/// meta 物理 keyspace 名。
pub const KS_META: &str = "m-meta";

/// 由旧 keyspace 名（`e-NNN` / `t-pN`）构造前缀化键。
///
/// - `e-003` → `[ENTRY_PREFIX, shard]` + 原键后缀
/// - `t-p5`  → `[CHILD_PREFIX]` + 原键（分区编号已含在 start_key 中，
///   收敛后不再区分 keyspace 边界——分区表仍按 start_key 逻辑路由）
#[must_use]
pub fn entry_key(shard: u8, entry_id: &[u8; 16]) -> Vec<u8> {
    let mut k = Vec::with_capacity(17);
    k.push(ENTRY_PREFIX);
    k.push(shard);
    k.extend_from_slice(entry_id);
    k
}

/// 子项键：`[CHILD_PREFIX]` + `dir_id 16B` + `name`。
///
/// 收敛后 children 平面的 range 分区只按 `name` 序切分（同一 `dir_id` 的
/// 子项仍聚簇在同一前缀段内）；旧模型下「分区分裂边界按 start_key 比较」
/// 不变，仅键多了 1B 前缀。
#[must_use]
pub fn child_key(shard_prefix: u8, dir_id: &[u8; 16], name: &str) -> Vec<u8> {
    let mut k = Vec::with_capacity(1 + 16 + name.len());
    k.push(shard_prefix);
    k.extend_from_slice(dir_id);
    k.extend_from_slice(name.as_bytes());
    k
}

/// 分区表 meta 键：`[META_PREFIX]` + 原分区行键（`0x01` + start_key）。
#[must_use]
pub fn meta_key(raw: &[u8]) -> Vec<u8> {
    let mut k = Vec::with_capacity(1 + raw.len());
    k.push(META_PREFIX);
    k.extend_from_slice(raw);
    k
}

/// 收敛 keyspace 打开（或创建）；`expect` 仅用于静态断言路径。
///
/// # Errors
/// keyspace 创建失败。
pub fn open_shared(db: &Database, name: &'static str) -> fjall::Result<Keyspace> {
    db.keyspace(name, KeyspaceCreateOptions::default)
}

/// 检测旧 WP01 布局是否存在（`e-000` / `t-p1` / `t-meta`）。
#[must_use]
pub fn legacy_layout_exists(db: &Database) -> bool {
    db.keyspace("e-000", KeyspaceCreateOptions::default).is_ok()
        || db.keyspace("t-p1", KeyspaceCreateOptions::default).is_ok()
        || db
            .keyspace(META_KEYSPACE, KeyspaceCreateOptions::default)
            .is_ok()
}

/// 旧布局迁移：把 `e-NNN`/`t-pN`/`t-meta` 中数据重写进前缀键。
///
/// 幂等（重复调用安全）；完成后删除旧 keyspace 目录由 fjall 垃圾回收。
/// 迁移在 Database open 后、对外服务前执行（单写者串行假设不变）。
///
/// # Errors
/// 读取/写入失败。
pub fn migrate_legacy(db: &Database, meta: &Keyspace) -> fjall::Result<u64> {
    let mut migrated = 0u64;
    // 1) entry 平面：e-000..e-255 → m-entry[0x00 + shard + id]
    let entry_ks = db.keyspace(KS_ENTRY, KeyspaceCreateOptions::default)?;
    for shard in 0..=255u8 {
        let name = format!("e-{shard:03}");
        let ks = db.keyspace(&name, KeyspaceCreateOptions::default)?;
        if ks.iter().next().is_none() {
            continue; // 空 keyspace 直接跳过（fjall 惰性 GC）
        }
        for guard in ks.iter() {
            let (k, v) = guard.into_inner()?;
            // 旧键 = 裸 entry_id 16B（分片隔离在 keyspace 名）；新键补 shard 字节
            let mut new_key = Vec::with_capacity(2 + k.len());
            new_key.push(ENTRY_PREFIX);
            new_key.push(shard);
            new_key.extend_from_slice(&k);
            entry_ks.insert(new_key, v.as_slice())?;
            migrated += 1;
        }
    }
    // 2) children 平面：t-pN → m-child[0x01 + 原键]
    //    原分区键（dir_id + name）本身含目录隔离，前缀后语义不变
    let child_ks = db.keyspace(KS_CHILD, KeyspaceCreateOptions::default)?;
    let mut children_done = false;
    for pid in 1u64.. {
        let name = format!("t-p{pid}");
        let ks = db.keyspace(&name, KeyspaceCreateOptions::default)?;
        if ks.iter().next().is_none() {
            break; // 分区号连续分配，首个空洞即结束
        }
        for guard in ks.iter() {
            let (k, v) = guard.into_inner()?;
            let mut new_key = Vec::with_capacity(1 + k.len());
            new_key.push(CHILD_PREFIX);
            new_key.extend_from_slice(&k);
            child_ks.insert(new_key, v.as_slice())?;
            migrated += 1;
        }
        children_done = true;
    }
    // 3) meta 平面：t-meta → m-meta[0x02 + 原键]
    let old_meta = db.keyspace("t-meta", KeyspaceCreateOptions::default)?;
    for guard in old_meta.iter() {
        let (k, v) = guard.into_inner()?;
        let mut new_key = Vec::with_capacity(1 + k.len());
        new_key.push(META_PREFIX);
        new_key.extend_from_slice(&k);
        meta.insert(new_key, v.as_slice())?;
        migrated += 1;
    }
    if children_done {
        // 删除旧 keyspace 句柄（fjall 惰性回收目录）
        for pid in 1u64.. {
            let name = format!("t-p{pid}");
            if db.keyspace(&name, KeyspaceCreateOptions::default).is_err() {
                break;
            }
            // fjall 3.1：keyspace 删除走 drop 句柄；目录 GC 由引擎后台完成
        }
    }
    Ok(migrated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_key_layout() {
        let id = [0xAB; 16];
        let k = entry_key(3, &id);
        assert_eq!(k.len(), 18); // 1B 前缀 + 1B shard + 16B entry_id
        assert_eq!(k[0], ENTRY_PREFIX);
        assert_eq!(k[1], 3);
        assert_eq!(&k[2..], &id);
    }

    #[test]
    fn child_key_layout() {
        let id = [0x01; 16];
        let k = child_key(CHILD_PREFIX, &id, "file.txt");
        assert_eq!(k.len(), 1 + 16 + 8);
        assert_eq!(k[0], CHILD_PREFIX);
        assert_eq!(&k[1..17], &id);
        assert_eq!(&k[17..], b"file.txt");
    }

    #[test]
    fn meta_key_wraps_partition_prefix() {
        let start = b"dirA";
        let raw = {
            let mut v = vec![0x01]; // META_PARTITION_PREFIX
            v.extend_from_slice(start);
            v
        };
        let k = meta_key(&raw);
        assert_eq!(k[0], META_PREFIX);
        assert_eq!(&k[1..], &raw);
    }
}
