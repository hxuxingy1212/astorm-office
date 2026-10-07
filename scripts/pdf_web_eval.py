#!/usr/bin/env python3
"""web 端（PdfViewer 组件）截图与参照渲染的抽样比对。

复用 pdf_render_eval 的参照（/tmp/pdf-render-eval/ref，与 CLI 同批同 DPI），
对 /tmp/pdf-web-eval/shots 逐份算差异分 + 出并排图（上=参照 下=web）。
"""
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from pdf_render_eval import CORPUS, diff_score, side_by_side  # noqa: E402

REF = Path("/tmp/pdf-render-eval/ref")
SHOTS = Path("/tmp/pdf-web-eval/shots")
OUT = Path("/tmp/pdf-web-eval/sbs")
OUT.mkdir(parents=True, exist_ok=True)

# web 样本 NNN = results.ok 列表序号；ref PNG 按语料池索引命名——用种子重放建立映射
ok_files = [
    r["file"]
    for r in map(json.loads, Path("/tmp/pdf-render-eval/results.jsonl").read_text().splitlines())
    if r.get("ok")
]
pool = sorted(CORPUS.rglob("*.pdf"))
rng = random.Random(7)
rng.shuffle(pool)
pool = pool[:400]
file2ref = {
    p.relative_to(CORPUS).as_posix(): f"{i:03d}" for i, p in enumerate(pool)
}
shot2ref = {
    f"{i:03d}": file2ref.get(f)
    for i, f in enumerate(ok_files)
}

recs = []
for shot in sorted(SHOTS.glob("*.png")):
    stem = shot.stem
    ref_name = shot2ref.get(stem)
    if not ref_name:
        continue
    ref = REF / f"{ref_name}.png"
    if not ref.exists():
        continue
    try:
        score = diff_score(ref, shot)
    except Exception as e:  # noqa: BLE001
        recs.append({"file": shot.stem, "diff": None, "skip": str(e)[:80]})
        continue
    side_by_side(ref, shot, OUT / f"{ref_name}.png", f"web {stem} = {Path(ok_files[int(stem)]).name}", score)
    recs.append({"file": stem, "ref": ref_name, "diff": round(score, 2)})

(Path("/tmp/pdf-web-eval") / "web_results.json").write_text(json.dumps(recs, indent=1))
ok = sorted((r for r in recs if r.get("diff") is not None), key=lambda r: -r["diff"])
vals = [r["diff"] for r in ok]
if vals:
    vals.sort()
    n = len(vals)
    print(f"web 样本 {n} 份：中位 {vals[n//2]:.2f} / p75 {vals[int(n*0.75)]:.2f} / p90 {vals[int(n*0.9)]:.2f} / max {vals[-1]:.2f}")
print("最差 10 份：")
for r in ok[:10]:
    print(f"  {r['diff']:7.2f}  {r['file']}.jpg")
skips = [r for r in recs if r.get("diff") is None]
if skips:
    print("跳过:", skips)
