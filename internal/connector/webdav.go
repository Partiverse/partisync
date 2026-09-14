// Package connector 实现外部数据源的连接与扫描能力。
// 当前实现：纯标准库 WebDAV 客户端（net/http + encoding/xml），
// 无需第三方依赖，适合网络受限环境。
package connector

import (
	"context"
	"encoding/xml"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path"
	"regexp"
	"strings"
	"sync"
	"time"
)

// WebDAVConfig WebDAV 连接配置。
type WebDAVConfig struct {
	URL        string        // e.g. "https://nas.example.com/webdav/"
	Username   string
	Password   string
	RemotePath string        // e.g. "/documents"
	Timeout    time.Duration // 0 表示 DefaultTimeout
}

// DefaultTimeout 默认超时。
const DefaultTimeout = 30 * time.Second

// FileInfo 远程文件信息。
type FileInfo struct {
	Path     string // 相对于 RemotePath 的路径
	Name     string
	Size     int64
	MimeType string
}

// WebDAVClient WebDAV 客户端。
type WebDAVClient struct {
	config  WebDAVConfig
	client  *http.Client
	baseURL *url.URL
}

// propfindRequest PROPFIND 请求体。
// xmlns="DAV:" 是必需的——123pan 等服务器在没有命名空间时返回 404。
const propfindBody = `<?xml version="1.0" encoding="utf-8"?>
<propfind xmlns="DAV:">
  <prop>
    <getcontentlength/>
    <getcontenttype/>
    <getlastmodified/>
    <displayname/>
    <resourcetype/>
    <iscollection/>
  </prop>
</propfind>`

// —— XML 解析结构（无命名空间前缀，兼容所有 WebDAV 服务器）——
// Go 的 encoding/xml 按 Local Name 匹配，xml:"href" 可以匹配
// <d:href>、<DAV:href>、<href> 等任何前缀。

// multistatus PROPFIND 响应最外层。
type multistatus struct {
	Responses []response `xml:"response"`
}

// response 单个资源条目。
type response struct {
	Href      string       `xml:"href"`
	Propstats []propstat   `xml:"propstat"`
}

// propstat 属性状态块（可能有多个：200 OK + 404 Not Found）。
type propstat struct {
	Prop   prop   `xml:"prop"`
	Status string `xml:"status"`
}

// prop 属性集合。
type prop struct {
	ContentLength int64   `xml:"getcontentlength"`
	ContentType   string  `xml:"getcontenttype"`
	LastModified  string  `xml:"getlastmodified"`
	DisplayName   string  `xml:"displayname"`
	IsCollection  string  `xml:"iscollection"`                      // Microsoft 扩展: "1"/"t"=目录，"0"/"f"=文件
	Type          *xml.Name `xml:"resourcetype>collection,omitempty"` // DAV <collection> 元素（目录标志）
}

// NewWebDAVClient 创建 WebDAV 客户端。
func NewWebDAVClient(cfg WebDAVConfig) (*WebDAVClient, error) {
	if cfg.Timeout == 0 {
		cfg.Timeout = DefaultTimeout
	}
	base, err := url.Parse(cfg.URL)
	if err != nil {
		return nil, fmt.Errorf("parse webdav url: %w", err)
	}
	base.Path = strings.TrimSuffix(base.Path, "/") + "/"
	return &WebDAVClient{
		config:  cfg,
		client:  &http.Client{Timeout: cfg.Timeout},
		baseURL: base,
	}, nil
}

// listFiles 列出 RemotePath 下的所有文件（递归）。
// 策略：Depth:1 + 手动递归（goroutine 并发）。
// 不尝试 Depth:infinity，因为大量服务器（包括 123pan）报告成功但实际只返回 Depth:1 等价结果。
func (c *WebDAVClient) ListFiles(ctx context.Context) ([]FileInfo, error) {
	targetPath := c.config.RemotePath
	if targetPath == "" {
		targetPath = "/"
	}

	seenDirs := make(map[string]bool)
	mu := &sync.Mutex{}
	return c.listRecursive(ctx, targetPath, seenDirs, mu)
}

