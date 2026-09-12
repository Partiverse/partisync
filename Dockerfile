# 多阶段构建：静态 Go 二进制 + 最小运行层（含 wget 供容器 healthcheck 使用）。
FROM golang:1.22-alpine AS build
WORKDIR /src
# 依赖已 vendored：镜像构建完全离线（本环境容器内无法访问 proxy.golang.org）。
ENV GOFLAGS=-mod=vendor GOPROXY=off
COPY go.mod go.sum ./
COPY vendor ./vendor
COPY cmd ./cmd
COPY internal ./internal
RUN CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /out/partisync-server ./cmd/server

# 前端产物由宿主机构建（本环境容器内无法访问 npm registry，与 Go vendor 同理）：
# 先在宿主机 `cd playground && npm run build`，dist 随构建上下文进入镜像。
FROM alpine:3.20
# busybox 自带 wget，供容器 healthcheck 使用（避免 apk 联网）。
RUN adduser -D -u 10001 app \
 && mkdir -p /data/assets \
 && chown -R app:app /data
COPY --from=build /out/partisync-server /usr/local/bin/partisync-server
COPY playground/dist /app/web
USER app
WORKDIR /data
EXPOSE 8080
ENV ADDR=:8080 \
    PG_DSN=postgres://partisync:partisync_dev_password@postgres:5432/partisync?sslmode=disable \
    MEILI_URL=http://meilisearch:7700 \
    MEILI_KEY=partisync_master_key_for_dev_only_32_chars_long \
    STORAGE_ROOT=/data/assets \
    WEB_DIST=/app/web
CMD ["partisync-server"]
