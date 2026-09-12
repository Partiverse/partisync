// Package search 提供 Meilisearch HTTP API 的最小薄封装（纯 net/http，无第三方 SDK）。
// 只覆盖 MCD 所需四个动作：建索引、写文档、搜索、幂等初始化。
package search

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"slices"
	"strings"
	"time"

	"partisync/server/internal/models"
)

const assetsIndex = "assets"

// maxTotalHits 放宽 Meilisearch 默认的 1000 命中截断（T6″ C2-5）：默认值会把
// total 截到 1000、让 offset>1000 静默返回空列表，与"百万资产可检索"口径冲突。
// 取 1,100,000 覆盖 100 万压测语料并留余量；Meili 仅在内存中维护命中位图，
// 该上限只约束分页窗口，实测对常规分页查询内存与延迟无可测影响。
const maxTotalHits = 1100000

// SortableFields 是排序白名单，必须与 EnsureIndex 写入的 sortableAttributes 逐项一致。
// 只收录排序语义稳定的字段：`created_at` 虽然已由 UpsertAssetDocument 写入文档
// （展示用），但压测语料是 `2023-11-15T06:13:20+08:00` 这类带偏移的时间串、App 写入
// 的是 RFC3339 UTC 秒级以下精度的 time.Time，字典序排序结果不一致；要开放它需先统一
// 格式并全量重建索引。在此之前由 ValidateSort 明确拒绝（400），而不是转发给 Meilisearch
// 得到 400 再被上层升级为 502。
var SortableFields = []string{"name", "size_bytes"}

// ValidateSort 校验 "field:asc|desc" 形式的排序参数；空字符串合法（不排序）。
// 非法值在此表达为调用方错误（400），而不是交给 Meilisearch 拒绝后升级成 502。
func ValidateSort(s string) error {
	if s == "" {
		return nil
	}
	field, dir, found := strings.Cut(s, ":")
	if !found || field == "" || dir == "" || strings.Contains(dir, ":") {
		return errors.New(`sort must look like "field:asc" or "field:desc"`)
	}
	if !slices.Contains(SortableFields, field) {
		return fmt.Errorf("sort field %q is not sortable (allowed: %s)", field, strings.Join(SortableFields, ", "))
	}
	if dir != "asc" && dir != "desc" {
		return errors.New("sort direction must be asc or desc")
	}
	return nil
}

// Client 是 Meilisearch 的薄封装客户端。
type Client struct {
	baseURL string
	apiKey  string
	http    *http.Client
}

// NewClient 创建客户端。baseURL 取 http://host:port 形式，末尾斜杠会被裁掉。
func NewClient(baseURL, apiKey string) *Client {
	return &Client{
		baseURL: strings.TrimRight(baseURL, "/"),
		apiKey:  apiKey,
		http:    &http.Client{Timeout: 10 * time.Second},
	}
}

// EnsureIndex 幂等创建 assets 索引（主键 id）并设置可过滤属性。
// Meilisearch 对已存在索引返回 409，这里视为成功。
func (c *Client) EnsureIndex(ctx context.Context) error {
	ctx, cancel := context.WithTimeout(ctx, 15*time.Second)
	defer cancel()

	if _, err := c.do(ctx, http.MethodPost, "/indexes",
		map[string]any{"uid": assetsIndex, "primaryKey": "id"}); err != nil {
		return fmt.Errorf("ensure index: %w", err)
	}

	settings := map[string]any{
		"filterableAttributes": []string{"resource_type", "mime_type"},
		// sortableAttributes 必须与 SortableFields 白名单一致：未声明的属性一旦出现在
		// search 的 sort 参数中，Meilisearch 返回 400，被上层映射成 502 并使整个检索
		// 请求失败（见 T6′ C2-2）。
		"sortableAttributes":   SortableFields,
		"searchableAttributes": []string{"name"},
		// 放宽 maxTotalHits（见 maxTotalHits 常量注释，T6″ C2-5）。
		"pagination": map[string]any{"maxTotalHits": maxTotalHits},
	}
	if _, err := c.do(ctx, http.MethodPatch, "/indexes/"+assetsIndex+"/settings", settings); err != nil {
		return fmt.Errorf("update index settings: %w", err)
	}
	return nil
}

