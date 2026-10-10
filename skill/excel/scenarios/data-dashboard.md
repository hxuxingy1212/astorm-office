# 场景配方：数据看板（data-dashboard）

单表 KPI + 图表 + 明细的"一屏看板"。接到"做个仪表盘/经营看板/周报汇总"时按本配方组织。

## 布局约定（单工作表）

| 区域 | 行 | 内容 |
|------|-----|------|
| 标题行 | 1 | 合并单元格 `range[A1:H1] merge=true`，大字号标题 |
| KPI 卡片区 | 3–5 | 每个 KPI 两行：标签行（灰字小号）+ 数值行（大号加粗、千分位） |
| 图表区 | 7–20 | `add --type chart`（趋势用 line，构成用 pie/column） |
| 明细区 | 22+ | 原始明细，首行表头冻结，`auto_filter` 开启 |

## 生成要点

1. **KPI 数值**：引用明细区聚合（`SUMIFS/COUNTIFS/AVERAGE`），不写死：
   `--prop formula=SUMIFS(明细!D:D,明细!B:B,"华东") number_format=#,##0`
2. **图表**：
   ```bash
   json2xlsx edit book/ '/sheet[1]' add --type chart \
     --prop chart_type=line --prop data_range=明细!B2:D13 --prop title=月度趋势
   ```
3. **条件格式/边框**：KPI 数值行 `border.bottom=medium`；明细区隔行 `fill=F7F9FC`。
4. **列宽**：明细列按内容 `set '/sheet[1]/col[C]' width=14`；KPI 列 18。
5. **看板交付**：需要截图给汇报时用 PNG 渲染（需本机 Chrome）：
   ```bash
   json2xlsx render book/ --format png -o dashboard.png
   ```
   无 Chrome 时回退 `--format html` 交付。
6. **数据更新**：换数据不改结构时，把明细写好后 `batch` 重算 KPI 公式引用（公式是活的不需要重算，只有写死值才需要）。

## 校验

交付前 `validate`（公式引用、缺失图片、空表头），再 `view '/sheet[1]' layout` 确认图表锚定区域不与明细重叠。
