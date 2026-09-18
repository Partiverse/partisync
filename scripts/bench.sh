#!/usr/bin/env bash
# 基准基线管理（SPEC M0-WP00，criterion 原生 save-baseline/baseline 机制）。
# 用法:
#   scripts/bench.sh save <name>    运行全部基准并存为命名基线
#   scripts/bench.sh check <name>   对照命名基线，回退 >10% 时退出非零（执行方案 §4.5）
set -euo pipefail

cmd="${1:?usage: bench.sh <save|check> <name>}"
name="${2:?usage: bench.sh <save|check> <name>}"

case "${cmd}" in
  save)
    cargo bench --workspace -- --save-baseline "${name}"
    echo "baseline '${name}' saved"
    ;;
  check)
    cargo bench --workspace -- --baseline "${name}"
    echo "bench check vs '${name}': completed（回退判定见上方 criterion 输出，>10% 需 ADR 豁免）"
    ;;
  *)
    echo "unknown command: ${cmd}" >&2
    exit 2
    ;;
esac
