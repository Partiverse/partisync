// Package api 提供 REST 处理器（Go 1.22 模式路由 + net/http 标准库）。
// 约定：错误统一 {"error":"..."}；请求路径内禁止 panic，所有依赖错误转状态码。
package api

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"net/http"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"

	"partisync/server/internal/models"
	"partisync/server/internal/search"
	"partisync/server/internal/storage"
	"partisync/server/internal/store"
)

const (
	defaultLimit   = 20
	maxLimit       = 100
	maxBodyBytes   = 1 << 20 // 请求体上限 1MB
	maxNameLen     = 255     // VARCHAR(255) alignment with schema
	maxPathLen     = 8192    // reasonable TEXT upper bound
	maxResourceLen = 64      // VARCHAR(64) alignment with schema
	maxPromptLen   = 10000   // arbitrary generous cap for annotation prompt
	maxSizeBytes   = 1 << 40 // 1 TiB sanity cap; prevents BIGINT overflow
)

// sha256Re 校验 64 位十六进制哈希。
var sha256Re = regexp.MustCompile(`^[0-9a-fA-F]{64}$`)

// uuidRe 校验所有合法 UUID 文本格式（v1–v8）。
// 版本号支持 1-8（含 UUIDv7/v8），variant nibble 支持 8/9/a/b。
var uuidRe = regexp.MustCompile(
	`^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[1-8][0-9a-fA-F]{3}-[89abAB][0-9a-fA-F]{3}-[0-9a-fA-F]{12}$`)

// mimeRe 校验形如 "type/subtype" 的 MIME。
var mimeRe = regexp.MustCompile(`^[A-Za-z0-9!#$&^_.+-]+/[A-Za-z0-9!#$&^_.+-]+$`)

// Server 聚合 API 依赖。store 为必需依赖；meili 可为 nil（检索降级）；
// content 为文件内容存储，缺失时上传端点返回 503（元数据端点仍可用）。
type Server struct {
	store   *store.Store
	meili   *search.Client
	content *storage.Store
}

// NewServer 构造 API 服务。content 可为 nil（未配置存储时上传端点不可用）。
func NewServer(st *store.Store, meili *search.Client, content *storage.Store) *Server {
	return &Server{store: st, meili: meili, content: content}
}

// NewServeMux 注册全部路由（Go 1.22 方法+通配模式）。
func (s *Server) NewServeMux() *http.ServeMux {
	mux := http.NewServeMux()
	mux.HandleFunc("GET /healthz", s.handleHealth)
	mux.HandleFunc("POST /api/v1/assets", s.handleCreateAsset)
	mux.HandleFunc("POST /api/v1/assets/upload", s.handleUploadAsset)
	mux.HandleFunc("GET /api/v1/assets", s.handleListAssets)
	mux.HandleFunc("GET /api/v1/assets/{id}", s.handleGetAsset)
	mux.HandleFunc("POST /api/v1/assets/{id}/annotate", s.handleAnnotate)
	mux.HandleFunc("GET /api/v1/assets/{id}/preview", s.handlePreviewAsset)
	mux.HandleFunc("GET /api/v1/assets/{id}/tags", s.handleListAssetTags)
	mux.HandleFunc("POST /api/v1/assets/{id}/tags", s.handleTagAsset)
	mux.HandleFunc("DELETE /api/v1/assets/{id}/tags/{tagID}", s.handleUntagAsset)
	mux.HandleFunc("GET /api/v1/tags", s.handleListTags)
	mux.HandleFunc("POST /api/v1/jobs/{id}/confirm", s.handleConfirmSuggestions)
	mux.HandleFunc("GET /api/v1/jobs", s.handleListJobs)
	mux.HandleFunc("GET /api/v1/jobs/{id}", s.handleGetJob)
	return mux
}

// createAssetRequest POST /api/v1/assets 请求体。
type createAssetRequest struct {
	Name         string          `json:"name"`
	Path         string          `json:"path"`
	SHA256       string          `json:"sha256"`
	SizeBytes    int64           `json:"size_bytes"`
	MimeType     string          `json:"mime_type"`
	ResourceType string          `json:"resource_type"`
	Metadata     json.RawMessage `json:"metadata"`
}

