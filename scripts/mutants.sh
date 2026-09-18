#!/usr/bin/env bash
# 变异抽检（SPEC M0-WP00，D5）：对 partisync-core 运行 cargo-mutants。
# 注：mutants.toml 的 examine_globs 在 v27.1.0 未生效（已核验 config.rs 键名存在，
# 疑似 CLI 覆盖语义），故范围用 -f glob 显式声明——以实测行为为准。
# 安全：默认在临时副本目录运行变异（禁止 --in-place）——
# in-place 会临时改写工作区源码，与并行 git 操作冲突（M0-WP03 实际踩坑）。
# 用法: scripts/mutants.sh
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
exec cargo mutants \
  -f 'crates/partisync-core/src/**/*.rs' \
  -f 'crates/partisync-cas/src/**/*.rs' \
  -e 'xtask/*' -e 'crates/partisync-cli/*' -e 'crates/partisd/*' \
  --timeout 90 "$@"