// DirFoundCallback 每扫完一个目录后调用（用于进度报告）。
// 参数：发现的文件数、发现的子目录数、当前目录的绝对路径。
type DirFoundCallback func(filesFound, dirsFound int, dirPath string)

// ListFilesChan 边扫描边通过 channel yield 文件，不等全部完成。
// onDirFound 每完成一个目录的 PROPFIND 时调用（可传 nil）。
// channel 在所有文件扫完时关闭（由 sync.WaitGroup 跟踪，goroutine 正常返回后才关闭，
// 避免 panic 或阻塞导致 channel 永不关闭）。
func (c *WebDAVClient) ListFilesChan(ctx context.Context, onDirFound DirFoundCallback) (<-chan FileInfo, error) {
	targetPath := c.config.RemotePath
	if targetPath == "" {
		targetPath = "/"
	}
	seenDirs := make(map[string]bool)
	mu := &sync.Mutex{}
	out := make(chan FileInfo, 256)
	var wg sync.WaitGroup

	wg.Add(1)
	go func() {
		defer wg.Done()
		c.listRecursiveStreaming(ctx, targetPath, seenDirs, mu, out, onDirFound)
	}()

	// 在独立 goroutine 中等待所有扫描 goroutine 完成后关闭 channel。
	// 这样即producer 被 channel 满阻塞无法返回，wg.Wait() 也会等它们自然结束
	//（semaphore 保证所有 goroutine 最终都会归还 slot 并 return）。
	go func() {
		wg.Wait()
		close(out)
	}()

	return out, nil
}

// listRecursiveStreaming 递归列出目录，结果通过 out channel 实时 yield。
func (c *WebDAVClient) listRecursiveStreaming(ctx context.Context, remotePath string, seenDirs map[string]bool, mu *sync.Mutex, out chan<- FileInfo, onDirFound DirFoundCallback) {
	defer func() {
		if p := recover(); p != nil {
			// 防止任意 panic 杀死 producer goroutine；错误会被静默吞掉，
			// 目录已在 seenDirs 中，不会重复扫描。
		}
	}()

	absPath := c.fullURL(remotePath)
	mu.Lock()
	if seenDirs[absPath] {
		mu.Unlock()
		return
	}
	seenDirs[absPath] = true
	mu.Unlock()

	entries, err := c.listAllAtDepth(ctx, remotePath, "1")
	if err != nil {
		return
	}

	// 保护：123pan 把某些文件（.iso 等）标记为 <collection>（目录）。
	// 对文件路径做 PROPFIND 时服务器返回该文件自身（1个条目）。
	// 仅当条目明确是"文件"（非目录）时才走此分支；目录走正常递归。
	if len(entries) == 1 && !entries[0].IsDir && entries[0].Name == path.Base(remotePath) {
		select {
		case out <- entries[0].FileInfo:
		default:
			// channel 满时跳过（流式回压保护），不阻塞扫描进度
		}
		if onDirFound != nil {
			onDirFound(1, 0, absPath)
		}
		return
	}

	fileCount, dirCount := 0, 0
	var subDirs []string

	for _, e := range entries {
		if e.IsDir {
			subDirs = append(subDirs, e.Href)
			dirCount++
		} else {
			select {
			case out <- e.FileInfo:
			default:
				// channel 满时跳过，不阻塞扫描
			}
			fileCount++
		}
	}

	if onDirFound != nil {
		onDirFound(fileCount, dirCount, absPath)
	}

	if len(subDirs) == 0 {
		return
	}

	// 并发扫描所有子目录（限制最大并发数）
	sem := make(chan struct{}, maxConcurrentDirs)
	var wg sync.WaitGroup

	for _, dir := range subDirs {
		wg.Add(1)
		go func(d string) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()
			c.listRecursiveStreaming(ctx, d, seenDirs, mu, out, onDirFound)
		}(dir)
	}

	wg.Wait()
}