// handleHealth 存活探针。
func (s *Server) handleHealth(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// handleCreateAsset 写入资产元数据并同步 Meilisearch。
// 哈希冲突返回 200 + existing；Meili 写失败不回滚 PG，仅附加 warning。
func (s *Server) handleCreateAsset(w http.ResponseWriter, r *http.Request) {
	var req createAssetRequest
	if err := decodeJSON(r, &req); err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}

	req.Name = strings.TrimSpace(req.Name)
	req.Path = strings.TrimSpace(req.Path)
	req.SHA256 = strings.ToLower(strings.TrimSpace(req.SHA256))
	req.MimeType = strings.TrimSpace(req.MimeType)
	req.ResourceType = strings.TrimSpace(req.ResourceType)

	if req.Name == "" || req.Path == "" || req.SHA256 == "" {
		writeError(w, http.StatusBadRequest, "name, path and sha256 are required")
		return
	}
	if len(req.Name) > maxNameLen {
		writeError(w, http.StatusBadRequest, "name exceeds maximum length of 255 characters")
		return
	}
	if len(req.Path) > maxPathLen {
		writeError(w, http.StatusBadRequest, "path exceeds maximum length of 8192 characters")
		return
	}
	if !sha256Re.MatchString(req.SHA256) {
		writeError(w, http.StatusBadRequest, "sha256 must be 64 hex characters")
		return
	}
	if req.SizeBytes < 0 || req.SizeBytes > maxSizeBytes {
		writeError(w, http.StatusBadRequest, "size_bytes out of reasonable range")
		return
	}
	if !mimeRe.MatchString(req.MimeType) {
		writeError(w, http.StatusBadRequest, `mime_type must look like "type/subtype"`)
		return
	}
	if req.ResourceType == "" {
		req.ResourceType = "image"
	}
	if len(req.ResourceType) > maxResourceLen {
		writeError(w, http.StatusBadRequest, "resource_type exceeds maximum length of 64 characters")
		return
	}

	// 依赖缺失守卫位于校验之后：非法请求始终优先返回 400。
	if s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}

	asset := &models.Asset{
		Name:         req.Name,
		Path:         req.Path,
		SHA256:       req.SHA256,
		SizeBytes:    req.SizeBytes,
		MimeType:     req.MimeType,
		ResourceType: req.ResourceType,
		Metadata:     req.Metadata,
	}

	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	inserted, err := s.store.InsertAsset(ctx, asset)
	if err != nil {
		log.Printf("create asset failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to store asset")
		return
	}

	// sha256 冲突：回读已有资产并返回 200。
	if !inserted {
		existing, err := s.store.GetAssetBySHA256(ctx, req.SHA256)
		if err != nil {
			log.Printf("fetch conflicting asset failed: %v", err)
			writeError(w, http.StatusInternalServerError, "failed to load existing asset")
			return
		}
		if existing == nil {
			// 理论上不应出现（刚发生冲突却查不到）。
			writeError(w, http.StatusInternalServerError, "conflicting asset not found")
			return
		}
		writeJSON(w, http.StatusOK, map[string]any{"existing": true, "asset": existing})
		return
	}

	// 回读服务端生成的完整记录（id / created_at 等）。
	created, err := s.store.GetAssetBySHA256(ctx, req.SHA256)
	if err != nil || created == nil {
		log.Printf("readback asset failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to load created asset")
		return
	}

	resp := map[string]any{"asset": created}
	// Meilisearch 同步失败仅告警，PG 保持权威，不回滚。
	if s.meili != nil {
		if err := s.meili.UpsertAssetDocument(ctx, created); err != nil {
			log.Printf("meilisearch upsert failed (asset %s): %v", created.ID, err)
			resp["warning"] = "asset stored, but search index update failed"
		}
	}
	writeJSON(w, http.StatusCreated, resp)
}

