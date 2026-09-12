// cmd/bench/main.go
// partisync 1M asset benchmark: batch-index to MeiliSearch then measure search p95.
//
// Usage: go run ./cmd/bench --meili-url=http://localhost:7700
//                             --meili-key=partisync_master_key_for_dev_only_32_chars_long
//                             --count=1000000
//                             --concurrency=50
//                             --requests=5000
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"math"
	"net/http"
	"os"
	"runtime"
	"sort"
	"strings"
	"sync"
	"sync/atomic"
	"time"
)

var (
	flagMeiliURL    = flag.String("meili-url", "http://localhost:7700", "MeiliSearch base URL")
	flagMeiliKey    = flag.String("meili-key", "partisync_master_key_for_dev_only_32_chars_long", "MeiliSearch master key")
	flagCount       = flag.Int("count", 1_000_000, "Target total number of documents in index")
	flagStartIndex  = flag.Int("start-index", 0, "Start index for document generation (allows appending)")
	flagConcurrency = flag.Int("concurrency", 50, "Number of concurrent search clients")
	flagRequests    = flag.Int("requests", 5000, "Total number of search requests")
	flagBatchSize   = flag.Int("batch-size", 5000, "Documents per batch")
	flagOnlySearch  = flag.Bool("only-search", false, "Skip indexing and run search benchmark only")
)

type config struct {
	meiliURL    string
	meiliKey    string
	count       int
	startIndex  int
	concurrency int
	requests    int
	batchSize   int
	onlySearch  bool
	searchTerms []string
}

func main() {
	runtime.GOMAXPROCS(runtime.NumCPU())
	flag.Parse()
	cfg := config{
		meiliURL:    *flagMeiliURL,
		meiliKey:    *flagMeiliKey,
		count:       *flagCount,
		startIndex:  *flagStartIndex,
		concurrency: *flagConcurrency,
		requests:    *flagRequests,
		batchSize:   *flagBatchSize,
		onlySearch:  *flagOnlySearch,
		searchTerms: []string{
			// High frequency terms
			"photo", "image", "document", "report", "landscape",
			// Mid frequency terms
			"portrait", "chart", "diagram", "receipt", "invoice",
			"meeting", "contract", "certificate", "blueprint", "license",
			// Long-tail / specific combinations / domain terms
			"aerial", "drone", "daylight", "outdoor", "conference",
			"seminar", "workshop", "snapshot", "scan", "table",
			"natural lighting", "statistical data", "high resolution",
		},
	}

	log("=== partisync benchmark ===")
	log("Target Docs : %d", cfg.count)
	log("Start Index : %d", cfg.startIndex)
	log("Batch size  : %d", cfg.batchSize)
	log("Concurrency : %d clients", cfg.concurrency)
	log("Requests    : %d total", cfg.requests)
	log("Only Search : %v", cfg.onlySearch)
	log("")

	if !cfg.onlySearch {
		// --- Phase 1: Batch index (stream — no big slice in memory) ---
		log("--- Phase 1: Batch indexing to MeiliSearch ---")
		totalToIndex := cfg.count - cfg.startIndex
		if totalToIndex < 0 {
			totalToIndex = 0
		}
		batches := (totalToIndex + cfg.batchSize - 1) / cfg.batchSize
		log("Total batches to index: %d (from doc %d to %d)", batches, cfg.startIndex, cfg.count)

		for batchNum := 0; batchNum < batches; batchNum++ {
			start := cfg.startIndex + batchNum*cfg.batchSize
			end := start + cfg.batchSize
			if end > cfg.count {
				end = cfg.count
			}
			taskUID, err := indexBatch(cfg, start, end)
			if err != nil {
				fmt.Fprintf(os.Stderr, "FATAL: batch %d (docs %d-%d) failed: %v\n", batchNum+1, start, end, err)
				os.Exit(1)
			}
			log("Batch %d/%d (docs %d-%d): task UID %d", batchNum+1, batches, start, end, taskUID)
			if err := waitForTask(cfg, taskUID, 5*time.Minute); err != nil {
				fmt.Fprintf(os.Stderr, "FATAL: wait for task %d: %v\n", taskUID, err)
				os.Exit(1)
			}
		}
		log("Indexing complete")
	} else {
		log("--- Phase 1: Skipping index (--only-search specified) ---")
	}

	stats := getIndexStats(cfg)
	log("Documents in index: %d", stats.NumberOfDocuments)
	log("")
	log("--- Phase 2: Search benchmark (%d clients, %d requests) ---", cfg.concurrency, cfg.requests)

	var latenciesMS []float64
	var errors int64
	var mu sync.Mutex
	var wg sync.WaitGroup
	rate := make(chan struct{}, cfg.concurrency)

	start := time.Now()
	for i := 0; i < cfg.requests; i++ {
		rate <- struct{}{}
		wg.Add(1)
		go func(reqNum int) {
			defer wg.Done()
			term := cfg.searchTerms[reqNum%len(cfg.searchTerms)]
			t0 := time.Now()
			_, err := search(cfg, term)
			lat := float64(time.Since(t0).Microseconds()) / 1000.0
			if err != nil {
				atomic.AddInt64(&errors, 1)
			} else {
				mu.Lock()
				latenciesMS = append(latenciesMS, lat)
				mu.Unlock()
			}
			<-rate
		}(i)
	}
	wg.Wait()
	elapsed := time.Since(start)

	if len(latenciesMS) == 0 {
		fmt.Fprintf(os.Stderr, "FATAL: all requests failed\n")
		os.Exit(1)
	}

	sort.Float64s(latenciesMS)
	n := len(latenciesMS)
	p50 := percentile(latenciesMS, 0.50)
	p90 := percentile(latenciesMS, 0.90)
	p95 := percentile(latenciesMS, 0.95)
	p99 := percentile(latenciesMS, 0.99)
	min := latenciesMS[0]
	max := latenciesMS[n-1]
	avg := mean(latenciesMS)
	rps := float64(n) / elapsed.Seconds()

	log("")
	log("=== Results (%d ok / %d errors) ===", n, errors)
	log("Duration : %.2fs", elapsed.Seconds())
	log("RPS      : %.2f req/s", rps)
	log("p50      : %.2f ms", p50)
	log("p90      : %.2f ms", p90)
	log("p95      : %.2f ms", p95)
	log("p99      : %.2f ms", p99)
	log("min      : %.2f ms", min)
	log("max      : %.2f ms", max)
	log("avg      : %.2f ms", avg)
	log("")
	printHistogram(latenciesMS)
	log("")
	if p95 < 100.0 {
		log("PASS: p95 (%.2f ms) < 100 ms", p95)
	} else {
		log("FAIL: p95 (%.2f ms) >= 100 ms", p95)
		os.Exit(1)
	}
}

