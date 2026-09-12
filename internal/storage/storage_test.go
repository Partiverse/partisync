package storage

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"image"
	"image/color"
	"image/png"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// makePNG 生成一张 w×h 的合法 PNG，作为测试用真实内容。
func makePNG(t *testing.T, w, h int) []byte {
	t.Helper()
	img := image.NewRGBA(image.Rect(0, 0, w, h))
	img.Set(0, 0, color.RGBA{R: 1, G: 2, B: 3, A: 255})
	var buf bytes.Buffer
	if err := png.Encode(&buf, img); err != nil {
		t.Fatalf("encode test png: %v", err)
	}
	return buf.Bytes()
}

func newTestStore(t *testing.T, maxBytes int64) *Store {
	t.Helper()
	s, err := New(t.TempDir(), maxBytes)
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	return s
}

// countFiles 统计 root 下的普通文件数（排除 .tmp 工作目录）。
func countFiles(t *testing.T, root string) int {
	t.Helper()
	n := 0
	err := filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() {
			if info.Name() == tmpDirName {
				return filepath.SkipDir
			}
			return nil
		}
		n++
		return nil
	})
	if err != nil {
		t.Fatalf("walk root: %v", err)
	}
	return n
}

// countTempFiles 统计 .tmp 目录下的残留文件数（用于断言失败路径不留垃圾）。
func countTempFiles(t *testing.T, root string) int {
	t.Helper()
	entries, err := os.ReadDir(filepath.Join(root, tmpDirName))
	if err != nil {
		t.Fatalf("read tmp dir: %v", err)
	}
	return len(entries)
}

func TestSaveContentAddressedObject(t *testing.T) {
	s := newTestStore(t, 0)
	payload := makePNG(t, 3, 2)

	obj, err := s.Save(bytes.NewReader(payload), "Photo.PNG")
	if err != nil {
		t.Fatalf("Save: %v", err)
	}

	sum := sha256.Sum256(payload)
	wantHash := hex.EncodeToString(sum[:])
	if obj.SHA256 != wantHash {
		t.Fatalf("sha256 = %s, want %s", obj.SHA256, wantHash)
	}
	if wantRel := wantHash[:2] + "/" + wantHash[2:]; obj.RelPath != wantRel {
		t.Fatalf("rel path = %s, want %s", obj.RelPath, wantRel)
	}
	if obj.SizeBytes != int64(len(payload)) {
		t.Fatalf("size = %d, want %d", obj.SizeBytes, len(payload))
	}
	if obj.MimeType != "image/png" || !obj.ContentSniffed {
		t.Fatalf("mime = %q sniffed = %v, want image/png true", obj.MimeType, obj.ContentSniffed)
	}
	if obj.Width != 3 || obj.Height != 2 {
		t.Fatalf("dimensions = %dx%d, want 3x2", obj.Width, obj.Height)
	}
	if obj.Deduped {
		t.Fatal("first save must not be reported as deduped")
	}
	if obj.Ext != ".png" {
		t.Fatalf("ext = %q, want .png (lowercased)", obj.Ext)
	}

	abs, err := s.Resolve(obj.RelPath)
	if err != nil {
		t.Fatalf("Resolve: %v", err)
	}
	onDisk, err := os.ReadFile(abs)
	if err != nil {
		t.Fatalf("read stored object: %v", err)
	}
	if !bytes.Equal(onDisk, payload) {
		t.Fatal("stored bytes differ from uploaded bytes")
	}
	if got := countTempFiles(t, s.Root()); got != 0 {
		t.Fatalf("temp files left behind: %d", got)
	}
}

func TestSaveDeduplicatesIdenticalContent(t *testing.T) {
	s := newTestStore(t, 0)
	payload := makePNG(t, 2, 2)

	first, err := s.Save(bytes.NewReader(payload), "a.png")
	if err != nil {
		t.Fatalf("first Save: %v", err)
	}
	second, err := s.Save(bytes.NewReader(payload), "b.png")
	if err != nil {
		t.Fatalf("second Save: %v", err)
	}
	if !second.Deduped {
		t.Fatal("second save of identical content must be deduped")
	}
	if first.RelPath != second.RelPath {
		t.Fatalf("rel paths differ: %s vs %s", first.RelPath, second.RelPath)
	}
	if got := countFiles(t, s.Root()); got != 1 {
		t.Fatalf("object files on disk = %d, want 1", got)
	}
	if got := countTempFiles(t, s.Root()); got != 0 {
		t.Fatalf("temp files left behind: %d", got)
	}
}