// listAtDepth 发送指定 Depth 的 PROPFIND 请求并解析文件。
func (c *WebDAVClient) listAtDepth(ctx context.Context, remotePath, depth string) ([]FileInfo, error) {
	body, err := c.propfindRaw(ctx, remotePath, depth)
	if err != nil {
		return nil, err
	}
	return c.parseMultistatus(body, remotePath)
}

// parseMultistatus 解析 PROPFIND 响应体。
func (c *WebDAVClient) parseMultistatus(body []byte, remotePath string) ([]FileInfo, error) {
	var ms multistatus
	if err := xml.Unmarshal(body, &ms); err != nil {
		return nil, fmt.Errorf("decode multistatus: %w", err)
	}

	var files []FileInfo
	remoteBase := c.normalizeRemoteBase(remotePath)

	for _, r := range ms.Responses {
		href := c.parseHref(r.Href)
		if href == "" {
			continue
		}

		// 找到第一个 200 OK 的 propstat
		var p prop
		var statusOK bool
		for _, ps := range r.Propstats {
			if strings.Contains(ps.Status, "200") || strings.Contains(ps.Status, "OK") {
				p = ps.Prop
				statusOK = true
				break
			}
		}
		if !statusOK {
			continue
		}

		// 判断目录：优先 iscollection 字段（Microsoft 扩展）
		// 其次检测 resourcetype XML 中是否包含 <collection> 子元素
		isDir := c.isDirectoryFromProp(p, r.Href, body)

		if isDir {
			continue
		}

		rel := c.relativePath(href, remoteBase)
		if rel == "" || rel == "." {
			continue
		}
		rel = strings.TrimPrefix(rel, "/")

		mimeType := p.ContentType
		if mimeType == "" {
			mimeType = MimeTypeFromName(path.Base(href))
		}

		files = append(files, FileInfo{
			Path:     rel,
			Name:     path.Base(href),
			Size:     p.ContentLength,
			MimeType: mimeType,
		})
	}
	return files, nil
}

// isDirectoryFromProp 判断是否目录（rclone 方案）。
// 1. Type 字段：<resourcetype><collection/></resourcetype> 存在即为目录
// 2. iscollection 字段（Microsoft 扩展）："1"/"t" = 目录
// 3. 正则 fallback：扫描 body 中对应 response 块是否含 <D:collection
func (c *WebDAVClient) isDirectoryFromProp(p prop, href string, body []byte) bool {
	// 1. Type 字段（rclone 方案）：<collection> 元素存在即为目录
	if p.Type != nil && p.Type.Space == "DAV:" && p.Type.Local == "collection" {
		return true
	}

	// 2. Microsoft iscollection 扩展
	switch strings.ToLower(p.IsCollection) {
	case "1", "true", "t":
		return true
	case "0", "false", "f":
		return false
	}

	// 3. 正则 fallback
	return c.hasCollectionChild(href, body)
}

// hasCollectionChildInBlock 在 body 中找到包含目标 href 的 response 块，
// 检测其中是否含 <D:collection/>（表示目录）。
// 两侧 href 都先走 parseHref（XML 实体解码 + URL 解码）再比较：
// 服务器返回的 href 通常是 percent-encoded（`/webdav/a%20b/`），
// 若拿原始 href 与解码后的 href 比较，任何含空格/非 ASCII 的目录都会判成文件（扫描截断的根因）。
func (c *WebDAVClient) hasCollectionChildInBlock(body []byte, decodedHref string) bool {
	targetPath := normalizeHrefPath(c.parseHref(decodedHref))
	if targetPath == "" {
		return false
	}

	// 用贪婪匹配每个 <D:response>...</D:response> 块
	// 注意：不用 .+? 非贪婪，因为 response 块可能很大
	responseRe := regexp.MustCompile(`(?i)<D:response[^>]*>(.+?)</D:response>`)
	hrefRe := regexp.MustCompile(`(?i)<D:href[^>]*>([^<]*)</D:href>`)

	// collection 检测：直接匹配 <D:collection ... />（允许有属性，不能跨行）
	// 去掉 [^>]*/> 因为 123pan 返回 <D:collection xmlns:D="DAV:"/> 有换行
	collectionRe := regexp.MustCompile(`(?i)<D:collection\b`)

	for _, respMatch := range responseRe.FindAllSubmatch(body, -1) {
		block := respMatch[0]

		// 提取此 block 内的 href
		hrefMatch := hrefRe.FindSubmatch(block)
		if hrefMatch == nil {
			continue
		}

		// 解码 href，与目标 path 比较（两侧同口径归一化）
		foundHref := normalizeHrefPath(c.parseHref(string(hrefMatch[1])))
		if foundHref != targetPath {
			continue
		}

		// 找到了目标 response 块；检查是否含 <D:collection
		if collectionRe.Match(block) {
			return true
		}
	}
	return false
}

