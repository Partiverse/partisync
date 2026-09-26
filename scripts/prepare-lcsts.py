#!/usr/bin/env python3
"""
LCSTS 子集抽出器（SPEC M6-D67 §D7 评估集）。

输入：一个 LCSTS JSON / JSONL 文件，结构为 list[dict]，每条字段至少包含
      "summary"（=查询的 query）与 "content"（=ground-truth 文档）。
      兼容字段别名： "title" / "text"。

输出：在 OUT_DIR 下写 3 个 artifact：
  - corpus/<i>.md                # 索引用 markdown（frontmatter + 原 content）
  - corpus.tsv                   # EvalRunner 期望的 TSV 格式
  - queries.jsonl                # {"id": "qNN", "text": ..., ...}
  - qrels.jsonl                  # {"query_id": "qNN", "content_id": "dNNN", "relevance": N}

子集策略： 取前 N_DOC 文档 + 前 N_QUERY 个查询（每个查询用对应的文档作
           ground-truth）。  LCSTS 标准的 (summary → content) 是天然的相关性
           标注（summary 即 query 的 high-quality 对齐）。
           relevance 用一句话合成的"对齐强度" heuristic（基于词汇重叠），
           取值 {1, 2, 3}： 让 metrics 出现 nDCG 档位， 不强制要求人类标注。

用法:
  python3 scripts/prepare-lcsts.py /path/to/lcsts.json OUT_DIR [--docs 200] [--queries 40]
  python3 scripts/prepare-lcsts.py --help
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
from pathlib import Path
from typing import Iterable

# LCSTS 常见字段别名（中文短文摘要数据来源各异）
SUMMARY_KEYS = ("summary", "summarization", "title", "headline", "短文摘要")
CONTENT_KEYS = ("content", "text", "article", "正文")

_TOKEN_RE = re.compile(r"[\u4e00-\u9fff]+|[A-Za-z0-9]+", re.UNICODE)


def pick(d: dict, keys: tuple[str, ...]) -> str | None:
    for k in keys:
        v = d.get(k)
        if v:
            return str(v).strip()
    return None


def tokenize(s: str) -> set[str]:
    return {m.group(0).lower() for m in _TOKEN_RE.finditer(s)}


def jaccard(a: set[str], b: set[str]) -> float:
    if not a or not b:
        return 0.0
    u = a | b
    return len(a & b) / max(1, len(u))


def relevance_from_overlap(query: str, doc: str) -> int:
    """对齐强度 heuristic。 LCSTS 天然高对齐， 分三档： ≤0.10 → 1， ≤0.25 → 2， >0.25 → 3。"""
    j = jaccard(tokenize(query), tokenize(doc))
    if j > 0.25:
        return 3
    if j > 0.10:
        return 2
    return 1


def load_pairs(src: Path) -> Iterable[tuple[str, str]]:
    """Yield (summary, content) pairs. 支持 JSON list / JSONL."""
    text = src.read_text(encoding="utf-8")
    if text.lstrip().startswith("["):
        for d in json.loads(text):
            q = pick(d, SUMMARY_KEYS)
            c = pick(d, CONTENT_KEYS)
            if q and c:
                yield q, c
    else:
        for line in text.splitlines():
            line = line.strip()
            if not line:
                continue
            d = json.loads(line)
            q = pick(d, SUMMARY_KEYS)
            c = pick(d, CONTENT_KEYS)
            if q and c:
                yield q, c


def write_corpus(out_corpus: Path, ts_lines: list[str],
                 pairs: list[tuple[str, str]], n_docs: int) -> list[tuple[str, str, int, list[str]]]:
    """写 corpus/<i>.md 和 corpus.tsv. 返回 docs 元组 (cid, filename, rel, tags)."""
    out_corpus.mkdir(parents=True, exist_ok=True)
    docs = []
    for i, (summary, content) in enumerate(pairs[:n_docs]):
        cid = f"d{i+1:04d}"
        filename = f"{cid}_lcsts.md"
        rel = f"{cid}.md"
        tags = ["lcsts"]
        frontmatter = (
            f"---\n"
            f"title: LCSTS doc {cid}\n"
            f"filename: {filename}\n"
            f"updated_ns: 0\n"
            f"---\n"
        )
        body = f"# LCSTS {cid}\n\n{content.strip()}\n"
        (out_corpus / rel).write_text(frontmatter + body, encoding="utf-8")
        # TSV 行（与 EvalRunner 期望格式严格对齐）
        ts_lines.append(f"{cid}\t{rel}\t{filename}\tLCSTS doc {cid}\ttext/markdown\t0\tlcsts")
        docs.append((cid, rel, filename, tags))
    return docs


def write_queries_qrels(out_dir: Path,
                        pairs: list[tuple[str, str]],
                        docs: list[tuple[str, str, str, list[str]]],
                        n_queries: int) -> int:
    """写 queries.jsonl + qrels.jsonl. n_queries ≤ len(pairs)；截断至 docs 末端。"""
    q_path = out_dir / "queries.jsonl"
    qr_path = out_dir / "qrels.jsonl"
    n_q = 0
    with q_path.open("w", encoding="utf-8") as fq, qr_path.open("w", encoding="utf-8") as fqr:
        for i, (summary, content) in enumerate(pairs[:n_queries]):
            if i >= len(docs):
                break
            cid = docs[i][0]
            qid = f"q{i+1:04d}"
            rel = relevance_from_overlap(summary, content)
            q = {
                "id": qid,
                "text": summary,
                "type": "short",
                "limit": 20,
                "include_transcript": True,
                "vector_kind": "auto",
            }
            fq.write(json.dumps(q, ensure_ascii=False) + "\n")
            fqr.write(json.dumps(
                {"query_id": qid, "content_id": cid, "relevance": rel},
                ensure_ascii=False,
            ) + "\n")
            n_q += 1
    return n_q


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("src", type=Path, help="LCSTS 源 JSON / JSONL 文件")
    p.add_argument("out_dir", type=Path, help="输出目录（corpus/ + corpus.tsv + queries.jsonl + qrels.jsonl）")
    p.add_argument("--docs", type=int, default=200, help="抽取语料文档数（默认 200）")
    p.add_argument("--queries", type=int, default=40, help="抽取查询数（默认 40 ≤ docs）")
    p.add_argument("--tsv-header", action="store_true", help="显式写 TSV header（EvalRunner 默认跳过首行）")
    args = p.parse_args()

    if not args.src.is_file():
        print(f"ERROR: 源文件不存在：{args.src}", file=sys.stderr)
        return 2
    if args.queries > args.docs:
        print(f"ERROR: --queries ({args.queries}) 必须 ≤ --docs ({args.docs})", file=sys.stderr)
        return 2

    pairs = list(load_pairs(args.src))
    if len(pairs) < args.docs:
        print(f"WARN: 源仅 {len(pairs)} 对，可用 < {args.docs}", file=sys.stderr)

    args.out_dir.mkdir(parents=True, exist_ok=True)
    ts_lines = []
    if args.tsv_header:
        ts_lines.append("content_id\trelpath\tfilename\ttitle\tmime\tupdated_ns\ttags")

    docs = write_corpus(args.out_dir / "corpus", ts_lines, pairs, args.docs)
    (args.out_dir / "corpus.tsv").write_text("\n".join(ts_lines) + "\n", encoding="utf-8")

    n_q = write_queries_qrels(args.out_dir, pairs, docs, args.queries)

    print(f"OK  out_dir = {args.out_dir}")
    print(f"    corpus/        = {len(docs)} markdown")
    print(f"    corpus.tsv     = {len(ts_lines)} 行{'（含 header）' if args.tsv_header else '（无 header, EvalRunner 自动跳首行）'}")
    print(f"    queries.jsonl  = {n_q} 条查询")
    print(f"    qrels.jsonl    = {n_q} 条 ground-truth")
    return 0


if __name__ == "__main__":
    sys.exit(main())
