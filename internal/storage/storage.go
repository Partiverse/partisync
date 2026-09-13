// Package storage 提供本地内容寻址存储（content-addressed local store）。
//
// 安全约束（T6-01 修复的核心）：
//   - 存储名由服务端生成（sha256 十六进制），**绝不使用客户端文件名作为路径组成部分**；
//   - 客户端文件名只用于取扩展名（白名单）并作为展示用 name 落库；
//   - 所有对外暴露的路径解析（Resolve/Open）都校验最终绝对路径位于 Root 之内，
//     因此 `..`、绝对路径、符号链接指向 Root 之外都会被拒绝。
//
// 写入流程：流式写入 Root/.tmp 下的临时文件，同时计算 SHA256；校验大小上限与扩展名白名单后，
// 按 sha256 归属到最终路径（Root/<前2位>/<其余62位>）。内容已存在则不写新副本（去重）。
package storage

import (
	"bufio"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const (
	// DefaultMaxBytes 单文件默认上限：100 MiB。
	DefaultMaxBytes = 100 << 20
	// sniffPeekBytes 内容嗅探与图片尺寸探测的读取前缀大小。
	sniffPeekBytes = 64 << 10
	// SniffPeekBytes 对外暴露的嗅探前缀大小（预览端点内容校验用，A2-04）。
	SniffPeekBytes = sniffPeekBytes
	tmpDirName     = ".tmp"
)

// 可预期的调用方错误（handler 依据这些错误返回 4xx）。
var (
	ErrEmptyFile       = errors.New("storage: empty file")
	ErrTooLarge        = errors.New("storage: file exceeds maximum size")
	ErrUnsupportedType = errors.New("storage: unsupported file extension")
	ErrOutsideRoot     = errors.New("storage: path escapes storage root")
)

// AllowedExtensions 首版扩展名白名单（图像 + 文档），值为默认 MIME（最终以内容嗅探为准）。
var AllowedExtensions = map[string]string{
	// 图像
	".png":  "image/png",
	".jpg":  "image/jpeg",
	".jpeg": "image/jpeg",
	".gif":  "image/gif",
	".webp": "image/webp",
	".svg":  "image/svg+xml",
	".bmp":  "image/bmp",
	".ico":  "image/x-icon",
	".tiff": "image/tiff",
	".heic": "image/heic",
	// 视频
	".mp4":  "video/mp4",
	".mkv":  "video/x-matroska",
	".avi":  "video/x-msvideo",
	".mov":  "video/quicktime",
	".wmv":  "video/x-ms-wmv",
	".flv":  "video/x-flv",
	".webm": "video/webm",
	".m4v":  "video/x-m4v",
	".mpeg": "video/mpeg",
	".mpg":  "video/mpeg",
	// 音频
	".mp3":  "audio/mpeg",
	".wav":  "audio/wav",
	".flac": "audio/flac",
	".aac":  "audio/aac",
	".ogg":  "audio/ogg",
	".m4a":  "audio/mp4",
	".wma":  "audio/x-ms-wma",
	".opus": "audio/opus",
	// 文档
	".pdf":  "application/pdf",
	".txt":  "text/plain",
	".md":   "text/markdown",
	".csv":  "text/csv",
	".json": "application/json",
	".xml":  "application/xml",
	".html": "text/html",
	".htm":  "text/html",
	".css":  "text/css",
	".js":   "application/javascript",
	// Office
	".doc":  "application/msword",
	".docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
	".xls":  "application/vnd.ms-excel",
	".xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
	".ppt":  "application/vnd.ms-powerpoint",
	".pptx": "application/vnd.openxmlformats-officedocument.presentationml.presentation",
	// 电子书
	".epub": "application/epub+zip",
	".mobi": "application/x-mobipocket-ebook",
	".azw":  "application/vnd.amazon.ebook",
	// 压缩包
	".zip":  "application/zip",
	".rar":  "application/vnd.rar",
	".7z":   "application/x-7z-compressed",
	".tar":  "application/x-tar",
	".gz":   "application/gzip",
	".tgz":  "application/gzip",
	// 磁盘镜像
	".iso":  "application/x-iso9660-image",
	".dmg":  "application/x-apple-diskimage",
	".img":  "application/x-raw-disk-image",
	// 可执行文件
	".exe":  "application/x-msdownload",
	".msi":  "application/x-msdownload",
	".deb":  "application/x-debian-package",
	".rpm":  "application/x-rpm",
	".appimage": "application/vnd.appimage",
	".pkg":  "application/x-newton-compatible-pkg",
	// 代码
	".py":   "text/x-python",
	".go":   "text/x-go",
	".java": "text/x-java",
	".c":    "text/x-c",
	".cpp":  "text/x-c++",
	".h":    "text/x-c",
	".sh":   "application/x-sh",
	".bash": "application/x-sh",
	".ts":   "application/typescript",
	".tsx":  "application/typescript",
	".jsx":  "application/javascript",
	// 其他
	".rtf":  "application/rtf",
	".odt":  "application/vnd.oasis.opendocument.text",
	".ods":  "application/vnd.oasis.opendocument.spreadsheet",
	".odp":  "application/vnd.oasis.opendocument.presentation",
	".xps":  "application/vnd.ms-xpsdocument",
	".pages": "application/x-iwork-pages-sff",
	".numbers": "application/x-iwork-numbers-sff",
	".key":  "application/x-iwork-keynote-sff",
}

// Store 本地内容寻址存储。
type Store struct {
	root     string
	maxBytes int64
}

// New 创建（必要时初始化）存储根目录。
func New(root string, maxBytes int64) (*Store, error) {
	if strings.TrimSpace(root) == "" {
		return nil, errors.New("storage: empty root")
	}
	abs, err := filepath.Abs(root)
	if err != nil {
		return nil, fmt.Errorf("storage: resolve root: %w", err)
	}
	if err := os.MkdirAll(filepath.Join(abs, tmpDirName), 0o755); err != nil {
		return nil, fmt.Errorf("storage: init root: %w", err)
	}
	if maxBytes <= 0 {
		maxBytes = DefaultMaxBytes
	}
	return &Store{root: abs, maxBytes: maxBytes}, nil
}

// Root 返回存储根的绝对路径。
func (s *Store) Root() string { return s.root }

// Healthy 检查存储可用（readiness 探针用，A2-02）。
// P2 独立复审 F-I1：只 os.Stat 是弱探针——只读挂载或 .tmp 被替换成普通文件时
// 会假阳性 healthy。这里改为真实写探针：确认 .tmp 存在且为目录，并在其中
// 创建+删除一个临时文件验证写权限。磁盘剩余空间检查需要平台相关 syscall，
// 本版不含（已知盲区，见 docs/SESSION.md §14）。
func (s *Store) Healthy() error {
	tmpDir := filepath.Join(s.root, tmpDirName)
	info, err := os.Stat(tmpDir)
	if err != nil {
		return fmt.Errorf("storage tmp dir %q not statable: %w", tmpDir, err)
	}
	if !info.IsDir() {
		return fmt.Errorf("storage tmp path %q is not a directory", tmpDir)
	}
	f, err := os.CreateTemp(tmpDir, "healthcheck-*")
	if err != nil {
		return fmt.Errorf("storage tmp dir %q not writable: %w", tmpDir, err)
	}
	name := f.Name()
	_ = f.Close()
	if err := os.Remove(name); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("storage healthcheck file cleanup failed in %q: %w", tmpDir, err)
	}
	return nil
}

