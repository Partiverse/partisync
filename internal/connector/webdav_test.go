package connector

import (
	"context"
	"fmt"
	"net/http"
	"net/http/httptest"
	"strings"
	"sync/atomic"
	"testing"
	"time"
)

// TestWebDAVDirectoryDetectionVariants 验证 4 种常见 WebDAV 目录标记变体均能被正确识别为目录：
// 1. 标准 <resourcetype><collection/></resourcetype>
// 2. 带命名空间前缀 <D:collection xmlns:D="DAV:"/>
// 3. Microsoft 扩展 <iscollection>1</iscollection>
// 4. 普通文件不应被误判为目录
func TestWebDAVDirectoryDetectionVariants(t *testing.T) {
	xmlResp := `<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/webdav/</D:href>
    <D:propstat>
      <D:prop>
        <D:resourcetype><D:collection/></D:resourcetype>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/webdav/%E6%96%87%E6%A1%A3%E5%BA%93/</D:href>
    <D:propstat>
      <D:prop>
        <D:resourcetype><D:collection xmlns:D="DAV:"/></D:resourcetype>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/webdav/ms-folder/</D:href>
    <D:propstat>
      <D:prop>
        <D:iscollection>1</D:iscollection>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/webdav/sample%20image.png</D:href>
    <D:propstat>
      <D:prop>
        <D:getcontentlength>12345</D:getcontentlength>
        <D:getcontenttype>image/png</D:getcontenttype>
        <D:resourcetype></D:resourcetype>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>`

	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != "PROPFIND" {
			t.Fatalf("unexpected method: %s", r.Method)
		}
		w.Header().Set("Content-Type", "application/xml; charset=utf-8")
		w.WriteHeader(http.StatusMultiStatus)
		_, _ = w.Write([]byte(xmlResp))
	}))
	defer ts.Close()

	client, err := NewWebDAVClient(WebDAVConfig{
		URL:        ts.URL + "/webdav",
		RemotePath: "/",
		Timeout:    5 * time.Second,
	})
	if err != nil {
		t.Fatalf("NewWebDAVClient: %v", err)
	}

	entries, err := client.listAllAtDepth(context.Background(), "/", "1")
	if err != nil {
		t.Fatalf("listAllAtDepth: %v", err)
	}

	dirHrefs := make(map[string]bool)
	fileHrefs := make(map[string]bool)
	for _, e := range entries {
		if e.IsDir {
			dirHrefs[e.Href] = true
		} else {
			fileHrefs[e.Href] = true
		}
	}

	// 验证已正确解码中文目录为 /webdav/文档库/
	if !dirHrefs["/webdav/文档库/"] {
		t.Errorf("expected /webdav/文档库/ to be detected as directory, got dirs=%v", dirHrefs)
	}
	if !dirHrefs["/webdav/ms-folder/"] {
		t.Errorf("expected ms-folder to be detected as directory, got dirs=%v", dirHrefs)
	}
	if !fileHrefs["/webdav/sample image.png"] {
		t.Errorf("expected sample image.png to be detected as file, got files=%v", fileHrefs)
	}
}

// TestWebDAVRelativePathComputation 验证深层子目录下的相对路径不会把整个绝对路径或父路径截丢。
func TestWebDAVRelativePathComputation(t *testing.T) {
	client, err := NewWebDAVClient(WebDAVConfig{
		URL:        "https://example.com/webdav",
		RemotePath: "/",
	})
	if err != nil {
		t.Fatalf("NewWebDAVClient: %v", err)
	}

	cases := []struct {
		name     string
		href     string
		expected string
	}{
		{"root file", "/webdav/readme.txt", "readme.txt"},
		{"nested file", "/webdav/movies/2026/action.mp4", "movies/2026/action.mp4"},
		{"with full scheme", "https://example.com/webdav/docs/guide.pdf", "docs/guide.pdf"},
		{"url encoded with spaces", "/webdav/books/Python%20Cookbook.pdf", "books/Python%20Cookbook.pdf"},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			got := client.relativePath(tc.href, "/")
			if got != tc.expected {
				t.Fatalf("relativePath(%q) = %q; want %q", tc.href, got, tc.expected)
			}
		})
	}
}

