# Task: M12-WP03-T02 示例扩展签名腿实施（签名工作流 + 脚本 + e2e 三态守卫）

## 任务卡

| 项 | 内容 |
|---|---|
| **Task-ID** | M12-WP03-T02 |
| **类型** | 实施（CI 工作流 + 脚本 + 测试守卫，R1；SPEC docs/specs/M12-WP03.md 已批准 #252） |
| **来源** | M12-WP03 SPEC §2.1/§2.2/§2.3/§2.4 + charter WP03 |
| **创建日期** | 2026-10-11 |

## 交付

分两个 PR（本卡挂全程）：

**PR-a（本提交）**：①`.github/workflows/sign-demo-ext.yml`（新）：workflow_dispatch 手动触发；install minisign（release.yml:178-185 同款）→ **drift-check 步**（已入仓 minisig 对当前 wasm 的 `minisign -V` 防漂移，SPEC §2.4）→ sign 步（CI 子钥 + trusted comment 沿 release.yml:197-199 + **签后即时回验**，输出即 SPEC §3 验收 1 的证据）→ artifact 上传；secret 未配置显式 fail 不假绿。私钥仅 GitHub secret 注入（ADR-0027）。②`scripts/install-demo-ext.sh`：minisig 拷贝 + 缺签 stderr 显式告警（SPEC §2.2）。③`commands.rs mcp_call_real_sidecar_ext_list` 三态守卫（SPEC §2.3）：无安装 skip（现状）/有装无签 skip-with-reason（stderr 文案指 EXT-SIGNING）/有装有签真实断言。

**PR-b（随签名往返）**：`demo_ext.wasm.minisig` 入仓 + EXT-SIGNING「示例扩展签名窗口」节回填闭合注记 + 真装载链证据归档（docs/reviews/M12-WP03-t02-evidence.txt）。

## 涉及文件清单（Iron Rule 9）

.github/workflows/sign-demo-ext.yml（新）· scripts/install-demo-ext.sh · crates/partisync-desktop/tests/commands.rs · docs/tasks/M12-WP03-T02-impl.md（本卡）；PR-b 增 examples/extensions/demo_ext.wasm.minisig · docs/release/EXT-SIGNING.md · docs/reviews/M12-WP03-t02-evidence.txt

## 验收

- [ ] minisig 入仓且签后回验通过（工作流 run log 输出 SIGN-AND-VERIFY=ok 归档）；
- [ ] install 脚本执行后四件齐，真 sidecar 面 ext_list 列出 ext_demo_echo（desktop e2e 全链 PASS 实录归档，沿 D19 修订验证面）；
- [ ] e2e 三态行为正确（无安装 skip / 有装无签 skip-with-reason / 有装有签 PASS）；
- [ ] 零 ANCHOR_PUBKEYS 变更、零 partisync-ext-host 代码变更、零新依赖（git diff 实证）；
- [ ] EXT-SIGNING 窗口节回填闭合注记（PR-b）；
- [ ] 三门禁绿。
