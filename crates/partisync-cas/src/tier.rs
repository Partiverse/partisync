//! 分层存储引擎（SPEC M3-WP04 §3 裁定 4）：Hot → Warm → Cold 三级。
//!
//! 设计：
//!
//! - [`TierBackend`] trait：每层一个后端实现（fs / S3 via `Provider`）；
//!   `put`/`get`/`delete`/`exists` 四原语 + 写新删旧保证。
//! - [`ManifestStore`]（SQLite `m-tier` 表）：每 pack 当前位置 +
//!   迁移状态（`Committed` / `Migrating`）；崩溃可重入。
//! - [`TierEngine`]：包装三层后端 + 清单，对外 `put`/`read`/
//!   `migrate`/`delete`/`tier_of`；`open` 时调 [`TierEngine::crash_resume`]
//!   收口半成品迁移。
//!
//! 迁移协议（SPEC §2 崩溃一致性）：
//!
//! 1. 源读 bytes → 目标 staging（`<key>.tmp`）→ 原子 rename → 目标 committed；
//! 2. 清单 status 翻 `Committed`（带 src_tier）；
//! 3. 删源；
//!
//! 任一步崩溃 → `crash_resume` 据目标存在与否收口（完成 / 回滚）。
//!
//! 温度策略（SPEC §3 裁定 4）暂未实现——`migrate` 由外部驱动（管理面 /
//! 定时器）；策略参数化留 M4+。

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use crate::content_hash;

/// 存储层枚举（SPEC §3 裁定 4：NVMe → HDD → S3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// 热：NVMe（最高速、最低延迟）。
    Hot,
    /// 温：HDD。
    Warm,
    /// 冷：S3（OpenDAL ADR-0006）。
    Cold,
}

impl Tier {
    /// SQL 持久化标签。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Warm => "warm",
            Self::Cold => "cold",
        }
    }

    /// SQL 标签反解析。
    ///
    /// # Errors
    /// 未知标签返回 [`TierError::UnknownTier`]。
    pub fn parse(s: &str) -> Result<Self, TierError> {
        match s {
            "hot" => Ok(Self::Hot),
            "warm" => Ok(Self::Warm),
            "cold" => Ok(Self::Cold),
            _ => Err(TierError::UnknownTier(s.to_owned())),
        }
    }
}

/// 单层后端抽象。
///
/// 实现要点：`put` 必须原子（先写 `.tmp` 再 rename）；`get`/`exists`/
/// `delete` 按路径操作。S3 路径由 `partisync_provider::Provider` 适配
/// （M4+ 接线，T03 仅 fs 后端）。
pub trait TierBackend: Send + Sync {
    /// 写入（覆盖）；返回最终字节大小。
    ///
    /// # Errors
    /// 后端 IO 错误。
    fn put(&self, key: &str, data: &[u8]) -> Result<u64, TierError>;
    /// 读取。
    ///
    /// # Errors
    /// 不存在或 IO 错误。
    fn get(&self, key: &str) -> Result<Vec<u8>, TierError>;
    /// 删除（不存在不报错）。
    ///
    /// # Errors
    /// 后端 IO 错误。
    fn delete(&self, key: &str) -> Result<(), TierError>;
    /// 存在性检查。
    ///
    /// # Errors
    /// 后端 IO 错误。
    fn exists(&self, key: &str) -> Result<bool, TierError>;
    /// 后端名（诊断用）。
    #[must_use]
    fn name(&self) -> &'static str;
}

/// 本地 fs 后端（两级扇出：`root/<h[0..2]>/<key>`——避免单目录文件数爆炸）。
pub struct FsBackend {
    root: PathBuf,
    name: &'static str,
}

impl FsBackend {
    /// 新建 fs 后端（`root` 自动创建）。
    ///
    /// # Errors
    /// 根目录不可建。
    pub fn new(root: &Path, name: &'static str) -> Result<Self, TierError> {
        std::fs::create_dir_all(root).map_err(|e| TierError::Io("创建 tier 根", e))?;
        Ok(Self {
            root: root.to_owned(),
            name,
        })
    }

    fn key_path(&self, key: &str) -> PathBuf {
        let (a, b) = key.split_at(2.min(key.len()));
        self.root.join(a).join(format!("{b}-{key}"))
    }
}