// handleListAssets:
// 当包含 q、resource_type、mime_type 或 sort 时走 Meilisearch 检索（filter/sort 白名单校验）；
// 否则走 PG 分页；limit 默认 20、上限 100。两条路径均返回 results/total/limit/offset。
func (s *Server) handleListAssets(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	limit, offset, err := parsePaging(r)
	if err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}

	q := strings.TrimSpace(r.URL.Query().Get("q"))
	resourceType := strings.TrimSpace(r.URL.Query().Get("resource_type"))
	mimeType := strings.TrimSpace(r.URL.Query().Get("mime_type"))
	sortParam := strings.TrimSpace(r.URL.Query().Get("sort"))

	// 排序参数先做白名单校验：非法字段/方向是调用方错误（400），
	// 不能放过给 Meilisearch 再由上层统一映射成 502（见 T6′ C2-3）。
	if err := search.ValidateSort(sortParam); err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}

	// 指定搜索词、过滤条件或排序时走 Meilisearch（支持高性能过滤、全文索引与排序）
	if q != "" || resourceType != "" || mimeType != "" || sortParam != "" {
		if s.meili == nil {
			writeError(w, http.StatusBadGateway, "search backend unavailable")
			return
		}
		opts := search.SearchOptions{
			ResourceType: resourceType,
			MimeType:     mimeType,
			Sort:         sortParam,
		}
		hits, total, err := s.meili.Search(ctx, q, limit, offset, opts)
		if err != nil {
			log.Printf("meilisearch search failed: %v", err)
			writeError(w, http.StatusBadGateway, "search backend unavailable")
			return
		}
		writeJSON(w, http.StatusOK, map[string]any{
			"results": hits,
			"total":   total,
			"limit":   limit,
			"offset":  offset,
		})
		return
	}

	if s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	assets, err := s.store.ListAssets(ctx, limit, offset)
	if err != nil {
		log.Printf("list assets failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to list assets")
		return
	}
	// PG 路径同样返回 total：前端用 total 渲染总数与分页控件，缺字段会让分页恒失效
	// （见 T6′ C2-4）。
	total, err := s.store.CountAssets(ctx)
	if err != nil {
		log.Printf("count assets failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to count assets")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"results": assets,
		"total":   total,
		"limit":   limit,
		"offset":  offset,
	})
}

// handleGetAsset 按 ID 查询资产，ID 必须是合法 UUID。
func (s *Server) handleGetAsset(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if !uuidRe.MatchString(id) {
		writeError(w, http.StatusBadRequest, "invalid asset id")
		return
	}
	if s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	asset, err := s.store.GetAssetByID(ctx, id)
	if err != nil {
		log.Printf("get asset %s failed: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to load asset")
		return
	}
	if asset == nil {
		writeError(w, http.StatusNotFound, "asset not found")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"asset": asset})
}

// annotateRequest POST /api/v1/assets/{id}/annotate 请求体。
type annotateRequest struct {
	Prompt string `json:"prompt"`
}

// handleAnnotate 为已存在资产创建 pending 标注任务。
func (s *Server) handleAnnotate(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if !uuidRe.MatchString(id) {
		writeError(w, http.StatusBadRequest, "invalid asset id")
		return
	}

	var req annotateRequest
	if err := decodeJSON(r, &req); err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	if strings.TrimSpace(req.Prompt) == "" {
		writeError(w, http.StatusBadRequest, "prompt is required")
		return
	}
	if len(req.Prompt) > maxPromptLen {
		writeError(w, http.StatusBadRequest, "prompt exceeds maximum length of 10000 characters")
		return
	}
	if s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	asset, err := s.store.GetAssetByID(ctx, id)
	if err != nil {
		log.Printf("annotate: get asset %s failed: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to load asset")
		return
	}
	if asset == nil {
		writeError(w, http.StatusNotFound, "asset not found")
		return
	}

	job, err := s.store.CreateAnnotationJob(ctx, id, req.Prompt)
	if err != nil {
		log.Printf("create annotation job failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to create annotation job")
		return
	}
	writeJSON(w, http.StatusAccepted, map[string]any{"job": job})
}

// handleGetJob 按 ID 查询单个标注任务。
func (s *Server) handleGetJob(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if !uuidRe.MatchString(id) {
		writeError(w, http.StatusBadRequest, "invalid job id")
		return
	}
	if s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	job, err := s.store.GetJob(ctx, id)
	if err != nil {
		log.Printf("get job %s failed: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to load job")
		return
	}
	if job == nil {
		writeError(w, http.StatusNotFound, "job not found")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"job": job})
}

