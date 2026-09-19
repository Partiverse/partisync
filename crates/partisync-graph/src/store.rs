//! 仓储层：SQLite 上的 PartiGraph 读写（SPEC M0-WP02 契约）。
//!
//! 错误分类约定：sqlx 错误一律 Fatal（库级故障重试无意义）；
//! UNIQUE 冲突（path 已存在）在 [`Store::add_entry`] 内部转为幂等语义，不外溢。

use std::path::Path;

use partisync_core::error::{PartisyError, Severity};
use partisync_core::Ulid;
use serde::Serialize;
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};

/// entry 类型（v1 仅文件/目录；符号链接在索引器中跳过，SPEC 非目标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EntryKind {
    File = 0,
    Dir = 1,
}

/// 条目行（演示面 JSON 直出）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct EntryRow {
    pub id: String,
    pub kind: i64,
    pub name: String,
    pub path: String,
    pub content_id: Option<String>,
    pub size: i64,
    pub mtime_ns: i64,
    pub chunk_root: Option<String>,
    pub owner_device: Option<String>,
    #[sqlx(default)]
    pub state: i64,
    #[sqlx(default)]
    pub content_hydrated_at_ns: Option<i64>,
    #[sqlx(default)]
    pub pin_count: i64,
}

/// entry 状态枚举（M2-WP06）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(i64)]
pub enum EntryState {
    Materialized = 0,
    Placeholder = 1,
}

impl EntryState {
    #[must_use]
    pub const fn from_i64(v: i64) -> Self {
        if v == 1 {
            Self::Placeholder
        } else {
            Self::Materialized
        }
    }
}

/// oplog 行视图（M2-WP01）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct OplogRow {
    pub hlc: String,
    pub space_id: String,
    pub domain: i64,
    pub entity: String,
    pub entity_id: String,
    pub op: String,
    pub origin_device: String,
    pub payload: String,
    pub at_ns: i64,
}

/// 远端条目应用结果（M2-WP02）：`path` 为最终落位；
/// `conflict = Some` 表示触发「保留两者」改挂（P11 血缘由 session 落档）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub path: String,
    pub conflict: Option<ApplyConflict>,
}

/// 一次冲突改挂的血缘对：双方主张的同一路径 + 来方版本的实际落位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyConflict {
    pub base_path: String,
    pub incoming_path: String,
}

/// Tag 行（M2-WP02 共享域；墓碑行不参与业务查询）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct TagRow {
    pub id: String,
    pub space_id: String,
    pub name: String,
    pub color: Option<String>,
    pub deleted: i64,
    pub updated_hlc: Option<String>,
}

/// 冲突血缘记录（P11：同名双改 → 两者皆可寻址，血缘可查）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ConflictRow {
    pub id: String,
    pub space_id: String,
    pub base_path: String,
    pub local_path: String,
    pub incoming_path: String,
    pub origin_device: String,
    pub detected_hlc: String,
    pub at_ns: i64,
}

/// 设备 id 字符串 → u64 哈希（HLC device 段；FNV-1a，无依赖确定性）。
#[must_use]
pub fn hash_u64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 全局统计（去重口径：saved = Σentry.size − Σcontent.size）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stats {
    pub files: i64,
    pub dirs: i64,
    pub total_bytes: i64,
    pub unique_contents: i64,
    pub unique_bytes: i64,
    pub duplicate_groups: i64,
    pub saved_bytes: i64,
}

/// 一组重复内容（同 content_id ≥2 个 entry）。
#[derive(Debug, Clone, Serialize)]
pub struct DupGroup {
    pub content_id: String,
    pub size: i64,
    pub copies: Vec<EntryRow>,
}

/// PartiGraph 仓储句柄。
#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
    /// oplog 时钟（M2-WP01）：进程内单调 HLC；None = 尚未初始化（首次捕获时从墙钟起步）
    oplog_clock: std::sync::Arc<tokio::sync::Mutex<Option<partisync_core::Hlc>>>,
}

fn db_err(what: &str, e: sqlx::Error) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {e}").into()),
    }
}

/// 批量文件插入记录（[`Store::add_file_batch`] 的输入）。
pub struct FileInsert {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub path: String,
    pub size: u64,
    pub mtime_ns: u64,
    pub content: Option<(String, u64)>,
    pub chunk_root: Option<String>,
}

impl Store {
    /// 供 journal/watch 等同 crate 模块直接执行 SQL 的池访问。
    pub(crate) fn pool_ref(&self) -> &SqlitePool {
        &self.pool
    }

