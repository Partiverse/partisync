title: GitHub Actions 模式
filename: github_actions_patterns.md
tags: [github-actions, ci, devops, automation, yaml]
updated_ns: 1704067200000000000

# GitHub Actions 模式

## 关键概念

- **Workflow**：YAML 文件
- **Job**：并行/串行的执行单元
- **Step**：Job 内步骤
- **Action**：可复用 step
- **Runner**：执行环境

## 触发

```yaml
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  workflow_dispatch:    # 手动触发
```

## 矩阵

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    rust: [stable, 1.94]
```

## 缓存

```yaml
- uses: Swatinem/rust-cache@v2
  with:
    workspaces: |
      . -> target
      crates/partisync-cli -> target
```

## 步骤模式

### 编译

```yaml
- run: cargo build --workspace --all-targets
```

### Lint

```yaml
- run: cargo clippy --workspace --all-targets -- -D warnings
```

### 测试

```yaml
- run: cargo test --workspace
```

### 格式

```yaml
- run: cargo fmt --all --check
```

## 安全检查

- cargo audit
- cargo deny check
- CodeQL
- Trivy（容器）

## 发布

```yaml
- uses: dtolnay/rust-toolchain@stable
- run: cargo build --release
- uses: softprops/action-gh-release@v1
  with:
    files: target/release/partisync
```

## 缓存策略

- cargo target：必须 cache
- 依赖 DB（cargo-deny）：cache
- 构建产物（.deb, .msi）：upload artifact

## 经验

- ❌ 不缓存 cargo target（每次重编）
- ❌ 一次跑太多 step（无意义分割）
- ✅ 矩阵测试按项目调整
- ✅ Secrets 走 GitHub Secrets

## 调试

- `act` 本地跑 workflow
- 详细日志：`ACTIONS_RUNNER_DEBUG=1`
- 时间线视图（GitHub UI）