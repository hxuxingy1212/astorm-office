#!/usr/bin/env bash
# 生成 5 个场景报表的「真实 xlsx」（产物目录 -> repack）。
# 产物：
#   examples/out/reports/*.xlsx          可直接用 Excel/LibreOffice 打开
#   examples/out/reports/<name>/         产物目录（workbook.json + sheets）
# 用法：./scripts/gen_reports.sh
set -euo pipefail
cd "$(dirname "$0")/.."
BIN="$(pwd)/target/debug/json2xlsx"
cargo build -q -p json2xlsx
OUT="examples/out/reports"
rm -rf "$OUT"; mkdir -p "$OUT"

mk() { mkdir -p "$OUT/$1/xl/worksheets"; }

# ---------------------------------------------------------------- 1 销售汇总
mk finance
cat > "$OUT/finance/workbook.json" <<'JSON'
{
  "meta": { "title": "全球销售汇总", "author": "ai-excel" },
  "default_font": { "name": "微软雅黑", "size": 11 },
  "styles": {
    "header-blue": { "font": { "bold": true, "color": "FFFFFF", "name": "微软雅黑" }, "fill": "4472C4", "alignment": { "horizontal": "center", "vertical": "center" } },
    "total": { "font": { "bold": true, "name": "微软雅黑" }, "border": { "top": "thin", "color": "808080" }, "fill": "F2F2F2" }
  },
  "sheets": [ "xl/worksheets/sheet1.json" ]
}
JSON
cat > "$OUT/finance/xl/worksheets/sheet1.json" <<'JSON'
{
  "name": "销售汇总",
  "freeze": "A3",
  "gridlines": false,
  "columns": [ { "width": 26 }, { "width": 14 }, { "width": 14 }, { "width": 10 } ],
  "merges": ["A1:D1"],
  "auto_filter": "A2:D8",
  "print_area": "A1:D8",
  "charts": [
    { "type": "column", "data_range": "B3:B8", "categories": "A3:A8", "title": "各国销售收入",
      "legend": "none", "show_values": true, "colors": ["4472C4"], "anchor": "F2", "size": { "w": 8, "h": 5 } }
  ],
  "rows": [
    { "index": 1, "cells": [ { "ref": "A1", "value": "全球销售汇总（按国家）", "style": { "font": { "bold": true, "size": 14, "name": "微软雅黑" }, "alignment": { "horizontal": "center" } } } ] },
    { "index": 2, "cells": [
      { "ref": "A2", "value": "国家", "style": "header-blue" },
      { "ref": "B2", "value": "销售收入", "style": "header-blue" },
      { "ref": "C2", "value": "利润", "style": "header-blue" },
      { "ref": "D2", "value": "利润率", "style": "header-blue" } ] },
    { "index": 3, "cells": [ { "ref": "A3", "value": "United States of America" }, { "ref": "B3", "value": 27269358, "number_format": "#,##0" }, { "ref": "C3", "value": 2995541, "number_format": "#,##0" }, { "ref": "D3", "value": 0.1099, "number_format": "0.0%" } ] },
    { "index": 4, "cells": [ { "ref": "A4", "value": "Canada" }, { "ref": "B4", "value": 1753334, "number_format": "#,##0" }, { "ref": "C4", "value": 187126, "number_format": "#,##0" }, { "ref": "D4", "value": 0.1067, "number_format": "0.0%" } ] },
    { "index": 5, "cells": [ { "ref": "A5", "value": "France" }, { "ref": "B5", "value": 2460642, "number_format": "#,##0" }, { "ref": "C5", "value": 269339, "number_format": "#,##0" }, { "ref": "D5", "value": 0.1095, "number_format": "0.0%" } ] },
    { "index": 6, "cells": [ { "ref": "A6", "value": "Germany" }, { "ref": "B6", "value": 2402960, "number_format": "#,##0" }, { "ref": "C6", "value": 214294, "number_format": "#,##0" }, { "ref": "D6", "value": 0.0892, "number_format": "0.0%" } ] },
    { "index": 7, "cells": [ { "ref": "A7", "value": "Mexico" }, { "ref": "B7", "value": 2024103, "number_format": "#,##0" }, { "ref": "C7", "value": 227251, "number_format": "#,##0" }, { "ref": "D7", "value": 0.1123, "number_format": "0.0%" } ] },
    { "index": 8, "cells": [ { "ref": "A8", "value": "合计", "style": "total" }, { "ref": "B8", "formula": "SUM(B3:B7)", "number_format": "#,##0", "style": "total" }, { "ref": "C8", "formula": "SUM(C3:C7)", "number_format": "#,##0", "style": "total" }, { "ref": "D8", "formula": "C8/B8", "number_format": "0.0%", "style": "total" } ] }
  ]
}
JSON