// CleanStaleTemp 清扫 .tmp 内残留的 upload-* 临时文件（P2 独立复审 S5）：
// 进程崩溃会把已创建、未 rename 的临时文件永久留在 .tmp 里，Save 的 defer
// 只覆盖本次调用。返回删除的文件数；maxAge 之内的文件视为仍在写入，不动。
// 建议在服务启动时调用一次。
func (s *Store) CleanStaleTemp(maxAge time.Duration) (int, error) {
	tmpDir := filepath.Join(s.root, tmpDirName)
	entries, err := os.ReadDir(tmpDir)
	if err != nil {
		return 0, fmt.Errorf("storage: read tmp dir: %w", err)
	}
	deadline := time.Now().Add(-maxAge)
	removed := 0
	for _, e := range entries {
		if e.IsDir() || !strings.HasPrefix(e.Name(), "upload-") {
			continue
		}
		info, err := e.Info()
		if err != nil {
			continue // 并发窗口内文件可能刚被 rename 走，跳过即可
		}
		if info.ModTime().After(deadline) {
			continue
		}
		if err := os.Remove(filepath.Join(tmpDir, e.Name())); err == nil {
			removed++
		}
	}
	return removed, nil
}

// Discard 删除已落盘的对象（A2-05：入库失败时清理孤儿文件，防磁盘泄漏）。
// relPath 必须是 Save 返回的相对路径（会再次经过 Resolve 的越界校验）；
// 对象不存在视为成功（幂等）。
func (s *Store) Discard(relPath string) error {
	abs, err := s.Resolve(relPath)
	if err != nil {
		return err
	}
	if err := os.Remove(abs); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("storage: discard object: %w", err)
	}
	return nil
}

