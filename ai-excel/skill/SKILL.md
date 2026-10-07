---
name: ai-excel
description: 为 ai-excel（json2xlsx）项目生成与编辑专业 Excel 工作簿。工作方式：以 json2xlsx 的“产物目录”为中间形态（workbook.json + 每个工作表 xl/worksheets/sheetN.json），用 json2xlsx 完成从 JSON 构建 xlsx（repack）、查看（view）、精准修改（edit）、HTML 预览（render）。当用户说“做个表格”“生成 Excel”“导出报表”“把这份数据整理成 xlsx”“修改这个表格的第 N 行/某列”“加个汇总公式”，或需要程序化处理 xlsx 文件（解包/编辑/重建）时使用，即使用户没有提到 JSON 或命令行也应触发。
version: 1.0.0
metadata:
  category: 办公自动化
  tags: xlsx, excel, 表格, spreadsheet, json2xlsx, 报表, CLI
compatibility: 需要 Rust 工具链（cargo）或已安装的 json2xlsx
---

# Excel 分册（json2xlsx）

> 通用契约、工作流、CLI 速查、edit 语义与通用约束见统一总纲
> [../../skill/SKILL.md](../../skill/SKILL.md)。本册只含 **Excel 格式特有**内容：
> 产物目录结构、JSON 生成规范、模板库与报表自查。

工具是两层结构：

1. **产物目录**（中间形态，JSON 优先）：`workbook.json` + 每个工作表一个 `xl/worksheets/sheetN.json`，是 CLI 操作 xlsx 的统一载体。字段完整定义见 [references/schema.json](references/schema.json)；
2. **json2xlsx**（执行层）：构建/重建、查看、修改、预览。命令细节见 [references/cli.md](references/cli.md)。

## 产物目录结构（核心概念）

```text
book/                                # 产物根
├── workbook.json                    # 顶层：meta/date1904/styles/named_ranges + sheets 文件路径数组
└── xl/
    └── worksheets/
        ├── sheet1.json              # 每个工作表一个 JSON
        └── sheet2.json
```

- `workbook.json` 的 `sheets` 字段在产物目录中是**相对路径数组**：`["xl/worksheets/sheet1.json", …]`
- 从零生成时，Agent 直接写这两个层级的文件；也可从现有 xlsx `unpack` 得到
- `view` / `edit` / `render` 的输入可以是**产物目录**，也可以是 **.xlsx**（一次性解包到临时目录，命令结束自动清理）

## JSON 生成规范

### 输出纪律（Excel 特有）

- 日期用 ISO 字符串 `"2026-01-31"`，并让 `type` 为 `"date"`（会自动配 `yyyy-mm-dd` 数字格式）。
- 数值必须显式或可推断类型；公式写 `formula`（**不带 `=`**），不要写死计算结果。

### workbook.json（顶层）

```json
{
  "meta": { "title": "销售报表", "author": "ai-excel" },
  "date1904": false,
  "styles": {
    "header": { "font": { "bold": true, "color": "FFFFFF" }, "fill": "4472C4", "alignment": { "horizontal": "center" } },
    "money":  { "number_format": "#,##0.00" }
  },
  "named_ranges": [ { "name": "Tax", "ref": "Sheet1!$B$1" } ],
  "sheets": ["xl/worksheets/sheet1.json", "xl/worksheets/sheet2.json"]
}
```

### sheetN.json（工作表）

```json
{
  "name": "Sheet1",
  "tab_color": "4472C4",
  "hidden": false,
  "freeze": "A2",
  "auto_filter": "A1:D10",
  "columns": [ { "width": 20 }, { "width": 14 } ],
  "merges": ["A1:C1"],
  "rows": [
    { "index": 1, "height": 20, "cells": [
      { "ref": "A1", "value": "季度", "style": "header" },
      { "ref": "B1", "value": "营收", "style": "header" }
    ]},
    { "index": 2, "cells": [
      { "ref": "A2", "value": "Q1" },
      { "ref": "B2", "value": 1234.5, "number_format": "#,##0.00" },
      { "ref": "C2", "formula": "SUM(B2:B8)" },
      { "ref": "D2", "type": "date", "value": "2026-01-31" }
    ]}
  ]
}
```

