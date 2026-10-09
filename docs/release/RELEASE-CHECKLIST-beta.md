# v0.1.0-beta 发布 checklist（M9-WP06）

> SPEC: docs/specs/M9-WP06.md · 判例: docs/reviews/M8-WP02-release-report.md
> 红线前置：**§C 起的所有发布动作仅在用户确认 SPEC §2.1 三轴口径后执行**。
> 本清单步骤全部可脚本化；执行人在 [ ] 内打勾并在 T03 发布报告
> （docs/reviews/M9-WP06-*.md）留痕命令输出。

## A. 准备（T01/T02，确认前可做）

- [x] A1 SPEC + changelog 草稿 + 本 checklist 入仓（T01，本 PR）
- [x] A2 本地核验：`cargo metadata --locked --format-version 1 >/dev/null && echo LOCK_OK`
      （可复现构建 lock 前提；2026-10-05 实跑 LOCK_OK）
- [x] A3 本地核验：`cargo build --locked --release -p partisync-core`
      （release profile 叶子冒烟；实跑 Finished，见任务卡）
- [x] A4 本地核验：`cargo deny check bans licenses sources`（依赖门禁；
      实跑全绿，见任务卡）
- [ ] A5 release.yml 修订：产物面扩 bin（partisync-mcp/partisync-fuse
      入 tar）+ 产物名 `-beta` 后缀 + cyclonedx SBOM 导出 +
      reproducibility 双构建 job（T02）
- [x] A6 干跑核验：`cargo install --locked cargo-auditable`（0.7.7，
      install 20s）+ `cargo auditable build --locked --release
      -p partisync-core`（Finished 1.67s，SBOM 嵌入路径实测）+
      `cargo build --locked --release -p partisync-cli`（6m00s，
      <10 分钟线，产物 target/release/partisync-cli 38M 实测）
      （2026-10-05 实跑）
- [ ] A7 口径确认：用户对 SPEC §2.1 三轴（版本号/产物面/含 FUSE 写回）
      确认或修订——**未确认不进入 §B**

## B. 发布窗口（T03，容器/CI 步）

- [ ] B1 drop-caches 冷缓存复测（M8-WP07-bench §2 债）：Linux 容器
      `--device /dev/fuse`，release profile 构建 `partisync-fuse`，
      复测 `/by-hash` vs 目录透传（断言 cas ≤ max(3×dir, 50ms)）+
      顺序读基线对照（M8-WP01-bench §1 1567 MiB/s 口径）；报告落
      docs/reviews/
      （**M10-WP06 窗口注记** 2026-10-08：本项改挂 M10-WP06 复测
      窗口执行，触发条件 = Linux 台架可得，不随 rc 发布窗口强制；
      非阻塞依据 = 读路径自 M8 未改 + 热缓存无算法性劣化
      （M8-WP07-bench §2/§3）+ M8 冷基线 1567 MiB/s 维持
      （M9-WP06-T03 报告 §3-2 原依据）——SPEC
      docs/specs/M10-WP06.md §2.2）
      **（执行记录 2026-10-09，Kubuntu 台架）**：复测已完成——tmpfs
      对齐口径透传四指标 ≥ M8 基线量级（冷读中位 3.20ms / 顺序
      2932 MiB/s / 随机 p50 17µs），**读路径无劣化实证成立**；
      **by-hash 侧断言按字面 FAIL**（open 时整载全对象语义——
      M8-WP07-T04 设计使然，非回归；64MiB 对象热开 ~32ms 恒定成本），
      处置三选（A 重划断言口径【推荐】/ B 登记流式读改进债 /
      C 维持挂账）**待拍板后本框闭合**；报告 =
      docs/reviews/M10-WP06-T03-fuse-cold-retest.md（含两轮口径 +
      脚本归档）
- [ ] B2 门禁全绿：`cargo fmt --all --check && cargo clippy --workspace
      --all-targets -- -D warnings && cargo test --workspace`
- [ ] B3 tag（**确认后执行**）：`git tag -a v0.1.0-beta -m "v0.1.0-beta"
      && git push origin v0.1.0-beta`
- [ ] B4 release workflow 全绿：`gh run watch`（draft Release 产出；
      任一步红 → 修复 PR 后删 tag 重打，沿 alpha 9 跑判例）
- [ ] B5 产物齐备核验：3 tar.gz/dmg + 各 `.minisig` + SHA256SUMS +
      `.minisig` + CycloneDX SBOM JSON
- [ ] B6 干净环境验签：`minisign -Vm SHA256SUMS -p partisync.pub &&
      sha256sum -c SHA256SUMS`（RELEASE-PUB-KEY.md 步骤原样）
- [ ] B7 reproducibility：reproducibility job 内 linux 双构建 SHA256
      一致；不一致 → 偏差清单登记（M8-WP02 R1 判例），不假绿
- [ ] B8 changelog 终审：CHANGELOG.md beta 节与 Release draft 正文
      一致（用户终审；generate_release_notes 关闭或附录化登记）

## C. 发布（用户终审动作）

- [ ] C1 用户终审 changelog + 产物清单 → publish（draft=false）
- [ ] C2 发布记录落盘 docs/reviews/M9-WP06-*-release.md（run id、
      产物 sha256、偏差/已知限制引用 CHANGELOG Known limitations）

## D. 关账

- [ ] D1 SPEC §3 勾选 + M9-WP00 §1-WP06 / §2 拍板项④ / §4 债表
      drop-caches 行清账回填
- [ ] D2 追溯：`cargo xtask trace M9-WP06-T01` 链路完整
