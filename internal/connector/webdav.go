// Package connector 实现外部数据源的连接与扫描能力。
// 当前实现：纯标准库 WebDAV 客户端（net/http + encoding/xml），
// 无需第三方依赖，适合网络受限环境。
package connector

import (
	"bytes"
	"context"
	"encoding/xml"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path"
	"strings"
	"time"
)

// WebDAVConfig WebDAV 连接配置。
type WebDAVConfig struct {
	URL        string // e.g. "https://nas.example.com/webdav/"
	Username   string
	Password   string
	RemotePath string // e.g. "/documents"
	Timeout    time.Duration
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

// WebDAVClient WebDAV 客户端，使用标准库实现。
type WebDAVClient struct {
	config   WebDAVConfig
	client   *http.Client
	baseURL  *url.URL
}

// propfindResponse WebDAV PROPFIND 响应。
type propfindResponse struct {
	XMLName xml.Name `xml:"DAV:response"`
	Href   string   `xml:"href"`
	Stat   stat     `xml:"propstat>prop"`
}

type stat struct {
	GetContentLength int64  `xml:"getcontentlength"`
	GetContentType   string `xml:"getcontenttype"`
	GetLastModified  string `xml:"getlastmodified"`
	DisplayName      string `xml:"displayname"`
	IsCollection     string `xml:"iscollection"`
}

// multistatus WebDAV 多状态响应。
type multistatus struct {
	XMLName   xml.Name         `xml:"DAV:multistatus"`
	Responses []propfindResponse `xml:"response"`
}

// propfindRequest PROPFIND 请求体。
const propfindBody = `<?xml version="1.0" encoding="utf-8"?>
<propfind xmlns="DAV:">
  <prop>
    <getcontentlength/>
    <getcontenttype/>
    <getlastmodified/>
    <displayname/>
    <iscollection/>
  </prop>
</propfind>`

// NewWebDAVClient 创建 WebDAV 客户端。
func NewWebDAVClient(cfg WebDAVConfig) (*WebDAVClient, error) {
	if cfg.Timeout == 0 {
		cfg.Timeout = DefaultTimeout
	}
	base, err := url.Parse(cfg.URL)
	if err != nil {
		return nil, fmt.Errorf("parse webdav url: %w", err)
	}
	// 确保 base path 以 / 结尾
	base.Path = strings.TrimSuffix(base.Path, "/") + "/"

	return &WebDAVClient{
		config:  cfg,
		client: &http.Client{
			Timeout: cfg.Timeout,
		},
		baseURL: base,
	}, nil
}

// listFiles 列出 RemotePath 下的所有文件（递归 depth=infinity）。
// 返回文件路径（相对于 RemotePath）列表和错误。
func (c *WebDAVClient) ListFiles(ctx context.Context) ([]FileInfo, error) {
	targetPath := strings.TrimSuffix(c.config.RemotePath, "/")
	if targetPath == "" {
		targetPath = "/"
	}

	req, err := http.NewRequestWithContext(ctx, "PROPFIND", c.fullURL(targetPath), strings.NewReader(propfindBody))
	if err != nil {
		return nil, fmt.Errorf("create propfind request: %w", err)
	}
	c.setAuth(req)
	req.Header.Set("Content-Type", "application/xml")
	req.Header.Set("Depth", "infinity")

	resp, err := c.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("propfind: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusMultiStatus {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("propfind status %d: %s", resp.StatusCode, string(body))
	}

	var ms multistatus
	if err := xml.NewDecoder(resp.Body).Decode(&ms); err != nil {
		return nil, fmt.Errorf("decode multistatus: %w", err)
	}

	var files []FileInfo
	remoteBase := path.Clean(c.config.RemotePath)
	if !strings.HasPrefix(remoteBase, "/") {
		remoteBase = "/" + remoteBase
	}
	remoteBase = strings.TrimSuffix(remoteBase, "/") + "/"

	for _, r := range ms.Responses {
		// 跳过目录（iscollection=1）
		if r.Stat.IsCollection == "1" || r.Stat.IsCollection == "t" {
			continue
		}
		// 解码 href
		href, err := url.PathUnescape(r.Href)
		if err != nil {
			continue
		}
		// 计算相对路径
		rel, err := c.relativePath(href)
		if err != nil || rel == "" {
			continue
		}
		files = append(files, FileInfo{
			Path:     rel,
			Name:     path.Base(href),
			Size:     r.Stat.GetContentLength,
			MimeType: r.Stat.GetContentType,
		})
	}
	return files, nil
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

	// 写到临时文件
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

// fullURL 拼接完整 URL。
func (c *WebDAVClient) fullURL(p string) string {
	u := *c.baseURL
	u.Path = path.Join(u.Path, p)
	return u.String()
}

// relativePath 计算 href 相对于 RemotePath 的路径。
func (c *WebDAVClient) relativePath(href string) (string, error) {
	base := c.baseURL.Path
	if !strings.HasSuffix(base, "/") {
		base += "/"
	}
	// href 可能以 /dav/ 或类似路径开头
	remoteBase := strings.TrimSuffix(c.config.RemotePath, "/")
	if !strings.HasPrefix(remoteBase, "/") {
		remoteBase = "/" + remoteBase
	}

	// 尝试直接去除 base
	if strings.HasPrefix(href, base) {
		rel := strings.TrimPrefix(href, base)
		rel = strings.TrimPrefix(rel, remoteBase)
		rel = strings.TrimPrefix(rel, "/")
		return rel, nil
	}
	// href 是绝对路径，尝试直接裁剪 RemotePath
	if strings.HasPrefix(href, remoteBase) {
		return strings.TrimPrefix(href, remoteBase), nil
	}
	return "", fmt.Errorf("href %q not under remote path %q", href, c.config.RemotePath)
}

// setAuth 设置 Basic Auth。
func (c *WebDAVClient) setAuth(req *http.Request) {
	if c.config.Username != "" {
		req.SetBasicAuth(c.config.Username, c.config.Password)
	}
}

// Probe 连接探针：尝试 PROPFIND 根目录，返回是否可达。
func (c *WebDAVClient) Probe(ctx context.Context) error {
	req, err := http.NewRequestWithContext(ctx, "PROPFIND", c.fullURL("/"), strings.NewReader(propfindBody))
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
	if resp.StatusCode >= 400 {
		return fmt.Errorf("probe status %d", resp.StatusCode)
	}
	return nil
}

// mimeTypeFromName 根据文件扩展名推断 MIME 类型。
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
		rt := ResourceType(f.MimeType)
		if typeSet[rt] {
			keep = append(keep, f)
		}
	}
	return keep
}

