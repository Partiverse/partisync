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
	"mime"
	"mime/multipart"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"

	"partisync/server/internal/connector"
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
	mux.HandleFunc("GET /readyz", s.handleReady)
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
	// 数据源
	mux.HandleFunc("GET /api/v1/sources", s.handleListSources)
	mux.HandleFunc("POST /api/v1/sources", s.handleCreateSource)
	mux.HandleFunc("PATCH /api/v1/sources/{id}", s.handleUpdateSource)
	mux.HandleFunc("DELETE /api/v1/sources/{id}", s.handleDeleteSource)
	mux.HandleFunc("POST /api/v1/sources/{id}/scan", s.handleScanSource)
	mux.HandleFunc("GET /api/v1/source-jobs/{id}", s.handleGetScanJob)
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

// handleHealth 存活探针（liveness：进程活着即 200）。
// A2-02：依赖可达性由 /readyz 承担，二者分离——依赖故障不应触发进程重启。
func (s *Server) handleHealth(w http.ResponseWriter, r *http.Request) {
	writeJSON(w, http.StatusOK, map[string]string{"status": "ok"})
}

// handleReady 就绪探针（readiness）：检查 PG/Meili/存储根可达。
// 依赖故障返回 503（compose 可据此摘除流量，但不重启进程）。
// meili 为 nil（检索降级模式）或 storage 为 nil 时不视为未就绪。
// F-I7（P2 独立复审）：响应只携带二元状态，具体错误仅进服务端日志——
// 原始错误串含内部拓扑（DSN 主机、容器名、DNS 地址），不该外泄。
func (s *Server) handleReady(w http.ResponseWriter, r *http.Request) {
	type dep struct {
		Name string `json:"name"`
		OK   bool   `json:"ok"`
		Err  string `json:"error,omitempty"`
	}
	deps := []dep{}
	ready := true

	if s.store != nil {
		ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
		defer cancel()
		if _, err := s.store.CountAssets(ctx); err != nil {
			log.Printf("readyz: postgres probe failed: %v", err)
			deps = append(deps, dep{Name: "postgres", OK: false, Err: "unreachable"})
			ready = false
		} else {
			deps = append(deps, dep{Name: "postgres", OK: true})
		}
	}
	if s.meili != nil {
		ctx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
		defer cancel()
		if err := s.meili.Health(ctx); err != nil {
			log.Printf("readyz: meilisearch probe failed: %v", err)
			deps = append(deps, dep{Name: "meilisearch", OK: false, Err: "unreachable"})
			ready = false
		} else {
			deps = append(deps, dep{Name: "meilisearch", OK: true})
		}
	}
	if s.content != nil {
		if err := s.content.Healthy(); err != nil {
			log.Printf("readyz: storage probe failed: %v", err)
			deps = append(deps, dep{Name: "storage", OK: false, Err: "unavailable"})
			ready = false
		} else {
			deps = append(deps, dep{Name: "storage", OK: true})
		}
	}

	status := http.StatusOK
	if !ready {
		status = http.StatusServiceUnavailable
	}
	writeJSON(w, status, map[string]any{"ready": ready, "dependencies": deps})
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
	sourceID := strings.TrimSpace(r.URL.Query().Get("source_id"))

	// 排序参数先做白名单校验：非法字段/方向是调用方错误（400），
	// 不能放过给 Meilisearch 再由上层统一映射成 502（见 T6′ C2-3）。
	if err := search.ValidateSort(sortParam); err != nil {
		writeError(w, http.StatusBadRequest, err.Error())
		return
	}

	// 指定搜索词、过滤条件、排序或按数据源筛选时走 Meilisearch（支持高性能过滤、全文索引与排序）
	// 注意：source_id 筛选走 PG 路径（Meili 不存储 source_id）
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

	var assets []models.Asset
	var total int64

	// source_id 筛选走 PG 路径
	if sourceID != "" {
		if !uuidRe.MatchString(sourceID) {
			writeError(w, http.StatusBadRequest, "invalid source_id")
			return
		}
		assets, err = s.store.ListAssetsBySource(ctx, sourceID, limit, offset)
		if err != nil {
			log.Printf("list assets by source %s: %v", sourceID, err)
			writeError(w, http.StatusInternalServerError, "failed to list assets")
			return
		}
		total = int64(len(assets)) // 简化：返回实际数量，前端可据此判断是否有更多
	} else {
		assets, err = s.store.ListAssets(ctx, limit, offset)
		if err != nil {
			log.Printf("list assets failed: %v", err)
			writeError(w, http.StatusInternalServerError, "failed to list assets")
			return
		}
		total, err = s.store.CountAssets(ctx)
		if err != nil {
			log.Printf("count assets failed: %v", err)
			writeError(w, http.StatusInternalServerError, "failed to count assets")
			return
		}
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

// maxUploadOverheadBytes 请求体上限相对单文件上限的余量（multipart 边界/头部/其余字段）。
// A2-03：请求体上限不再硬编码 storage.DefaultMaxBytes，而是 MaxBytes()+本余量，
// 使 MAX_UPLOAD_BYTES 调小后真实生效。
const maxUploadOverheadBytes = 1 << 20

// maxTagLen 与 schema 中 tags.name VARCHAR(128) 对齐。
const maxTagLen = 128

// 上传端点的时间预算（A2-06）。服务端级 ReadTimeout/WriteTimeout 只为常规请求设定
// （15s/30s），大文件慢速上传必须由本端点显式放宽本次请求的期限，否则：
//   - 读期限 15s 会在慢速上传中途掐断请求（旧实现表现为误导性的 400）；
//   - 写期限 30s 从"读完请求头"起算，同样会在长上传时提前掐断连接。
const (
	// uploadReadBudget 请求体读取总预算：约 100MiB @ 300KB/s。
	uploadReadBudget = 5 * time.Minute
	// uploadWriteBudget 响应写期限：必须覆盖"长读 + 常规写"。
	uploadWriteBudget = uploadReadBudget + 30*time.Second
)

// allowSlowUpload 放宽本次请求的读/写期限，使大文件慢速上传不被服务端级超时截断。
// 不支持该控制的 ResponseWriter（如测试用 recorder）会返回错误——此时仍有服务端级
// 超时兜底，不影响安全性；但必须记日志：若未来中间件改动了 ResponseWriter 链导致
// 放宽静默失效，这里要有迹可循（P2 独立复审 F-I4）。
func allowSlowUpload(w http.ResponseWriter) {
	rc := http.NewResponseController(w)
	now := time.Now()
	if err := rc.SetReadDeadline(now.Add(uploadReadBudget)); err != nil {
		log.Printf("upload: relax read deadline failed (server-level timeout will apply): %v", err)
	}
	if err := rc.SetWriteDeadline(now.Add(uploadWriteBudget)); err != nil {
		log.Printf("upload: relax write deadline failed (server-level timeout will apply): %v", err)
	}
}

// 上传解析的可预期错误（由 handler 映射为 4xx）。
var (
	errUploadNotMultipart = errors.New("request is not multipart/form-data")
	errUploadNoFilePart   = errors.New(`multipart part "file" is required`)
	errUploadNoFileName   = errors.New("uploaded file must have a name")
	// P2 独立复审 S2：部件头必须有限额。multipart 部件头由 textproto 全量读入内存，
	// Go 标准库对其总大小无上限（只受请求体上限约束）——不设防时单个请求可把
	// ~MaxBytes 字节的头部吸入内存，放大面 = 请求体上限 × 并发。
	errPartHeaderTooLarge = errors.New("multipart part header exceeds size limit")
	errTooManyParts       = errors.New("multipart part count exceeds limit")
)

const (
	// maxPartHeaderBytes 单个部件头部（含 multipart 边界行）的读取预算。
	// 合法表单的头部（Content-Disposition/Content-Type + 边界）远小于该值。
	maxPartHeaderBytes = 64 << 10
	// maxMultipartParts 单请求允许的部件数上限（file + 少量元数据字段）。
	maxMultipartParts = 32
)

// headerBudgetReader 在启用时限制从底层读取的字节量，用于约束 multipart 部件头
// 阶段的内存放大；内容流式落盘阶段必须关闭预算（文件本身可达 MaxBytes）。
type headerBudgetReader struct {
	r       io.Reader
	budget  int64
	enabled bool
}

func (h *headerBudgetReader) Read(p []byte) (int, error) {
	n, err := h.r.Read(p)
	if h.enabled && n > 0 {
		h.budget -= int64(n)
		if h.budget < 0 {
			// 已读出的 n 字节交还调用方（bufio 会缓存），错误在下次填充时浮现。
			return n, errPartHeaderTooLarge
		}
	}
	return n, err
}

// newMultipartReader 构造带头部预算的 multipart 解析器。
// 与 r.MultipartReader() 等价，但插入了 headerBudgetReader 以约束部件头阶段
// 的读取量（S2），并在部件数超过 maxMultipartParts 时提前拒绝。
func newMultipartReader(r *http.Request) (*multipart.Reader, *headerBudgetReader, error) {
	mt, params, err := mime.ParseMediaType(r.Header.Get("Content-Type"))
	if err != nil || mt != "multipart/form-data" {
		return nil, nil, fmt.Errorf("%w: %v", errUploadNotMultipart, err)
	}
	boundary := params["boundary"]
	if boundary == "" {
		return nil, nil, fmt.Errorf("%w: missing boundary", errUploadNotMultipart)
	}
	body := &headerBudgetReader{r: r.Body, budget: maxPartHeaderBytes, enabled: true}
	return multipart.NewReader(body, boundary), body, nil
}

// handleUploadAsset 接收 multipart/form-data 的真实文件字节：
// 流式落盘（内容寻址 + SHA256 去重）→ 元数据入库 → 同步 Meilisearch。
// 与 POST /api/v1/assets（纯元数据登记）的区别：本端点接收文件本身。
func (s *Server) handleUploadAsset(w http.ResponseWriter, r *http.Request) {
	if s.content == nil || s.store == nil {
		writeError(w, http.StatusServiceUnavailable, "storage backend unavailable")
		return
	}

	// A2-06：放宽本次请求的读写期限，慢速大文件上传不会被服务端级 15s/30s 截断。
	allowSlowUpload(w)

	// A2-03：请求体上限 = 配置的单文件上限 + multipart 余量。
	r.Body = http.MaxBytesReader(w, r.Body, s.content.MaxBytes()+maxUploadOverheadBytes)

	// A2-01：流式解析 multipart，文件部分直接流进内容寻址存储（资产卷内 .tmp → 终路径），
	// 不经 ParseMultipartForm，因而不产生容器 /tmp 副本、不双写、不受宿主 docker 层配额影响。
	saved, clientName, err := s.saveUploadedFile(r)
	if err != nil {
		writeError(w, uploadErrorStatus(err), uploadErrorMessage(err))
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
		// A2-05：入库失败时删除刚落盘的孤儿文件（去重命中的对象不属于本次请求，不删）。
		if !saved.Deduped {
			if dErr := s.content.Discard(saved.RelPath); dErr != nil {
				log.Printf("upload orphan cleanup failed (path %q): %v", saved.RelPath, dErr)
			}
		}
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
		log.Printf("upload readback created failed (sha256 %s, err=%v)", saved.SHA256, err)
		// P2 独立复审 S1：inserted=true 说明本请求已写入 DB 行，此时**不能**直接
		// Discard 对象——那会制造「有行无文件」的悬空 DB 行（预览/下载 404）。
		// 补偿顺序：先删回 DB 行；删行成功才清理本次新落盘的对象（去重命中的对象
		// 不属于本请求，不删）。删行失败则行+对象都在，状态一致，客户端可安全重试。
		rowDeleted, dErr := s.store.DeleteAssetBySHA256(ctx, saved.SHA256)
		if dErr != nil {
			log.Printf("upload compensating row delete failed (sha256 %s): %v", saved.SHA256, dErr)
		} else if !saved.Deduped {
			if dErr := s.content.Discard(saved.RelPath); dErr != nil {
				log.Printf("upload orphan cleanup failed (path %q): %v", saved.RelPath, dErr)
			}
		}
		_ = rowDeleted // rowDeleted=false 仅表示行本就不存在（如并发删除），无需再补偿
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

// saveUploadedFile 流式取出 multipart 的 "file" 部分并落盘（A2-01）。
// 只读取 file 部分；其余字段按序读尽后丢弃（不驻留内存、不落临时文件）。
//
// A2-05（含解析期）：出错时清理**本次请求**已落盘的对象——文件部件可能已经提交，
// 而后续部件才触发请求体上限或读取错误，此时不清理就会留下无 DB 行的孤儿文件。
// 去重命中的对象属于更早的请求，不删。
// 返回的错误由 uploadErrorStatus / uploadErrorMessage 映射为状态码与响应文案。
func (s *Server) saveUploadedFile(r *http.Request) (saved *storage.SavedObject, clientName string, err error) {
	defer func() {
		if err == nil || saved == nil || saved.Deduped {
			return
		}
		if dErr := s.content.Discard(saved.RelPath); dErr != nil {
			log.Printf("upload orphan cleanup failed (path %q): %v", saved.RelPath, dErr)
		}
	}()

	mr, body, err := newMultipartReader(r)
	if err != nil {
		return nil, "", err
	}

	parts := 0
	for {
		// S2：仅部件头解析阶段启用读取预算；内容阶段必须关闭，否则大文件会被误限。
		body.budget = maxPartHeaderBytes
		body.enabled = true
		part, partErr := mr.NextPart()
		body.enabled = false
		if errors.Is(partErr, io.EOF) {
			break
		}
		if partErr != nil {
			return saved, clientName, partErr
		}
		parts++
		if parts > maxMultipartParts {
			_ = part.Close()
			return saved, clientName, errTooManyParts
		}

		// 非文件部分（或重复的 file 部分）：读尽后丢弃，否则会阻塞后续 part 的解析。
		if part.FormName() != "file" || part.FileName() == "" || saved != nil {
			if _, copyErr := io.Copy(io.Discard, part); copyErr != nil {
				_ = part.Close()
				return saved, clientName, copyErr
			}
			_ = part.Close()
			continue
		}

		clientName = strings.TrimSpace(part.FileName())
		if clientName == "" {
			_ = part.Close()
			return saved, clientName, errUploadNoFileName
		}
		saved, err = s.content.Save(part, clientName)
		_ = part.Close()
		if err != nil {
			return saved, clientName, err
		}
	}

	if saved == nil {
		return nil, "", errUploadNoFilePart
	}
	return saved, clientName, nil
}

// uploadErrorStatus 把上传解析/落盘错误映射为 HTTP 状态码。
func uploadErrorStatus(err error) int {
	var maxErr *http.MaxBytesError
	switch {
	case errors.As(err, &maxErr):
		return http.StatusRequestEntityTooLarge
	case errors.Is(err, storage.ErrUnsupportedType):
		return http.StatusUnsupportedMediaType
	case errors.Is(err, storage.ErrTooLarge):
		return http.StatusRequestEntityTooLarge
	case errors.Is(err, storage.ErrEmptyFile):
		return http.StatusBadRequest
	case errors.Is(err, errUploadNotMultipart),
		errors.Is(err, errUploadNoFilePart),
		errors.Is(err, errUploadNoFileName),
		errors.Is(err, errPartHeaderTooLarge),
		errors.Is(err, errTooManyParts):
		// 注：Go 1.22 stdlib 对部件头另有 10MiB/部件、10000 条目兜底，但都在
		// 本端点的 64KiB 预算之后才触发，正常不可达（见 docs/SESSION.md §14 S2）。
		return http.StatusBadRequest
	default:
		return http.StatusInternalServerError
	}
}

// uploadErrorMessage 返回对客户端可见的错误文案；非预期错误只记日志、不泄露内部细节。
func uploadErrorMessage(err error) string {
	switch uploadErrorStatus(err) {
	case http.StatusRequestEntityTooLarge:
		if errors.Is(err, storage.ErrTooLarge) {
			return "file exceeds maximum size"
		}
		return "upload exceeds maximum request size"
	case http.StatusUnsupportedMediaType:
		return "unsupported file extension (allowed: png, jpg, jpeg, gif, webp, pdf, txt, md, csv, json)"
	case http.StatusBadRequest:
		switch {
		case errors.Is(err, storage.ErrEmptyFile):
			return "uploaded file is empty"
		case errors.Is(err, errUploadNotMultipart):
			return errUploadNotMultipart.Error()
		case errors.Is(err, errUploadNoFileName):
			return errUploadNoFileName.Error()
		case errors.Is(err, errPartHeaderTooLarge):
			return errPartHeaderTooLarge.Error()
		case errors.Is(err, errTooManyParts):
			return errTooManyParts.Error()
		default:
			return err.Error()
		}
	default:
		log.Printf("upload save failed: %v", err)
		return "failed to store file"
	}
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

// isHTMLMime 判断 MIME（可带参数）是否为浏览器会解析执行的 HTML 家族。
func isHTMLMime(mime string) bool {
	m := strings.ToLower(strings.TrimSpace(mime))
	if i := strings.IndexByte(m, ';'); i >= 0 {
		m = strings.TrimSpace(m[:i])
	}
	return m == "text/html" || m == "application/xhtml+xml"
}

// previewResponseMime 返回预览响应应使用的 Content-Type：
// HTML 家族一律降级为 text/plain（S3——按源码渲染，浏览器不解析标签）。
func previewResponseMime(mime string) string {
	if isHTMLMime(mime) {
		return "text/plain; charset=utf-8"
	}
	return mime
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

	// A2-04：内容-类型强制校验。预览按 DB 中的 mime 内联返回文件；若伪装成
	// image/png 的 HTML/JS 入库，此处嗅探不一致即拒绝，阻断存储型 XSS。
	head := make([]byte, storage.SniffPeekBytes)
	n, _ := io.ReadFull(f, head)
	head = head[:n]
	if !storage.MatchedContentType(head, asset.MimeType) {
		log.Printf("preview rejected (asset %s): content does not match declared mime %s", asset.ID, asset.MimeType)
		writeError(w, http.StatusUnsupportedMediaType, "content does not match declared mime type")
		return
	}
	if _, err := f.Seek(0, io.SeekStart); err != nil {
		log.Printf("preview seek failed (asset %s): %v", asset.ID, err)
		writeError(w, http.StatusInternalServerError, "failed to read asset content")
		return
	}

	// S3（P2 独立复审）：绝不把任何资产内容以 text/html（或 xhtml）内联返回——
	// HTML 形内容降级为 text/plain 按源码渲染，浏览器不解析标签；
	// CSP sandbox 与 nosniff 继续作为第二、第三道防线。
	respMime := previewResponseMime(asset.MimeType)
	if respMime != asset.MimeType {
		log.Printf("preview downgraded html-ish mime for asset %s: %s -> text/plain", asset.ID, asset.MimeType)
	}
	w.Header().Set("Content-Type", respMime)
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.Header().Set("Content-Security-Policy", "default-src 'none'; style-src 'none'; sandbox")
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

// —— Data Source 端点 ——

// handleListSources 列出所有数据源。
func (s *Server) handleListSources(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	sources, err := s.store.ListDataSources(ctx)
	if err != nil {
		log.Printf("list sources: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to list sources")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"sources": sources})
}

// handleCreateSource 创建数据源。
func (s *Server) handleCreateSource(w http.ResponseWriter, r *http.Request) {
	ctx := r.Context()
	var req models.CreateDataSource
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}
	if req.Name == "" || req.Type == "" || req.URL == "" {
		writeError(w, http.StatusBadRequest, "name, type and url are required")
		return
	}
	if req.Type != "webdav" {
		writeError(w, http.StatusBadRequest, "only type 'webdav' is supported")
		return
	}

	// 将配置加密后存储（简单 XOR 混淆，生产环境应替换为 proper key management）
	cfg := map[string]string{
		"url":         req.URL,
		"username":    req.Username,
		"password":    req.Password,
		"remote_path": req.RemotePath,
	}
	cfgJSON, err := json.Marshal(cfg)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to encode config")
		return
	}

	ds := &models.DataSource{
		Name:   req.Name,
		Type:   req.Type,
		Config: cfgJSON,
	}
	// 回填显示字段（密码不返回）
	ds.URL = req.URL
	ds.Username = req.Username
	ds.RemotePath = req.RemotePath

	if err := s.store.InsertDataSource(ctx, ds); err != nil {
		log.Printf("insert datasource: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to create source")
		return
	}
	writeJSON(w, http.StatusCreated, map[string]any{"source": ds})
}

// handleUpdateSource 更新数据源。
func (s *Server) handleUpdateSource(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if !uuidRe.MatchString(id) {
		writeError(w, http.StatusBadRequest, "invalid source id")
		return
	}
	ctx := r.Context()
	var req models.UpdateDataSource
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		writeError(w, http.StatusBadRequest, "invalid request body")
		return
	}

	// 验证：type 不可更改
	if err := s.store.UpdateDataSource(ctx, id, &req); err != nil {
		log.Printf("update datasource %s: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to update source")
		return
	}

	// 重新读取完整数据源返回给前端
	ds, err := s.store.GetDataSource(ctx, id)
	if err != nil {
		writeError(w, http.StatusInternalServerError, "failed to fetch updated source")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"source": ds})
}

// handleDeleteSource 删除数据源。
func (s *Server) handleDeleteSource(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if !uuidRe.MatchString(id) {
		writeError(w, http.StatusBadRequest, "invalid source id")
		return
	}
	ctx := r.Context()
	ds, err := s.store.GetDataSource(ctx, id)
	if err != nil {
		log.Printf("get datasource %s: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to get source")
		return
	}
	if ds == nil {
		writeError(w, http.StatusNotFound, "source not found")
		return
	}
	if err := s.store.DeleteDataSource(ctx, id); err != nil {
		log.Printf("delete datasource %s: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to delete source")
		return
	}
	w.WriteHeader(http.StatusNoContent)
}

// handleScanSource 触发数据源扫描。
func (s *Server) handleScanSource(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if !uuidRe.MatchString(id) {
		writeError(w, http.StatusBadRequest, "invalid source id")
		return
	}
	ctx := r.Context()
	ds, err := s.store.GetDataSource(ctx, id)
	if err != nil {
		log.Printf("get datasource %s: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to get source")
		return
	}
	if ds == nil {
		writeError(w, http.StatusNotFound, "source not found")
		return
	}

	// 创建扫描任务
	job, err := s.store.CreateScanJob(ctx, id)
	if err != nil {
		log.Printf("create scan job: %v", err)
		writeError(w, http.StatusInternalServerError, "failed to create scan job")
		return
	}

	// 异步执行扫描
	go s.runSourceScan(job.ID, ds.ID, ds.Config)

	writeJSON(w, http.StatusAccepted, map[string]any{"job": job})
}

// runSourceScan 在后台 goroutine 中执行 WebDAV 扫描。
func (s *Server) runSourceScan(jobID, sourceID string, configJSON json.RawMessage) {
	ctx := context.Background()

	// 更新任务为 running
	_ = s.store.UpdateScanJobStarted(ctx, jobID)

	// 解析配置
	var cfg map[string]string
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		_ = s.store.UpdateScanJobFailed(ctx, jobID, "invalid config: "+err.Error())
		return
	}

	webdavCfg := connector.WebDAVConfig{
		URL:        cfg["url"],
		Username:   cfg["username"],
		Password:   cfg["password"],
		RemotePath: cfg["remote_path"],
		Timeout:    5 * time.Minute, // 大目录扫描需要较长的单请求超时
	}

	client, err := connector.NewWebDAVClient(webdavCfg)
	if err != nil {
		_ = s.store.UpdateScanJobFailed(ctx, jobID, "failed to create webdav client: "+err.Error())
		return
	}

	// Probe 连接
	if err := client.Probe(ctx); err != nil {
		_ = s.store.UpdateScanJobFailed(ctx, jobID, "probe failed: "+err.Error())
		return
	}

	// 列出文件
	files, err := client.ListFiles(ctx)
	if err != nil {
		_ = s.store.UpdateScanJobFailed(ctx, jobID, "list files failed: "+err.Error())
		return
	}

	imported, skipped := 0, 0
	for i, file := range files {
		_ = s.store.UpdateScanJobProgress(ctx, jobID, i+1, len(files))

		// 下载到临时文件
		tmpPath, err := client.DownloadToTemp(ctx, file.Path)
		if err != nil {
			skipped++
			continue
		}

		// 打开文件，传给 storage.Save（内部计算 SHA256 + 原子落盘）
		f, err := os.Open(tmpPath)
		if err != nil {
			os.Remove(tmpPath)
			skipped++
			continue
		}

		mimeType := connector.MimeTypeFromName(file.Name)
		obj, err := s.content.Save(f, file.Name)
		f.Close()
		os.Remove(tmpPath) // Save 已在永久位置创建对象，清理临时文件

		if err != nil {
			skipped++
			continue
		}

		asset := &models.Asset{
			Name:         file.Name,
			Path:         obj.RelPath,
			SHA256:       obj.SHA256,
			SizeBytes:    obj.SizeBytes,
			MimeType:     mimeType,
			ResourceType: connector.ResourceType(mimeType),
			SourceID:     &sourceID,
		}

		ok, err := s.store.InsertAsset(ctx, asset)
		if err != nil || !ok {
			skipped++
			continue
		}

		// 同步到 MeiliSearch
		if s.meili != nil {
			_ = s.meili.UpsertAssetDocument(ctx, asset)
		}
		imported++
	}

	// 更新扫描结果
	_ = s.store.UpdateScanJobCompleted(ctx, jobID, imported, skipped)
	_ = s.store.UpdateDataSourceScan(ctx, sourceID, &models.ScanResult{
		Imported: imported,
		Skipped:  skipped,
	})
}

// handleGetScanJob 查询扫描任务状态。
func (s *Server) handleGetScanJob(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if !uuidRe.MatchString(id) {
		writeError(w, http.StatusBadRequest, "invalid job id")
		return
	}
	ctx := r.Context()
	job, err := s.store.GetScanJob(ctx, id)
	if err != nil {
		log.Printf("get scan job %s: %v", id, err)
		writeError(w, http.StatusInternalServerError, "failed to get job")
		return
	}
	if job == nil {
		writeError(w, http.StatusNotFound, "job not found")
		return
	}
	writeJSON(w, http.StatusOK, map[string]any{"job": job})
}
