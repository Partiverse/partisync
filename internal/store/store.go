// Package store 封装 PostgreSQL 数据访问层（database/sql + lib/pq）。
// 所有方法接受 ctx，由调用方控制超时；不返回驱动专有错误类型。
package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"

	_ "github.com/lib/pq" // 注册 postgres 驱动

	"partisync/server/internal/models"
)

// Store 持有连接池句柄，可并发安全共享。
type Store struct {
	db *sql.DB
}

// NewStore 打开 DSN 对应连接池，限制最大连接数并 ping 验证连通性。
func NewStore(dsn string) (*Store, error) {
	db, err := sql.Open("postgres", dsn)
	if err != nil {
		return nil, fmt.Errorf("open postgres: %w", err)
	}
	db.SetMaxOpenConns(20)
	db.SetMaxIdleConns(10)
	db.SetConnMaxLifetime(30 * time.Minute)
	if err := db.Ping(); err != nil {
		_ = db.Close()
		return nil, fmt.Errorf("ping postgres: %w", err)
	}
	return &Store{db: db}, nil
}

// Close 关闭连接池。
func (s *Store) Close() error {
	if s == nil || s.db == nil {
		return nil
	}
	return s.db.Close()
}

// assetColumns 统一列清单，避免 SELECT *。
const assetColumns = "id, name, path, sha256, size_bytes, mime_type, resource_type, metadata, created_at, updated_at"

// InsertAsset 按 sha256 去重写入资产。
// 返回 inserted=false 表示哈希冲突（已存在），不视为错误。
func (s *Store) InsertAsset(ctx context.Context, a *models.Asset) (bool, error) {
	metadata := a.Metadata
	if len(metadata) == 0 {
		metadata = []byte("{}")
	}
	const q = `INSERT INTO assets (name, path, sha256, size_bytes, mime_type, resource_type, metadata)
VALUES ($1, $2, $3, $4, $5, $6, $7)
ON CONFLICT (sha256) DO NOTHING`
	res, err := s.db.ExecContext(ctx, q,
		a.Name, a.Path, a.SHA256, a.SizeBytes, a.MimeType, a.ResourceType, metadata)
	if err != nil {
		return false, fmt.Errorf("insert asset: %w", err)
	}
	n, err := res.RowsAffected()
	if err != nil {
		return false, fmt.Errorf("insert asset rows affected: %w", err)
	}
	return n > 0, nil
}

// GetAssetByID 按主键查询；未找到返回 (nil, nil)。
func (s *Store) GetAssetByID(ctx context.Context, id string) (*models.Asset, error) {
	q := "SELECT " + assetColumns + " FROM assets WHERE id = $1"
	row := s.db.QueryRowContext(ctx, q, id)
	a, err := scanAsset(row)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("get asset: %w", err)
	}
	return a, nil
}

// GetAssetBySHA256 按内容哈希查询；未找到返回 (nil, nil)。
// 供冲突分支回读完整资产（含服务端生成的 id/时间戳）。
func (s *Store) GetAssetBySHA256(ctx context.Context, sha256 string) (*models.Asset, error) {
	q := "SELECT " + assetColumns + " FROM assets WHERE sha256 = $1"
	row := s.db.QueryRowContext(ctx, q, sha256)
	a, err := scanAsset(row)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("get asset by sha256: %w", err)
	}
	return a, nil
}

// ListAssets 按创建时间倒序分页列出资产。
func (s *Store) ListAssets(ctx context.Context, limit, offset int) ([]models.Asset, error) {
	q := "SELECT " + assetColumns + " FROM assets ORDER BY created_at DESC LIMIT $1 OFFSET $2"
	rows, err := s.db.QueryContext(ctx, q, limit, offset)
	if err != nil {
		return nil, fmt.Errorf("list assets: %w", err)
	}
	defer rows.Close()

	out := make([]models.Asset, 0, limit)
	for rows.Next() {
		a, err := scanAsset(rows)
		if err != nil {
			return nil, fmt.Errorf("scan asset row: %w", err)
		}
		out = append(out, *a)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate assets: %w", err)
	}
	return out, nil
}

// CountAssets 返回资产总数，供列表接口在 PG 路径上填充 total。
func (s *Store) CountAssets(ctx context.Context) (int64, error) {
	var n int64
	if err := s.db.QueryRowContext(ctx, "SELECT count(*) FROM assets").Scan(&n); err != nil {
		return 0, fmt.Errorf("count assets: %w", err)
	}
	return n, nil
}

