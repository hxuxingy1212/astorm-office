# 场景配方（Recipes）

面向大模型的“按用户意图快速生成”配方。每个配方给出数据来源、表结构、数字格式、对齐、推荐模板与图表，可直接照抄字段名与设置。

> 通用规则（务必遵守）：
> - 含中文时给表头/数据设置 `font.name`（`微软雅黑` / `PingFang SC` / `等线`）或工作簿 `default_font`。
> - 数字列显式 `number_format`：金额 `#,##0`/`#,##0.00`、比率 `0.0%`、日期 `yyyy-mm-dd`。
> - 对齐：数值右、文本左、表头居中；合计/强调行加粗+顶部边框。
> - 列宽按内容显式设置；图表放数据右侧/下方；需要打印时设 `print_area`。
> - 生成后 `repack` + `render` 目视核对。

---

## 1. 财务 / 销售汇总（financial summary）

- 数据来源示例：`Microsoft Financial Sample`（字段 `Segment, Country, Product, Units Sold, Gross Sales, Profit, Date, Year ...`）。
- 结构：标题行（合并 A1:D1）+ 表头 + 数据 + 合计行。
- 列与格式：`国家`(文本左) / `销售收入`(`#,##0` 右) / `利润`(`#,##0` 右) / `利润率`(`0.0%` 右)。
- 模板：`header-blue`；图表：`column`，`data_range` 取收入列、`categories` 取国家列，`show_values: true`、`legend: "none"`。
- 产物片段：

```json
{
  "name": "销售汇总",
  "freeze": "A3",
  "columns": [ { "width": 26 }, { "width": 14 }, { "width": 14 }, { "width": 10 } ],
  "merges": ["A1:D1"],
  "auto_filter": "A2:D7",
  "charts": [
    { "type": "column", "data_range": "B3:B7", "categories": "A3:A7",
      "title": "各国销售收入", "legend": "none", "show_values": true,
      "colors": ["4472C4"], "anchor": "F2", "size": { "w": 8, "h": 5 } }
  ],
  "rows": [
    { "index": 1, "cells": [ { "ref": "A1", "value": "全球销售汇总", "style": { "font": { "bold": true, "size": 14, "name": "微软雅黑" }, "alignment": { "horizontal": "center" } } } ] },
    { "index": 2, "cells": [
      { "ref": "A2", "value": "国家", "style": "header-blue" },
      { "ref": "B2", "value": "销售收入", "style": "header-blue" },
      { "ref": "C2", "value": "利润", "style": "header-blue" },
      { "ref": "D2", "value": "利润率", "style": "header-blue" } ] },
    { "index": 3, "cells": [
      { "ref": "A3", "value": "United States of America" },
      { "ref": "B3", "value": 27269358, "number_format": "#,##0" },
      { "ref": "C3", "value": 2995541, "number_format": "#,##0" },
      { "ref": "D3", "value": 0.1099, "number_format": "0.0%" } ] }
  ]
}
```

---

## 2. 学术成绩表（gradebook）

- 数据：学生名单 + 各次成绩；计算总评与等级（`=ROUND(AVERAGE(C2:F2),1)`、`=IF(G2>=90,"A",...)`）。
- 列与格式：`学号`(文本) / `姓名`(文本) / 各项成绩(`0`) / `总评`(`0.0`) / `等级`(文本居中)。
- 模板：`header-green`；可加条件格式 `colorScale` 于成绩区。

```json
{
  "name": "成绩表",
  "freeze": "A3",
  "columns": [ { "width": 10 }, { "width": 10 }, { "width": 8 }, { "width": 8 }, { "width": 8 }, { "width": 8 }, { "width": 8 }, { "width": 8 } ],
  "merges": ["A1:H1"],
  "conditional_formats": [ { "range": "C3:F8", "type": "colorScale", "colors": ["63BE7B", "FFEB84", "F8696B"] } ],
  "rows": [
    { "index": 1, "cells": [ { "ref": "A1", "value": "2026 春季学期 成绩表", "style": { "font": { "bold": true, "size": 14, "name": "微软雅黑" }, "alignment": { "horizontal": "center" } } } ] },
    { "index": 2, "cells": [
      { "ref": "A2", "value": "学号", "style": "header-green" }, { "ref": "B2", "value": "姓名", "style": "header-green" },
      { "ref": "C2", "value": "作业1", "style": "header-green" }, { "ref": "D2", "value": "作业2", "style": "header-green" },
      { "ref": "E2", "value": "期中", "style": "header-green" }, { "ref": "F2", "value": "期末", "style": "header-green" },
      { "ref": "G2", "value": "总评", "style": "header-green" }, { "ref": "H2", "value": "等级", "style": "header-green" } ] },
    { "index": 3, "cells": [
      { "ref": "A3", "value": "S001" }, { "ref": "B3", "value": "张伟" },
      { "ref": "C3", "value": 88 }, { "ref": "D3", "value": 92 }, { "ref": "E3", "value": 85 }, { "ref": "F3", "value": 90 },
      { "ref": "G3", "formula": "ROUND(AVERAGE(C3:F3),1)", "number_format": "0.0" },
      { "ref": "H3", "formula": "IF(G3>=90,\"A\",IF(G3>=80,\"B\",IF(G3>=70,\"C\",\"D\")))", "style": { "alignment": { "horizontal": "center" } } } ] }
  ]
}
```

