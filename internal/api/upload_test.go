// 上传端点（POST /api/v1/assets/upload）的 L1 测试：覆盖真实失败路径。
// 只覆盖在任何数据库/Meilisearch 访问之前就会返回的分支；成功路径由 L2 集成验证覆盖
// （见 docs/evidence/l2-integration-t6-repair.md）。
package api

import (
	"bytes"
	"mime/multipart"
	"net/http"
	"net/http/httptest"
	"testing"

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
