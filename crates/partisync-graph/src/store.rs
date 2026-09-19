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
        Ok(Store {
            pool,
            oplog_clock: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        })
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
        Ok(Store {
            pool,
            oplog_clock: std::sync::Arc::new(tokio::sync::Mutex::new(None)),
        })
    }

    async fn migrate(pool: &SqlitePool) -> Result<(), PartisyError> {
        sqlx::raw_sql(include_str!("schema.sql"))
            .execute(pool)
            .await
            .map_err(|e| db_err("迁移", e))?;
        // schema v2/v6（M0-WP03/M2-WP01）：旧库防御性补列；新库已含，duplicate 忽略
        let _ = sqlx::raw_sql("ALTER TABLE entry ADD COLUMN chunk_root TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE entry ADD COLUMN owner_device TEXT")
            .execute(pool)
            .await;
        let _ = sqlx::raw_sql("ALTER TABLE entry ADD COLUMN synced_seq INTEGER")
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
            "SELECT id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device FROM entry WHERE path = ?",
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
            "SELECT id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device FROM entry
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
        Ok(key)
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

    /// 应用远端同步条目（设备自有域全量行；冲突 = 保留两者 + 血缘后缀，P11 前置）。
    ///
    /// owner_device 随 payload 走（单写者：非属主不产生本地图谱修改语义——
    /// 远端行就是属主行的投影）。
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
    ) -> Result<String, PartisyError> {
        // 同路径不同属主 = 双端独立创建冲突 → 保留两者：来方改挂冲突后缀
        let mut final_path = path.to_string();
        if let Some(existing) = self.entry_by_path(path).await? {
            let existing_owner = existing
                .owner_device
                .clone()
                .unwrap_or_else(|| "unknown".into());
            if existing_owner != owner_device {
                final_path = format!("{path}.conflict-{owner_device}");
            }
        }
        // 父目录链（远端树的目录由 oplog 目录条目或此处 ensure 保证）
        crate::journal::ensure_dir_chain_pub(self, &final_path).await?;
        let parent_path = match final_path.rsplit_once('/') {
            Some((p, _)) if !p.is_empty() => p.to_string(),
            _ => "/".to_string(),
        };
        let parent_id = self.entry_by_path(&parent_path).await?.map(|e| e.id);
        if let Some((hash, csize)) = content {
            sqlx::query("INSERT OR IGNORE INTO content (id, size) VALUES (?, ?)")
                .bind(hash)
                .bind(i64::try_from(csize).unwrap_or(i64::MAX))
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("登记远端内容", e))?;
        }
        let id = partisync_core::Ulid::now().to_string();
        sqlx::query(
            "INSERT INTO entry (id, parent_id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(path) DO UPDATE SET
                 size = excluded.size, mtime_ns = excluded.mtime_ns,
                 content_id = excluded.content_id, chunk_root = excluded.chunk_root,
                 owner_device = excluded.owner_device
             RETURNING id",
        )
        .bind(&id)
        .bind(parent_id)
        .bind(kind as i64)
        .bind(name)
        .bind(&final_path)
        .bind(content.map(|(h, _)| h))
        .bind(i64::try_from(size).unwrap_or(i64::MAX))
        .bind(i64::try_from(mtime_ns).unwrap_or(i64::MAX))
        .bind(chunk_root)
        .bind(owner_device)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_err("应用远端条目", e))?;
        Ok(id)
    }

    /// 名称子串检索（LIKE，上限防全表外溢）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn search(&self, q: &str, limit: u32) -> Result<Vec<EntryRow>, PartisyError> {
        sqlx::query_as::<_, EntryRow>(
            "SELECT id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device FROM entry
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
                "SELECT id, kind, name, path, content_id, size, mtime_ns, chunk_root, owner_device FROM entry
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

use std::str::FromStr as _;