    /// 打开（或创建）库并执行幂等迁移。
    ///
    /// # Errors
    /// 库路径不可用或迁移失败 → Fatal。
    pub async fn open(path: &Path) -> Result<Self, PartisyError> {
        let url = format!("sqlite://{}", path.display());
        let opts = SqliteConnectOptions::from_str(&url)
            .map_err(|e| db_err("连接串解析", e))?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            // WAL+NORMAL：提交不 fsync（KPI 实测 28m33s→优化，元凶是每语句 fsync）。
            // 代价：OS 断电可能丢尾部提交；索引可幂等重建，接受（SPEC M0-WP07 留痕）
            .synchronous(SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .map_err(|e| db_err("打开数据库", e))?;
        Self::migrate(&pool).await?;
        Self::attach(pool).await
    }

    /// 内存库（测试/临时索引）。单连接：内存库不跨连接共享。
    ///
    /// # Errors
    /// 同 [`Store::open`]。
    pub async fn open_in_memory() -> Result<Self, PartisyError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| db_err("打开内存库", e))?;
        Self::migrate(&pool).await?;
        Self::attach(pool).await
    }

    /// 迁移后装配：从 `sync_clock` 恢复 HLC 时钟（M2-WP03）——
    /// 重启后新键严格大于关闭前（修复 WP01 时钟随进程回退的隐患）。
    async fn attach(pool: SqlitePool) -> Result<Self, PartisyError> {
        let top: Option<String> = sqlx::query_scalar("SELECT top FROM sync_clock WHERE id = 1")
            .fetch_optional(&pool)
            .await
            .map_err(|e| db_err("恢复时钟", e))?;
        Ok(Store {
            pool,
            oplog_clock: std::sync::Arc::new(tokio::sync::Mutex::new(
                top.and_then(|k| partisync_core::Hlc::from_key(&k)),
            )),
        })
    }

    /// 时钟顶落盘（单调 UPSERT——只接受更大的键，防御乱序持久化）。
    async fn persist_clock(&self, key: &str) -> Result<(), PartisyError> {
        sqlx::query(
            "INSERT INTO sync_clock (id, top) VALUES (1, ?)
             ON CONFLICT(id) DO UPDATE SET top = excluded.top
             WHERE excluded.top > sync_clock.top",
        )
        .bind(key)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("持久化时钟", e))?;
        Ok(())
    }

    async fn migrate(pool: &SqlitePool) -> Result<(), PartisyError> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(pool)
            .await
            .map_err(|e| db_err("迁移", e))?;
        // schema v2/v6/v9（M0-WP03/M2-WP01/M2-WP06）：旧库防御性补列；新库已含，
        // duplicate 忽略（ALTER 错误吞掉模式——仅添加列，逐个迁移）
        let _ = sqlx::raw_sql("ALTER TABLE entry ADD COLUMN chunk_root TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE entry ADD COLUMN owner_device TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE entry ADD COLUMN synced_seq INTEGER")
            .execute(pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE entry ADD COLUMN content_hydrated_at_ns INTEGER")
            .execute(pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE entry ADD COLUMN pin_count INTEGER NOT NULL DEFAULT 0")
            .execute(pool)
            .await;
        let _ = sqlx::raw_sql("UPDATE entry SET state = 1 WHERE state IS NULL")
            .execute(pool)
            .await;
        // v11（M2-WP04）：device 扩 endpoint / pairing_state / last_endpoint_update
        let _ = sqlx::raw_sql("ALTER TABLE device ADD COLUMN endpoint TEXT")
            .execute(pool)
            .await;
        let _ =
            sqlx::raw_sql("ALTER TABLE device ADD COLUMN pairing_state INTEGER NOT NULL DEFAULT 0")
                .execute(pool)
                .await;
        let _ = sqlx::raw_sql("ALTER TABLE device ADD COLUMN last_endpoint_update_ns INTEGER")
            .execute(pool)
            .await;
        Ok(())
    }

    /// 登记本设备与卷（v1：单设备演示，字段精简）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn seed_device_volume(
        &self,
        device_id: &str,
        device_name: &str,
        volume_fingerprint: &str,
    ) -> Result<(), PartisyError> {
        sqlx::query("INSERT OR IGNORE INTO device (id, name, slug) VALUES (?, ?, ?)")
            .bind(device_id)
            .bind(device_name)
            .bind(device_name)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("登记设备", e))?;
        sqlx::query("INSERT OR IGNORE INTO volume (id, device_id, fingerprint) VALUES (?, ?, ?)")
            .bind(Ulid::now().to_string())
            .bind(device_id)
            .bind(volume_fingerprint)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("登记卷", e))?;
        Ok(())
    }

    /// 插入条目并维护闭包表；path 已存在时幂等返回既有 id。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    #[allow(clippy::too_many_arguments)]
    pub async fn add_entry(
        &self,
        parent_id: Option<&str>,
        name: &str,
        path: &str,
        kind: EntryKind,
        size: u64,
        mtime_ns: u64,
        content: Option<(&str, u64)>, // (blake3 hex, size)
        chunk_root: Option<&str>,     // v2：大文件块清单根
    ) -> Result<String, PartisyError> {
        // upsert（SPEC M0-WP04 修订）：path 已存在时保留 id 与位置、刷新可变字段
        // （幂等 + 新鲜度；索引器重跑与 watch 应用共用此路径）
        if let Some(id) = sqlx::query_scalar::<_, String>("SELECT id FROM entry WHERE path = ?")
            .bind(path)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("查询条目", e))?
        {
            // 旧内容孤儿清理：内容变化后旧 content 行若无引用则删除
            // （否则 Σcontent 虚增，saved_bytes 变负——实测踩坑）
            let old_content: Option<Option<String>> =
                sqlx::query_scalar("SELECT content_id FROM entry WHERE path = ?")
                    .bind(path)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(|e| db_err("查旧内容", e))?;
            let new_hash = content.map(|(h, _)| h);
            if let Some((hash, csize)) = content {
                sqlx::query("INSERT OR IGNORE INTO content (id, size) VALUES (?, ?)")
                    .bind(hash)
                    .bind(i64::try_from(csize).unwrap_or(i64::MAX))
                    .execute(&self.pool)
                    .await
                    .map_err(|e| db_err("登记内容", e))?;
            }
            sqlx::query(
                "UPDATE entry SET size = ?, mtime_ns = ?, content_id = ?, chunk_root = ?
                 WHERE path = ?",
            )
            .bind(i64::try_from(size).unwrap_or(i64::MAX))
            .bind(i64::try_from(mtime_ns).unwrap_or(i64::MAX))
            .bind(new_hash)
            .bind(chunk_root)
            .bind(path)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("更新条目", e))?;
            if let Some(old) = old_content.flatten() {
                if new_hash != Some(old.as_str()) {
                    sqlx::query(
                        "DELETE FROM content WHERE id = ?
                         AND NOT EXISTS (SELECT 1 FROM entry WHERE content_id = content.id)",
                    )
                    .bind(&old)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| db_err("清理旧内容", e))?;
                }
            }
            return Ok(id);
        }
        if let Some((hash, csize)) = content {
            sqlx::query("INSERT OR IGNORE INTO content (id, size) VALUES (?, ?)")
                .bind(hash)
                .bind(i64::try_from(csize).unwrap_or(i64::MAX))
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("登记内容", e))?;
        }
        let id = Ulid::now().to_string();
        sqlx::query(
            "INSERT INTO entry (id, parent_id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(parent_id)
        .bind(kind as i64)
        .bind(name)
        .bind(path)
        .bind(content.map(|(h, _)| h))
        .bind(i64::try_from(size).unwrap_or(i64::MAX))
        .bind(i64::try_from(mtime_ns).unwrap_or(i64::MAX))
        .bind(chunk_root)
        .bind(self.device_id().await.ok()) // 域归属（M2-WP01）：本机创建 = 本机属主
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("插入条目", e))?;
        // 闭包维护：父链全部指向新节点（含自身 depth=0）
        sqlx::query(
            "INSERT INTO entry_closure (ancestor, descendant, depth)
             SELECT ancestor, ?, depth + 1 FROM entry_closure WHERE descendant = ?
             UNION ALL SELECT ?, ?, 0",
        )
        .bind(&id)
        .bind(parent_id)
        .bind(&id)
        .bind(&id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("维护闭包", e))?;
        Ok(id)
    }

    /// 按路径取条目。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn entry_by_path(&self, path: &str) -> Result<Option<EntryRow>, PartisyError> {
        sqlx::query_as::<_, EntryRow>(
            "SELECT id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device, state, content_hydrated_at_ns, pin_count FROM entry WHERE path = ?",
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_err("查询条目", e))
    }

    /// 列出直接子项（目录在前，名称序；LIMIT 1000——病态大目录的 UI 分页，
    /// M1-WP08 实测：顶层 16 万子项目录无限制时序列化 1.3s）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn children(&self, parent_path: &str) -> Result<Vec<EntryRow>, PartisyError> {
        sqlx::query_as::<_, EntryRow>(
            "SELECT id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device, state, content_hydrated_at_ns, pin_count FROM entry
             WHERE parent_id = (SELECT id FROM entry WHERE path = ?)
             ORDER BY kind DESC, name LIMIT 1000",
        )
        .bind(parent_path)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("列子项", e))
    }

    /// 祖先链（根→父，不含自身；面包屑）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn ancestors_of(&self, path: &str) -> Result<Vec<EntryRow>, PartisyError> {
        sqlx::query_as::<_, EntryRow>(
            "SELECT e.id, e.kind, e.name, e.path, e.content_id, e.size, e.mtime_ns, e.chunk_root, e.owner_device
             FROM entry e JOIN entry_closure c ON e.id = c.ancestor
             WHERE c.descendant = (SELECT id FROM entry WHERE path = ?) AND c.depth > 0
             ORDER BY c.depth DESC",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("祖先链", e))
    }

    /// 删除条目（级联：子树整体 + 相关闭包行；无引用的 content 行随删；
    /// 块库孤儿块留待 M3 GC——SPEC M0-WP03 风险节）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn remove_entry(&self, path: &str) -> Result<u64, PartisyError> {
        let subtree: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT e.id, e.content_id FROM entry e
             JOIN entry_closure c ON e.id = c.descendant
             WHERE c.ancestor = (SELECT id FROM entry WHERE path = ?)
             ORDER BY c.depth DESC", // 深度降序：子先于父删（parent_id FK）
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("子树收集", e))?;
        if subtree.is_empty() {
            return Ok(0);
        }
        let ids: Vec<&str> = subtree.iter().map(|(id, _)| id.as_str()).collect();
        // 先删 entry（content 清理需以「entry 已删」为前提）
        let mut deleted = 0u64;
        for id in &ids {
            deleted += sqlx::query("DELETE FROM entry WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("删除条目", e))?
                .rows_affected();
        }
        // 闭包行：涉及子树任一节点的全部行
        for id in &ids {
            sqlx::query("DELETE FROM entry_closure WHERE descendant = ? OR ancestor = ?")
                .bind(id)
                .bind(id)
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("删除闭包", e))?;
        }
        // 孤儿 content：无 entry 引用者删行
        for (_, content_id) in &subtree {
            if let Some(cid) = content_id {
                sqlx::query(
                    "DELETE FROM content WHERE id = ?
                     AND NOT EXISTS (SELECT 1 FROM entry WHERE content_id = content.id)",
                )
                .bind(cid)
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("清理内容", e))?;
            }
        }
        Ok(deleted)
    }
}