func TestSaveRejectsUnsupportedExtension(t *testing.T) {
	s := newTestStore(t, 0)

	_, err := s.Save(strings.NewReader("MZ..."), "payload.exe")
	if !errors.Is(err, ErrUnsupportedType) {
		t.Fatalf("err = %v, want ErrUnsupportedType", err)
	}
	if got := countFiles(t, s.Root()); got != 0 {
		t.Fatalf("no object should be stored, found %d", got)
	}
	if got := countTempFiles(t, s.Root()); got != 0 {
		t.Fatalf("temp files left behind: %d", got)
	}
}

func TestSaveRejectsFileWithoutExtension(t *testing.T) {
	s := newTestStore(t, 0)

	if _, err := s.Save(strings.NewReader("data"), "noext"); !errors.Is(err, ErrUnsupportedType) {
		t.Fatalf("err = %v, want ErrUnsupportedType", err)
	}
}

func TestSaveRejectsEmptyFile(t *testing.T) {
	s := newTestStore(t, 0)

	if _, err := s.Save(bytes.NewReader(nil), "empty.png"); !errors.Is(err, ErrEmptyFile) {
		t.Fatalf("err = %v, want ErrEmptyFile", err)
	}
	if got := countTempFiles(t, s.Root()); got != 0 {
		t.Fatalf("temp files left behind: %d", got)
	}
}

func TestSaveRejectsOversizedFile(t *testing.T) {
	s := newTestStore(t, 4)

	_, err := s.Save(strings.NewReader("0123456789"), "big.txt")
	if !errors.Is(err, ErrTooLarge) {
		t.Fatalf("err = %v, want ErrTooLarge", err)
	}
	if got := countFiles(t, s.Root()); got != 0 {
		t.Fatalf("oversized payload must not be stored, found %d files", got)
	}
	if got := countTempFiles(t, s.Root()); got != 0 {
		t.Fatalf("temp files left behind: %d", got)
	}
}

// 客户端文件名带目录穿越成分时，落盘路径仍必须是内容寻址路径（不含客户端路径）。
func TestSaveIgnoresClientPathTraversal(t *testing.T) {
	s := newTestStore(t, 0)
	payload := makePNG(t, 1, 1)

	for _, name := range []string{
		"../../../../etc/passwd.png",
		"/etc/passwd.png",
		`..\..\windows\system32\evil.png`,
		"./nested/../photo.png",
	} {
		obj, err := s.Save(bytes.NewReader(payload), name)
		if err != nil {
			t.Fatalf("Save(%q): %v", name, err)
		}
		if strings.Contains(obj.RelPath, "..") {
			t.Fatalf("rel path %q must not contain traversal components", obj.RelPath)
		}
		abs, err := s.Resolve(obj.RelPath)
		if err != nil {
			t.Fatalf("Resolve(%q): %v", obj.RelPath, err)
		}
		rootPrefix := s.Root() + string(filepath.Separator)
		if !strings.HasPrefix(abs, rootPrefix) {
			t.Fatalf("stored path %q escapes root %q", abs, s.Root())
		}
	}
}

func TestResolveRejectsEscapeAttempts(t *testing.T) {
	s := newTestStore(t, 0)

	for _, rel := range []string{
		"../outside.txt",
		"../../outside.txt",
		"/etc/passwd",
		"a/../../outside.txt",
		"",
	} {
		if _, err := s.Resolve(rel); err == nil {
			t.Fatalf("Resolve(%q) must fail", rel)
		}
	}

	inside, err := s.Resolve("ab/cd")
	if err != nil {
		t.Fatalf("Resolve inside root: %v", err)
	}
	if want := filepath.Join(s.Root(), "ab", "cd"); inside != want {
		t.Fatalf("resolved = %q, want %q", inside, want)
	}
}

// Open 必须拒绝指向 Root 之外的符号链接。
func TestOpenRejectsSymlinkOutsideRoot(t *testing.T) {
	s := newTestStore(t, 0)
	outsideDir := t.TempDir()
	target := filepath.Join(outsideDir, "secret.txt")
	if err := os.WriteFile(target, []byte("secret"), 0o600); err != nil {
		t.Fatalf("write outside file: %v", err)
	}
	link := filepath.Join(s.Root(), "link.txt")
	if err := os.Symlink(target, link); err != nil {
		t.Skipf("symlink unsupported in this environment: %v", err)
	}

	if _, err := s.Open("link.txt"); !errors.Is(err, ErrOutsideRoot) {
		t.Fatalf("err = %v, want ErrOutsideRoot", err)
	}
}

func TestExtOf(t *testing.T) {
	cases := map[string]string{
		"photo.PNG":      ".png",
		"a/b/c.JPEG":     ".jpeg",
		"archive.tar.gz": ".gz",
		"noext":          "",
	}
	for name, want := range cases {
		if got := ExtOf(name); got != want {
			t.Errorf("ExtOf(%q) = %q, want %q", name, got, want)
		}
	}
}
