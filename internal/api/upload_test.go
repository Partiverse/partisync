// 上传端点（POST /api/v1/assets/upload）的 L1 测试：覆盖真实失败路径。
// 只覆盖在任何数据库/Meilisearch 访问之前就会返回的分支；成功路径由 L2 集成验证覆盖
// （见 docs/evidence/l2-integration-t6-repair.md）。
package api

import (
	"bytes"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"partisync/server/internal/storage"
	"partisync/server/internal/store"
)

// newTestStorage 返回临时目录上的内容存储。
func newTestStorage(t *testing.T, maxBytes int64) *storage.Store {
	t.Helper()
	s, err := storage.New(t.TempDir(), maxBytes)
	if err != nil {
		t.Fatalf("storage.New: %v", err)
	}
	return s
}

// uploadRequest 构造 multipart/form-data 上传请求；filename/content 为空表示缺失对应部分。
func uploadRequest(t *testing.T, filename string, content []byte, includeFile bool) *http.Request {
	t.Helper()
	var body bytes.Buffer
	mw := multipart.NewWriter(&body)
	if includeFile {
		part, err := mw.CreateFormFile("file", filename)
		if err != nil {
			t.Fatalf("CreateFormFile: %v", err)
		}
		if _, err := part.Write(content); err != nil {
			t.Fatalf("write part: %v", err)
		}
	}
	if err := mw.Close(); err != nil {
		t.Fatalf("close multipart writer: %v", err)
	}
	r := httptest.NewRequest(http.MethodPost, "/api/v1/assets/upload", &body)
	r.Header.Set("Content-Type", mw.FormDataContentType())
	return r
}

func serveUpload(t *testing.T, s *Server, r *http.Request) *httptest.ResponseRecorder {
	t.Helper()
	w := httptest.NewRecorder()
	s.NewServeMux().ServeHTTP(w, r)
	return w
}

func TestUploadUnavailableWithoutContentStore(t *testing.T) {
	s := &Server{store: new(store.Store), meili: nil, content: nil}

	w := serveUpload(t, s, uploadRequest(t, "photo.png", []byte("data"), true))
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("status = %d, want 503", w.Code)
	}
}

func TestUploadUnavailableWithoutStore(t *testing.T) {
	s := &Server{store: nil, meili: nil, content: newTestStorage(t, 0)}

	w := serveUpload(t, s, uploadRequest(t, "photo.png", []byte("data"), true))
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("status = %d, want 503", w.Code)
	}
}

func TestUploadRejectsMissingFilePart(t *testing.T) {
	s := &Server{store: new(store.Store), meili: nil, content: newTestStorage(t, 0)}

	w := serveUpload(t, s, uploadRequest(t, "", nil, false))
	if w.Code != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400", w.Code)
	}
}

func TestUploadRejectsUnsupportedExtension(t *testing.T) {
	s := &Server{store: new(store.Store), meili: nil, content: newTestStorage(t, 0)}

	for _, name := range []string{"payload.exe", "noext", "script.sh", "../../etc/passwd"} {
		w := serveUpload(t, s, uploadRequest(t, name, []byte("MZ binary"), true))
		if w.Code != http.StatusUnsupportedMediaType {
			t.Fatalf("name %q: status = %d, want 415", name, w.Code)
		}
	}
}

func TestUploadRejectsEmptyFile(t *testing.T) {
	s := &Server{store: new(store.Store), meili: nil, content: newTestStorage(t, 0)}

	w := serveUpload(t, s, uploadRequest(t, "empty.png", nil, true))
	if w.Code != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400", w.Code)
	}
}

func TestUploadRejectsOversizedFile(t *testing.T) {
	s := &Server{store: new(store.Store), meili: nil, content: newTestStorage(t, 4)}

	w := serveUpload(t, s, uploadRequest(t, "big.txt", []byte("0123456789"), true))
	if w.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("status = %d, want 413", w.Code)
	}
}