### 图片与图表（sheet 级）

```json
{
  "name": "Dashboard",
  "images": [
    { "src": "assets/logo.png", "anchor": "C3", "size": { "w": 2, "h": 1.5 } }
  ],
  "charts": [
    { "type": "column", "data_range": "B2:B8", "categories": "A2:A8",
      "title": "季度营收", "anchor": "E2", "size": { "w": 6, "h": 4 } }
  ]
}
```

- 图片 `src`：相对产物根（`assets/logo.png`）或绝对路径；本地产物目录内需真实存在，`repack` 会解析并打包进 `xl/media/`。支持本地文件（GIF/PNG/JPEG/BMP）与 http(s) URL（URL 图片在 repack 时下载并打包进 `xl/media/`）。
- 图表 `type`：`column` / `bar` / `line` / `area` / `pie` / `ring` / `scatter`；`data_range` 与 `categories` 都是当前工作表的 A1 区域。
- **多系列**：`data_range` 跨多列（如 `B2:C8`）时，每列生成一个数据系列；`categories` 为分类轴。
- 增强：`legend`（right/left/top/bottom/none）、`category_axis_title` / `value_axis_title`、`show_values`（数据标签）、`colors`（各系列颜色）。
- `anchor` 为左上角单元格（如 `E2`），`size` 单位为英寸。

> 单元格超链接：`{ "ref": "A4", "value": "官网", "link": "https://example.com", "link_tooltip": "打开" }`；内部跳转用 `"link": "#Sheet1!B2"`。

### 数据验证 / 条件格式 / 打印与保护（sheet 级）

```json
{
  "name": "Form",
  "protect": true,
  "print": { "orientation": "landscape", "fit_to_page": true, "fit_to_width": 1, "fit_to_height": 1 },
  "validations": [
    { "range": "A2:A10", "type": "list", "values": ["低", "中", "高"], "allow_blank": true, "show_error": true, "error_title": "无效值", "error_message": "请选择低/中/高" },
    { "range": "B2:B10", "type": "whole", "operator": "between", "formula1": "0", "formula2": "1000" }
  ],
  "conditional_formats": [
    { "range": "B2:B10", "type": "cellIs", "operator": "greaterThan", "formulas": ["100"], "font": { "bold": true, "color": "FF0000" } },
    { "range": "C2:C10", "type": "colorScale", "colors": ["63BE7B", "FFEB84", "F8696B"] },
    { "range": "D2:D10", "type": "dataBar", "color": "638EC6" }
  ]
}
```

- 验证 `type`：`list` / `whole` / `decimal` / `textLength` / `date` / `time` / `custom`；`list` 可直接给 `values`，或用 `formula1` 引用区域。
- 条件格式 `type`：`cellIs`（配 `operator` + `formulas` + `font`/`fill`）、`expression`、`colorScale`（2–3 个 `colors`）、`dataBar`（`color`）、`containsText`/`beginsWith`/`endsWith`（`text`）、`timePeriod`（`time_period`）、`top10`（`rank`/`percent`/`bottom`）、`duplicateValues`、`aboveAverage`（`above_average`/`equal_average`/`std_dev`）、`iconSet`（`icon_style`/`reverse_icons`/`show_value`）。
- `print`：`orientation` / `fit_to_page` / `fit_to_width` / `fit_to_height` / `scale` / `paper_size`。
- `print_area`（如 `A1:C20`）、`print_title_rows`（如 `1:1`）、`print_title_cols`（如 `A:A`）：打印区域与重复标题。
- `protect`：开启工作表保护。

### 结构化表格（sheet.tables）

```json
{
  "name": "Scores",
  "tables": [
    { "range": "A1:B3", "name": "Scores", "style": "TableStyleMedium9",
      "show_banded_rows": true, "columns": ["Name", "Score"] }
  ]
}
```

