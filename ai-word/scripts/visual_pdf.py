#!/usr/bin/env python3
"""基于 LibreOffice(PDF) 的回环视觉对比：
- 原 docx 与回环 docx 各转 PDF，比较页数、页面尺寸(pt) 与首页缩略图像素差。
- 比 qlmanage 直接渲染 docx 更可靠（可正确处理 ISO strict 文档）。

用法: scripts/visual_pdf.py [docx ...]   # 无参则抽样 /tmp/docx_dataset
"""
import os, sys, glob, re, subprocess, tempfile, shutil, struct, zipfile

BIN = os.path.abspath("target/release/json2docx")
WORK = "/tmp/vispdf"
PROFILE = "file:///tmp/vispdf_profile"
QL = "qlmanage"


def soffice_pdf(docx, outdir):
    os.makedirs(outdir, exist_ok=True)
    try:
        subprocess.run(
            ["soffice", f"-env:UserInstallation={PROFILE}", "--headless",
             "--convert-to", "pdf", "--outdir", outdir, docx],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=90,
        )
    except subprocess.TimeoutExpired:
        subprocess.run(["pkill", "-f", "soffice"], stdout=subprocess.DEVNULL)
    stem = os.path.splitext(os.path.basename(docx))[0]
    p = os.path.join(outdir, stem + ".pdf")
    return p if os.path.exists(p) else None


def pdf_info(pdf):
    d = open(pdf, "rb").read()
    mb = re.findall(rb"/MediaBox\s*\[([^\]]*)\]", d)
    size = None
    if mb:
        nums = mb[0].split()
        try:
            size = (float(nums[2]), float(nums[3]))
        except Exception:
            size = None
    # 页数：优先 mdls，其次 /Count
    pages = None
    try:
        out = subprocess.run(["mdls", "-name", "kMDItemNumberOfPages", pdf],
                             capture_output=True, text=True).stdout
        m = re.search(r"= (\d+)", out)
        if m:
            pages = int(m.group(1))
    except Exception:
        pass
    if pages is None:
        m = re.findall(rb"/Count\s+(\d+)", d)
        if m:
            pages = max(int(x) for x in m)
    return pages, size


def app_pages(docx):
    """读取 docProps/app.xml 中 Word 记录的页数（原始文档的真实页数）"""
    try:
        with zipfile.ZipFile(docx) as z:
            x = z.read("docProps/app.xml").decode("utf-8", "replace")
        m = re.search(r"<Pages>(\d+)</Pages>", x)
        return int(m.group(1)) if m else None
    except Exception:
        return None


def pdf_thumb(pdf, out_png):
    od = tempfile.mkdtemp()
    try:
        p = subprocess.Popen([QL, "-t", "-s", "1000", "-o", od, pdf],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            p.wait(timeout=20)
        except subprocess.TimeoutExpired:
            p.kill()
        png = None
        for r, _, fs in os.walk(od):
            for f in fs:
                if f.endswith(".png"):
                    png = os.path.join(r, f)
        if png and os.path.getsize(png) > 0:
            shutil.copy(png, out_png)
            return True
    finally:
        shutil.rmtree(od, ignore_errors=True)
    return False


def bmp(png):
    b = png + ".bmp"
    subprocess.run(["sips", "-s", "format", "bmp", png, "--out", b],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    if not os.path.exists(b):
        return None
    d = open(b, "rb").read()
    if d[:2] != b"BM":
        return None
    off = struct.unpack_from("<I", d, 10)[0]
    w = struct.unpack_from("<i", d, 18)[0]
    h = struct.unpack_from("<i", d, 22)[0]
    return w, h, d[off:]


def diff(a, b):
    pa, pb = bmp(a), bmp(b)
    if not pa or not pb:
        return None
    if pa[:2] != pb[:2]:
        return -1.0
    n = min(len(pa[2]), len(pb[2]))
    step = max(1, n // 400000)
    idx = range(0, n, step)
    c = sum(1 for i in idx if pa[2][i] != pb[2][i])
    return c / max(1, len(idx))


def main():
    files = sys.argv[1:]
    if not files:
        files = sorted(glob.glob("/tmp/docx_dataset/*.docx"))[:40]
    shutil.rmtree(WORK, ignore_errors=True)
    os.makedirs(WORK, exist_ok=True)
    rows = []
    for f in files:
        name = os.path.basename(f)[:40]
        d = os.path.join(WORK, re.sub(r"[^\w.-]", "_", name))
        os.makedirs(d, exist_ok=True)
        # 回环
        subprocess.run(["rm", "-rf", "/tmp/vp_prod", "/tmp/vp_rt.docx"])
        u = subprocess.run([BIN, "unpack", f, "-o", "/tmp/vp_prod"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if u.returncode != 0:
            rows.append((name, "UNPACK-ERR", None, None, None, None))
            continue
        r = subprocess.run([BIN, "repack", "/tmp/vp_prod", "-o", "/tmp/vp_rt.docx"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if r.returncode != 0:
            rows.append((name, "REPACK-ERR", None, None, None, None))
            continue
        po = soffice_pdf(f, os.path.join(d, "o"))
        pr = soffice_pdf("/tmp/vp_rt.docx", os.path.join(d, "r"))
        if not po or not pr:
            rows.append((name, "PDF-FAIL", None, None, None, None))
            continue
        pno, szo = pdf_info(po)
        pnr, szr = pdf_info(pr)
        ap = app_pages(f)
        ratio = None
        if pdf_thumb(po, os.path.join(d, "o.png")) and pdf_thumb(pr, os.path.join(d, "r.png")):
            ratio = diff(os.path.join(d, "o.png"), os.path.join(d, "r.png"))
        status = "ok"
        # 同一渲染器（LibreOffice）下原/回环一致，或与 Word 记录的页数一致，均视为通过
        match_soffice = pno is not None and pno == pnr
        match_word = ap is not None and pnr is not None and ap == pnr
        if not (match_soffice or match_word):
            status = f"PAGES soff{pno}/word{ap}!=rt{pnr}"
        if status == "ok" and szo and szr and (abs(szo[0] - szr[0]) > 2 or abs(szo[1] - szr[1]) > 2):
            status = f"SIZE {szo}!={szr}"
        rows.append((name, status, ratio, szo, szr, (ap, pno, pnr)))
    print(f"{'name':40} {'status':20} {'diff':10} size(orig->rt)")
    for name, st, ratio, szo, szr, pg in sorted(
        rows, key=lambda x: (x[1] != "ok", x[2] if isinstance(x[2], float) else 9)
    ):
        rt = "" if ratio is None else f"{ratio:.4f}"
        sz = "" if not szo else f"{szo[0]:.0f}x{szo[1]:.0f}->{szr[0]:.0f}x{szr[1]:.0f}"
        print(f"{name:40} {st:20} {rt:10} {sz}")
    ok = [x for x in rows if x[1] == "ok"]
    good = [x for x in ok if isinstance(x[2], float) and x[2] <= 0.02]
    print(f"\n总计 {len(rows)}：ok {len(ok)}，其中 diff<=0.02 {len(good)}；"
          f"页数/尺寸/失败异常 {len(rows)-len(ok)}")


if __name__ == "__main__":
    main()
