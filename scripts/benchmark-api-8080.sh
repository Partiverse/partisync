#!/usr/bin/env bash
# scripts/benchmark-api-8080.sh
# B-02（audit-t6-prime）：8080 产品链路（client → server API → Meilisearch → JSON）延迟基准。
# 与 scripts/benchmark-1m.sh（直连 7700）互补：本脚本测的是真实产品入口。
#
# 口径（与 B-01 修正一致）：
#   - 输出的是「指定并发饱和下的客户端观测延迟」，不是单请求检索延迟；
#   - 另取 20 个单请求 curl 分解（DNS/连接/首字节），给出低负载参照。
#
# 前置：docker compose up -d（server healthy）；Meili 索引已含基准语料（先跑 benchmark-1m.sh）。
# 运行：bash scripts/benchmark-api-8080.sh [并发] [请求数]
set -euo pipefail

BASE="${BASE:-http://127.0.0.1:8080}"
CONCURRENCY="${1:-50}"
REQUESTS="${2:-5000}"
QUERY="${QUERY:-asset_}"
REPORT="${REPORT:-docs/evidence/benchmark-api-8080.md}"

say() { printf '\n=== %s ===\n' "$*"; }

say "预检"
HEALTH=$(curl -s -f "$BASE/healthz" || true)
[[ "$HEALTH" == *"ok"* ]] || { echo "server not healthy at $BASE"; exit 1; }
TOTAL=$(curl -s "$BASE/api/v1/assets?q=$QUERY&limit=1" | jq -r '.total // 0')
echo "server=$BASE query=$QUERY hits=$TOTAL concurrency=$CONCURRENCY requests=$REQUESTS"
[[ "$TOTAL" -gt 1000 ]] || echo "WARN: hits <= 1000，结果可能不代表大规模索引（先跑 benchmark-1m.sh）"

say "单请求分解（20 次，低负载参照）"
for i in $(seq 1 20); do
  curl -so /dev/null -w '%{time_namelookup} %{time_connect} %{time_starttransfer} %{time_total}\n' \
    "$BASE/api/v1/assets?q=$QUERY&limit=20" || true
done > /tmp/b8080-single.txt
awk '{t=$4; n++; if(t<mn||n==1)mn=t; if(t>mx)mx=t; s+=t} END{printf "single: min=%.1fms avg=%.1fms max=%.1fms (n=%d)\n", mn*1000, s/n*1000, mx*1000, n}' /tmp/b8080-single.txt

say "饱和压测（$CONCURRENCY 并发 × $REQUESTS 请求）"
TMP=$(mktemp)
for i in $(seq 1 "$REQUESTS"); do
  echo "$i"
done | xargs -P "$CONCURRENCY" -I{} curl -so /dev/null -w '%{http_code} %{time_total}\n' \
  "$BASE/api/v1/assets?q=$QUERY&limit=20&offset=$(( (RANDOM % 10) * 20 ))" > "$TMP"

echo "request summary:"; cat > /tmp/b8080-report.py <<'PY'
import sys
codes, times = {}, []
for line in open(sys.argv[1]):
    parts = line.split()
    if len(parts) == 2:
        codes[parts[0]] = codes.get(parts[0], 0) + 1
        times.append(float(parts[1]) * 1000)
times.sort()
def pct(p):
    i = min(len(times) - 1, int(len(times) * p / 100))
    return times[i]
print(f"requests={len(times)}")
print("codes:", dict(sorted(codes.items())))
print(f"p50={pct(50):.2f}ms p90={pct(90):.2f}ms p95={pct(95):.2f}ms p99={pct(99):.2f}ms")
ok = codes.get("200", 0)
print(f"success_rate={ok/len(times)*100:.2f}%" if times else "no data")
PY
python3 /tmp/b8080-report.py "$TMP" | tee /tmp/b8080-summary.txt

say "写报告"
{
  echo "# 8080 产品链路延迟基准（B-02，$(date '+%F %H:%M:%S')）"
  echo
  echo "- 链路：client → partisync-server :8080 \`GET /api/v1/assets\`（q=$QUERY, limit=20, 随机 offset 前 10 页）→ Meilisearch → JSON"
  echo "- 负载：$CONCURRENCY 并发 × $REQUESTS 请求；命中 $TOTAL 条基准语料"
  echo "- 口径：**指定并发饱和下的客户端观测延迟**（含 API 序列化与网络），非单请求检索延迟（B-01 修正）"
  echo
  echo '```'
  cat /tmp/b8080-summary.txt
  echo '```'
  echo
  echo "- 单请求低负载参照（20 次 min/avg/max，见运行日志）：MCD 目标 p95 < 100ms 在**两种口径下**均需满足才算完整。"
} > "$REPORT"
rm -f "$TMP" /tmp/b8080-single.txt /tmp/b8080-report.py /tmp/b8080-summary.txt
echo "report: $REPORT"
