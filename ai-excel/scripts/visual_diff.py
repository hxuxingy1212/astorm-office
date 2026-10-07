#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""xlsx 视觉回归：原始 vs unpack→repack 回环，QuickLook(qlmanage) 渲染 + 像素比较。

用法：
    python3 scripts/visual_diff.py <dir|file> [more...]      # 目录默认抽样
    LIMIT=60 python3 scripts/visual_diff.py /tmp/xlsx_more
    python3 scripts/visual_diff.py --list list.txt
    python3 scripts/visual_diff.py --list list.txt --limit 40

产物：/tmp/vcmp/results.tsv（name, mean, pct, status）+ 差异并排图。
环境：需要 macOS QuickLook（qlmanage）与 Pillow/numpy。
"""
import glob
import json
import os
import shutil
import subprocess
import sys
import tempfile

# 需要一个带 Pillow/numpy 的 Python；缺失时自动（必要时创建）项目 venv。
_VENV_DIR = os.path.join(tempfile.gettempdir(), "xlsxvenv")
_VENV_PY = os.path.join(_VENV_DIR, "bin", "python")
if os.environ.get("_VD_REEXEC") != "1":
    try:
        import numpy  # noqa: F401
    except ImportError:
        if not os.path.exists(_VENV_PY):
            subprocess.run([sys.executable, "-m", "venv", _VENV_DIR], check=True)
        subprocess.run(
            [_VENV_PY, "-m", "pip", "install", "-q", "pillow", "numpy"], check=True
        )
        os.environ["_VD_REEXEC"] = "1"
        os.execv(_VENV_PY, [_VENV_PY, os.path.abspath(__file__)] + sys.argv[1:])

import numpy as np
from PIL import Image, ImageChops

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "release", "json2xlsx")
WORK = "/tmp/vcmp"
RENDER_SIZE = "1500"
QL_TIMEOUT = 240
CHUNK = 20
THRESH = 0.01  # pct 阈值

os.environ.setdefault("LIMIT", "0")


def sh(cmd, timeout=None):
    try:
        return subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                              timeout=timeout)
    except subprocess.TimeoutExpired:
        return None


def collect(args):
    limit = int(os.environ.get("LIMIT", "0") or 0)
    files = []
    if args and args[0] == "--list":
        with open(args[1]) as f:
            files = [l.strip() for l in f if l.strip()]
        rest = args[2:]
    else:
        rest = args
    for a in rest:
        if os.path.isdir(a):
            found = [p for p in glob.glob(os.path.join(a, "**", "*.xlsx"), recursive=True)
                     if os.path.getsize(p) <= 2 * 1024 * 1024]
            files.extend(sorted(found))
        else:
            files.append(a)
    if limit and len(files) > limit:
        step = max(1, len(files) // limit)
        files = files[::step][:limit]
    return files


def ql_render(pairs, outdir):
    """pairs: list of (id, path). 批量渲染到 outdir，返回 {id: png}。"""
    os.makedirs(outdir, exist_ok=True)
    result = {}
    for i in range(0, len(pairs), CHUNK):
        chunk = pairs[i:i + CHUNK]
        # 拷贝为统一名字，避免同名冲突与 QuickLook 缓存
        stage = tempfile.mkdtemp()
        for cid, path in chunk:
            shutil.copy(path, os.path.join(stage, cid + ".xlsx"))
        inputs = sorted(glob.glob(os.path.join(stage, "*.xlsx")))
        p = subprocess.Popen(["qlmanage", "-t", "-s", RENDER_SIZE, "-o", outdir] + inputs,
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            p.wait(timeout=QL_TIMEOUT)
        except subprocess.TimeoutExpired:
            p.kill()
        for cid, _ in chunk:
            png = os.path.join(outdir, cid + ".xlsx.png")
            if os.path.exists(png) and os.path.getsize(png) > 0:
                result[cid] = png
        shutil.rmtree(stage, ignore_errors=True)
    return result


def compare(a, b, prefix):
    ia = Image.open(a).convert("RGB")
    ib = Image.open(b).convert("RGB")
    if ib.size != ia.size:
        ib = ib.resize(ia.size)
    na = np.asarray(ia, dtype=np.int16)
    nb = np.asarray(ib, dtype=np.int16)
    d = np.abs(na - nb)
    mean = float(d.mean() / 255.0)
    pct = float((d.max(axis=2) > 32).mean())
    w, h = ia.size
    side = Image.new("RGB", (w * 2 + 8, h), (255, 255, 255))
    side.paste(ia, (0, 0))
    side.paste(ib, (w + 8, 0))
    side.save(prefix + "_side.png")
    ImageChops.difference(ia, ib).point(lambda x: min(255, x * 3)).save(prefix + "_diff.png")
    return mean, pct


def main():
    args = sys.argv[1:]
    files = collect(args)
    if not files:
        sys.exit("没有输入文件")
    shutil.rmtree(WORK, ignore_errors=True)
    os.makedirs(WORK, exist_ok=True)
    sh(["cargo", "build", "--release", "-q", "-p", "json2xlsx"], timeout=600)

    # 1) 预处理：unpack -> repack，准备 orig / rt 输入
    prep = []          # (id, orig_path, rt_path, name)
    skipped = []
    for i, f in enumerate(files):
        cid = "%04d" % i
        prod = os.path.join(WORK, "prod", cid)
        rt = os.path.join(WORK, "rt", cid + ".xlsx")
        os.makedirs(os.path.dirname(rt), exist_ok=True)
        r1 = sh([BIN, "unpack", f, "-o", prod], timeout=120)
        if r1 is None or r1.returncode != 0:
            skipped.append(("unpack", os.path.basename(f)))
            continue
        r2 = sh([BIN, "repack", prod, "-o", rt], timeout=120)
        if r2 is None or r2.returncode != 0:
            skipped.append(("repack", os.path.basename(f)))
            continue
        prep.append((cid, f, rt, os.path.basename(f)))
    print("prepared %d/%d（skip %d）" % (len(prep), len(files), len(skipped)))

    # 2) 批量渲染
    out_o = os.path.join(WORK, "png_orig")
    out_r = os.path.join(WORK, "png_rt")
    orig_png = ql_render([(c, o) for c, o, _, _ in prep], out_o)
    rt_png = ql_render([(c, r) for c, _, r, _ in prep], out_r)

    # 3) 比较
    results = os.path.join(WORK, "results.tsv")
    rows = []
    same = diff = fail = 0
    for cid, orig, rt, name in prep:
        if cid not in orig_png or cid not in rt_png:
            fail += 1
            rows.append((name, "", "", "NORENDER"))
            continue
        mean, pct = compare(orig_png[cid], rt_png[cid], os.path.join(WORK, name))
        status = "DIFF" if pct > THRESH else "ok"
        if status == "DIFF":
            diff += 1
        else:
            same += 1
        rows.append((name, "%.4f" % mean, "%.4f" % pct, status))
    with open(results, "w") as f:
        for r in rows:
            f.write("\t".join(r) + "\n")

    print("---- 视觉对比：一致 %d，有差异 %d，未渲染 %d，跳过 %d（共 %d）"
          % (same, diff, fail, len(skipped), len(files)))
    print("按差异度排序（top 20）：")
    for r in sorted([x for x in rows if x[2]], key=lambda x: float(x[2]), reverse=True)[:20]:
        print("  %-8s mean=%-8s pct=%-8s %s" % (r[3], r[1], r[2], r[0]))
    print("结果：", results)


if __name__ == "__main__":
    main()
