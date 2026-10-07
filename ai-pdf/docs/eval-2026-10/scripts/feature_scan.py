#!/usr/bin/env python3
"""扫描 PDF 使用的特性，与 round-trip 视觉差异关联。"""
import json, re, sys, glob
from pathlib import Path
import pymupdf

CORPUS = Path('/tmp/pdf-eval/corpus')

FEATS = {
    'icc_sc_color':  r'/Cs\d*\s+cs|/ColorSpace.*\bcs\b|(?<![\w/])(?:\d+\.?\d*\s+){1,4}scn?\b',
    'pattern_fill':  r'/Pattern\s+cs|/P\d+\s+scn',
    'extgstate_alpha': r'/GS\d*\s+gs',
    'form_xobject':  None,   # 单独检测 Subtype /Form
    'shading':       r'\bsh\b',
    'clip_re':       r'\bre\s+W\*?\s+n\b',
    'inline_image':  r'\bBI\b',
    'jbig2_ccitt':   None,
}

def scan(path: Path):
    out = {k: 0 for k in FEATS}
    out['annots'] = 0
    out['rotated_text'] = 0
    out['pages'] = 0
    try:
        doc = pymupdf.open(path)
    except Exception:
        return None
    try:
        try:
            n_pages = min(doc.page_count, 3)
        except Exception:
            return out
        out['pages'] = n_pages
        for pno in range(n_pages):
            try:
                pg = doc[pno]
                obj = doc.xref_object(pg.xref)
            except Exception:
                continue
            # 注释
            if '/Annots' in obj:
                out['annots'] += 1
            texts = []
            for x in pg.get_contents():
                try:
                    t = doc.xref_stream(x).decode('latin-1', 'replace')
                except Exception:
                    continue
                texts.append(t)
                for k, rx in FEATS.items():
                    if rx and re.search(rx, t):
                        out[k] += 1
                # 旋转文本：Tm 里 b/c 非零
                for m in re.finditer(r'([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+Tm', t):
                    b, c = float(m.group(2)), float(m.group(3))
                    if abs(b) > 0.01 or abs(c) > 0.01:
                        out['rotated_text'] += 1
                        break
                # Form XObject 与图像滤镜
                for name in set(re.findall(r'/([A-Za-z0-9_.+-]+)\s+Do', t)):
                    try:
                        v = doc.xref_get_key(pg.xref, f'Resources/XObject/{name}')
                    except Exception:
                        continue
                    if v and v[0] == 'xref':
                        sx = int(v[1].split()[0])
                        try:
                            so = doc.xref_object(sx)
                        except Exception:
                            continue
                        if '/Subtype /Form' in so or '/Subtype/Form' in so:
                            out['form_xobject'] += 1
                        if '/Image' in so:
                            if re.search(r'JBIG2Decode|CCITTFaxDecode', so):
                                out['jbig2_ccitt'] += 1
                # 页面资源里的图像滤镜（即使未绘制）
                try:
                    xo = doc.xref_get_key(pg.xref, 'Resources/XObject')
                except Exception:
                    xo = None
            # 全页图像滤镜扫描（资源级）
            try:
                for img in pg.get_images(full=True):
                    xref = img[0]
                    so = doc.xref_object(xref)
                    if re.search(r'JBIG2Decode|CCITTFaxDecode', so):
                        out['jbig2_ccitt'] += 1
            except Exception:
                pass
    finally:
        try: doc.close()
        except Exception: pass
    return out

def main():
    mode = sys.argv[1] if len(sys.argv) > 1 else 'samples'
    if mode == 'samples':
        entries = []
        for f in sorted(glob.glob('/tmp/pdf-eval/samples/*.json')):
            d = json.load(open(f))
            entries.append(d)
        rows = []
        for d in entries:
            feat = scan(CORPUS / d['file'])
            if feat is None: continue
            rows.append({**d, **{f'f_{k}': v for k, v in feat.items()}})
        json.dump(rows, open('/tmp/pdf-eval/feature_samples.json', 'w'), ensure_ascii=False, indent=1)
        # 关联：有该特性的平均 diff vs 无
        print(f"{'特性':<20} {'有特性份数':>8} {'均diff':>8} {'无特性均diff':>10}")
        for k in list(FEATS) + ['annots', 'rotated_text']:
            key = f'f_{k}'
            with_ = [r['diff_mean'] for r in rows if r.get(key, 0) > 0 and r['diff_mean'] is not None]
            without = [r['diff_mean'] for r in rows if r.get(key, 0) == 0 and r['diff_mean'] is not None]
            if not with_:
                print(f"{k:<20} {0:>8}")
                continue
            import statistics
            print(f"{k:<20} {len(with_):>8} {statistics.mean(with_):>8.2f} {statistics.mean(without) if without else 0:>10.2f}")
    else:
        # 全语料统计（前3页）
        recs = [json.loads(l) for l in open('/tmp/pdf-eval/results.jsonl')]
        ok = [r for r in recs if r['unpack'] == 'ok']
        agg = {k: 0 for k in list(FEATS) + ['annots', 'rotated_text']}
        n = 0
        for r in ok:
            feat = scan(CORPUS / r['file'])
            if feat is None: continue
            n += 1
            for k, v in feat.items():
                if k in agg and v > 0:
                    agg[k] += 1
        print(f"全语料成功样例 {n} 份（仅前3页）:")
        for k, v in sorted(agg.items(), key=lambda x: -x[1]):
            print(f"  {k:<20} {v:>5} 份 ({v/max(n,1)*100:.1f}%)")

if __name__ == '__main__':
    main()
