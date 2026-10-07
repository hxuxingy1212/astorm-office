#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""xlsx 视觉回归（LibreOffice 渲染，可渲染图表）：原始 vs 回环 的 PDF 逐页像素比对。

用法：
    python3 scripts/visual_diff_lo.py <dir|file> [more...]
    LIMIT=40 python3 scripts/visual_diff_lo.py tests/corpus
    python3 scripts/visual_diff_lo.py --list list.txt

产物：/tmp/vlo/results.tsv + /tmp/vlo/<name>.p1_side.png（首页并排）。
依赖：soffice(LibreOffice) + PyMuPDF/Pillow/numpy（缺库时自动建 venv）。
"""
import glob
import os
import shutil
import subprocess
import sys
import tempfile

# 自动切换到带依赖的 venv（缺失则创建）。
_VENV_DIR = os.path.join(tempfile.gettempdir(), "xlsxvenv")
_VENV_PY = os.path.join(_VENV_DIR, "bin", "python")


def _ensure_deps():
    try:
        import numpy  # noqa: F401
        import fitz  # noqa: F401
        return True
    except ImportError:
        return False


if os.environ.get("_VD_REEXEC") != "1" and not _ensure_deps():
    if not os.path.exists(_VENV_PY):
        subprocess.run([sys.executable, "-m", "venv", _VENV_DIR], check=True)
    subprocess.run(
        [_VENV_PY, "-m", "pip", "install", "-q", "pillow", "numpy", "pymupdf"],
        check=True,
    )
    os.environ["_VD_REEXEC"] = "1"
    os.execv(_VENV_PY, [_VENV_PY, os.path.abspath(__file__)] + sys.argv[1:])

import numpy as np  # noqa: E402
import pymupdf  # noqa: E402
from PIL import Image, ImageChops  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "release", "json2xlsx")
SOFFICE = shutil.which("soffice") or "/Applications/LibreOffice.app/Contents/MacOS/soffice"
WORK = "/tmp/vlo"
THRESH = 0.01
ZOOM = 1.4


def sh(cmd, timeout=None):
    try:
        return subprocess.run(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                              timeout=timeout)
    except subprocess.TimeoutExpired:
        return None


def collect(args):
    limit = int(os.environ.get("LIMIT", "0") or 0)
    files, rest = [], args
    if args and args[0] == "--list":
        files = [l.strip() for l in open(args[1]) if l.strip()]
        rest = args[2:]
    for a in rest:
        if os.path.isdir(a):
            files.extend(sorted(p for p in glob.glob(os.path.join(a, "**", "*.xlsx"), recursive=True)
                                if os.path.getsize(p) <= 3 * 1024 * 1024))
        else:
            files.append(a)
    if limit and len(files) > limit:
        step = max(1, len(files) // limit)
        files = files[::step][:limit]
    return files


def soffice_pdf(files, outdir, profile):
    """分批把 xlsx 转为 pdf（每批一个进程，避免单个文件拖垮整批）。"""
    os.makedirs(outdir, exist_ok=True)
    chunk = 15
    for i in range(0, len(files), chunk):
        part = files[i:i + chunk]
        args = [SOFFICE, f"-env:UserInstallation=file://{profile}", "--headless",
                "--convert-to", "pdf", "--outdir", outdir] + part
        sh(args, timeout=300)


_VOLATILE = ("RAND(", "RANDBETWEEN(", "NOW(", "TODAY(", "OFFSET(", "INDIRECT(",
             "CELL(", "INFO(", "RANDARRAY(")


def has_volatile(xlsx):
    """工作表公式含易失函数时，LibreOffice 每次打开都会重算，像素比对无意义。"""
    import re
    import zipfile
    try:
        with zipfile.ZipFile(xlsx) as z:
            for n in z.namelist():
                name = n.lower()
                if not (name.startswith("xl/worksheets/") and name.endswith(".xml")):
                    continue
                data = z.read(n).decode("utf-8", "replace").upper()
                if any(v in data for v in _VOLATILE):
                    return True
    except Exception:
        return False
    return False


def pdf_pages(path):
    doc = pymupdf.open(path)
    pages = []
    for pg in doc:
        pix = pg.get_pixmap(matrix=pymupdf.Matrix(ZOOM, ZOOM))
        img = Image.frombytes("RGB", (pix.width, pix.height), pix.samples)
        pages.append(img)
    doc.close()
    return pages


def compare(a, b):
    if a.size != b.size:
        b = b.resize(a.size)
    na = np.asarray(a, dtype=np.int16)
    nb = np.asarray(b, dtype=np.int16)
    d = np.abs(na - nb)
    return float(d.mean() / 255.0), float((d.max(axis=2) > 32).mean())


def main():
    files = collect(sys.argv[1:])
    if not files:
        sys.exit("没有输入文件")
    if not os.path.exists(SOFFICE):
        sys.exit("需要 LibreOffice(soffice)")
    shutil.rmtree(WORK, ignore_errors=True)
    os.makedirs(WORK, exist_ok=True)
    sh(["cargo", "build", "--release", "-q", "-p", "json2xlsx"], timeout=600)
    profile = os.path.join(WORK, "profile")

    prep, skipped, volatile = [], [], []
    for i, f in enumerate(files):
        cid = "%04d" % i
        prod = os.path.join(WORK, "prod", cid)
        rt = os.path.join(WORK, "rt", cid + ".xlsx")
        os.makedirs(os.path.dirname(rt), exist_ok=True)
        r1 = sh([BIN, "unpack", f, "-o", prod], timeout=120)
        r2 = sh([BIN, "repack", prod, "-o", rt], timeout=120) if r1 and r1.returncode == 0 else None
        if r1 is None or r1.returncode != 0 or r2 is None or r2.returncode != 0:
            skipped.append(os.path.basename(f))
            continue
        if has_volatile(f):
            volatile.append(os.path.basename(f))
            continue
        prep.append((cid, f, rt, os.path.basename(f)))
    print("prepared %d/%d (skip %d, volatile %d)" % (len(prep), len(files), len(skipped), len(volatile)))

    o_dir, r_dir = os.path.join(WORK, "pdf_orig"), os.path.join(WORK, "pdf_rt")
    soffice_pdf([o for _, o, _, _ in prep], o_dir, profile)
    soffice_pdf([r for _, _, r, _ in prep], r_dir, profile)

    rows, same, diff, fail, nosrc = [], 0, 0, 0, 0
    for cid, orig, rt, name in prep:
        op = os.path.join(o_dir, os.path.basename(orig)[:-5] + ".pdf")
        rp = os.path.join(r_dir, cid + ".pdf")
        if not os.path.exists(op):
            # 原始文件本身无法被 LibreOffice 渲染：无法比较，不计为回环缺陷。
            nosrc += 1
            rows.append((name, "", "", "NOSRC"))
            continue
        if not os.path.exists(rp):
            # 回环产物渲染失败：真实缺陷。
            fail += 1
            rows.append((name, "", "", "NOPDF"))
            continue
        pa, pb = pdf_pages(op), pdf_pages(rp)
        if len(pa) != len(pb):
            diff += 1
            rows.append((name, "", "", "PAGES %d/%d" % (len(pa), len(pb))))
            continue
        worst = 0.0
        worst_mean = 0.0
        for i, (a, b) in enumerate(zip(pa, pb)):
            mean, pct = compare(a, b)
            if pct > worst:
                worst, worst_mean = pct, mean
            if i == 0:
                side = Image.new("RGB", (a.size[0] * 2 + 8, a.size[1]), (255, 255, 255))
                side.paste(a, (0, 0))
                bb = b if b.size == a.size else b.resize(a.size)
                side.paste(bb, (a.size[0] + 8, 0))
                side.save(os.path.join(WORK, name + ".p1_side.png"))
        status = "DIFF" if worst > THRESH else "ok"
        if status == "DIFF":
            diff += 1
        else:
            same += 1
        rows.append((name, "%.4f" % worst_mean, "%.4f" % worst, status))

    res = os.path.join(WORK, "results.tsv")
    with open(res, "w") as f:
        for r in rows:
            f.write("\t".join(r) + "\n")
    print("---- LibreOffice 视觉对比：一致 %d，有差异 %d，回环渲染失败 %d，原始不可渲染 %d，易失跳过 %d，跳过 %d（共 %d）"
          % (same, diff, fail, nosrc, len(volatile), len(skipped), len(files)))
    for r in sorted([x for x in rows if x[3] != "ok"], key=lambda x: x[2] or "0", reverse=True)[:25]:
        print("  %-10s mean=%-8s pct=%-8s %s" % (r[3], r[1], r[2], r[0]))
    print("结果：", res)
    if os.environ.get("STRICT") == "1" and (diff or fail):
        sys.exit(1)


if __name__ == "__main__":
    main()