// hasCollectionChild 兼容旧签名的桥接函数。
func (c *WebDAVClient) hasCollectionChild(href string, body []byte) bool {
	return c.hasCollectionChildInBlock(body, href)
}

// propfindRetryAttempts 单次 PROPFIND 的最大尝试次数。
// 123pan 在限流/瞬时故障下会返回 404 或 5xx；没有重试会静默丢掉整棵子树
// （表现为"扫描出的文件数远少于客户端"）。
const propfindRetryAttempts = 3

// propfindRaw 发送 PROPFIND 并返回响应体，对瞬时失败做退避重试。
func (c *WebDAVClient) propfindRaw(ctx context.Context, remotePath, depth string) ([]byte, error) {
	var lastErr error
	for attempt := 1; attempt <= propfindRetryAttempts; attempt++ {
		req, err := http.NewRequestWithContext(ctx, "PROPFIND", c.fullURL(remotePath), strings.NewReader(propfindBody))
		if err != nil {
			return nil, err
		}
		c.setAuth(req)
		req.Header.Set("Content-Type", "application/xml")
		req.Header.Set("Depth", depth)

		resp, err := c.client.Do(req)
		if err != nil {
			lastErr = err
		} else {
			body, readErr := io.ReadAll(resp.Body)
			resp.Body.Close()
			switch {
			case readErr != nil:
				lastErr = readErr
			case resp.StatusCode == http.StatusMultiStatus:
				return body, nil
			case propfindRetryable(resp.StatusCode):
				lastErr = fmt.Errorf("propfind status %d", resp.StatusCode)
			default:
				return nil, fmt.Errorf("propfind status %d: %s", resp.StatusCode, truncateBody(body))
			}
		}

		if attempt < propfindRetryAttempts {
			delay := time.Duration(300*(1<<(attempt-1))) * time.Millisecond
			select {
			case <-ctx.Done():
				return nil, ctx.Err()
			case <-time.After(delay):
			}
		}
	}
	return nil, lastErr
}

// propfindRetryable 判断状态码是否值得重试（限流/瞬时故障）。
func propfindRetryable(status int) bool {
	switch status {
	case http.StatusNotFound, http.StatusTooManyRequests, http.StatusRequestTimeout,
		http.StatusInternalServerError, http.StatusBadGateway, http.StatusServiceUnavailable,
		http.StatusGatewayTimeout:
		return true
	}
	return false
}

// truncateBody 截断错误响应体，避免把整个 HTML 错误页写进日志。
func truncateBody(body []byte) string {
	s := strings.TrimSpace(string(body))
	if len(s) > 200 {
		return s[:200]
	}
	return s
}

// maxConcurrentDirs 最多同时扫描的子目录数量。
const maxConcurrentDirs = 64