impl Store {
    /// 批量插入文件条目：单事务（KPI 优化，SPEC M0-WP07）——
    /// 每文件 3 条语句共享一次提交；ON CONFLICT(path) DO UPDATE RETURNING id
    /// 保持 upsert 语义（旧 id 保留）。调用方须保证父目录行已存在。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn add_file_batch(&self, files: &[FileInsert]) -> Result<(), PartisyError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| db_err("开启批事务", e))?;
        for f in files {
            if let Some((hash, csize)) = &f.content {
                sqlx::query("INSERT OR IGNORE INTO content (id, size) VALUES (?, ?)")
                    .bind(hash)
                    .bind(i64::try_from(*csize).unwrap_or(i64::MAX))
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| db_err("批量登记内容", e))?;
            }
            let id: String = sqlx::query_scalar(
                "INSERT INTO entry (id, parent_id, kind, name, path, content_id, size, mtime_ns, chunk_root)
                 VALUES (?, ?, 0, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(path) DO UPDATE SET
                     size = excluded.size, mtime_ns = excluded.mtime_ns,
                     content_id = excluded.content_id, chunk_root = excluded.chunk_root
                 RETURNING id",
            )
            .bind(&f.id)
            .bind(f.parent_id.as_deref())
            .bind(&f.name)
            .bind(&f.path)
            .bind(f.content.as_ref().map(|(h, _)| h))
            .bind(i64::try_from(f.size).unwrap_or(i64::MAX))
            .bind(i64::try_from(f.mtime_ns).unwrap_or(i64::MAX))
            .bind(f.chunk_root.as_deref())
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| db_err("批量插入条目", e))?;
            sqlx::query(
                "INSERT OR IGNORE INTO entry_closure (ancestor, descendant, depth)
                 SELECT ancestor, ?, depth + 1 FROM entry_closure WHERE descendant = ?
                 UNION ALL SELECT ?, ?, 0",
            )
            .bind(&id)
            .bind(f.parent_id.as_deref())
            .bind(&id)
            .bind(&id)
            .execute(&mut *tx)
            .await
            .map_err(|e| db_err("批量闭包", e))?;
        }
        tx.commit().await.map_err(|e| db_err("提交批事务", e))?;
        Ok(())
    }
}

