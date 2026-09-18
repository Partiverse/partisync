#!/usr/bin/env bash
# 安装 git 钩子（一次性）：./scripts/install-hooks.sh
set -euo pipefail
root="$(git rev-parse --show-toplevel)"
cp "${root}/scripts/hooks/commit-msg" "${root}/.git/hooks/commit-msg"
chmod +x "${root}/.git/hooks/commit-msg"
echo "installed: .git/hooks/commit-msg"
