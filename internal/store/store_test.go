// Store 层集成测试：需要真实 Postgres，仅在 PG_DSN 指向可用实例时运行
// （L2 阶段对 docker compose 里的 partisync-postgres 执行；无 DSN 时跳过）。
package store

import (
	"context"
	"os"
	"testing"
	"time"

	"partisync/server/internal/models"
)

func testDSN(t *testing.T) string {
	t.Helper()
	dsn := os.Getenv("PG_DSN")
	if dsn == "" {
		t.Skip("PG_DSN not set: skipping store integration test")
	}
	return dsn
}

// DeleteAssetBySHA256（P2 独立复审 S1）：必须删除该哈希的资产行并如实报告，
// 之后 GetAssetBySHA256 返回 nil——上传回读失败分支的补偿删除依赖此语义。
func TestInsertAndDeleteAssetBySHA256(t *testing.T) {
	s, err := NewStore(testDSN(t))
	if err != nil {
		t.Fatalf("NewStore: %v", err)
	}
	defer s.Close()

	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()

	sha := "p21-test-" + time.Now().Format("150405.000000000")
	asset := &models.Asset{
		Name:         "p2.1-delete-test",
		Path:         "aa/bb",
		SHA256:       sha,
		SizeBytes:    1,
		MimeType:     "text/plain",
		ResourceType: "document",
	}
	inserted, err := s.InsertAsset(ctx, asset)
	if err != nil || !inserted {
		t.Fatalf("InsertAsset: inserted=%v err=%v", inserted, err)
	}
	got, err := s.GetAssetBySHA256(ctx, sha)
	if err != nil || got == nil {
		t.Fatalf("GetAssetBySHA256 after insert: got=%v err=%v", got, err)
	}

	deleted, err := s.DeleteAssetBySHA256(ctx, sha)
	if err != nil || !deleted {
		t.Fatalf("DeleteAssetBySHA256: deleted=%v err=%v", deleted, err)
	}
	gone, err := s.GetAssetBySHA256(ctx, sha)
	if err != nil || gone != nil {
		t.Fatalf("row still present after delete: got=%v err=%v", gone, err)
	}

	// 重复删除同一哈希：deleted=false（没有行可删），不视为错误。
	deleted, err = s.DeleteAssetBySHA256(ctx, sha)
	if err != nil {
		t.Fatalf("second DeleteAssetBySHA256: %v", err)
	}
	if deleted {
		t.Fatal("second delete reported deleted=true, want false")
	}
}