impl Store {
    /// 本机设备 id（device 表首行；未登记 → 'device-local'）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn device_id(&self) -> Result<String, PartisyError> {
        sqlx::query_scalar::<_, String>("SELECT id FROM device ORDER BY id LIMIT 1")
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten()
            .ok_or_else(|| PartisyError {
                severity: Severity::Fatal,
                source: Some("device 表为空：先 seed_device_volume".into()),
            })
    }

    /// 记录一条 oplog（HLC 由本店时钟分配，严格单调）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    #[allow(clippy::too_many_arguments)]
    pub async fn record_oplog(
        &self,
        space_id: &str,
        domain: i64,
        entity: &str,
        entity_id: &str,
        op: &str,
        origin_device: &str,
        payload: &str,
    ) -> Result<String, PartisyError> {
        let wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let device_hash = hash_u64(origin_device);
        let mut clock = self.oplog_clock.lock().await;
        let hlc = match *clock {
            Some(mut h) => {
                h.tick(wall).map_err(|e| PartisyError {
                    severity: Severity::Fatal,
                    source: Some(format!("oplog 时钟溢出: {e:?}").into()),
                })?;
                h
            }
            None => partisync_core::Hlc::from_wall(device_hash, wall),
        };
        *clock = Some(hlc);
        let key = hlc.to_key();
        sqlx::query(
            "INSERT OR IGNORE INTO sync_oplog (hlc, space_id, domain, entity, entity_id, op, origin_device, payload, at_ns)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&key)
        .bind(space_id)
        .bind(domain)
        .bind(entity)
        .bind(entity_id)
        .bind(op)
        .bind(origin_device)
        .bind(payload)
        .bind(i64::try_from(hlc.phys_ms() * 1_000_000).unwrap_or(i64::MAX))
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("记录 oplog", e))?;
        self.persist_clock(&key).await?;
        Ok(key)
    }

    /// 以**给定 hlc key** 记录 oplog 行（M2-WP02 中继专用：一个写入全网同一个键，
    /// `INSERT OR IGNORE` 对同写多径到达天然去重）。插入前对本端时钟做
    /// [`Hlc::recv`](partisync_core::Hlc::recv) 因果更新——此后本端新写入严格晚于
    /// 一切已见远端写（LWW 全序前提）。
    ///
    /// # Errors
    /// DB 错误 → Fatal；hlc key 非法 → Fatal。
    #[allow(clippy::too_many_arguments)]
    pub async fn record_oplog_raw(
        &self,
        hlc_key: &str,
        space_id: &str,
        domain: i64,
        entity: &str,
        entity_id: &str,
        op: &str,
        origin_device: &str,
        payload: &str,
        at_ns: i64,
    ) -> Result<(), PartisyError> {
        let remote = partisync_core::Hlc::from_key(hlc_key).ok_or_else(|| PartisyError {
            severity: Severity::Fatal,
            source: Some(format!("oplog hlc key 非法: {hlc_key}").into()),
        })?;
        let wall = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let local = self
            .device_id()
            .await
            .unwrap_or_else(|_| "device-local".into());
        {
            let mut clock = self.oplog_clock.lock().await;
            let mut h =
                clock.unwrap_or_else(|| partisync_core::Hlc::from_wall(hash_u64(&local), wall));
            h.recv(wall, remote).map_err(|e| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("oplog 时钟合并失败: {e:?}").into()),
            })?;
            *clock = Some(h);
        }
        sqlx::query(
            "INSERT OR IGNORE INTO sync_oplog (hlc, space_id, domain, entity, entity_id, op, origin_device, payload, at_ns)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(hlc_key)
        .bind(space_id)
        .bind(domain)
        .bind(entity)
        .bind(entity_id)
        .bind(op)
        .bind(origin_device)
        .bind(payload)
        .bind(at_ns)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("中继 oplog", e))?;
        self.persist_clock(hlc_key).await?;
        Ok(())
    }

    /// 全部待同步 oplog 行（HLC 全序）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn pending_oplog(&self) -> Result<Vec<OplogRow>, PartisyError> {
        sqlx::query_as::<_, OplogRow>(
            "SELECT hlc, space_id, domain, entity, entity_id, op, origin_device, payload, at_ns
             FROM sync_oplog ORDER BY hlc",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("读 oplog", e))
    }

    /// ACK 裁剪：删除指定 hlc 的 oplog 行（对端已确认应用）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn trim_oplog(&self, hlcs: &[String]) -> Result<u64, PartisyError> {
        let mut n = 0u64;
        for h in hlcs {
            n += sqlx::query("DELETE FROM sync_oplog WHERE hlc = ?")
                .bind(h)
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("裁剪 oplog", e))?
                .rows_affected();
        }
        Ok(n)
    }

    /// 应用远端同步条目（设备自有域全量行；冲突 = 保留两者 + 血缘后缀，P11）。
    ///
    /// owner_device 随 payload 走（单写者：非属主不产生本地图谱修改语义——
    /// 远端行就是属主行的投影）。冲突后缀防覆盖：候选 `{path}.conflict-{owner}`
    /// 已存在且同 content → 幂等复用（重复投递安全）；内容不同 → `-2`、`-3`…
    /// 递增找空位（防后缀名恰被他人文件占用导致覆盖丢数据）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    #[allow(clippy::too_many_arguments)]
    pub async fn apply_remote_entry(
        &self,
        path: &str,
        name: &str,
        kind: EntryKind,
        size: u64,
        mtime_ns: u64,
        content: Option<(&str, u64)>,
        chunk_root: Option<&str>,
        owner_device: &str,
    ) -> Result<ApplyOutcome, PartisyError> {
        // 同路径不同属主 = 双端独立创建冲突 → 保留两者：来方改挂冲突后缀
        let mut final_path = path.to_string();
        let mut conflict = None;
        if let Some(existing) = self.entry_by_path(path).await? {
            let existing_owner = existing
                .owner_device
                .clone()
                .unwrap_or_else(|| "unknown".into());
            if existing_owner != owner_device {
                let incoming_content = content.map(|(h, _)| h);
                // 候选序：base、-2、-3…-64；全被占则 ULID 兜底（病态多版本，
                // 版本回收归 WP08）。同内容复用 = 同版本重复投递（幂等），不另开副本
                let mut placed = false;
                for sfx in std::iter::once(String::new()).chain((2..=64).map(|k| format!("-{k}"))) {
                    let cand = format!("{path}.conflict-{owner_device}{sfx}");
                    match self.entry_by_path(&cand).await? {
                        None => {
                            final_path = cand;
                            placed = true;
                            break;
                        }
                        Some(c) if c.content_id.as_deref() == incoming_content => {
                            final_path = cand;
                            placed = true;
                            break;
                        }
                        Some(_) => {}
                    }
                }
                if !placed {
                    final_path = format!("{path}.conflict-{owner_device}-{}", Ulid::now());
                }
                conflict = Some(ApplyConflict {
                    base_path: path.to_string(),
                    incoming_path: final_path.clone(),
                });
            }
        }
        // 父目录链（远端树的目录由 oplog 目录条目或此处 ensure 保证）
        crate::journal::ensure_dir_chain_pub(self, &final_path).await?;
        let parent_path = match final_path.rsplit_once('/') {
            Some((p, _)) if !p.is_empty() => p.to_string(),
            _ => "/".to_string(),
        };
        let parent_id = self.entry_by_path(&parent_path).await?.map(|e| e.id);
        let existed = self.entry_by_path(&final_path).await?.is_some();
        if let Some((hash, csize)) = content {
            sqlx::query("INSERT OR IGNORE INTO content (id, size) VALUES (?, ?)")
                .bind(hash)
                .bind(i64::try_from(csize).unwrap_or(i64::MAX))
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("登记远端内容", e))?;
        }
        // M2-WP06：远端条目无 content → 以 placeholder 形态入库
        let entry_state: i64 = if content.is_some() { 0 } else { 1 };
        let id = partisync_core::Ulid::now().to_string();
        sqlx::query(
            "INSERT INTO entry (id, parent_id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device, state)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(path) DO UPDATE SET
                 size = excluded.size, mtime_ns = excluded.mtime_ns,
                 content_id = excluded.content_id, chunk_root = excluded.chunk_root,
                 owner_device = excluded.owner_device,
                 state = excluded.state
             RETURNING id",
        )
        .bind(&id)
        .bind(&parent_id)
        .bind(kind as i64)
        .bind(name)
        .bind(&final_path)
        .bind(content.map(|(h, _)| h))
        .bind(i64::try_from(size).unwrap_or(i64::MAX))
        .bind(i64::try_from(mtime_ns).unwrap_or(i64::MAX))
        .bind(chunk_root)
        .bind(owner_device)
        .bind(entry_state)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_err("应用远端条目", e))?;
        // 新插入行须维护闭包（与 add_entry 同款）——否则远端条目 remove_entry/
        // ancestors_of 全部失效（WP02 测试暴露的 WP01 潜在缺陷）；复用既有行
        // （ON CONFLICT 更新）时闭包已在，跳过防 PK 冲突
        if !existed {
            sqlx::query(
                "INSERT INTO entry_closure (ancestor, descendant, depth)
                 SELECT ancestor, ?, depth + 1 FROM entry_closure WHERE descendant = ?
                 UNION ALL SELECT ?, ?, 0",
            )
            .bind(&id)
            .bind(parent_id)
            .bind(&id)
            .bind(&id)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("维护远端闭包", e))?;
        }
        Ok(ApplyOutcome {
            path: final_path,
            conflict,
        })
    }

    /// 名称子串检索（LIKE，上限防全表外溢）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn search(&self, q: &str, limit: u32) -> Result<Vec<EntryRow>, PartisyError> {
        sqlx::query_as::<_, EntryRow>(
            "SELECT id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device, state, content_hydrated_at_ns, pin_count FROM entry
             WHERE name LIKE '%' || ? || '%' ORDER BY kind DESC, name LIMIT ?",
        )
        .bind(q)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("搜索", e))
    }

    /// 闭包子树查询（L4 不变量的被测路径）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn subtree(&self, root_path: &str) -> Result<Vec<String>, PartisyError> {
        sqlx::query_scalar::<_, String>(
            "SELECT e.path FROM entry e
             JOIN entry_closure c ON e.id = c.descendant
             WHERE c.ancestor = (SELECT id FROM entry WHERE path = ?)
             ORDER BY e.path",
        )
        .bind(root_path)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("子树查询", e))
    }

    /// 全局统计（去重口径见 [`Stats`]）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn stats(&self) -> Result<Stats, PartisyError> {
        let (files, dirs, total_bytes): (i64, i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(kind = 0), 0), COALESCE(SUM(kind = 1), 0),
                    COALESCE(SUM(CASE WHEN kind = 0 THEN size END), 0) FROM entry",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_err("统计", e))?;
        let (unique_contents, unique_bytes): (i64, i64) =
            sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(size), 0) FROM content")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| db_err("统计内容", e))?;
        let (duplicate_groups,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM (
                 SELECT content_id FROM entry WHERE content_id IS NOT NULL
                 GROUP BY content_id HAVING COUNT(*) > 1
             )",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_err("统计重复", e))?;
        Ok(Stats {
            files,
            dirs,
            total_bytes,
            unique_contents,
            unique_bytes,
            duplicate_groups,
            saved_bytes: total_bytes - unique_bytes,
        })
    }

    /// 重复内容分组（按浪费字节降序）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn duplicates(&self, limit: u32) -> Result<Vec<DupGroup>, PartisyError> {
        let groups: Vec<(String, i64)> = sqlx::query_as(
            "SELECT content_id, SUM(size) AS wasted FROM entry
             WHERE content_id IS NOT NULL GROUP BY content_id
             HAVING COUNT(*) > 1 ORDER BY wasted DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("重复分组", e))?;
        let mut out = Vec::with_capacity(groups.len());
        for (content_id, _wasted) in groups {
            let (size,): (i64,) = sqlx::query_as("SELECT size FROM content WHERE id = ?")
                .bind(&content_id)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| db_err("重复组大小", e))?;
            let copies = sqlx::query_as::<_, EntryRow>(
                "SELECT id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device, state, content_hydrated_at_ns, pin_count FROM entry
                 WHERE content_id = ? ORDER BY path",
            )
            .bind(&content_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_err("重复组成员", e))?;
            out.push(DupGroup {
                content_id,
                size,
                copies,
            });
        }
        Ok(out)
    }
}

