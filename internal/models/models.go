// Package models 定义 partisync 领域数据结构（资产/标签/标注任务）。
// 字段与 deploy/postgres/init.sql 中的 schema 一一对应；UUID 以 string 表示。
package models

import (
	"encoding/json"
	"time"
)

// Asset 数据资产（assets 表）。
// SHA256 为文件内容哈希，用于去重；ResourceType 预留扩展点（image/document/video/...）。
type Asset struct {
	ID           string          `json:"id"`
	Name         string          `json:"name"`
	Path         string          `json:"path"`
	SHA256       string          `json:"sha256"`
	SizeBytes    int64           `json:"size_bytes"`
	MimeType     string          `json:"mime_type"`
	ResourceType string          `json:"resource_type"`
	Metadata     json.RawMessage `json:"metadata,omitempty"`
	CreatedAt    time.Time       `json:"created_at"`
	UpdatedAt    time.Time       `json:"updated_at"`
}

// Tag 标签（tags 表）。
type Tag struct {
	ID        string    `json:"id"`
	Name      string    `json:"name"`
	Color     string    `json:"color"`
	CreatedAt time.Time `json:"created_at"`
}

// AnnotationJob 慢标注任务（annotation_jobs 表）。
// 状态机：pending -> processing -> completed|failed；RetryCount 记录重试次数。
type AnnotationJob struct {
	ID           string          `json:"id"`
	AssetID      string          `json:"asset_id"`
	Status       string          `json:"status"`
	Prompt       string          `json:"prompt"`
	Result       json.RawMessage `json:"result,omitempty"`
	ErrorMessage string          `json:"error_message,omitempty"`
	RetryCount   int             `json:"retry_count"`
	CreatedAt    time.Time       `json:"created_at"`
	UpdatedAt    time.Time       `json:"updated_at"`
}

// 状态常量，与 schema 中 annotation_jobs.status 的取值一致。
const (
	StatusPending    = "pending"
	StatusProcessing = "processing"
	StatusCompleted  = "completed"
	StatusFailed     = "failed"
)

// TagSuggestion 一条待人工确认的 AI 建议标签。
// 来自标注任务 result JSON 中的 tags 数组项。
type TagSuggestion struct {
	Name       string  `json:"name"`
	Confidence float64 `json:"confidence"`
}