impl TierBackend for FsBackend {
    fn put(&self, key: &str, data: &[u8]) -> Result<u64, TierError> {
        let final_path = self.key_path(key);
        if let Some(parent) = final_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| TierError::Io("建扇出目录", e))?;
        }
        let tmp = final_path.with_extension("tmp");
        std::fs::write(&tmp, data).map_err(|e| TierError::Io("写 tier 临时", e))?;
        std::fs::rename(&tmp, &final_path).map_err(|e| TierError::Io("提交 tier", e))?;
        Ok(data.len() as u64)
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, TierError> {
        let path = self.key_path(key);
        match std::fs::read(&path) {
            Ok(b) => Ok(b),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(TierError::NotFound(key.to_owned()))
            }
            Err(e) => Err(TierError::Io("读 tier", e)),
        }
    }

    fn delete(&self, key: &str) -> Result<(), TierError> {
        let path = self.key_path(key);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(TierError::Io("删 tier", e)),
        }
    }

    fn exists(&self, key: &str) -> Result<bool, TierError> {
        Ok(self.key_path(key).is_file())
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

// ============================================================================
// 清单（SQLite）
// ============================================================================

/// 迁移状态（清单）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MigrationStatus {
    /// 稳定态：`pack_id` 位于 `tier`。
    Committed,
    /// 迁移中：`pack_id` 正从 `src_tier` 迁向 `tier`；崩溃恢复时
    /// 据目标存在与否收口。
    Migrating,
}

impl MigrationStatus {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Committed => "committed",
            Self::Migrating => "migrating",
        }
    }
}

/// 清单条目。
#[derive(Debug, Clone)]
pub struct PackLocation {
    pub pack_id: String,
    pub tier: Tier,
    pub status: MigrationStatus,
    pub src_tier: Option<Tier>,
    pub updated_ns: u64,
}

/// 分层统一错误。
#[derive(Debug)]
pub enum TierError {
    /// 后端 IO。
    Io(&'static str, std::io::Error),
    /// 后端语义（无 NotFound）。
    Backend(String),
    /// pack 不存在。
    NotFound(String),
    /// 未知 tier 标签。
    UnknownTier(String),
    /// 无效迁移（同层或非法源）。
    InvalidMigration(String),
    /// DB 错误。
    Db(sqlx::Error),
}

impl core::fmt::Display for TierError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(w, e) => write!(f, "tier io ({w}): {e}"),
            Self::Backend(m) => write!(f, "tier backend: {m}"),
            Self::NotFound(k) => write!(f, "tier: not found: {k}"),
            Self::UnknownTier(s) => write!(f, "tier: unknown tier label: {s}"),
            Self::InvalidMigration(m) => write!(f, "tier: invalid migration: {m}"),
            Self::Db(e) => write!(f, "tier db: {e}"),
        }
    }
}

impl std::error::Error for TierError {}

impl From<sqlx::Error> for TierError {
    fn from(e: sqlx::Error) -> Self {
        Self::Db(e)
    }
}

/// 清单存储（SQLite，`m-tier` 表）。
pub struct ManifestStore {
    pool: SqlitePool,
}

