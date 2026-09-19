-- PartiGraph schema v1（SPEC M0-WP02；钉子清单：只增不改，改列需新迁移版本）
-- v1 裁剪：无 space/provider/chunk/tag/sidecar/oplog（各自归属后续工作包）。

CREATE TABLE IF NOT EXISTS device (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    slug         TEXT NOT NULL UNIQUE,
    pubkey       BLOB,
    capabilities TEXT,
    last_seen    INTEGER
);

CREATE TABLE IF NOT EXISTS volume (
    id          TEXT PRIMARY KEY,
    device_id   TEXT NOT NULL REFERENCES device(id),
    fingerprint TEXT NOT NULL,
    is_removable INTEGER NOT NULL DEFAULT 0,
    is_cloud    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS content (
    id   TEXT PRIMARY KEY,            -- blake3 hex（内容身份，调研方案 §5.3 ContentIdentity）
    size INTEGER NOT NULL,
    mime TEXT,
    kind TEXT
);

CREATE TABLE IF NOT EXISTS entry (
    id         TEXT PRIMARY KEY,      -- ULID
    space_id   TEXT NOT NULL DEFAULT 'default',  -- v1：单默认空间
    parent_id  TEXT REFERENCES entry(id),
    kind       INTEGER NOT NULL,      -- 0=file, 1=dir
    name       TEXT NOT NULL,
    path       TEXT NOT NULL,         -- v1 反规范化全路径（SPEC 风险节：WP07 评估 CTE 化）
    content_id TEXT REFERENCES content(id),
    size       INTEGER NOT NULL DEFAULT 0,
    mtime_ns   INTEGER NOT NULL DEFAULT 0,
    state      INTEGER NOT NULL DEFAULT 0, -- 0=materialized
    chunk_root TEXT,                      -- v2：大文件块清单根（SPEC M0-WP03）
    owner_device TEXT,                    -- v6：域归属（设备自有域属主）
    synced_seq INTEGER                    -- v6：已同步水位（对端 ACK 后推进）
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_entry_path ON entry(path);
CREATE INDEX IF NOT EXISTS idx_entry_parent ON entry(parent_id);
CREATE INDEX IF NOT EXISTS idx_entry_content ON entry(content_id);
CREATE INDEX IF NOT EXISTS idx_entry_name ON entry(name);

-- 闭包表（Spacedrive EntryClosure 模式）：O(1) 子树/祖先查询
CREATE TABLE IF NOT EXISTS entry_closure (
    ancestor   TEXT NOT NULL,
    descendant TEXT NOT NULL,
    depth      INTEGER NOT NULL,
    PRIMARY KEY (ancestor, descendant)
);
CREATE INDEX IF NOT EXISTS idx_closure_desc ON entry_closure(descendant);

-- v2 迁移（旧库升级；新库此列为已建，ALTER 报 duplicate 可忽略——由迁移代码吞掉）

-- v3（M0-WP04）：scan_journal 事件队列——应用成功后删行（队列语义）
CREATE TABLE IF NOT EXISTS scan_journal (
    seq   INTEGER PRIMARY KEY AUTOINCREMENT,
    path  TEXT NOT NULL,
    kind  INTEGER NOT NULL,      -- 0=created, 1=modified, 2=removed
    at_ns INTEGER NOT NULL
);

-- v4（M0-WP05）：持久作业——checkpoint 每 200 文件提交；陈旧 running 视同 interrupted
CREATE TABLE IF NOT EXISTS jobs (
    id         TEXT PRIMARY KEY,
    kind       TEXT NOT NULL,
    status     INTEGER NOT NULL, -- 0=queued,1=running,2=interrupted,3=completed,4=failed
    root       TEXT NOT NULL,
    checkpoint TEXT,
    done_files INTEGER NOT NULL DEFAULT 0,
    error      TEXT,
    created_ns INTEGER NOT NULL,
    updated_ns INTEGER NOT NULL
);

-- v5（M1-WP08）：目录列表排序复合索引——顶层 16 万子项目录
-- ORDER BY kind,name 全排序 500ms+（实测），复合索引消除
CREATE INDEX IF NOT EXISTS idx_entry_parent_order ON entry(parent_id, kind DESC, name);

-- v6（M2-WP01）：同步核——域归属与 oplog
-- 设备自有域：owner_device = 属主设备（单写者，属主状态权威）
-- 共享域：预留（Tag/用户元数据，WP02 启用）
CREATE TABLE IF NOT EXISTS sync_oplog (
    hlc           TEXT PRIMARY KEY,  -- HLC key（字符串序 == 全序）
    space_id      TEXT NOT NULL DEFAULT 'default',
    domain        INTEGER NOT NULL,  -- 0=设备自有 1=共享
    entity        TEXT NOT NULL,     -- 'entry' / …
    entity_id     TEXT NOT NULL,
    op            TEXT NOT NULL,     -- 'upsert' / 'remove'
    origin_device TEXT NOT NULL,     -- 源头设备（回环防护）
    payload       TEXT NOT NULL,     -- JSON（设备自有域=全量行；remove={path}）
    at_ns         INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_oplog_hlc ON sync_oplog(hlc);

-- v7（M2-WP02）：共享域实体（Tag）+ 冲突血缘（P11「保留两者 + 血缘可查」）
-- tag.id 随创建 oplog 传播 = 全局共享身份；删除 = 墓碑（LWW 防复活）。
CREATE TABLE IF NOT EXISTS tag (
    id          TEXT PRIMARY KEY,   -- ULID
    space_id    TEXT NOT NULL DEFAULT 'default',
    name        TEXT NOT NULL,
    color       TEXT,
    deleted     INTEGER NOT NULL DEFAULT 0,
    updated_hlc TEXT                -- 本行最后生效写入的 oplog HLC key（LWW 水位）
);
-- 链接不用 FK、以 entry path 为身份：entry ULID 每节点各自生成（设备自有域），
-- 不可做跨节点身份；path 唯一且随 entry oplog 收敛。悬空行（entry/tag 未到位）
-- 合法，查询 JOIN 自然过滤；unlink 亦为墓碑（防晚到 link 复活）。
CREATE TABLE IF NOT EXISTS entry_tag (
    tag_id      TEXT NOT NULL,
    entry_path  TEXT NOT NULL,
    deleted     INTEGER NOT NULL DEFAULT 0,
    updated_hlc TEXT,
    PRIMARY KEY (tag_id, entry_path)
);
CREATE INDEX IF NOT EXISTS idx_entry_tag_path ON entry_tag(entry_path);
-- 冲突血缘：base/local/incoming/origin + 触发 oplog 行（中继保键 ⇒ 全局同一键，可去重）
CREATE TABLE IF NOT EXISTS sync_conflict (
    id            TEXT PRIMARY KEY,  -- ULID
    space_id      TEXT NOT NULL DEFAULT 'default',
    base_path     TEXT NOT NULL,
    local_path    TEXT NOT NULL,
    incoming_path TEXT NOT NULL,
    origin_device TEXT NOT NULL,
    detected_hlc  TEXT NOT NULL,
    at_ns         INTEGER NOT NULL
);
-- v10（M2-WP08）：版本回收——staggered 版本化与 trash-can
CREATE TABLE IF NOT EXISTS entry_version (
    id            TEXT PRIMARY KEY,  -- 版本 ULID
    path          TEXT NOT NULL,
    content_id    TEXT,
    size          INTEGER NOT NULL,
    mtime_ns      INTEGER NOT NULL,
    owner_device  TEXT,
    state         INTEGER NOT NULL DEFAULT 0,  -- 0=staggered-versioned, 1=trashed
    retired_at_ns INTEGER NOT NULL,
    expires_at_ns INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_version_path ON entry_version(path);
CREATE INDEX IF NOT EXISTS idx_version_expires ON entry_version(expires_at_ns);

-- v9（M2-WP06）：占位符骨架同步与按需 hydrate
-- entry.state 0=materialized, 1=placeholder；content_hydrated_at_ns 是内容到位时间戳；
-- pin_count 是本端 pin 引用数（WP08 回收使用）。ALTER 列由 migrate() 防御性补列执行，
-- 此处只声明新库应具备的最终形态以供新库识别
CREATE INDEX IF NOT EXISTS idx_entry_state ON entry(state);

-- v8（M2-WP03）：对账基建——持久时钟与来源水位
-- 本店 HLC 时钟顶（oplog 键同构）：修复时钟随进程重启回退的潜在缺陷，
-- 也是「本机是否又产生了新写入」的对账判据
CREATE TABLE IF NOT EXISTS sync_clock (
    id  INTEGER PRIMARY KEY CHECK (id = 1),
    top TEXT NOT NULL
);
-- 水位：来自 device 的 oplog 行，本店已应用（或 LWW 裁决过）的最大键
CREATE TABLE IF NOT EXISTS sync_watermark (
    device   TEXT NOT NULL,
    space_id TEXT NOT NULL DEFAULT 'default',
    last_hlc TEXT NOT NULL,
    PRIMARY KEY (device, space_id)
);