# ---------------------------------------------------------------- 2 成绩表
mk gradebook
cat > "$OUT/gradebook/workbook.json" <<'JSON'
{
  "meta": { "title": "2026 春季学期成绩表", "author": "ai-excel" },
  "default_font": { "name": "微软雅黑", "size": 11 },
  "styles": {
    "header-green": { "font": { "bold": true, "color": "FFFFFF", "name": "微软雅黑" }, "fill": "548235", "alignment": { "horizontal": "center", "vertical": "center" } }
  },
  "sheets": [ "xl/worksheets/sheet1.json" ]
}
JSON
cat > "$OUT/gradebook/xl/worksheets/sheet1.json" <<'JSON'
{
  "name": "成绩表",
  "freeze": "A3",
  "columns": [ { "width": 10 }, { "width": 10 }, { "width": 8 }, { "width": 8 }, { "width": 8 }, { "width": 8 }, { "width": 9 }, { "width": 8 } ],
  "merges": ["A1:H1"],
  "auto_filter": "A2:H8",
  "conditional_formats": [ { "range": "C3:F8", "type": "colorScale", "colors": ["63BE7B", "FFEB84", "F8696B"] } ],
  "rows": [
    { "index": 1, "cells": [ { "ref": "A1", "value": "2026 春季学期 成绩表", "style": { "font": { "bold": true, "size": 14, "name": "微软雅黑" }, "alignment": { "horizontal": "center" } } } ] },
    { "index": 2, "cells": [
      { "ref": "A2", "value": "学号", "style": "header-green" }, { "ref": "B2", "value": "姓名", "style": "header-green" },
      { "ref": "C2", "value": "作业1", "style": "header-green" }, { "ref": "D2", "value": "作业2", "style": "header-green" },
      { "ref": "E2", "value": "期中", "style": "header-green" }, { "ref": "F2", "value": "期末", "style": "header-green" },
      { "ref": "G2", "value": "总评", "style": "header-green" }, { "ref": "H2", "value": "等级", "style": "header-green" } ] },
    { "index": 3, "cells": [ { "ref": "A3", "value": "S001" }, { "ref": "B3", "value": "张伟" }, { "ref": "C3", "value": 88 }, { "ref": "D3", "value": 92 }, { "ref": "E3", "value": 85 }, { "ref": "F3", "value": 90 }, { "ref": "G3", "formula": "ROUND(AVERAGE(C3:F3),1)", "number_format": "0.0" }, { "ref": "H3", "formula": "IF(G3>=90,\"A\",IF(G3>=80,\"B\",IF(G3>=70,\"C\",\"D\")))", "style": { "alignment": { "horizontal": "center" } } } ] },
    { "index": 4, "cells": [ { "ref": "A4", "value": "S002" }, { "ref": "B4", "value": "李娜" }, { "ref": "C4", "value": 76 }, { "ref": "D4", "value": 81 }, { "ref": "E4", "value": 79 }, { "ref": "F4", "value": 84 }, { "ref": "G4", "formula": "ROUND(AVERAGE(C4:F4),1)", "number_format": "0.0" }, { "ref": "H4", "formula": "IF(G4>=90,\"A\",IF(G4>=80,\"B\",IF(G4>=70,\"C\",\"D\")))", "style": { "alignment": { "horizontal": "center" } } } ] },
    { "index": 5, "cells": [ { "ref": "A5", "value": "S003" }, { "ref": "B5", "value": "王芳" }, { "ref": "C5", "value": 93 }, { "ref": "D5", "value": 95 }, { "ref": "E5", "value": 91 }, { "ref": "F5", "value": 96 }, { "ref": "G5", "formula": "ROUND(AVERAGE(C5:F5),1)", "number_format": "0.0" }, { "ref": "H5", "formula": "IF(G5>=90,\"A\",IF(G5>=80,\"B\",IF(G5>=70,\"C\",\"D\")))", "style": { "alignment": { "horizontal": "center" } } } ] },
    { "index": 6, "cells": [ { "ref": "A6", "value": "S004" }, { "ref": "B6", "value": "刘洋" }, { "ref": "C6", "value": 68 }, { "ref": "D6", "value": 72 }, { "ref": "E6", "value": 70 }, { "ref": "F6", "value": 75 }, { "ref": "G6", "formula": "ROUND(AVERAGE(C6:F6),1)", "number_format": "0.0" }, { "ref": "H6", "formula": "IF(G6>=90,\"A\",IF(G6>=80,\"B\",IF(G6>=70,\"C\",\"D\")))", "style": { "alignment": { "horizontal": "center" } } } ] },
    { "index": 7, "cells": [ { "ref": "A7", "value": "S005" }, { "ref": "B7", "value": "陈静" }, { "ref": "C7", "value": 85 }, { "ref": "D7", "value": 88 }, { "ref": "E7", "value": 90 }, { "ref": "F7", "value": 87 }, { "ref": "G7", "formula": "ROUND(AVERAGE(C7:F7),1)", "number_format": "0.0" }, { "ref": "H7", "formula": "IF(G7>=90,\"A\",IF(G7>=80,\"B\",IF(G7>=70,\"C\",\"D\")))", "style": { "alignment": { "horizontal": "center" } } } ] }
  ]
}
JSON

