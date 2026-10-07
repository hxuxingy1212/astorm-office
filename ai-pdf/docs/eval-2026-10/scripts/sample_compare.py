#!/usr/bin/env python3
"""抽样 100 份成功 round-trip 样例：现场重跑 unpack+repack，渲染原图与重建图，
计算像素差异，生成并排对比图供目检。"""
import json
import random
import statistics
import subprocess
import sys
from pathlib import Path

import numpy as np
import pymupdf

BIN = "/Users/xuxin/Desktop/workspace/ai-pdf/target/release/ai-pdf"
RESULTS = Path("/tmp/pdf-eval/results.jsonl")
OUT = Path("/tmp/pdf-eval/samples")
CORPUS = Path("/tmp/pdf-eval/corpus")
WORK = Path("/tmp/pdf-eval/sample_work")
N = 100
DPI = 100
MAX_PAGES = 3
TIMEOUT = 90


def render_page(pdf_path: Path, pno: int) -> np.ndarray | None:
    try:
        doc = pymupdf.open(pdf_path)
        if pno >= doc.page_count:
            doc.close()
            return None
        page = doc[pno]
        pix = page.get_pixmap(dpi=DPI)
        arr = np.frombuffer(pix.samples, dtype=np.uint8).reshape(pix.height, pix.width, pix.n)
        if pix.n == 4:
            arr = arr[..., :3]
        gray = arr.mean(axis=2)
        doc.close()
        return gray
    except Exception:
        return None


def diff_score(a: np.ndarray, b: np.ndarray) -> float:
    """归一化像素差（0-100）。尺寸不一致时截到公共尺寸再比。"""
    if a.size == 0 or b.size == 0:
        return 100.0
    h = min(a.shape[0], b.shape[0])
    w = min(a.shape[1], b.shape[1])
    if h < 10 or w < 10:
        return 100.0
    a2 = a[:h, :w].astype(np.float32)
    b2 = b[:h, :w].astype(np.float32)
    return float(np.abs(a2 - b2).mean() / 255.0 * 100.0)


def side_by_side(pdf_a: Path, pdf_b: Path, out_png: Path):
    """每页 上=原 下=重建，最多两页。"""
    doc_a = pymupdf.open(pdf_a)
    doc_b = pymupdf.open(pdf_b)
    n = min(2, doc_a.page_count)
    combos = []
    for p in range(n):
        pa = doc_a[p].get_pixmap(dpi=DPI)
        pb = doc_b[p].get_pixmap(dpi=DPI) if p < doc_b.page_count else None
        combos.append((pa, pb))
    doc_a.close()
    doc_b.close()
    from PIL import Image
    import io
    rows = []
    for pa, pb in combos:
        rows.append(Image.open(io.BytesIO(pa.tobytes("png"))))
        if pb is not None:
            rows.append(Image.open(io.BytesIO(pb.tobytes("png"))))
        else:
            rows.append(Image.new("RGB", (rows[-1].width, rows[-1].height), (255, 0, 255)))
    w = max(im.width for im in rows)
    h = sum(im.height for im in rows) + 8 * len(rows)
    canvas = Image.new("RGB", (w, h), (128, 128, 128))
    y = 0
    for im in rows:
        canvas.paste(im, (0, y))
        y += im.height + 8
    canvas.save(out_png)


def rebuild(rel: str) -> Path | None:
    """现场重跑 unpack+repack，返回重建 PDF 路径。"""
    pdf = CORPUS / rel
    tag = rel.replace("/", "__").replace(".pdf", "")
    art = WORK / tag / "art"
    rebuilt = WORK / tag / "rebuilt.pdf"
    art.parent.mkdir(parents=True, exist_ok=True)
    r = subprocess.run([BIN, "unpack", str(pdf), "-o", str(art)],
                       capture_output=True, text=True, timeout=TIMEOUT)
    if r.returncode != 0:
        return None
    r2 = subprocess.run([BIN, "repack", str(art), "-o", str(rebuilt)],
                        capture_output=True, text=True, timeout=TIMEOUT)
    if r2.returncode != 0:
        return None
    return rebuilt


def main():
    recs = [json.loads(l) for l in RESULTS.read_text().splitlines() if l.strip()]
    ok = [r for r in recs if r["unpack"] == "ok" and r["repack"] == "ok"]
    print(f"round-trip ok: {len(ok)} / {len(recs)}", flush=True)

    # 分层：按 source 分组，按比例分配 100 个名额
    random.seed(42)
    by_src: dict[str, list] = {}
    for r in ok:
        by_src.setdefault(r["source"], []).append(r)
    picked = []
    for src, lst in by_src.items():
        k = round(N * len(lst) / len(ok))
        picked.extend(random.sample(lst, min(k, len(lst))))
    while len(picked) > N:
        picked.pop(random.randrange(len(picked)))
    while len(picked) < N and len(picked) < len(ok):
        cand = random.choice(ok)
        if cand not in picked:
            picked.append(cand)
    print(f"sampled: {len(picked)}", flush=True)

    OUT.mkdir(parents=True, exist_ok=True)
    results = []
    for i, r in enumerate(picked):
        orig = CORPUS / r["file"]
        try:
            rebuilt = rebuild(r["file"])
        except subprocess.TimeoutExpired:
            rebuilt = None
        scores = []
        pages_ok = 0
        if rebuilt is not None:
            for p in range(MAX_PAGES):
                a = render_page(orig, p)
                b = render_page(rebuilt, p)
                if a is None or b is None:
                    break
                pages_ok += 1
                scores.append(diff_score(a, b))
            if pages_ok > 0:
                side_by_side(orig, rebuilt, OUT / f"{i:03d}.png")
        entry = {**{k: r.get(k) for k in ("file", "source", "size", "elements", "images", "warnings")},
                 "rebuild_ok": rebuilt is not None,
                 "pages_compared": pages_ok,
                 "diff_pages": [round(s, 2) for s in scores],
                 "diff_mean": round(statistics.mean(scores), 2) if scores else None,
                 "diff_max": round(max(scores), 2) if scores else None}
        results.append(entry)
        json.dump({"i": i, **entry}, open(OUT / f"{i:03d}.json", "w"), ensure_ascii=False)
        print(f"[{i+1}/{len(picked)}] mean={entry['diff_mean']} pages={pages_ok} {entry['file']}", flush=True)

    json.dump(results, open("/tmp/pdf-eval/diff_summary.json", "w"), ensure_ascii=False, indent=1)
    ranked = sorted([r for r in results if r["diff_mean"] is not None], key=lambda r: -r["diff_mean"])
    print("\n=== worst 15 ===")
    for r in ranked[:15]:
        print(f"{r['diff_mean']:6.2f} max={r['diff_max']:6.2f} {r['file']}")
    if ranked:
        print("\n=== median ===", statistics.median([r["diff_mean"] for r in ranked]))
        print("=== mean ===", round(statistics.mean([r["diff_mean"] for r in ranked]), 2))
        print("=== <=1.0 份数 ===", sum(1 for r in ranked if r["diff_mean"] <= 1.0))
        print("=== >5.0 份数 ===", sum(1 for r in ranked if r["diff_mean"] > 5.0))


if __name__ == "__main__":
    main()
