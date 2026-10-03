//! PartiFuse 文件系统实现（SEMANTICS.md 语义的唯一实现点）。
//!
//! 从 M7-WP03-T01 spike（crates/partisync-fuse-spike）移植产品化：
//! 语义逐行保持；差异 = 审计标注 + 权限模型注释 + ino 表/写句柄封装不变。

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use fuser::{
    Errno, FileAttr, FileHandle, FileType, Filesystem, INodeNo, OpenAccMode, OpenFlags, ReplyAttr,
    ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyWrite, Request,
    TimeOrNow,
};

use std::sync::Arc;

use partisync_cas::ChunkStore;

use crate::writeback::{self, WriteBackOp, WAL_DIR};

/// by-hash 虚拟树节点分类。
#[derive(Debug, Clone, PartialEq, Eq)]
enum ByHashKind {
    /// `by-hash` 目录。
    HashRoot,
    /// `by-hash/blake3` 目录。
    Algo,
    /// `by-hash/blake3/<digest>` 内容文件。
    Digest(String),
}

/// by-hash 虚拟命名空间根（SPEC M8-WP07 §2.2）：挂载根下
/// `by-hash/blake3/<digest>` 直连 CAS 只读。
const BY_HASH: &str = "by-hash";
const BY_HASH_ALGO: &str = "blake3";
/// digest 合法形态：blake3 hex = 64 个小写十六进制字符。
fn is_digest(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
const EROFS: i32 = 30;

const TTL: Duration = Duration::from_secs(1);

/// 打开的写句柄：新文件顺序写游标（offset 必须等于 cur_len）。
struct WriteHandle {
    path: PathBuf,
    cur_len: u64,
    /// overlay 暂存会话（M8-WP07-T03）：Some = 已存在文件的整文件替换
    /// 会话（写入 staging，release 时原子 rename 到位）。
    overlay: Option<OverlaySession>,
}

/// 整文件替换暂存会话。
struct OverlaySession {
    /// staging blob 绝对路径。
    staging: PathBuf,
    /// WAL 暂存名（release 时随 Replace 日志项下发）。
    staging_name: String,
    /// 目标相对路径（日志/holder 键）。
    target_rel: String,
}

struct InoTable {
    /// ino → 后备相对路径（root = ""）
    paths: HashMap<u64, PathBuf>,
    /// 路径 → ino（路径规范化后复用）
    by_path: HashMap<PathBuf, u64>,
    next: u64,
}

/// 诊断日志（容器探针调试用；`PARTIFUSE_DBG_LOG` 未设时零开销静默）。
pub(crate) fn dbg_log(msg: impl std::fmt::Display) {
    if let Ok(path) = std::env::var("PARTIFUSE_DBG_LOG") {
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = std::io::Write::write_all(&mut f, format!("{msg}\n").as_bytes());
        }
    }
}

impl InoTable {
    fn new() -> Self {
        let mut paths = HashMap::new();
        paths.insert(1u64, PathBuf::new());
        Self {
            paths,
            by_path: HashMap::new(),
            next: 2,
        }
    }

    fn get_or_alloc(&mut self, rel: &Path) -> u64 {
        if let Some(ino) = self.by_path.get(rel) {
            return *ino;
        }
        let ino = self.next;
        self.next += 1;
        self.paths.insert(ino, rel.to_path_buf());
        self.by_path.insert(rel.to_path_buf(), ino);
        ino
    }

    /// rename 后同步表（内核缓存 inode 号在 rename 后不变——不同步则
    /// open(旧 ino) 解析到旧 rel，backing 已无此文件 → ENOENT，容器
    /// 探针实证）。
    fn rename_entry(&mut self, old: &Path, new: &Path) {
        if let Some(ino) = self.by_path.remove(old) {
            self.paths.insert(ino, new.to_path_buf());
            self.by_path.insert(new.to_path_buf(), ino);
        }
    }

    /// unlink/rmdir 后清理表项（inode 复用时不得指向已删除路径）。
    fn remove_entry(&mut self, rel: &Path) {
        if let Some(ino) = self.by_path.remove(rel) {
            self.paths.remove(&ino);
        }
    }
}