# ---------------------------------------------------------------- 3 统计公报
mk govstats
cat > "$OUT/govstats/workbook.json" <<'JSON'
{
  "meta": { "title": "地区主要经济指标", "author": "ai-excel" },
  "default_font": { "name": "微软雅黑", "size": 11 },
  "styles": {
    "header-gray": { "font": { "bold": true, "color": "FFFFFF", "name": "微软雅黑" }, "fill": "404040", "alignment": { "horizontal": "center", "vertical": "center" } }
  },
  "sheets": [ "xl/worksheets/sheet1.json" ]
}
JSON
cat > "$OUT/govstats/xl/worksheets/sheet1.json" <<'JSON'
{
  "name": "主要指标",
  "freeze": "A3",
  "columns": [ { "width": 10 }, { "width": 18 }, { "width": 16 }, { "width": 14 }, { "width": 14 }, { "width": 12 } ],
  "merges": ["A1:F1"],
  "print_area": "A1:F8",
  "rows": [
    { "index": 1, "cells": [ { "ref": "A1", "value": "2021—2025 年地区主要经济指标", "style": { "font": { "bold": true, "size": 14, "name": "微软雅黑" }, "alignment": { "horizontal": "center" } } } ] },
    { "index": 2, "cells": [
      { "ref": "A2", "value": "年份", "style": "header-gray" }, { "ref": "B2", "value": "地区生产总值(万元)", "style": "header-gray" },
      { "ref": "C2", "value": "固定资产投资(万元)", "style": "header-gray" }, { "ref": "D2", "value": "社会消费品零售(万元)", "style": "header-gray" },
      { "ref": "E2", "value": "居民人均可支配收入(元)", "style": "header-gray" }, { "ref": "F2", "value": "同比", "style": "header-gray" } ] },
    { "index": 3, "cells": [ { "ref": "A3", "value": "2021", "style": { "alignment": { "horizontal": "center" } } }, { "ref": "B3", "value": 4187300, "number_format": "#,##0" }, { "ref": "C3", "value": 1654200, "number_format": "#,##0" }, { "ref": "D3", "value": 1823400, "number_format": "#,##0" }, { "ref": "E3", "value": 38520, "number_format": "#,##0" }, { "ref": "F3", "value": 0.081, "number_format": "0.0%" } ] },
    { "index": 4, "cells": [ { "ref": "A4", "value": "2022", "style": { "alignment": { "horizontal": "center" } } }, { "ref": "B4", "value": 4536100, "number_format": "#,##0" }, { "ref": "C4", "value": 1780900, "number_format": "#,##0" }, { "ref": "D4", "value": 1901200, "number_format": "#,##0" }, { "ref": "E4", "value": 41260, "number_format": "#,##0" }, { "ref": "F4", "value": 0.071, "number_format": "0.0%" } ] },
    { "index": 5, "cells": [ { "ref": "A5", "value": "2023", "style": { "alignment": { "horizontal": "center" } } }, { "ref": "B5", "value": 4908700, "number_format": "#,##0" }, { "ref": "C5", "value": 1932700, "number_format": "#,##0" }, { "ref": "D5", "value": 2093400, "number_format": "#,##0" }, { "ref": "E5", "value": 44180, "number_format": "#,##0" }, { "ref": "F5", "value": 0.082, "number_format": "0.0%" } ] },
    { "index": 6, "cells": [ { "ref": "A6", "value": "2024", "style": { "alignment": { "horizontal": "center" } } }, { "ref": "B6", "value": 5293400, "number_format": "#,##0" }, { "ref": "C6", "value": 2085600, "number_format": "#,##0" }, { "ref": "D6", "value": 2271900, "number_format": "#,##0" }, { "ref": "E6", "value": 47250, "number_format": "#,##0" }, { "ref": "F6", "value": 0.078, "number_format": "0.0%" } ] },
    { "index": 7, "cells": [ { "ref": "A7", "value": "2025", "style": { "alignment": { "horizontal": "center" } } }, { "ref": "B7", "value": 5710800, "number_format": "#,##0" }, { "ref": "C7", "value": 2251400, "number_format": "#,##0" }, { "ref": "D7", "value": 2467300, "number_format": "#,##0" }, { "ref": "E7", "value": 50530, "number_format": "#,##0" }, { "ref": "F7", "value": 0.079, "number_format": "0.0%" } ] },
    { "index": 8, "cells": [ { "ref": "A8", "value": "年均增速", "style": { "font": { "bold": true, "name": "微软雅黑" } } }, { "ref": "B8", "formula": "(B7/B3)^(1/4)-1", "number_format": "0.0%", "style": { "font": { "bold": true } } }, { "ref": "C8", "formula": "(C7/C3)^(1/4)-1", "number_format": "0.0%", "style": { "font": { "bold": true } } }, { "ref": "D8", "formula": "(D7/D3)^(1/4)-1", "number_format": "0.0%", "style": { "font": { "bold": true } } }, { "ref": "E8", "formula": "(E7/E3)^(1/4)-1", "number_format": "0.0%", "style": { "font": { "bold": true } } }, { "ref": "F8", "formula": "AVERAGE(F3:F7)", "number_format": "0.0%", "style": { "font": { "bold": true } } } ] }
  ]
}
JSON