// listRecursive 递归列出目录（Depth:1 手动递归，goroutine 并发加速）。
// seenDirs 必须被 mutex 保护。key 必须是绝对路径（fullURL 规范化后）。
func (c *WebDAVClient) listRecursive(ctx context.Context, remotePath string, seenDirs map[string]bool, mu *sync.Mutex) ([]FileInfo, error) {
	// 用绝对路径作为 seenDirs 的 key，避免 /webdav/ vs webdav/ vs /webdav 等变体重复
	absPath := c.fullURL(remotePath)
	mu.Lock()
	if seenDirs[absPath] {
		mu.Unlock()
		return nil, nil
	}
	seenDirs[absPath] = true
	mu.Unlock()

	entries, err := c.listAllAtDepth(ctx, remotePath, "1")
	if err != nil {
		return nil, err
	}

	// 保护：123pan 把某些文件（.iso 等）标记为 <collection>（目录）。
	// 对文件路径做 PROPFIND 时服务器返回该文件自身（1个条目）。
	// 仅当条目明确是"文件"（非目录）时才走此分支；目录走正常递归。
	if len(entries) == 1 && !entries[0].IsDir && entries[0].Name == path.Base(remotePath) {
		return []FileInfo{entries[0].FileInfo}, nil
	}

	var allFiles []FileInfo
	var subDirs []string

	for _, e := range entries {
		if e.IsDir {
			subDirs = append(subDirs, e.Href)
		} else {
			allFiles = append(allFiles, e.FileInfo)
		}
	}

	if len(subDirs) == 0 {
		return allFiles, nil
	}

	// 并发扫描所有子目录（限制最大并发数）
	type dirResult struct {
		path  string
		files []FileInfo
		err   error
	}

	// 使用 semaphore 模式限制并发数
	sem := make(chan struct{}, maxConcurrentDirs)
	resultCh := make(chan dirResult, len(subDirs))

	var wg sync.WaitGroup
	for _, dir := range subDirs {
		wg.Add(1)
		go func(d string) {
			defer wg.Done()
			sem <- struct{}{} // 获取令牌（阻塞直到有空闲槽位）
			defer func() { <-sem }() // 释放令牌
			files, err := c.listRecursive(ctx, d, seenDirs, mu)
			resultCh <- dirResult{path: d, files: files, err: err}
		}(dir)
	}

	// 并发完成时关闭通道
	go func() {
		wg.Wait()
		close(resultCh)
	}()

	for r := range resultCh {
		if r.err != nil {
			fmt.Printf("[webdav] recurse error %s: %v\n", r.path, r.err)
			continue
		}
		allFiles = append(allFiles, r.files...)
	}

	return allFiles, nil
}

// listEntry 单个条目（含是否目录的判断）。
type listEntry struct {
	FileInfo
	Href   string
	IsDir  bool
}

// listAllAtDepth 列出指定路径下所有条目（含文件/目录分类），内部走带重试的 propfindRaw。
func (c *WebDAVClient) listAllAtDepth(ctx context.Context, remotePath, depth string) ([]listEntry, error) {
	body, err := c.propfindRaw(ctx, remotePath, depth)
	if err != nil {
		return nil, err
	}

	var ms multistatus
	if err := xml.Unmarshal(body, &ms); err != nil {
		return nil, err
	}

	remoteBase := c.normalizeRemoteBase(remotePath)
	var entries []listEntry

	for _, r := range ms.Responses {
		href := c.parseHref(r.Href)
		if href == "" {
			continue
		}

		var p prop
		var statusOK bool
		for _, ps := range r.Propstats {
			if strings.Contains(ps.Status, "200") || strings.Contains(ps.Status, "OK") {
				p = ps.Prop
				statusOK = true
				break
			}
		}
		if !statusOK {
			continue
		}

		isDir := c.isDirectoryFromProp(p, r.Href, body)

		rel := c.relativePath(href, remoteBase)
		if rel == "" || rel == "." {
			continue
		}
		rel = strings.TrimPrefix(rel, "/")

		mimeType := p.ContentType
		if mimeType == "" && !isDir {
			mimeType = MimeTypeFromName(path.Base(href))
		}

		entries = append(entries, listEntry{
			FileInfo: FileInfo{
				Path:     rel,
				Name:     path.Base(href),
				Size:     p.ContentLength,
				MimeType: mimeType,
			},
			Href:  href, // 存绝对路径（服务器返回的 /webdav/abc/），避免 fullURL 拼接出双斜杠
			IsDir: isDir,
		})
	}
	return entries, nil
}

