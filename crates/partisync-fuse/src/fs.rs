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

const TTL: Duration = Duration::from_secs(1);

/// 打开的写句柄：新文件顺序写游标（offset 必须等于 cur_len）。
struct WriteHandle {
    path: PathBuf,
    cur_len: u64,
}

struct InoTable {
    /// ino → 后备相对路径（root = ""）
    paths: HashMap<u64, PathBuf>,
    /// 路径 → ino（路径规范化后复用）
    by_path: HashMap<PathBuf, u64>,
    next: u64,
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
}

pub struct PartiFuse {
    backing: PathBuf,
    table: Mutex<InoTable>,
    writes: Mutex<HashMap<u64, WriteHandle>>,
}

impl PartiFuse {
    pub fn new(backing: PathBuf) -> Self {
        Self {
            backing,
            table: Mutex::new(InoTable::new()),
            writes: Mutex::new(HashMap::new()),
        }
    }

    fn backing_path(&self, ino: INodeNo) -> Option<PathBuf> {
        self.table
            .lock()
            .unwrap()
            .paths
            .get(&ino.0)
            .map(|rel| self.backing.join(rel))
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
        let Some(parent_rel) = self.backing_path(parent) else {
            reply.error(Errno::ENOENT);
            return;
        };
        let rel = parent_rel.join(name);
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
        _ino: INodeNo,
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
        // P15：truncate/perm/时间戳全部显式拒绝——不允许任何破坏性 setattr
        let _ = size;
        reply.error(Errno::EPERM);
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        let Some(abs) = self.backing_path(ino) else {
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
        // 已存在文件：只读。写打开显式拒绝（mountpoint-s3 语义——
        // 对象不可覆盖/追加改写；改写走新版本新文件）。
        if flags.acc_mode() != OpenAccMode::O_RDONLY {
            reply.error(Errno::EACCES);
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
            },
        );
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
        // 新文件创建后不可回写/跳写——写一次成型（SEMANTICS.md）。
        if offset != h.cur_len {
            reply.error(Errno::EINVAL);
            return;
        }
        let mut file = match fs::OpenOptions::new().append(true).open(&h.path) {
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

    fn flush(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _fh: FileHandle,
        _lock_owner: fuser::LockOwner,
        reply: ReplyEmpty,
    ) {
        // 写路径为直写（append 直落后备文件），无缓存 flush 语义
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
        self.writes.lock().unwrap().remove(&fh.0);
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
            let child_ino = table.get_or_alloc(&rel);
            entries.push((child_ino, kind, name));
        }
        drop(table);
        for (i, (child_ino, kind, name)) in entries.iter().enumerate().skip(offset as usize) {
            if reply.add(INodeNo(*child_ino), (i + 1) as u64, *kind, name.as_str()) {
                break;
            }
        }
        reply.ok();
    }

    // ── 显式拒绝面（P15）：EPERM 优先于默认 ENOSYS——语义是「本面不支持
    // 且不允许」，不是「未实现」 ──

    fn unlink(&self, _req: &Request, _parent: INodeNo, _name: &OsStr, reply: ReplyEmpty) {
        reply.error(Errno::EPERM);
    }

    fn rmdir(&self, _req: &Request, _parent: INodeNo, _name: &OsStr, reply: ReplyEmpty) {
        reply.error(Errno::EPERM);
    }

    fn rename(
        &self,
        _req: &Request,
        _parent: INodeNo,
        _name: &OsStr,
        _newparent: INodeNo,
        _newname: &OsStr,
        _flags: fuser::RenameFlags,
        reply: ReplyEmpty,
    ) {
        reply.error(Errno::EPERM);
    }

    fn mkdir(
        &self,
        _req: &Request,
        _parent: INodeNo,
        _name: &OsStr,
        _mode: u32,
        _umask: u32,
        reply: ReplyEntry,
    ) {
        reply.error(Errno::EPERM);
    }

    fn mknod(
        &self,
        _req: &Request,
        _parent: INodeNo,
        _name: &OsStr,
        _mode: u32,
        _umask: u32,
        _rdev: u32,
        reply: ReplyEntry,
    ) {
        reply.error(Errno::EPERM);
    }

    fn symlink(
        &self,
        _req: &Request,
        _parent: INodeNo,
        _link_name: &OsStr,
        _target: &Path,
        reply: ReplyEntry,
    ) {
        reply.error(Errno::EPERM);
    }
}