// 客户端文件名含路径成分时，展示名必须退化为纯文件名（不含目录）。
func TestDisplayNameStripsPathAndControlChars(t *testing.T) {
	cases := map[string]string{
		"../../etc/passwd.png": "passwd.png",
		"a/b/photo.png":        "photo.png",
		"photo\x00.png":        "photo.png",
		"":                     "unnamed",
		".":                    "unnamed",
	}
	for in, want := range cases {
		if got := displayName(in); got != want {
			t.Errorf("displayName(%q) = %q, want %q", in, got, want)
		}
	}
}

func TestResourceTypeFor(t *testing.T) {
	cases := map[string]string{
		"image/png":       "image",
		"image/jpeg":      "image",
		"application/pdf": "document",
		"text/plain":      "document",
	}
	for mime, want := range cases {
		if got := resourceTypeFor(mime); got != want {
			t.Errorf("resourceTypeFor(%q) = %q, want %q", mime, got, want)
		}
	}
}

// 请求体上限必须跟随配置的单文件上限（A2-03），而不是硬编码的默认值。
// 用一个超过「单文件上限 + multipart 余量」的非 file 字段触发 MaxBytesReader。
func TestUploadRequestBodyCapFollowsStoreLimit(t *testing.T) {
	content := newTestStorage(t, 4) // 上限 4B → 请求体上限 = 4B + maxUploadOverheadBytes
	s := &Server{store: new(store.Store), meili: nil, content: content}

	var body bytes.Buffer
	mw := multipart.NewWriter(&body)
	field, err := mw.CreateFormField("junk")
	if err != nil {
		t.Fatalf("CreateFormField: %v", err)
	}
	if _, err := field.Write(bytes.Repeat([]byte("A"), int(maxUploadOverheadBytes)+1024)); err != nil {
		t.Fatalf("write field: %v", err)
	}
	if err := mw.Close(); err != nil {
		t.Fatalf("close multipart writer: %v", err)
	}

	r := httptest.NewRequest(http.MethodPost, "/api/v1/assets/upload", &body)
	r.Header.Set("Content-Type", mw.FormDataContentType())

	w := serveUpload(t, s, r)
	if w.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("status = %d, want 413 (body %s)", w.Code, w.Body.String())
	}
}

// 流式解析（A2-01）：文件字节不经 ParseMultipartForm，因而不落容器 /tmp；
// 大小超过旧实现的 8MiB 内存阈值，若改回 ParseMultipartForm 本测试会因临时文件而失败。
func TestSaveUploadedFileStreamsWithoutTempSpill(t *testing.T) {
	spillDir := t.TempDir()
	t.Setenv("TMPDIR", spillDir)

	content := newTestStorage(t, 0)
	s := &Server{content: content}

	payload := bytes.Repeat([]byte{0x89, 'P', 'N', 'G'}, 3<<20) // 12MiB
	var body bytes.Buffer
	mw := multipart.NewWriter(&body)
	if err := mw.WriteField("name", "ignored-before"); err != nil {
		t.Fatalf("WriteField: %v", err)
	}
	part, err := mw.CreateFormFile("file", "big.png")
	if err != nil {
		t.Fatalf("CreateFormFile: %v", err)
	}
	if _, err := part.Write(payload); err != nil {
		t.Fatalf("write part: %v", err)
	}
	if err := mw.WriteField("description", "ignored-after"); err != nil {
		t.Fatalf("WriteField: %v", err)
	}
	if err := mw.Close(); err != nil {
		t.Fatalf("close multipart writer: %v", err)
	}

	r := httptest.NewRequest(http.MethodPost, "/api/v1/assets/upload", &body)
	r.Header.Set("Content-Type", mw.FormDataContentType())

	saved, clientName, err := s.saveUploadedFile(r)
	if err != nil {
		t.Fatalf("saveUploadedFile: %v", err)
	}
	if clientName != "big.png" {
		t.Fatalf("clientName = %q, want big.png", clientName)
	}
	if saved.SizeBytes != int64(len(payload)) {
		t.Fatalf("size = %d, want %d", saved.SizeBytes, len(payload))
	}

	// 1) 对象落在资产卷内（内容寻址路径），且字节完整。
	stored, err := os.ReadFile(filepath.Join(content.Root(), filepath.FromSlash(saved.RelPath)))
	if err != nil {
		t.Fatalf("read stored object: %v", err)
	}
	if !bytes.Equal(stored, payload) {
		t.Fatalf("stored object differs from uploaded payload")
	}

	// 2) 进程临时目录零残留（旧 ParseMultipartForm(8MiB) 实现会在此留下 4MiB 临时文件）。
	entries, err := os.ReadDir(spillDir)
	if err != nil {
		t.Fatalf("read spill dir: %v", err)
	}
	if len(entries) != 0 {
		names := make([]string, 0, len(entries))
		for _, e := range entries {
			names = append(names, e.Name())
		}
		t.Fatalf("temp spill detected: %v", names)
	}

	// 3) 资产卷内 .tmp 也无残留（Save 已把临时文件 rename 到终路径）。
	tmpEntries, err := os.ReadDir(filepath.Join(content.Root(), ".tmp"))
	if err != nil {
		t.Fatalf("read storage tmp: %v", err)
	}
	if len(tmpEntries) != 0 {
		t.Fatalf("storage tmp not empty: %d entries", len(tmpEntries))
	}
}

