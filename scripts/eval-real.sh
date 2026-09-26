#!/usr/bin/env bash
# M6-D67 真实评估驱动器（SPEC M6-D67 v0.1）
#
# 流：
#   1. 准备评估集（EVAL_INPUT 真档 → prepare-lcsts.py； 退化 → wp06 fixture）
#   2. cargo run -p partisync-index --example eval_real 跑 EvalRunner
#   3. 落盘 eval.json + 简短 KPI 表
#
# 与旧版的差异： 不再依赖 wp06_eval.rs 的内部 stdout 格式。
#
# 用法:
#   scripts/eval-real.sh                                          # 退化到 wp06 fixture
#   EVAL_INPUT=/path/to/lcsts.json scripts/eval-real.sh           # LCSTS 真档
#   EVAL_DOCS=200 EVAL_QUERIES=40 scripts/eval-real.sh             # 子集大小
#   EVAL_OUT=docs/reports/bench/eval.json scripts/eval-real.sh     # 输出位置
#   EVAL_MODE=hybrid_no_rerank scripts/eval-real.sh                # 跑 hybrid（feature-gated）
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

# ───── 配置 ─────
EVAL_DOCS="${EVAL_DOCS:-200}"
EVAL_QUERIES="${EVAL_QUERIES:-40}"
EVAL_K="${EVAL_K:-10}"
EVAL_MODE="${EVAL_MODE:-bm25_only}"
EVAL_OUT="${EVAL_OUT:-$(mktemp -t eval-real.XXXXXX.json)}"
WORK="$(mktemp -d -t eval-real-work.XXXXXX)"
INDEX_DIR="$WORK/bm25_index"

# numkong cc issue workaround（参见 M5-WP07 记忆）
_nk_vars=(NK_TARGET_NEON NK_TARGET_NEONHALF NK_TARGET_NEONSDOT NK_TARGET_NEONBFDOT
          NK_TARGET_NEONFHM NK_TARGET_SVE NK_TARGET_SVEHALF NK_TARGET_SVEBFDOT
          NK_TARGET_SVESDOT NK_TARGET_SVE2 NK_TARGET_SVE2P1 NK_TARGET_NEONFP8
          NK_TARGET_SME NK_TARGET_SME2 NK_TARGET_SME2P1 NK_TARGET_SMEF64
          NK_TARGET_SMEHALF NK_TARGET_SMEBF16 NK_TARGET_SMEBI32
          NK_TARGET_SMELUT2 NK_TARGET_SMEFA64)
for v in "${_nk_vars[@]}"; do export "$v=0"; done

step() { printf "\n\033[1;36m▶ %s\033[0m\n" "$*"; }
ok()   { printf "  \033[1;32m✓\033[0m %s\n" "$*"; }
warn() { printf "  \033[1;33m!\033[0m %s\n" "$*"; }
fail() { printf "  \033[1;31m✗\033[0m %s\n" "$*" >&2; exit 1; }
trap 'rm -rf "${WORK:-}"' EXIT INT TERM

# ───── 1. 准备数据 ─────
step "1/3 准备评估数据"
DATASET_LABEL=""

if [[ -n "${EVAL_INPUT:-}" ]] && [[ -f "${EVAL_INPUT}" ]]; then
    DATASET_LABEL="lcsts ($(basename "$EVAL_INPUT"))"
    ok "使用 EVAL_INPUT=${EVAL_INPUT}（LCSTS 真档）"
    command -v python3 >/dev/null || fail "python3 未安装（LCSTS 抽取需要）"
    SUBSET="$WORK/subset"
    python3 scripts/prepare-lcsts.py "$EVAL_INPUT" "$SUBSET" \
        --docs "$EVAL_DOCS" --queries "$EVAL_QUERIES"
    CORPUS_DIR="$SUBSET/corpus"
    CORPUS_TSV="$SUBSET/corpus.tsv"
    QUERIES="$SUBSET/queries.jsonl"
    QRELS="$SUBSET/qrels.jsonl"
elif [[ -f crates/partisync-index/tests/fixtures/wp06_corpus.tsv \
     && -f crates/partisync-index/tests/fixtures/wp06_queries.jsonl \
     && -f crates/partisync-index/tests/fixtures/wp06_qrels.jsonl ]]; then
    DATASET_LABEL="wp06 fixture (M4-WP06 合成基线)"
    warn "EVAL_INPUT 未提供 / LCSTS 真档不可达， 退化为 wp06 fixture"
    CORPUS_DIR="crates/partisync-index/tests/fixtures/wp06_corpus"
    CORPUS_TSV="crates/partisync-index/tests/fixtures/wp06_corpus.tsv"
    QUERIES="crates/partisync-index/tests/fixtures/wp06_queries.jsonl"
    QRELS="crates/partisync-index/tests/fixtures/wp06_qrels.jsonl"
else
    fail "既无 EVAL_INPUT 也无 wp06 fixture 可用"
fi

mkdir -p "$INDEX_DIR" "$(dirname "$EVAL_OUT")"
ok "数据就绪（corpus=$(ls "$CORPUS_DIR" | wc -l | xargs) markdown）"

# ───── 2. 跑 EvalRunner ─────
step "2/3 跑 EvalRunner (mode=$EVAL_MODE)"
# hybrid 模式需要 fastembed feature
if [[ "$EVAL_MODE" != "bm25_only" ]]; then
    FEATURES="--features index-embed"
else
    FEATURES=""
fi

# 单测或 examples 都可以跑； examples 是干净的「单二进制」入口
cargo run --quiet -p partisync-index --example eval_real $FEATURES -- \
    --k "$EVAL_K" \
    --mode "$EVAL_MODE" \
    --corpus-dir "$CORPUS_DIR" \
    --corpus-tsv "$CORPUS_TSV" \
    --queries "$QUERIES" \
    --qrels "$QRELS" \
    --index-dir "$INDEX_DIR" \
    --out "$EVAL_OUT" \
    | tee "$WORK/run.txt" \
    | grep -E '"(mean|num)' | head -20 || true
ok "EvalRunner 完成 → ${EVAL_OUT}"

# ───── 3. KPI 摘要 ─────
step "3/3 KPI 摘要"
python3 - "$EVAL_OUT" "$DATASET_LABEL" <<'PY'
import json, sys
out_path, label = sys.argv[1], sys.argv[2]
data = json.load(open(out_path, encoding="utf-8"))
print(f"  数据集:    {label}")
print(f"  模式:      {data.get('mode','?')}  K={data.get('k','?')}")
print(f"  查询数:    {data.get('num_queries',0)}")
r = data.get("mean_recall_at_k", 0.0)
m = data.get("mean_mrr", 0.0)
n = data.get("mean_ndcg_at_k", 0.0)
print(f"  Recall@{data.get('k')}: {r:.4f}")
print(f"  MRR:        {m:.4f}")
print(f"  nDCG@{data.get('k')}:   {n:.4f}")
PY
ok "完成 → ${EVAL_OUT}"
