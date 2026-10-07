#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""从模板目录的 template.json 生成/补全 skeleton/（产物目录骨架）。

用法：
    python3 scripts/gen_template_skeleton.py skill/templates/<name> [...]
    python3 scripts/gen_template_skeleton.py --all        # 遍历 skill/templates/*

生成的 skeleton 与 `template extract` 的产物同构：workbook.json + 每个工作表的
xl/worksheets/sheetN.json，保留表头行的样式与列宽，数据区留空。
仅当 skeleton/ 不存在或 --force 时写入。
"""
import json
import os
import sys


def col_letter(n):
    s = ""
    while n > 0:
        n, r = divmod(n - 1, 26)
        s = chr(65 + r) + s
    return s


def default_font(tpl):
    if isinstance(tpl.get("default_font"), dict):
        return tpl["default_font"]
    for f in tpl.get("fonts", []):
        return {"name": f.get("name", "Calibri"), "size": f.get("size", 11.0),
                "color": f.get("color", "000000")}
    return {"name": "Calibri", "size": 11.0, "color": "000000"}


def skeleton(tpl):
    header = tpl.get("header") or {}
    row = int(header.get("row") or 1)
    values = header.get("values") or ["Column 1", "Column 2", "Column 3"]
    style = header.get("style")
    cols = tpl.get("columns") or []
    ncol = max(len(values), len(cols), 1)

    columns = []
    for i in range(ncol):
        columns.append({"width": (cols[i].get("width") if i < len(cols) else None) or 9.14})

    hdr_cells = []
    for i in range(ncol):
        c = {"ref": f"{col_letter(i + 1)}{row}"}
        v = values[i] if i < len(values) else None
        if v is not None:
            c["value"] = str(v)
            c["type"] = "string"
        if style:
            c["style"] = style
        hdr_cells.append(c)

    rows = [{"index": row, "cells": hdr_cells}]
    for r in range(row + 1, row + 4):
        rows.append({"index": r, "cells": [{} for _ in range(ncol)]})

    sheet = {
        "name": header.get("sheet") or tpl.get("name") or "Sheet1",
        "default_row_height": tpl.get("default_row_height") or 15.0,
        "columns": columns,
        "rows": rows,
    }
    wb = {
        "default_font": default_font(tpl),
        "active_tab": 0,
        "sheets": ["xl/worksheets/sheet1.json"],
    }
    return wb, sheet


def write_one(tdir, force):
    skel = os.path.join(tdir, "skeleton")
    src = os.path.join(tdir, "template.json")
    if not os.path.isfile(src):
        print(f"跳过（无 template.json）：{tdir}")
        return False
    if os.path.isdir(skel) and not force:
        print(f"跳过（已存在 skeleton）：{os.path.basename(tdir)}")
        return False
    tpl = json.load(open(src, encoding="utf-8"))
    wb, sheet = skeleton(tpl)
    ws = os.path.join(skel, "xl", "worksheets")
    os.makedirs(ws, exist_ok=True)
    with open(os.path.join(skel, "workbook.json"), "w", encoding="utf-8") as f:
        json.dump(wb, f, ensure_ascii=False, indent=2)
    with open(os.path.join(ws, "sheet1.json"), "w", encoding="utf-8") as f:
        json.dump(sheet, f, ensure_ascii=False, indent=2)
    print(f"已生成 skeleton：{os.path.basename(tdir)}")
    return True


def main():
    args = sys.argv[1:]
    force = "--force" in args
    args = [a for a in args if a != "--force"]
    dirs = []
    if not args or args == ["--all"]:
        base = "skill/templates"
        dirs = [os.path.join(base, d) for d in sorted(os.listdir(base))
                if os.path.isdir(os.path.join(base, d))]
    else:
        dirs = args
    n = sum(1 for d in dirs if write_one(d, force))
    print(f"完成：写入 {n} 个 skeleton")


if __name__ == "__main__":
    main()