// --- MeiliSearch helpers ---

func doReq(cfg config, method, path string, body io.Reader, contentType string) ([]byte, int, error) {
	req, err := http.NewRequest(method, cfg.meiliURL+path, body)
	if err != nil {
		return nil, 0, err
	}
	req.Header.Set("Authorization", "Bearer "+cfg.meiliKey)
	if contentType != "" {
		req.Header.Set("Content-Type", contentType)
	}
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		return nil, 0, err
	}
	defer resp.Body.Close()
	var buf bytes.Buffer
	buf.ReadFrom(resp.Body)
	return buf.Bytes(), resp.StatusCode, nil
}

type indexStats struct {
	NumberOfDocuments int64 `json:"numberOfDocuments"`
}

func getIndexStats(cfg config) indexStats {
	var r indexStats
	b, _, _ := doReq(cfg, "GET", "/indexes/assets/stats", nil, "")
	json.Unmarshal(b, &r)
	return r
}

// indexBatch generates docs start..end-1 and streams JSON directly to MeiliSearch
func indexBatch(cfg config, start, end int) (int64, error) {
	// Build NDJSON stream in a bytes.Buffer
	buf := bytes.NewBuffer(nil)
	enc := json.NewEncoder(buf)
	for i := start; i < end; i++ {
		doc := generateDoc(i)
		// NDJSON: one JSON object per line, no surrounding array
		enc.Encode(doc)
	}

	body := bytes.NewReader(buf.Bytes())
	var result struct{ TaskUID int64 `json:"taskUid"` }
	b, code, err := doReq(cfg, "POST", "/indexes/assets/documents", body, "application/x-ndjson")
	if err != nil {
		return 0, err
	}
	if code != 202 && code != 201 {
		return 0, fmt.Errorf("bulk index status %d: %s", code, string(b))
	}
	json.Unmarshal(b, &result)
	return result.TaskUID, nil
}

func waitForTask(cfg config, uid int64, timeout time.Duration) error {
	deadline := time.Now().Add(timeout)
	for time.Now().Before(deadline) {
		var result struct{ Status string `json:"status"` }
		b, _, err := doReq(cfg, "GET", fmt.Sprintf("/tasks/%d", uid), nil, "")
		if err != nil {
			return err
		}
		json.Unmarshal(b, &result)
		switch result.Status {
		case "succeeded":
			return nil
		case "failed":
			return fmt.Errorf("task failed: %s", string(b))
		}
		time.Sleep(500 * time.Millisecond)
	}
	return fmt.Errorf("timeout waiting for task %d", uid)
}

func search(cfg config, q string) ([]string, error) {
	body := map[string]interface{}{"q": q, "limit": 20}
	b, code, err := func() ([]byte, int, error) {
		var buf bytes.Buffer
		json.NewEncoder(&buf).Encode(body)
		return doReq(cfg, "POST", "/indexes/assets/search", &buf, "application/json")
	}()
	if err != nil {
		return nil, err
	}
	if code != 200 {
		return nil, fmt.Errorf("search status %d: %s", code, string(b))
	}
	var r struct {
		Hits []struct {
			ID string `json:"id"`
		} `json:"hits"`
	}
	json.Unmarshal(b, &r)
	ids := make([]string, len(r.Hits))
	for i, h := range r.Hits {
		ids[i] = h.ID
	}
	return ids, nil
}

