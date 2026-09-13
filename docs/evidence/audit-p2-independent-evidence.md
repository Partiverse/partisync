# P2 加固批独立复审证据报告（fresh-context 独立审计员）

- **审计对象**：提交 `75fd604`（A2-01…A2-06）+ 复审修复 `a6a6702`（F6）
- **审计面**：证据真实性（文档声称的验证结论能否被独立复现）
- **审计方式**：fresh-context 独立审计员，仅依据仓库现状；除本报告外未修改任何仓库文件；未 checkout/stash/改工作区；未删容器/卷/索引
- **核心纪律遵守**：`GOCACHE=/tmp/gocache-audit-evi`；用文件重定向/`job_output` 取真实退出码（未用吞码管道）；脚本先通读再执行；一次性容器用命名卷而非宿主机 `/tmp` 绑定（见 §1 根因）

---

## Verdict

**pass** —— 文档声称的结论（L1 三绿、A2-01…A2-06 行为正确、F6 已修、L3 8/8）均被独立复现；未发现证据伪造或夸大。存在 3 项需披露的低/中风险（脚本在本沙箱不可直接跑全量、单测计数口径不一致、L3 通过判定未含 `badResponses`），均不影响核心结论。

---

## 1. 逐项复现记录

### 1.1 L1（build / vet / test -race）

| 命令（均已 `export GOCACHE=/tmp/gocache-audit-evi GOFLAGS=-mod=vendor`） | 真实退出码 | 输出摘要 |
|---|---|---|
| `go build ./... > /tmp/l1-build.log 2>&1; echo $?` | **0** | 空输出 |
| `go vet ./... > /tmp/l1-vet.log 2>&1; echo $?` | **0** | 空输出 |
| `go test -race -count=1 ./... > /tmp/l1-test.log 2>&1; echo $?` | **0** | `ok partisync/server/cmd/server`、`ok internal/api`、`ok internal/search`、`ok internal/storage`；`store`/`models`/`worker`/`bench` 无测试文件 |

**吻合度：一致。** L1 三绿可复现。

**单测数量核对**（与文档声称比对）：
- 文档说法：SESSION §13「16 个新用例」、l2 证据 §2「15 个（75fd604）」、audit-p2 §7「16 个」、SESSION 行 201「初版 14 个新用例」——**四份口径互不一致**。
- 独立统计（按 `git show 75fd604` / `a6a6702` 中 `^+func Test`）：`75fd604` 新增 **17** 个测试函数（readyz 4、upload 6、image 3、storage 2、main 2），`a6a6702` 新增 **1** 个（F6），共 **18** 个新测试函数。
- **结论**：文档计数偏少（14/15/16 vs 实际 18），属**统计口径不一致**，但**所有新增用例均存在、均通过 `-race`**，且具真实断言力——属低估而非夸大。详见 Finding L2/L3。

### 1.2 L2（提交 `l2-p2-hardening.sh` 的真实可跑性 + 独立等价复现）

**关键发现（根因）**：直接运行 `bash scripts/l2-p2-hardening.sh` 在本沙箱**无法跑到全量**——其 S5/S8 一次性容器把宿主机 `/tmp/p2-*-data` 绑定挂载进容器，而本沙箱的 `/tmp` 与 docker 守护进程隔离：从 docker 视角该路径不存在 → docker 以 root:root 自动创建 → 容器内 app（uid 10001）`mkdir /data/assets: permission denied` → 容器崩溃。报错见容器日志：`init content storage: storage: init root: mkdir /data/assets: permission denied`。

- 这不是脚本逻辑缺陷（S1–S4/S6/S7 全部命中**主服务 8080**，与宿主机 `/tmp` 无关，逻辑可跑）；在 `/tmp` 对 docker 守护进程可见的普通主机上该脚本应能 60/60。
- 我用**命名卷**替代绑定挂载（`docker volume create` 由守护进程管理，绕开隔离），独立等价复现了 S5/S8，二者均 PASS（见下）。即：脚本失能是**环境限制**，被测行为本身可复现。

**独立等价复现（全部断言命中；用命名卷跑通 S5/S8，其余打主服务 8080）**：

