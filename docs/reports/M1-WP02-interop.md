# rclone 互操作认证记录（M1-WP02 首轮）

- 日期：2026-09-19 · rclone v1.75.1 · 端点：`partisync-cli ui --s3 127.0.0.1:8081 --data ./s3data`
- remote 配置：`type = s3 / provider = Other / endpoint = http://127.0.0.1:8081`（path-style，无鉴权）
- 命令：`./target/debug/partisync-cli ui --db … --s3 … --data …`（双服务并行：演示面 8080 + S3 网关 8081）

## 互操作矩阵（首轮全绿）

| 操作 | 结果 | 备注 |
|---|---|---|
| lsd（列桶） | ✅ | ListAllMyBuckets XML |
| lsf / lsjson | ✅ | ListObjectsV2 + prefix/delimiter/分页字段 |
| copy 上行 | ✅ | 含 CreateBucket（rclone Mkdir 语义）|
| copy 下行 | ✅ | |
| check | ✅ | 7 matching files；MD5 ETag 口径 |
| moveto / deletefile | ✅ | |
| **MPU 大文件** | ✅ | 12MB / 5Mi 块 ×3 parts / md5 往返一致 |
| **bisync** | ✅ | --resync + 正式跑 + **双向变更注入收敛**（远端新增→本地出现）|

## 过程中抓到并修复的互操作缺陷（5 个）

1. axum 0.8 通配符路由语法（`*key`→`{*key}`）——首跑即 panic；
2. **HEAD 对目录返回 200** → rclone 把目录当文件、状态机挂死——改 404（S3 语义）；
3. LIST prefix 错位：磁盘遍历根未跟随 prefix（假 CommonPrefix `src/src/`）；
4. `Last-Modified` 响应头用 ISO8601——Go SDK 期望 RFC1123，下行解析失败；
5. **axum 默认 2MB body 限制**——MPU part 5MiB 被 413 拒（`DefaultBodyLimit` 解除）。

另：`x-amz-meta-mtime` 边车往返已实现（.meta/ 树，不进对象 LIST）——rclone sync 语义基础；
残留 NOTICE（"Failed to read last modified"）为良性（上传前对不存在对象的预检 404），
已登记为已知噪音。

## 未尽项（登记）

- SigV4 签名验证（当前接受任意凭证——仅限本机）；bucket 版本化；ListObjectsV2
  的 encoding-type/url 解码；并发 LIST 分页边界压力（cutover token 精确性）；
- bisync 的空目录/重命名边角（rclone 自认 S3 后端的已知限制面）。

## 追加：WebDAV 服务面互操作（M1-WP05 第二协议，2026-09-19）

- 端点：`partisync-cli ui --dav 127.0.0.1:8082`（与 S3 共用 `--data` 数据根）
- rclone 配置：`type = webdav / vendor = other`（pass 需 `rclone obscure`——实测踩坑）

| 操作 | 结果 |
|---|---|
| OPTIONS（DAV: 1,2 能力头） | ✅ |
| PROPFIND 根/桶（Depth 0/1，207 multistatus） | ✅ |
| copy 上行/下行 | ✅ |
| check | ✅ 8 matching files |
| moveto（MOVE）/deletefile | ✅ |
| **跨协议**：WebDAV PUT → S3 GET | ✅ 同一数据根即时可见 |

实现缺陷修复：根路径 Path 提取器 500（双路由化）、or-pattern guard 绑定、
响应构造器 body 移动。语义声明：Lock 未实现（占位 200）、目录 MOVE 未支持（405）。
