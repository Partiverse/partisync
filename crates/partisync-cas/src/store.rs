//! 内容寻址块库（SPEC M0-WP03 契约 §2）：objects/ 两级扇出 + SQLite 引用计数索引。
//!
//! 崩溃一致性口径（SPEC 风险节）：对象文件先落盘、索引后提交——
//! 孤儿对象（文件有、行无）由 M3 分代 GC 回收；反向（行有文件无）视为库损坏。

use std::path::{Path, PathBuf};

use partisync_core::error::{PartisyError, Severity};
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};

use crate::chunker::chunk_root;
use crate::content_hash;

/// 块库句柄。
#[derive(Clone)]
pub struct ChunkStore {
    pool: SqlitePool,
    objects: PathBuf,
}

/// 块库统计（块级去重口径：saved = Σ(size×refcount) − Σsize）。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CasStats {
    pub chunks: i64,
    pub chunk_bytes: i64,
    pub refs: i64,
    pub saved_bytes: i64,
}

fn db_err(what: &str, e: sqlx::Error) -> PartisyError {
    PartisyError {
        severity: Severity::Fatal,
        source: Some(format!("{what}: {e}").into()),
    }
}

fn io_err(what: &'static str, e: std::io::Error) -> PartisyError {
    let severity = partisync_core::error::classify_io(e.kind());
    let source: Box<dyn std::error::Error + Send + Sync> = format!("{what}: {e}").into();
    PartisyError {
        severity,
        source: Some(source),
    }
}

impl ChunkStore {
    /// 打开（或创建）块库。`root/objects` 存对象、`root/index.db` 存引用计数。
    ///
    /// # Errors
    /// 目录不可建 / 库不可开 → 按分类。
    pub async fn open(root: &Path) -> Result<Self, PartisyError> {
        let objects = root.join("objects");
        std::fs::create_dir_all(&objects).map_err(|e| io_err("创建块库目录", e))?;
        let opts = SqliteConnectOptions::from_str(&format!(
            "sqlite://{}",
            root.join("index.db").display()
        ))
        .map_err(|e| db_err("连接串", e))?
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        // 同 graph Store（SPEC M0-WP07：WAL+NORMAL 提交不 fsync）
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(opts)
            .await
            .map_err(|e| db_err("打开块索引", e))?;
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS chunks (
                hash     TEXT PRIMARY KEY,
                size     INTEGER NOT NULL,
                refcount INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await
        .map_err(|e| db_err("迁移块索引", e))?;
        Ok(ChunkStore { pool, objects })
    }

    /// 内存索引 + 指定对象目录（测试用）。
    ///
    /// # Errors
    /// 同 [`ChunkStore::open`]。
    pub async fn open_in_memory(root: &Path) -> Result<Self, PartisyError> {
        let objects = root.join("objects");
        std::fs::create_dir_all(&objects).map_err(|e| io_err("创建块库目录", e))?;
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .map_err(|e| db_err("打开内存块索引", e))?;
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS chunks (hash TEXT PRIMARY KEY, size INTEGER NOT NULL, refcount INTEGER NOT NULL DEFAULT 0)",
        )
        .execute(&pool)
        .await
        .map_err(|e| db_err("迁移块索引", e))?;
        Ok(ChunkStore {
            pool,
            objects: objects.to_owned(),
        })
    }

    fn object_path(&self, hash: &str) -> PathBuf {
        // 两级扇出：objects/<h[0..2]>/<hash>
        self.objects.join(&hash[..2]).join(hash)
    }