// listDirectDirs 列出指定路径下的直接子目录。
func (c *WebDAVClient) listDirectDirs(ctx context.Context, remotePath string) ([]string, error) {
	entries, err := c.listAllAtDepth(ctx, remotePath, "1")
	if err != nil {
		return nil, err
	}
	var dirs []string
	for _, e := range entries {
		if e.IsDir {
			dirs = append(dirs, e.Href)
		}
	}
	return dirs, nil
}

// parseHref 解析 href，处理 URL 编码和 XML 实体编码。
func (c *WebDAVClient) parseHref(href string) string {
	href = xmlEntityDecode(href)
	href, _ = url.PathUnescape(href)
	if idx := strings.IndexAny(href, "?#"); idx != -1 {
		href = href[:idx]
	}
	return strings.TrimSpace(href)
}

// xmlEntityDecode 解码 XML 实体。
func xmlEntityDecode(s string) string {
	s = strings.ReplaceAll(s, "&amp;", "&")
	s = strings.ReplaceAll(s, "&lt;", "<")
	s = strings.ReplaceAll(s, "&gt;", ">")
	s = strings.ReplaceAll(s, "&quot;", `"`)
	s = strings.ReplaceAll(s, "&apos;", `'`)
	s = strings.ReplaceAll(s, "&#x2F;", "/")
	return s
}

// normalizeRemoteBase 返回规范化后的 RemotePath 基路径（以 / 结尾）。
func (c *WebDAVClient) normalizeRemoteBase(remotePath string) string {
	base := strings.TrimSuffix(remotePath, "/")
	if !strings.HasPrefix(base, "/") {
		base = "/" + base
	}
	if !strings.HasSuffix(base, "/") {
		base += "/"
	}
	return base
}

// normalizeHrefPath 只保留 href 的 path 部分（去掉 scheme://host），
// 用于两侧同口径比较与相对路径裁剪。
func normalizeHrefPath(href string) string {
	if idx := strings.Index(href, "://"); idx != -1 {
		rest := href[idx+3:]
		if p := strings.Index(rest, "/"); p != -1 {
			return rest[p:]
		}
		return "/"
	}
	return href
}

// sourceRootPath 数据源在服务器上的根路径（baseURL.Path + RemotePath，以 / 结尾）。
// 条目相对路径必须相对它计算，否则子目录内文件会丢掉父目录前缀。
func (c *WebDAVClient) sourceRootPath() string {
	base := c.baseURL.Path
	if base == "" {
		base = "/"
	}
	rp := c.config.RemotePath
	if rp == "" {
		rp = "/"
	}
	joined := path.Join(base, rp)
	if !strings.HasSuffix(joined, "/") {
		joined += "/"
	}
	return joined
}

// relativePath 计算 href 相对于数据源根的路径。
// 注意：不属于源根（异常 href）时退回按当前目录裁剪，避免产生带 base path 的脏路径。
func (c *WebDAVClient) relativePath(href, remoteBase string) string {
	h := normalizeHrefPath(href)
	root := c.sourceRootPath()
	rel := strings.TrimPrefix(h, root)
	if rel == h {
		rel = strings.TrimPrefix(h, c.normalizeRemoteBase(remoteBase))
	}
	rel = strings.TrimPrefix(rel, "/")
	if rel == "." {
		return ""
	}
	return rel
}

// fullURL 拼接完整 URL。如果 p 已是绝对路径（以 / 开头），
// 则替换 baseURL 的路径部分，避免双 base path。
func (c *WebDAVClient) fullURL(p string) string {
	u := *c.baseURL
	if p == "/" {
		return u.String()
	}
	if strings.HasPrefix(p, "/") {
		u.Path = p
		return u.String()
	}
	u.Path = path.Join(u.Path, p)
	return u.String()
}