pub struct PartiFuse {
    backing: PathBuf,
    table: Mutex<InoTable>,
    writes: Mutex<HashMap<u64, WriteHandle>>,
    /// 写回日志（M8-WP07-T02；P16：先日志后应用 + 重放幂等）。
    writeback: writeback::WriteBackLog,
    /// overlay 暂存 holder（落锤 Q1：同路径暂存持有期间第二写打开
    /// EBUSY）。键 = 目标相对路径。
    overlay_holders: Mutex<std::collections::BTreeSet<String>>,
    /// overlay 暂存 blob 计数（staging 文件名唯一化）。
    staging_seq: std::sync::atomic::AtomicU64,
    /// CAS 直连（M8-WP07-T04 by-hash；None = 未装配，虚拟树返回 ENOENT）。
    cas: Option<Arc<ChunkStore>>,
    /// CAS 异步 API 的专用 runtime（fuser handler 为同步线程）。
    cas_rt: tokio::runtime::Runtime,
    /// by-hash 打开文件的内容缓存（open 时取一次，read 直读）。
    cas_reads: Mutex<HashMap<u64, Arc<Vec<u8>>>>,
    /// 装配层事件汇（M9-WP01-T01；None = 未装配，行为与 M8 全等）。
    events: Option<crate::events::EventSink>,
}

impl PartiFuse {
    pub fn new(backing: PathBuf) -> Self {
        Self::with_cas(backing, None)
    }

    /// 带 CAS 装配（by-hash 命名空间激活；gateway/`partifuse --cas` 用）。
    ///
    /// # Panics
    /// 写回日志初始化/恢复失败（同 [`Self::new`]）或 CAS runtime 创建失败。
    pub fn with_cas(backing: PathBuf, cas: Option<Arc<ChunkStore>>) -> Self {
        Self::assemble(backing, cas, None)
    }

    /// 带装配层事件汇（M9-WP01-T01；gateway/`partifuse --graph` 用）。
    /// 挂载写 apply 成功 → 事件发汇端；收端掉线时事件丢弃（发送
    /// 静默失败——挂载面永不因装配层阻塞，丢失窗口由 bisync 全量
    /// 对账兜底，SEMANTICS §同步接线节登记）。
    ///
    /// # Panics
    /// 同 [`Self::with_cas`]。
    pub fn with_wiring(
        backing: PathBuf,
        cas: Option<Arc<ChunkStore>>,
        events: crate::events::EventSink,
    ) -> Self {
        Self::assemble(backing, cas, Some(events))
    }

    fn assemble(
        backing: PathBuf,
        cas: Option<Arc<ChunkStore>>,
        events: Option<crate::events::EventSink>,
    ) -> Self {
        let writeback = writeback::WriteBackLog::open(&backing)
            .and_then(|log| {
                log.recover()?;
                Ok(log)
            })
            .expect("写回日志初始化/恢复失败（.partisync-writeback 不可用或重放出错）");
        let cas_rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("CAS runtime 创建失败");
        Self {
            backing,
            table: Mutex::new(InoTable::new()),
            writes: Mutex::new(HashMap::new()),
            writeback,
            overlay_holders: Mutex::new(std::collections::BTreeSet::new()),
            staging_seq: std::sync::atomic::AtomicU64::new(1),
            cas,
            cas_rt,
            cas_reads: Mutex::new(HashMap::new()),
            events,
        }
    }

    /// 事件发汇（apply 成功分支调用；收端掉线静默丢弃——见
    /// [`Self::with_wiring`]）。
    fn emit(&self, event: crate::events::FuseWriteEvent) {
        if let Some(tx) = &self.events {
            let _ = tx.send(event);
        }
    }

    /// 是否写回日志内部路径（挂载面上隐藏 + 写操作拒绝）。
    fn is_internal(rel: &Path) -> bool {
        rel.starts_with(WAL_DIR)
    }

    /// by-hash 虚拟树分类（None = 不在虚拟树）：
    /// `HashRoot`（by-hash）/ `Algo`（by-hash/blake3）/ `Digest`（具体内容）。
    fn by_hash_kind(rel: &Path) -> Option<ByHashKind> {
        let parts: Vec<_> = rel
            .iter()
            .map(|c| c.to_string_lossy().into_owned())
            .collect();
        match parts.as_slice() {
            [b] if b == BY_HASH => Some(ByHashKind::HashRoot),
            [b, a] if b == BY_HASH && a == BY_HASH_ALGO => Some(ByHashKind::Algo),
            [b, a, d] if b == BY_HASH && a == BY_HASH_ALGO && is_digest(d) => {
                Some(ByHashKind::Digest(d.clone()))
            }
            _ => None,
        }
    }

