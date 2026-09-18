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
    state      INTEGER NOT NULL DEFAULT 0  -- 0=materialized
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
