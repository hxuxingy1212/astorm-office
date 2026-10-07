# json2xlsx (ai-excel)

**JSON ↔ XLSX 双向转换库与 CLI — 面向 AI Agent 的 Excel 生成工具**

通过直接操作 OOXML（Office Open XML），实现 JSON 与 Excel 工作簿的双向转换，零 MS Office / LibreOffice 运行时依赖。设计对齐 [ai-ppt / json2pptx](https://github.com)，Excel 字段与语义参考 OfficeCLI。

## 特性

| 类别 | 支持 |
|---|---|
| 方向 | JSON → XLSX 生成、XLSX → JSON 解析、roundtrip 回环 |
| 值 | 字符串 / 数字 / 布尔 / 日期 / 错误；共享字符串去重 |
| 公式 | 写入公式；**生成时对无缓存值的公式求值并写缓存**（约 345 个函数，含动态数组溢出、引用函数、LET/LAMBDA 高阶函数：SUM/AVERAGE/COUNT/SUMIFS/COUNTIFS/SUMPRODUCT/SUBTOTAL、IF/IFS/SWITCH/AND/OR/XOR、VLOOKUP/HLOOKUP/XLOOKUP/INDEX/MATCH、MID/FIND/SUBSTITUTE/TEXTJOIN、ROUND/CEILING/MOD/GCD/COMBIN、RANK/PERCENTILE/CORREL/SLOPE、DATE/EDATE/EOMONTH/NETWORKDAYS、NPV/PMT/IRR/RATE/IPMT/DDB 等），并置 `fullCalcOnLoad` |
| 样式 | 字体（粗体/斜体/删除线/下划线/字号/颜色/字体名）、填充、边框、对齐（水平/垂直/换行/缩进/旋转）、数字格式、锁定、列/行默认样式、主题色解析、gray125 图案 |
| 布局 | 列宽/隐藏、行高、合并单元格、冻结窗格、自动筛选、标签色、隐藏表、RTL |
| 富内容 | 图片（本地/远程）、**形状（几何/填充/渐变/旋转/翻转/文本）**、图表（column/bar/line/area/pie/doughnut/radar/scatter/bubble/stock/combo、3D、扩展 waterfall/funnel/treemap/sunburst/histogram/pareto/boxWhisker；图例/数据标签/**坐标轴 min/max/可见/网格线/数字格式**/系列色）、**迷你图**、**透视表**、**切片器**、**表格自动识别（detected table）**、**OLE 嵌入对象** |
| 校验/格式 | 数据验证、条件格式（cellIs/expression/colorScale/dataBar/containsText/notContains/beginsWith/endsWith/top10/duplicateValues/uniqueValues/aboveAverage/containsBlanks|Errors/timePeriod/iconSet） |
| 表格 | 结构化表格（ListObject）：表头、样式、斑马纹、列名 |
| 筛选/分组 | 自动筛选、**排序状态（sortState）**、行/列分组、视图选项（网格线/标题/缩放/公式显示） |
| 批注 | 单元格批注（legacy note）：`commentsN.xml` + VML |
| 页面 | 工作表保护、打印设置（方向/缩放/适应页宽高/纸张）、打印区域与重复标题 |
| 链接/富文本 | 单元格超链接（外部 URL / 内部锚点 / 悬浮提示）、单元格内多 run 富文本（粗体/斜体/删除线/下划线/上下标/颜色/字号/字体） |
| 模板 | `template extract`：从存量 Excel 提取风格模板（配色/字体/表头/列宽/数字格式）+ Markdown 说明 + 可填数骨架；内置 16 类风格模板库 |
| 结构 | 多工作表、命名区域、工作簿元数据、1900/1904 日期系统 |
| CLI | `unpack` / `repack` / `view` / `edit` / `render` / `import`(CSV/TSV) / `raw`/`raw-set`(整部件) / `schema` / `templates`（无状态） |
| 预览 | 渲染为 HTML（agent 目视验收） |

> 已具备：数据透视表创建、切片器创建、迷你图、公式求值；复杂图表样式与公式覆盖范围见「边界」。

## 架构

```
JSON (serde model) --generate--> OOXML 各 part --zip--> .xlsx
                                                          │
JSON (serde model) <--parse----- quick-xml 解析 <--unzip--/
```

单一数据模型 `Workbook` 作为枢轴，`generate` 与 `parse` 互为逆运算。核心库 `json2xlsx` + CLI `json2xlsx`。

## 快速开始

```bash
# 构建（astorm-office monorepo 根目录；单项目也可 cd ai-excel）
cargo build -p json2xlsx -p json2xlsx-cli --release

# 生成物 → 产物目录
cargo run -p json2xlsx-cli -- unpack book.xlsx -o prod/

# 产物目录 → xlsx
cargo run -p json2xlsx-cli -- repack prod/ -o book.xlsx
```

## Agent 用法（JSON 优先）

Agent 只写结构化的**产物目录**（`workbook.json` + `xl/worksheets/sheetN.json`），再用 CLI 构建并验证：

```bash
json2xlsx repack deck/ -o out.xlsx          # 构建
json2xlsx view deck/ /sheet[1] values       # 读内容
json2xlsx edit deck/ '/sheet[1]/cell[A1]' set --prop value=标题 --prop font.bold=true
json2xlsx render deck/ -o preview.html      # 目视验收
```

字段规范见 [`skill/SKILL.md`](skill/SKILL.md) 与 [`skill/references/schema.json`](skill/references/schema.json)。

## CLI 参考

| 命令 | 功能 |
|---|---|
| `unpack <in.xlsx> [-o DIR]` | xlsx → 产物目录（workbook.json + 每个工作表一个 JSON + `xl/media/`） |
| `repack <DIR> [-o out.xlsx]` | 产物目录 → xlsx（AI 从零生成也用它） |
| `view <输入> [路径] [text\|layout]` | 只读查看内容 / 结构（旧值 values/structure 兼容） |
| `edit <输入> <路径> get\|set\|add\|remove [--prop k=v] [-o out.xlsx]` | 精准修改 |
| `render <输入> [-o out.html]` | 生成 HTML 预览 |
| `import <in.csv\|tsv> -o out.xlsx [--delim D] [--header] [--sheet 名]` | CSV/TSV 导入为 xlsx |
| `validate <输入>` | 结构校验，问题非空时退出码 3 |
| `dump <输入>` | 导出可回放的编辑指令（batch JSON） |
| `query <输入> <选择器>` | `sheets` / `sheet[N]` / `sheet[name=X]` / `sheet:empty` / `cell:contains("文本")` |
| `raw <输入> <部件>` / `raw-set <输入> <部件> --file F -o <out.xlsx>` | 原始 OOXML 部件字节级读写（`raw-set` 不覆盖原文件） |
| `extract <in.xlsx> [-o DIR] [--name X]` | 提取样式模板（template.json + TEMPLATE.md + skeleton/）；旧写法 `template extract` 兼容 |
| `schema [--json]` / `templates [--json]` | Schema 指引 / 内置模板（`--json` 输出机器可读） |

全局 flag（三个 CLI 一致）：`--json`（写盘命令输出结果 JSON、状态与错误结构化为 JSON 行）、`--quiet`、`--verbose`、`-o/--output`。stdout 只输出数据，状态/进度走 stderr；退出码 `0` 成功、`1` 运行时错误、`2` 用法错误、`3` 产物有校验问题。详见仓库根目录 `docs/cli-conventions.md`。

**路径语法**（首段 `sheet`，索引从 1 开始）：

```text
/sheet[1]                    工作表
/sheet[1]/cell[A1]           单元格
/sheet[1]/range[A1:C10]      区域（set 广播样式 / merge）
/sheet[1]/row[2]             行
/sheet[1]/col[C]             列（也支持 col[3]）
```

`edit` 语义：输入为**产物目录**则原地修改；输入为 **.xlsx** 时写操作必须给 `-o`（临时解包，不覆盖原文件），`get` 免 `-o`。

## 测试

```bash
cargo test --workspace
```

覆盖：model↔OOXML roundtrip、产物目录 unpack/repack、CLI 端到端（spawn 二进制）。
另有**真实语料回归**：`tests/corpus/` 收录约 20 份来自 PHPOffice/calamine/umya/exceljs/poiji 的
精简 `.xlsx` 边界样本（UTF-16 部件、`\` 路径分隔符、根级包、1904 日期系统等），
见 [`tests/corpus/SOURCES.md`](tests/corpus/SOURCES.md)。

## 语料与数据集

大规模测试语料与真实数据集不入库，用脚本按需拉取到 `/tmp`：

```bash
./scripts/collect_corpus.sh            # 拉取 OSS .xlsx 测试夹具 -> /tmp/xlsx_more
./scripts/corpus_check.sh /tmp/xlsx_more   # 逐份 unpack→repack，统计失败
LIMIT=80 python3 scripts/visual_diff.py /tmp/xlsx_more  # QuickLook 渲染比对
LIMIT=40 python3 scripts/visual_diff_lo.py tests/corpus # LibreOffice 渲染比对（含图表/分页）
./scripts/collect_datasets.sh          # 拉取真实数据集 -> /tmp/datasets_raw
python3 scripts/gen_dataset_reports.py # 用真实数据集生成报表 -> examples/out/reports/ds_*.xlsx
./scripts/gen_reports.sh               # 生成 5 个场景报表 -> examples/out/reports/*.xlsx
```

## 边界（当前版本）

- 公式求值覆盖常见函数（SUM/AVERAGE/IF/VLOOKUP 等），**不保证与 Excel 完全一致**（数组公式/易失函数/迭代求解不覆盖）；仍置 `fullCalcOnLoad` 交由 Excel 重算。
- 数据透视表、切片器：忽略 / 只读。
- 图表支持基础类型；复杂图表格式在解析回环时降级。
- 主题色解析为 RGB 并计算 **tint（明暗）**；支持 `indexed` 调色板；数据表（what-if TABLE）公式降级为缓存值。
- 图片/图表保留单元格内偏移与尺寸；**形状/连接线（`sp`/`cxnSp`）原样保留**；保留主题部件（`theme1.xml`）。**图表往返原样保留**（含各类型与完整样式）；仅当结构化字段被修改时才重新生成（支持 column/bar/line/area/pie/ring/scatter，样式从简）。
- **不透明部件原样保留**（VBA `vbaProject.bin`、透视/切片缓存、`customXml`、`calcChain` 等）：回写时不丢失，但工具不解析/编辑它们。**图表工作表（chartsheet）**与**表单控件 VML（按钮）**原样保留。
- 加密文件（OLE/CFB，含密码保护的 xlsx）不支持，会给出明确错误提示。
- 命名样式在解析回环后会内联到单元格（`workbook.styles` 不被还原）。

## 目录结构

```
src/           核心库（model / generate / parse / product / utils）
cli/           CLI 二进制
skill/         Agent 技能包（SKILL.md + references/）
examples/      demo.rs（生成示例）、read.rs（解析打印 JSON）
tests/         roundtrip / product 集成测试
```

## License

MIT
