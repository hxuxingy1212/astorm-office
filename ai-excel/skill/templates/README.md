# Excel 样式模板库

由 `json2xlsx template extract` 从 ~2120 份真实 Excel（SpreadsheetBench 评测集、ClosedXML、pandas、LibreOffice、excelize、Microsoft 官方样例等）批量提取并按风格聚类而成；
每个模板含：结构化 `template.json`、面向大模型的 `TEMPLATE.md`、以及可直接填数的 `skeleton/` 产物目录。
色板同时给出「样式色板」（来自单元格样式）与「视觉色板」（LibreOffice 渲染后图像识别的主色）。

## 模板一览

| 模板 | 风格 | 样本数 | 表头色 | 视觉主色 | 骨架 | 数字格式 |
|------|------|-------|--------|----------|------|----------|
| [plain-report](plain-report/TEMPLATE.md) | plain | 1442 | `-` | `FAF240`、`B2B0AE`、`726C69` | 有 | `"$"#,##0.00`、`#,##0`、`#,##0.00\ [$€-40C]` |
| [accent-yellow](accent-yellow/TEMPLATE.md) | plain-yellow | 186 | `-` | `FFD7D7`、`FEF1F1`、`4B4A4A` | 有 | - |
| [header-yellow](header-yellow/TEMPLATE.md) | hdr-yellow | 154 | `FFFF00` | `B9CDE5`、`FFFF99`、`4A4E50` | 有 | `" Excellent"`、`" Fair"`、`" Good"` |
| [accent-red](accent-red/TEMPLATE.md) | plain-red | 113 | `-` | `ACB2B0`、`273249`、`D1D1CF` | 有 | `#,##0.00`、`0.0`、`0.0%` |
| [accent-green](accent-green/TEMPLATE.md) | plain-green | 46 | `-` | - | 有 | `@`、`mmmm\ d\,\ yyyy\ \-\ h:mm:ss\ AM/PM;@`、`mmmm\ d\,\ yyyy\ h:mmAM/PM` |
| [header-light](header-light/TEMPLATE.md) | hdr-light | 36 | `FFFFFF` | `FFFF00`、`B7B7B7`、`C0C0C0` | 有 | `#,##0.00`、`0.00`、`m/d/yyyy;@` |
| [header-blue](header-blue/TEMPLATE.md) | hdr-blue | 33 | `004477` | `DCE6F2`、`6B98E5`、`00FFFF` | 有 | `$ #,##0`、`d-mmm-yy` |
| [header-cyan](header-cyan/TEMPLATE.md) | hdr-cyan | 22 | `00FFFF` | `C0C0C0`、`858583`、`000000` | 有 | `0`、`0%`、`0_);[Red]\(0\)` |
| [header-red](header-red/TEMPLATE.md) | hdr-red | 20 | `FF0000` | `FA7F72`、`FFA07A`、`A2A2D0` | 有 | - |
| [header-orange](header-orange/TEMPLATE.md) | hdr-orange | 16 | `FFCC99` | `FFCC99`、`B19F90`、`58545B` | 有 | `#,##0`、`#,##0.00`、`#,##0.0000` |
| [accent-blue](accent-blue/TEMPLATE.md) | plain-blue | 15 | `-` | `C4D0C7`、`000000`、`0D1C55` | 有 | - |
| [header-gray](header-gray/TEMPLATE.md) | hdr-gray | 11 | `000000` | `FF0E0E`、`1C1212`、`686868` | 有 | `d-mmm-yy`、`h:mm` |
| [header-green](header-green/TEMPLATE.md) | hdr-green | 10 | `92D050` | `585858`、`7E7E7E`、`C1C1C1` | 有 | - |
| [header-magenta](header-magenta/TEMPLATE.md) | hdr-magenta | 6 | `D569B6` | `D569B6`、`222A35`、`674267` | 有 | `"£"#,##0`、`"£"#,##0.00`、`"£"#,##0;[Red]\-"£"#,##0` |
| [accent-magenta](accent-magenta/TEMPLATE.md) | plain-magenta | 4 | `-` | `FF0000`、`FFA40E`、`0000EF` | 有 | - |
| [accent-cyan](accent-cyan/TEMPLATE.md) | plain-cyan | 4 | `-` | `BFBFBF`、`F2F2F2`、`DCDCDC` | 有 | `0.0`、`0.00` |

## 使用要点（务必遵守）

- **中文/字体**：模板默认字体多为 Calibri（拉丁）。生成含中文的内容时，给表头与数据设置 `font.name`（`微软雅黑` / `PingFang SC` / `等线`），或设工作簿 `default_font`；否则某些环境会显示空白。
- **数字格式**：数值列显式设 `number_format`（金额 `#,##0`、比率 `0.0%`、日期 `yyyy-mm-dd`），避免裸浮点（`13.100000000000001%`）。
- **对齐**：数值右、文本左、表头居中；合计/强调行加粗+顶部边框；深色填充配浅色文字。
- **列宽**：按内容显式设置（中文按约 2 字符宽估算）。
- **图表/打印**：图表放数据右侧/下方；需打印时设 `print_area`；类别标签过长时用短标签。
- **验收**：`repack` 后用 `render` 目视核对。

## 用法（大模型/CLI）

1. 用户上传任意 Excel → 提取其风格：

```bash
json2xlsx template extract 用户文件.xlsx -o my_tpl/
# 产出 my_tpl/template.json + my_tpl/TEMPLATE.md + my_tpl/skeleton/
```

2. 也可直接选用本库中风格接近的模板：

```bash
cp -r skill/templates/header-blue/skeleton ./work/
# 在 work/xl/worksheets/sheetN.json 中填写业务数据（首行表头样式保留）
json2xlsx repack work/ -o out.xlsx
json2xlsx render work/ -o preview.html   # 视觉验收
```

3. 生成规范见 [`../SKILL.md`](../SKILL.md)；字段定义见 [`../references/schema.json`](../references/schema.json)。