- `range` 必须包含表头行；`columns` 省略时从表头行自动推导。
- `style` 用 `TableStyle*` 名称（如 `TableStyleMedium2/9`、`TableStyleLight1`）；省略则无样式。
- 生成 `xl/tables/tableN.xml` + 工作表 `tableParts`。
- **汇总行**：`totals_row: true` 开启汇总行；`totals`（与 `columns` 对齐）每项 `{ "label": "合计", "function": "sum", "formula": "SUBTOTAL(...)" }`（`function` 取 `sum`/`average`/`count`/`countNums`/`max`/`min`/`stdDev`/`var`/`custom`）。
- **计算列**：`calculated_formulas`（与 `columns` 对齐）给每列一个公式字符串（不带 `=`）。
- **插入行扩展**：`insert_row: true` 对应 `insertRow="1"`（插入行时自动扩展表格）。

```json
{ "range": "A1:C10", "name": "Scores", "style": "TableStyleMedium9",
  "columns": ["Name", "Score", "Grade"], "totals_row": true,
  "totals": [{ "label": "合计" }, {}, { "function": "average" }],
  "calculated_formulas": ["", "", "IF(B2>=60,\"及格\",\"不及格\")"] }
```

### 视图 / 分组 / 筛选条件 / 批注

```json
{
  "name": "M",
  "gridlines": false, "headings": false, "zoom": 150,
  "show_formulas": true, "tab_selected": true,
  "auto_filter": "A1:C10",
  "filter_columns": [
    { "col_id": 0, "values": ["A", "B"] },
    { "col_id": 2, "operator": "greaterThan", "criteria": "100" }
  ],
  "columns": [ { "width": 12, "outline_level": 1 }, { "hidden": true, "collapsed": true } ],
  "rows": [ { "index": 1, "outline_level": 1, "cells": [ { "ref": "A1", "value": "x", "comment": "这是备注" } ] } ]
}
```

- 视图：`gridlines` / `headings`（设 false 隐藏）、`zoom`（10–400）、`show_formulas`、`tab_selected`。
- 分组：行/列的 `outline_level`（1–7）与 `collapsed`；`hidden` 单独控制隐藏。
- 筛选条件：`filter_columns` 用 `values`（值列表）或 `operator`+`criteria`/`criteria2`（自定义比较，如 `greaterThan` 100）。
- 批注：单元格 `comment` 字段（生成 `commentsN.xml` + VML）。
- 单元格斜线边框：`border.diagonal` + `border.diagonal_up` / `border.diagonal_down` + `border.diagonal_color`。

### Cell 字段

| 字段 | 说明 |
|------|------|
| `ref` | A1 地址，可省略（按位置推断） |
| `type` | `string`/`number`/`boolean`/`date`/`error`；省略时按 `value` 推断 |
| `value` | 字面值；数字/布尔不加引号；日期用 ISO 字符串 |
| `formula` | 公式，**不带前导 `=`** |
| `number_format` | 数字格式码，如 `#,##0.00`、`0%`、`yyyy-mm-dd`、`@` |
| `style` | 命名样式字符串，或内联样式对象 |
| `link` / `link_tooltip` / `comment` | 超链接（含悬浮提示）/ 批注 |
| `runs` | 单元格内富文本数组：`[{ "text": "重点", "bold": true, "color": "FF0000", "strike": true, "underline": "single", "vert_align": "superscript" }]`（与 `type: "richtext"` 搭配） |

内联样式对象（`StyleDef`）：

```json
{
  "font": { "bold": true, "italic": false, "color": "FF0000", "name": "Calibri", "size": 11, "underline": "single", "strike": false },
  "fill": "FFFF00",
  "border": { "all": "thin", "color": "808080" },
  "alignment": { "horizontal": "center", "vertical": "center", "wrap_text": true, "indent": 0, "text_rotation": 0 },
  "number_format": "#,##0.00",
  "locked": false
}
```

> **歧义提醒**：单元格不要用裸 `color` 同时表达文字色与底色；文字色用 `style.font.color`，底色用 `style.fill`。

## Excel 特有命令与工作流

通用命令（unpack/repack/view/edit/render/validate/dump/schema…）见总纲；以下是 Excel 特有部分。