impl Store {
    // ---- Tag 共享域（M2-WP02：HLC LWW，墓碑防复活）----

    /// 新建 tag（本地写；LWW 水位由 capture 在 oplog 落笔后回填）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn add_tag(&self, name: &str, color: Option<&str>) -> Result<String, PartisyError> {
        let id = Ulid::now().to_string();
        sqlx::query("INSERT INTO tag (id, name, color) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(name)
            .bind(color)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("新建 tag", e))?;
        Ok(id)
    }

    /// 更新 tag 字段（本地写）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn update_tag(
        &self,
        id: &str,
        name: &str,
        color: Option<&str>,
    ) -> Result<(), PartisyError> {
        sqlx::query("UPDATE tag SET name = ?, color = ? WHERE id = ?")
            .bind(name)
            .bind(color)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("更新 tag", e))?;
        Ok(())
    }

    /// 删除 tag（墓碑：deleted=1，行保留——晚到的旧 upsert 靠 LWW 水位拒绝）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn delete_tag(&self, id: &str) -> Result<(), PartisyError> {
        sqlx::query("UPDATE tag SET deleted = 1 WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("删除 tag", e))?;
        Ok(())
    }

    /// 打标签（本地写；entry 以 path 为跨节点身份，悬空软引用合法）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn tag_entry(&self, tag_id: &str, entry_path: &str) -> Result<(), PartisyError> {
        sqlx::query(
            "INSERT INTO entry_tag (tag_id, entry_path, deleted) VALUES (?, ?, 0)
             ON CONFLICT(tag_id, entry_path) DO UPDATE SET deleted = 0",
        )
        .bind(tag_id)
        .bind(entry_path)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("打标签", e))?;
        Ok(())
    }

    /// 摘标签（墓碑 upsert——防晚到 link 复活）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn untag_entry(&self, tag_id: &str, entry_path: &str) -> Result<(), PartisyError> {
        sqlx::query(
            "INSERT INTO entry_tag (tag_id, entry_path, deleted) VALUES (?, ?, 1)
             ON CONFLICT(tag_id, entry_path) DO UPDATE SET deleted = 1",
        )
        .bind(tag_id)
        .bind(entry_path)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("摘标签", e))?;
        Ok(())
    }

    /// 按 id 取 tag（含墓碑——LWW 判定需要看到墓碑行）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn tag_by_id(&self, id: &str) -> Result<Option<TagRow>, PartisyError> {
        sqlx::query_as::<_, TagRow>(
            "SELECT id, space_id, name, color, deleted, updated_hlc FROM tag WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_err("查询 tag", e))
    }

    /// 存活 tag 列表（墓碑不参与）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn list_tags(&self) -> Result<Vec<TagRow>, PartisyError> {
        sqlx::query_as::<_, TagRow>(
            "SELECT id, space_id, name, color, deleted, updated_hlc FROM tag
             WHERE deleted = 0 ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("列 tag", e))
    }

    /// 条目的存活标签（悬空链接与墓碑 JOIN 过滤）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn tags_of_entry(&self, entry_path: &str) -> Result<Vec<TagRow>, PartisyError> {
        sqlx::query_as::<_, TagRow>(
            "SELECT g.id, g.space_id, g.name, g.color, g.deleted, g.updated_hlc
             FROM tag g JOIN entry_tag t ON t.tag_id = g.id
             WHERE t.entry_path = ? AND t.deleted = 0 AND g.deleted = 0 ORDER BY g.name",
        )
        .bind(entry_path)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("查条目标签", e))
    }

    /// 标签下的存活条目。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn entries_for_tag(&self, tag_id: &str) -> Result<Vec<EntryRow>, PartisyError> {
        sqlx::query_as::<_, EntryRow>(
            "SELECT e.id, e.kind, e.name, e.path, e.content_id, e.size, e.mtime_ns, e.chunk_root, e.owner_device
             FROM entry e JOIN entry_tag t ON t.entry_path = e.path
             WHERE t.tag_id = ? AND t.deleted = 0 ORDER BY e.path",
        )
        .bind(tag_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("查标签条目", e))
    }

    /// 应用远端 tag 写（共享域 LWW：`existing.updated_hlc >= hlc_key` → 跳过；
    /// HLC key 定宽 hex，字符串序 == 时间序）。`deleted=true` 为墓碑应用。
    /// 返回是否实际生效（落选/已见 = false，session 据此决定是否转发）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn apply_remote_tag(
        &self,
        id: &str,
        name: &str,
        color: Option<&str>,
        deleted: bool,
        hlc_key: &str,
    ) -> Result<bool, PartisyError> {
        let existing = self.tag_by_id(id).await?;
        if let Some(row) = existing {
            if row.updated_hlc.as_deref().is_some_and(|h| h >= hlc_key) {
                return Ok(false);
            }
            sqlx::query(
                "UPDATE tag SET name = ?, color = ?, deleted = ?, updated_hlc = ? WHERE id = ?",
            )
            .bind(name)
            .bind(color)
            .bind(i64::from(deleted))
            .bind(hlc_key)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("应用远端 tag", e))?;
        } else {
            sqlx::query(
                "INSERT INTO tag (id, name, color, deleted, updated_hlc) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(name)
            .bind(color)
            .bind(i64::from(deleted))
            .bind(hlc_key)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("应用远端 tag", e))?;
        }
        Ok(true)
    }

    /// 应用远端 link/unlink（同 [`Store::apply_remote_tag`] 的 LWW 口径）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn apply_remote_tag_link(
        &self,
        tag_id: &str,
        entry_path: &str,
        deleted: bool,
        hlc_key: &str,
    ) -> Result<bool, PartisyError> {
        let existing: Option<(i64, Option<String>)> = sqlx::query_as(
            "SELECT deleted, updated_hlc FROM entry_tag WHERE tag_id = ? AND entry_path = ?",
        )
        .bind(tag_id)
        .bind(entry_path)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_err("查询链接", e))?;
        if let Some((_, hlc)) = existing {
            if hlc.as_deref().is_some_and(|h| h >= hlc_key) {
                return Ok(false);
            }
        }
        sqlx::query(
            "INSERT INTO entry_tag (tag_id, entry_path, deleted, updated_hlc) VALUES (?, ?, ?, ?)
             ON CONFLICT(tag_id, entry_path) DO UPDATE SET deleted = excluded.deleted, updated_hlc = excluded.updated_hlc",
        )
        .bind(tag_id)
        .bind(entry_path)
        .bind(i64::from(deleted))
        .bind(hlc_key)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("应用远端链接", e))?;
        Ok(true)
    }
}

