#!/usr/bin/env python3
"""json2pdf render 与真实物料（pymupdf 参照渲染）的抽样视觉比对 harness。

流程（对齐 docs/eval-2026-10 的既有方法）：
1. 语料池：/tmp/pdf-eval/corpus（1463 份清单 / 1324 份在盘，取自 7 个 GitHub 公开测试集）。
2. 随机抽样：seed 固定分层随机，逐个尝试「参照渲染 + 我方渲染」，双双成功才计入样本，
   直到凑满 --sample 份（跳过原因如实记录）。
3. 参照：pymupdf 按第 1 页以 --scale×72 DPI 渲染（"物料本来的样子"）。
   我方：`json2pdf render <pdf> /page[1] --scale N -o our.png`（PDF 输入自动临时解包）。
4. 产出：results.jsonl（逐份差异分）+ side-by-side JPG（上=参照 下=我方，供人工目检）。
   差异分 = 灰度平均绝对差 / 255 × 100（0=逐像素一致；字体栅格差异会贡献基础分，
   结构性错误（位置/颜色/缺失元素）表现为高分）。

用法：
  python3 scripts/pdf_render_eval.py                # 100 份，seed=7
  python3 scripts/pdf_render_eval.py --list-worst 30
"""

import argparse
import json
import random
import subprocess
import sys
import time
from pathlib import Path

CORPUS = Path("/tmp/pdf-eval/corpus")
OUT = Path("/tmp/pdf-render-eval")
JSON2PDF = Path(__file__).resolve().parent.parent / "target/release/json2pdf"


def ref_render_worker(pdf: str, png: str, scale: float) -> None:
    """在子进程中执行（畸形语料可能让 MuPDF 原生渲染死循环，只能靠进程超时回收）"""
    import pymupdf

    doc = pymupdf.open(pdf)
    if doc.is_encrypted or doc.page_count < 1:
        raise ValueError("encrypted or no pages")
    page = doc[0]
    # 与我方同 DPI：pymupdf zoom 单位是 1/72，我方 scale 单位是 1/96
    zoom = scale * 96 / 72
    long_pt = max(page.rect.width, page.rect.height)
    while long_pt * zoom > 4200:
        zoom /= 2
    pix = page.get_pixmap(matrix=pymupdf.Matrix(zoom, zoom), colorspace=pymupdf.csRGB, alpha=False)
    Path(png).write_bytes(pix.tobytes("png"))
    doc.close()


def ref_render(pdf: Path, png: Path, scale: float) -> None:
    r = subprocess.run(
        [sys.executable, __file__, "--ref-worker", str(pdf), str(png), str(scale)],
        capture_output=True,
        text=True,
        timeout=60,
    )
    if r.returncode != 0 or not png.exists():
        raise RuntimeError(f"pymupdf: {(r.stderr or r.stdout).strip().splitlines()[-1][:100]}")


def our_render(pdf: Path, png: Path, scale: float) -> None:
    # 与参照同约束：超长页按比例减小 scale（最长边 ≈4200px）
    import pymupdf

    doc = pymupdf.open(pdf)
    long_pt = max(doc[0].rect.width, doc[0].rect.height)
    doc.close()
    s = scale
    while long_pt * s * 96 / 72 > 4200:
        s /= 2
    r = subprocess.run(
        [str(JSON2PDF), "render", str(pdf), "/page[1]", "--scale", str(s), "-o", str(png), "--quiet"],
        capture_output=True,
        text=True,
        timeout=120,
    )
    if r.returncode != 0 or not png.exists():
        raise RuntimeError(r.stderr.strip().splitlines()[-1] if r.stderr.strip() else "no output")


def diff_score(ref_png: Path, our_png: Path) -> float:
    import numpy as np
    from PIL import Image

    a = Image.open(ref_png).convert("L")
    b = Image.open(our_png).convert("L")
    if b.size != a.size:
        b = b.resize(a.size)
    da = np.asarray(a, dtype=np.int16)
    db = np.asarray(b, dtype=np.int16)
    return float(np.abs(da - db).mean()) / 255 * 100


def side_by_side(ref_png: Path, our_png: Path, out: Path, label: str, score: float) -> None:
    from PIL import Image, ImageDraw

    a = Image.open(ref_png).convert("RGB")
    b = Image.open(our_png).convert("RGB")
    if b.size != a.size:
        b = b.resize(a.size)
    w, h = a.size
    target_w = 860
    if w > target_w:
        nh = round(h * target_w / w)
        a = a.resize((target_w, nh))
        b = b.resize((target_w, nh))
        w, h = a.size
    banner = 22
    canvas = Image.new("RGB", (w, h * 2 + banner * 2), "#202124")
    draw = ImageDraw.Draw(canvas)
    canvas.paste(a, (0, banner))
    draw.text((6, 4), f"REF  {label}  diff={score:.1f}", fill="#9ad0ff")
    canvas.paste(b, (0, h + banner * 2))
    draw.text((6, h + banner + 4), f"OURS {label}", fill="#9fc3f0")
    canvas.save(out, quality=80)