```bash
json2xlsx import <in.csv|tsv> -o out.xlsx [--delim D] [--header]   # CSV/TSV 导入（xlsx 特有）
json2xlsx query <输入> <选择器>                                     # 选择器查询
json2xlsx help cell                                                 # 能力速查（属性/示例，--json 机器可读）
json2xlsx dump in.xlsx > cmds.json && json2xlsx batch in.xlsx --input-file cmds.json -o out.xlsx
json2xlsx serve prod/                                               # 常驻编辑（stdin JSON 行）
```

### 路径语法

首段必须是 `sheet[...]`，索引从 **1** 开始；也支持按表名 `/sheet[Report]`。

```text
/sheet[1]                     工作表
/sheet[1]/cell[A1]            单元格
/sheet[1]/range[A1:C10]       区域（set 广播样式 / merge）
/sheet[1]/row[2]              行
/sheet[1]/col[C]              列（col[3] 亦可）
```

### 常用 `--prop`

- 单元格：`value`、`formula`、`type`、`number_format`、`link`、`comment`
- 样式：`font.bold`、`font.italic`、`font.color`、`font.size`、`font.name`、`underline`、`strike`、`fill`、`halign`、`valign`、`wrapText`、`border.all`、`border.color`、`locked`
- 工作表：`name`、`tab_color`、`hidden`、`freeze`、`direction`、`auto_filter`
- 行：`height`；列：`width`、`hidden`
- 区域：`merge=true|false` + 上述样式属性（广播到区域内所有单元格）

### 典型工作流

```bash
# 从零生成
json2xlsx repack book/ -o book.xlsx
json2xlsx render book/ -o preview.html          # --format png 可截全片 PNG（需 Chrome）

# 改现有 xlsx（输出新文件，原文件不变）
json2xlsx edit in.xlsx '/sheet[1]/cell[A1]' set --prop value=新标题 -o out.xlsx

# 多次修改：先解包，原地改，最后重建
json2xlsx unpack in.xlsx -o prod/
json2xlsx edit prod/ '/sheet[1]/cell[A2]' set --prop value=Q1 --prop fill=EEEEEE
json2xlsx edit prod/ '/sheet[1]' add --type row --prop index=9
json2xlsx repack prod/ -o out.xlsx
```

## 模板提取与模板库（复用存量风格）

**场景**：用户上传一份 Excel，要求“照这个风格再生成别的表格”。

### 从用户文件提取风格

```bash
json2xlsx template extract 用户文件.xlsx -o my_tpl/
```

产出三件套：

- `my_tpl/template.json`：结构化风格模板（`palette` 配色、`fonts` 字体、`header` 表头样式与列名、`columns` 列宽、`number_formats` 数字格式）。
- `my_tpl/TEMPLATE.md`：面向大模型的可读说明（配色表、表头样式 JSON、生成步骤）。
- `my_tpl/skeleton/`：**产物目录骨架**——保留表头行与全部样式、清空数据；直接填数据后 `repack` 即得同风格文件。

`template extract` 会按配色/表头风格自动聚类，也内置了跨行业样本库：

### 内置模板库（`skill/templates/`）

从 **~2120 份真实 Excel**（SpreadsheetBench 评测集、ClosedXML、NPOI、Open‑XML‑SDK、pandas、LibreOffice、excelize、Microsoft 官方样例等）批量提取，
并用 **LibreOffice 渲染 + 视觉识别** 提取主色板，按风格聚类为 16 类：

| 模板 | 适用意图 | 表头/强调色 | 样本数 |
|------|----------|-------------|--------|
| `plain-report` | 通用报表 / 数据表（最常见） | 无底色 | 1442 |
| `accent-yellow` | 需高亮强调的清单 | 黄 `FFFF00` | 186 |
| `header-yellow` | 财务 / 醒目表头 | 黄底 `FFFF00` | 154 |
| `accent-red` | 风险 / 差异强调 | 红 `FF0000` | 113 |
| `accent-green` | 环保 / 健康 | 绿 `92D050` | 46 |
| `header-light` | 学术 / 浅色表头 | 白底 `FFFFFF` | 36 |
| `header-blue` | 商务 / 国企 / 汇报 | 深蓝 `004477` | 33 |
| `header-cyan` | IT / 技术 | 青底 `00FFFF` | 22 |
| `header-red` | 告警 | 红底 `FF0000` | 20 |
| `header-orange` | 营销 / 创意 | 橙底 `FFCC99` | 16 |
| `accent-blue` | 蓝系强调 | 蓝 `4472C4` | 15 |
| `header-gray` | 极简 / 高端 / 打印 | 黑底 `000000` | 11 |
| `header-green` | 自然 / 环保表头 | 绿底 `92D050` | 10 |
| `header-magenta` | 品牌 / 活动 | 品红 `D569B6` | 6 |
| `accent-cyan` / `accent-magenta` | 青系 / 品红系强调 | — | 4 / 4 |