impl Store {
    // ---- 冲突血缘（P11：同名双改 → 两者皆可寻址，血缘可查）----

    /// 落档一次冲突血缘（同 base/incoming/detected_hlc 重复落档幂等返回既有 id）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn record_conflict(
        &self,
        space_id: &str,
        base_path: &str,
        local_path: &str,
        incoming_path: &str,
        origin_device: &str,
        detected_hlc: &str,
    ) -> Result<String, PartisyError> {
        sqlx::query(
            "INSERT OR IGNORE INTO sync_conflict (id, space_id, base_path, local_path, incoming_path, origin_device, detected_hlc, at_ns)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Ulid::now().to_string())
        .bind(space_id)
        .bind(base_path)
        .bind(local_path)
        .bind(incoming_path)
        .bind(origin_device)
        .bind(detected_hlc)
        .bind(i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        )
        .unwrap_or(i64::MAX))
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("落档冲突血缘", e))?;
        sqlx::query_scalar::<_, String>(
            "SELECT id FROM sync_conflict WHERE base_path = ? AND incoming_path = ? AND detected_hlc = ?",
        )
        .bind(base_path)
        .bind(incoming_path)
        .bind(detected_hlc)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_err("查冲突血缘", e))
    }

    /// 冲突血缘列表（新→旧）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn list_conflicts(&self, limit: u32) -> Result<Vec<ConflictRow>, PartisyError> {
        sqlx::query_as::<_, ConflictRow>(
            "SELECT id, space_id, base_path, local_path, incoming_path, origin_device, detected_hlc, at_ns
             FROM sync_conflict ORDER BY at_ns DESC, id DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("列冲突血缘", e))
    }
}

impl Store {
    // ---- 水位（M2-WP03 对账快路径用）----

    /// 记录单条应用过的 oplog 键对应的 origin 水位（按 origin 单调）；
    /// 同 origin 多次调用取最大键——可重入安全。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn note_applied(&self, origin: &str, hlc: &str) -> Result<(), PartisyError> {
        sqlx::query(
            "INSERT INTO sync_watermark (device, space_id, last_hlc) VALUES (?, 'default', ?)
             ON CONFLICT(device, space_id) DO UPDATE SET last_hlc = excluded.last_hlc
             WHERE excluded.last_hlc > sync_watermark.last_hlc",
        )
        .bind(origin)
        .bind(hlc)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("记水位", e))?;
        Ok(())
    }

    /// 全表导出（origin → 最高键）：对账快路径互换。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn watermarks(&self) -> Result<Vec<(String, String)>, PartisyError> {
        sqlx::query_as::<_, (String, String)>("SELECT device, last_hlc FROM sync_watermark")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_err("列水位", e))
    }

    /// 本店时钟顶（HLC key 形式）——「本机是否又产生了新写入」的对账判据。
    /// 无键 ⇒ 本机尚未做任何写入（首次对账与 fresh joiner 校验）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn clock_top(&self) -> Result<Option<String>, PartisyError> {
        sqlx::query_scalar::<_, String>("SELECT top FROM sync_clock WHERE id = 1")
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("读时钟顶", e))
    }

    // ---- 状态导出（M2-WP03 Merkle 叶集合扫描）----

    /// entry 状态叶：(path, kind, content_id, owner, size, mtime_ns)——对账可比性需全字段。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn entry_state_leaves(
        &self,
    ) -> Result<Vec<(String, i64, Option<String>, Option<String>, i64, i64)>, PartisyError> {
        sqlx::query_as("SELECT path, kind, content_id, owner_device, size, mtime_ns FROM entry")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_err("扫 entry 叶", e))
    }

    /// tag 状态叶：(key="tag/{id}", name, color, deleted)——LWW 水位不进叶哈希。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn tag_state_leaves(
        &self,
    ) -> Result<Vec<(String, String, Option<String>, i64)>, PartisyError> {
        sqlx::query_as("SELECT id, name, color, deleted FROM tag")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_err("扫 tag 叶", e))
    }

    /// entry_tag 状态叶：(key="link/{tag_id}␟{path}", deleted)。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn link_state_leaves(&self) -> Result<Vec<(String, String, i64)>, PartisyError> {
        sqlx::query_as("SELECT tag_id, entry_path, deleted FROM entry_tag")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| db_err("扫链接叶", e))
    }
}

impl Store {
    // ---- 占位符（M2-WP06：骨架同步与按需 hydrate）----

