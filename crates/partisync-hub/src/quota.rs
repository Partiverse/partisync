//! 空间配额（M8-WP03-T02，SPEC M8-WP03 §2.2）。
//!
//! v0.1 计量域 = hub 数据平面整体（单空间部署形态；per-space 归属
//! 需 entry→space 映射，随 T03 多租户演化）。计量口径 = **逻辑字节**
//! （EntryRow.size 聚合，非 EC/pack 物理放大）。状态（limit/used/
//! soft_alerted）由状态机 apply 单写维护并逐笔落盘 `q-meta`；
//! `used` 在 Put（含覆盖写差额）/Remove 时增量更新。
//!
//! 拒绝语义：SM apply 侧权威判定（limit 满 → 响应位 0x02、无状态
//! 变更），门面在提交前另做乐观快查（省一次 raft 往返）。软告警
//! 阈值默认 80%，跨线触发一次（audit 行），回落复位后可再触发。

/// 配额状态 keyspace（行 `state`）。
pub const KS_QUOTA: &str = "q-meta";
/// 软告警阈值（百分比）。
pub const SOFT_ALERT_PCT: u64 = 80;

/// 配额状态（apply 单写；`q-meta`/`state` JSON）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub struct QuotaState {
    /// 硬限字节（None = 不限量）。
    pub limit_bytes: Option<u64>,
    /// 已用逻辑字节（活跃行 size 聚合）。
    pub used_bytes: u64,
    /// 软告警已触发（回落阈值后复位）。
    pub soft_alerted: bool,
}

impl QuotaState {
    /// apply 侧权威硬限判定：`used + delta > limit` → true（delta 可为负）。
    pub fn would_exceed(&self, delta: i64) -> bool {
        match self.limit_bytes {
            None => false,
            Some(limit) => {
                let next = self.used_bytes as i64 + delta;
                next > limit as i64
            }
        }
    }

    /// 软告警跨线判定（含回落复位）。
    pub fn update_alert(&mut self) -> bool {
        match self.limit_bytes {
            Some(limit) if limit > 0 => {
                let pct = self.used_bytes * 100 / limit;
                if pct >= SOFT_ALERT_PCT {
                    if self.soft_alerted {
                        false
                    } else {
                        self.soft_alerted = true;
                        true
                    }
                } else {
                    self.soft_alerted = false;
                    false
                }
            }
            _ => false,
        }
    }
}

/// 状态行 key。
pub const STATE_KEY: &[u8] = b"state";

/// 读取状态（无行 = 默认不限量）。
///
/// # Errors
/// 引擎读取或解码失败。
pub fn read_state(db: &fjall::Database) -> Result<QuotaState, fjall::Error> {
    let ks = db.keyspace(KS_QUOTA, fjall::KeyspaceCreateOptions::default)?;
    read_ks(&ks)
}

/// keyspace 句柄版读取（SM/门面共用）。
///
/// # Errors
/// 引擎读取或解码失败。
pub fn read_ks(ks: &fjall::Keyspace) -> Result<QuotaState, fjall::Error> {
    match ks.get(STATE_KEY)? {
        Some(v) => Ok(serde_json::from_slice(&v).unwrap_or_default()),
        None => Ok(QuotaState::default()),
    }
}

/// 写状态（SM apply 专用）。
///
/// # Errors
/// 引擎写入失败。
pub fn write_state(ks: &fjall::Keyspace, st: &QuotaState) -> Result<(), fjall::Error> {
    let json = serde_json::to_vec(st)
        .map_err(|e| fjall::Error::Io(std::io::Error::other(e.to_string())))?;
    ks.insert(STATE_KEY, json)
}