    /// 入库一块：已存在 ⇒ refcount+1、不重写对象（P4）；新块 ⇒ 写对象 + 插行。
    ///
    /// # Errors
    /// IO 或 DB 错误 → 按分类。
    pub async fn put(&self, data: &[u8]) -> Result<String, PartisyError> {
        let hash = content_hash(data);
        let exists = sqlx::query_scalar::<_, i64>("SELECT refcount FROM chunks WHERE hash = ?")
            .bind(&hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("查询块", e))?;
        if exists.is_some() {
            return self.incr(&hash).await.map(|_| hash);
        }
        let path = self.object_path(&hash);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_err("创建扇出目录", e))?;
        }
        // 先写临时文件再改名：避免半写对象被并发读见
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, data).map_err(|e| io_err("写块对象", e))?;
        std::fs::rename(&tmp, &path).map_err(|e| io_err("提交块对象", e))?;
        sqlx::query("INSERT INTO chunks (hash, size, refcount) VALUES (?, ?, 1)")
            .bind(&hash)
            .bind(i64::try_from(data.len()).unwrap_or(i64::MAX))
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("插入块", e))?;
        Ok(hash)
    }

    /// 取块内容。
    ///
    /// # Errors
    /// 块不存在 → Fatal；读失败 → 按分类。
    pub async fn get(&self, hash: &str) -> Result<Vec<u8>, PartisyError> {
        let path = self.object_path(hash);
        let hash = hash.to_owned();
        tokio::task::spawn_blocking(move || {
            std::fs::read(&path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    PartisyError {
                        severity: Severity::Fatal,
                        source: Some(format!("块不存在: {hash}").into()),
                    }
                } else {
                    io_err("读块对象", e)
                }
            })
        })
        .await
        .map_err(|e| PartisyError::with_source(Severity::Fatal, Box::new(e)))?
    }

    /// 引用计数 +1，返回新值。
    ///
    /// # Errors
    /// 块不存在 → Fatal；DB 错误 → Fatal。
    pub async fn incr(&self, hash: &str) -> Result<i64, PartisyError> {
        sqlx::query("UPDATE chunks SET refcount = refcount + 1 WHERE hash = ?")
            .bind(hash)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("引用+1", e))?;
        sqlx::query_scalar::<_, i64>("SELECT refcount FROM chunks WHERE hash = ?")
            .bind(hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("读引用", e))?
            .ok_or_else(|| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("块不存在: {hash}").into()),
            })
    }

    /// 引用计数 −1，返回新值；归零 ⇒ 删行 + 删对象（GC 微操作，P4 验收）。
    ///
    /// # Errors
    /// 块不存在 → Fatal。
    pub async fn decr(&self, hash: &str) -> Result<i64, PartisyError> {
        sqlx::query("UPDATE chunks SET refcount = refcount - 1 WHERE hash = ?")
            .bind(hash)
            .execute(&self.pool)
            .await
            .map_err(|e| db_err("引用-1", e))?;
        let rc = sqlx::query_scalar::<_, i64>("SELECT refcount FROM chunks WHERE hash = ?")
            .bind(hash)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| db_err("读引用", e))?
            .ok_or_else(|| PartisyError {
                severity: Severity::Fatal,
                source: Some(format!("块不存在: {hash}").into()),
            })?;
        if rc <= 0 {
            sqlx::query("DELETE FROM chunks WHERE hash = ?")
                .bind(hash)
                .execute(&self.pool)
                .await
                .map_err(|e| db_err("删行", e))?;
            let _ = std::fs::remove_file(self.object_path(hash));
            return Ok(0);
        }
        Ok(rc)
    }

    /// 统计（P4/演示口径）。
    ///
    /// # Errors
    /// DB 错误 → Fatal。
    pub async fn stats(&self) -> Result<CasStats, PartisyError> {
        let (chunks, chunk_bytes): (i64, i64) =
            sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(size), 0) FROM chunks")
                .fetch_one(&self.pool)
                .await
                .map_err(|e| db_err("统计", e))?;
        let (refs, ref_bytes): (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(refcount), 0), COALESCE(SUM(size * refcount), 0) FROM chunks",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| db_err("统计引用", e))?;
        Ok(CasStats {
            chunks,
            chunk_bytes,
            refs,
            saved_bytes: ref_bytes - chunk_bytes,
        })
    }
}

use std::str::FromStr as _;

/// 分块入库便捷函数：切块 → 逐块 put → 返回 (块哈希清单, 清单根)。
///
/// # Errors
/// 任一块 put 失败 → 透传。
pub async fn put_chunks(
    store: &ChunkStore,
    data: &[u8],
    cfg: crate::chunker::CdcConfig,
) -> Result<(Vec<String>, String), PartisyError> {
    let mut hashes = Vec::new();
    for (offset, len) in crate::chunker::chunk_boundaries(data, cfg) {
        let h = store.put(&data[offset..offset + len]).await?;
        hashes.push(h);
    }
    let root = chunk_root(&hashes.iter().map(String::as_str).collect::<Vec<_>>());
    Ok((hashes, root))
}