    /// 新建占位条目（content_id=NULL, state=Placeholder）。元数据已就位，
    /// 内容待 hydrate（本地调用或由 WP05 块传输完成后调）。
    ///
    /// # Errors
    /// DB 错误 → Fatal；path 冲突 → 幂等返回既有 id。
    pub async fn add_placeholder(
        &self,
        parent_id: Option<&str>,
        name: &str,
        path: &str,
        size: u64,
        mtime_ns: u64,
    ) -> Result<String, PartisyError> {
        if let Some(id) = sqlx::query_scalar::<_, String>("SELECT id FROM entry WHERE path = ?")
            .bind(path)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("查询 placeholder", e))?
        {
            return Ok(id);
        }
        let id = Ulid::now().to_string();
        sqlx::query(
            "INSERT INTO entry (id, parent_id, kind, name, path, size, mtime_ns, owner_device, state)
             VALUES (?, ?, 0, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&id)
        .bind(parent_id)
        .bind(name)
        .bind(path)
        .bind(i64::try_from(size).unwrap_or(i64::MAX))
        .bind(i64::try_from(mtime_ns).unwrap_or(i64::MAX))
        .bind(self.device_id().await.ok())
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("新建 placeholder", e))?;
        // 闭包维护（同 add_entry）
        sqlx::query(
            "INSERT INTO entry_closure (ancestor, descendant, depth)
             SELECT ancestor, ?, depth + 1 FROM entry_closure WHERE descendant = ?
             UNION ALL SELECT ?, ?, 0",
        )
        .bind(&id)
        .bind(parent_id)
        .bind(&id)
        .bind(&id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("占位闭包", e))?;
        Ok(id)
    }

    /// hydrate：内容到位后清 placeholder 标记，记时间戳。
    ///
    /// # Errors
    /// path 不存在 → Fatal；DB 错误 → Fatal。
    pub async fn hydrate_entry(
        &self,
        path: &str,
        content: (&str, u64),
    ) -> Result<(), PartisyError> {
        let (hash, csize) = content;
        sqlx::query("INSERT OR IGNORE INTO content (id, size) VALUES (?, ?)")
            .bind(hash)
            .bind(i64::try_from(csize).unwrap_or(i64::MAX))
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("hydrate 登记内容", e))?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        sqlx::query(
            "UPDATE entry SET state = 0, content_id = ?, content_hydrated_at_ns = ?
             WHERE path = ?",
        )
        .bind(hash)
        .bind(i64::try_from(now).unwrap_or(i64::MAX))
        .bind(path)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("hydrate 更新", e))?;
        Ok(())
    }

    /// pin（用户显式保留）：pin_count > 0 的条目 WP08 永不被回收。
    /// 重复 pin 不增计数（幂等）。
    ///
    /// # Errors
    /// DB 错误 → Fatal；path 不存在 → 0 行影响（不报错——壳层允许 pin 路径未到位）。
    pub async fn pin(&self, path: &str) -> Result<(), PartisyError> {
        sqlx::query("UPDATE entry SET pin_count = pin_count + 1 WHERE path = ? AND pin_count = 0")
            .bind(path)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("pin", e))?;
        Ok(())
    }

    /// unpin：仅减一次（pin_count ≥ 0）；已 0 时为 no-op。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn unpin(&self, path: &str) -> Result<(), PartisyError> {
        sqlx::query("UPDATE entry SET pin_count = MAX(pin_count - 1, 0) WHERE path = ?")
            .bind(path)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("unpin", e))?;
        Ok(())
    }
}

/// 条目版本记录（M2-WP08：staggered 版本化 + trash-can）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct EntryVersionRow {
    pub id: String,
    pub path: String,
    pub content_id: Option<String>,
    pub size: i64,
    pub mtime_ns: i64,
    pub owner_device: Option<String>,
    /// 0=staggered-versioned, 1=trashed
    pub state: i64,
    pub retired_at_ns: i64,
    pub expires_at_ns: i64,
}

impl Store {
    // ---- 版本回收（M2-WP08）----

    /// 覆盖写前自动调用：把 entry 表当前行复制到 entry_version（staggered-versioned）。
    /// 已存在版本行的不重复添加（幂等）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn retire_for_overwrite(&self, path: &str) -> Result<(), PartisyError> {
        if let Some(row) = self.entry_by_path(path).await? {
            sqlx::query(
                "INSERT OR IGNORE INTO entry_version
                    (id, path, content_id, size, mtime_ns, owner_device, state, retired_at_ns, expires_at_ns)
                 VALUES (?, ?, ?, ?, ?, ?, 0, ?, ?)",
            )
            .bind(Ulid::now().to_string())
            .bind(path)
            .bind(&row.content_id)
            .bind(row.size)
            .bind(row.mtime_ns)
            .bind(&row.owner_device)
            .bind(self.now_ns())
            .bind(i64::MAX) // staggered 不自动过期——只受 K=5 滚动裁剪
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("退役条目快照", e))?;
        }
        Ok(())
    }

    /// trash：标记 entry 删除 + 在 entry_version 留 trashed 行（ttl_ns 后可物理删）。
    /// pin_count > 0 时不 trash（pin 防回收）。
    ///
    /// # Errors
    /// DB 错误 → Fatal；path 不存在 → no-op。
    pub async fn trash_entry(&self, path: &str, ttl_ns: i64) -> Result<(), PartisyError> {
        if let Some(row) = self.entry_by_path(path).await? {
            if row.pin_count > 0 {
                return Ok(()); // pin 防回收
            }
            let now = self.now_ns();
            sqlx::query(
                "INSERT INTO entry_version
                    (id, path, content_id, size, mtime_ns, owner_device, state, retired_at_ns, expires_at_ns)
                 VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
            )
            .bind(Ulid::now().to_string())
            .bind(path)
            .bind(&row.content_id)
            .bind(row.size)
            .bind(row.mtime_ns)
            .bind(&row.owner_device)
            .bind(now)
            .bind(now.saturating_add(ttl_ns))
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("trash", e))?;
            self.remove_entry(path).await?;
        }
        Ok(())
    }

    /// resurrect：从 entry_version 找最近一次 trashed 行，重新插入 entry。
    ///
    /// # Errors
    /// path 已有 entry → Fatal（防覆盖）；无 trashed 版本 → no-op 返回空串。
    pub async fn resurrect(&self, path: &str) -> Result<String, PartisyError> {
        if self.entry_by_path(path).await?.is_some() {
            return Err(PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("resurrect 目标路径已存在: {path}").into()),
            });
        }
        let v: Option<EntryVersionRow> = sqlx::query_as(
            "SELECT id, path, content_id, size, mtime_ns, owner_device, state, retired_at_ns, expires_at_ns
             FROM entry_version WHERE path = ? AND state = 1
             ORDER BY retired_at_ns DESC LIMIT 1",
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_err("resurrect 查询", e))?;
        let Some(v) = v else { return Ok(String::new()) };
        let parent = match v.path.rsplit_once('/') {
            Some((p, _)) if !p.is_empty() => p.to_string(),
            _ => "/".to_string(),
        };
        crate::journal::ensure_dir_chain_pub(self, path).await?;
        let parent_id = self.entry_by_path(&parent).await?.map(|e| e.id);
        // content 行需预先存在（FK 约束）
        if let Some(cid) = &v.content_id {
            sqlx::query("INSERT OR IGNORE INTO content (id, size) VALUES (?, ?)")
                .bind(cid)
                .bind(v.size)
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("resurrect 登记内容", e))?;
        }
        let id = Ulid::now().to_string();
        let kind = 0;
        sqlx::query(
            "INSERT INTO entry (id, parent_id, kind, name, path, content_id, size, mtime_ns, owner_device, state)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0)",
        )
        .bind(&id)
        .bind(&parent_id)
        .bind(kind)
        .bind(v.path.rsplit('/').next().unwrap_or(&v.path))
        .bind(path)
        .bind(&v.content_id)
        .bind(v.size)
        .bind(v.mtime_ns)
        .bind(&v.owner_device)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("resurrect 插入", e))?;
        sqlx::query(
            "INSERT INTO entry_closure (ancestor, descendant, depth)
             SELECT ancestor, ?, depth + 1 FROM entry_closure WHERE descendant = ?
             UNION ALL SELECT ?, ?, 0",
        )
        .bind(&id)
        .bind(parent_id)
        .bind(&id)
        .bind(&id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("resurrect 闭包", e))?;
        Ok(id)
    }

    /// 列某 path 的版本（按 retired_at_ns DESC）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn list_versions(&self, path: &str) -> Result<Vec<EntryVersionRow>, PartisyError> {
        sqlx::query_as::<_, EntryVersionRow>(
            "SELECT id, path, content_id, size, mtime_ns, owner_device, state, retired_at_ns, expires_at_ns
             FROM entry_version WHERE path = ? ORDER BY retired_at_ns DESC",
        )
        .bind(path)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| db_err("列版本", e))
    }

    /// GC：删 expires_at_ns < now 的 trashed 行；staggered 滚动裁剪（K=5）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn gc_expired(&self) -> Result<u64, PartisyError> {
        let now = self.now_ns();
        let mut tx = self.pool.begin().await.map_err(|e| db_err("GC tx", e))?;
        let n_trashed =
            sqlx::query("DELETE FROM entry_version WHERE state = 1 AND expires_at_ns < ?")
                .bind(now)
                .execute(&mut *tx)
                .await
                .map_err(|e| db_err("GC trash", e))?
                .rows_affected();
        sqlx::query(
            "DELETE FROM entry_version WHERE state = 0 AND id NOT IN (
                SELECT id FROM entry_version
                WHERE state = 0 AND id IN (
                    SELECT id FROM (
                        SELECT id, ROW_NUMBER() OVER (PARTITION BY path ORDER BY retired_at_ns DESC) AS rn
                        FROM entry_version WHERE state = 0
                    ) WHERE rn <= 5
                )
            )",
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| db_err("GC stagger", e))?;
        tx.commit().await.map_err(|e| db_err("GC 提交", e))?;
        Ok(n_trashed)
    }

    fn now_ns(&self) -> i64 {
        i64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        )
        .unwrap_or(i64::MAX)
    }
}

