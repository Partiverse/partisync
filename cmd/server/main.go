// partisync 核心 API 服务入口：读环境变量装配 Store/Meilisearch/HTTP，
// 并在收到 SIGTERM/SIGINT 时优雅停机（关闭 Worker + HTTP Server + Store）。
package main

import (
	"context"
	"log"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strconv"
	"strings"
	"syscall"
	"time"

	"partisync/server/internal/api"
	"partisync/server/internal/search"
	"partisync/server/internal/storage"
	"partisync/server/internal/store"
	"partisync/server/internal/worker"
)

const (
	defaultPGDSN  = "postgres://partisync:partisync_dev_password@localhost:5432/partisync?sslmode=disable"
	defaultMeili  = "http://localhost:7700"
	defaultMeiliK = "partisync_master_key_for_dev_only_32_chars_long"
	defaultAddr   = ":8080"
	defaultStore  = "./data/assets"
	writeTimeout  = 30 * time.Second
	readTimeout   = 15 * time.Second
	idleTimeout   = 60 * time.Second
)

// getenv 返回环境变量，空则回退默认值。
func getenv(key, def string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return def
}

// spaHandler 服务前端构建产物（playground/dist）：静态文件存在则直出，
// 否则回退 index.html（SPA 路由）。WEB_DIST 为空时未注册（纯 API 模式）。
func spaHandler(dist string) http.Handler {
	fs := http.FileServer(http.Dir(dist))
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		p := filepath.Join(dist, filepath.Clean(r.URL.Path))
		if info, err := os.Stat(p); err == nil && !info.IsDir() {
			fs.ServeHTTP(w, r)
			return
		}
		http.ServeFile(w, r, filepath.Join(dist, "index.html"))
	})
}

// uploadLimitFromEnv 读取 MAX_UPLOAD_BYTES；未设置或非法时返回 0（由 storage 使用默认上限）。
func uploadLimitFromEnv() int64 {
	v := strings.TrimSpace(os.Getenv("MAX_UPLOAD_BYTES"))
	if v == "" {
		return 0
	}
	n, err := strconv.ParseInt(v, 10, 64)
	if err != nil || n <= 0 {
		log.Printf("WARN: ignoring invalid MAX_UPLOAD_BYTES=%q, using default", v)
		return 0
	}
	return n
}

func main() {
	log.SetFlags(log.LstdFlags | log.Lmsgprefix)
	log.SetPrefix("[partisync] ")

	dsn := getenv("PG_DSN", defaultPGDSN)
	meiliURL := getenv("MEILI_URL", defaultMeili)
	meiliKey := getenv("MEILI_KEY", defaultMeiliK)
	addr := getenv("ADDR", defaultAddr)

	st, err := store.NewStore(dsn)
	if err != nil {
		log.Fatalf("init postgres store: %v", err)
	}

	// 文件内容存储：STORAGE_ROOT 为落盘根目录，MAX_UPLOAD_BYTES 可覆盖单文件上限。
	content, err := storage.New(getenv("STORAGE_ROOT", defaultStore), uploadLimitFromEnv())
	if err != nil {
		log.Fatalf("init content storage: %v", err)
	}

	meili := search.NewClient(meiliURL, meiliKey)
	if err := meili.EnsureIndex(context.Background()); err != nil {
		log.Printf("WARN: ensure meilisearch index failed (search degraded): %v", err)
	}

	srv := api.NewServer(st, meili, content)
	rootHandler := http.Handler(srv.NewServeMux())
	// 前端静态托管（WEB_DIST 指向 playground/dist 时启用）；/api 路由优先。
	if webDist := getenv("WEB_DIST", ""); webDist != "" {
		mux := http.NewServeMux()
		mux.Handle("/api/", srv.NewServeMux())
		mux.Handle("/healthz", srv.NewServeMux())
		mux.Handle("/", spaHandler(webDist))
		rootHandler = mux
	}
	httpSrv := &http.Server{
		Addr:         addr,
		Handler:      rootHandler,
		ReadTimeout:  readTimeout,
		WriteTimeout: writeTimeout,
		IdleTimeout:  idleTimeout,
	}

	// 创建共享 context 以协调优雅停机
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// 启动后台 Worker（与 HTTP Server 同进程，生命周期由 ctx 控制）
	w := worker.NewWorker(st, worker.NewMockAnnotator(), worker.DefaultConfig)
	go w.Run(ctx)

	// 捕获系统信号以触发优雅停机
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGINT, syscall.SIGTERM)

	go func() {
		<-sigCh
		log.Printf("received shutdown signal, stopping...")
		cancel() // 通知 Worker 停止

		shutdownCtx, done := context.WithTimeout(context.Background(), 15*time.Second)
		defer done()

		if err := httpSrv.Shutdown(shutdownCtx); err != nil {
			log.Printf("http server shutdown: %v", err)
		}
		if err := st.Close(); err != nil {
			log.Printf("store close: %v", err)
		}
	}()

	log.Printf("listening on %s (meili=%s)", addr, meiliURL)
	if err := httpSrv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatalf("listen: %v", err)
	}
	log.Printf("server stopped")
}