// handleListJobs 按状态过滤列出标注任务。
func (s *Server) handleListJobs(w http.ResponseWriter, r *http.Request) {
	limit, _, err := parsePaging(r)
	if err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	status := strings.TrimSpace(r.URL.Query().Get("status"))
	if s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	jobs, err := s.store.ListJobs(ctx, status, limit)
	if err != nil {
		log.Printf("list jobs failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to list jobs")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"jobs": jobs, "limit": limit})
}

// parsePaging 解析 limit/offset 查询参数：limit 默认 20、上限 100，非法即报错。
func parsePaging(r *http.Request) (limit, offset int, err error) {
	limit = defaultLimit
	if v := r.URL.Query().Get("limit"); v != "" {
		n, cerr := strconv.Atoi(v)
		if cerr != nil || n <= 0 {
			return 0, 0, errors.New("limit must be a positive integer")
		}
		if n > maxLimit {
			n = maxLimit
		}
		limit = n
	}
	if v := r.URL.Query().Get("offset"); v != "" {
		n, cerr := strconv.Atoi(v)
		if cerr != nil || n < 0 {
			return 0, 0, errors.New("offset must be a non-negative integer")
		}
		offset = n
	}
	return limit, offset, nil
}

// decodeJSON 严格解析请求体 JSON；空体或语法错误返回说明性错误。
func decodeJSON(r *http.Request, dst any) error {
	body, err := io.ReadAll(io.LimitReader(r.Body, maxBodyBytes))
	if err != nil {
		return errors.New("failed to read request body")
	}
	if len(strings.TrimSpace(string(body))) == 0 {
		return errors.New("request body must be JSON")
	}
	if err := json.Unmarshal(body, dst); err != nil {
		return errors.New("invalid JSON body")
	}
	return nil
}

// writeJSON 输出 JSON 响应。
func writeJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	w.WriteHeader(status)
	if err := json.NewEncoder(w).Encode(v); err != nil {
		log.Printf("encode response failed: %v", err)
	}
}

// writeError 统一错误响应格式。
func writeError(w http.ResponseWriter, status int, msg string) {
	writeJSON(w, status, map[string]string{"error": msg})
}

// maxUploadBytes 单次上传请求体上限（文件上限 + 少量 multipart 开销）。
const maxUploadBytes = storage.DefaultMaxBytes + (1 << 20)

// maxTagLen 与 schema 中 tags.name VARCHAR(128) 对齐。
const maxTagLen = 128

// maxUploadFieldMemory 解析 multipart 时允许驻留内存的字节数，超出部分落临时文件。
const maxUploadFieldMemory = 8 << 20

