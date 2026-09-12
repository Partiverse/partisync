package storage

import (
	"bytes"
	"image"
	_ "image/gif"  // 注册 GIF 解码器
	_ "image/jpeg" // 注册 JPEG 解码器
	_ "image/png"  // 注册 PNG 解码器
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