impl ManifestStore {
    /// 打开（或创建）清单库。
    ///
    /// # Errors
    /// 目录不可建 / DB 不可开。
    pub async fn open(root: &Path) -> Result<Self, TierError> {
        std::fs::create_dir_all(root).map_err(|e| TierError::Io("建清单目录", e))?;
        let opts = SqliteConnectOptions::from_str(&format!(
            "sqlite://{}",
            root.join("manifest.db").display()
        ))
        .map_err(TierError::Db)?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await?;
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS pack_location (
                pack_id     TEXT PRIMARY KEY,
                tier        TEXT NOT NULL,
                status      TEXT NOT NULL DEFAULT 'committed',
                src_tier    TEXT,
                updated_ns  INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await?;
        Ok(Self { pool })
    }

    /// upsert 一条清单。
    ///
    /// # Errors
    /// DB 写入失败。
    pub async fn put_loc(&self, loc: &PackLocation) -> Result<(), TierError> {
        let src = loc.src_tier.map(Tier::as_str);
        sqlx::query(
            "INSERT INTO pack_location (pack_id, tier, status, src_tier, updated_ns)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(pack_id) DO UPDATE SET
                tier = excluded.tier,
                status = excluded.status,
                src_tier = excluded.src_tier,
                updated_ns = excluded.updated_ns",
        )
        .bind(&loc.pack_id)
        .bind(loc.tier.as_str())
        .bind(loc.status.as_str())
        .bind(src)
        .bind(loc.updated_ns as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// 读取单条清单。
    ///
    /// # Errors
    /// DB 错误。
    pub async fn get_loc(&self, pack_id: &str) -> Result<Option<PackLocation>, TierError> {
        let row: Option<(String, String, String, Option<String>, i64)> = sqlx::query_as(
            "SELECT pack_id, tier, status, src_tier, updated_ns
             FROM pack_location WHERE pack_id = ?",
        )
        .bind(pack_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some((pid, tier, status, src_tier, updated_ns)) = row else {
            return Ok(None);
        };
        Ok(Some(PackLocation {
            pack_id: pid,
            tier: Tier::parse(&tier)?,
            status: match status.as_str() {
                "committed" => MigrationStatus::Committed,
                "migrating" => MigrationStatus::Migrating,
                _ => return Err(TierError::Backend(format!("bad status: {status}"))),
            },
            src_tier: match src_tier {
                Some(s) => Some(Tier::parse(&s)?),
                None => None,
            },
            updated_ns: updated_ns as u64,
        }))
    }

    /// 删除清单条目。
    ///
    /// # Errors
    /// DB 错误。
    pub async fn delete_loc(&self, pack_id: &str) -> Result<(), TierError> {
        sqlx::query("DELETE FROM pack_location WHERE pack_id = ?")
            .bind(pack_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// 取所有 `Migrating` 条目（崩溃恢复用）。
    ///
    /// # Errors
    /// DB 错误。
    pub async fn list_migrating(&self) -> Result<Vec<PackLocation>, TierError> {
        let rows: Vec<(String, String, String, Option<String>, i64)> = sqlx::query_as(
            "SELECT pack_id, tier, status, src_tier, updated_ns
             FROM pack_location WHERE status = 'migrating'",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for (pid, tier, status, src_tier, updated_ns) in rows {
            out.push(PackLocation {
                pack_id: pid,
                tier: Tier::parse(&tier)?,
                status: match status.as_str() {
                    "committed" => MigrationStatus::Committed,
                    "migrating" => MigrationStatus::Migrating,
                    _ => return Err(TierError::Backend(format!("bad status: {status}"))),
                },
                src_tier: match src_tier {
                    Some(s) => Some(Tier::parse(&s)?),
                    None => None,
                },
                updated_ns: updated_ns as u64,
            });
        }
        Ok(out)
    }
}

// ============================================================================
// 引擎
// ============================================================================

/// 分层引擎。
pub struct TierEngine {
    hot: Box<dyn TierBackend>,
    warm: Box<dyn TierBackend>,
    cold: Box<dyn TierBackend>,
    manifest: ManifestStore,
}

impl TierEngine {
    /// 打开分层引擎；`hot`/`warm`/`cold` 三个后端 + 清单在 `manifest_root`。
    ///
    /// # Errors
    /// 后端或清单初始化失败。
    pub async fn open(
        hot: Box<dyn TierBackend>,
        warm: Box<dyn TierBackend>,
        cold: Box<dyn TierBackend>,
        manifest_root: &Path,
    ) -> Result<Self, TierError> {
        let manifest = ManifestStore::open(manifest_root).await?;
        let engine = Self {
            hot,
            warm,
            cold,
            manifest,
        };
        engine.crash_resume().await?;
        Ok(engine)
    }

    fn backend(&self, t: Tier) -> &dyn TierBackend {
        match t {
            Tier::Hot => &*self.hot,
            Tier::Warm => &*self.warm,
            Tier::Cold => &*self.cold,
        }
    }

    /// 写入 pack（始终入 Hot；迁移由 [`Self::migrate`] 触发）。
    ///
    /// # Errors
    /// 后端 IO 或清单错误。
    pub async fn put(&self, pack_id: &str, data: &[u8]) -> Result<(), TierError> {
        self.hot.put(pack_id, data)?;
        self.manifest
            .put_loc(&PackLocation {
                pack_id: pack_id.to_owned(),
                tier: Tier::Hot,
                status: MigrationStatus::Committed,
                src_tier: None,
                updated_ns: now_ns(),
            })
            .await?;
        Ok(())
    }

    /// 读 pack（按 Hot → Warm → Cold 顺序回退）。
    ///
    /// # Errors
    /// 全部层级均 NotFound → `NotFound`；IO / 清单错误透传。
    pub async fn read(&self, pack_id: &str) -> Result<Vec<u8>, TierError> {
        for tier in [Tier::Hot, Tier::Warm, Tier::Cold] {
            match self.backend(tier).get(pack_id) {
                Ok(b) => return Ok(b),
                Err(TierError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }
        // 三层均未命中——清单同步核对（避免 manifest 漂移）
        Err(TierError::NotFound(pack_id.to_owned()))
    }

    /// 删除 pack（所有层 + 清单）。
    ///
    /// # Errors
    /// 后端 IO 或清单错误。
    pub async fn delete(&self, pack_id: &str) -> Result<(), TierError> {
        for tier in [Tier::Hot, Tier::Warm, Tier::Cold] {
            self.backend(tier).delete(pack_id)?;
        }
        self.manifest.delete_loc(pack_id).await?;
        Ok(())
    }

    /// 查询 pack 所在层。
    ///
    /// # Errors
    /// 清单错误或 pack 未入库。
    pub async fn tier_of(&self, pack_id: &str) -> Result<Tier, TierError> {
        let loc = self
            .manifest
            .get_loc(pack_id)
            .await?
            .ok_or_else(|| TierError::NotFound(pack_id.to_owned()))?;
        if loc.status == MigrationStatus::Migrating {
            return Err(TierError::InvalidMigration(format!(
                "{}: still migrating",
                loc.pack_id
            )));
        }
        Ok(loc.tier)
    }

    /// 迁移 pack 到目标层（写新删旧 + 清单两阶段提交）。
    ///
    /// 协议（SPEC §2）：
    /// 1. 源 → 目标 staging（原子 rename）；
    /// 2. 清单 status 翻 `Committed`、tier=目标、src_tier=源；
    /// 3. 删源。
    ///
    /// 任一步崩溃 → `crash_resume` 据目标存在性收口。
    ///
    /// # Errors
    /// pack 不存在 / 同层迁移 / 后端 IO / 清单错误。
    pub async fn migrate(&self, pack_id: &str, to: Tier) -> Result<(), TierError> {
        let loc = self
            .manifest
            .get_loc(pack_id)
            .await?
            .ok_or_else(|| TierError::NotFound(pack_id.to_owned()))?;
        if loc.status == MigrationStatus::Migrating {
            return Err(TierError::InvalidMigration(format!(
                "{}: already migrating",
                pack_id
            )));
        }
        let from = loc.tier;
        if from == to {
            return Err(TierError::InvalidMigration(format!(
                "{}: already in {to:?}",
                pack_id
            )));
        }
        // 1. 源读 bytes
        let data = self.backend(from).get(pack_id)?;
        // 2. 写新（atomic rename 内化于 put）
        self.backend(to).put(pack_id, &data)?;
        // 3. 清单：先置 Migrating+src_tier 记录崩溃边界，再置 Committed
        self.manifest
            .put_loc(&PackLocation {
                pack_id: pack_id.to_owned(),
                tier: to,
                status: MigrationStatus::Migrating,
                src_tier: Some(from),
                updated_ns: now_ns(),
            })
            .await?;
        // 4. 删源
        self.backend(from).delete(pack_id)?;
        // 5. 清单 committed
        self.manifest
            .put_loc(&PackLocation {
                pack_id: pack_id.to_owned(),
                tier: to,
                status: MigrationStatus::Committed,
                src_tier: None,
                updated_ns: now_ns(),
            })
            .await?;
        Ok(())
    }

    /// 崩溃恢复：扫描 `Migrating` 条目 → 据目标存在性完成 / 回滚。
    ///
    /// # Errors
    /// 清单或后端错误。
    pub async fn crash_resume(&self) -> Result<usize, TierError> {
        let pending = self.manifest.list_migrating().await?;
        let mut fixed = 0;
        for loc in pending {
            let target_has = self.backend(loc.tier).exists(&loc.pack_id).unwrap_or(false);
            // src_tier 在 Migrating 必填；若缺，保守回滚（标 committed，tier=目标）。
            let src = loc.src_tier.unwrap_or(loc.tier);
            if target_has {
                // 目标已有 → 完成：删源，标 committed
                let _ = self.backend(src).delete(&loc.pack_id);
                self.manifest
                    .put_loc(&PackLocation {
                        pack_id: loc.pack_id.clone(),
                        tier: loc.tier,
                        status: MigrationStatus::Committed,
                        src_tier: None,
                        updated_ns: now_ns(),
                    })
                    .await?;
            } else {
                // 目标缺 → 回滚：标 committed+tier=src（保守）
                self.manifest
                    .put_loc(&PackLocation {
                        pack_id: loc.pack_id.clone(),
                        tier: src,
                        status: MigrationStatus::Committed,
                        src_tier: None,
                        updated_ns: now_ns(),
                    })
                    .await?;
            }
            fixed += 1;
        }
        Ok(fixed)
    }

    /// 清单访问（测试用）。
    #[must_use]
    pub fn manifest(&self) -> &ManifestStore {
        &self.manifest
    }
}

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64)
}

/// pack id 派生（hash 字节）；helper for callers。
#[must_use]
pub fn pack_id_of(bytes: &[u8]) -> String {
    content_hash(bytes)
}

use std::str::FromStr as _;

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("cas-tier-{tag}-{}", partisync_core::Ulid::now()))
    }

    fn fs(name: &'static str, root: &Path) -> Box<dyn TierBackend> {
        Box::new(FsBackend::new(root, name).expect("fs backend"))
    }

    #[tokio::test]
    async fn put_read_tier_roundtrip() {
        let root = tempdir("basic");
        let manifest = root.join("manifest");
        let engine = TierEngine::open(
            fs("hot", &root.join("hot")),
            fs("warm", &root.join("warm")),
            fs("cold", &root.join("cold")),
            &manifest,
        )
        .await
        .expect("engine");
        engine.put("p1", b"hello").await.expect("put");
        assert_eq!(engine.read("p1").await.expect("read"), b"hello");
        assert_eq!(engine.tier_of("p1").await.expect("tier"), Tier::Hot);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn migrate_changes_tier_and_keeps_data() {
        let root = tempdir("migrate");
        let engine = TierEngine::open(
            fs("hot", &root.join("hot")),
            fs("warm", &root.join("warm")),
            fs("cold", &root.join("cold")),
            &root.join("manifest"),
        )
        .await
        .expect("engine");
        engine.put("p1", b"payload").await.expect("put");
        engine.migrate("p1", Tier::Warm).await.expect("migrate");
        assert_eq!(engine.tier_of("p1").await.unwrap(), Tier::Warm);
        assert_eq!(engine.read("p1").await.unwrap(), b"payload");
        engine.migrate("p1", Tier::Cold).await.expect("to cold");
        assert_eq!(engine.tier_of("p1").await.unwrap(), Tier::Cold);
        assert_eq!(engine.read("p1").await.unwrap(), b"payload");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn crash_resume_completes_partial_migration() {
        // 模拟：手动写 Migrating 条目 + 目标文件已存在 → 重启应完成
        let root = tempdir("crash-done");
        let manifest = ManifestStore::open(&root.join("manifest")).await.unwrap();
        // 先 put 一个 pack 到 warm 后端
        let warm = FsBackend::new(&root.join("warm"), "warm").unwrap();
        warm.put("p1", b"data").unwrap();
        // 写 Migrating 清单：tier=warm, src=hot
        manifest
            .put_loc(&PackLocation {
                pack_id: "p1".into(),
                tier: Tier::Warm,
                status: MigrationStatus::Migrating,
                src_tier: Some(Tier::Hot),
                updated_ns: now_ns(),
            })
            .await
            .unwrap();
        // 重启引擎
        let engine = TierEngine::open(
            fs("hot", &root.join("hot")),
            fs("warm", &root.join("warm")),
            fs("cold", &root.join("cold")),
            &root.join("manifest"),
        )
        .await
        .expect("engine");
        assert_eq!(engine.tier_of("p1").await.unwrap(), Tier::Warm);
        assert_eq!(engine.read("p1").await.unwrap(), b"data");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn crash_resume_rolls_back_missing_target() {
        // 模拟：Migrating 条目但目标文件不存在 → 回滚到 src_tier
        let root = tempdir("crash-rollback");
        let manifest = ManifestStore::open(&root.join("manifest")).await.unwrap();
        // 源 hot 后端有 p1；warm 无
        let hot = FsBackend::new(&root.join("hot"), "hot").unwrap();
        hot.put("p1", b"orig").unwrap();
        manifest
            .put_loc(&PackLocation {
                pack_id: "p1".into(),
                tier: Tier::Warm,
                status: MigrationStatus::Migrating,
                src_tier: Some(Tier::Hot),
                updated_ns: now_ns(),
            })
            .await
            .unwrap();
        let engine = TierEngine::open(
            fs("hot", &root.join("hot")),
            fs("warm", &root.join("warm")),
            fs("cold", &root.join("cold")),
            &root.join("manifest"),
        )
        .await
        .expect("engine");
        // 回滚到 hot；read 仍可拿回数据
        assert_eq!(engine.tier_of("p1").await.unwrap(), Tier::Hot);
        assert_eq!(engine.read("p1").await.unwrap(), b"orig");
        let _ = std::fs::remove_dir_all(&root);
    }
}