// CreateAnnotationJob 创建慢标注任务，初始状态 pending。
func (s *Store) CreateAnnotationJob(ctx context.Context, assetID, prompt string) (*models.AnnotationJob, error) {
	const q = `INSERT INTO annotation_jobs (asset_id, prompt, status)
VALUES ($1, $2, 'pending')
RETURNING id, asset_id, status, COALESCE(prompt, ''), COALESCE(result, '{}'::jsonb),
         COALESCE(error_message, ''), retry_count, created_at, updated_at`
	row := s.db.QueryRowContext(ctx, q, assetID, prompt)
	var j models.AnnotationJob
	if err := row.Scan(&j.ID, &j.AssetID, &j.Status, &j.Prompt, &j.Result,
		&j.ErrorMessage, &j.RetryCount, &j.CreatedAt, &j.UpdatedAt); err != nil {
		return nil, fmt.Errorf("create annotation job: %w", err)
	}
	return &j, nil
}

// GetJob 按 ID 查询单个任务；未找到返回 (nil, nil)。
func (s *Store) GetJob(ctx context.Context, id string) (*models.AnnotationJob, error) {
	const cols = `id, asset_id, status, COALESCE(prompt, ''), COALESCE(result, '{}'::jsonb),
COALESCE(error_message, ''), retry_count, created_at, updated_at`
	q := "SELECT " + cols + " FROM annotation_jobs WHERE id = $1"
	row := s.db.QueryRowContext(ctx, q, id)
	var j models.AnnotationJob
	if err := row.Scan(&j.ID, &j.AssetID, &j.Status, &j.Prompt, &j.Result,
		&j.ErrorMessage, &j.RetryCount, &j.CreatedAt, &j.UpdatedAt); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, nil
		}
		return nil, fmt.Errorf("get job: %w", err)
	}
	return &j, nil
}

// ListJobs 按创建时间倒序列出任务；status 为空表示不过滤。
func (s *Store) ListJobs(ctx context.Context, status string, limit int) ([]models.AnnotationJob, error) {
	const cols = `id, asset_id, status, COALESCE(prompt, ''), COALESCE(result, '{}'::jsonb),
COALESCE(error_message, ''), retry_count, created_at, updated_at`

	var (
		q    string
		args []any
	)
	if status == "" {
		q = "SELECT " + cols + " FROM annotation_jobs ORDER BY created_at DESC LIMIT $1"
		args = []any{limit}
	} else {
		q = "SELECT " + cols + " FROM annotation_jobs WHERE status = $1 ORDER BY created_at DESC LIMIT $2"
		args = []any{status, limit}
	}

	rows, err := s.db.QueryContext(ctx, q, args...)
	if err != nil {
		return nil, fmt.Errorf("list jobs: %w", err)
	}
	defer rows.Close()

	out := make([]models.AnnotationJob, 0, limit)
	for rows.Next() {
		var j models.AnnotationJob
		if err := rows.Scan(&j.ID, &j.AssetID, &j.Status, &j.Prompt, &j.Result,
			&j.ErrorMessage, &j.RetryCount, &j.CreatedAt, &j.UpdatedAt); err != nil {
			return nil, fmt.Errorf("scan job row: %w", err)
		}
		out = append(out, j)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate jobs: %w", err)
	}
	return out, nil
}

// DequeuePendingJob 事务内用 FOR UPDATE SKIP LOCKED 原子领取一个 pending 任务。
// 领取成功即置为 processing；无待处理任务返回 (nil, nil)。
func (s *Store) DequeuePendingJob(ctx context.Context) (*models.AnnotationJob, error) {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return nil, fmt.Errorf("begin tx: %w", err)
	}
	// 出错路径统一回滚，避免事务悬挂。
	committed := false
	defer func() {
		if !committed {
			_ = tx.Rollback()
		}
	}()

	const sel = `SELECT id, asset_id, COALESCE(prompt, ''), retry_count
FROM annotation_jobs
WHERE status = 'pending'
ORDER BY created_at
LIMIT 1
FOR UPDATE SKIP LOCKED`

	var (
		id      string
		assetID string
		prompt  string
		retries int
	)
	err = tx.QueryRowContext(ctx, sel).Scan(&id, &assetID, &prompt, &retries)
	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("dequeue select: %w", err)
	}

	const upd = `UPDATE annotation_jobs SET status = 'processing', updated_at = NOW() WHERE id = $1`
	if _, err := tx.ExecContext(ctx, upd, id); err != nil {
		return nil, fmt.Errorf("dequeue update: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("dequeue commit: %w", err)
	}
	committed = true

	return &models.AnnotationJob{
		ID:         id,
		AssetID:    assetID,
		Status:     models.StatusProcessing,
		Prompt:     prompt,
		RetryCount: retries,
	}, nil
}

