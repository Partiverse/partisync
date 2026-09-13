#!/usr/bin/env bash
# partisync 测试残留清理（P2 收尾 ①，用户 2026-09-13 批准）
#
# 范围：删除「验证期测试资产」——DB 行（assets / annotation_jobs / asset_tags /
# 失去引用的 tags）+ Meilisearch 对应文档 + 资产卷内不再被引用的对象。
# 按显式名称模式匹配，绝不 TRUNCATE；非匹配名的资产一概不动。
# 已核实（2026-09-13）：本 dev 实例 235/235 资产均匹配残留模式，匹配后余 0。
#
# 运行：bash scripts/cleanup-test-residue.sh   （幂等，可重复执行）
set -uo pipefail

PG_CT=${PG_CT:-partisync-postgres}
MEILI_URL=${MEILI_URL:-http://127.0.0.1:7700}
MEILI_KEY=${MEILI_KEY:-partisync_master_key_for_dev_only_32_chars_long}
PSQL=(docker exec "$PG_CT" psql -U partisync -d partisync -tAc)

# 残留名称模式（审计/浏览器回归/L3/L2 各轮上传的测试文件）。
PATTERNS="name LIKE 'p2-%' OR name LIKE 'p2.%' OR name LIKE 'p22-%' OR name LIKE 's8-%' OR name LIKE 's2-%' \
OR name LIKE 'l3-%' OR name LIKE 't4-%' OR name LIKE 't6-%' OR name LIKE 'evil%' \
OR name LIKE 'selfaudit-%' OR name LIKE 'probe-%' OR name LIKE 'audit-%' \
OR name LIKE 'big%' OR name LIKE 'img-a-%' OR name LIKE 'aaaa%' OR name LIKE '..%' \
OR name IN ('doc-report.pdf','first.txt','hero.png','html-doc.png','html-inline','mismatch','ok.md','readme.txt','regress.txt','small100k.png','tiny.png','a.md','mm.md','h.md','x.md','x.pdf','x.png','p.txt','t.txt','pw-smoke.png','passwd.png','1m.txt','test-l2.jpg','\$(whoami).md')"

say() { printf '\n=== %s ===\n' "$*"; }

say "匹配残留"
MATCHED=$("${PSQL[@]}" "SELECT count(*) FROM assets WHERE ($PATTERNS)")
echo "matched assets: $MATCHED"

IDS_FILE=$(mktemp)
if [ "${MATCHED:-0}" -gt 0 ]; then
  say "DB 删除（annotation_jobs → asset_tags → 孤儿 tags → assets）"
  "${PSQL[@]}" "DELETE FROM annotation_jobs WHERE asset_id IN (SELECT id FROM assets WHERE ($PATTERNS)); SELECT 'jobs deleted: ' || count(*) FROM annotation_jobs;" | tail -1
  "${PSQL[@]}" "DELETE FROM asset_tags WHERE asset_id IN (SELECT id FROM assets WHERE ($PATTERNS)); SELECT 'asset_tags left: ' || count(*) FROM asset_tags;" | tail -1
  "${PSQL[@]}" "DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM asset_tags); SELECT 'tags left: ' || count(*) FROM tags;" | tail -1
  # 先导出待删 Meili 文档 id，再删行。
  "${PSQL[@]}" "SELECT id FROM assets WHERE ($PATTERNS)" > "$IDS_FILE"
  "${PSQL[@]}" "DELETE FROM assets WHERE ($PATTERNS); SELECT 'assets left: ' || count(*) FROM assets;" | tail -1

  say "Meilisearch delete-batch"
  IDS_JSON=$(jq -R -s 'split("\n") | map(select(length > 0))' "$IDS_FILE")
  curl -sS -X POST "$MEILI_URL/indexes/assets/documents/delete-batch" \
    -H "Authorization: Bearer $MEILI_KEY" -H 'Content-Type: application/json' \
    -d "$IDS_JSON"; echo
else
  echo "nothing to clean in DB"
fi

say "资产卷孤儿对象清扫（恒执行：keep 集合 = 现存 assets.path）"
"${PSQL[@]}" "SELECT path FROM assets" > /tmp/cleanup-keep-paths.txt
docker exec partisync-server sh -c 'find /data/assets -type f ! -path "*/.tmp/*" | sed "s|^/data/assets/||" | sort' > /tmp/cleanup-volume-files.txt
grep -vx -F -f /tmp/cleanup-keep-paths.txt /tmp/cleanup-volume-files.txt > /tmp/cleanup-orphans.txt || true
REMOVED=$(grep -c . /tmp/cleanup-orphans.txt || true)
if [ "${REMOVED:-0}" -gt 0 ]; then
  # 路径均为服务端生成的十六进制内容寻址名（无空格/特殊字符），可安全内插。
  ORPHAN_ARGS=$(sed 's|^|/data/assets/|' /tmp/cleanup-orphans.txt | tr '\n' ' ')
  docker exec partisync-server sh -c "rm -f $ORPHAN_ARGS"
  # 删除后清掉空分片目录（保留 .tmp）。
  docker exec partisync-server sh -c 'cd /data/assets && find . -mindepth 1 -maxdepth 1 -type d ! -name .tmp -exec sh -c "rmdir \"\$1\"/* 2>/dev/null; rmdir \"\$1\" 2>/dev/null" _ {} \;'
fi
echo "orphan objects removed: $REMOVED"
echo "objects left: $(docker exec partisync-server sh -c 'find /data/assets -type f ! -path "*/.tmp/*" 2>/dev/null | wc -l')"
echo ".tmp leftover: $(docker exec partisync-server sh -c 'find /data/assets/.tmp -type f 2>/dev/null | wc -l')"

say "复核"
"${PSQL[@]}" "SELECT 'assets=' || count(*) FROM assets;"
"${PSQL[@]}" "SELECT 'jobs=' || count(*) FROM annotation_jobs;"
curl -sS "$MEILI_URL/indexes/assets/stats" -H "Authorization: Bearer $MEILI_KEY" | jq -c '{numberOfDocuments}'
rm -f "$IDS_FILE" /tmp/cleanup-keep-paths.txt /tmp/cleanup-volume-files.txt /tmp/cleanup-orphans.txt
echo "DONE"