/// 配对会话行（M2-WP04：助记词配对状态机）。
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PairingSessionRow {
    pub id: String,
    pub code: String,
    pub initiator_dev: String,
    pub responder_dev: Option<String>,
    /// 0=open, 1=accepted, 2=closed
    pub state: i64,
    pub ephemeral_sk: Vec<u8>,
    pub ephemeral_pk: Vec<u8>,
    pub shared_secret: Option<Vec<u8>>,
    pub created_ns: i64,
    pub expires_ns: i64,
}

impl Store {
    // ---- 设备注册与配对（M2-WP04）----

    /// 登记设备 endpoint（M2-WP04：iroh NodeAddr 序列化）。
    ///
    /// # Errors
    /// DB 错误 → Fatal；device_id 不存在 → Fatal。
    pub async fn set_device_endpoint(
        &self,
        device_id: &str,
        endpoint: &str,
    ) -> Result<(), PartisyError> {
        sqlx::query(
            "UPDATE device SET endpoint = ?, pairing_state = 2, last_endpoint_update_ns = ?
             WHERE id = ?",
        )
        .bind(endpoint)
        .bind(self.now_ns())
        .bind(device_id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("登记 endpoint", e))?;
        Ok(())
    }

    /// 设备配对状态查询。
    ///
    /// # Errors
    /// DB 错误 → Fatal；device 不存在 → 0。
    pub async fn device_pairing_state(&self, device_id: &str) -> Result<i64, PartisyError> {
        let v: Option<i64> = sqlx::query_scalar("SELECT pairing_state FROM device WHERE id = ?")
            .bind(device_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("查 pairing_state", e))?;
        Ok(v.unwrap_or(0))
    }

    /// 创建配对会话（发起方调用）。code = 12 词助记词空格串；ephemeral_sk/pk
    /// 为发起方临时 X25519 密钥对字节（32 字节私钥 / 32 字节公钥）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn create_pairing_session(
        &self,
        id: &str,
        code: &str,
        initiator_dev: &str,
        ephemeral_sk: &[u8],
        ephemeral_pk: &[u8],
        ttl_ns: i64,
    ) -> Result<(), PartisyError> {
        let now = self.now_ns();
        sqlx::query(
            "INSERT INTO pairing_session
                (id, code, initiator_dev, state, ephemeral_sk, ephemeral_pk, created_ns, expires_ns)
             VALUES (?, ?, ?, 0, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(code)
        .bind(initiator_dev)
        .bind(ephemeral_sk)
        .bind(ephemeral_pk)
        .bind(now)
        .bind(now.saturating_add(ttl_ns))
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("建配对会话", e))?;
        Ok(())
    }

    /// 按 code 查未过期 open 会话。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn pairing_session_by_code(
        &self,
        code: &str,
    ) -> Result<Option<PairingSessionRow>, PartisyError> {
        sqlx::query_as::<_, PairingSessionRow>(
            "SELECT id, code, initiator_dev, responder_dev, state, ephemeral_sk, ephemeral_pk,
                    shared_secret, created_ns, expires_ns
             FROM pairing_session
             WHERE code = ? AND state = 0 AND expires_ns > ?
             ORDER BY created_ns DESC LIMIT 1",
        )
        .bind(code)
        .bind(self.now_ns())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_err("查配对会话", e))
    }

    /// 接收方完成 ECDH 后回写共享密钥与会话状态。
    ///
    /// # Errors
    /// DB 错误 → Fatal；id 不存在 → 0 行影响。
    pub async fn complete_pairing(
        &self,
        id: &str,
        responder_dev: &str,
        shared_secret: &[u8],
    ) -> Result<(), PartisyError> {
        sqlx::query(
            "UPDATE pairing_session
             SET responder_dev = ?, shared_secret = ?, state = 2
             WHERE id = ?",
        )
        .bind(responder_dev)
        .bind(shared_secret)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| db_err("完成配对", e))?;
        Ok(())
    }

    /// 发起方 fetch 已完成会话以派生 device_keypair。
    ///
    /// # Errors
    /// DB 错误 → Fatal；id 不存在 → None。
    pub async fn pairing_session_by_id(
        &self,
        id: &str,
    ) -> Result<Option<PairingSessionRow>, PartisyError> {
        sqlx::query_as::<_, PairingSessionRow>(
            "SELECT id, code, initiator_dev, responder_dev, state, ephemeral_sk, ephemeral_pk,
                    shared_secret, created_ns, expires_ns
             FROM pairing_session WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| db_err("查配对 id", e))
    }
}

use std::str::FromStr as _;
