#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""用真实世界数据集生成场景报表（.xlsx）。

数据来源：`scripts/collect_datasets.sh` 拉到 `/tmp/datasets_raw`。
产物：`examples/out/reports/ds_*.xlsx`（以及同名产物目录），用 CLI `repack` 打包。

用法：python3 scripts/gen_dataset_reports.py [数据目录]
"""
import csv
import json
import os
import subprocess
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(ROOT, "target", "debug", "json2xlsx")
DATA = sys.argv[1] if len(sys.argv) > 1 else "/tmp/datasets_raw"
OUT = os.path.join(ROOT, "examples", "out", "reports")
os.makedirs(OUT, exist_ok=True)

ZH = {"name": "微软雅黑", "size": 11}


def style(fill, color="FFFFFF", bold=True, align="center"):
    return {
        "font": {"bold": bold, "color": color, "name": "微软雅黑"},
        "fill": fill,
        "alignment": {"horizontal": align, "vertical": "center"},
    }


HEADERS = {
    "blue": style("4472C4"),
    "green": style("548235"),
    "orange": style("ED7D31"),
    "cyan": style("2E9CA6"),
    "gray": style("404040"),
    "red": style("C00000"),
}


def title_cell(text, span):
    return {
        "ref": "A1",
        "value": text,
        "style": {
            "font": {"bold": True, "size": 14, "name": "微软雅黑"},
            "alignment": {"horizontal": "center"},
        },
    }


def header_row(cells, hstyle):
    return {"index": 2, "cells": [{"ref": r, "value": v, "style": hstyle} for r, v in cells]}


def num_cell(ref, value, fmt=None, align=None):
    c = {"ref": ref, "value": value}
    if fmt:
        c["number_format"] = fmt
    if align:
        c["style"] = {"alignment": {"horizontal": align}}
    return c


def write_product(name, sheet, header_style):
    d = os.path.join(OUT, name)
    os.makedirs(os.path.join(d, "xl", "worksheets"), exist_ok=True)
    wb = {
        "meta": {"title": sheet["name"], "author": "ai-excel"},
        "default_font": ZH,
        "styles": {header_style: HEADERS[header_style]},
        "sheets": ["xl/worksheets/sheet1.json"],
    }
    with open(os.path.join(d, "workbook.json"), "w") as f:
        json.dump(wb, f, ensure_ascii=False)
    with open(os.path.join(d, "xl", "worksheets", "sheet1.json"), "w") as f:
        json.dump(sheet, f, ensure_ascii=False)
    subprocess.run([BIN, "repack", d, "-o", os.path.join(OUT, name + ".xlsx")],
                   check=True, stdout=subprocess.DEVNULL)
    print("  built  examples/out/reports/%s.xlsx" % name)


def read_csv(fn):
    with open(os.path.join(DATA, fn), newline="", encoding="utf-8", errors="replace") as f:
        return list(csv.DictReader(f))


def read_json(fn):
    with open(os.path.join(DATA, fn), encoding="utf-8", errors="replace") as f:
        return json.load(f)


# ---------------------------------------------------------------- 1 航班趋势
def flights():
    rows = read_csv("flights.csv")
    months = ["January", "February", "March", "April", "May", "June", "July",
              "August", "September", "October", "November", "December"]
    mnum = {m: i + 1 for i, m in enumerate(months)}
    data = defaultdict(lambda: defaultdict(int))
    for r in rows:
        data[int(r["year"])][mnum[r["month"]]] = int(r["passengers"])
    years = sorted(data)
    cols = [{"width": 8}] + [{"width": 9}] * 13
    sheet = {
        "name": "航空客运量",
        "freeze": "B3",
        "columns": cols,
        "merges": ["A1:N1"],
        "rows": [{"index": 1, "cells": [title_cell("航空公司月度乘客数（1949–1960，千人）", "N")]}],
        "charts": [{
            "type": "line", "data_range": "N3:N%d" % (2 + len(years)),
            "categories": "A3:A%d" % (2 + len(years)), "title": "年度总乘客数",
            "legend": "none", "colors": ["4472C4"], "anchor": "P2", "size": {"w": 9, "h": 6},
        }],
    }
    hdr = [("A2", "年份")] + [("%s2" % chr(64 + m), "%d月" % m) for m in range(1, 13)] + [("N2", "合计")]
    sheet["rows"].append(header_row(hdr, "blue"))
    for i, y in enumerate(years):
        ri = 3 + i
        cells = [{"ref": "A%d" % ri, "value": y, "style": {"alignment": {"horizontal": "center"}}}]
        for m in range(1, 13):
            cells.append(num_cell("%s%d" % (chr(64 + m), ri), data[y].get(m, 0), "#,##0"))
        cells.append(num_cell("N%d" % ri, sum(data[y].values()), "#,##0"))
        sheet["rows"].append({"index": ri, "cells": cells})
    write_product("ds_flights_trend", sheet, "blue")


# ---------------------------------------------------------------- 2 汽车马力
def cars():
    rows = read_json("cars.json")
    rows = [r for r in rows if r.get("Horsepower")]
    rows.sort(key=lambda r: r["Horsepower"], reverse=True)
    rows = rows[:15]
    cols = [{"width": 26}] + [{"width": 12}] * 5
    sheet = {
        "name": "汽车性能",
        "freeze": "A3",
        "columns": cols,
        "merges": ["A1:F1"],
        "rows": [{"index": 1, "cells": [title_cell("汽车马力 Top 15（1970–1982）", "F")]}],
        "charts": [{
            "type": "bar", "data_range": "B3:B%d" % (2 + len(rows)),
            "categories": "A3:A%d" % (2 + len(rows)), "title": "马力",
            "legend": "none", "colors": ["ED7D31"], "anchor": "H2", "size": {"w": 10, "h": 7},
        }],
    }
    sheet["rows"].append(header_row(
        [("A2", "车型"), ("B2", "马力"), ("C2", "重量(lbs)"), ("D2", "油耗(mpg)"),
         ("E2", "气缸"), ("F2", "产地")], "orange"))
    for i, r in enumerate(rows):
        ri = 3 + i
        sheet["rows"].append({"index": ri, "cells": [
            {"ref": "A%d" % ri, "value": r["Name"]},
            num_cell("B%d" % ri, r["Horsepower"], "#,##0"),
            num_cell("C%d" % ri, r["Weight_in_lbs"], "#,##0"),
            num_cell("D%d" % ri, r["Miles_per_Gallon"], "0.0"),
            num_cell("E%d" % ri, r["Cylinders"], "0"),
            {"ref": "F%d" % ri, "value": r["Origin"]},
        ]})
    write_product("ds_cars_horsepower", sheet, "orange")


# ---------------------------------------------------------------- 3 各国寿命
def gapminder():
    rows = read_json("gapminder.json")
    year = max(r["year"] for r in rows)
    rows = [r for r in rows if r["year"] == year]
    rows.sort(key=lambda r: r["life_expect"], reverse=True)
    rows = rows[:20]
    cols = [{"width": 20}, {"width": 12}, {"width": 14}, {"width": 16}]
    sheet = {
        "name": "预期寿命",
        "freeze": "A3",
        "columns": cols,
        "merges": ["A1:D1"],
        "rows": [{"index": 1, "cells": [title_cell("各国预期寿命 Top 20（%d 年）" % year, "D")]}],
        "charts": [{
            "type": "bar", "data_range": "B3:B%d" % (2 + len(rows)),
            "categories": "A3:A%d" % (2 + len(rows)), "title": "预期寿命（岁）",
            "legend": "none", "colors": ["548235"], "anchor": "F2", "size": {"w": 10, "h": 7},
        }],
    }
    sheet["rows"].append(header_row(
        [("A2", "国家"), ("B2", "预期寿命"), ("C2", "生育率"), ("D2", "人口")], "green"))
    for i, r in enumerate(rows):
        ri = 3 + i
        sheet["rows"].append({"index": ri, "cells": [
            {"ref": "A%d" % ri, "value": r["country"]},
            num_cell("B%d" % ri, round(r["life_expect"], 1), "0.0"),
            num_cell("C%d" % ri, round(r["fertility"], 2), "0.00"),
            num_cell("D%d" % ri, r["pop"], "#,##0"),
        ]})
    write_product("ds_gapminder_life", sheet, "green")


# ---------------------------------------------------------------- 4 西雅图天气
def seattle():
    rows = read_csv("seattle-weather.csv")
    agg = defaultdict(lambda: {"tmax": 0.0, "tmin": 0.0, "precip": 0.0, "sun": 0, "n": 0})
    for r in rows:
        m = int(r["date"][5:7])
        a = agg[m]
        a["tmax"] += float(r["temp_max"])
        a["tmin"] += float(r["temp_min"])
        a["precip"] += float(r["precipitation"])
        a["sun"] += 1 if r["weather"] == "sun" else 0
        a["n"] += 1
    cols = [{"width": 8}, {"width": 12}, {"width": 12}, {"width": 12}, {"width": 10}]
    sheet = {
        "name": "月度天气",
        "freeze": "A3",
        "columns": cols,
        "merges": ["A1:E1"],
        "rows": [{"index": 1, "cells": [title_cell("西雅图月度天气（2012–2015 平均）", "E")]}],
        "charts": [{
            "type": "line", "data_range": "B3:B14", "categories": "A3:A14",
            "title": "平均最高气温（℃）", "legend": "none", "colors": ["2E9CA6"],
            "anchor": "G2", "size": {"w": 9, "h": 6},
        }],
    }
    sheet["rows"].append(header_row(
        [("A2", "月份"), ("B2", "均高温"), ("C2", "均低温"), ("D2", "总降水(mm)"), ("E2", "晴天数")], "cyan"))
    for m in range(1, 13):
        a = agg[m]
        ri = 2 + m
        sheet["rows"].append({"index": ri, "cells": [
            {"ref": "A%d" % ri, "value": "%d月" % m, "style": {"alignment": {"horizontal": "center"}}},
            num_cell("B%d" % ri, round(a["tmax"] / a["n"], 1), "0.0"),
            num_cell("C%d" % ri, round(a["tmin"] / a["n"], 1), "0.0"),
            num_cell("D%d" % ri, round(a["precip"], 1), "0.0"),
            num_cell("E%d" % ri, a["sun"], "0"),
        ]})
    write_product("ds_seattle_weather", sheet, "cyan")


# ---------------------------------------------------------------- 5 专业起薪
def majors():
    rows = read_csv("college-majors.csv")
    rows = [r for r in rows if r.get("Median")]
    rows.sort(key=lambda r: int(r["Median"]), reverse=True)
    rows = rows[:12]
    cols = [{"width": 28}, {"width": 14}, {"width": 12}, {"width": 12}]
    sheet = {
        "name": "专业起薪",
        "freeze": "A3",
        "columns": cols,
        "merges": ["A1:D1"],
        "rows": [{"index": 1, "cells": [title_cell("本科专业中位起薪 Top 12（美元）", "D")]}],
        "charts": [{
            "type": "bar", "data_range": "B3:B%d" % (2 + len(rows)),
            "categories": "A3:A%d" % (2 + len(rows)), "title": "中位起薪",
            "legend": "none", "colors": ["C00000"], "anchor": "F2", "size": {"w": 10, "h": 7},
        }],
    }
    sheet["rows"].append(header_row(
        [("A2", "专业"), ("B2", "中位起薪"), ("C2", "就业率"), ("D2", "女性占比")], "red"))
    for i, r in enumerate(rows):
        ri = 3 + i
        sheet["rows"].append({"index": ri, "cells": [
            {"ref": "A%d" % ri, "value": r["Major"].title()},
            num_cell("B%d" % ri, int(r["Median"]), "#,##0"),
            num_cell("C%d" % ri, round(1 - float(r["Unemployment_rate"]), 4), "0.0%"),
            num_cell("D%d" % ri, round(float(r["ShareWomen"]), 4), "0.0%"),
        ]})
    write_product("ds_college_majors", sheet, "red")


def main():
    if not os.path.isdir(DATA):
        sys.exit("数据目录不存在: %s（先运行 scripts/collect_datasets.sh）" % DATA)
    for fn in (flights, cars, gapminder, seattle, majors):
        try:
            fn()
        except Exception as e:  # noqa: BLE001
            print("  FAIL  %s: %s" % (fn.__name__, e))
    print("DONE ->", OUT)


if __name__ == "__main__":
    main()