索引见 [`templates/README.md`](templates/README.md) 与 [`templates/library.json`](templates/library.json)（含每个模板的样式色板、视觉主色、数字格式与样本数）。

### 复用流程

```bash
# 方式 A：用用户文件提取的风格
cp -r my_tpl/skeleton ./work/
# 方式 B：用库中风格接近的模板
cp -r skill/templates/header-blue/skeleton ./work/

# 编辑 work/xl/worksheets/sheetN.json 填业务数据（保留表头行样式）
json2xlsx repack work/ -o out.xlsx
json2xlsx render work/ -o preview.html   # 目视验收
```

> 规则：**不要另发明配色**，优先沿用 template 的 `palette` / `header.style`；表头样式可直接复制 `template.json` 的 `header.style` 到数据单元格。
>
> 若某模板缺 `skeleton/`（或需按 `template.json` 重建），执行：
> `python3 scripts/gen_template_skeleton.py skill/templates/<name>`（`--all` 遍历全部）。生成后应对每个 skeleton 跑一次 `repack` 确认可用。

## 报表最佳实践（生成后自查）

结合真实数据集生成报表时，务必逐条自查（否则 JSON 合法但外观/可用性会出问题）：

1. **中文/字体**：模板默认多为拉丁字体（Calibri）。含中文时给表头与数据设置 `font.name`（`微软雅黑` / `PingFang SC` / `等线`），或设置工作簿 `default_font`；否则某些环境（LibreOffice 无 CJK 回退）会显示空白。
2. **数字格式**：数值列必须显式设 `number_format` —— 金额 `#,##0`/`#,##0.00`、比率 `0.0%`、日期 `yyyy-mm-dd`；不要留裸浮点（会出现 `13.100000000000001%`）。
3. **对齐**：数值右对齐、文本左对齐、表头居中。
4. **强调/合计行**：加粗 + 顶部细边框；深色填充配浅色文字。
5. **列宽**：按内容显式设置（中文按约 2 字符宽估算），避免 `####` 或截断。
6. **图表**：放数据右侧/下方；需打印加 `print_area`；类别标签过长时用短标签（预览会自动截断）。
7. **验收**：`repack` 后用 `render` 目视核对（HTML 预览支持合并单元格/图表/条件格式）；LibreOffice headless 对中文可能无回退字体，优先用本工具预览核对。

> 各场景开箱配方（财务汇总 / 成绩表 / 统计公报 / IT 资产 / 项目跟踪）见 [`references/recipes.md`](references/recipes.md)。

> **专项场景技能**（结构化配方，命中场景优先照此执行）：
>
> - 财务模型/预算/盈利预测 → [`scenarios/financial-model.md`](scenarios/financial-model.md)
> - 经营看板/KPI 仪表盘/周报汇总 → [`scenarios/data-dashboard.md`](scenarios/data-dashboard.md)

## 格式保真（解析 / 回写）

