//! Hub 节点密钥存储（ADR-0015 裁定 5 扩展 / M4-WP04-T03）。
//!
//! 使用 fjall keyspace `h-iroh-node` 持久化本 hub 的 iroh 节点私钥
//! （Base64 编码，键名 `sk`）。

use std::sync::Arc;

use fjall::Keyspace;

use partisync_core::error::{PartisyError, Severity};

use crate::iroh_channel::HubIrohKeyspace;

/// hub 节点密钥 fjall keyspace 常量。
pub const KS_IROH_NODE: &str = "h-iroh-node";
/// 节点私钥的键名（keyspace 内单键）。
const SK_KEY: &str = "sk";

/// Hub 节点密钥存储实现（ADR-0015 裁定 5 扩展 / M4-WP04-T03）。
///
/// 使用单 keyspace `h-iroh-node`，键 `sk` 存 Base64 编码的 iroh SecretKey。
///
/// # 线程安全
/// `fjall::Keyspace` 是 `Send + Sync`（读时无锁，写时 fjall 内部锁），
/// 满足 `HubIrohKeyspace: Send + Sync` 约束。
pub struct HubIrohKeyspaceImpl {
    ks: Keyspace,
}

impl std::fmt::Debug for HubIrohKeyspaceImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HubIrohKeyspaceImpl").finish()
    }
}

impl HubIrohKeyspaceImpl {
    /// 在既有 Database 上打开 hub 节点密钥 keyspace（bootstrap 幂等）。
    ///
    /// # Errors
    /// keyspace 打开失败。
    pub fn open(db: &Arc<fjall::Database>) -> Result<Self, PartisyError> {
        let ks = db
            .keyspace(KS_IROH_NODE, fjall::KeyspaceCreateOptions::default)
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("open iroh-node keyspace: {e}").into()),
            })?;
        Ok(Self { ks })
    }
}

impl HubIrohKeyspace for HubIrohKeyspaceImpl {
    fn get_node_sk(&self) -> Result<Option<String>, PartisyError> {
        match self.ks.get(SK_KEY) {
            Ok(Some(v)) => {
                let s = String::from_utf8(v.to_vec()).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("invalid utf8 in sk value: {e}").into()),
                })?;
                Ok(Some(s))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("keyspace get: {e}").into()),
            }),
        }
    }

    fn put_node_sk(&self, sk: &str) -> Result<(), PartisyError> {
        self.ks
            .insert(SK_KEY, sk.as_bytes())
            .map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("keyspace insert: {e}").into()),
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{HubIrohKeyspace, HubIrohKeyspaceImpl};

    fn temp_db() -> (Arc<fjall::Database>, std::path::PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("partisync-hub-ks-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let db = Arc::new(fjall::Database::open(fjall::Config::new(&dir)).unwrap());
        (db, dir)
    }

    #[tokio::test]
    async fn keyspace_returns_none_when_missing() {
        let (db, _dir) = temp_db();
        let ks = HubIrohKeyspaceImpl::open(&db).unwrap();
        let result = ks.get_node_sk().unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn keyspace_roundtrip() {
        let (db, _dir) = temp_db();
        let ks = HubIrohKeyspaceImpl::open(&db).unwrap();
        ks.put_node_sk("SGVsbG8gV29ybGQ=").unwrap();

        let retrieved = ks.get_node_sk().unwrap();
        assert_eq!(retrieved, Some("SGVsbG8gV29ybGQ=".to_string()));
    }

    #[tokio::test]
    async fn keyspace_overwrites_sk() {
        let (db, _dir) = temp_db();
        let ks = HubIrohKeyspaceImpl::open(&db).unwrap();
        ks.put_node_sk("first").unwrap();
        ks.put_node_sk("second").unwrap();

        let retrieved = ks.get_node_sk().unwrap();
        assert_eq!(retrieved, Some("second".to_string()));
    }
}
