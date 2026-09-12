#!/usr/bin/env bash
# partisync P2 加固批（A2-01…A2-06）L2 验证 —— 真实容器 + 真实 HTTP 请求
#
# 覆盖：
#   AC-01 内容-声明 MIME 不一致 → 预览 415（存储型 XSS 防线）
#   AC-02 文本家族（md/csv/json）预览不误拒
#   AC-03 12MiB 上传不落容器 /tmp（可写层零增长）
#   AC-04 MAX_UPLOAD_BYTES 生效（1MiB → 2MiB 上传 413，512KiB 通过）
#   AC-05 入库失败（PG 停机）不留孤儿文件
#   AC-06 /readyz 依赖故障 → 503；/healthz 保持 200（语义分离）
#   AC-07 慢速上传（4MiB @100KB/s ≈ 41s，超过服务端 15s/30s 常规超时）→ 201
#
# 前置：docker compose up -d（三容器 healthy）；镜像 partisync-server:local 已构建。
# 运行：bash scripts/l2-p2-hardening.sh
# 退出码：0 = 全部检查通过。
#
# 副作用（可恢复）：短暂停止/启动 partisync-postgres 与 partisync-meilisearch；
# 写入若干 p2-* 测试资产；在 /tmp 下创建并清理一次性容器数据目录。

set -uo pipefail