| 段落 | 关键断言 | 独立结果 | 吻合度 |
|---|---|---|---|
| S1 探针基线 | `/healthz`=200；`/readyz`=200 且 postgres/meili/storage `ok:true` | 全 PASS | 一致 |
| S2 A2-04 内容-声明 | HTML 伪装 `.png` 上传 201、嗅探 mime=`text/html`、预览 200；元数据声明 `image/png` 同内容预览 **415** | 全 PASS | 一致 |
| S3 文本家族 | notes.md→`text/markdown`、data.csv→`text/csv`、config.json→`application/json`、photo.png→`image/png` 孪生预览均 **200**，字节非空 | 全 PASS（注：config.json 首次上传返回 200 是内容去重命中我的静态夹具，非缺陷；孪生登记/预览均 201/200 正常） | 一致 |
| S4 零 /tmp 落盘 | 12MiB 上传 201；容器可写层 delta=0B；`/tmp` 条目=0 | 全 PASS | 一致 |
| S5 MAX_UPLOAD_BYTES | 命名卷一次性容器 `MAX_UPLOAD_BYTES=1MiB`：2MiB→**413**、512KiB→**201** | 全 PASS | 一致 |
| S6 探针语义分离 + 孤儿清理 | PG 停→`/readyz` 503 且 postgres `ok:false`；`/healthz` 仍 200；PG 停时上传→**500** 且对象**不存在**（已清理）；PG/Meili 恢复→200 | 全 PASS | 一致 |
| S7 慢速上传 | 4MiB @100KB/s → **201**，用时 **40.97s**（>15s/30s 服务端常规超时） | PASS（脚本内 `耗时>35s` 判定因我的探针小 bug 误报，实测 40.97s 显然达标） | 一致 |
| S8 F6 孤儿回归 | 命名卷一次性容器 1MiB：对照 201（已提交）；超限请求（file 已落盘后解析失败）→**413**；**NO_ORPHAN**（失败对象被清理）；先前成功对象**未误删**；残留文件数=1；`.tmp`=0 | 全 PASS（对照上传返回 200 为命名卷内去重命中，残留文件数=1 已证明对象存在，不影响 F6 结论） | 一致 |

**吻合度（L2 行为）**：**一致**——A2-01…A2-06 与 F6 的全部行为均被独立复现。唯一不能在本环境用**原脚本字面值**产出「60/60」是因 `/tmp`↔docker 隔离；等价 harness 已确认每一条行为。

### 1.3 L3（浏览器闭环 8/8）

独立重跑（截图写入 `/tmp/l3-shots`，**不触碰仓库**）：

```
L3_BASE=http://127.0.0.1:8080/ L3_SHOTS=/tmp/l3-shots node scripts/l3-mcd-acceptance.mjs
→ L3_EXIT=0；ok=true；8 项 checks 全 true；failed=[]；consoleErrors=[]；badResponses=[]
细节：cards=24 / notice shown / cards=1 / preview 200·82B / badge visible /
  polled 20s completed=true / confirm notice / ai badges=3
```

与仓库内 `docs/verification/p2-hardening-run.json` **逐字段一致**（cards=24、ai badges=3、preview bytes=82、consoleErrors/badResponses 空）。截图时间戳 2026-09-13T02:07（a6a6702 提交前 3 分钟，符合「跑完即提交」）也与运行窗口自洽。

**吻合度：一致。** L3 8/8 证据真实且可复现；8 条断言确实覆盖 MCD 闭环（列表→上传去重→搜索命中→预览→人工标签→慢标注任务完成→确认写入→AI 标签），非凑数。

---

## 2. Findings 分类

### blocker
- 无。

### high
- 无。

### medium
- **M1（环境可移植性 / 复现风险）**：提供的 `l2-p2-hardening.sh` 在「宿主机 `/tmp` 对 docker 守护进程不可见」的环境里 S5/S8 必然失败（容器 `/data/assets` 权限拒绝）。本环境即如此，故原脚本无法在此产出 60/60。这是**验证脚本的可移植性问题**，不是被测代码缺陷——等价 harness（命名卷）已确认行为全部正确。建议脚本改用命名卷或预先 `chown` 挂载点，使 L2 在任何环境可复现。对「证据真实性」结论无影响（行为已独立复现），但影响「他人按脚本原样复现」的可行性。

### low
- **L1（文档计数不一致）**：单测新增数在 SESSION/audit/l2 文档中分别为 14/15/16，git 实数为 18 个新函数（75fd604 17 + a6a6702 1）。属口径低估，所有用例均存在且通过 `-race`，非夸大。
- **L2（L3 通过判定缺口）**：`l3-mcd-acceptance.mjs` 的 `ok = failed.length===0 && consoleErrors.length===0` **未把 `badResponses` 纳入通过条件**，而 AC-08 明文要求 `badResponses=0`。本次重跑 `badResponses=[]` 故无碍，但判定逻辑弱于声明的验收标准。建议把 `badResponses.length===0` 加入 `ok`。
- **L3（历史 53/53 不可回放）**：audit-p2 §2 称「`75fd604` 上 L2 复跑 53/53」，该历史状态需 checkout `75fd604`（违反「不改工作区」纪律）才能回放；我只验证了当前 `a6a6702` 状态（等价 60 项行为）。属方法学局限，已在 §1.2 说明。