// 重复的 file 部分只取第一个，后续部分被丢弃且不产生第二个对象。
func TestSaveUploadedFileKeepsFirstFilePart(t *testing.T) {
	content := newTestStorage(t, 0)
	s := &Server{content: content}

	first := []byte("first-payload")
	second := []byte("second-payload-longer")
	var body bytes.Buffer
	mw := multipart.NewWriter(&body)
	for _, payload := range [][]byte{first, second} {
		part, err := mw.CreateFormFile("file", "a.txt")
		if err != nil {
			t.Fatalf("CreateFormFile: %v", err)
		}
		if _, err := part.Write(payload); err != nil {
			t.Fatalf("write part: %v", err)
		}
	}
	if err := mw.Close(); err != nil {
		t.Fatalf("close multipart writer: %v", err)
	}

	r := httptest.NewRequest(http.MethodPost, "/api/v1/assets/upload", &body)
	r.Header.Set("Content-Type", mw.FormDataContentType())

	saved, _, err := s.saveUploadedFile(r)
	if err != nil {
		t.Fatalf("saveUploadedFile: %v", err)
	}
	if saved.SizeBytes != int64(len(first)) {
		t.Fatalf("size = %d, want %d (first part)", saved.SizeBytes, len(first))
	}

	var files int
	err = filepath.WalkDir(content.Root(), func(_ string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if !d.IsDir() {
			files++
		}
		return nil
	})
	if err != nil {
		t.Fatalf("walk storage root: %v", err)
	}
	if files != 1 {
		t.Fatalf("stored files = %d, want 1", files)
	}
}

// 非 multipart 请求必须 400，而不是 500 或 panic。
func TestSaveUploadedFileRejectsNonMultipart(t *testing.T) {
	s := &Server{content: newTestStorage(t, 0)}

	r := httptest.NewRequest(http.MethodPost, "/api/v1/assets/upload", bytes.NewReader([]byte("plain")))
	r.Header.Set("Content-Type", "application/json")

	saved, _, err := s.saveUploadedFile(r)
	if saved != nil {
		t.Fatalf("saved = %v, want nil", saved)
	}
	if status := uploadErrorStatus(err); status != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400 (err %v)", status, err)
	}
	// 对客户端可见的文案不得回显包装后的内部错误串。
	if got := uploadErrorMessage(err); got != errUploadNotMultipart.Error() {
		t.Fatalf("message = %q, want %q", got, errUploadNotMultipart.Error())
	}
}

// A2-06 的口径断言：上传端点的读写预算必须显著大于服务端级常规超时（15s/30s）。
// 注意：这里只锁定数值口径；"慢速上传真的能成功"必须由 L2 实测（>30s 的限速上传）证明。
func TestUploadBudgetsExceedServerDefaults(t *testing.T) {
	if uploadReadBudget <= 15*time.Second {
		t.Fatalf("uploadReadBudget = %v, want > 15s", uploadReadBudget)
	}
	if uploadWriteBudget <= 30*time.Second {
		t.Fatalf("uploadWriteBudget = %v, want > 30s", uploadWriteBudget)
	}
	if uploadWriteBudget <= uploadReadBudget {
		t.Fatalf("uploadWriteBudget = %v must cover the read phase + response write", uploadWriteBudget)
	}
}