    /// 虚拟目录 attr（by-hash 与 blake3 层）。
    fn vdir_attr(ino: INodeNo) -> FileAttr {
        let now = UNIX_EPOCH;
        FileAttr {
            ino,
            size: 0,
            blocks: 0,
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: FileType::Directory,
            perm: 0o555,
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }

    /// 写类操作对 by-hash 虚拟树返回 EROFS（SPEC §2.2）。
    fn vdir_write_errno(rel: &Path) -> Option<Errno> {
        Self::by_hash_kind(rel).map(|_| Errno::from_i32(EROFS))
    }

    /// CAS 是否装配（by-hash 虚拟树是否激活）。
    fn cas_available(&self) -> bool {
        self.cas.is_some()
    }

    /// CAS 内容文件 attr（只读、uid/gid 挂载进程、nlink 1）。
    fn cas_file_attr(size: u64, ino: INodeNo) -> FileAttr {
        let now = UNIX_EPOCH;
        FileAttr {
            ino,
            size,
            blocks: size.div_ceil(512),
            atime: now,
            mtime: now,
            ctime: now,
            crtime: now,
            kind: FileType::RegularFile,
            perm: 0o444,
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }

    /// CAS 内容按 digest 读取（block_on 专用 runtime）。读失败（含
    /// digest 不存在）一律 `None` → 调用方映射 ENOENT（脚本视角
    /// 「digest 无效 = 无内容」语义；IO 错误同形登记 SEMANTICS）。
    fn cas_get(&self, digest: &str) -> Option<Vec<u8>> {
        let cas = self.cas.as_ref()?;
        self.cas_rt.block_on(async { cas.get(digest).await.ok() })
    }

    fn apply_overlay_replace(&self, target_rel: &str, staging_name: &str) -> std::io::Result<()> {
        let op = WriteBackOp::Replace {
            path: target_rel.to_string(),
            staging: staging_name.to_string(),
        };
        self.writeback.execute(&op).inspect(|()| {
            self.emit(crate::events::FuseWriteEvent::Upsert {
                path: target_rel.to_string(),
            });
        })
    }

    /// 相对路径 → 日志用 '/' 分隔字符串（unix 分隔符即 '/'；反斜杠是
    /// 合法文件名字符，不转换）。
    fn rel_str(rel: &Path) -> String {
        rel.to_string_lossy().into_owned()
    }

    fn backing_path(&self, ino: INodeNo) -> Option<PathBuf> {
        self.table
            .lock()
            .unwrap()
            .paths
            .get(&ino.0)
            .map(|rel| self.backing.join(rel))
    }

    /// 表内**相对**路径（写回日志只能存相对路径——绝对路径进 WAL 会在
    /// `rel()` 重建时错位，容器探针实证的集成缺陷）。
    fn rel_path(&self, ino: INodeNo) -> Option<PathBuf> {
        self.table.lock().unwrap().paths.get(&ino.0).cloned()
    }

    fn attr_from(md: &fs::Metadata, ino: INodeNo) -> FileAttr {
        let (kind, size, mtime) = if md.is_dir() {
            (FileType::Directory, 0, md.modified())
        } else {
            (FileType::RegularFile, md.len(), md.modified())
        };
        let mtime = mtime.unwrap_or(UNIX_EPOCH);
        FileAttr {
            ino,
            size,
            blocks: size.div_ceil(512),
            atime: mtime,
            mtime,
            ctime: mtime,
            crtime: mtime,
            kind,
            perm: 0o644,
            nlink: 1,
            uid: md.uid(),
            gid: md.gid(),
            rdev: 0,
            flags: 0,
            blksize: 512,
        }
    }
}

impl Filesystem for PartiFuse {
    fn init(&mut self, _req: &Request, _config: &mut fuser::KernelConfig) -> std::io::Result<()> {
        Ok(())
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        // 表键统一用**相对**路径（T03 修复：lookup 原走 backing_path
        // 绝对链、写回 handler 走 rel_path 相对链——键形态不一致导致
        // rename_entry/remove_entry 永不命中，容器探针实证）。
        let Some(parent_rel) = self.rel_path(parent) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let rel = parent_rel.join(name);
        if Self::is_internal(&rel) {
            // 写回日志目录在挂载面不可见（落锤 Q2）
            reply.error(Errno::EACCES);
            return;
        }
        // by-hash 虚拟树（M8-WP07-T04）：合成条目，digest 不存在 → ENOENT
        if let Some(kind) = Self::by_hash_kind(&rel) {
            if !self.cas_available() {
                reply.error(Errno::ENOENT);
                return;
            }
            let ino = self.table.lock().unwrap().get_or_alloc(&rel);
            match kind {
                ByHashKind::Digest(d) => match self.cas_get(&d) {
                    Some(bytes) => {
                        let size = bytes.len() as u64;
                        self.cas_reads.lock().unwrap().insert(ino, Arc::new(bytes));
                        reply.entry(
                            &TTL,
                            &Self::cas_file_attr(size, INodeNo(ino)),
                            fuser::Generation(0),
                        );
                    }
                    None => reply.error(Errno::ENOENT),
                },
                _ => reply.entry(&TTL, &Self::vdir_attr(INodeNo(ino)), fuser::Generation(0)),
            }
            return;
        }
        let abs = self.backing.join(&rel);
        match fs::symlink_metadata(&abs) {
            Ok(md) if md.is_file() || md.is_dir() => {
                let ino = self.table.lock().unwrap().get_or_alloc(&rel);
                reply.entry(
                    &TTL,
                    &Self::attr_from(&md, INodeNo(ino)),
                    fuser::Generation(0),
                );
            }
            Ok(_) => reply.error(Errno::EPERM), // symlink 等不支持类型：显式拒绝
            Err(_) => reply.error(Errno::ENOENT),
        }
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        // by-hash 虚拟树 getattr（T04）
        if let Some(rel) = self.rel_path(ino) {
            if let Some(kind) = Self::by_hash_kind(&rel) {
                match kind {
                    ByHashKind::Digest(d) => {
                        let Some(bytes) = self.cas_get(&d) else {
                            reply.error(Errno::ENOENT);
                            return;
                        };
                        reply.attr(&TTL, &Self::cas_file_attr(bytes.len() as u64, ino));
                    }
                    _ => reply.attr(&TTL, &Self::vdir_attr(ino)),
                }
                return;
            }
        }
        let Some(abs) = self.backing_path(ino) else {
            reply.error(Errno::ENOENT);
            return;
        };
        match fs::symlink_metadata(&abs) {
            Ok(md) => reply.attr(&TTL, &Self::attr_from(&md, ino)),
            Err(_) => reply.error(Errno::ENOENT),
        }
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<TimeOrNow>,
        _mtime: Option<TimeOrNow>,
        _ctime: Option<std::time::SystemTime>,
        _fh: Option<FileHandle>,
        _crtime: Option<std::time::SystemTime>,
        _chgtime: Option<std::time::SystemTime>,
        _bkuptime: Option<std::time::SystemTime>,
        _flags: Option<fuser::BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        // M8-WP07-T03：truncate = 整文件替换特例（SPEC §2.1）——原子
        // 替换为原内容截断到 size（size 0 = 清空写）；其余 setattr 形式
        // （perm/时间戳）维持拒绝（一期口径）。
        let Some(size) = size else {
            reply.error(Errno::EPERM);
            return;
        };
        let Some(rel) = self.rel_path(ino) else {
            reply.error(Errno::ENOENT);
            return;
        };
        if Self::is_internal(&rel) {
            reply.error(Errno::EACCES);
            return;
        }
        let target_rel = Self::rel_str(&rel);
        if let Some(e) = Self::vdir_write_errno(&rel) {
            reply.error(e);
            return;
        }
        // fuser 判例：O_TRUNC 走 open → setattr(size) 序列。若该路径
        // 已有 overlay 会话（内核紧随 open 的 truncate），在 staging 上
        // 截断（后续 release 整文件替换生效）——非 EBUSY。
        if self.overlay_holders.lock().unwrap().contains(&target_rel) {
            let mut writes = self.writes.lock().unwrap();
            let Some(h) = writes.get_mut(&ino.0) else {
                reply.error(Errno::EBUSY);
                return;
            };
            let Some(o) = h.overlay.as_ref() else {
                reply.error(Errno::EBUSY);
                return;
            };
            // set_len 直接 resize（保留前 size 字节）——不可用
            // truncate(true)+set_len 组合（先清零会把预填内容变零，容器
            // 探针实证 [0,0]）。
            if fs::OpenOptions::new()
                .write(true)
                .open(&o.staging)
                .and_then(|f| f.set_len(size))
                .is_err()
            {
                reply.error(Errno::EIO);
                return;
            }
            h.cur_len = h.cur_len.min(size);
            // attr 以 staging 现状为准（backing 在 release 前保持旧内容）
            match fs::symlink_metadata(&o.staging) {
                Ok(md) => {
                    let mut attr = Self::attr_from(&md, ino);
                    attr.size = size;
                    attr.blocks = size.div_ceil(512);
                    reply.attr(&TTL, &attr);
                }
                Err(_) => reply.error(Errno::EIO),
            }
            return;
        }
        let abs = self.backing.join(&rel);
        let md = match fs::symlink_metadata(&abs) {
            Ok(md) if md.is_file() => md,
            Ok(_) => {
                reply.error(Errno::EPERM);
                return;
            }
            Err(_) => {
                reply.error(Errno::ENOENT);
                return;
            }
        };
        let staging_name = format!("{ino}-t");
        let staging = self.writeback.staging_path(&staging_name);
        // 预填 = 原内容截断到 size（整文件替换特例的内容语义）
        let pre = match fs::read(&abs) {
            Ok(mut buf) => {
                buf.truncate(size as usize);
                buf
            }
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };
        if fs::write(&staging, &pre).is_err() {
            reply.error(Errno::EIO);
            return;
        }
        let op = WriteBackOp::Replace {
            path: target_rel.clone(),
            staging: staging_name,
        };
        match self.writeback.execute(&op) {
            Ok(()) => {
                self.emit(crate::events::FuseWriteEvent::Upsert {
                    path: target_rel.clone(),
                });
                let new_len = pre.len() as u64;
                let mut attr = Self::attr_from(&md, ino);
                attr.size = new_len;
                attr.blocks = new_len.div_ceil(512);
                reply.attr(&TTL, &attr);
            }
            Err(e) => reply.error(e.into()),
        }
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        // by-hash 内容文件（T04）：只读打开（内容缓存一次）；写打开 EROFS
        if let Some(rel) = self.rel_path(ino) {
            if let Some(ByHashKind::Digest(d)) = Self::by_hash_kind(&rel) {
                if flags.acc_mode() != OpenAccMode::O_RDONLY {
                    reply.error(Errno::from_i32(EROFS));
                    return;
                }
                match self.cas_get(&d) {
                    Some(bytes) => {
                        self.cas_reads
                            .lock()
                            .unwrap()
                            .insert(ino.0, Arc::new(bytes));
                        reply.opened(FileHandle(ino.0), fuser::FopenFlags::empty());
                    }
                    None => reply.error(Errno::ENOENT),
                }
                return;
            }
        }
        let Some(abs) = self.backing_path(ino) else {
            dbg_log("[DBG] open NO-PATH");
            reply.error(Errno::ENOENT);
            return;
        };
        let md = match fs::symlink_metadata(&abs) {
            Ok(md) => md,
            Err(_) => {
                reply.error(Errno::ENOENT);
                return;
            }
        };
        if md.is_dir() {
            // 目录打开（readdir 前置）只允许只读
            if flags.acc_mode() != OpenAccMode::O_RDONLY {
                reply.error(Errno::EISDIR);
            } else {
                reply.opened(FileHandle(ino.0), fuser::FopenFlags::empty());
            }
            return;
        }
        // 写打开已存在文件 → overlay 暂存会话（M8-WP07-T03；SPEC §2.1
        // 「写打开已存在文件」行）：写入 staging，release 时整文件替换。
        // 落锤 Q1：同路径已有未 release 暂存 → EBUSY（不排队不覆盖）。
        // 注（fuser 判例）：O_TRUNC 被 ABI strip（lib.rs:569），内核对
        // O_TRUNC 走 open → setattr(size=0) 序列——截断语义在 setattr
        // 分支落地（staging 截断，非 EBUSY）。
        if flags.acc_mode() != OpenAccMode::O_RDONLY {
            let rel = match self.rel_path(ino) {
                Some(r) => r,
                None => {
                    reply.error(Errno::ENOENT);
                    return;
                }
            };
            let target_rel = Self::rel_str(&rel);
            {
                let mut holders = self.overlay_holders.lock().unwrap();
                if !holders.insert(target_rel.clone()) {
                    reply.error(Errno::EBUSY);
                    return;
                }
            }
            let seq = self
                .staging_seq
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let staging_name = format!("{}-{seq}.staged", ino.0);
            let staging = self.writeback.staging_path(&staging_name);
            // 预填 backing 现内容（「打开-截短-改尾」编辑器模式安全；
            // 内核随后的 setattr(size) 在 staging 上截断）
            dbg_log(format!(
                "[DBG] open-w copying {} -> {}",
                abs.display(),
                staging.display()
            ));
            let cur_len = match fs::copy(&abs, &staging) {
                Ok(n) => n,
                Err(_) => {
                    self.overlay_holders.lock().unwrap().remove(&target_rel);
                    reply.error(Errno::EIO);
                    return;
                }
            };
            self.writes.lock().unwrap().insert(
                ino.0,
                WriteHandle {
                    path: abs,
                    cur_len,
                    overlay: Some(OverlaySession {
                        staging,
                        staging_name,
                        target_rel,
                    }),
                },
            );
            reply.opened(FileHandle(ino.0), fuser::FopenFlags::empty());
            return;
        }
        reply.opened(FileHandle(ino.0), fuser::FopenFlags::empty());
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyData,
    ) {
        // by-hash 内容读（T04）：open 时的缓存切片
        let cas_bytes = self.cas_reads.lock().unwrap().get(&ino.0).cloned();
        if let Some(bytes) = cas_bytes {
            let start = (offset as usize).min(bytes.len());
            let end = (start + size as usize).min(bytes.len());
            reply.data(&bytes[start..end]);
            return;
        }
        let Some(abs) = self.backing_path(ino) else {
            reply.error(Errno::ENOENT);
            return;
        };
        // 随机读：pread 语义（offset 任意）
        let mut file = match fs::File::open(&abs) {
            Ok(f) => f,
            Err(_) => {
                reply.error(Errno::ENOENT);
                return;
            }
        };
        use std::io::{Seek, SeekFrom};
        if file.seek(SeekFrom::Start(offset)).is_err() {
            reply.error(Errno::EINVAL);
            return;
        }
        let mut buf = vec![0u8; size as usize];
        match file.read(&mut buf) {
            Ok(n) => {
                buf.truncate(n);
                reply.data(&buf);
            }
            Err(_) => reply.error(Errno::EIO),
        }
    }

    fn create(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        _flags: i32,
        reply: ReplyCreate,
    ) {
        if let Some(parent_rel) = self.rel_path(parent) {
            if let Some(e) = Self::vdir_write_errno(&parent_rel.join(name)) {
                reply.error(e);
                return;
            }
        }
        let Some(parent_rel) = self.backing_path(parent) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let rel = parent_rel.join(name);
        let abs = self.backing.join(&rel);
        // O_CREAT|O_EXCL 强制：create 只允许真正的新建（存在即 EEXIST，
        // 与「已存在文件不可写打开」同一条语义的两面）
        if let Err(e) = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&abs)
        {
            reply.error(match e.kind() {
                std::io::ErrorKind::AlreadyExists => Errno::EEXIST,
                _ => Errno::EACCES,
            });
            return;
        }
        let md = fs::symlink_metadata(&abs).expect("create_new 刚成功");
        let ino = self.table.lock().unwrap().get_or_alloc(&rel);
        let fh = ino; // fh = ino（本 FS 中写句柄与 ino 一一对应）
        self.writes.lock().unwrap().insert(
            fh,
            WriteHandle {
                path: abs,
                cur_len: 0,
                overlay: None,
            },
        );
        // 事件路径必须 backing 相对——create 的 rel 经 backing_path()
        // 构造是绝对路径（join 语义，M8 时代直写 fs 恰好兼容）；strip
        // backing 前缀还原相对口径（T03 容器探针抓出：绝对路径事件把
        // 整条目录链灌进 graph）。
        let event_rel = rel.strip_prefix(&self.backing).unwrap_or(rel.as_path());
        self.emit(crate::events::FuseWriteEvent::Upsert {
            path: Self::rel_str(event_rel),
        });
        reply.created(
            &TTL,
            &Self::attr_from(&md, INodeNo(ino)),
            fuser::Generation(0),
            FileHandle(fh),
            fuser::FopenFlags::empty(),
        );
    }

    fn write(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: fuser::WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        reply: ReplyWrite,
    ) {
        let mut writes = self.writes.lock().unwrap();
        let Some(h) = writes.get_mut(&fh.0) else {
            reply.error(Errno::EBADF);
            return;
        };
        // 顺序写契约：offset 必须 == 当前长度（追加），否则 EINVAL。
        // 新文件写直落 backing；overlay 会话（已存在文件替换）写 staging。
        if offset != h.cur_len {
            reply.error(Errno::EINVAL);
            return;
        }
        let mut file = match fs::OpenOptions::new()
            .append(true)
            .open(h.overlay.as_ref().map_or(&h.path, |o| &o.staging))
        {
            Ok(f) => f,
            Err(_) => {
                reply.error(Errno::EIO);
                return;
            }
        };
        match file.write_all(data) {
            Ok(()) => {
                h.cur_len += data.len() as u64;
                reply.written(data.len() as u32);
            }
            Err(_) => reply.error(Errno::EIO),
        }
    }

    /// overlay 整文件替换（P16 协议）：WAL Replace 先落盘 → staging
    /// rename 到位（同 FS 原子，无半提交）→ 压实。apply 失败 → backing
    /// 原状 + 回滚日志。幂等：blob 不在（已应用）时 no-op——flush 可
    /// 多次触发、release 兜底重放均安全。
    fn flush(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        _lock_owner: fuser::LockOwner,
        reply: ReplyEmpty,
    ) {
        // FUSE 判例：release 异步（内核 close 不等 release）——overlay
        // 整文件替换必须在 **flush（同步）** 应用，保证 close 返回时
        // backing 已是新内容（close 后一致性，mountpoint-s3 判例翻转：
        // 它 close 后对象才出现，我们 close 后 backing 即新）。flush 可
        // 多次触发（dup fd）——Replace 幂等。release 兜底重放（fd 泄漏
        // 路径）。写会话保持开启（其他 dup fd 可继续写，再 flush 再替换）。
        let overlay = {
            let writes = self.writes.lock().unwrap();
            writes
                .get(&fh.0)
                .and_then(|h| h.overlay.as_ref())
                .map(|o| (o.target_rel.clone(), o.staging_name.clone()))
        };
        if let Some((target_rel, staging_name)) = overlay {
            if self
                .apply_overlay_replace(&target_rel, &staging_name)
                .is_err()
            {
                // 替换失败：backing 原状（P16），release 兜底会再试
            }
        }
        reply.ok();
    }

    fn release(
        &self,
        _req: &Request,
        _ino: INodeNo,
        fh: FileHandle,
        _flags: OpenFlags,
        _lock_owner: Option<fuser::LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        let handle = self.writes.lock().unwrap().remove(&fh.0);
        if let Some(h) = handle {
            if let Some(o) = h.overlay {
                // 兜底：flush 已应用过则 Replace 幂等 no-op（blob 不在）；
                // 无 flush 路径（fd 泄漏后强制 umount）在此完成最终替换。
                if self
                    .apply_overlay_replace(&o.target_rel, &o.staging_name)
                    .is_err()
                {
                    // 兜底替换失败：backing 原状，下次 recover/flush 再试
                }
                self.overlay_holders.lock().unwrap().remove(&o.target_rel);
            }
        }
        reply.ok();
    }

    fn readdir(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        mut reply: ReplyDirectory,
    ) {
        let Some(abs) = self.backing_path(ino) else {
            reply.error(Errno::ENOENT);
            return;
        };
        // by-hash 虚拟树 readdir（T04/R6 受限口径）：by-hash → [blake3]；
        // blake3 层不枚举 digest（lookup-only 导航）
        if let Some(rel) = self.rel_path(ino) {
            match Self::by_hash_kind(&rel) {
                Some(ByHashKind::HashRoot) => {
                    let ino = self
                        .table
                        .lock()
                        .unwrap()
                        .get_or_alloc(&Path::new(BY_HASH).join(BY_HASH_ALGO));
                    if offset == 0 {
                        let _ = reply.add(INodeNo(ino), 1, FileType::Directory, BY_HASH_ALGO);
                    }
                    reply.ok();
                    return;
                }
                Some(ByHashKind::Algo) | Some(ByHashKind::Digest(_)) => {
                    reply.ok(); // 受限空列表（R6：digest 目录 lookup-only）
                    return;
                }
                None => {}
            }
        }
        if !abs.is_dir() {
            reply.error(Errno::ENOTDIR);
            return;
        }
        // FUSE readdir 无需 "." / ".."（内核自行解析）——只回真实条目
        let mut entries: Vec<(u64, FileType, String)> = Vec::new();
        let rd = match fs::read_dir(&abs) {
            Ok(rd) => rd,
            Err(_) => {
                reply.error(Errno::EACCES);
                return;
            }
        };
        let mut table = self.table.lock().unwrap();
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let kind = if e.path().is_dir() {
                FileType::Directory
            } else {
                FileType::RegularFile
            };
            let rel = abs
                .join(&name)
                .strip_prefix(&self.backing)
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| PathBuf::from(&name));
            // 写回日志目录在挂载面上隐藏（落锤 Q2；T04 readdir 口径沿此）
            if Self::is_internal(&rel) {
                continue;
            }
            let child_ino = table.get_or_alloc(&rel);
            entries.push((child_ino, kind, name));
        }
        // 根目录合成 by-hash 虚拟条目（T04；真实同名条目优先已入列）
        if abs == self.backing && !entries.iter().any(|(_, _, n)| n == BY_HASH) {
            let ino = table.get_or_alloc(Path::new(BY_HASH));
            entries.push((ino, FileType::Directory, BY_HASH.to_string()));
        }
        drop(table);
        for (i, (child_ino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(INodeNo(*child_ino), (i + 1) as u64, *kind, name.as_str()) {
                break;
            }
        }
        reply.ok();
    }