def attempt(args):
    pdf, scale, i = args
    rel = pdf.relative_to(CORPUS)
    source = rel.parts[0]
    stem = rel.as_posix().replace("/", "__").removesuffix(".pdf")
    ref_png = OUT / "ref" / f"{i:03d}.png"
    our_png = OUT / "our" / f"{i:03d}.png"
    rec = {"file": rel.as_posix(), "source": source}
    try:
        t0 = time.time()
        ref_render(pdf, ref_png, scale)
        our_render(pdf, our_png, scale)
        rec["secs"] = round(time.time() - t0, 1)
        rec["diff"] = round(diff_score(ref_png, our_png), 2)
        side_by_side(ref_png, our_png, OUT / "sbs" / f"{i:03d}.jpg", rel.as_posix(), rec["diff"])
        rec["ok"] = True
    except subprocess.TimeoutExpired:
        rec["ok"] = False
        rec["skip"] = "our-render-timeout"
    except Exception as e:  # noqa: BLE001
        rec["ok"] = False
        rec["skip"] = f"{type(e).__name__}: {str(e)[:120]}"
    return rec


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sample", type=int, default=100)
    ap.add_argument("--seed", type=int, default=7)
    ap.add_argument("--scale", type=float, default=2.0)
    ap.add_argument("--attempts", type=int, default=400, help="最多尝试的候选数")
    ap.add_argument("--workers", type=int, default=4)
    ap.add_argument("--list-worst", type=int, default=0, help="只打印最差 N 份（不重跑）")
    ap.add_argument("--ref-worker", nargs=3, metavar=("PDF", "PNG", "SCALE"), help=argparse.SUPPRESS)
    args = ap.parse_args()
    if args.ref_worker:
        ref_render_worker(args.ref_worker[0], args.ref_worker[1], float(args.ref_worker[2]))
        return

    (OUT / "ref").mkdir(parents=True, exist_ok=True)
    (OUT / "our").mkdir(parents=True, exist_ok=True)
    (OUT / "sbs").mkdir(parents=True, exist_ok=True)

    if args.list_worst:
        recs = [json.loads(l) for l in (OUT / "results.jsonl").read_text().splitlines()]
        ok = sorted((r for r in recs if r.get("ok")), key=lambda r: -r["diff"])
        for r in ok[: args.list_worst]:
            print(f"{r['diff']:7.2f}  {r['file']}")
        return

    pool = sorted(CORPUS.rglob("*.pdf"))
    if not pool:
        sys.exit(f"语料为空: {CORPUS}")
    rng = random.Random(args.seed)
    rng.shuffle(pool)
    pool = pool[: args.attempts]

    # 断点续跑：已成功的样本跳过（按文件名对应）
    results_path = OUT / "results.jsonl"
    prev = {}
    if results_path.exists():
        for line in results_path.read_text().splitlines():
            r = json.loads(line)
            prev[r["file"]] = r

    print(f"语料 {len(pool)} 候选（seed={args.seed}），目标样本 {args.sample}，workers={args.workers}")
    pending = []
    for i, pdf in enumerate(pool):
        rel = pdf.relative_to(CORPUS).as_posix()
        if rel in prev and prev[rel].get("ok"):
            continue
        pending.append((pdf, args.scale, i))

    # 已完成样本的记录直接带入（保持原索引命名）
    recs: list[dict] = []
    for i, pdf in enumerate(pool):
        rel = pdf.relative_to(CORPUS).as_posix()
        if rel in prev:
            recs.append(prev[rel])
    ok = sum(1 for r in recs if r.get("ok"))
    print(f"续跑：已有记录 {len(recs)}（成功 {ok}），待处理 {len(pending)}")

    # 顺序执行：Chrome 子进程与 Python 多线程 fork 在 macOS 上偶发死锁，且单线程已够快
    for w in pending:
        if ok >= args.sample:
            break
        rec = attempt(w)
        recs.append(rec)
        if rec.get("ok"):
            ok += 1
        done = len([r for r in recs if r["file"] not in prev])
        if done % 10 == 0:
            print(f"  [本轮 {done}] 成功 {ok}，失败 {len(recs) - len(prev) - ok}")

    # 只保留已收到结果的记录
    recs = [r for r in recs if r.get("file")]
    results_path.write_text("\n".join(json.dumps(r, ensure_ascii=False) for r in recs))
    ok = [r for r in recs if r.get("ok")]
    skips = {}
    for r in recs:
        if not r.get("ok"):
            key = r.get("skip", "?").split(":")[0]
            skips[key] = skips.get(key, 0) + 1
    diffs = sorted(r["diff"] for r in ok)
    n = len(diffs)
    print(f"\n样本 {n} 份，跳过 {len(recs) - n}（{skips}）")
    if n:
        print(
            f"差异分: 中位 {diffs[n // 2]:.2f} / p75 {diffs[int(n * 0.75)]:.2f} / "
            f"p90 {diffs[int(n * 0.9)]:.2f} / max {diffs[-1]:.2f}"
        )
        print("最差 10 份：")
        for r in sorted(ok, key=lambda r: -r["diff"])[:10]:
            print(f"  {r['diff']:7.2f}  {r['file']}")


if __name__ == "__main__":
    main()