// handleUploadAsset 接收 multipart/form-data 的真实文件字节：
// 流式落盘（内容寻址 + SHA256 去重）→ 元数据入库 → 同步 Meilisearch。
// 与 POST /api/v1/assets（纯元数据登记）的区别：本端点接收文件本身。
func (s *Server) handleUploadAsset(w http.ResponseWriter, r *http.Request) {
	if s.content == nil || s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}

	r.Body = http.MaxBytesReader(w, r.Body, maxUploadBytes)
	if err := r.ParseMultipartForm(maxUploadFieldMemory); err != nil {
		var maxErr *http.MaxBytesError
		if errors.As(err, &maxErr) {
			writeError(w, http.StatusRequestEntityTooLarge, "upload exceeds maximum request size")
			return
		}
		writeError(w, http.StatusBadRequest, "invalid multipart form")
		return
	}
	defer func() {
		if r.MultipartForm != nil {
			_ = r.MultipartForm.RemoveAll()
		}
	}()

	file, hdr, err := r.FormFile("file")
	if err != nil {
		writeError(w, http.StatusBadRequest, `multipart part "file" is required`)
		return
	}
	defer func() { _ = file.Close() }()

	clientName := strings.TrimSpace(hdr.Filename)
	if clientName == "" {
		writeError(w, http.StatusBadRequest, "uploaded file must have a name")
		return
	}

	saved, err := s.content.Save(file, clientName)
	switch {
	case errors.Is(err, storage.ErrUnsupportedType):
		writeError(w, http.StatusUnsupportedMediaType,
			"unsupported file extension (allowed: png, jpg, jpeg, gif, webp, pdf, txt, md, csv, json)")
		return
	case errors.Is(err, storage.ErrTooLarge):
		writeError(w, http.StatusRequestEntityTooLarge, "file exceeds maximum size")
		return
	case errors.Is(err, storage.ErrEmptyFile):
		writeError(w, http.StatusBadRequest, "uploaded file is empty")
		return
	case err != nil:
		log.Printf("upload save failed (client name %q): %v", clientName, err)
		writeError(w, http.StatusInternalServerError, "failed to store file")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 20*time.Second)
	defer cancel()

	asset := &models.Asset{
		Name:         displayName(clientName),
		Path:         saved.RelPath,
		SHA256:       saved.SHA256,
		SizeBytes:    saved.SizeBytes,
		MimeType:     saved.MimeType,
		ResourceType: resourceTypeFor(saved.MimeType),
		Metadata:     uploadMetadata(clientName, saved),
	}

	inserted, err := s.store.InsertAsset(ctx, asset)
	if err != nil {
		log.Printf("upload insert failed (sha256 %s): %v", saved.SHA256, err)
		writeError(w, http.StatusInternalServerError, "failed to store asset")
		return
	}

	resp := map[string]any{"file_deduped": saved.Deduped}
	if !inserted {
		// 内容哈希已存在：返回既有资产（文件按内容寻址，不产生第二份副本）。
		existing, err := s.store.GetAssetBySHA256(ctx, saved.SHA256)
		if err != nil {
			log.Printf("upload readback existing failed: %v", err)
			writeError(w, http.StatusInternalServerError, "failed to load existing asset")
			return
		}
		if existing == nil {
			writeError(w, http.StatusInternalServerError, "conflicting asset not found")
			return
		}
		resp["existing"] = true
		resp["asset"] = existing
		writeJSON(w, http.StatusOK, resp)
		return
	}

	created, err := s.store.GetAssetBySHA256(ctx, saved.SHA256)
	if err != nil || created == nil {
		log.Printf("upload readback created failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to load created asset")
		return
	}
	resp["asset"] = created
	if s.meili != nil {
		if err := s.meili.UpsertAssetDocument(ctx, created); err != nil {
			log.Printf("meilisearch upsert failed (asset %s): %v", created.ID, err)
			resp["warning"] = "asset stored, but search index update failed"
		}
	}
	writeJSON(w, http.StatusCreated, resp)
}

// displayName 由客户端文件名生成展示用名称：去目录成分、去控制字符、按 rune 截断到列上限。
func displayName(clientName string) string {
	base := filepath.Base(filepath.FromSlash(clientName))
	cleaned := strings.Map(func(r rune) rune {
		if r < 0x20 || r == 0x7f {
			return -1
		}
		return r
	}, base)
	cleaned = strings.TrimSpace(cleaned)
	if cleaned == "" || cleaned == "." || cleaned == string(filepath.Separator) {
		return "unnamed"
	}
	runes := []rune(cleaned)
	if len(runes) > maxNameLen {
		runes = runes[:maxNameLen]
	}
	return string(runes)
}

// resourceTypeFor 依据嗅探到的 MIME 归类资产类型（首版仅 image/document）。
func resourceTypeFor(mimeType string) string {
	if strings.HasPrefix(mimeType, "image/") {
		return "image"
	}
	return "document"
}

// uploadMetadata 记录上传侧的最小元数据（原始名/扩展名/嗅探结果/图片尺寸）。
func uploadMetadata(clientName string, saved *storage.SavedObject) json.RawMessage {
	meta := map[string]any{
		"original_name":   displayName(clientName),
		"ext":             saved.Ext,
		"content_sniffed": saved.ContentSniffed,
	}
	if saved.Width > 0 && saved.Height > 0 {
		meta["width"] = saved.Width
		meta["height"] = saved.Height
	}
	raw, err := json.Marshal(meta)
	if err != nil {
		log.Printf("upload metadata encode failed: %v", err)
		return nil
	}
	return raw
}