    // ── 写回面（M8-WP07-T02；SPEC §2.1）：unlink/rmdir/rename 经日志
    // 路径——先日志后应用（P16），预处理拒绝先于任何破坏性效果（P15）。
    // mkdir/mknod/symlink 维持 EPERM（目录拓扑由同步管线管理） ──

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let Some(parent_rel) = self.rel_path(parent) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let rel = parent_rel.join(name);
        if Self::is_internal(&rel) {
            reply.error(Errno::EACCES);
            return;
        }
        if let Some(e) = Self::vdir_write_errno(&rel) {
            reply.error(e);
            return;
        }
        let abs = self.backing.join(&rel);
        match fs::symlink_metadata(&abs) {
            Ok(md) if md.is_dir() => {
                reply.error(Errno::EPERM); // POSIX unlink 目录 = EPERM
                return;
            }
            Ok(_) => {}
            Err(_) => {
                reply.error(Errno::ENOENT);
                return;
            }
        }
        let op = WriteBackOp::Unlink {
            path: Self::rel_str(&rel),
        };
        match self.writeback.execute(&op) {
            Ok(()) => {
                self.table.lock().unwrap().remove_entry(&rel);
                self.emit(crate::events::FuseWriteEvent::Remove {
                    path: Self::rel_str(&rel),
                });
                reply.ok();
            }
            Err(e) => reply.error(e.into()),
        }
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let Some(parent_rel) = self.rel_path(parent) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let rel = parent_rel.join(name);
        if Self::is_internal(&rel) {
            reply.error(Errno::EACCES);
            return;
        }
        if let Some(e) = Self::vdir_write_errno(&rel) {
            reply.error(e);
            return;
        }
        let abs = self.backing.join(&rel);
        // 预处理：仅空目录可删（拒绝先于日志写入——P15）
        match fs::symlink_metadata(&abs) {
            Ok(md) if !md.is_dir() => {
                reply.error(Errno::ENOTDIR);
                return;
            }
            Ok(_) => {}
            Err(_) => {
                reply.error(Errno::ENOENT);
                return;
            }
        }
        match fs::read_dir(&abs) {
            Ok(mut rd) => {
                if rd.next().is_some() {
                    reply.error(Errno::ENOTEMPTY);
                    return;
                }
            }
            Err(_) => {
                reply.error(Errno::EACCES);
                return;
            }
        }
        let op = WriteBackOp::Rmdir {
            path: Self::rel_str(&rel),
        };
        match self.writeback.execute(&op) {
            Ok(()) => {
                self.table.lock().unwrap().remove_entry(&rel);
                self.emit(crate::events::FuseWriteEvent::Remove {
                    path: Self::rel_str(&rel),
                });
                reply.ok();
            }
            Err(e) => reply.error(e.into()),
        }
    }

    fn rename(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        newparent: INodeNo,
        newname: &OsStr,
        _flags: fuser::RenameFlags,
        reply: ReplyEmpty,
    ) {
        let Some(parent_rel) = self.rel_path(parent) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let Some(newparent_rel) = self.rel_path(newparent) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let rel = parent_rel.join(name);
        let newrel = newparent_rel.join(newname);
        if Self::is_internal(&rel) || Self::is_internal(&newrel) {
            reply.error(Errno::EACCES);
            return;
        }
        if let Some(e) = Self::vdir_write_errno(&rel).or_else(|| Self::vdir_write_errno(&newrel)) {
            reply.error(e);
            return;
        }
        let abs = self.backing.join(&rel);
        let newabs = self.backing.join(&newrel);
        // 预处理：源必须存在；目标是目录时须为空目录或不同类型拒绝
        // （rename 目录覆盖目录走内核原子语义，文件覆盖目录由内核
        // EISDIR 拒绝——无半提交）。
        match fs::symlink_metadata(&abs) {
            Ok(_) => {}
            Err(_) => {
                reply.error(Errno::ENOENT);
                return;
            }
        }
        if let Ok(md) = fs::symlink_metadata(&newabs) {
            if md.is_dir() {
                let empty = fs::read_dir(&newabs).is_ok_and(|mut rd| rd.next().is_none());
                if !empty {
                    reply.error(Errno::ENOTEMPTY);
                    return;
                }
            }
        }
        let from_str = Self::rel_str(&rel);
        let to_str = Self::rel_str(&newrel);
        let op = WriteBackOp::Rename {
            from: from_str.clone(),
            to: to_str.clone(),
        };
        match self.writeback.execute(&op) {
            Ok(()) => {
                self.table.lock().unwrap().rename_entry(&rel, &newrel);
                self.emit(crate::events::FuseWriteEvent::Rename {
                    from: from_str,
                    to: to_str,
                });
                reply.ok();
            }
            Err(e) => reply.error(e.into()),
        }
    }

    fn mkdir(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        if let Some(parent_rel) = self.rel_path(parent) {
            if let Some(e) = Self::vdir_write_errno(&parent_rel.join(name)) {
                reply.error(e);
                return;
            }
        }
        reply.error(Errno::EPERM);
    }

    fn mknod(
        &self,
        _req: &Request,
        parent: INodeNo,
        name: &OsStr,
        _mode: u32,
        _umask: u32,
        _rdev: u32,
        reply: ReplyEntry,
    ) {
        if let Some(parent_rel) = self.rel_path(parent) {
            if let Some(e) = Self::vdir_write_errno(&parent_rel.join(name)) {
                reply.error(e);
                return;
            }
        }
        reply.error(Errno::EPERM);
    }

    fn symlink(
        &self,
        _req: &Request,
        parent: INodeNo,
        link_name: &OsStr,
        _target: &Path,
        reply: ReplyEntry,
    ) {
        if let Some(parent_rel) = self.rel_path(parent) {
            if let Some(e) = Self::vdir_write_errno(&parent_rel.join(link_name)) {
                reply.error(e);
                return;
            }
        }
        reply.error(Errno::EPERM);
    }
}
