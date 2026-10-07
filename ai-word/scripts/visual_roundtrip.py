import os, sys, glob, shutil, struct, subprocess, tempfile

BIN = os.path.abspath("target/release/json2docx")
OUT = "/tmp/visrt"
QL = "qlmanage"


def render(docx, out_png):
    od = tempfile.mkdtemp()
    try:
        p = subprocess.Popen([QL, "-t", "-s", "1000", "-o", od, docx],
                             stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            p.wait(timeout=12)
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


def bmp_pixels(png):
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


def diff_ratio(a, b):
    pa = bmp_pixels(a)
    pb = bmp_pixels(b)
    if not pa or not pb:
        return None
    if pa[:2] != pb[:2]:
        (w1, h1), (w2, h2) = pa[:2], pb[:2]
        # 容忍 ±2px 的缩略图差异，否则视为版面尺寸不同
        if abs(w1 - w2) <= 2 and abs(h1 - h2) <= 2:
            pass
        else:
            return -1.0
    n = min(len(pa[2]), len(pb[2]))
    if n == 0:
        return None
    step = max(1, n // 400000)
    c = sum(1 for i in range(0, n, step) if pa[2][i] != pb[2][i])
    return c / len(range(0, n, step))


def main():
    # 用法：visual_roundtrip.py [docx ...]；无参则用 /tmp/docx_dataset 下抽样
    files = sys.argv[1:]
    if not files:
        import glob as _g
        files = sorted(_g.glob("/tmp/docx_dataset/*.docx"))[:40]
    os.makedirs(OUT, exist_ok=True)
    results = []
    for f in files:
        name = os.path.basename(f).rsplit(".", 1)[0][:40]
        d = os.path.join(OUT, name)
        os.makedirs(d, exist_ok=True)
        subprocess.run(["rm", "-rf", "/tmp/vr_prod", "/tmp/vr_rt.docx"])
        u = subprocess.run([BIN, "unpack", f, "-o", "/tmp/vr_prod"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if u.returncode != 0:
            results.append((name, "UNPACK-ERR", None))
            continue
        r = subprocess.run([BIN, "repack", "/tmp/vr_prod", "-o", "/tmp/vr_rt.docx"],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if r.returncode != 0:
            results.append((name, "REPACK-ERR", None))
            continue
        ok1 = render(f, os.path.join(d, "orig.png"))
        ok2 = render("/tmp/vr_rt.docx", os.path.join(d, "rt.png"))
        if not (ok1 and ok2):
            results.append((name, "RENDER-FAIL", None))
            continue
        ratio = diff_ratio(os.path.join(d, "orig.png"), os.path.join(d, "rt.png"))
        results.append((name, "ok", ratio))
    print(f"{'name':40} {'status':10} diff")
    for name, st, ratio in sorted(results, key=lambda x: (x[1] != "ok", x[2] if isinstance(x[2], float) else 9)):
        print(f"{name:40} {st:10} {ratio}")


if __name__ == "__main__":
    main()
