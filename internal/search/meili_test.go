package search

import (
	"context"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"sync"
	"testing"
)

// captureServer 是记录请求路径与 JSON 请求体的假 Meilisearch。
// 与改造前的测试（恒返 200、不校验请求体）的区别：这里断言真实发出的 payload，
// 因此能拦住 sort/filter 语法与索引设置项的回归（T6′ C2-8）。
type captureServer struct {
	*httptest.Server
	mu   sync.Mutex
	path string
	body map[string]any
}

func newCaptureServer(t *testing.T, respBody string) *captureServer {
	t.Helper()
	cs := &captureServer{}
	cs.Server = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		raw, _ := io.ReadAll(r.Body)
		var body map[string]any
		if len(raw) > 0 {
			_ = json.Unmarshal(raw, &body)
		}
		cs.mu.Lock()
		cs.path = r.URL.Path
		cs.body = body
		cs.mu.Unlock()

		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(respBody))
	}))
	t.Cleanup(cs.Close)
	return cs
}

func (c *captureServer) snapshot() (string, map[string]any) {
	c.mu.Lock()
	defer c.mu.Unlock()
	return c.path, c.body
}

func TestSearchOptionsFormatting(t *testing.T) {
	cs := newCaptureServer(t, `{"hits":[{"id":"1","name":"test.png"}],"estimatedTotalHits":1}`)
	client := NewClient(cs.URL, "test-key")

	hits, total, err := client.Search(context.Background(), "photo", 10, 0, SearchOptions{
		ResourceType: "image",
		MimeType:     "image/png",
		Sort:         "size_bytes:asc",
	})
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}
	if total != 1 {
		t.Errorf("total = %d, want 1", total)
	}
	if len(hits) != 1 {
		t.Fatalf("hits len = %d, want 1", len(hits))
	}
	if hits[0]["name"] != "test.png" {
		t.Errorf("hits[0].name = %v, want test.png", hits[0]["name"])
	}

	path, body := cs.snapshot()
	if path != "/indexes/assets/search" {
		t.Errorf("path = %s, want /indexes/assets/search", path)
	}
	if got, want := body["filter"], `resource_type = "image" AND mime_type = "image/png"`; got != want {
		t.Errorf("filter = %v, want %s", got, want)
	}
	sort, ok := body["sort"].([]any)
	if !ok || len(sort) != 1 || sort[0] != "size_bytes:asc" {
		t.Errorf("sort = %v, want [size_bytes:asc]", body["sort"])
	}
	if body["q"] != "photo" {
		t.Errorf("q = %v, want photo", body["q"])
	}
}

func TestSearchOmitsSortWhenEmpty(t *testing.T) {
	cs := newCaptureServer(t, `{"hits":[],"estimatedTotalHits":0}`)
	client := NewClient(cs.URL, "test-key")

	if _, _, err := client.Search(context.Background(), "photo", 10, 0, SearchOptions{ResourceType: "image"}); err != nil {
		t.Fatalf("Search failed: %v", err)
	}
	if _, body := cs.snapshot(); body["sort"] != nil {
		t.Errorf("sort = %v, want absent", body["sort"])
	}
}

// TestEnsureIndexConfiguresSortableAttributes 覆盖 T6′ C2-2 的根因：
// 索引设置未声明 sortableAttributes，导致任何带 sort 的检索被 Meilisearch
// 以 400 拒绝，再被上层统一映射成 502。
func TestEnsureIndexConfiguresSortableAttributes(t *testing.T) {
	cs := newCaptureServer(t, `{}`)
	client := NewClient(cs.URL, "test-key")

	if err := client.EnsureIndex(context.Background()); err != nil {
		t.Fatalf("EnsureIndex failed: %v", err)
	}

	path, body := cs.snapshot()
	if path != "/indexes/assets/settings" {
		t.Fatalf("path = %s, want /indexes/assets/settings", path)
	}
	rawSortable, ok := body["sortableAttributes"].([]any)
	if !ok {
		t.Fatalf("sortableAttributes missing from settings payload: %v", body)
	}
	if len(rawSortable) != len(SortableFields) {
		t.Fatalf("sortableAttributes = %v, want %v", rawSortable, SortableFields)
	}
	for i, v := range rawSortable {
		if v != SortableFields[i] {
			t.Errorf("sortableAttributes[%d] = %v, want %s", i, v, SortableFields[i])
		}
	}
	if body["filterableAttributes"] == nil {
		t.Error("filterableAttributes missing from settings payload")
	}
}

// TestEnsureIndexRaisesMaxTotalHits 覆盖 T6″ C2-5 的根因：
// maxTotalHits 停留在 Meilisearch 默认 1000 会让 total 被截断、
// offset>1000 静默返回空列表，与"百万资产可检索"口径冲突。
func TestEnsureIndexRaisesMaxTotalHits(t *testing.T) {
	cs := newCaptureServer(t, `{}`)
	client := NewClient(cs.URL, "test-key")

	if err := client.EnsureIndex(context.Background()); err != nil {
		t.Fatalf("EnsureIndex failed: %v", err)
	}

	_, body := cs.snapshot()
	pag, ok := body["pagination"].(map[string]any)
	if !ok {
		t.Fatalf("pagination missing from settings payload: %v", body)
	}
	maxTotal, ok := pag["maxTotalHits"].(float64)
	if !ok {
		t.Fatalf("pagination.maxTotalHits missing or not a number: %v", pag)
	}
	if maxTotal < 1000000 {
		t.Errorf("pagination.maxTotalHits = %v, want >= 1000000 (must cover the 1M benchmark corpus)", maxTotal)
	}
}

// TestValidateSort 保证非法排序在 API 层被表达成 400，
// 而不是等 Meilisearch 拒绝后把整个检索请求升级为 502。
func TestValidateSort(t *testing.T) {
	cases := []struct {
		in      string
		wantErr bool
	}{
		{"", false},
		{"size_bytes:asc", false},
		{"size_bytes:desc", false},
		{"name:asc", false},
		{"bogus:asc", true},
		{"created_at:desc", true}, // 未在 SortableFields 声明 → 拒绝而非转发给 Meili
		{"size_bytes", true},
		{"size_bytes:up", true},
		{"size_bytes:asc:desc", true},
		{":asc", true},
		{"size_bytes:", true},
	}
	for _, tc := range cases {
		err := ValidateSort(tc.in)
		if tc.wantErr && err == nil {
			t.Errorf("ValidateSort(%q) = nil, want error", tc.in)
		}
		if !tc.wantErr && err != nil {
			t.Errorf("ValidateSort(%q) = %v, want nil", tc.in, err)
		}
	}
}