### info
- **I1（F4 诚实未测）**：audit-p2 与 SESSION 均明确 F4（并发同内容 + 单边 PG 失败窗口）**未实测**（需故障注入），仅代码推演。披露充分，非夸大。
- **I2（非独立声明一致）**：audit-p2 / SESSION / 任务卡均诚实标注 AC-11 未满足、结论为实施者自证；未把自证冒充独立复审。

---

## 3. 证伪尝试清单（独立验证「声称已修/已验证」是否真成立）

| 声称 | 独立证伪/验证手法 | 结果 |
|---|---|---|
| **F6 已修（file 落盘后超限不残留孤儿）** | ① 读代码：`saveUploadedFile` 有 `defer` 在 `err!=nil && !saved.Deduped` 时 `Discard(saved.RelPath)`（`git diff a6a6702` 确认该 defer 系本次新增）；② `go test -race` 中 `TestUploadCleansUpObjectWhenBodyCapTripsAfterFilePart` 通过（对照证明先提交、实验证明再清理、计数 0 残留）；③ 独立 live S8：`NO_ORPHAN`、先前对象未误删、残留=1 | **成立** |
| **F1 已修（400 文案不回显内部错误串）** | 读 `uploadErrorMessage`：非 multipart 分支仅返回 `errUploadNotMultipart.Error()`（"request is not multipart/form-data"），不再包内部 `err` | **成立** |
| **A2-04 伪装图片预览 415 / 文本家族不误拒** | ① 单测 `TestMatchedContentType` 14 例（含 html@image/png→false、md/csv/json@对应→true）通过；② live S2 415、S3 四家族 200 | **成立** |
| **A2-01 不落容器 /tmp** | 单测 `TestSaveUploadedFileStreamsWithoutTempSpill`（12MiB、TMPDIR 零残留、`.tmp` 零残留）通过；live S4 delta=0、/tmp=0 | **成立** |
| **A2-03 上限跟随 MAX_UPLOAD_BYTES** | 单测 `TestUploadRequestBodyCapFollowsStoreLimit` 通过；live S5 1MiB→2MiB 413 / 512KiB 201 | **成立** |
| **A2-02 / A2-05 探针语义 + 孤儿清理** | 单测 readyz/storage 系列通过；live S6 503/200/500/孤儿清理 | **成立** |
| **A2-06 慢速上传不被截断** | live S7 40.97s→201（服务端 ReadTimeout 15s/WriteTimeout 30s 未放宽本次请求被拒绝的情形） | **成立** |
| **「还原旧写法后新单测会失败」(反向验证)** | 我**未改仓库**做变异（遵守纪律），改用代码走查论证：若删除 `saveUploadedFile` 的 `defer` 清理与 A2-05 的 `Discard` 分支，则已提交对象在 MaxBytesError 后不会被删 → `countFiles != 0` → 测试报 `orphan objects left`；控制组已证明对象确实先落盘。该测试具判别力 | **走查成立**（未做实变异，因禁止改仓库） |

---

## 4. 盲区

1. **原 L2 脚本在本沙箱无法全量自跑**（M1）：用命名卷等价复现弥补，但「原样 60/60」未在此环境字面产出。
2. **F4 并发窗口未实测**：需故障注入，本次未做；文档已诚实标注。
3. **存储型 XSS 浏览器实测未由我重跑**：audit-p2 称真实 Chromium 下 CSP `sandbox` 拦截脚本执行；我以「代码 + 单测 + L3 `consoleErrors=[]`」间接佐证，未亲自起无头浏览器做对照/实验组脚本执行对照。
4. **历史 `75fd604` 的 53/53 不可回放**（纪律限制，见 L3）。
5. **公网暴露面 / 认证缺失（A2-07…A2-09）**：上传与元数据端点无认证，本审计仅在环回环境验证，未评估公网风险（与文档盲区一致）。
6. **规模边界**：慢上传仅验到 41s（未逼近 5min 预算上限）；`/tmp` 零落盘仅 12MiB 量级。

---

## 5. 结论摘要（供 AC-11 裁决）

- 证据真实性：**通过**。L1/L2 行为/L3 均独立复现，无伪造或夸大。
- F1–F6 与代码现状**逐条吻合**：F6 修复真实有效，F1 文案收口真实，F2/F3/F4/F5 接受/待办描述与代码一致且披露充分。
- SESSION §13 与证据文档**自洽**（除单测计数口径 14/15/16 互不一致、实为 18，属低估）。
- 独立性缺口（原 AC-11）**由本次 fresh-context 独立审计补齐**；前述 M1/L2/L3 为可披露的改进项，不改变「结论可独立复现」的总体判定。
