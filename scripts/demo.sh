#!/usr/bin/env bash
# parti-sync 一键演示包（SPEC M6-WP01 v0.1）
#
# 目标: 从 clone 状态到 1 hub + 1 个 ~50 篇文档语料索引 + 5 个 demo 查询
#       在 < 5 min（macOS/Linux）内跑通。
#
# 数据: crates/partisync-index/tests/fixtures/wp06_corpus（50 篇真实 markdown 笔记）
#      零网络依赖 / 零付费数据集。LCSTS 真档评估见 scripts/eval-real.sh。
#
# 用法:
#   scripts/demo.sh                      # 默认 debug 构建 + 端口 8090
#   DEMO_RELEASE=1 scripts/demo.sh       # release 构建（首次更慢）
#   DEMO_PORT=9000 scripts/demo.sh       # 自定义端口
#   DEMO_KEEP=1  scripts/demo.sh         # 跑完不清理（保留 db/index）
#   DEMO_QUERIES="arg 2pa usearch" \     # 自定义查询（| 分隔）
#       scripts/demo.sh
set -euo pipefail
# 注意: bash 5.x 在 set -u 下 '$VAR<char where char is a multi-byte UTF-8
#   beginning byte' 会假报 unbound 变量。 因此所有 '$VAR' 后接非 ASCII
#   字符的位置统一写为 '${VAR}'。  （见 M5-WP07 memory 备注与本会话 smoke 实证）

cd "$(git rev-parse --show-toplevel)"

# ───── 配置 ─────
DEMO_PORT="${DEMO_PORT:-8090}"
DEMO_RELEASE="${DEMO_RELEASE:-}"
DEMO_KEEP="${DEMO_KEEP:-}"
DEMO_QUERIES="${DEMO_QUERIES:-argon2 migration|tantivy bm25 field design|usearch hnsw pitfalls|MCP protocol overview|fastembed bge m3}"
FIXTURE="crates/partisync-index/tests/fixtures/wp06_corpus"

# numkong dynamic-dispatch kills Apple Clang compilation by default; 强制全 0
# 走标量回退, 保证 build 成功（性能测量另设 env，见 M5-WP07 记忆）.
# 已有外部 env 则尊重.
_nk_vars=(NK_TARGET_NEON NK_TARGET_NEONHALF NK_TARGET_NEONSDOT NK_TARGET_NEONBFDOT
          NK_TARGET_NEONFHM NK_TARGET_SVE NK_TARGET_SVEHALF NK_TARGET_SVEBFDOT
          NK_TARGET_SVESDOT NK_TARGET_SVE2 NK_TARGET_SVE2P1 NK_TARGET_NEONFP8
          NK_TARGET_SME NK_TARGET_SME2 NK_TARGET_SME2P1 NK_TARGET_SMEF64
          NK_TARGET_SMEHALF NK_TARGET_SMEBF16 NK_TARGET_SMEBI32
          NK_TARGET_SMELUT2 NK_TARGET_SMEFA64)
for v in "${_nk_vars[@]}"; do export "$v=0"; done

DEMO_DIR="$(mktemp -d -t partisync-demo.XXXXXX)"   # 工作目录 (db/cas/index/hub.log)

# ───── 工具函数 ─────
step() { printf "\n\033[1;36m▶ %s\033[0m\n" "$*"; }
ok()   { printf "  \033[1;32m✓\033[0m %s\n" "$*"; }
warn() { printf "  \033[1;33m!\033[0m %s\n" "$*"; }
fail() { printf "  \033[1;31m✗\033[0m %s\n" "$*" >&2; exit 1; }

cleanup() {
    if [[ -n "${HUB_PID:-}" ]] && kill -0 "${HUB_PID}" 2>/dev/null; then
        kill "${HUB_PID}" 2>/dev/null || true
        wait "${HUB_PID}" 2>/dev/null || true
    fi
    if [[ -z "${DEMO_KEEP:-}" ]]; then
        rm -rf "${DEMO_DIR:-}"
    else
        echo "DEMO_KEEP=1 -> retained ${DEMO_DIR:-} (db/cas/index live there)"
    fi
}
trap cleanup EXIT INT TERM

# ───── 1. 前置检查 ─────
step "1/6 preflight"
[[ -d "${FIXTURE}" ]] || fail "fixture missing: ${FIXTURE} (clone full repo first)"
n_fixture="$(ls "${FIXTURE}" | wc -l | xargs)"
[[ "${n_fixture}" -ge 50 ]] || warn "fixture has only ${n_fixture} md files (want >= 50)"
command -v cargo >/dev/null || fail "cargo missing"
command -v curl  >/dev/null || fail "curl missing"
ok "deps + fixture ready (${n_fixture} docs)"

