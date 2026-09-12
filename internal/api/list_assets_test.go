// 列表接口的契约与排序路由测试（T6′ C2-1 / C2-3 / C2-8 回归）。
// 旧的 handlers_test.go 只覆盖 meili=nil 的降级路径，无法拦住"响应字段名错位"
// 与"sort 参数被静默丢弃"这两类缺陷；这里用假 Meilisearch 覆盖 Meili 分支。
package api

import (
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync"
	"testing"

	"partisync/server/internal/search"
)

// meiliStub 是假 Meilisearch：记录收到的请求体，返回固定命中。
type meiliStub struct {
	mu     sync.Mutex
	bodies []map[string]any
}

func (s *meiliStub) record(body map[string]any) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.bodies = append(s.bodies, body)
}

func (s *meiliStub) last() map[string]any {
	s.mu.Lock()
	defer s.mu.Unlock()
	if len(s.bodies) == 0 {
		return nil
	}
	return s.bodies[len(s.bodies)-1]
}

func newMeiliStubServer(t *testing.T) (*meiliStub, *httptest.Server) {
	t.Helper()
	stub := &meiliStub{}
	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var body map[string]any
		if r.Body != nil {
			_ = json.NewDecoder(r.Body).Decode(&body)
		}
		stub.record(body)
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"hits":[{"id":"11111111-1111-1111-1111-111111111111","name":"hero.png","mime_type":"image/png","resource_type":"image","size_bytes":13057}],"estimatedTotalHits":42}`))
	}))
	t.Cleanup(ts.Close)
	return stub, ts
}

// TestListAssetsMeiliContract 断言 Meili 路径的响应键为 results/total
// （T6′ C2-1：前端曾读 r.assets，后端只给 results，导致列表恒为空）。
func TestListAssetsMeiliContract(t *testing.T) {
	_, ts := newMeiliStubServer(t)
	mux := (&Server{meili: search.NewClient(ts.URL, "test-key")}).NewServeMux()

	w := doReq(t, mux, http.MethodGet, "/api/v1/assets?q=hero&limit=24", "")
	if w.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200 (body=%s)", w.Code, w.Body.String())
	}

	var resp map[string]any
	if err := json.Unmarshal(w.Body.Bytes(), &resp); err != nil {
		t.Fatalf("decode response: %v", err)
	}
	results, ok := resp["results"].([]any)
	if !ok {
		t.Fatalf("response has no results array: %s", w.Body.String())
	}
	if len(results) != 1 {
		t.Errorf("results len = %d, want 1", len(results))
	}
	if total, ok := resp["total"].(float64); !ok || total != 42 {
		t.Errorf("total = %v, want 42", resp["total"])
	}
	if _, stale := resp["assets"]; stale {
		t.Error("response still carries the unused assets key")
	}
}

// TestListAssetsSortOnlyReachesMeili：仅带 sort 的请求也必须走检索后端，
// 否则 sort 会被 PG 路径静默丢弃（T6′ C2-3）。
func TestListAssetsSortOnlyReachesMeili(t *testing.T) {
	stub, ts := newMeiliStubServer(t)
	mux := (&Server{meili: search.NewClient(ts.URL, "test-key")}).NewServeMux()

	w := doReq(t, mux, http.MethodGet, "/api/v1/assets?sort=size_bytes:desc&limit=5", "")
	if w.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200 (body=%s)", w.Code, w.Body.String())
	}
	sent := stub.last()
	if sent == nil {
		t.Fatal("no request reached the search backend: sort was dropped")
	}
	sort, ok := sent["sort"].([]any)
	if !ok || len(sort) != 1 || sort[0] != "size_bytes:desc" {
		t.Errorf("forwarded sort = %v, want [size_bytes:desc]", sent["sort"])
	}
}

// TestListAssetsRejectsInvalidSort：非法排序是调用方错误（400），
// 不能转发给 Meilisearch 后再被映射成 502（T6′ C2-3）。
func TestListAssetsRejectsInvalidSort(t *testing.T) {
	stub, ts := newMeiliStubServer(t)
	mux := (&Server{meili: search.NewClient(ts.URL, "test-key")}).NewServeMux()

	for _, target := range []string{
		"/api/v1/assets?sort=bogus:asc",
		"/api/v1/assets?sort=size_bytes",
		"/api/v1/assets?sort=size_bytes:up",
	} {
		w := doReq(t, mux, http.MethodGet, target, "")
		if w.Code != http.StatusBadRequest {
			t.Errorf("%s: status = %d, want 400 (body=%s)", target, w.Code, w.Body.String())
		}
		if !strings.Contains(w.Body.String(), "sort") {
			t.Errorf("%s: body = %s, want a sort-related message", target, w.Body.String())
		}
	}
	if sent := stub.last(); sent != nil {
		t.Errorf("invalid sort reached the search backend: %v", sent)
	}
}

// TestListAssetsWithoutDepsFailsCleanly：无 store 且无检索条件时报 503（保持既有降级语义）。
func TestListAssetsWithoutDepsFailsCleanly(t *testing.T) {
	mux := (&Server{}).NewServeMux()
	w := doReq(t, mux, http.MethodGet, "/api/v1/assets", "")
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("status = %d, want 503", w.Code)
	}
}
