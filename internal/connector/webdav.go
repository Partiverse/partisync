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
	ContentLength int64  `xml:"getcontentlength"`
	ContentType   string `xml:"getcontenttype"`
	LastModified  string `xml:"getlastmodified"`
	DisplayName   string `xml:"displayname"`
	IsCollection  string `xml:"iscollection"` // Microsoft 扩展: "1"/"t"=目录，"0"/"f"=文件
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
// 策略：优先 Depth:infinity，失败则降级 Depth:1 手动递归。
func (c *WebDAVClient) ListFiles(ctx context.Context) ([]FileInfo, error) {
	targetPath := c.config.RemotePath
	if targetPath == "" {
		targetPath = "/"
	}

	// 先尝试 Depth:infinity
	files, err := c.listAtDepth(ctx, targetPath, "infinity")
	if err == nil {
		return files, nil
	}

	// 降级：Depth:1 + 手动递归
	seenDirs := make(map[string]bool)
	return c.listRecursive(ctx, targetPath, seenDirs)
}

// listAtDepth 发送指定 Depth 的 PROPFIND 请求并解析。
func (c *WebDAVClient) listAtDepth(ctx context.Context, remotePath, depth string) ([]FileInfo, error) {
	req, err := http.NewRequestWithContext(ctx, "PROPFIND", c.fullURL(remotePath), strings.NewReader(propfindBody))
	if err != nil {
		return nil, fmt.Errorf("create propfind request: %w", err)
	}
	c.setAuth(req)
	req.Header.Set("Content-Type", "application/xml")
	req.Header.Set("Depth", depth)

	resp, err := c.client.Do(req)
	if err != nil {
		return nil, fmt.Errorf("propfind: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusMultiStatus {
		body, _ := io.ReadAll(resp.Body)
		return nil, fmt.Errorf("propfind status %d: %s", resp.StatusCode, string(body))
	}

	body, err := io.ReadAll(resp.Body)
	if err != nil {
		return nil, fmt.Errorf("read response body: %w", err)
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

// isDirectoryFromProp 判断是否目录。
// 1. iscollection 字段（Microsoft 扩展）："1"/"t" = 目录
// 2. resourcetype XML 中是否包含 <collection> 子元素（标准 WebDAV）
func (c *WebDAVClient) isDirectoryFromProp(p prop, href string, body []byte) bool {
	switch strings.ToLower(p.IsCollection) {
	case "1", "true", "t":
		return true
	case "0", "false", "f":
		return false
	}

	// iscollection 未知，检测 resourcetype XML 中是否有 <collection> 子元素
	return c.hasCollectionChild(href, body)
}

// hasCollectionChild 在响应体中找到指定 href 的 <resourcetype> 块，
// 检测其中是否包含 <collection> 子元素。
func (c *WebDAVClient) hasCollectionChild(href string, body []byte) bool {
	dec := xml.NewDecoder(bytes.NewReader(body))
	var curHref string
	inResType := false
	depth := 0
	var tokenDepth int

	for {
		token, err := dec.Token()
		if err == io.EOF {
			break
		}
		if err != nil {
			break
		}

		switch tok := token.(type) {
		case xml.StartElement:
			local := strings.ToLower(tok.Name.Local)
			depth++
			if depth == 1 && local == "response" {
				curHref = ""
			}
			if depth == 2 && local == "href" {
				var hv string
				if err := dec.DecodeElement(&hv, &tok); err == nil {
					curHref = c.parseHref(hv)
				}
				depth--
				continue
			}
			if curHref == c.parseHref(href) {
				if local == "resourcetype" {
					inResType = true
					tokenDepth = depth
				}
			}
		case xml.EndElement:
			if inResType && depth == tokenDepth {
				inResType = false
			}
			depth--
		}
	}
	return false
}

// listRecursive 递归列出目录（Depth:1 手动递归模式）。
func (c *WebDAVClient) listRecursive(ctx context.Context, remotePath string, seenDirs map[string]bool) ([]FileInfo, error) {
	if seenDirs[remotePath] {
		return nil, nil
	}
	seenDirs[remotePath] = true

	// 获取当前目录的所有条目（含目录）
	entries, err := c.listAllAtDepth(ctx, remotePath, "1")
	if err != nil {
		return nil, err
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

	for _, dir := range subDirs {
		dirFiles, err := c.listRecursive(ctx, dir, seenDirs)
		if err != nil {
			continue
		}
		allFiles = append(allFiles, dirFiles...)
	}
	return allFiles, nil
}

// listEntry 单个条目（含是否目录的判断）。
type listEntry struct {
	FileInfo
	Href   string
	IsDir  bool
}

// listAllAtDepth 列出指定路径下所有条目（含文件/目录分类）。
func (c *WebDAVClient) listAllAtDepth(ctx context.Context, remotePath, depth string) ([]listEntry, error) {
	req, err := http.NewRequestWithContext(ctx, "PROPFIND", c.fullURL(remotePath), strings.NewReader(propfindBody))
	if err != nil {
		return nil, err
	}
	c.setAuth(req)
	req.Header.Set("Content-Type", "application/xml")
	req.Header.Set("Depth", depth)

	resp, err := c.client.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusMultiStatus {
		return nil, fmt.Errorf("propfind status %d", resp.StatusCode)
	}

	body, _ := io.ReadAll(resp.Body)
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

		isDir := false
		switch strings.ToLower(p.IsCollection) {
		case "1", "true", "t":
			isDir = true
		case "0", "false", "f":
			isDir = false
		default:
			isDir = c.hasCollectionChild(href, body)
		}

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
			Href:  rel,
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

// relativePath 计算 href 相对于 remoteBase 的路径。
func (c *WebDAVClient) relativePath(href, remoteBase string) string {
	// 移除 URL 中的 scheme://host 部分
	if idx := strings.Index(href, "://"); idx != -1 {
		if pathIdx := strings.Index(href[idx+3:], "/"); pathIdx != -1 {
			href = "/" + href[idx+3+pathIdx:]
		}
	}
	rel := strings.TrimPrefix(href, remoteBase)
	rel = strings.TrimPrefix(rel, c.config.RemotePath)
	rel = strings.TrimPrefix(rel, "/")
	return rel
}

// fullURL 拼接完整 URL。
func (c *WebDAVClient) fullURL(p string) string {
	u := *c.baseURL
	if p == "/" {
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
