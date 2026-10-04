# fixtures 签名文件（[P21] / SPEC M9-WP04 §2.3）

`*.minisig` 均由 **test-only** keypair（`test-signing.pub` /
`test-signing.key`，无口令，pub 注释显式标注非生产钥）对同名 fixture
字节一次性签署（minisign CLI，2026-10-04）。该钥**不被任何锚集信任**
（`ANCHOR_PUBKEYS` 仅含发布双钥），仅供测试经
`ExtTool::load_with_anchors` 注入；生产钥材料零接触（key-custody）。

- `demo_tool.minisig` / `probe_deny.minisig` / `spin_loop.minisig`：
  既有 fixture 补签（存量测试经测试钥签名路径零回归，SPEC §4）；
- `garbage.wasm` + `garbage.minisig`：签名合法但字节非法的探针对照
  （验签放行 → 编译必拒，探针⑤反证）。

重新签署（fixture 变更时）：`minisign -S -s test-signing.key -m
<fixture> -x <stem>.minisig -t "PartiSync test-only signature (fixtures;
not production)"`。
