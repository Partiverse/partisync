//! 写回日志（SPEC M8-WP07 §2.1/§2.3；P16：先日志后应用 + 重放幂等）。
//!
//! 形态（拆卡落锤 Q2）：backing 旁 `.partisync-writeback/wal.jsonl`
//! append-only JSONL，单文件顺序写（无 fsync 放大——R4；kill -9 相位
//! 由 `flush()` 保证，掉电相位归 T03 crash 矩阵）。
//!
//! 执行协议：**append → apply → 压实**。apply 失败时 backing 保持原状
//! （unlink/rmdir/rmdir 在破坏性 syscall 前失败；rename 为内核单原子
//! 操作）→ 回滚刚 append 的日志项，拒绝先于破坏性效果（P15 延续）。
//! 崩溃后重挂 `recover()`：逐条重放（幂等——同一日志项重放 N 次结果
//! 一致），全量成功后压实为空。
//!
//! 日志项 payload 只含路径与操作（不引用 CAS 对象）→ claim_due 协议
//! 零感知（GC 零交互面，落锤 Q2）。

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// 写回日志目录名（backing root 下；readdir/索引接线按此名隐藏）。
pub const WAL_DIR: &str = ".partisync-writeback";

const WAL_FILE: &str = "wal.jsonl";

/// 写回操作（相对 backing root 的 '/' 分隔路径）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WriteBackOp {
    /// 删除文件。
    Unlink { path: String },
    /// 删除空目录（非空在预处理层被拒，不入日志）。
    Rmdir { path: String },
    /// 同挂载点内改名（跨目录允许；目录拓扑仍由同步管线管理）。
    Rename { from: String, to: String },
}

/// 一条日志项（seq 单调；压实只删已应用项，seq 永不重排）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalEntry {
    pub seq: u64,
    #[serde(flatten)]
    pub op: WriteBackOp,
}

/// 写回日志句柄（单挂载点单实例；`Mutex` 串行化 append/压实）。
pub struct WriteBackLog {
    backing: PathBuf,
    wal: PathBuf,
    next_seq: Mutex<u64>,
}