// TestWebDAVPropfindRetryOn404Or500 验证网盘限流或瞬时 404/503 会走指数退避重试并在成功后返回内容。
func TestWebDAVPropfindRetryOn404Or500(t *testing.T) {
	var attempts int32
	xmlSuccess := `<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/webdav/</D:href>
    <D:propstat>
      <D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
  <D:response>
    <D:href>/webdav/success.txt</D:href>
    <D:propstat>
      <D:prop>
        <D:getcontentlength>42</D:getcontentlength>
        <D:resourcetype></D:resourcetype>
      </D:prop>
      <D:status>HTTP/1.1 200 OK</D:status>
    </D:propstat>
  </D:response>
</D:multistatus>`

	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		att := atomic.AddInt32(&attempts, 1)
		if att < 3 {
			// 前两次模拟 123pan 限流瞬时返回 404
			http.Error(w, "Not Found", http.StatusNotFound)
			return
		}
		w.Header().Set("Content-Type", "application/xml")
		w.WriteHeader(http.StatusMultiStatus)
		_, _ = w.Write([]byte(xmlSuccess))
	}))
	defer ts.Close()

	client, err := NewWebDAVClient(WebDAVConfig{
		URL:        ts.URL + "/webdav",
		RemotePath: "/",
		Timeout:    5 * time.Second,
	})
	if err != nil {
		t.Fatalf("NewWebDAVClient: %v", err)
	}

	files, err := client.listAtDepth(context.Background(), "/", "1")
	if err != nil {
		t.Fatalf("listAtDepth failed after retries: %v", err)
	}
	if attempts != 3 {
		t.Fatalf("expected 3 attempts, got %d", attempts)
	}
	if len(files) != 1 || files[0].Name != "success.txt" {
		t.Fatalf("expected 1 file success.txt, got %+v", files)
	}
}

// TestWebDAVRecursiveTraversalMock 验证完整的递归扫描逻辑在多层嵌套树下的正确性与并发安全性。
func TestWebDAVRecursiveTraversalMock(t *testing.T) {
	mockTree := map[string]string{
		"/webdav/": `<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/webdav/</D:href>
    <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/webdav/root_file.txt</D:href>
    <D:propstat><D:prop><D:getcontentlength>100</D:getcontentlength><D:resourcetype></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/webdav/sub1/</D:href>
    <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/webdav/sub2/</D:href>
    <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>`,
		"/webdav/sub1/": `<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/webdav/sub1/</D:href>
    <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/webdav/sub1/nested1.jpg</D:href>
    <D:propstat><D:prop><D:getcontentlength>200</D:getcontentlength><D:resourcetype></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>`,
		"/webdav/sub2/": `<?xml version="1.0" encoding="utf-8"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/webdav/sub2/</D:href>
    <D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
  <D:response>
    <D:href>/webdav/sub2/nested2.png</D:href>
    <D:propstat><D:prop><D:getcontentlength>300</D:getcontentlength><D:resourcetype></D:resourcetype></D:prop><D:status>HTTP/1.1 200 OK</D:status></D:propstat>
  </D:response>
</D:multistatus>`,
	}

	ts := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		normPath := r.URL.Path
		if !strings.HasSuffix(normPath, "/") {
			normPath += "/"
		}
		body, ok := mockTree[normPath]
		if !ok {
			http.Error(w, fmt.Sprintf("not found %s", normPath), http.StatusNotFound)
			return
		}
		w.Header().Set("Content-Type", "application/xml")
		w.WriteHeader(http.StatusMultiStatus)
		_, _ = w.Write([]byte(body))
	}))
	defer ts.Close()

	client, err := NewWebDAVClient(WebDAVConfig{
		URL:        ts.URL + "/webdav",
		RemotePath: "/",
		Timeout:    5 * time.Second,
	})
	if err != nil {
		t.Fatalf("NewWebDAVClient: %v", err)
	}

	files, err := client.ListFiles(context.Background())
	if err != nil {
		t.Fatalf("ListFiles: %v", err)
	}

	if len(files) != 3 {
		t.Fatalf("expected 3 files, got %d: %+v", len(files), files)
	}

	paths := make(map[string]bool)
	for _, f := range files {
		paths[f.Path] = true
	}
	expected := []string{"root_file.txt", "sub1/nested1.jpg", "sub2/nested2.png"}
	for _, p := range expected {
		if !paths[p] {
			t.Errorf("missing expected path %q in %+v", p, paths)
		}
	}
}