// rowScanner 抽象 QueryRow 与 Rows 的 Scan。
type rowScanner interface {
	Scan(dest ...any) error
}

// scanAsset 将一行结果扫描为 Asset。metadata 允许为 NULL。
func scanAsset(r rowScanner) (*models.Asset, error) {
	var (
		a        models.Asset
		metadata []byte
	)
	if err := r.Scan(&a.ID, &a.Name, &a.Path, &a.SHA256, &a.SizeBytes, &a.MimeType,
		&a.ResourceType, &metadata, &a.CreatedAt, &a.UpdatedAt); err != nil {
		return nil, err
	}
	if len(metadata) > 0 {
		a.Metadata = json.RawMessage(metadata)
	}
	return &a, nil
}

// RecoverStaleProcessing 将所有 processing 状态超过 timeout 未完成的任务
// 重置为 pending 并递增 retry_count。原子事务保证无漏扫、无重复计数。
func (s *Store) RecoverStaleProcessing(ctx context.Context, timeout time.Duration) (int, error) {
	secs := int(timeout.Seconds())
	const q = `
		UPDATE annotation_jobs
		SET status = 'pending',
		    updated_at = NOW(),
		    retry_count = retry_count + 1,
		    error_message = 'lease timeout: worker crashed or stalled'
		WHERE status = 'processing'
		  AND updated_at < NOW() - ($1 || ' seconds')::interval
		RETURNING id`
	rows, err := s.db.ExecContext(ctx, q, secs)
	if err != nil {
		return 0, err
	}
	n, err := rows.RowsAffected()
	return int(n), err
}

// CompleteJob 将任务标记为 completed（含 result）或 failed（含 error_message）。
func (s *Store) CompleteJob(ctx context.Context, jobID string, result json.RawMessage, errMsg string) error {
	var q string
	var args []any
	if errMsg != "" {
		q = `UPDATE annotation_jobs
SET status = 'failed', result = $2, error_message = $3, updated_at = NOW()
WHERE id = $1`
		args = []any{jobID, result, errMsg}
	} else {
		if result == nil {
			result = []byte("{}")
		}
		q = `UPDATE annotation_jobs
SET status = 'completed', result = $2, error_message = '', updated_at = NOW()
WHERE id = $1`
		args = []any{jobID, result}
	}
	_, err := s.db.ExecContext(ctx, q, args...)
	return err
}

// RequeueJob 将失败任务重试：status → pending，retry_count + 1。
func (s *Store) RequeueJob(ctx context.Context, jobID string) error {
	const q = `UPDATE annotation_jobs
SET status = 'pending', updated_at = NOW(), retry_count = retry_count + 1
WHERE id = $1`
	_, err := s.db.ExecContext(ctx, q, jobID)
	return err
}

// maxTagLen 与 schema 中 tags.name VARCHAR(128) 对齐。
const maxTagLen = 128

// UpsertTag 幂等创建标签（按 name 去重）；颜色缺省交由 schema 默认值。
// 返回标签的完整记录（含服务端生成的 id / created_at）。
func (s *Store) UpsertTag(ctx context.Context, name, color string) (*models.Tag, error) {
	var q string
	var args []any
	if color == "" {
		q = `INSERT INTO tags (name) VALUES ($1)
ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
RETURNING id, name, color, created_at`
		args = []any{name}
	} else {
		q = `INSERT INTO tags (name, color) VALUES ($1, $2)
ON CONFLICT (name) DO UPDATE SET color = EXCLUDED.color
RETURNING id, name, color, created_at`
		args = []any{name, color}
	}
	row := s.db.QueryRowContext(ctx, q, args...)
	var t models.Tag
	if err := row.Scan(&t.ID, &t.Name, &t.Color, &t.CreatedAt); err != nil {
		return nil, fmt.Errorf("upsert tag: %w", err)
	}
	return &t, nil
}

// ListTags 列出全部标签（按名称排序）。
func (s *Store) ListTags(ctx context.Context) ([]models.Tag, error) {
	rows, err := s.db.QueryContext(ctx, "SELECT id, name, color, created_at FROM tags ORDER BY name")
	if err != nil {
		return nil, fmt.Errorf("list tags: %w", err)
	}
	defer rows.Close()

	out := []models.Tag{}
	for rows.Next() {
		var t models.Tag
		if err := rows.Scan(&t.ID, &t.Name, &t.Color, &t.CreatedAt); err != nil {
			return nil, fmt.Errorf("scan tag row: %w", err)
		}
		out = append(out, t)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate tags: %w", err)
	}
	return out, nil
}