// ResourceType 从 MIME 类型推断 ResourceType。
func ResourceType(mime string) string {
	if strings.HasPrefix(mime, "image/") {
		return "image"
	}
	if strings.HasPrefix(mime, "video/") {
		return "video"
	}
	if strings.HasPrefix(mime, "audio/") {
		return "audio"
	}
	if strings.HasPrefix(mime, "text/") || mime == "application/pdf" || mime == "application/json" {
		return "document"
	}
	return "document"
}

// parseGetContentType 解析 Content-Type header，提取 MIME 类型。
func parseGetContentType(ct string) string {
	ct = strings.TrimSpace(ct)
	if idx := strings.Index(ct, ";"); idx != -1 {
		ct = strings.TrimSpace(ct[:idx])
	}
	return strings.ToLower(ct)
}

// —— 以下是 io.Copy 需要的 reader wrapper，避免导入 io ——

// readCloser 组合 io.Reader 和 io.Closer。
type readCloser struct {
	io.Reader
	io.Closer
}

// —— WebDAVError 错误类型 ——

// WebDAVError WebDAV 操作错误。
type WebDAVError struct {
	Op  string
	URL string
	Err error
}

func (e *WebDAVError) Error() string {
	return fmt.Sprintf("webdav %s %s: %v", e.Op, e.URL, e.Err)
}

func (e *WebDAVError) Unwrap() error { return e.Err }

// parsePropfindHref 从 propfind response 中解析 href，处理 XML 编码。
func parsePropfindHref(href string) string {
	// href 可能被 XML 实体编码
	href = strings.ReplaceAll(href, "&amp;", "&")
	href = strings.ReplaceAll(href, "&lt;", "<")
	href = strings.ReplaceAll(href, "&gt;", ">")
	href = strings.ReplaceAll(href, "&quot;", `"`)
	return href
}

// propfindRequest 创建指定深度的 PROPFIND 请求。
func propfindRequest(depth string) *bytes.Buffer {
	return bytes.NewBufferString(propfindBody)
}