BASE=${BASE:-http://127.0.0.1:8080}
CAP_BASE=${CAP_BASE:-http://127.0.0.1:8081}
SERVER_CT=${SERVER_CT:-partisync-server}
PG_CT=${PG_CT:-partisync-postgres}
MEILI_CT=${MEILI_CT:-partisync-meilisearch}
NET=${NET:-partisync_default}
IMAGE=${IMAGE:-partisync-server:local}
CAP_DATA=${CAP_DATA:-/tmp/p2-cap-data}

WORK=$(mktemp -d /tmp/p2l2.XXXXXX)

# cleanup_cap_data 删除一次性容器的数据目录：文件由容器内 uid 10001 创建，
# 宿主 rm 无权限，故用同镜像容器删除，再删空目录。
cleanup_cap_data() {
  if [ -d "$CAP_DATA" ]; then
    docker run --rm -v "$CAP_DATA:/cleanup" "$IMAGE" sh -c 'rm -rf /cleanup/* /cleanup/.[!.]*' >/dev/null 2>&1 || true
    rmdir "$CAP_DATA" >/dev/null 2>&1 || true
  fi
}

trap 'rm -rf "$WORK"; cleanup_cap_data' EXIT

PASSED=0
FAILED=0

say() { printf '\n=== %s ===\n' "$*"; }
pass() { PASSED=$((PASSED + 1)); printf 'PASS  %s\n' "$*"; }
fail() { FAILED=$((FAILED + 1)); printf 'FAIL  %s\n' "$*"; }

# http METHOD URL [curl extras...]：状态码写 stdout，body→$WORK/body，headers→$WORK/hdr
http() {
  local method="$1" url="$2"
  shift 2
  curl -sS -X "$method" -D "$WORK/hdr" -o "$WORK/body" -w '%{http_code}' "$@" "$url"
}

jget() { jq -r "$1" "$WORK/body"; }

check_code() { # desc expected actual
  if [ "$2" = "$3" ]; then pass "$1 → $3"; else fail "$1: want $2, got $3"; fi
}

check_eq() { # desc expected actual
  if [ "$2" = "$3" ]; then pass "$1 = $3"; else fail "$1: want $2, got $3"; fi
}

check_true() { # desc condition(0=ok)
  if [ "$2" = "0" ]; then pass "$1"; else fail "$1"; fi
}

# wait_code URL WANT TIMEOUT_S
wait_code() {
  local url="$1" want="$2" timeout="${3:-60}" i=0 code
  while [ "$i" -lt "$timeout" ]; do
    code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 5 "$url" 2>/dev/null || true)
    [ "$code" = "$want" ] && return 0
    sleep 1
    i=$((i + 1))
  done
  return 1
}

# dep_ok /readyz-body name → true|false|missing
dep_ok() {
  jq -r --arg n "$2" '.dependencies[]? | select(.name==$n) | .ok' "$1" | head -1
}

bytes_of() { # "4.1kB (virtual 16MB)" → 4199
  python3 - "$1" <<'PY'
import re, sys
tok = sys.argv[1].split()[0]
m = re.match(r'^([0-9.]+)([kKmMgG]?)[bB]?$', tok)
if not m:
    print(0)
else:
    n, unit = float(m.group(1)), m.group(2).lower()
    print(int(n * {'': 1, 'k': 1024, 'm': 1024 ** 2, 'g': 1024 ** 3}[unit]))
PY
}

say "前置检查：容器与镜像"
for ct in "$SERVER_CT" "$PG_CT" "$MEILI_CT"; do
  status=$(docker inspect -f '{{.State.Health.Status}}' "$ct" 2>/dev/null || echo missing)
  if [ "$status" = "healthy" ]; then pass "container $ct healthy"; else fail "container $ct = $status"; fi
done
if docker image inspect "$IMAGE" >/dev/null 2>&1; then pass "image $IMAGE present"; else fail "image $IMAGE missing"; fi

# ---------------------------------------------------------------- S1 探针基线
say "S1 探针基线（AC-06）"
code=$(http GET "$BASE/healthz")
check_code "/healthz" 200 "$code"
if [ "$(jget '.status')" = "ok" ]; then pass "/healthz payload status=ok"; else fail "/healthz payload = $(cat "$WORK/body")"; fi

code=$(http GET "$BASE/readyz")
check_code "/readyz（全依赖可达）" 200 "$code"
cp "$WORK/body" "$WORK/readyz-baseline.json"
for dep in postgres meilisearch storage; do
  check_eq "/readyz 依赖 $dep ok" "true" "$(dep_ok "$WORK/readyz-baseline.json" "$dep")"
done

# ------------------------------------------------- S2 内容-声明一致性（AC-01）
say "S2 A2-04 内容-声明一致性（AC-01）"
NONCE=$(python3 -c 'import secrets;print(secrets.token_hex(8))')
printf '<html><script>alert("p2-xss-%s")</script></html>' "$NONCE" >"$WORK/xss.png"
code=$(http POST "$BASE/api/v1/assets/upload" -F "file=@$WORK/xss.png;filename=p2-xss.png")
check_code "上传 HTML 伪装 .png" 201 "$code"
XSS_ASSET=$(jq -c '.asset' "$WORK/body")
XSS_PATH=$(printf '%s' "$XSS_ASSET" | jq -r '.path')
XSS_SHA=$(printf '%s' "$XSS_ASSET" | jq -r '.sha256')
XSS_SIZE=$(printf '%s' "$XSS_ASSET" | jq -r '.size_bytes')
XSS_ID=$(printf '%s' "$XSS_ASSET" | jq -r '.id')
check_eq "入库 MIME 为嗅探结果（未回退扩展名）" "text/html" "$(printf '%s' "$XSS_ASSET" | jq -r '.mime_type')"

code=$(http GET "$BASE/api/v1/assets/$XSS_ID/preview")
check_code "预览该资产（声明 text/html 与内容一致）" 200 "$code"
check_true "预览响应含 X-Content-Type-Options: nosniff" \
  "$(grep -qi '^x-content-type-options: *nosniff' "$WORK/hdr" && echo 0 || echo 1)"
check_true "预览响应含 CSP sandbox" \
  "$(grep -qi '^content-security-policy:.*sandbox' "$WORK/hdr" && echo 0 || echo 1)"

# 元数据端点声明 image/png，但 path 指向上面那段 HTML 内容 → 预览必须 415
FAKE_SHA=$(python3 -c 'import secrets;print(secrets.token_hex(32))')
evil_payload=$(jq -nc --arg p "$XSS_PATH" --arg s "$FAKE_SHA" --argjson n "$XSS_SIZE" \
  '{name:"p2-evil-png.png", path:$p, sha256:$s, size_bytes:$n, mime_type:"image/png", resource_type:"image"}')
code=$(http POST "$BASE/api/v1/assets" -H 'Content-Type: application/json' -d "$evil_payload")
check_code "登记声明 image/png 的伪装资产" 201 "$code"
EVIL_ID=$(jget '.asset.id')
check_eq "伪装资产声明 MIME" "image/png" "$(jget '.asset.mime_type')"

code=$(http GET "$BASE/api/v1/assets/$EVIL_ID/preview")
check_code "预览伪装资产（内容与声明不一致）" 415 "$code"

# --------------------------------------------------- S3 文本家族误拒回归（AC-02）
say "S3 A2-04 文本家族误拒回归（AC-02）"
printf '# 标题 %s\n\n这是 Markdown 正文。\n' "$NONCE" >"$WORK/notes.md"
printf 'a,b\n%s,2\n' "${NONCE:0:8}" >"$WORK/data.csv"
printf '{"k":"v","n":1,"nonce":"%s"}\n' "$NONCE" >"$WORK/config.json"
# 末尾追加随机字节：内容唯一（避免命中既有去重库），PNG 头仍决定嗅探结果与尺寸。
python3 - "$WORK/photo.png" <<'PY'
import base64, os, sys
png = base64.b64decode(
    'iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAGklEQVQYV2NkYPj/n4GBgYGRgYGBAQQABQAExgABKQK1NwAAAABJRU5ErkJggg=='
)
open(sys.argv[1], 'wb').write(png + os.urandom(16))
PY

# 每个文件：先上传拿到内容寻址对象，再以「客户端声明的家族 MIME」登记一个孪生资产
# （上传端点存的是嗅探结果，声明与嗅探不同的情形只能由元数据端点构造，正是 A2-04 的威胁模型）。
idx=0
for spec in "notes.md:text/markdown" "data.csv:text/csv" "config.json:application/json" "photo.png:image/png"; do
  idx=$((idx + 1))
  file=${spec%%:*}
  declared=${spec##*:}

  code=$(http POST "$BASE/api/v1/assets/upload" -F "file=@$WORK/$file;filename=p2-$file")
  check_code "上传 $file" 201 "$code"
  up_path=$(jget '.asset.path')
  up_size=$(jget '.asset.size_bytes')

  # 每次运行用唯一 sha：元数据端点对 sha256 有唯一约束，固定值会在第二次运行起撞上残留行（返回 200 existing）。
  twin_sha=$(python3 -c 'import secrets;print(secrets.token_hex(32))')
  twin_payload=$(jq -nc --arg p "$up_path" --arg s "$twin_sha" --argjson n "$up_size" --arg m "$declared" \
    '{name:("p2-twin-" + $m), path:$p, sha256:$s, size_bytes:$n, mime_type:$m, resource_type:"document"}')
  code=$(http POST "$BASE/api/v1/assets" -H 'Content-Type: application/json' -d "$twin_payload")
  check_code "登记声明 $declared 的孪生资产（$file 的真实字节）" 201 "$code"
  twin_id=$(jget '.asset.id')

  code=$(http GET "$BASE/api/v1/assets/$twin_id/preview")
  check_code "预览 声明=$declared 内容=$file" 200 "$code"
  size=$(curl -sS -o /dev/null -w '%{size_download}' "$BASE/api/v1/assets/$twin_id/preview")
  check_true "预览 $file 返回非空字节（$size bytes）" "$([ "$size" -gt 0 ] && echo 0 || echo 1)"
done

# ------------------------------------------------------- S4 无 /tmp 落盘（AC-03）
say "S4 A2-01 大文件不落容器 /tmp（AC-03）"
head -c 12M /dev/urandom >"$WORK/big.txt"
before=$(bytes_of "$(docker ps --size --filter "name=$SERVER_CT" --format '{{.Size}}' | awk '{print $1}')")
code=$(http POST "$BASE/api/v1/assets/upload" -F "file=@$WORK/big.txt;filename=p2-big.txt")
check_code "上传 12MiB 文件" 201 "$code"
after=$(bytes_of "$(docker ps --size --filter "name=$SERVER_CT" --format '{{.Size}}' | awk '{print $1}')")
delta=$((after - before))
printf 'INFO  可写层 before=%s bytes after=%s bytes delta=%s bytes\n' "$before" "$after" "$delta"
check_true "容器可写层增量 < 4MiB（12MiB 未落在 /tmp）" "$([ "$delta" -lt 4194304 ] && echo 0 || echo 1)"
tmpcount=$(docker exec "$SERVER_CT" sh -c 'ls -A /tmp | wc -l')
check_eq "容器 /tmp 条目数" 0 "$tmpcount"

# --------------------------------------------- S5 请求体上限跟随配置（AC-04）
say "S5 A2-03 请求体上限跟随 MAX_UPLOAD_BYTES（AC-04）"
cleanup_cap_data
mkdir -p "$CAP_DATA"
chmod 777 "$CAP_DATA"
docker rm -f p2-cap-test >/dev/null 2>&1 || true
docker run -d --name p2-cap-test --network "$NET" -p 127.0.0.1:8081:8080 \
  -e MAX_UPLOAD_BYTES=1048576 -e STORAGE_ROOT=/data/assets -e WEB_DIST= \
  -v "$CAP_DATA:/data" "$IMAGE" >/dev/null
if wait_code "$CAP_BASE/healthz" 200 30; then pass "一次性容器（MAX_UPLOAD_BYTES=1MiB）就绪"; else fail "一次性容器未就绪"; fi

head -c 2M /dev/urandom >"$WORK/over.png"
code=$(http POST "$CAP_BASE/api/v1/assets/upload" -F "file=@$WORK/over.png;filename=p2-over.png")
check_code "1MiB 上限下上传 2MiB" 413 "$code"

head -c 512K /dev/urandom >"$WORK/under.png"
code=$(http POST "$CAP_BASE/api/v1/assets/upload" -F "file=@$WORK/under.png;filename=p2-under.png")
check_code "1MiB 上限下上传 512KiB" 201 "$code"

docker rm -f p2-cap-test >/dev/null 2>&1 || true
cleanup_cap_data

# ------------------------------- S6 孤儿清理（AC-05）+ 依赖故障探针（AC-06）
say "S6 A2-05 孤儿文件清理 + A2-02 探针语义分离（AC-05/AC-06）"
docker stop "$PG_CT" >/dev/null
if wait_code "$BASE/readyz" 503 30; then pass "/readyz 在 PG 停机后转为 503"; else fail "/readyz 未在 30s 内转为 503"; fi
code=$(http GET "$BASE/readyz")
check_eq "/readyz 标记 postgres 未就绪" "false" "$(dep_ok "$WORK/body" postgres)"
check_eq "/readyz 标记 meilisearch 仍然就绪" "true" "$(dep_ok "$WORK/body" meilisearch)"
code=$(http GET "$BASE/healthz")
check_code "/healthz 在依赖故障时仍 200（纯存活语义）" 200 "$code"

head -c 64K /dev/urandom >"$WORK/orphan.png"
ORPHAN_SHA=$(sha256sum "$WORK/orphan.png" | awk '{print $1}')
code=$(http POST "$BASE/api/v1/assets/upload" -F "file=@$WORK/orphan.png;filename=p2-orphan.png")
check_code "PG 停机时上传（入库必失败）" 500 "$code"
if docker exec "$SERVER_CT" sh -c "test -e /data/assets/${ORPHAN_SHA:0:2}/${ORPHAN_SHA:2}"; then
  fail "孤儿文件未清理：/data/assets/${ORPHAN_SHA:0:2}/${ORPHAN_SHA:2} 仍存在"
else
  pass "孤儿文件已清理（/data/assets/${ORPHAN_SHA:0:2}/${ORPHAN_SHA:2} 不存在）"
fi

docker start "$PG_CT" >/dev/null
if wait_code "$BASE/readyz" 200 90; then pass "/readyz 在 PG 恢复后回到 200"; else fail "/readyz 未在 90s 内恢复 200"; fi

docker stop "$MEILI_CT" >/dev/null
if wait_code "$BASE/readyz" 503 30; then pass "/readyz 在 Meili 停机后转为 503"; else fail "/readyz 未在 30s 内转为 503"; fi
code=$(http GET "$BASE/readyz")
check_eq "/readyz 标记 meilisearch 未就绪" "false" "$(dep_ok "$WORK/body" meilisearch)"
code=$(http GET "$BASE/healthz")
check_code "/healthz 在 Meili 故障时仍 200" 200 "$code"

docker start "$MEILI_CT" >/dev/null
if wait_code "$BASE/readyz" 200 240; then pass "/readyz 在 Meili 恢复后回到 200"; else fail "/readyz 未在 240s 内恢复 200"; fi

# ----------------------------------------------------- S7 慢速上传（AC-07）
say "S7 A2-06 慢速大文件上传（AC-07）"
head -c 4M /dev/urandom >"$WORK/slow.png"
result=$(curl -sS -o "$WORK/body" -w '%{http_code} %{time_total}' --max-time 300 --limit-rate 100k \
  -F "file=@$WORK/slow.png;filename=p2-slow.png" "$BASE/api/v1/assets/upload" 2>"$WORK/slow.err" || true)
code=${result%% *}
secs=${result##* }
printf 'INFO  慢速上传状态=%s 用时=%ss（服务端常规读/写超时 15s/30s）\n' "$code" "$secs"
check_code "4MiB @100KB/s 慢速上传" 201 "$code"
check_true "上传耗时 > 35s（超过旧 15s 读超时与 30s 写超时）" \
  "$(python3 -c "import sys;sys.exit(0 if float('${secs:-0}')>35 else 1)" && echo 0 || echo 1)"

# ------------------------------------------------------------------- 汇总
say "汇总"
printf 'PASS=%d FAIL=%d\n' "$PASSED" "$FAILED"
if [ "$FAILED" -eq 0 ]; then
  echo "RESULT=PASS"
  exit 0
else
  echo "RESULT=FAIL"
  exit 1
fi
