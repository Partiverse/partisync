# M9-WP06-T03 v0.1.0-beta 发布报告

> Task-ID: M9-WP06-T03 · 日期: 2026-10-05 · SPEC: docs/specs/M9-WP06.md
> 执行: GLM-5.3-Flash (ZCode)（用户口径确认 + 全程 CI 门禁 + checklist）

## 1. 发布事实

| 项 | 值 |
|---|---|
| Release | https://github.com/Partiverse/partisync/releases/tag/v0.1.0-beta |
| 状态 | **Published（draft=false，Latest）** 2026-10-05T04:15:21Z |
| tag | `v0.1.0-beta`（Cargo workspace 维持 0.1.0——SPEC §2.1 版本号轴） |
| release workflow | run [37256427573](https://github.com/Partiverse/partisync/actions/runs/37256427573) 全绿（6 jobs：linux/macos/desktop/sbom/reproducibility/release） |
| 产物 | 41 assets：linux tar（cli+mcp+fuse+hub-demo-web）、macOS tar（cli+mcp）、desktop dmg（aarch64）、SHA256SUMS、各 `.minisig`、22 个 CycloneDX `.cdx.json`、reproducibility.txt |

## 2. 核验（B5–B8）

- **B5 产物齐备** ✅（41 项清点，命名全部带 `-beta` 后缀）。
- **B6 验签** ✅ 干净目录复验：`minisign -Vm SHA256SUMS -p CI子钥` →
  "Signature and comment signature verified"（trusted comment
  `PartiSync release (CI subkey; 2026-10-05T03:15:55Z)`，key id
  E056CBB62BF3EF34）；linux tar 全量下载（276,426,064 字节 = 服务端
  asset size 一致）后 `sha256sum -c` → **OK**。
- **B7 reproducibility** ⚠ **REPRODUCIBLE=no**——cargo 构建路径相关性
  （M8-WP02 R1 判例：偏差登记不假绿）；reproducibility.txt 已随 Release
  产物分发。改进项（rc 口径）：`CARGO_PROFILE_RELEASE_DEBUG=false` 等
  确定性构建手段评估。
- **B8 changelog**：generate_release_notes 开启（auto notes 为附录），
  Release 正文为手写 beta 摘要；仓库 CHANGELOG.md beta 节为详版——
  两者并存，报告登记为口径（M8 判例差异：本期产物面/已知限制变化大，
  手写正文优先）。

## 3. 偏差与已知限制（如实登记）

1. **macOS 产物不含 partisync-fuse**（SPEC §2.1 产物面轴偏差）：fuser
   需 osxfuse 系统件、runner 构建必败——linux tar 四 bin 全量；macOS
   挂载路径写 入 Release notes 已知限制。
2. **B1 drop-caches 冷缓存复测未跑**（SPEC §6-R3 预登记偏差）：交互式
   手动台架本窗口无法安全自动化；依据 = 读路径自 M8 未改 + 热缓存无
   算法性劣化（M8-WP07-bench §2）+ M8 冷基线 1567 MiB/s 维持——
   **改挂 M10-WP06 复测窗口**（非发布阻塞）。
3. **产物未 strip**（linux tar 276MB）：rc 口径评估打包步加 strip
   （体积改进项，非缺陷）。
4. hub 产物 = hub-demo-web（demo 面，如实标注，release-report §4-4 债
   延续）。
5. 桌面 dmg 未公证（SPEC 非目标，README 绕过方式维持）。

## 4. 关账回填（随本报告 PR）

- SPEC M9-WP06 §3 全勾（T03 复测项标偏差执行）；§2.1 注记口径确认
  2026-10-05。
- M9-WP00：§1-WP06 状态行发布完成 + §2 拍板项④ 落锤 + §4 drop-caches
  行改挂 M10-WP06。
- M9-report §1-WP06/§8-2 对应行由 G3 后续修订或 M10-WP00 滚动登记
  （本报告为凭）。

## 5. 修订记录

| 版本 | 日期 | 内容 |
|---|---|---|
| 1.0 | 2026-10-05 | 初版：发布事实 + 核验 + 偏差五项 + 关账回填 |
