// Package api 处理器单元测试：只覆盖请求校验分支。
// Server 以 nil store/meili 构造 —— 非法请求必须在触达依赖前被拒绝，
// 因此测试不连接任何真实数据库或 Meilisearch。
package api

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
)

// newTestMux 返回仅含路由的测试服务器（依赖全部为 nil）。
func newTestMux() http.Handler {
	return (&Server{store: nil, meili: nil}).NewServeMux()
}

// doReq 发送请求并返回 ResponseRecorder。
func doReq(t *testing.T, h http.Handler, method, target, body string) *httptest.ResponseRecorder {
	t.Helper()
	var r *http.Request
	if body == "" {
		r = httptest.NewRequest(method, target, nil)
	} else {
		r = httptest.NewRequest(method, target, strings.NewReader(body))
		r.Header.Set("Content-Type", "application/json")
	}
	w := httptest.NewRecorder()
	h.ServeHTTP(w, r)
	return w
}

func TestHealthz(t *testing.T) {
	w := doReq(t, newTestMux(), http.MethodGet, "/healthz", "")
	if w.Code != http.StatusOK {
		t.Fatalf("healthz status = %d, want 200", w.Code)
	}
	if !strings.Contains(w.Body.String(), `"status":"ok"`) {
		t.Fatalf("healthz body = %s, want status ok", w.Body.String())
	}
}

func TestCreateAssetValidation(t *testing.T) {
	const validSHA = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

	cases := []struct {
		name string
		body string
	}{
		{"missing name", `{"path":"/a/b.png","sha256":"` + validSHA + `","mime_type":"image/png"}`},
		{"missing path", `{"name":"b.png","sha256":"` + validSHA + `","mime_type":"image/png"}`},
		{"missing sha256", `{"name":"b.png","path":"/a/b.png","mime_type":"image/png"}`},
		{"empty name", `{"name":"   ","path":"/a/b.png","sha256":"` + validSHA + `","mime_type":"image/png"}`},
		{"sha too short", `{"name":"b.png","path":"/a/b.png","sha256":"abc123","mime_type":"image/png"}`},
		{"sha non hex", `{"name":"b.png","path":"/a/b.png","sha256":"` + strings.Repeat("g", 64) + `","mime_type":"image/png"}`},
		{"sha too long", `{"name":"b.png","path":"/a/b.png","sha256":"` + validSHA + `a","mime_type":"image/png"}`},
		{"bad mime no slash", `{"name":"b.png","path":"/a/b.png","sha256":"` + validSHA + `","mime_type":"imagepng"}`},
		{"bad mime leading slash", `{"name":"b.png","path":"/a/b.png","sha256":"` + validSHA + `","mime_type":"/png"}`},
		{"missing mime", `{"name":"b.png","path":"/a/b.png","sha256":"` + validSHA + `"}`},
		{"empty body", ""},
		{"malformed json", `{"name":`},
	}

	mux := newTestMux()
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			w := doReq(t, mux, http.MethodPost, "/api/v1/assets", tc.body)
			if w.Code != http.StatusBadRequest {
				t.Fatalf("status = %d, want 400 (body=%q)", w.Code, tc.body)
			}
			if !strings.Contains(w.Body.String(), `"error"`) {
				t.Fatalf("body = %s, want error field", w.Body.String())
			}
		})
	}
}

func TestGetAssetInvalidUUID(t *testing.T) {
	mux := newTestMux()
	for _, id := range []string{
		"not-a-uuid",
		"123",
		"6f1e2d3c-4b5a-4938-2716-0000000000ag", // 非十六进制
		"6f1e2d3c-4b5a-6938-2716-0000000000aa", // 版本号非法
	} {
		target := "/api/v1/assets/" + id
		w := doReq(t, mux, http.MethodGet, target, "")
		if w.Code != http.StatusBadRequest {
			t.Fatalf("GET %s status = %d, want 400", target, w.Code)
		}
	}
}

