#!/usr/bin/env bash
# Task-ID trailer 正则单元测试——SPEC M6-WP02 验收。
# 与 scripts/hooks/commit-msg + scripts/check-task-ids.sh 同源正则字面；
# 失败列表（含接受与拒绝两类）见 SPEC §验收。
# 用法: bash scripts/check-task-id-format.sh
# 退出 0 = 全绿，非 0 = 有用例失败。
set -euo pipefail

# 与 hooks/commit-msg:6 一致的字面量；改一处必须改三处（commit-msg、
# check-task-ids.sh、本测试顶部）。
pattern='^Task-ID: M-?[0-9]+-(WP[0-9]{2}|D[0-9]+)-T[0-9]{2}$'

pass=0
fail=0
fails=()

# 用 printf %s\\n 而非 echo，避免多字节或前缀 -n 误判
assert_accept() {
  local body="$1"
  local label="$2"
  if printf '%s\n' "${body}" | grep -qE "${pattern}"; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
    fails+=("ACCEPT 失败: ${label} | body=${body}")
  fi
}

assert_reject() {
  local body="$1"
  local label="$2"
  if printf '%s\n' "${body}" | grep -qE "${pattern}"; then
    fail=$((fail + 1))
    fails+=("REJECT 失败: ${label} | body=${body}")
  else
    pass=$((pass + 1))
  fi
}

# === 接受类（既有 WP） ===
assert_accept 'Task-ID: M0-WP01-T07' 'M0 WP 任务'
assert_accept 'Task-ID: M2-WP09-T02' 'M2 WP 任务（双位）'
assert_accept 'Task-ID: M5-WP01-T01' 'M5 WP 任务'
assert_accept 'Task-ID: M-1-WP07-T01' 'M-1 阶段 WP 任务'
assert_accept 'Task-ID: M-1-WP05-T03' 'M-1 阶段 WP 任务（钩子源）'

# === 接受类（新 D 前缀） ===
assert_accept 'Task-ID: M6-D67-T01' 'M6 评估档 D67（首个用例）'
assert_accept 'Task-ID: M6-D6-T01' 'M6 评估档 D6 单位'
assert_accept 'Task-ID: M6-D123-T01' 'M6 评估档 D123 三位'
assert_accept 'Task-ID: M6-D1-T01' 'M6 评估档 D1'

# === 接受类（squash merge trailer，trailer 不在首行） ===
# 注：本测试仅校验正则；commit-msg 钩子只在首行匹配。squash merge 的 trailer
# 多行场景由 xtask trace 启发式处理，不在本钩子覆盖范围。
assert_reject 'this is a commit' '无 Task-ID 行'
assert_reject 'Task-ID:' '空 Task-ID 行'
assert_reject 'Task-ID: X0-WP01-T01' '错误前缀 X'
assert_reject 'Task-ID: M0-WP1-T01' 'WP 后一位数字'
assert_reject 'Task-ID: M0-WP123-T01' 'WP 后三位数字'
assert_reject 'Task-ID: M0-WP01-T1' 'T 后一位数字'
assert_reject 'Task-ID: M0-WP01-T123' 'T 后三位数字'
assert_reject 'Task-ID: M6-DAB-T01' 'D 后非数字'
assert_reject 'Task-ID: M6-D-T01' 'D 后无数字'
assert_reject 'Task-ID: M0WP01-T01' '缺分隔符 -'
assert_reject 'Task-ID: M0-WP01T07' 'T 前缺分隔符'

echo "Task-ID regex unit: ${pass} passed, ${fail} failed"
if [[ ${fail} -ne 0 ]]; then
  printf '  %s\n' "${fails[@]}" >&2
  exit 1
fi