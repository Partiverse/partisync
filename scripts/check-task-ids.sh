#!/usr/bin/env bash
# 校验提交区间内每个非合并提交都携带合规 Task-ID trailer（铁律 2 / SPEC M-1-WP05）。
# 用法: scripts/check-task-ids.sh <base> <head>
set -euo pipefail

base="${1:?usage: check-task-ids.sh <base> <head>}"
head="${2:?usage: check-task-ids.sh <base> <head>}"

pattern='^Task-ID: M-?[0-9]+-(WP[0-9]{2}|D[0-9]+)-T[0-9]{2}$'
fail=0
count=0

for rev in $(git rev-list --no-merges "${base}..${head}"); do
  count=$((count + 1))
  body="$(git log -1 --format=%B "${rev}")"
  if ! grep -qE "${pattern}" <<<"${body}"; then
    echo "FAIL: ${rev} ($(git log -1 --format=%s "${rev}")) 缺少合规 Task-ID trailer" >&2
    fail=1
  fi
done

if [[ ${fail} -ne 0 ]]; then
  exit 1
fi
echo "task-id check: OK（${count} commits）"
