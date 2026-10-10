# 场景配方：融资路演（pitch-deck）

10–12 页投资路演结构。接到"做个 BP/路演 PPT/融资演示"时按本配方组织，风格库见 [`../references/design-styles.md`](../references/design-styles.md)。

## 页面结构

| # | 页面 | 关键元素 |
|---|------|----------|
| 1 | 封面 | 一句话定位 + Logo/日期；大标题 40pt+ |
| 2 | 问题 | 2–3 个痛点卡片（shape + text），配数据 |
| 3 | 解决方案 | 产品截图（image）+ 3 个价值点 |
| 4 | 市场 | TAM/SAM/SOM 三层（嵌套 shape 或数据表） |
| 5 | 产品 | 功能演示图 + 核心指标（kpi_card 组件） |
| 6 | 商业模式 | 收入结构图（pie_chart 组件）+ 定价 |
| 7 | 竞争 | 2×2 定位矩阵（line + shape 画轴）或对比表（table） |
| 8 | 团队 | 头像 image + 一行履历 |
| 9 | 财务与里程碑 | line_chart 增长曲线 + 里程碑时间线 |
| 10 | 融资计划 | 本轮金额/用途/里程碑（progress_bar 组件） |

## 生成要点

1. **风格先行**：选一套设计风格统一全片（深色科技选 dark 系，商务选 light 系），背景/主色/字体全部走该风格，不要逐页混搭。
2. **图表组件**：数据图优先用内置组件（line_chart/pie_chart/ring_chart/kpi_card），真实 chart 仅在需要 Excel 级数据编辑时用。
3. **多步构建用 batch**：整套 deck 一次写好后，微调用 batch 而不是逐条 edit：
   ```bash
   json2pptx dump deck/ > current.json        # 备份当前状态
   json2pptx batch deck/ --input-file fix.json --force
   ```
4. **稳定寻址**：编辑时用 `@name=` / `@id=` 定位（`add` 会回显分配的 `id`），删除/插入元素后索引会漂移但 id 不变：
   ```bash
   json2pptx edit deck/ '/slide[3]/text[@name=Slogan]' set --prop text=新口号
   ```
5. **视觉闭环**：每页 `render '/slide[N]'` 截图检查溢出/对比度；不确定属性先 `json2pptx help <元素> --json`。
6. **数据填充**：同一模板换项目数据时，文字写 `{{key}}` 占位符，用 `merge --data '{"company":"ACME"}'` 批量填充。
