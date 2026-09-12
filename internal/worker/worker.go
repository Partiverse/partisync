// Package worker 提供后台慢标注 Worker：轮询 annotation_jobs 队列表，
// 领取 pending 任务，执行 Mock AI 标注并回写结果或错误。
// 支持优雅停机与租约超时回收（M3 修复）。
package worker

import (
	"context"
	"encoding/json"
	"errors"
	"log"
	"math/rand"
	"time"

	"partisync/server/internal/store"
)

// Config 控制 Worker 行为。
type Config struct {
	PollInterval           time.Duration
	LeaseTimeout           time.Duration
	MaxRetries             int
	MockAnnotateDelayRange [2]time.Duration
}

var DefaultConfig = Config{
	PollInterval:           2 * time.Second,
	LeaseTimeout:           5 * time.Minute,
	MaxRetries:             3,
	MockAnnotateDelayRange: [2]time.Duration{100 * time.Millisecond, 500 * time.Millisecond},
}

// Annotator 执行一次标注的接口（可替换为真实 AI SDK）。
type Annotator interface {
	Annotate(ctx context.Context, assetID, prompt string) (json.RawMessage, error)
}

// MockAnnotator 实现 Annotator：随机延迟后返回结构化 mock 结果，
// 有约 5% 概率返回模拟错误以验证重试机制。
type MockAnnotator struct {
	DelayRange [2]time.Duration
	RNG        *rand.Rand
}

func NewMockAnnotator() *MockAnnotator {
	return &MockAnnotator{
		DelayRange: DefaultConfig.MockAnnotateDelayRange,
		RNG:        rand.New(rand.NewSource(time.Now().UnixNano())),
	}
}

func (m *MockAnnotator) Annotate(ctx context.Context, assetID, prompt string) (json.RawMessage, error) {
	delay := m.DelayRange[0] + time.Duration(m.RNG.Int63n(int64(m.DelayRange[1]-m.DelayRange[0])))
	select {
	case <-time.After(delay):
	case <-ctx.Done():
		return nil, ctx.Err()
	}
	if m.RNG.Float64() < 0.05 {
		return nil, errors.New("mock AI provider temporarily unavailable")
	}
	result := map[string]any{
		"tags": []map[string]any{
			{"name": "landscape", "confidence": 0.91 + m.RNG.Float64()*0.08},
			{"name": "outdoor", "confidence": 0.87 + m.RNG.Float64()*0.10},
			{"name": "daylight", "confidence": 0.93 + m.RNG.Float64()*0.06},
		},
		"description": "A scenic outdoor photograph featuring natural landscapes with clear visibility and natural lighting.",
		"model":       "mock-annotator-v1",
		"asset_id":    assetID,
		"prompt_used": prompt,
	}
	raw, err := json.Marshal(result)
	if err != nil {
		return nil, err
	}
	return json.RawMessage(raw), nil
}

// Worker 后台标注 Worker。
type Worker struct {
	store     *store.Store
	annotator Annotator
	cfg       Config
}

func NewWorker(st *store.Store, a Annotator, cfg Config) *Worker {
	if cfg.PollInterval == 0 {
		cfg.PollInterval = DefaultConfig.PollInterval
	}
	if cfg.LeaseTimeout == 0 {
		cfg.LeaseTimeout = DefaultConfig.LeaseTimeout
	}
	if cfg.MaxRetries == 0 {
		cfg.MaxRetries = DefaultConfig.MaxRetries
	}
	return &Worker{store: st, annotator: a, cfg: cfg}
}

// Run 启动 Worker 循环；ctx 取消时优雅退出。
// 已在租约超时的任务会被本轮回收并重新入队。
func (w *Worker) Run(ctx context.Context) {
	log.Printf("[worker] starting (poll=%v, lease_timeout=%v, max_retries=%d)",
		w.cfg.PollInterval, w.cfg.LeaseTimeout, w.cfg.MaxRetries)

	for {
		select {
		case <-ctx.Done():
			log.Printf("[worker] shutting down")
			return
		default:
		}

		// 1. 先回收超期租约
		reclaimed, err := w.store.RecoverStaleProcessing(ctx, w.cfg.LeaseTimeout)
		if err != nil {
			log.Printf("[worker] recover stale: %v", err)
		} else if reclaimed > 0 {
			log.Printf("[worker] recovered %d stale processing job(s)", reclaimed)
		}

		// 2. 尝试领取一个 pending 任务
		job, err := w.store.DequeuePendingJob(ctx)
		if err != nil {
			log.Printf("[worker] dequeue: %v", err)
			time.Sleep(w.cfg.PollInterval)
			continue
		}
		if job == nil {
			time.Sleep(w.cfg.PollInterval)
			continue
		}

		log.Printf("[worker] processing job %s (asset=%s, attempt=%d)",
			job.ID, job.AssetID, job.RetryCount)

		// 3. 执行标注
		result, err := w.annotator.Annotate(ctx, job.AssetID, job.Prompt)
		if err != nil {
			log.Printf("[worker] annotate error job %s: %v", job.ID, err)
			if job.RetryCount+1 >= w.cfg.MaxRetries {
				if doneErr := w.store.CompleteJob(ctx, job.ID, nil, err.Error()); doneErr != nil {
					log.Printf("[worker] mark job %s failed: %v", job.ID, doneErr)
				}
				continue
			}
			if requeueErr := w.store.RequeueJob(ctx, job.ID); requeueErr != nil {
				log.Printf("[worker] requeue job %s: %v", job.ID, requeueErr)
			}
			continue
		}

		// 4. 标注成功，回写结果
		if err := w.store.CompleteJob(ctx, job.ID, result, ""); err != nil {
			log.Printf("[worker] complete job %s: %v", job.ID, err)
		} else {
			log.Printf("[worker] job %s completed successfully", job.ID)
		}
	}
}

// Ensure Worker satisfies the Annotator interface.
var _ Annotator = (*MockAnnotator)(nil)
