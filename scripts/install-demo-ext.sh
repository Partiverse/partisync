#!/usr/bin/env bash
# 安装示例扩展到扩展目录（SPEC M7-WP01 §2.2 发现约定）。
# 用法：bash scripts/install-demo-ext.sh
# 之后重启 partisync-mcp（或桌面壳），扩展面板 / mcp_call("ext_list") 可见。
set -euo pipefail

SRC="$(cd "$(dirname "$0")/.." && pwd)/examples/extensions"
DEST="${HOME}/.partisync/extensions"

mkdir -p "$DEST"
cp "$SRC/demo_ext.wasm" "$DEST/demo_ext.wasm"
cp "$SRC/demo_ext.json" "$DEST/demo_ext.json"

echo "installed: $DEST/demo_ext.{wasm,json}"
echo "next: restart partisync-mcp (or the desktop shell), then check the 扩展 tab / mcp_call ext_list."