// tagAssetRequest POST /api/v1/assets/{id}/tags 请求体。
type tagAssetRequest struct {
	Name       string  `json:"name"`       // 标签名（不存在则创建）
	Color      string  `json:"color"`      // 可选颜色（仅创建时生效）
	TagID      string  `json:"tag_id"`     // 或直接指定既有标签 ID
	Confidence float64 `json:"confidence"` // 默认 1.0（人工标注）
}

// requireAsset 加载资产或返回相应错误；ok=false 表示已写响应。
func (s *Server) requireAsset(w http.ResponseWriter, r *http.Request, id string) (*models.Asset, bool) {
	if !uuidRe.MatchString(id) {
		writeError(w, http.StatusBadRequest, "invalid asset id")
		return nil, false
	}
	if s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return nil, false
	}
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()
	asset, err := s.store.GetAssetByID(ctx, id)
	if err != nil {
		log.Printf("get asset %s failed: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to load asset")
		return nil, false
	}
	if asset == nil {
		writeError(w, http.StatusNotFound, "asset not found")
		return nil, false
	}
	return asset, true
}

// handlePreviewAsset 输出资产文件内容（真实文件读取，MCD「预览」闭环）。
// 路径来自 assets.path（服务端生成的内容寻址相对路径），经 storage.Store.Open
// 校验不越界；ext 可选强制下载文件名。
func (s *Server) handlePreviewAsset(w http.ResponseWriter, r *http.Request) {
	asset, ok := s.requireAsset(w, r, r.PathValue("id"))
	if !ok {
		return
	}
	if s.content == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}

	f, err := s.content.Open(asset.Path)
	if err != nil {
		if errors.Is(err, storage.ErrOutsideRoot) {
			log.Printf("preview rejected (asset %s, path %q): %v", asset.ID, asset.Path, err)
			writeError(w, http.StatusBadRequest, "invalid asset path")
			return
		}
		log.Printf("preview open failed (asset %s): %v", asset.ID, err)
		writeError(w, http.StatusNotFound, "asset content not found")
		return
	}
	defer func() { _ = f.Close() }()

	w.Header().Set("Content-Type", asset.MimeType)
	w.Header().Set("X-Content-Type-Options", "nosniff")
	if r.URL.Query().Get("download") == "1" {
		w.Header().Set("Content-Disposition", fmt.Sprintf("attachment; filename=%q", asset.Name))
	}
	w.Header().Set("Cache-Control", "private, max-age=3600")
	if _, err := io.Copy(w, f); err != nil {
		log.Printf("preview copy failed (asset %s): %v", asset.ID, err)
	}
}

// handleListAssetTags 列出资产上的全部标签（含来源 human/ai 与置信度）。
func (s *Server) handleListAssetTags(w http.ResponseWriter, r *http.Request) {
	asset, ok := s.requireAsset(w, r, r.PathValue("id"))
	if !ok {
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()
	tags, err := s.store.ListAssetTags(ctx, asset.ID)
	if err != nil {
		log.Printf("list asset tags failed (asset %s): %v", asset.ID, err)
		writeError(w, http.StatusInternalServerError, "failed to list asset tags")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"tags": tags})
}

