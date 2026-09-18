# ADR-0001: partisync-core 引入 getrandom 作为 OS 熵源

状态: 已接受 · 日期: 2026-09-18 · 决策人: @lead（AI 代理起草，人工签核待补）
关联: SPEC M-1-WP07（Ulid::now 的随机部分）

## 背景

`Ulid::now()` 需要为随机段（80 bit）取操作系统熵。零依赖方案需自行封装 /dev/urandom、getentropy(2)、BCryptGenRandom 等平台差异，属于「易错的密码学邻接代码」，违反 AGENTS.md「不自造安全邻接轮子」的约束。

## 决策

引入 `getrandom = "0.2"`（workspace 依赖）作为唯一的 OS 熵源抽象。

选型理由：
- Rust 密码学生态事实标准（rustls/rsa/ed25519-dalek 等均经它取熵），久经审计；
- 极小（无传递依赖负担）、Apache-2.0/MIT 双许可（deny.toml 白名单内）；
- 与铁律 8「无聊依赖」一致：这是地基件，不是实验件。

## 备选方案

- 零依赖手写平台熵封装：维护成本与出错面大，否决；
- `rand` 全家桶：仅取熵用不到其体积与特性面，否决（未来统计模拟需要时另行 ADR）；
- `RandomState` 哈希种子充当熵：非密码学用途设计、跨进程保证弱，否决。

## 后果

- `Ulid::now()` 在熵源不可用时 panic（明确文档化：OS 熵失败属不可恢复环境错误）；
- deny.toml 白名单无需变更；
- 该依赖同时服务未来 M2 的设备身份（Ed25519 密钥生成）。

## 重新评估条件

- getrandom 0.2 停止安全维护 → 升级 0.3（API 变更：`getrandom::getrandom` → `fill`），伴随全量回归。