// setAuth 设置 Basic Auth。
func (c *WebDAVClient) setAuth(req *http.Request) {
	if c.config.Username != "" {
		req.SetBasicAuth(c.config.Username, c.config.Password)
	}
}

// Probe 连接探针：尝试 PROPFIND RemotePath，返回是否可达。
func (c *WebDAVClient) Probe(ctx context.Context) error {
	remotePath := c.config.RemotePath
	if remotePath == "" {
		remotePath = "/"
	}
	req, err := http.NewRequestWithContext(ctx, "PROPFIND", c.fullURL(remotePath), strings.NewReader(propfindBody))
	if err != nil {
		return err
	}
	c.setAuth(req)
	req.Header.Set("Content-Type", "application/xml")
	req.Header.Set("Depth", "0")

	resp, err := c.client.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	// 接受 207 (MultiStatus) 或 2xx
	if resp.StatusCode >= 400 {
		return fmt.Errorf("probe status %d", resp.StatusCode)
	}
	return nil
}

// DownloadToTemp 下载远程文件到系统临时目录，返回本地路径。
func (c *WebDAVClient) DownloadToTemp(ctx context.Context, relPath string) (string, error) {
	src := path.Join(c.config.RemotePath, relPath)
	req, err := http.NewRequestWithContext(ctx, "GET", c.fullURL(src), nil)
	if err != nil {
		return "", fmt.Errorf("create get request: %w", err)
	}
	c.setAuth(req)

	resp, err := c.client.Do(req)
	if err != nil {
		return "", fmt.Errorf("get %s: %w", relPath, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		return "", fmt.Errorf("get status %d: %s", resp.StatusCode, string(body))
	}

	tmp, err := os.CreateTemp("", "webdav-*")
	if err != nil {
		return "", fmt.Errorf("create temp file: %w", err)
	}
	tmpPath := tmp.Name()
	defer tmp.Close()

	if _, err := io.Copy(tmp, resp.Body); err != nil {
		os.Remove(tmpPath)
		return "", fmt.Errorf("copy to temp: %w", err)
	}
	return tmpPath, nil
}

// MimeTypeFromName 根据文件扩展名推断 MIME 类型。
func MimeTypeFromName(name string) string {
	ext := strings.ToLower(path.Ext(name))
	switch ext {
	case ".png":
		return "image/png"
	case ".jpg", ".jpeg":
		return "image/jpeg"
	case ".gif":
		return "image/gif"
	case ".webp":
		return "image/webp"
	case ".pdf":
		return "application/pdf"
	case ".txt":
		return "text/plain"
	case ".md":
		return "text/markdown"
	case ".csv":
		return "text/csv"
	case ".json":
		return "application/json"
	case ".mp4":
		return "video/mp4"
	case ".mp3":
		return "audio/mpeg"
	case ".wav":
		return "audio/wav"
	default:
		return "application/octet-stream"
	}
}

// ResourceType 从 MIME 类型推断资源类型。
func ResourceType(mime string) string {
	switch {
	case strings.HasPrefix(mime, "image/"):
		return "image"
	case strings.HasPrefix(mime, "video/"):
		return "video"
	case strings.HasPrefix(mime, "audio/"):
		return "audio"
	case strings.HasPrefix(mime, "text/"), mime == "application/pdf", mime == "application/json":
		return "document"
	default:
		return "document"
	}
}

// FilterByTypes 过滤出指定资源类型的文件。
func FilterByTypes(files []FileInfo, types []string) []FileInfo {
	if len(types) == 0 {
		return files
	}
	var keep []FileInfo
	typeSet := make(map[string]bool)
	for _, t := range types {
		typeSet[t] = true
	}
	for _, f := range files {
		if typeSet[ResourceType(f.MimeType)] {
			keep = append(keep, f)
		}
	}
	return keep
}