func TestAnnotateInvalidUUID(t *testing.T) {
	w := doReq(t, newTestMux(), http.MethodPost, "/api/v1/assets/not-a-uuid/annotate",
		`{"prompt":"describe"}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400", w.Code)
	}
}

func TestAnnotateInvalidJSON(t *testing.T) {
	const uuid = "6f1e2d3c-4b5a-4938-a716-0000000000aa"
	w := doReq(t, newTestMux(), http.MethodPost, "/api/v1/assets/"+uuid+"/annotate", `{oops`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400", w.Code)
	}
}

func TestAnnotateEmptyPrompt(t *testing.T) {
	const uuid = "6f1e2d3c-4b5a-4938-a716-0000000000aa"
	w := doReq(t, newTestMux(), http.MethodPost, "/api/v1/assets/"+uuid+"/annotate",
		`{"prompt":"  "}`)
	if w.Code != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400", w.Code)
	}
}

func TestListAssetsInvalidLimit(t *testing.T) {
	mux := newTestMux()
	for _, q := range []string{"limit=abc", "limit=0", "limit=-5", "offset=-1", "offset=x"} {
		w := doReq(t, mux, http.MethodGet, "/api/v1/assets?"+q, "")
		if w.Code != http.StatusBadRequest {
			t.Fatalf("query %s status = %d, want 400", q, w.Code)
		}
	}
}

func TestListJobsInvalidLimit(t *testing.T) {
	w := doReq(t, newTestMux(), http.MethodGet, "/api/v1/jobs?limit=xyz", "")
	if w.Code != http.StatusBadRequest {
		t.Fatalf("status = %d, want 400", w.Code)
	}
}

// TestSearchUnavailableWithoutMeili 验证 meili=nil 且带 q 或 filter 时降级为 502 而非 panic。
func TestSearchUnavailableWithoutMeili(t *testing.T) {
	mux := newTestMux()
	queries := []string{"q=cat", "resource_type=image", "mime_type=image/png", "q=test&resource_type=document"}
	for _, q := range queries {
		w := doReq(t, mux, http.MethodGet, "/api/v1/assets?"+q, "")
		if w.Code != http.StatusBadGateway {
			t.Fatalf("query %s status = %d, want 502", q, w.Code)
		}
		if !strings.Contains(w.Body.String(), `"error"`) {
			t.Fatalf("body = %s, want error field", w.Body.String())
		}
	}
}

// TestNilStoreReturns500NotPanic 验证依赖缺失时，校验通过的请求返回 503，
// 请求路径不得 panic（httptest 不 recover，panic 会使测试崩溃）。
func TestNilStoreReturns503NotPanic(t *testing.T) {
	const validSHA = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
	const uuid = "6f1e2d3c-4b5a-4938-a716-0000000000aa"
	mux := newTestMux()

	w := doReq(t, mux, http.MethodPost, "/api/v1/assets",
		`{"name":"x.png","path":"/x.png","sha256":"`+validSHA+`","mime_type":"image/png"}`)
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("POST assets status = %d, want 503", w.Code)
	}

	w = doReq(t, mux, http.MethodGet, "/api/v1/assets?limit=5", "")
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("GET assets status = %d, want 503", w.Code)
	}

	w = doReq(t, mux, http.MethodGet, "/api/v1/assets/"+uuid, "")
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("GET asset status = %d, want 503", w.Code)
	}

	w = doReq(t, mux, http.MethodPost, "/api/v1/assets/"+uuid+"/annotate", `{"prompt":"p"}`)
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("POST annotate status = %d, want 503", w.Code)
	}

	w = doReq(t, mux, http.MethodGet, "/api/v1/jobs", "")
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("GET jobs status = %d, want 503", w.Code)
	}
}

// TestParsePaging 直接覆盖分页解析：默认值、上限截断、非法输入。
func TestParsePaging(t *testing.T) {
	cases := []struct {
		query      string
		wantLimit  int
		wantOffset int
		wantErr    bool
	}{
		{"", 20, 0, false},
		{"limit=5&offset=10", 5, 10, false},
		{"limit=500", 100, 0, false}, // 超上限截断
		{"limit=0", 0, 0, true},
		{"limit=-1", 0, 0, true},
		{"limit=abc", 0, 0, true},
		{"offset=-1", 0, 0, true},
		{"offset=abc", 0, 0, true},
	}
	for _, tc := range cases {
		r := httptest.NewRequest(http.MethodGet, "/api/v1/assets?"+tc.query, nil)
		limit, offset, err := parsePaging(r)
		if tc.wantErr {
			if err == nil {
				t.Fatalf("query %q: want error, got limit=%d", tc.query, limit)
			}
			continue
		}
		if err != nil {
			t.Fatalf("query %q: unexpected error %v", tc.query, err)
		}
		if limit != tc.wantLimit || offset != tc.wantOffset {
			t.Fatalf("query %q: got (%d,%d), want (%d,%d)",
				tc.query, limit, offset, tc.wantLimit, tc.wantOffset)
		}
	}
}