// MaxBytes 返回单文件上限。
func (s *Store) MaxBytes() int64 { return s.maxBytes }

// SavedObject 一次落盘的结果。
type SavedObject struct {
	SHA256         string
	SizeBytes      int64
	MimeType       string // 内容嗅探结果，嗅探失败时回退为白名单默认值
	Ext            string
	RelPath        string // 相对 Root 的路径，写入 assets.path
	Width          int    // 图片宽（非图片/解码失败为 0）
	Height         int    // 图片高
	ContentSniffed bool
	Deduped        bool // true 表示该内容此前已存在，本次未写入新副本
}

// ExtOf 返回客户端文件名的规范化扩展名（小写，含点）；无扩展名返回空串。
func ExtOf(clientName string) string {
	return strings.ToLower(filepath.Ext(filepath.Base(clientName)))
}

// Save 流式接收 r 并落盘，返回内容寻址结果。
// 调用方负责关闭 r；Save 不会读取超过 maxBytes+1 字节。
func (s *Store) Save(r io.Reader, clientName string) (*SavedObject, error) {
	ext := ExtOf(clientName)
	if _, ok := AllowedExtensions[ext]; !ok {
		return nil, fmt.Errorf("%w: %q", ErrUnsupportedType, ext)
	}

	tmp, err := os.CreateTemp(filepath.Join(s.root, tmpDirName), "upload-*")
	if err != nil {
		return nil, fmt.Errorf("storage: create temp: %w", err)
	}
	tmpPath := tmp.Name()
	committed := false
	defer func() {
		if !committed {
			_ = os.Remove(tmpPath)
		}
	}()

	br := bufio.NewReaderSize(r, sniffPeekBytes)
	head, _ := br.Peek(sniffPeekBytes)

	hasher := sha256.New()
	n, copyErr := io.Copy(io.MultiWriter(tmp, hasher), io.LimitReader(br, s.maxBytes+1))
	closeErr := tmp.Close()
	if copyErr != nil {
		return nil, fmt.Errorf("storage: write temp: %w", copyErr)
	}
	if closeErr != nil {
		return nil, fmt.Errorf("storage: close temp: %w", closeErr)
	}
	if n == 0 {
		return nil, ErrEmptyFile
	}
	if n > s.maxBytes {
		return nil, fmt.Errorf("%w: limit %d bytes", ErrTooLarge, s.maxBytes)
	}

	sum := hex.EncodeToString(hasher.Sum(nil))
	rel := filepath.Join(sum[:2], sum[2:])
	finalPath := filepath.Join(s.root, rel)

	obj := &SavedObject{
		SHA256:    sum,
		SizeBytes: n,
		Ext:       ext,
		RelPath:   filepath.ToSlash(rel),
	}
	obj.MimeType, obj.ContentSniffed = sniffMime(head, AllowedExtensions[ext])
	obj.Width, obj.Height = imageSize(head)

	if err := os.MkdirAll(filepath.Dir(finalPath), 0o755); err != nil {
		return nil, fmt.Errorf("storage: create shard dir: %w", err)
	}
	// S4（P2 独立复审）：用 link() 原子提交，取代 Stat→Rename——旧实现下并发
	// 同内容上传双方都可能看到「对象不存在」，随后任意一方失败路径 Discard 会
	// 误删另一方行引用的共享对象。link() 保证恰好一个请求创建终路径对象，
	// 其余得到 ErrExist（Deduped=true），Discard 只可能由创建者执行。
	// （link() 在 Windows 不可用；部署目标为 Linux 容器。）
	linkErr := os.Link(tmpPath, finalPath)
	switch {
	case linkErr == nil:
		obj.Deduped = false
	case errors.Is(linkErr, os.ErrExist):
		// 终路径已被（并发请求或更早请求）提交：本次不产生新副本。
		obj.Deduped = true
	default:
		return nil, fmt.Errorf("storage: commit object: %w", linkErr)
	}
	// link 成功后 tmp 仍是第二个名字，显式摘除；dedup 时 tmp 也应删除。
	_ = os.Remove(tmpPath)
	committed = true
	return obj, nil
}

