// /readyz 就绪探针的断言测试（A2-02）：
// 依赖可达性必须逐项上报，任一依赖不可达即 503；/healthz 保持纯存活语义。
package api

import (
	"database/sql"
	"encoding/json"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"partisync/server/internal/search"
	"partisync/server/internal/store"
)

// readyBody 解析 /readyz 响应。
type readyBody struct {
	Ready        bool `json:"ready"`
	Dependencies []struct {
		Name string `json:"name"`
		OK   bool   `json:"ok"`
		Err  string `json:"error"`
	} `json:"dependencies"`
}

func decodeReady(t *testing.T, body string) readyBody {
	t.Helper()
	var b readyBody
	if err := json.Unmarshal([]byte(body), &b); err != nil {
		t.Fatalf("decode /readyz body %q: %v", body, err)
	}
	return b
}

// depOK 返回指定依赖项的 OK 状态；不存在时 ok=false、found=false。
func (b readyBody) depOK(name string) (ok, found bool) {
	for _, d := range b.Dependencies {
		if d.Name == name {
			return d.OK, true
		}
	}
	return false, false
}

// 存储可达、其余依赖未配置（nil）→ 就绪。
func TestReadyWithStorageOnly(t *testing.T) {
	s := &Server{store: nil, meili: nil, content: newTestStorage(t, 0)}

	w := doReq(t, s.NewServeMux(), http.MethodGet, "/readyz", "")
	if w.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200 (body %s)", w.Code, w.Body.String())
	}
	body := decodeReady(t, w.Body.String())
	if !body.Ready {
		t.Fatalf("ready = false, want true")
	}
	if ok, found := body.depOK("storage"); !found || !ok {
		t.Fatalf("storage dependency = (%v, found=%v), want ok", ok, found)
	}
}

// Meilisearch 不可达 → 503，且故障项点名 meilisearch。
func TestReadyNotReadyWhenMeiliUnreachable(t *testing.T) {
	s := &Server{
		store:   nil,
		meili:   search.NewClient("http://127.0.0.1:1", "k"), // 无监听端口：连接被拒
		content: newTestStorage(t, 0),
	}

	w := doReq(t, s.NewServeMux(), http.MethodGet, "/readyz", "")
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("status = %d, want 503 (body %s)", w.Code, w.Body.String())
	}
	body := decodeReady(t, w.Body.String())
	if body.Ready {
		t.Fatalf("ready = true, want false")
	}
	if ok, found := body.depOK("meilisearch"); !found || ok {
		t.Fatalf("meilisearch dependency = (%v, found=%v), want not ok", ok, found)
	}
}

// 存储根不可用（.tmp 被删）→ 503。
func TestReadyNotReadyWhenStorageBroken(t *testing.T) {
	content := newTestStorage(t, 0)
	if err := os.RemoveAll(filepath.Join(content.Root(), ".tmp")); err != nil {
		t.Fatalf("remove tmp dir: %v", err)
	}
	s := &Server{store: nil, meili: nil, content: content}

	w := doReq(t, s.NewServeMux(), http.MethodGet, "/readyz", "")
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("status = %d, want 503 (body %s)", w.Code, w.Body.String())
	}
	if ok, found := decodeReady(t, w.Body.String()).depOK("storage"); !found || ok {
		t.Fatalf("storage dependency = (%v, found=%v), want not ok", ok, found)
	}
}

// 存活探针不检查依赖：存储坏掉时 /healthz 仍必须 200（A2-02 的语义分离）。
func TestHealthzStaysAliveWhenStorageBroken(t *testing.T) {
	content := newTestStorage(t, 0)
	if err := os.RemoveAll(filepath.Join(content.Root(), ".tmp")); err != nil {
		t.Fatalf("remove tmp dir: %v", err)
	}
	s := &Server{store: nil, meili: search.NewClient("http://127.0.0.1:1", "k"), content: content}

	mux := s.NewServeMux()
	if w := doReq(t, mux, http.MethodGet, "/healthz", ""); w.Code != http.StatusOK {
		t.Fatalf("healthz status = %d, want 200", w.Code)
	}
	if w := doReq(t, mux, http.MethodGet, "/readyz", ""); w.Code != http.StatusServiceUnavailable {
		t.Fatalf("readyz status = %d, want 503", w.Code)
	}
}

// F-I6（P2 独立复审）：PG 不可达 → 503 且 postgres 项点名失败。
func TestReadyNotReadyWhenPostgresUnreachable(t *testing.T) {
	// 127.0.0.1:1 无监听：连接被拒（快速失败，不触发超时等待）。
	db, err := sql.Open("postgres", "postgres://partisync:x@127.0.0.1:1/db?sslmode=disable&connect_timeout=2")
	if err != nil {
		t.Fatalf("sql.Open: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	s := &Server{store: store.NewStoreFromDB(db), meili: nil, content: newTestStorage(t, 0)}

	w := doReq(t, s.NewServeMux(), http.MethodGet, "/readyz", "")
	if w.Code != http.StatusServiceUnavailable {
		t.Fatalf("status = %d, want 503 (body %s)", w.Code, w.Body.String())
	}
	body := decodeReady(t, w.Body.String())
	if body.Ready {
		t.Fatal("ready = true, want false")
	}
	if ok, found := body.depOK("postgres"); !found || ok {
		t.Fatalf("postgres dependency = (%v, found=%v), want not ok", ok, found)
	}
}

// F-I7（P2 独立复审）：/readyz 的 error 字段必须是二元口径——
// 不得回显驱动错误串（含 DSN 主机、容器名、DNS 地址等内部拓扑）。
func TestReadyErrorFieldCarriesNoTopology(t *testing.T) {
	db, err := sql.Open("postgres", "postgres://partisync:x@127.0.0.1:1/db?sslmode=disable&connect_timeout=2")
	if err != nil {
		t.Fatalf("sql.Open: %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	s := &Server{store: store.NewStoreFromDB(db), meili: nil, content: newTestStorage(t, 0)}

	w := doReq(t, s.NewServeMux(), http.MethodGet, "/readyz", "")
	body := decodeReady(t, w.Body.String())
	for _, d := range body.Dependencies {
		if d.OK && d.Err != "" {
			t.Errorf("dependency %s: ok=true but error=%q", d.Name, d.Err)
		}
		if !d.OK {
			if d.Err != "unreachable" && d.Err != "unavailable" {
				t.Errorf("dependency %s: error = %q, want binary marker (unreachable/unavailable)", d.Name, d.Err)
			}
			for _, leak := range []string{"127.0.0", "postgres:", "5432", "dial", "DSN", "password"} {
				if strings.Contains(d.Err, leak) {
					t.Errorf("dependency %s: error %q leaks topology (contains %q)", d.Name, d.Err, leak)
				}
			}
		}
	}
}
