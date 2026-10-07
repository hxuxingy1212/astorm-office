#!/usr/bin/env python3
"""旧版 Office 转换保真度评估

- 语料池：/Users/xuxin/office-legacy-corpus/{doc,xls,ppt}（govdocs1 真实文件）
- 抽样：每格式 2 轮 × 100 份（种子 42/43；第 2 轮与第 1 轮不重叠）
- 每份样本：
  1) 候选转换器（本仓库 CLI convert，纯 Rust）→ 候选新格式
  2) oracle（LibreOffice，仅用于评估、不随产品分发）→ oracle 新格式
  3) 用对应 CLI 的 view 命令把两份新格式都归一化为 JSON
  4) 指标：成功率、文本相似度（difflib 归一化 ratio）、结构计数、耗时
- 输出：docs/legacy-eval-data/<fmt>-r<N>.json + summary
"""
import argparse
import difflib
import glob
import json
import os
import random
import re
import subprocess
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = {
    "doc": os.path.join(ROOT, "target/release/json2docx"),
    "xls": os.path.join(ROOT, "target/release/json2xlsx"),
    "ppt": os.path.join(ROOT, "target/release/json2pptx"),
}
EXT = {"doc": "docx", "xls": "xlsx", "ppt": "pptx"}
CORPUS = "/Users/xuxin/office-legacy-corpus"


def sh(cmd, timeout=120):
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return r.returncode, r.stdout, r.stderr
    except subprocess.TimeoutExpired:
        return -1, "", "timeout"


def convert_one(fmt, src, out):
    t0 = time.time()
    code, so, se = sh([BIN[fmt], "convert", src, "-o", out], timeout=120)
    return {
        "ok": code == 0 and os.path.exists(out),
        "ms": int((time.time() - t0) * 1000),
        "err": (se or so).strip()[:200],
    }


def oracle_convert(fmt, src, out):
    with tempfile.TemporaryDirectory() as td:
        code, so, se = sh(
            ["soffice", "--headless", "--convert-to", EXT[fmt], "--outdir", td, src],
            timeout=180,
        )
        produced = glob.glob(os.path.join(td, f"*.{EXT[fmt]}"))
        if code == 0 and produced:
            os.replace(produced[0], out)
            return {"ok": True, "err": ""}
    return {"ok": False, "err": (se or "no output").strip()[:200]}


def normalize_text(s):
    return re.sub(r"\s+", " ", s).strip()


def view_json(fmt, path):
    """统一归一化视图：全文文本 + 结构计数"""
    if fmt == "doc":
        code, so, _ = sh([BIN["doc"], "unpack", path, "-o", path + ".unpack"], timeout=120)
        blocks = []
        import glob as g
        for f in sorted(g.glob(path + ".unpack/word/parts/*.json")):
            d = json.load(open(f))
            blocks.extend(d.get("blocks", []))
        texts = [normalize_text(json.dumps(b.get("text") or "", ensure_ascii=False)) for b in blocks]
        texts = [t for t in texts if t and t != '""']
        return {
            "text": normalize_text(" ".join(texts)),
            "tables": sum(1 for b in blocks if b.get("type") == "table"),
            "images": sum(1 for b in blocks if b.get("type") == "image"),
        }
    if fmt == "xls":
        code, so, _ = sh([BIN["xls"], "view", path, "/sheet[1]", "text"], timeout=120)
        d = json.loads(so) if so.strip().startswith("{") else {}
        rows = d.get("rows", [])
        cells = [normalize_text(str(c.get("value") or "")) for r in rows for c in r.get("cells", [])]
        return {
            "text": normalize_text(" ".join(cells)),
            "cells": len(cells),
            "rows": len(rows),
        }
    code, so, _ = sh([BIN["ppt"], "view", path, "/slide[1]", "text"], timeout=120)
    d = json.loads(so) if so.strip().startswith("{") else {}
    texts = [normalize_text(t.get("text") or "") for t in d.get("texts", [])]
    return {"text": normalize_text(" ".join(texts)), "slides": d.get("slide")}