---

## 3. 政府 / 统计公报式数据表（gov statistics）

- 数据：按年/地区汇总的指标；同比用公式或预计算。
- 列与格式：`年份`(文本居中) / `指标1`(`#,##0` 右) / `指标2`(`#,##0` 右) / `同比`(`0.0%`)。
- 模板：`header-gray`（黑底白字）或 `header-blue`；表头加粗、数据区简洁无斑马纹。

---

## 4. IT 资产清单（IT inventory）

- 列：`资产编号` / `主机名` / `责任人` / `操作系统` / `CPU` / `内存(GB)` / `采购日期`(`yyyy-mm-dd`) / `保修到期`(`yyyy-mm-dd`) / `状态`。
- 模板：`header-cyan`；`auto_filter` 开启；日期列显式 `yyyy-mm-dd`；`内存(GB)` 右对齐。
- 可选：条件格式高亮 `保修到期` 临近的日期（`expression` 规则）。

---

## 5. 项目进度跟踪（project tracker）

- 列：`任务` / `负责人` / `开始`(`yyyy-mm-dd`) / `结束`(`yyyy-mm-dd`) / `进度`(`0%`) / `状态` / `预算(元)`(`#,##0`)。
- 模板：`header-orange`；进度列可用 `dataBar` 条件格式。

```json
{ "range": "E3:E7", "type": "dataBar", "color": "638EC6" }
```

---

## 6~10. 真实世界数据集配方

数据获取：`scripts/collect_datasets.sh` → `/tmp/datasets_raw/`（来源：seaborn-data、vega-datasets、FiveThirtyEight、UCI、World Bank）。
示例生成：`scripts/gen_dataset_reports.py` → `examples/out/reports/ds_*.xlsx`（含图表，可直接打开核对）。

| 场景 | 数据集 | 结构要点 | 图表 | 产物 |
| --- | --- | --- | --- | --- |
| 航空客运量趋势 | seaborn `flights` | 年份×月份矩阵 + 合计列 | `line` | `ds_flights_trend.xlsx` |
| 汽车性能 | vega `cars` | 马力排序 Top 15 | `bar` | `ds_cars_horsepower.xlsx` |
| 各国预期寿命 | vega `gapminder` | 最新年份 Top 20 | `bar` | `ds_gapminder_life.xlsx` |
| 西雅图天气 | vega `seattle-weather` | 按月聚合（均温/总降水/晴天数） | `line` | `ds_seattle_weather.xlsx` |
| 本科专业起薪 | 538 `college-majors` | 中位起薪 Top 12 | `bar` | `ds_college_majors.xlsx` |

可复用的其它真实数据集（同为 `/tmp/datasets_raw`）：
`tips`、`titanic`、`iris`/`uci-iris`、`diamonds`、`penguins`、`planets`、`fmri`、
`stocks`、`sp500`、`movies`、`population`、`weather`、`airline-safety`、`uci-wine`、
`wb-gdp`/`wb-pop`/`wb-life`（World Bank JSON）。

## 交付前自查清单

1. 中文是否设置了中文字体（`font.name` / `default_font`）。
2. 每个数值列是否设置了正确的 `number_format`（金额/比率/日期）。
3. 表头是否居中、数值是否右对齐、合计行是否加粗。
4. 列宽是否足够（中文按 2 字符估算），无 `####` 或截断。
5. 图表是否放在数据旁、类别标签是否过long（可缩短）。
6. 是否需要 `print_area`（避免多页）。
7. 是否已 `repack` + `render` 目视核对，无空表头、无裸浮点。
