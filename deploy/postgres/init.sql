-- partisync PostgreSQL 初始化 Schema
-- 满足 MCD 资产存储、标签关联与慢标注队列需求

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- 1. 资产表 (Assets)
CREATE TABLE IF NOT EXISTS assets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    path TEXT NOT NULL,
    sha256 CHAR(64) NOT NULL UNIQUE,
    size_bytes BIGINT NOT NULL,
    mime_type VARCHAR(128) NOT NULL,
    resource_type VARCHAR(64) NOT NULL DEFAULT 'image', -- 预留 Resource.type 扩展点
    metadata JSONB DEFAULT '{}'::jsonb,                 -- 宽度/高度/格式/EXIF 等
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_assets_sha256 ON assets (sha256);
CREATE INDEX IF NOT EXISTS idx_assets_mime_type ON assets (mime_type);
CREATE INDEX IF NOT EXISTS idx_assets_resource_type ON assets (resource_type);
CREATE INDEX IF NOT EXISTS idx_assets_created_at ON assets (created_at DESC);

-- 2. 标签表 (Tags)
CREATE TABLE IF NOT EXISTS tags (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(128) NOT NULL UNIQUE,
    color VARCHAR(32) DEFAULT '#3b82f6',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 3. 资产与标签关联表 (Asset Tags)
CREATE TABLE IF NOT EXISTS asset_tags (
    asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    tag_id UUID NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    source VARCHAR(32) NOT NULL DEFAULT 'human', -- 'human' | 'ai'
    confidence REAL DEFAULT 1.0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (asset_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_asset_tags_asset_id ON asset_tags (asset_id);
CREATE INDEX IF NOT EXISTS idx_asset_tags_tag_id ON asset_tags (tag_id);

-- 4. 慢标注任务队列表 (Annotation Jobs)
-- 专门适配基于 DB 的并发安全任务出队 (FOR UPDATE SKIP LOCKED)
CREATE TABLE IF NOT EXISTS annotation_jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    asset_id UUID NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    status VARCHAR(32) NOT NULL DEFAULT 'pending', -- 'pending', 'processing', 'completed', 'failed'
    prompt TEXT,
    result JSONB DEFAULT '{}'::jsonb,
    error_message TEXT,
    retry_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_annotation_jobs_status_created ON annotation_jobs (status, created_at ASC);
CREATE INDEX IF NOT EXISTS idx_annotation_jobs_asset_id ON annotation_jobs (asset_id);
