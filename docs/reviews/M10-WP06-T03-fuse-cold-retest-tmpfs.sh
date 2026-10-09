#!/bin/sh
set -e
sed -i 's|http://archive.ubuntu.com|http://mirrors.aliyun.com|g; s|http://security.ubuntu.com|http://mirrors.aliyun.com|g' /etc/apt/sources.list 2>/dev/null || true
apt-get update -qq >/dev/null 2>&1
apt-get install -y -qq python3 build-essential pkg-config libsqlite3-dev >/dev/null 2>&1
export PATH=/rustup/toolchains/1.94.0-x86_64-unknown-linux-gnu/bin:/cargo/bin:$PATH
export RUSTUP_HOME=/rustup CARGO_HOME=/cargo RUSTUP_AUTO_INSTALL=0
echo "== toolchain: $(rustc --version) =="
CARGO_TARGET_DIR=/tmp/target cargo build --release --offline -p partisync-fuse 2>&1 | tail -1
mkdir -p /tmp/bench/mnt /tmp/bench/cas /out
mount -t tmpfs -o size=256m tmpfs /tmp/bench 2>/dev/null || { mkdir -p /tmp/bench/backing; mount -t tmpfs -o size=256m tmpfs /tmp/bench/backing; }
mkdir -p /tmp/bench/backing /tmp/bench/mnt /tmp/bench/cas
df -h /tmp/bench/backing | tail -1
cp /bench64.bin /tmp/bench/backing/bench64.bin
sync
D=$(cat /out/digest.txt)
mkdir -p "/tmp/bench/cas/objects/${D%${D#??}}"
cp /tmp/bench/backing/bench64.bin "/tmp/bench/cas/objects/${D%${D#??}}/$D"
/tmp/target/release/partifuse /tmp/bench/backing /tmp/bench/mnt --cas /tmp/bench/cas 2>/tmp/bench/fuse.log &
FUSE_PID=$!
ok=""
for i in $(seq 1 60); do ls /tmp/bench/mnt >/dev/null 2>&1 && ok=1 && break; sleep 0.25; done
[ -n "$ok" ] || { echo MOUNT_FAIL; cat /tmp/bench/fuse.log; exit 1; }
echo MOUNT_OK
sync && echo 3 > /proc/sys/vm/drop_caches && echo DROP_OK
python3 - "$D" <<'PYEOF' | tee /out/results-tmpfs.txt
import os, sys, time, random
digest = sys.argv[1]
MP  = "/tmp/bench/mnt/bench64.bin"
BYH = f"/tmp/bench/mnt/by-hash/blake3/{digest}"
MIB = 1024 * 1024
fsize = 64 * MIB
def drop():
    with open("/proc/sys/vm/drop_caches", "w") as f: f.write("3\n")
    time.sleep(0.2)
def read_at(path, off, n):
    t0 = time.perf_counter()
    fh = os.open(path, os.O_RDONLY); os.lseek(fh, off, 0); os.read(fh, n); os.close(fh)
    return (time.perf_counter() - t0) * 1e3
def seq_read(path):
    t0 = time.perf_counter()
    with open(path, "rb") as f:
        while f.read(4 * MIB): pass
    return time.perf_counter() - t0
print("== 目录透传 (tmpfs 后备 = M8 口径) ==")
cold = []
for i in range(5):
    drop(); cold.append(read_at(MP, i * 12 * MIB, 4 * MIB))
hot = [read_at(MP, i * 12 * MIB, 4 * MIB) for i in range(5)]
print(f"COLD_4MIX5  {[f'{x:.2f}' for x in cold]} ms")
print(f"HOT_4MIX5   {[f'{x:.2f}' for x in hot]} ms")
seq_s = seq_read(MP)
print(f"SEQ_64M     {fsize/seq_s/MIB:.0f} MiB/s ({seq_s*1000:.1f} ms)")
random.seed(0x9E3779B97F4A7C15)
lat = sorted(read_at(MP, random.randrange(fsize - 4096), 4096) * 1000 for _ in range(200))
print(f"RANDOM_4KB  n=200 p50={lat[100]:.0f} p95={lat[190]:.0f} max={lat[-1]:.0f} us")
print("== by-hash (tmpfs) ==")
cold_b = []
for i in range(5):
    drop(); cold_b.append(read_at(BYH, i * 12 * MIB, 4 * MIB))
hot_b = [read_at(BYH, i * 12 * MIB, 4 * MIB) for i in range(5)]
print(f"COLD_4MIX5  {[f'{x:.2f}' for x in cold_b]} ms")
print(f"HOT_4MIX5   {[f'{x:.2f}' for x in hot_b]} ms")
seq_bs = seq_read(BYH)
print(f"SEQ_64M     {fsize/seq_bs/MIB:.0f} MiB/s ({seq_bs*1000:.1f} ms)")
random.seed(0x9E3779B97F4A7C15)
lat_b = sorted(read_at(BYH, random.randrange(fsize - 4096), 4096) * 1000 for _ in range(200))
print(f"RANDOM_4KB  n=200 p50={lat_b[100]:.0f} p95={lat_b[190]:.0f} max={lat_b[-1]:.0f} us")
print("== B1 断言 (tmpfs 口径) ==")
allok = True
for name, cas, dr in [("cold_4Mi", sorted(cold_b)[2], sorted(cold)[2]),
                      ("hot_4Mi",  sorted(hot_b)[2],  sorted(hot)[2]),
                      ("rand_4Ki", lat_b[100],        lat[100])]:
    limit = max(3 * dr, 50.0)
    ok = cas <= limit
    allok = allok and ok
    print(f"{name}: cas={cas:.2f} dir={dr:.2f} limit={limit:.2f} -> {'PASS' if ok else 'FAIL'}")
ok = seq_bs <= 3 * seq_s
allok = allok and ok
print(f"seq_64M: cas={seq_bs:.2f}s dir={seq_s:.2f}s limit={3*seq_s:.2f}s -> {'PASS' if ok else 'FAIL'}")
print("B1_ASSERT", "PASS" if allok else "FAIL")
PYEOF
kill $FUSE_PID 2>/dev/null || true
sleep 0.5
umount /tmp/bench/mnt 2>/dev/null || true
umount /tmp/bench/backing 2>/dev/null || true
echo "== done =="
