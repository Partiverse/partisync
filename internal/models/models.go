// Package models 定义 partisync 领域数据结构（资产/标签/标注任务）。
// 字段与 deploy/postgres/init.sql 中的 schema 一一对应；UUID 以 string 表示。
package models

import (
	"encoding/json"
	"time"
)

// Asset 数据资产（assets 表）。
// SHA256 为文件内容哈希，用于去重；ResourceType 预留扩展点（image/document/video/...）。
// SourceID 标识资产来源：nil 表示本地上传，UUID 表示来自对应数据源。
type Asset struct {
	ID           string          `json:"id"`
	Name         string          `json:"name"`
	Path         string          `json:"path"`
	SHA256       string          `json:"sha256"`
	SizeBytes    int64           `json:"size_bytes"`
	MimeType     string          `json:"mime_type"`
	ResourceType string          `json:"resource_type"`
	Metadata     json.RawMessage `json:"metadata,omitempty"`
	SourceID     *string         `json:"source_id,omitempty"` // nullable
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

// DataSource 数据源（data_sources 表）。
// Config 仅内部使用，不序列化到 API 响应。
type DataSource struct {
	ID             string          `json:"id"`
	Name           string          `json:"name"`
	Type           string          `json:"type"` // "webdav"
	Config         json.RawMessage `json:"-"`    // 内部字段，永不序列化
	URL            string          `json:"url,omitempty"`    // 从 config 解密后回填，供 UI 显示
	Username       string          `json:"username,omitempty"`
	RemotePath     string          `json:"remote_path,omitempty"`
	LastScanAt     *time.Time      `json:"last_scan_at,omitempty"`
	LastScanResult *ScanResult     `json:"last_scan_result,omitempty"`
	AssetCount     int             `json:"asset_count,omitempty"` // 关联资产数量
	CreatedAt      time.Time       `json:"created_at"`
}

// FillDisplayFields 从 config JSON 中解密出显示字段（URL/Username/RemotePath）。
// 必须在返回给 API 之前调用。
func (ds *DataSource) FillDisplayFields() error {
	if len(ds.Config) == 0 {
		return nil
	}
	var cfg map[string]string
	if err := json.Unmarshal(ds.Config, &cfg); err != nil {
		return err
	}
	ds.URL = cfg["url"]
	ds.Username = cfg["username"]
	ds.RemotePath = cfg["remote_path"]
	return nil
}

// ScanResult 扫描结果统计。
type ScanResult struct {
	Imported int      `json:"imported"`
	Skipped  int      `json:"skipped"`
	Errors   []string `json:"errors,omitempty"`
}

// CreateDataSource 创建数据源的请求体。
type CreateDataSource struct {
	Name       string `json:"name"`
	Type      string `json:"type"` // "webdav"
	URL       string `json:"url"`
	Username  string `json:"username"`
	Password  string `json:"password"`
	RemotePath string `json:"remote_path"`
}

// UpdateDataSource 更新数据源的请求体（所有字段可选）。
type UpdateDataSource struct {
	Name       *string `json:"name,omitempty"`
	URL        *string `json:"url,omitempty"`
	Username   *string `json:"username,omitempty"`
	Password   *string `json:"password,omitempty"` // 空字符串表示不修改密码
	RemotePath *string `json:"remote_path,omitempty"`
}

// ScanRequest 扫描请求。
type ScanRequest struct {
	Recursive    *bool    `json:"recursive"`    // default true
	Types        []string `json:"types"`        // 资源类型过滤 ["image","document"]
	MetadataOnly bool     `json:"metadata_only"` // true=仅元数据入库，不下载文件内容（默认 true）
}

// ScanJob 异步扫描任务。
type ScanJob struct {
	ID            string     `json:"id"`
	SourceID      string     `json:"source_id"`
	Status        string     `json:"status"` // "queued" | "running" | "completed" | "failed"
	TotalFiles    int        `json:"total_files"`
	ProcessedFiles int        `json:"processed_files"`
	ImportedCount int        `json:"imported_count"`
	SkippedCount  int        `json:"skipped_count"`
	ErrorMessage  string     `json:"error,omitempty"`
	StartedAt     *time.Time `json:"started_at,omitempty"`
	FinishedAt    *time.Time `json:"finished_at,omitempty"`
	CreatedAt     time.Time  `json:"created_at"`
}
