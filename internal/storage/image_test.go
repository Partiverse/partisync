package storage

import (
	"testing"
)

// pngMagic 最小 PNG 头（嗅探只认魔数，不要求解码成功）。
var pngMagic = []byte{0x89, 'P', 'N', 'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13}

// MatchedContentType 的相容性判定（A2-04）：
// 伪装成图片的 HTML 必须被拒；文本家族内部差异不得误拒。
func TestMatchedContentType(t *testing.T) {
	cases := []struct {
		name       string
		head       []byte
		declared   string
		wantMatch  bool
		whyComment string
	}{
		{"png@image/png", pngMagic, "image/png", true, "逐字一致"},
		{"jpeg@image/jpeg", []byte{0xff, 0xd8, 0xff, 0xe0, 0, 0}, "image/jpeg", true, "逐字一致"},
		{"gif@image/gif", []byte("GIF89a\x01\x00\x01\x00"), "image/gif", true, "逐字一致"},
		{"pdf@application/pdf", []byte("%PDF-1.7\n"), "application/pdf", true, "逐字一致"},
		{"html@image/png", []byte("<html><script>alert(1)</script>"), "image/png", false, "伪装图片 → 存储型 XSS 面"},
		{"js@image/png", []byte("function f(){return 1}"), "image/png", false, "伪装图片"},
		{"png@application/pdf", pngMagic, "application/pdf", false, "图片声明成 PDF"},
		{"md@text/markdown", []byte("# 标题\n正文"), "text/markdown", true, "文本家族：嗅探为 text/plain"},
		{"csv@text/csv", []byte("a,b\n1,2\n"), "text/csv", true, "文本家族"},
		{"json@application/json", []byte(`{"a":1}`), "application/json", true, "文本家族"},
		{"txt@text/plain", []byte("plain text"), "text/plain", true, "逐字一致"},
		{"md-with-html@text/markdown", []byte("<h1>标题</h1>\n正文"), "text/markdown", true, "以 HTML 片段开头的 Markdown 不得误拒"},
		{"random@image/png", []byte{0x00, 0x01, 0x02, 0x03}, "image/png", true, "嗅探无法判定 → 放行，由 nosniff+CSP 兜底"},
		{"empty@image/png", nil, "image/png", false, "空内容不作为合法预览"},
	}

	for _, tc := range cases {
		if got := MatchedContentType(tc.head, tc.declared); got != tc.wantMatch {
			t.Errorf("%s: MatchedContentType(%q) = %v, want %v (%s)",
				tc.name, tc.declared, got, tc.wantMatch, tc.whyComment)
		}
	}
}

// 参数化的声明（分号后缀）不得影响判定。
func TestMatchedContentTypeIgnoresMimeParams(t *testing.T) {
	if !MatchedContentType([]byte("a,b\n1,2\n"), "text/csv; charset=utf-8") {
		t.Fatalf("declared mime with charset should still match text family")
	}
	if !MatchedContentType(pngMagic, "image/png; charset=binary") {
		t.Fatalf("declared mime with params should still compare by base type")
	}
}

// sniffMime 与 MatchedContentType 必须使用同一套规范化规则（A2-04）。
func TestSniffMimeNormalizesParams(t *testing.T) {
	got, sniffed := sniffMime([]byte("hello world"), "text/markdown")
	if !sniffed || got != "text/plain" {
		t.Fatalf("sniffMime = (%q, %v), want (text/plain, true)", got, sniffed)
	}
	if got != normalizeMime(got) {
		t.Fatalf("sniffMime result %q is not normalized", got)
	}
}
