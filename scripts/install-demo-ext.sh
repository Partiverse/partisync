#!/usr/bin/env bash
# 安装示例扩展到扩展目录（SPEC M7-WP01 §2.2 发现约定；M12-WP03 §2.2
# 增签名件拷贝与缺签显式告警）。
# 用法：bash scripts/install-demo-ext.sh
# 之后重启 partisync-mcp（或桌面壳），扩展面板 / mcp_call("ext_list") 可见。
set -euo pipefail

SRC="$(cd "$(dirname "$0")/.." && pwd)/examples/extensions"
DEST="${HOME}/.partisync/extensions"

mkdir -p "$DEST"
cp "$SRC/demo_ext.wasm" "$DEST/demo_ext.wasm"
cp "$SRC/demo_ext.json" "$DEST/demo_ext.json"

if [ -f "$SRC/demo_ext.minisig" ]; then
  cp "$SRC/demo_ext.minisig" "$DEST/demo_ext.minisig"
  echo "installed: $DEST/demo_ext.{wasm,json,minisig}"
else
  # [P21] 强制验签下无签扩展必被拒载（registry.rs Unsigned）——显式告警
  # 不静默（SPEC M12-WP03 §2.2）。
  echo "warning: 仓内无 demo_ext.minisig——[P21] 强制验签将拒载本扩展（预期行为；闭合动作见 docs/release/EXT-SIGNING.md「示例扩展签名窗口」节）" >&2
  echo "installed: $DEST/demo_ext.{wasm,json}（无签名件）"
fi
echo "next: restart partisync-mcp (or the desktop shell), then check the 扩展 tab / mcp_call ext_list."