# ---------------------------------------------------------------- 4 IT 资产
mk itassets
cat > "$OUT/itassets/workbook.json" <<'JSON'
{
  "meta": { "title": "IT 资产清单", "author": "ai-excel" },
  "default_font": { "name": "微软雅黑", "size": 11 },
  "styles": {
    "header-cyan": { "font": { "bold": true, "color": "FFFFFF", "name": "微软雅黑" }, "fill": "2E9CA6", "alignment": { "horizontal": "center", "vertical": "center" } }
  },
  "sheets": [ "xl/worksheets/sheet1.json" ]
}
JSON
cat > "$OUT/itassets/xl/worksheets/sheet1.json" <<'JSON'
{
  "name": "资产清单",
  "freeze": "A3",
  "columns": [ { "width": 12 }, { "width": 16 }, { "width": 10 }, { "width": 16 }, { "width": 24 }, { "width": 10 }, { "width": 12 }, { "width": 12 }, { "width": 10 } ],
  "merges": ["A1:I1"],
  "auto_filter": "A2:I9",
  "rows": [
    { "index": 1, "cells": [ { "ref": "A1", "value": "IT 资产清单（2026 Q1）", "style": { "font": { "bold": true, "size": 14, "name": "微软雅黑" }, "alignment": { "horizontal": "center" } } } ] },
    { "index": 2, "cells": [
      { "ref": "A2", "value": "资产编号", "style": "header-cyan" }, { "ref": "B2", "value": "主机名", "style": "header-cyan" },
      { "ref": "C2", "value": "责任人", "style": "header-cyan" }, { "ref": "D2", "value": "操作系统", "style": "header-cyan" },
      { "ref": "E2", "value": "CPU", "style": "header-cyan" }, { "ref": "F2", "value": "内存(GB)", "style": "header-cyan" },
      { "ref": "G2", "value": "采购日期", "style": "header-cyan" }, { "ref": "H2", "value": "保修到期", "style": "header-cyan" },
      { "ref": "I2", "value": "状态", "style": "header-cyan" } ] },
    { "index": 3, "cells": [ { "ref": "A3", "value": "IT-2023-001" }, { "ref": "B3", "value": "srv-erp-01" }, { "ref": "C3", "value": "王工" }, { "ref": "D3", "value": "Windows Server 2019" }, { "ref": "E3", "value": "Intel Xeon Silver 4310" }, { "ref": "F3", "value": 64, "number_format": "#,##0" }, { "ref": "G3", "type": "date", "value": "2023-03-15", "number_format": "yyyy-mm-dd" }, { "ref": "H3", "type": "date", "value": "2026-03-15", "number_format": "yyyy-mm-dd" }, { "ref": "I3", "value": "在用" } ] },
    { "index": 4, "cells": [ { "ref": "A4", "value": "IT-2023-002" }, { "ref": "B4", "value": "srv-db-01" }, { "ref": "C4", "value": "李工" }, { "ref": "D4", "value": "CentOS 7.9" }, { "ref": "E4", "value": "AMD EPYC 7302" }, { "ref": "F4", "value": 128, "number_format": "#,##0" }, { "ref": "G4", "type": "date", "value": "2023-06-01", "number_format": "yyyy-mm-dd" }, { "ref": "H4", "type": "date", "value": "2026-06-01", "number_format": "yyyy-mm-dd" }, { "ref": "I4", "value": "在用" } ] },
    { "index": 5, "cells": [ { "ref": "A5", "value": "IT-2024-011" }, { "ref": "B5", "value": "pc-hr-023" }, { "ref": "C5", "value": "张敏" }, { "ref": "D5", "value": "Windows 11 专业版" }, { "ref": "E5", "value": "Intel Core i5-13400" }, { "ref": "F5", "value": 16, "number_format": "#,##0" }, { "ref": "G5", "type": "date", "value": "2024-01-20", "number_format": "yyyy-mm-dd" }, { "ref": "H5", "type": "date", "value": "2027-01-20", "number_format": "yyyy-mm-dd" }, { "ref": "I5", "value": "在用" } ] },
    { "index": 6, "cells": [ { "ref": "A6", "value": "IT-2022-007" }, { "ref": "B6", "value": "srv-file-02" }, { "ref": "C6", "value": "王工" }, { "ref": "D6", "value": "Ubuntu 22.04" }, { "ref": "E6", "value": "Intel Xeon E-2336" }, { "ref": "F6", "value": 32, "number_format": "#,##0" }, { "ref": "G6", "type": "date", "value": "2022-05-10", "number_format": "yyyy-mm-dd" }, { "ref": "H6", "type": "date", "value": "2025-05-10", "number_format": "yyyy-mm-dd" }, { "ref": "I6", "value": "待更换" } ] },
    { "index": 7, "cells": [ { "ref": "A7", "value": "IT-2024-018" }, { "ref": "B7", "value": "pc-dev-101" }, { "ref": "C7", "value": "陈磊" }, { "ref": "D7", "value": "macOS 15" }, { "ref": "E7", "value": "Apple M3 Pro" }, { "ref": "F7", "value": 36, "number_format": "#,##0" }, { "ref": "G7", "type": "date", "value": "2024-09-01", "number_format": "yyyy-mm-dd" }, { "ref": "H7", "type": "date", "value": "2027-09-01", "number_format": "yyyy-mm-dd" }, { "ref": "I7", "value": "在用" } ] },
    { "index": 8, "cells": [ { "ref": "A8", "value": "IT-2021-003" }, { "ref": "B8", "value": "srv-mail-01" }, { "ref": "C8", "value": "李工" }, { "ref": "D8", "value": "Windows Server 2016" }, { "ref": "E8", "value": "Intel Xeon E5-2620" }, { "ref": "F8", "value": 32, "number_format": "#,##0" }, { "ref": "G8", "type": "date", "value": "2021-11-11", "number_format": "yyyy-mm-dd" }, { "ref": "H8", "type": "date", "value": "2024-11-11", "number_format": "yyyy-mm-dd" }, { "ref": "I8", "value": "已报废" } ] }
  ]
}
JSON