// 全空白文件名：与旧实现一致返回 400（而非落到 415「不支持的扩展名」）。
func TestSaveUploadedFileRejectsBlankFileName(t *testing.T) {
	s := &Server{store: new(store.Store), content: newTestStorage(t, 0)}

	w := serveUpload(t, s, uploadRequest(t, "   ", []byte("data"), true))
	if w.Code != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400 (body %s)", w.Code, w.Body.String())
	}
	if !strings.Contains(w.Body.String(), "must have a name") {
		t.Fatalf("body = %s, want name-required message", w.Body.String())
	}
}

// countFiles 统计存储根（含 .tmp）下的普通文件数量。
func countFiles(t *testing.T, root string) int {
	t.Helper()
	n := 0
	err := filepath.WalkDir(root, func(_ string, d os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if !d.IsDir() {
			n++
		}
		return nil
	})
	if err != nil {
		t.Fatalf("walk %s: %v", root, err)
	}
	return n
}

// F6 回归（P2 复审发现）：file 部件已提交落盘、随后部件触发请求体上限时，
// 已落盘对象必须被清理——否则留下无 DB 行的孤儿文件，可被匿名请求反复累积占盘。
func TestUploadCleansUpObjectWhenBodyCapTripsAfterFilePart(t *testing.T) {
	dir := t.TempDir()
	content, err := storage.New(dir, 4) // 单文件上限 4B；请求体上限 = 4B + maxUploadOverheadBytes
	if err != nil {
		t.Fatalf("storage.New: %v", err)
	}
	s := &Server{store: new(store.Store), content: content}

	// build 生成同一 file 部件；withJunk 时追加一个超过请求体上限的普通字段。
	build := func(withJunk bool) (*bytes.Buffer, string) {
		t.Helper()
		var buf bytes.Buffer
		mw := multipart.NewWriter(&buf)
		part, err := mw.CreateFormFile("file", "victim.txt")
		if err != nil {
			t.Fatalf("CreateFormFile: %v", err)
		}
		if _, err := part.Write([]byte("abcd")); err != nil { // 4B：等于上限，可提交
			t.Fatalf("write file part: %v", err)
		}
		if withJunk {
			if err := mw.WriteField("junk", strings.Repeat("J", maxUploadOverheadBytes+1024)); err != nil {
				t.Fatalf("write junk field: %v", err)
			}
		}
		if err := mw.Close(); err != nil {
			t.Fatalf("close multipart writer: %v", err)
		}
		return &buf, mw.FormDataContentType()
	}

	// 对照组：不带超限字段的同一请求必须真的提交落盘（证明清理发生在"落盘之后"）。
	controlBody, controlType := build(false)
	control := httptest.NewRequest(http.MethodPost, "/api/v1/assets/upload", controlBody)
	control.Header.Set("Content-Type", controlType)
	saved, _, err := s.saveUploadedFile(control)
	if err != nil {
		t.Fatalf("control save: %v", err)
	}
	if saved == nil || saved.Deduped {
		t.Fatalf("control: saved = %+v, want a freshly committed object", saved)
	}
	if files := countFiles(t, dir); files != 1 {
		t.Fatalf("control: files = %d, want 1 (the committed object)", files)
	}
	if err := content.Discard(saved.RelPath); err != nil {
		t.Fatalf("control cleanup: %v", err)
	}
	if files := countFiles(t, dir); files != 0 {
		t.Fatalf("control cleanup left %d files", files)
	}

	// 实验：同一 file 部件之后跟超限字段 → 413，且存储里不留任何文件。
	body, contentType := build(true)
	r := httptest.NewRequest(http.MethodPost, "/api/v1/assets/upload", body)
	r.Header.Set("Content-Type", contentType)
	w := serveUpload(t, s, r)
	if w.Code != http.StatusRequestEntityTooLarge {
		t.Fatalf("status = %d, want 413 (body %s)", w.Code, w.Body.String())
	}
	if files := countFiles(t, dir); files != 0 {
		t.Fatalf("orphan objects left after parse failure: %d files", files)
	}
}
