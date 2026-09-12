package storage

import (
	"bytes"
	"image"
	_ "image/gif"  // 注册 GIF 解码器
	_ "image/jpeg" // 注册 JPEG 解码器
	_ "image/png"  // 注册 PNG 解码器
	"net/http"
	"strings"
)

// imageSize 从文件前缀解析图片宽高；非图片或前缀信息不足时返回 (0, 0)。
// 仅解析配置头，不解码整张图片，因此对大图也廉价。
func imageSize(head []byte) (int, int) {
	if len(head) == 0 {
		return 0, 0
	}
	cfg, _, err := image.DecodeConfig(bytes.NewReader(head))
	if err != nil {
		return 0, 0
	}
	return cfg.Width, cfg.Height
}

// MatchedContentType 校验文件前缀内容与声明的 MIME 是否相容（A2-04）。
//
// 用途：预览端点按 DB 中的 mime 内联返回文件；若攻击者把 HTML/JS 伪装成 image/png
// 上传，再经预览渲染即成存储型 XSS，故内联返回前必须先核对内容与声明。
//
// 判定规则（宁可放行交由响应头兜底的"无法判定"，也不因家族内差异误拒合法文件）：
//  1. 嗅探无法判定（application/octet-stream）→ 放行，由 nosniff + CSP sandbox 兜底；
//  2. 嗅探结果与声明逐字一致 → 放行；
//  3. 声明与嗅探结果同属文本家族 → 放行：http.DetectContentType 只能区分
//     text/plain、text/html、text/xml，无法区分 text/markdown、text/csv、
//     application/json 等子类型（一律嗅探为 text/plain），逐字比较会把
//     .md/.csv/.json、甚至以 HTML 片段开头的 Markdown 误拒为 415；
//  4. 其余（图片/PDF 位置放了 HTML 或二进制）→ 不一致。
func MatchedContentType(head []byte, declared string) bool {
	if len(head) == 0 {
		return false
	}
	detected := normalizeMime(http.DetectContentType(head))
	declared = normalizeMime(declared)
	switch {
	case detected == "" || detected == "application/octet-stream":
		return true
	case strings.EqualFold(detected, declared):
		return true
	case isTextualMime(declared) && isTextualMime(detected):
		return true
	default:
		return false
	}
}

// isTextualMime 判断 MIME 是否属"文本家族"（嗅探器无法细分其子类型的那一类）。
func isTextualMime(mime string) bool {
	return strings.HasPrefix(mime, "text/") || mime == "application/json"
}
