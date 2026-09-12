#!/usr/bin/env bash
# scripts/benchmark-1m.sh
# partisync 100万资产端到端索引与低延迟搜索验证脚本
# 交付物对应：T6-02 / T6-04 审计项

set -euo pipefail

MEILI_URL="${MEILI_URL:-http://localhost:7700}"
MEILI_KEY="${MEILI_KEY:-partisync_master_key_for_dev_only_32_chars_long}"
TARGET_COUNT="${TARGET_COUNT:-1000000}"
CONCURRENCY="${CONCURRENCY:-50}"
REQUESTS="${REQUESTS:-5000}"
BATCH_SIZE="${BATCH_SIZE:-5000}"
REPORT_FILE="${REPORT_FILE:-docs/evidence/benchmark-1m.md}"

echo "=========================================================="
echo "    partisync 1M Asset Benchmark & Verification Suite     "
echo "=========================================================="
echo "MeiliSearch URL : $MEILI_URL"
echo "Target Docs     : $TARGET_COUNT"
echo "Search Clients  : $CONCURRENCY"
echo "Search Requests : $REQUESTS"
echo "Batch Size      : $BATCH_SIZE"
echo "Report Destination: $REPORT_FILE"
echo "----------------------------------------------------------"

# 1. 检查 MeiliSearch 状态
echo "[1/4] Checking MeiliSearch health..."
HEALTH=$(curl -s -f "$MEILI_URL/health" || true)
if [[ "$HEALTH" != *"available"* ]]; then
  echo "ERROR: MeiliSearch at $MEILI_URL is not available! Health output: $HEALTH"
  exit 1
fi
echo "MeiliSearch is healthy: $HEALTH"

# 2. 获取当前文档数
STATS=$(curl -s -H "Authorization: Bearer $MEILI_KEY" "$MEILI_URL/indexes/assets/stats" || echo "{}")
CURRENT_COUNT=$(echo "$STATS" | grep -o '"numberOfDocuments":[0-9]*' | cut -d: -f2 || echo "0")
if [ -z "$CURRENT_COUNT" ]; then
  CURRENT_COUNT=0
fi
echo "Current indexed documents: $CURRENT_COUNT"

START_INDEX=0
ONLY_SEARCH="false"

if [ "$CURRENT_COUNT" -ge "$TARGET_COUNT" ]; then
  echo "Index already has $CURRENT_COUNT documents (>= target $TARGET_COUNT). Will skip indexing and run search benchmark directly."
  ONLY_SEARCH="true"
elif [ "$CURRENT_COUNT" -gt 0 ]; then
  echo "Index has $CURRENT_COUNT documents. Will append documents from index $CURRENT_COUNT up to $TARGET_COUNT."
  START_INDEX=$CURRENT_COUNT
else
  echo "Index is empty or new. Will index $TARGET_COUNT documents from scratch."
  START_INDEX=0
fi

# 3. 编译并执行压测程序
echo "[2/4] Executing benchmark binary..."
TMP_OUTPUT=$(mktemp)
BIN_DIR=$(mktemp -d)
BENCH_BIN="$BIN_DIR/bench"

export GOCACHE="${GOCACHE:-/tmp/go-cache}"
mkdir -p "$GOCACHE"

echo "Building benchmark binary..."
go build -o "$BENCH_BIN" ./cmd/bench

CMD_ARGS=(
  "--meili-url=$MEILI_URL"
  "--meili-key=$MEILI_KEY"
  "--count=$TARGET_COUNT"
  "--start-index=$START_INDEX"
  "--concurrency=$CONCURRENCY"
  "--requests=$REQUESTS"
  "--batch-size=$BATCH_SIZE"
)

if [ "$ONLY_SEARCH" = "true" ]; then
  CMD_ARGS+=("--only-search=true")
fi

echo "Running: $BENCH_BIN ${CMD_ARGS[*]}"
if "$BENCH_BIN" "${CMD_ARGS[@]}" 2>&1 | tee "$TMP_OUTPUT"; then
  echo "Benchmark execution finished successfully."
else
  echo "ERROR: Benchmark execution failed!"
  rm -rf "$TMP_OUTPUT" "$BIN_DIR"
  exit 1
fi
rm -rf "$BIN_DIR"

# 4. 生成验证报告并写入 docs/evidence/benchmark-1m.md
echo "[3/4] Generating verification report..."
mkdir -p "$(dirname "$REPORT_FILE")"

FINAL_STATS=$(curl -s -H "Authorization: Bearer $MEILI_KEY" "$MEILI_URL/stats" || echo "{}")
DB_SIZE=$(echo "$FINAL_STATS" | grep -o '"databaseSize":[0-9]*' | cut -d: -f2 || echo "0")
DB_SIZE_MB=$(awk "BEGIN {printf \"%.2f\", $DB_SIZE / 1024 / 1024}")
TOTAL_DOCS=$(echo "$FINAL_STATS" | grep -o '"numberOfDocuments":[0-9]*' | cut -d: -f2 || echo "0")

DATE_STR=$(date -u +"%Y-%m-%d %H:%M:%S UTC")

cat <<EOF > "$REPORT_FILE"
# 100万资产检索性能基准测试报告 (1M Asset Benchmark)

- **测试日期**: $DATE_STR
- **脚本入口**: \`scripts/benchmark-1m.sh\` (实现项 T6-02)
- **验证项**: 解决审计 T6-04（100万资产 p95 < 100ms 实测证据）
- **MeiliSearch 实例**: $MEILI_URL
- **索引资产总数**: $TOTAL_DOCS
- **LMDB 数据库体积**: ${DB_SIZE_MB} MB
- **并发客户端数**: $CONCURRENCY
- **请求总数**: $REQUESTS

---

## 1. 压测完整执行日志

\`\`\`text
$(cat "$TMP_OUTPUT")
\`\`\`

---

## 2. 指标提取与 MCD 达标判定

从基准测试结果中提炼核心指标：
$(grep -E "(RPS|p50|p90|p95|p99|min|max|avg|PASS|FAIL)" "$TMP_OUTPUT" | sed 's/^/- /')

- **MCD 目标**: 100 万资产搜索 p95 < 100ms
- **实测判定**: $(if grep -q "PASS: p95" "$TMP_OUTPUT"; then echo "✅ **通过 (PASS)** — 实测 p95 严格小于 100ms，达成 MCD 工业级性能指标。"; else echo "❌ **未通过 (FAIL)**"; fi)

EOF

rm -f "$TMP_OUTPUT"
echo "[4/4] Done! Report written to $REPORT_FILE."
