# M11-WP01 发布报告:v0.2.0-rc.1(首个公开 GitHub Release)

> Task-ID: M11-WP01-T04(发布报告)/ 关联:T01–T03(PR #217)、#220(strip
> 修复)、#219(ADR-0033,另一会话)· cadence §3.2-4 · 发布时刻
> 2026-10-10T11:57:55Z · 发布人确认:用户「发布吧」(§3.2-4 终审)
> 负责人: @partiverse · 批准人: @lead

## 1. 发布摘要

- **tag** `v0.2.0-rc.1` → `e9659fa`(main,含窗口 PR #217 + ADR-0033
  #219);**Release**: https://github.com/Partiverse/partisync/releases/tag/v0.2.0-rc.1
  (draft=false,41 assets,generate_release_notes 开);
- **五 bin 口径首次落地**:linux/macOS tar 各含 partisync-cli /
  partisync-mcp / **partisync-mcp-http**(新)/ partisync-fuse(linux)/
  hub-demo-web(linux);桌面 dmg 随发(§4-2b);
- 许可口径:Apache-2.0 终局(ADR-0033)+ 商业分层登记;DCO 贡献策略。

## 2. 改进项实测结论(cadence §2.2 a/b)

| 项 | 实测 | 结论 |
|---|---|---|
| a) strip 前置 | linux tar.gz **52,255,605B(52MB)** vs beta 同产物 **276,426,064B(276MB)**——**-81%**;macOS tar 26.5MB 量级同向 | strip 永久固化(已入 release.yml);✅ |
| b) reproducibility `CARGO_PROFILE_RELEASE_DEBUG=false` | B7 = **REPRODUCIBLE=yes**——beta 时为 no(路径相关性偏差登记),rc 首次全可复现 | 评估生效,转常态配置;✅ |

## 3. B1–B8 复跑结论(RELEASE-CHECKLIST-beta.md rc 口径)

B1 ✅(drop-caches 复测执行记录 2026-10-09;by-hash 断言处置 A/B/C 待
拍板——不阻塞 tag,见 §遗留)/ B2 ✅(main CI 8/8)/ B3 ✅(tag 已推)
/ B4 ✅(run 38035248623 全 job success;**一次修复实录**:macOS BSD
strip 无 `-s` 旗标 → 裸 strip,PR #220)/ B5 ✅(41 assets:2 tar.gz +
1 dmg + 20 minisig + 16 SBOM + SHA256SUMS + repro)/ B6 ✅(docker
alpine 干净环境 `minisign -Vm SHA256SUMS` 通过 CI 子钥公钥
E056CBB62BF3EF34;linux tar.gz sha256 一致)/ B7 ✅(yes)/ B8 ⏳(本
报告 + draft 正文呈用户终审,publish 已按 §3.2-4 授权执行)。

## 4. 窗口实录与偏差

- run 38033804895 首跑 build-cli-macos 失败(BSD strip `-s`)→ PR
  #220 修复 → 删 tag 重打(B4 判例)→ run 38035248623 全绿;
- tag 首打后另会话合入 #219(ADR-0033)→ tag 重指新 main(文档/许可,
  二进制无差);CHANGELOG 补许可终局注记(973b858);
- deny job 曾因 Docker Hub 429 限流红一次(#213 线,重跑自愈)。

## 5. 遗留与后续

1. **B1 by-hash 断言处置 A/B/C 待拍板**(推荐 A:重划断言口径);
2. **GA 口径拍板文档**(cadence 尾项):GA 判据草案 = 候选 4 语料积累
   (T1 ≥10⁵)+ stored 原文域落地(M11-WP04)+ rc 反馈窗口;正式拍板
   文档随窗口出;
3. 发布报告落档本文件;README REPRODUCIBLE 表述已随本 PR 翻新。