impl WriteBackLog {
    /// 打开（不重放——恢复走 [`Self::recover`]）。
    ///
    /// # Errors
    /// 日志目录/文件不可创建或不可读 → `io::Error`（挂载失败，fail-fast）。
    pub fn open(backing: &Path) -> io::Result<Self> {
        let dir = backing.join(WAL_DIR);
        fs::create_dir_all(&dir)?;
        let wal = dir.join(WAL_FILE);
        let next_seq = match fs::read_to_string(&wal) {
            Ok(text) => text
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str::<WalEntry>(l).ok())
                .map(|e| e.seq)
                .max()
                .map_or(1, |s| s + 1),
            Err(e) if e.kind() == io::ErrorKind::NotFound => 1,
            Err(e) => return Err(e),
        };
        Ok(Self {
            backing: backing.to_path_buf(),
            wal,
            next_seq: Mutex::new(next_seq),
        })
    }

    /// 追加一条日志项（append + flush；不 fsync——R4 设计口径）。
    ///
    /// # Errors
    /// 序列化或写失败 → `io::Error`（调用方放弃本次写回，backing 未动）。
    pub fn append(&self, op: &WriteBackOp) -> io::Result<u64> {
        let seq = {
            let mut n = self.next_seq.lock().unwrap();
            let s = *n;
            *n += 1;
            s
        };
        let entry = WalEntry {
            seq,
            op: op.clone(),
        };
        let mut line = serde_json::to_string(&entry).map_err(io::Error::other)?;
        line.push('\n');
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal)?;
        f.write_all(line.as_bytes())?;
        f.flush()?;
        Ok(seq)
    }

    /// 应用单条操作到 backing（幂等：重放 N 次结果一致）。
    ///
    /// # Errors
    /// unlink：目标是目录 → `EISDIR`；rmdir：非空 → `ENOTEMPTY`、目标是
    /// 文件 → `ENOTDIR`；rename：内核语义错误透传（原子操作，无半提交）。
    /// `NotFound` 一律视为已生效（幂等重放的收敛路径）。
    pub fn apply_one(&self, op: &WriteBackOp) -> io::Result<()> {
        match op {
            WriteBackOp::Unlink { path } => {
                let p = self.backing.join(rel(path));
                match fs::symlink_metadata(&p) {
                    Ok(md) if md.is_dir() => {
                        Err(io::Error::from_raw_os_error(21)) // EISDIR
                    }
                    Ok(_) => fs::remove_file(&p).or_else(not_found_ok),
                    Err(_) => Ok(()), // 已不存在 = 已生效
                }
            }
            WriteBackOp::Rmdir { path } => {
                let p = self.backing.join(rel(path));
                match fs::symlink_metadata(&p) {
                    Ok(md) if !md.is_dir() => Err(io::Error::from_raw_os_error(20)), // ENOTDIR
                    Ok(_) => fs::remove_dir(&p).or_else(not_found_ok),
                    Err(_) => Ok(()),
                }
            }
            WriteBackOp::Rename { from, to } => {
                let f = self.backing.join(rel(from));
                let t = self.backing.join(rel(to));
                if fs::symlink_metadata(&f).is_err() {
                    return Ok(()); // 源已不在 = 已生效（或重复重放）
                }
                fs::rename(&f, &t).or_else(not_found_ok)
            }
        }
    }

    /// 执行一次写回（协议：append → apply → 压实）。
    ///
    /// apply 失败 → 回滚刚 append 的日志项（backing 未动，P15），
    /// 错误透传给调用方映射 errno。
    ///
    /// # Errors
    /// append 失败（backing 未动）或 apply 失败（backing 原状）→ `io::Error`。
    pub fn execute(&self, op: &WriteBackOp) -> io::Result<()> {
        let seq = self.append(op)?;
        match self.apply_one(op) {
            Ok(()) => {
                self.compact_applied(&BTreeSet::from([seq]))?;
                Ok(())
            }
            Err(e) => {
                // 回滚日志项：apply 未产生破坏性效果（unlink/rmdir 在
                // syscall 前失败；rename 原子），删除该项保持日志只含
                // 未应用操作。回滚自身失败不掩盖主错误（下次 recover
                // 重放该幂等项无害）。
                let _ = self.compact_applied(&BTreeSet::from([seq]));
                Err(e)
            }
        }
    }

    /// 崩溃恢复：重放全部日志项（幂等），成功后压实为空。
    /// 返回重放条数。
    ///
    /// # Errors
    /// 任一项重放失败 → `io::Error`（日志保留，下次挂载再试——fail-fast，
    /// 不静默丢弃未应用操作）。
    pub fn recover(&self) -> io::Result<usize> {
        let entries = self.read_all()?;
        let n = entries.len();
        for e in &entries {
            self.apply_one(&e.op)?;
        }
        if n > 0 {
            self.compact_applied(&entries.iter().map(|e| e.seq).collect())?;
        }
        Ok(n)
    }

    fn read_all(&self) -> io::Result<Vec<WalEntry>> {
        let text = match fs::read_to_string(&self.wal) {
            Ok(t) => t,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        Ok(text
            .lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect())
    }

    /// 压实：原子重写 WAL 只保留未应用项（tmp + rename，无半文件）。
    fn compact_applied(&self, applied: &BTreeSet<u64>) -> io::Result<()> {
        let keep: Vec<WalEntry> = self
            .read_all()?
            .into_iter()
            .filter(|e| !applied.contains(&e.seq))
            .collect();
        let tmp = self.wal.with_extension("tmp");
        {
            let mut f = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp)?;
            for e in keep {
                let mut line = serde_json::to_string(&e).map_err(io::Error::other)?;
                line.push('\n');
                f.write_all(line.as_bytes())?;
            }
            f.flush()?;
        }
        fs::rename(&tmp, &self.wal)
    }
}

/// '/' 分隔的日志路径 → 相对 `Path`（拒绝 `..` 与绝对注入——日志可能来自
/// crash 前的磁盘，不可信）。
fn rel(path: &str) -> PathBuf {
    path.split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .filter(|c| *c != "..")
        .collect::<PathBuf>()
}

/// `NotFound` → 已生效（幂等重放收敛）；其余错误透传。
fn not_found_ok(e: io::Error) -> io::Result<()> {
    if e.kind() == io::ErrorKind::NotFound {
        Ok(())
    } else {
        Err(e)
    }
}