# ---------------------------------------------------------------- 5 项目跟踪
mk project
cat > "$OUT/project/workbook.json" <<'JSON'
{
  "meta": { "title": "项目进度跟踪", "author": "ai-excel" },
  "default_font": { "name": "微软雅黑", "size": 11 },
  "styles": {
    "header-orange": { "font": { "bold": true, "color": "FFFFFF", "name": "微软雅黑" }, "fill": "ED7D31", "alignment": { "horizontal": "center", "vertical": "center" } }
  },
  "sheets": [ "xl/worksheets/sheet1.json" ]
}
JSON
cat > "$OUT/project/xl/worksheets/sheet1.json" <<'JSON'
{
  "name": "项目进度",
  "freeze": "A3",
  "columns": [ { "width": 24 }, { "width": 10 }, { "width": 12 }, { "width": 12 }, { "width": 10 }, { "width": 10 }, { "width": 14 } ],
  "merges": ["A1:G1"],
  "auto_filter": "A2:G8",
  "conditional_formats": [
    { "range": "E3:E8", "type": "dataBar", "color": "638EC6" }
  ],
  "rows": [
    { "index": 1, "cells": [ { "ref": "A1", "value": "XX 系统建设项目进度跟踪", "style": { "font": { "bold": true, "size": 14, "name": "微软雅黑" }, "alignment": { "horizontal": "center" } } } ] },
    { "index": 2, "cells": [
      { "ref": "A2", "value": "任务", "style": "header-orange" }, { "ref": "B2", "value": "负责人", "style": "header-orange" },
      { "ref": "C2", "value": "开始", "style": "header-orange" }, { "ref": "D2", "value": "结束", "style": "header-orange" },
      { "ref": "E2", "value": "进度", "style": "header-orange" }, { "ref": "F2", "value": "状态", "style": "header-orange" },
      { "ref": "G2", "value": "预算(元)", "style": "header-orange" } ] },
    { "index": 3, "cells": [ { "ref": "A3", "value": "需求调研" }, { "ref": "B3", "value": "张三" }, { "ref": "C3", "type": "date", "value": "2026-01-05", "number_format": "yyyy-mm-dd" }, { "ref": "D3", "type": "date", "value": "2026-01-20", "number_format": "yyyy-mm-dd" }, { "ref": "E3", "value": 1.0, "number_format": "0%" }, { "ref": "F3", "value": "已完成" }, { "ref": "G3", "value": 80000, "number_format": "#,##0" } ] },
    { "index": 4, "cells": [ { "ref": "A4", "value": "系统设计" }, { "ref": "B4", "value": "李四" }, { "ref": "C4", "type": "date", "value": "2026-01-21", "number_format": "yyyy-mm-dd" }, { "ref": "D4", "type": "date", "value": "2026-02-15", "number_format": "yyyy-mm-dd" }, { "ref": "E4", "value": 1.0, "number_format": "0%" }, { "ref": "F4", "value": "已完成" }, { "ref": "G4", "value": 120000, "number_format": "#,##0" } ] },
    { "index": 5, "cells": [ { "ref": "A5", "value": "后端开发" }, { "ref": "B5", "value": "王五" }, { "ref": "C5", "type": "date", "value": "2026-02-16", "number_format": "yyyy-mm-dd" }, { "ref": "D5", "type": "date", "value": "2026-04-30", "number_format": "yyyy-mm-dd" }, { "ref": "E5", "value": 0.6, "number_format": "0%" }, { "ref": "F5", "value": "进行中" }, { "ref": "G5", "value": 260000, "number_format": "#,##0" } ] },
    { "index": 6, "cells": [ { "ref": "A6", "value": "前端开发" }, { "ref": "B6", "value": "赵六" }, { "ref": "C6", "type": "date", "value": "2026-03-01", "number_format": "yyyy-mm-dd" }, { "ref": "D6", "type": "date", "value": "2026-05-15", "number_format": "yyyy-mm-dd" }, { "ref": "E6", "value": 0.45, "number_format": "0%" }, { "ref": "F6", "value": "进行中" }, { "ref": "G6", "value": 180000, "number_format": "#,##0" } ] },
    { "index": 7, "cells": [ { "ref": "A7", "value": "联调测试" }, { "ref": "B7", "value": "钱七" }, { "ref": "C7", "type": "date", "value": "2026-05-16", "number_format": "yyyy-mm-dd" }, { "ref": "D7", "type": "date", "value": "2026-06-20", "number_format": "yyyy-mm-dd" }, { "ref": "E7", "value": 0.1, "number_format": "0%" }, { "ref": "F7", "value": "未开始" }, { "ref": "G7", "value": 90000, "number_format": "#,##0" } ] },
    { "index": 8, "cells": [ { "ref": "A8", "value": "合计", "style": { "font": { "bold": true, "name": "微软雅黑" } } }, { "ref": "G8", "formula": "SUM(G3:G7)", "number_format": "#,##0", "style": { "font": { "bold": true } } } ] }
  ]
}
JSON

# ---------------------------------------------------------------- pack
for name in finance gradebook govstats itassets project; do
  "$BIN" repack "$OUT/$name" -o "$OUT/$name.xlsx" >/dev/null
  echo "  built  $OUT/$name.xlsx"
done
echo "DONE -> $OUT"