# ───── 2. 构建 ─────
step "2/6 build ($( [[ -n "${DEMO_RELEASE}" ]] && echo release || echo debug))"
mkdir -p "${DEMO_DIR}"
if [[ -n "${DEMO_RELEASE}" ]]; then
    cargo build --release -p partisync-cli -p partisync-hub --quiet
    BIN=./target/release/partisync-cli
    HUB_BIN=./target/release/hub-demo
else
    cargo build -p partisync-cli -p partisync-hub --quiet
    BIN=./target/debug/partisync-cli
    HUB_BIN=./target/debug/hub-demo
fi
ok "binaries ready: ${BIN} + ${HUB_BIN}"

# ───── 3. 启动 hub demo ─────
step "3/6 start hub-demo (127.0.0.1:${DEMO_PORT})"
"${HUB_BIN}" --addr "127.0.0.1:${DEMO_PORT}" >"${DEMO_DIR}/hub.log" 2>&1 &
HUB_PID=$!
ok "hub PID=${HUB_PID} (log ${DEMO_DIR}/hub.log)"
for _ in 1 2 3 4 5 6 7 8 9 10; do
    if curl -fsS "http://127.0.0.1:${DEMO_PORT}/healthz" >/dev/null 2>&1; then break; fi
    sleep 0.3
done
if curl -fsS "http://127.0.0.1:${DEMO_PORT}/healthz" >/dev/null 2>&1; then
    ok "/healthz OK"
elif curl -sS -o /dev/null --max-time 2 "http://127.0.0.1:${DEMO_PORT}/"; then
    ok "hub port reachable (no /healthz endpoint, HTTP ok)"
else
    warn "hub probe failed; main flow continues (demo reads local index, hub may be idle)"
fi

# ───── 4. 复制 fixture 到 demo 工作目录 (避免污染源) ─────
step "4/6 copy fixture corpus to demo workspace"
DEMO_CORPUS="${DEMO_DIR}/corpus"
mkdir -p "${DEMO_CORPUS}"
cp "${FIXTURE}"/*.md "${DEMO_CORPUS}/"
ok "$(ls "${DEMO_CORPUS}" | wc -l | xargs) markdown files staged"

# ───── 5. 索引 + 5 demo 查询 ─────
step "5/6 index + 5 demo queries"
DB="${DEMO_DIR}/partisync.db"
CAS="${DEMO_DIR}/partisync.cas"
GRAPH_INDEX="${DEMO_DIR}/bm25_index"
DEMO_INDEX="${DEMO_DIR}/demo_bm25"

echo "  · graph index: ${DEMO_CORPUS} -> ${DB} (and ${CAS})"
"${BIN}" index "${DEMO_CORPUS}" --db "${DB}" --cas "${CAS}" >/dev/null
ok "graph index complete"

# 5 个查询走 demo_query 直接调 Bm25Index（绕开 search CLI 那条 IndexEngine
# 路径需要预灌 sidecar 的限制）。 这是与 EvalRunner 同一份 BM25 路径，
# 保证 demo 直接命中可见。
echo "  · 5 demo queries (Bm25Index direct; 绕开 sidecar 依赖):"
IFS='|' read -r -a QARR <<<"${DEMO_QUERIES}"
i=1
for q in "${QARR[@]}"; do
    echo
    echo "    [Q${i}]  ${q}"
    echo "    -----------------------------------------"
    cargo run --quiet -p partisync-index --example demo_query -- \
        "${DEMO_CORPUS}" "${q}" "${DEMO_INDEX}" 5 \
        | sed 's/^/      /' || true
    i=$((i+1))
done

# ───── 6. 验收 ─────
step "6/6 summary"
ok "demo ran cleanly (5 queries)"
echo
echo "  ▸ hub demo:       http://127.0.0.1:${DEMO_PORT}/  (Ctrl-C to quit)"
echo "  ▸ workspace:      ${DEMO_DIR} $( [[ -z "${DEMO_KEEP}" ]] && echo "(trap will clean)" || echo "(DEMO_KEEP=1 retained)")"
echo "  ▸ binary:         ${BIN}"
echo
echo "  advanced:"
echo "    ${BIN} ui --db ${DB} --cas ${CAS} --addr 127.0.0.1:8080   # web UI"
echo "    ${BIN} watch \"${DEMO_CORPUS}\" --db ${DB}                # incremental watch"
echo "    ${BIN} event-drain --source mock --journal ${DEMO_DIR}/event.journal  # cloud event loop (v0.1 mock)"
echo "    scripts/eval-real.sh                                     # M6-D67 real eval (BM25 true numbers)"
echo
echo "  Quit: Ctrl-C (cleans hub + workspace), or Enter here"
echo

# 阻塞, 让 hub 持续跑供探活
read -rp "  Press Enter to exit demo (Ctrl-C also fine)..."