// UpsertAssetDocument 写入/覆盖单个资产文档（只索引检索与过滤所需字段）。
func (c *Client) UpsertAssetDocument(ctx context.Context, a *models.Asset) error {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()

	doc := map[string]any{
		"id":            a.ID,
		"name":          a.Name,
		"sha256":        a.SHA256,
		"mime_type":     a.MimeType,
		"resource_type": a.ResourceType,
		"size_bytes":    a.SizeBytes,
		// path 与 created_at 只供列表/详情展示，不参与检索。
		// 若不索引这两个字段，列表命中会缺字段并让前端渲染出 "Invalid Date"
		// 与空路径（T6′ C2-6）。
		"path":       a.Path,
		"created_at": a.CreatedAt,
	}
	if _, err := c.do(ctx, http.MethodPost, "/indexes/"+assetsIndex+"/documents", doc); err != nil {
		return fmt.Errorf("upsert asset document: %w", err)
	}
	return nil
}

// SearchOptions 封装检索的可选过滤与排序参数。
type SearchOptions struct {
	ResourceType string
	MimeType     string
	Sort         string // 如 "size_bytes:asc", "size_bytes:desc"
}

// Search 执行全文检索，支持 filter 与 sort，返回命中文档列表与估算总数。
func (c *Client) Search(ctx context.Context, q string, limit, offset int, opts ...SearchOptions) ([]map[string]any, int64, error) {
	ctx, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()

	payload := map[string]any{
		"q":      q,
		"limit":  limit,
		"offset": offset,
	}

	if len(opts) > 0 {
		opt := opts[0]
		var filters []string
		if opt.ResourceType != "" {
			filters = append(filters, fmt.Sprintf("resource_type = %q", opt.ResourceType))
		}
		if opt.MimeType != "" {
			filters = append(filters, fmt.Sprintf("mime_type = %q", opt.MimeType))
		}
		if len(filters) > 0 {
			payload["filter"] = strings.Join(filters, " AND ")
		}
		if opt.Sort != "" {
			payload["sort"] = []string{opt.Sort}
		}
	}

	raw, err := c.do(ctx, http.MethodPost, "/indexes/"+assetsIndex+"/search", payload)
	if err != nil {
		return nil, 0, fmt.Errorf("search: %w", err)
	}

	var resp struct {
		Hits               []map[string]any `json:"hits"`
		EstimatedTotalHits int64            `json:"estimatedTotalHits"`
	}
	if err := json.Unmarshal(raw, &resp); err != nil {
		return nil, 0, fmt.Errorf("decode search response: %w", err)
	}
	return resp.Hits, resp.EstimatedTotalHits, nil
}

// Health 检查 Meilisearch 可达性（readiness 探针用，A2-02）。
func (c *Client) Health(ctx context.Context) error {
	ctx, cancel := context.WithTimeout(ctx, 3*time.Second)
	defer cancel()
	raw, err := c.do(ctx, http.MethodGet, "/health", nil)
	if err != nil {
		return err
	}
	var resp struct {
		Status string `json:"status"`
	}
	if err := json.Unmarshal(raw, &resp); err != nil {
		return fmt.Errorf("decode health: %w", err)
	}
	if resp.Status != "available" {
		return fmt.Errorf("meilisearch status %q", resp.Status)
	}
	return nil
}

// do 发送 JSON 请求并返回响应体。非 2xx 返回含状态码与响应片的错误；
// 409（资源已存在）视为幂等成功。
func (c *Client) do(ctx context.Context, method, path string, body any) ([]byte, error) {
	var rd io.Reader
	if body != nil {
		buf, err := json.Marshal(body)
		if err != nil {
			return nil, fmt.Errorf("encode request: %w", err)
		}
		rd = bytes.NewReader(buf)
	}

	req, err := http.NewRequestWithContext(ctx, method, c.baseURL+path, rd)
	if err != nil {
		return nil, fmt.Errorf("build request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")
	if c.apiKey != "" {
		req.Header.Set("Authorization", "Bearer "+c.apiKey)
	}

	resp, err := c.http.Do(req)
	if err != nil {
		return nil, fmt.Errorf("%s %s: %w", method, path, err)
	}
	defer resp.Body.Close()

	raw, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	if err != nil {
		return nil, fmt.Errorf("read response: %w", err)
	}
	if resp.StatusCode >= 300 && resp.StatusCode != http.StatusConflict && resp.StatusCode != http.StatusMethodNotAllowed {
		return nil, fmt.Errorf("%s %s: status %d: %s",
			method, path, resp.StatusCode, truncate(string(raw), 256))
	}
	return raw, nil
}

// truncate 截断字符串，避免错误信息过长。
func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}
