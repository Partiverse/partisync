#!/usr/bin/env bash
# 互操作认证矩阵（SPEC M1-WP08；CI 首跑解除 D10 后接入 job）。
# 前置：partisync-cli 已构建（target/debug/partisync-cli）、rclone 在 PATH。
# 用法: scripts/interop.sh [数据根]（默认临时目录，跑完自清理）
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"

DATA="${1:-$(mktemp -d /tmp/ps-interop.XXXXXX)}"
PORT_S3=18081 PORT_DAV=18082 PORT_UI=18080
BIN=./target/debug/partisync-cli
export RCLONE_CONFIG="$(mktemp /tmp/ps-interop-conf.XXXXXX)"
PASS=0; FAIL=0

check() { # check <名称> <命令...>
  local name="$1"; shift
  if "$@" >/dev/null 2>&1; then
    echo "  PASS  $name"; PASS=$((PASS+1))
  else
    echo "  FAIL  $name"; FAIL=$((FAIL+1))
  fi
}

cleanup() {
  [[ -n "${SRV_PID:-}" ]] && kill "$SRV_PID" 2>/dev/null
  rm -rf "$DATA" "$RCLONE_CONFIG" /tmp/ps-ix-src /tmp/ps-ix-dst
}
trap cleanup EXIT

# —— 夹具与环境 ——
mkdir -p /tmp/ps-ix-src
for i in 1 2 3; do head -c 65536 /dev/urandom > "/tmp/ps-ix-src/f$i.bin"; done
echo "interop" > /tmp/ps-ix-src/note.txt
cat > "$RCLONE_CONFIG" <<CFG
[ps]
type = s3
provider = Other
access_key_id = demo
secret_access_key = demo
endpoint = http://127.0.0.1:$PORT_S3

[psdav]
type = webdav
url = http://127.0.0.1:$PORT_DAV/
vendor = other
user = demo
pass = $(rclone obscure demo)
CFG

# —— 服务（三端口，独立测试数据根）——
"$BIN" ui --db "$DATA/i.db" --cas "$DATA/cas" --addr "127.0.0.1:$PORT_UI" \
  --s3 "127.0.0.1:$PORT_S3" --dav "127.0.0.1:$PORT_DAV" --data "$DATA/s3data" \
  > /tmp/ps-interop-server.log 2>&1 &
SRV_PID=$!
for _ in $(seq 1 30); do
  curl -s --max-time 1 "http://127.0.0.1:$PORT_S3/" -o /dev/null && break
  sleep 0.3
done

echo "== S3 面（rclone）=="
check "lsd 列桶"        rclone lsd ps:
check "copy 上行"       rclone copy /tmp/ps-ix-src ps:bucket-ix/src
check "lsf 列对象"      rclone lsf ps:bucket-ix/src
check "copy 下行"       rclone copy ps:bucket-ix/src /tmp/ps-ix-dst
check "check 完整性"    rclone check /tmp/ps-ix-src ps:bucket-ix/src
check "moveto"          rclone moveto /tmp/ps-ix-src/note.txt ps:bucket-ix/moved.txt
check "deletefile"      rclone deletefile ps:bucket-ix/moved.txt
check "MPU 大文件"      rclone copyto /tmp/ps-ix-src/f1.bin ps:bucket-ix/big.bin --s3-upload-cutoff 32k --s3-chunk-size 5M

echo "== WebDAV 面（rclone）=="
check "PROPFIND 列表"   rclone lsf psdav:
check "copy 上行"       rclone copy /tmp/ps-ix-src psdav:bucket-ix/dav-src
check "copy 下行"       rclone copy psdav:bucket-ix/dav-src /tmp/ps-ix-dst2
check "check 完整性"    rclone check /tmp/ps-ix-src psdav:bucket-ix/dav-src

echo "== 消费面（index-remote 吃狗粮）=="
"$BIN" index-remote --scheme s3 --bucket bucket-ix --endpoint "http://127.0.0.1:$PORT_S3" \
  --db "$DATA/i.db" > /dev/null 2>&1
check "远端索引入图谱"  test -f "$DATA/i.db"
check "图谱可检索"      "$BIN" find f1 --db "$DATA/i.db"

echo "== 跨协议一致性 =="
printf "cross-%s" "$$" | curl -s -X PUT --data-binary @- "http://127.0.0.1:$PORT_DAV/bucket-ix/cross.txt" -o /dev/null
check "WebDAV 写 → S3 读" sh -c "curl -s http://127.0.0.1:$PORT_S3/bucket-ix/cross.txt | grep -q cross-$$"

echo
echo "互操作矩阵: $PASS PASS / $FAIL FAIL"
exit "$FAIL"