// handleTagAsset 将标签关联到资产（标签不存在时按名称创建）。
func (s *Server) handleTagAsset(w http.ResponseWriter, r *http.Request) {
	asset, ok := s.requireAsset(w, r, r.PathValue("id"))
	if !ok {
		return
	}
	var req tagAssetRequest
	if err := decodeJSON(r, &req); err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	req.Name = strings.TrimSpace(req.Name)
	if req.Name == "" && req.TagID == "" {
		writeError(w, http.StatusBadRequest, "name or tag_id is required")
		return
	}
	if len(req.Name) > maxTagLen {
		writeError(w, http.StatusBadRequest, "tag name exceeds maximum length of 128 characters")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	tagID := req.TagID
	if tagID == "" {
		tag, err := s.store.UpsertTag(ctx, req.Name, strings.TrimSpace(req.Color))
		if err != nil {
			log.Printf("tag asset: upsert tag %q failed: %v", req.Name, err)
			writeError(w, http.StatusInternalServerError, "failed to create tag")
			return
		}
		tagID = tag.ID
	} else if !uuidRe.MatchString(tagID) {
		writeError(w, http.StatusBadRequest, "invalid tag_id")
		return
	}
	if req.Confidence <= 0 || req.Confidence > 1 {
		req.Confidence = 1.0
	}
	if err := s.store.TagAsset(ctx, asset.ID, tagID, "human", req.Confidence); err != nil {
		log.Printf("tag asset %s failed: %v", asset.ID, err)
		writeError(w, http.StatusInternalServerError, "failed to attach tag")
		return
	}
	tags, err := s.store.ListAssetTags(ctx, asset.ID)
	if err != nil {
		log.Printf("list asset tags after attach failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to list asset tags")
		return
	}
	writeJSON(w, http.StatusCreated, map[string]any{"tags": tags})
}

// handleUntagAsset 移除资产上的一个标签。
func (s *Server) handleUntagAsset(w http.ResponseWriter, r *http.Request) {
	asset, ok := s.requireAsset(w, r, r.PathValue("id"))
	if !ok {
		return
	}
	tagID := r.PathValue("tagID")
	if !uuidRe.MatchString(tagID) {
		writeError(w, http.StatusBadRequest, "invalid tag id")
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()
	if err := s.store.UntagAsset(ctx, asset.ID, tagID); err != nil {
		log.Printf("untag asset %s failed: %v", asset.ID, err)
		writeError(w, http.StatusInternalServerError, "failed to remove tag")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"removed": true})
}

// handleListTags 列出全部标签。
func (s *Server) handleListTags(w http.ResponseWriter, r *http.Request) {
	if s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}
	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()
	tags, err := s.store.ListTags(ctx)
	if err != nil {
		log.Printf("list tags failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to list tags")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"tags": tags})
}

// confirmRequest POST /api/v1/jobs/{id}/confirm 请求体。
type confirmRequest struct {
	Suggestions []models.TagSuggestion `json:"suggestions"` // 人工勾选确认的 AI 建议（可编辑）
}

// handleConfirmSuggestions 人工确认 AI 建议并写入资产标注（asset_tags, source='ai'）。
// MCD「人工确认 AI 建议并写入标注」闭环的最后一步：body 中的 suggestions 是
// 人工审核后的清单（可修改/删除 AI 建议），服务端不再读取 job.result 原文。
func (s *Server) handleConfirmSuggestions(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if !uuidRe.MatchString(id) {
		writeError(w, http.StatusBadRequest, "invalid job id")
		return
	}
	var req confirmRequest
	if err := decodeJSON(r, &req); err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}
	if len(req.Suggestions) == 0 {
		writeError(w, http.StatusBadRequest, "suggestions must not be empty")
		return
	}
	if len(req.Suggestions) > 64 {
		writeError(w, http.StatusBadRequest, "too many suggestions (max 64)")
		return
	}
	if s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}

	ctx, cancel := context.WithTimeout(r.Context(), 10*time.Second)
	defer cancel()

	job, err := s.store.GetJob(ctx, id)
	if err != nil {
		log.Printf("confirm: get job %s failed: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to load job")
		return
	}
	if job == nil {
		writeError(w, http.StatusNotFound, "job not found")
		return
	}
	if job.Status != models.StatusCompleted {
		writeError(w, http.StatusConflict, "job is not completed (status: "+job.Status+")")
		return
	}

	confirmed, err := s.store.ConfirmSuggestions(ctx, job.AssetID, req.Suggestions)
	if err != nil {
		log.Printf("confirm suggestions for asset %s failed: %v", job.AssetID, err)
		writeError(w, http.StatusInternalServerError, "failed to confirm suggestions")
		return
	}
	tags, err := s.store.ListAssetTags(ctx, job.AssetID)
	if err != nil {
		log.Printf("list asset tags after confirm failed: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to list asset tags")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{
		"confirmed": confirmed,
		"asset_id":  job.AssetID,
		"tags":      tags,
	})
}