def similarity(a, b):
    if not a and not b:
        return 1.0
    if not a or not b:
        return 0.0
    return difflib.SequenceMatcher(None, a, b).ratio()


def run_round(fmt, files):
    results = []
    for i, src in enumerate(files):
        rec = {"i": i, "file": os.path.basename(src), "src_bytes": os.path.getsize(src)}
        tdir = tempfile.mkdtemp(prefix="legacy-eval-")
        cand = os.path.join(tdir, f"cand.{EXT[fmt]}")
        orc = os.path.join(tdir, f"oracle.{EXT[fmt]}")
        rec["convert"] = convert_one(fmt, src, cand)
        rec["oracle"] = oracle_convert(fmt, src, orc)
        mine_full = ref_full = None
        if rec["convert"]["ok"]:
            try:
                mine_full = view_json(fmt, cand)
                rec["mine_view"] = {
                    "text_len": len(mine_full["text"]),
                    "struct": {k: v for k, v in mine_full.items() if k != "text"},
                }
            except Exception as e:
                rec["convert"]["err"] = f"view failed: {e}"[:200]
        if rec["oracle"]["ok"]:
            try:
                ref_full = view_json(fmt, orc)
            except Exception:
                rec["oracle"]["ok"] = False
        if mine_full is not None and ref_full is not None:
            rec["sim"] = round(similarity(mine_full["text"], ref_full["text"]), 4)
            rec["struct"] = {
                k: {"mine": mine_full.get(k), "oracle": ref_full.get(k)}
                for k in mine_full
                if k != "text"
            }
        else:
            rec["sim"] = None
        results.append(rec)
        subprocess.run(["rm", "-rf", tdir])
        if (i + 1) % 20 == 0:
            print(f"  {fmt} {i + 1}/100", flush=True)
    return results


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--round", type=int, required=True)
    ap.add_argument("--formats", default="doc,xls,ppt")
    ap.add_argument("--outdir", default=None)
    args = ap.parse_args()
    outdir = args.outdir or os.path.join(
        ROOT, "docs", "legacy-eval-data", f"round{args.round}"
    )
    os.makedirs(outdir, exist_ok=True)
    summary = {}
    for fmt in args.formats.split(","):
        rnd = random.Random(42)
        pool = sorted(glob.glob(f"{CORPUS}/{fmt}/*"))
        if len(pool) < 200:
            print(f"{fmt}: 池不足 200，跳过")
            continue
        r1 = rnd.sample(pool, 100)
        files = r1 if args.round == 1 else rnd.sample(
            [p for p in pool if p not in set(r1)], 100
        )
        print(f"== {fmt} round{args.round}: {len(files)} 样本 ==", flush=True)
        t0 = time.time()
        results = run_round(fmt, files)
        el = int(time.time() - t0)
        sims = [r["sim"] for r in results if r.get("sim") is not None]
        rec = {
            "format": fmt,
            "round": args.round,
            "samples": len(results),
            "convert_ok": sum(1 for r in results if r["convert"]["ok"]),
            "oracle_ok": sum(1 for r in results if r["oracle"]["ok"]),
            "compared": len(sims),
            "sim_median": sorted(sims)[len(sims) // 2] if sims else None,
            "sim_mean": round(sum(sims) / len(sims), 4) if sims else None,
            "sim_min": min(sims) if sims else None,
            "convert_ms_median": sorted(r["convert"]["ms"] for r in results)[len(results) // 2],
            "elapsed_s": el,
        }
        summary[fmt] = rec
        with open(os.path.join(outdir, f"{fmt}-r{args.round}.json"), "w") as f:
            json.dump({"summary": rec, "results": results}, f, ensure_ascii=False, indent=1)
        print(json.dumps(rec, ensure_ascii=False), flush=True)
    with open(os.path.join(outdir, f"summary-r{args.round}.json"), "w") as f:
        json.dump(summary, f, ensure_ascii=False, indent=1)


if __name__ == "__main__":
    main()