// --- Data generation (one doc at a time) ---

var (
	tagPools = [][]string{
		{"landscape", "portrait", "nature", "urban", "abstract"},
		{"document", "report", "invoice", "contract", "certificate"},
		{"chart", "diagram", "graph", "table", "blueprint"},
		{"photo", "screenshot", "scan", "snapshot", "frame"},
		{"meeting", "event", "conference", "workshop", "seminar"},
		{"landscape", "outdoor", "daylight", "aerial", "drone"},
	}
	descTemplates = []string{
		"A %s photograph taken in natural lighting with high resolution.",
		"Professional %s document scan with clear text and formatting.",
		"Detailed %s chart showing statistical data and trends.",
		"High quality %s image captured with professional equipment.",
		"Compressed %s video file with standard encoding settings.",
	}
	adjPool     = []string{"landscape", "portrait", "detailed", "scenic", "professional", "casual"}
	exts        = []string{"jpg", "png", "pdf", "mp4", "gif", "webp"}
	mimes       = []string{"image/jpeg", "image/png", "application/pdf", "video/mp4", "image/gif", "image/webp"}
	restypes    = []string{"image", "document", "video"}
	sha256chars = "0123456789abcdef"
)

func generateDoc(i int) map[string]interface{} {
	m := i % 6
	var rt string
	if m == 0 || m == 1 || m == 4 || m == 5 {
		rt = "image"
	} else if m == 2 {
		rt = "document"
	} else {
		rt = "video"
	}

	pool := tagPools[i%len(tagPools)]
	cnt := 1 + i%3
	tags := make([]string, cnt)
	for j := 0; j < cnt; j++ {
		tags[j] = pool[(i+j)%len(pool)]
	}

	adj := adjPool[i%len(adjPool)]
	tmpl := descTemplates[i%len(descTemplates)]

	return map[string]interface{}{
		"id":            fmt.Sprintf("bench-%09d", i),
		"name":          fmt.Sprintf("asset_%09d.%s", i, exts[m]),
		"path":          fmt.Sprintf("/uploads/bench/asset_%09d.%s", i, exts[m]),
		"sha256":        sha256dummy(i),
		"mime_type":     mimes[m],
		"resource_type": rt,
		"size_bytes":    1024 + (i % 1024),
		"tags":          tags,
		"description":   fmt.Sprintf(tmpl, adj) + fmt.Sprintf(" Resource type: %s.", rt),
		"owner":         fmt.Sprintf("user_%03d", i%200),
		"created_at":    time.Unix(1700000000+int64(i%86400), 0).Format(time.RFC3339),
	}
}

func sha256dummy(seed int) string {
	b := make([]byte, 64)
	for j := 0; j < 64; j++ {
		b[j] = sha256chars[(seed*31+j*17)%16]
	}
	return string(b)
}

// --- Statistics ---

func percentile(sorted []float64, q float64) float64 {
	if len(sorted) == 0 {
		return 0
	}
	idx := int(math.Ceil(q*float64(len(sorted)))) - 1
	if idx < 0 {
		idx = 0
	}
	if idx >= len(sorted) {
		idx = len(sorted) - 1
	}
	return sorted[idx]
}

func mean(vals []float64) float64 {
	if len(vals) == 0 {
		return 0
	}
	var sum float64
	for _, v := range vals {
		sum += v
	}
	return sum / float64(len(vals))
}

func printHistogram(vals []float64) {
	if len(vals) == 0 {
		return
	}
	minV, maxV := vals[0], vals[len(vals)-1]
	buckets := 20
	binWidth := (maxV - minV) / float64(buckets)
	if binWidth <= 0 {
		binWidth = 1
	}
	bins := make([]int, buckets)
	for _, v := range vals {
		bin := int((v - minV) / binWidth)
		if bin >= buckets {
			bin = buckets - 1
		}
		if bin < 0 {
			bin = 0
		}
		bins[bin]++
	}
	maxCount := 0
	for _, c := range bins {
		if c > maxCount {
			maxCount = c
		}
	}
	barMax := 50
	fmt.Println("Latency distribution (ms):")
	for i := 0; i < buckets; i++ {
		lo := minV + float64(i)*binWidth
		hi := minV + float64(i+1)*binWidth
		count := bins[i]
		barLen := 0
		if maxCount > 0 {
			barLen = (count * barMax) / maxCount
		}
		fmt.Printf("  %7.1f - %7.1f ms | %5d | %s\n",
			lo, hi, count, strings.Repeat("█", barLen))
	}
}

func log(format string, args ...interface{}) {
	fmt.Printf(format+"\n", args...)
}
