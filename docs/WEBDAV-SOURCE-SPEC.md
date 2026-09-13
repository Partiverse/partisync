# SPEC: WebDAV 数据源连接器（阶段二首发功能）

## 1. 目标

在 Web UI 上新增「数据源」面板，用户可配置 WebDAV 源 → 触发扫描 → 资产从远程 WebDAV 目录入库（SHA256 去重）→ MeiliSearch 索引 → 资产列表显示来源标签并支持按源筛选。

## 2. 用户流程

```
1. 用户进入「数据源」面板
2. 点击「添加 WebDAV 源」→ 填写 URL / 用户名 / 密码 / 远程路径
3. 点击「扫描」→ 后端列出远程文件 → 过滤支持类型 → 对每个文件：
   a. 下载到临时位置
   b. 计算 SHA256
   c. 若 SHA256 未重复 → 入库 + 索引
   d. 否则跳过（已存在）
4. 扫描完成 → 显示入库数量 / 跳过数量
5. 资产列表新增「来源」列，显示 WebDAV 图标 + 源名称
6. 资产筛选栏新增「数据源」下拉，可按源筛选
```

## 3. 数据库变更

### 表：`data_sources`

| 列 | 类型 | 说明 |
|---|---|---|
| id | UUID | 主键 |
| name | VARCHAR(255) | 显示名称 |
| type | VARCHAR(32) | 来源类型，目前固定 `webdav` |
| config | JSONB | 加密存储的配置（URL、用户名、密码、远程路径） |
| last_scan_at | TIMESTAMPTZ | 上次扫描时间 |
| last_scan_result | JSONB | 上次扫描结果统计 |
| created_at | TIMESTAMPTZ | 创建时间 |

### 表：`assets` 新增列

| 列 | 类型 | 说明 |
|---|---|---|
| source_id | UUID | 所属数据源（可空，本地上传为 NULL） |

## 4. API 设计

### `GET /api/v1/sources`
返回所有数据源列表。

Response:
```json
{
  "sources": [
    {
      "id": "uuid",
      "name": "我的 NAS",
      "type": "webdav",
      "url": "https://nas.example.com/webdav/",
      "remote_path": "/documents",
      "last_scan_at": "2026-09-13T...",
      "asset_count": 42
    }
  ]
}
```

### `POST /api/v1/sources`
创建数据源。

Request:
```json
{
  "name": "我的 NAS",
  "type": "webdav",
  "url": "https://nas.example.com/webdav/",
  "username": "user",
  "password": "pass",
  "remote_path": "/documents"
}
```

Response: `201 Created` + 源对象（密码不返回）

### `DELETE /api/v1/sources/{id}`
删除数据源（不删除已入库资产，source_id 置 NULL）。

### `POST /api/v1/sources/{id}/scan`
触发扫描。

Request（可选）:
```json
{
  "recursive": true,
  "types": ["image", "document"]
}
```

Response:
```json
{
  "job_id": "uuid",
  "status": "queued"
}
```

扫描异步执行，通过 `GET /api/v1/jobs/{id}` 查询进度。

## 5. 后端实现

### `internal/connector/webdav/` 包

```
connector/webdav/
  client.go    — WebDAV 客户端封装（使用 go-webdav-client）
  lister.go    — 递归列出远程文件
  downloader.go— 下载单个文件到临时位置
```

**依赖**: `github.com/emersion/go-webdav-client@v0.0.0-20180810`

### `internal/worker/source_scanner.go`

异步扫描任务：
1. 调用 `webdav.Lister` 列出文件
2. 对每个文件调用 `storage.Save`（已有 SHA256 去重逻辑）
3. 更新 `assets.source_id`
4. 记录扫描结果到 `data_sources.last_scan_at / last_scan_result`

## 6. 前端实现

### 新增页面/面板：`DataSourcesPage`

- 源列表（卡片展示）
- 添加源表单（模态框）
- 每个源的「扫描」按钮 + 最后扫描时间 + 资产数量
- 扫描中显示进度 spinner

### 资产列表变更

- 「来源」列：图标 + 源名称（若无源显示"本地上传"）
- 筛选栏新增「数据源」下拉（多选）

### 文件变更

```
playground/src/
  pages/
    DataSourcesPage.tsx    — 新增
  components/
    SourceBadge.tsx       — 新增：来源标签
    SourceFilter.tsx      — 新增：来源筛选控件
  lib/
    api.ts                — 新增 sources API 调用
```

## 7. 安全考虑

- WebDAV 密码在传输和存储上加密（复用现有敏感字段处理逻辑）
- 支持 HTTPS URL（禁止明文 HTTP，除非用户在高级选项中明确确认）
- 扫描超时控制（防止挂起的 WebDAV 连接）
- 源配置写入 `config` JSONB 列，密码字段使用 `storage.Obscure` 加密

## 8. 非目标（禁区）

- 不在扫描过程中对远程文件做修改/删除（只读）
- 不实现双向同步
- 不实现 WebDAV 服务器功能（只做客户端）
- 不在 MCD 阶段引入 rclone 进程管理（未来通过 subprocess 调用 rclone 做 SFTP 等复杂协议）

## 9. 验收标准

- [ ] `POST /api/v1/sources` 可创建 WebDAV 源，密码不返回在响应中
- [ ] `DELETE /api/v1/sources/{id}` 删除源，source_id 被置 NULL
- [ ] `POST /api/v1/sources/{id}/scan` 触发异步扫描并返回 job_id
- [ ] 扫描完成后 `GET /api/v1/assets?source_id=xxx` 可筛选该源的资产
- [ ] 前端数据源面板显示源列表 + 添加表单 + 扫描按钮
- [ ] 前端资产列表显示来源列，可按源筛选
- [ ] `npm run build` TypeScript 零错误
