#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""比较两张渲染 PNG，输出差异指标并生成并排/差异图。

用法：img_diff.py <orig.png> <rt.png> <out_prefix>
输出（stdout）：JSON {"mean":..,"pct":..,"w":..,"h":..}，mean∈[0,1] 为平均像素差。
"""
import json
import sys

import numpy as np
from PIL import Image, ImageChops


def load(path):
    im = Image.open(path).convert("RGB")
    return im


def main():
    a, b, prefix = sys.argv[1], sys.argv[2], sys.argv[3]
    ia, ib = load(a), load(b)
    if ib.size != ia.size:
        ib = ib.resize(ia.size)
    na = np.asarray(ia, dtype=np.int16)
    nb = np.asarray(ib, dtype=np.int16)
    d = np.abs(na - nb)
    mean = float(d.mean() / 255.0)
    # 差异像素占比：任一通道差 > 32
    pct = float((d.max(axis=2) > 32).mean())
    w, h = ia.size

    # 并排图
    side = Image.new("RGB", (w * 2 + 8, h), (255, 255, 255))
    side.paste(ia, (0, 0))
    side.paste(ib, (w + 8, 0))
    side.save(prefix + "_side.png")

    # 差异热力图（放大差异）
    diff = ImageChops.difference(ia, ib)
    diff = diff.point(lambda x: min(255, x * 3))
    diff.save(prefix + "_diff.png")

    print(json.dumps({"mean": round(mean, 5), "pct": round(pct, 5), "w": w, "h": h}))


if __name__ == "__main__":
    main()