- **已支持**：列/行默认样式（`<col style>` / `<row s>`）、主题色与 **tint（明暗）**、`indexed` 调色板、**主题部件（`theme1.xml`）保留**、`gray125` 图案填充、共享公式（含相对引用平移）、**公式求值（生成时对无缓存值的公式计算并写 `<v>`）**、省略的 `r` 引用、合并、条件格式（含 **dxf 差分填充**）、数据验证、结构化表格（含汇总行）、批注、超链接、图片、**图表（往返原样保留，含全部类型与样式）**、形状/连接线原样保留、**迷你图（sparkline）**、**透视表创建**（缓存+定义，`refreshOnLoad`）、**切片器创建**（绑定透视字段）、**图表工作表（chartsheet）**、**表单控件 VML（按钮等）**、**不透明部件保留（VBA、透视/切片缓存、customXml 等）**、**形状创建**、**OLE 嵌入对象创建**、**排序（sortState）**、**CSV/TSV 导入**、**raw 整部件读写**、**表格自动识别**、**图表坐标轴选项**、打印设置/区域（含打印居中与手动分页）；兼容 `\` 路径分隔符、部件名大小写、根级/非标准包、UTF-16（含无 BOM）部件。
- **降级/不支持**：**新建/修改**图表支持 column/bar/line/area/pie/doughnut/radar/scatter/bubble/stock/combo、3D 变体及扩展图表（waterfall/funnel/treemap/sunburst/histogram/pareto/boxWhisker），样式从简；公式求值覆盖约 345 个常用函数（含动态数组溢出、引用函数、LET/LAMBDA/MAP/REDUCE、分布/检验）（聚合/查找/文本/日期/统计/财务等），**不保证与 Excel 完全一致**（数组公式、易失函数、迭代求解不在此列）；数据表（what-if TABLE）公式；ActiveX 控件。关键文件请用 Excel 复核。

## 视觉回归（渲染比对）

改动解析/生成后，建议用渲染比对验证保真度（macOS）：

```bash
./scripts/collect_corpus.sh                 # 拉取 OSS 夹具 -> /tmp/xlsx_more
LIMIT=80 python3 scripts/visual_diff.py /tmp/xlsx_more       # QuickLook 渲染比对（快）
LIMIT=40 python3 scripts/visual_diff_lo.py tests/corpus      # LibreOffice 渲染比对（含图表、分页）
./scripts/render_subset.sh                  # CI 门禁：仅对「基线外新差异」失败
# 差异并排图在 /tmp/vlo/<名称>.p1_side.png，明细在 /tmp/vlo/results.tsv
```

- `visual_diff.py`：QuickLook，快；但**不渲染图表内容**（空框），适合单元格/图片/颜色/布局。
- `visual_diff_lo.py`：LibreOffice→PDF→PNG，**能渲染图表并可比较分页**，适合图表/打印保真验证。会自动区分「原始不可渲染(NOSRC)」「回环渲染失败(NOPDF)」，并**跳过含易失函数（RAND/NOW/OFFSET…）的文件**（LibreOffice 每次打开都重算，比对无意义）。
- `render_subset.sh`：对 `tests/corpus` 跑渲染比对，只在出现 **`tests/corpus/render_baseline.txt` 基线之外的新差异**或回环渲染失败时才失败，可作 CI 门禁。
- 结构回归：`./scripts/corpus_check.sh`（unpack→repack，统计失败）。

## 参考资源（按需加载）

| 文件 | 内容 | 何时读 |
|------|------|--------|
| [references/schema.json](references/schema.json) | 完整 JSON Schema（草案 2020-12） | 字段/枚举拿不准时 |
| [references/cli.md](references/cli.md) | CLI 命令全参考：参数、路径语法、组合用法 | 执行 unpack/edit/render 细节不确定时 |
| [templates/README.md](templates/README.md) · [templates/library.json](templates/library.json) | 内置样式模板库（16 类风格，含色板/表头/数字格式） | 需要按用户意图套用存量风格时 |
| [references/recipes.md](references/recipes.md) | 10 个场景配方（含 5 个真实数据集配方：flights/cars/gapminder/seattle-weather/college-majors） | 按场景快速生成时 |
| [scenarios/](scenarios/) | 专项场景技能（财务模型 / 数据看板） | 需求命中场景时优先 |
| `examples/out/reports/*.xlsx` | 已生成的真实报表样例（含图表，可直接打开核对） | 需要参考成品效果时 |
| `scripts/collect_datasets.sh` · `scripts/gen_dataset_reports.py` | 拉取真实数据集 / 一键生成数据报表 | 需要更多真实数据做示例或回归时 |