// Resolve 将相对路径解析为 Root 内的绝对路径；越界返回 ErrOutsideRoot。
// relPath 为空或未找到对象时返回 os.ErrNotExist 包装错误。
func (s *Store) Resolve(relPath string) (string, error) {
	if strings.TrimSpace(relPath) == "" {
		return "", fmt.Errorf("%w: empty path", os.ErrNotExist)
	}
	// 拒绝绝对路径与任何形式的父目录逃逸（Clean 后仍以 .. 开头即非法）。
	cleaned := filepath.Clean(filepath.FromSlash(relPath))
	if filepath.IsAbs(cleaned) || cleaned == ".." || strings.HasPrefix(cleaned, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("%w: %q", ErrOutsideRoot, relPath)
	}
	abs := filepath.Join(s.root, cleaned)
	// filepath.Join 已 Clean；再做一次前缀校验，防御后续改动引入的绕过。
	rootPrefix := s.root + string(filepath.Separator)
	if abs != s.root && !strings.HasPrefix(abs, rootPrefix) {
		return "", fmt.Errorf("%w: %q", ErrOutsideRoot, relPath)
	}
	return abs, nil
}

// Open 打开存储中的对象（供后续下载/预览使用）。符号链接指向 Root 之外会被拒绝。
func (s *Store) Open(relPath string) (*os.File, error) {
	abs, err := s.Resolve(relPath)
	if err != nil {
		return nil, err
	}
	f, err := os.Open(abs)
	if err != nil {
		return nil, err
	}
	realPath, err := filepath.EvalSymlinks(abs)
	if err != nil {
		_ = f.Close()
		return nil, err
	}
	rootPrefix := s.root + string(filepath.Separator)
	if realPath != s.root && !strings.HasPrefix(realPath, rootPrefix) {
		_ = f.Close()
		return nil, fmt.Errorf("%w: symlink %q", ErrOutsideRoot, relPath)
	}
	return f, nil
}

// normalizeMime 去掉 MIME 参数（如 "; charset=utf-8"）并裁剪空白，便于比较。
func normalizeMime(mime string) string {
	if i := strings.IndexByte(mime, ';'); i >= 0 {
		mime = mime[:i]
	}
	return strings.TrimSpace(mime)
}

// sniffMime 嗅探前缀内容类型；无法判定时回退到扩展名默认值。
func sniffMime(head []byte, fallback string) (string, bool) {
	if len(head) == 0 {
		return fallback, false
	}
	detected := normalizeMime(http.DetectContentType(head))
	if detected == "" || detected == "application/octet-stream" {
		return fallback, false
	}
	return detected, true
}