// AssetTag 资产标签关联（asset_tags 表行）。
type AssetTag struct {
	TagID      string    `json:"tag_id"`
	Name       string    `json:"name"`
	Color      string    `json:"color"`
	Source     string    `json:"source"` // 'human' | 'ai'
	Confidence float64   `json:"confidence"`
	CreatedAt  time.Time `json:"created_at"`
}

// ListAssetTags 列出资产的全部标签（含来源与置信度）。
func (s *Store) ListAssetTags(ctx context.Context, assetID string) ([]AssetTag, error) {
	const q = `SELECT t.id, t.name, t.color, at.source, COALESCE(at.confidence, 1.0), at.created_at
FROM asset_tags at JOIN tags t ON t.id = at.tag_id
WHERE at.asset_id = $1 ORDER BY at.created_at`
	rows, err := s.db.QueryContext(ctx, q, assetID)
	if err != nil {
		return nil, fmt.Errorf("list asset tags: %w", err)
	}
	defer rows.Close()

	out := []AssetTag{}
	for rows.Next() {
		var at AssetTag
		if err := rows.Scan(&at.TagID, &at.Name, &at.Color, &at.Source, &at.Confidence, &at.CreatedAt); err != nil {
			return nil, fmt.Errorf("scan asset tag row: %w", err)
		}
		out = append(out, at)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate asset tags: %w", err)
	}
	return out, nil
}

// TagAsset 把标签关联到资产（幂等：同一资产同一标签不产生第二行）。
// source 取 'human' 或 'ai'；已存在的关联保留首次来源，不覆盖。
func (s *Store) TagAsset(ctx context.Context, assetID, tagID, source string, confidence float64) error {
	if source != "human" && source != "ai" {
		return fmt.Errorf("invalid tag source %q", source)
	}
	const q = `INSERT INTO asset_tags (asset_id, tag_id, source, confidence)
VALUES ($1, $2, $3, $4)
ON CONFLICT (asset_id, tag_id) DO NOTHING`
	if _, err := s.db.ExecContext(ctx, q, assetID, tagID, source, confidence); err != nil {
		return fmt.Errorf("tag asset: %w", err)
	}
	return nil
}

// UntagAsset 移除资产与标签的关联；关联不存在不视为错误。
func (s *Store) UntagAsset(ctx context.Context, assetID, tagID string) error {
	const q = `DELETE FROM asset_tags WHERE asset_id = $1 AND tag_id = $2`
	if _, err := s.db.ExecContext(ctx, q, assetID, tagID); err != nil {
		return fmt.Errorf("untag asset: %w", err)
	}
	return nil
}

// ConfirmSuggestions 把 AI 建议标签写入 asset_tags（source='ai'，带置信度）。
// 事务内完成：按名称幂等建标签 + 关联资产，保证建议全部落地或全部不落地。
// 返回本次确认的建议数（已存在的关联不重复计数）。
func (s *Store) ConfirmSuggestions(ctx context.Context, assetID string, suggestions []models.TagSuggestion) (int, error) {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return 0, fmt.Errorf("begin tx: %w", err)
	}
	committed := false
	defer func() {
		if !committed {
			_ = tx.Rollback()
		}
	}()

	confirmed := 0
	for _, sug := range suggestions {
		name := strings.TrimSpace(sug.Name)
		if name == "" || len(name) > maxTagLen {
			continue
		}
		var tagID string
		if err := tx.QueryRowContext(ctx,
			`INSERT INTO tags (name) VALUES ($1)
ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
RETURNING id`, name).Scan(&tagID); err != nil {
			return 0, fmt.Errorf("confirm: upsert tag %q: %w", name, err)
		}
		res, err := tx.ExecContext(ctx,
			`INSERT INTO asset_tags (asset_id, tag_id, source, confidence)
VALUES ($1, $2, 'ai', $3)
ON CONFLICT (asset_id, tag_id) DO NOTHING`,
			assetID, tagID, sug.Confidence)
		if err != nil {
			return 0, fmt.Errorf("confirm: tag asset: %w", err)
		}
		n, err := res.RowsAffected()
		if err != nil {
			return 0, fmt.Errorf("confirm: rows affected: %w", err)
		}
		confirmed += int(n)
	}
	if err := tx.Commit(); err != nil {
		return 0, fmt.Errorf("confirm commit: %w", err)
	}
	committed = true
	return confirmed, nil
}
