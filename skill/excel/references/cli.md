# json2xlsx CLI 完整参考

`json2xlsx` 提供 14 个子命令：`unpack`（解包）、`repack`（重建）、`view`（视图）、`edit`（修改）、`render`（预览）、`import`（导入 CSV/TSV）、`validate`（结构校验）、`dump`（可回放指令）、`query`（选择器查询）、`raw` / `raw-set`（原始 OOXML 部件字节级读写）、`extract`（提取样式模板，旧写法 `template extract` 兼容）、`schema`、`templates`。CLI 采用**无状态**设计：命令间不共享隐藏状态，输入输出均由参数显式指定。

全局 flag：`--json` / `--quiet` / `--verbose` / `-o`；stdout 只输出数据，状态走 stderr；退出码 0（成功）/ 1（运行时错误）/ 2（用法错误）/ 3（产物有校验问题）。详见仓库根 `docs/cli-conventions.md`。

---

## 概览

| 命令 | 功能 |
|------|------|
| `unpack <输入.xlsx> -o <目录>` | xlsx → 可编辑产物目录 |
| `repack <产物目录> -o <输出.xlsx>` | 产物目录 → xlsx（从零生成的目录同样适用） |
| `view <输入> [路径] values\|structure` | 只读查看内容 / 结构 |
| `edit <输入> <路径> get\|set\|add\|remove [--prop k=v] [-o <输出.xlsx>]` | 精准修改 |
| `render <输入> [-o <输出.html>]` | 渲染为 HTML 预览 |
| `import <输入.csv\|tsv> [-o <输出.xlsx>] [--delim D] [--header] [--sheet 名]` | CSV/TSV → xlsx |
| `raw <输入.xlsx> <部件路径>` | 读取原始 OOXML 部件（打印到 stdout） |
| `raw-set <输入> <部件路径> --file <新内容或 `-`> [-o <输出.xlsx>]` | 整部件替换 |
| `template extract <输入.xlsx> [-o <目录>]` | 提取样式模板：`template.json` + `TEMPLATE.md` + `skeleton/` 产物目录 |
| `schema` | 打印 Schema 文档指引 |
| `templates` | 列出内置模板库（16 个风格模板） |

> `view` / `edit` / `render` 的 `<输入>` 可以是 `.xlsx`（一次性解包到临时目录，结束后自动清理）或产物目录（直接读写）。

## 运行方式

```bash
cargo run --release --bin json2xlsx -- <命令>      # 开发运行
cargo install --path cli && json2xlsx <命令>       # 全局安装
```

---

## 1. unpack

```
out/                                  # json2xlsx unpack in.xlsx -o out/
├── workbook.json                     # meta/styles/named_ranges + sheets 路径数组
└── xl/
    └── worksheets/
        ├── sheet1.json               # 每个工作表一个 JSON
        └── sheet2.json
```

| 参数 | 说明 |
|------|------|
| `input` | 输入 xlsx（必填） |
| `-o, --output` | 输出目录；缺省为输入文件名去扩展名（`book.xlsx` → `book/`） |

## 2. repack

`unpack` 的逆操作，也是 AI 从零生成 xlsx 的构建入口。

| 参数 | 说明 |
|------|------|
| `input` | 产物目录（含 `workbook.json`） |
| `-o, --output` | 输出 xlsx；缺省为目录名加 `.xlsx` |

## 3. view

```bash
json2xlsx view <输入> [路径] [values|structure]
```

- `values`（默认）：单元格 `ref/type/value/formula/number_format/display`
- `structure`：工作表名、标签色、隐藏、冻结、方向、视图（网格线/标题/缩放/公式）、筛选与筛选条件、列宽、合并区、行数、保护、数据验证、条件格式、表格、打印设置、批注数、图片/图表数量与明细

路径缺省 `/sheet[1]`。

## 4. 路径语法

以 `/` 开头，首段必须是 `sheet[...]`，索引从 1 开始，也支持表名。

| 路径 | 含义 |
|------|------|
| `/sheet[1]` | 第 1 个工作表 |
| `/sheet[Sheet1]` | 名为 Sheet1 的工作表 |
| `/sheet[1]/cell[A1]` | 单元格 |
| `/sheet[1]/range[A1:C10]` | 区域 |
| `/sheet[1]/row[2]` | 第 2 行 |
| `/sheet[1]/col[C]` 或 `col[3]` | 第 C 列 |
| `/sheet[1]/merge[1]` | 第 1 个合并区 |
| `/sheet[1]/chart[1]` / `image[1]` | 图表 / 图片（可在产物 JSON 中编写，`repack` 生成） |

## 5. edit

```bash
json2xlsx edit <输入> <路径> <get|set|add|remove> [--prop k=v]... [--type T] [-o <输出.xlsx>]
```

**语义**：产物目录 → 原地改写；`.xlsx` → 写操作必须 `-o`（不覆盖原文件），`get` 免 `-o`。

| 操作 | 说明 |
|------|------|
| `get` | 输出节点（工作表结构 / 单元格 / 行 / 区域）JSON |
| `set` | 修改属性（见下） |
| `add` | `--type sheet\|row\|cell` |
| `remove` | 删除工作表 / 行 / 单元格 / 合并区 |

### set 支持的属性

- 单元格：`value`、`formula`、`type`、`number_format`（别名 `format`/`numfmt`）、`link`、`link_tooltip`/`tooltip`、`comment`
- 样式：`font.bold`/`bold`、`font.italic`/`italic`、`font.color`/`color`、`font.size`/`size`、`font.name`/`font`、`underline`、`strike`、`fill`/`bgcolor`、`halign`、`valign`、`wrapText`、`border`/`border.all`、`border.color`、`border.top|bottom|left|right`、`locked`
- 工作表：`name`、`tab_color`、`hidden`、`freeze`、`direction`、`auto_filter`
- 行：`height`；列：`width`、`hidden`
- 区域：`merge=true|false`，以及上述样式属性（广播到区域内每个单元格）

### add

```bash
json2xlsx edit prod/ '/sheet[1]' add --type row --prop index=5
json2xlsx edit prod/ '/sheet[1]' add --type cell --prop ref=B2 --prop value=42
json2xlsx edit prod/ '/sheet[1]' add --type sheet --prop name=Summary
json2xlsx edit prod/ '/sheet[1]' add --type chart --prop type=column --prop range=B2:C8 --prop categories=A2:A8 --prop anchor=E2 --prop w=6 --prop h=4
json2xlsx edit prod/ '/sheet[1]' add --type image --prop src=assets/logo.png --prop anchor=E10 --prop w=2 --prop h=1
```

`add --type` 支持：`sheet` / `row` / `cell` / `chart` / `image`。图表类型见 `column`/`bar`/`line`/`area`/`pie`/`ring`/`scatter`；`range` 跨多列即多系列。

### 示例

```bash
# 一次性修改输出新文件
json2xlsx edit in.xlsx '/sheet[1]/cell[A1]' set --prop value=标题 --prop font.bold=true -o out.xlsx

# 产物目录多次原地修改
json2xlsx unpack in.xlsx -o prod/
json2xlsx edit prod/ '/sheet[1]/cell[B2]' set --prop formula=SUM(B3:B8) --prop number_format=#,##0.00
json2xlsx repack prod/ -o out.xlsx
```

## 6. render

> 预览保真范围：内容/数值/数字格式（分段、会计括号、货币、颜色修饰符）/合并单元格/图表（内联 SVG）/
> 图片与工作表背景（解析为绝对路径，PNG 与 HTML 均可见）/列宽（`sheet.columns` 字符宽度）与行高（磅）/
> 数字默认右对齐；**按一页宽度截断**（读取 `sheet.print` 的纸张/方向/缩放/边距，整列语义与打印一致，
> 超宽表只出第 1 页的列并在表格下方标注截断了多少列；需要全部列时调大纸张/缩放或直接看 xlsx）。
> 差异项：网格线为屏幕态（打印态通常不画）；`-o` 输出与数据无关的预览抬头。
（PNG 截图需本机 Chrome：`--format png -o preview.png`；缺失时回退建议 `--format html`）

把工作簿渲染为独立 HTML 表格（含字体/填充/对齐/边框的近似样式），用于 agent 目视验收。

```bash
json2xlsx render prod/ -o preview.html
json2xlsx render in.xlsx -o in.html
```

| 参数 | 说明 |
|------|------|
| `-o, --output` | 输出 HTML；缺省为输入名加 `.html` |

## 6.1 import（CSV/TSV → xlsx）

```bash
json2xlsx import data.csv -o data.xlsx                 # 自动推断分隔符
json2xlsx import data.tsv -o data.xlsx --delim '\t' --header --sheet Data
```

| 参数 | 说明 |
|------|------|
| `input` | 输入的 CSV/TSV 文本文件 |
| `-o, --output` | 输出 xlsx（**必填**，不会默认推导，避免覆盖已有文件） |
| `--delim` | 分隔符；缺省自动推断（`,` / `\t` / `;`） |
| `--header` | 首行作为表头 |
| `--sheet` | 工作表名 |

> 值会自动推断为数字/布尔/空；日期保持为字符串（如需日期类型请用产物目录显式设 `type`）。

## 6.2 raw / raw-set（原始 OOXML 部件）

用于读取或**整体替换**生成端未覆盖的部件（如 VBA、透视/切片缓存、customXml、chartsheet 依赖等）。

```bash
json2xlsx raw book.xlsx xl/workbook.xml > workbook.xml     # 读取
json2xlsx raw-set book.xlsx xl/vbaProject.bin --file vb.bin -o out.xlsx
echo '<xml/>' | json2xlsx raw-set book.xlsx customXml/item1.xml --file - -o out.xlsx
```

| 参数 | 说明 |
|------|------|
| `input` | `.xlsx` 或产物目录 |
| `part` | 包内部件路径（如 `xl/workbook.xml`） |
| `--file` | 新内容文件；`-` 表示从 stdin 读取（仅 raw-set） |
| `-o, --output` | 输出 xlsx（raw-set 必填；**输入文件永不被修改**，`-o` 与输入同路径会报错） |

## 6.3 extract（提取样式模板）

从存量 Excel 提取可复用的**样式模板**，供大模型据此生成同类风格的表格。
（旧写法 `template extract` 仍兼容。）

```bash
json2xlsx extract in.xlsx -o my_tpl/
```

| 参数 | 说明 |
|------|------|
| `input` | 输入 xlsx 或产物目录 |
| `-o, --output` | 输出目录；缺省为 `<输入名>_template/` |
| `--name` | 模板名（缺省取文件名） |

产出：

- `template.json`：`palette` 配色 / `fonts` 字体 / `header` 表头样式与列名 / `columns` 列宽 / `number_formats` 数字格式 / `default_font`。
- `TEMPLATE.md`：面向大模型的可读说明（配色表、表头样式 JSON、生成步骤）。
- `skeleton/`：**产物目录骨架**（保留表头行与全部样式、清空数据），直接填数据后 `repack` 即得同风格文件。

配套内置模板库见 `../templates/README.md`。

## 6.4 validate / dump / query

```bash
json2xlsx validate in.xlsx            # 结构校验：工作表名重复/非法引用/未定义样式/公式带 = 等
                                      # 有问题时 stdout 输出 issues 列表并退出码 3
json2xlsx dump book/                  # 导出可回放的编辑指令（batch JSON，与 ppt/docx 同构）
json2xlsx query in.xlsx sheets                       # 工作表摘要
json2xlsx query in.xlsx 'sheet[Report]'              # 按索引或名字取工作表 JSON
json2xlsx query in.xlsx 'cell:contains("合计")'       # 跨表文本/公式查找
```

## 7. schema / templates

```bash
json2xlsx schema            # 打印 JSON Schema 指引
json2xlsx schema --json     # 输出机器可读 JSON Schema（由 Rust 模型自动生成）
json2xlsx templates         # 列出内置模板库（16 套，读 library.json）
json2xlsx templates --json  # 结构化输出
```

---

## 12.1 serve / mcp — 常驻编辑与 MCP

```bash
json2xlsx serve prod/     # stdin 逐行 JSON：edit/view/query/batch/merge/issues/save/quit
json2xlsx mcp             # stdio JSON-RPC 2.0（MCP）：unpack/view/edit/merge/batch/render/help/...
```

## 13. batch / help — 指令回放与能力速查（三 CLI 统一）

### batch：回放 dump 风格指令

```bash
json2xlsx dump book.xlsx > cmds.json          # 或手写指令
json2xlsx batch book/ --input-file cmds.json  # 产物目录原地回放
json2xlsx batch book.xlsx --input-file c.json -o new.xlsx   # 文件输入必须 -o
```

- 默认**遇错即停**；`--force` 跳过错误继续；任一步失败退出码 1。
- 结果 JSON：`{"ok","applied","failed","steps":[{"index","op","path","ok","result"/"error","suggestion"}]}`。
- dump → batch 是保真闭环：对同一文档回放 dump 输出，语义不变（同名 add sheet / 按 ref 的 add cell 均幂等）。

### help：能力速查（不确定属性时先查，不要猜）

```bash
json2xlsx help            # 概览：元素 + add 类型 + view 模式 + batch op
json2xlsx help cell       # 单元素全部可 set/add 属性（含示例）
json2xlsx help cell --json  # 机器可读能力 schema（与实现同源）
```

### 未知输入的自愈建议

未知属性/类型/取值/越界错误一律带 `建议:`（--json 时 `error.suggestion`）：最近匹配（`是否想用 "bold"？`）+ 合法取值/范围。

## 常见组合

| 场景 | 命令 |
|------|------|
| 从零生成 | 写产物目录 → `repack` → `render` 验收 |
| 查看他人表格 | `unpack in.xlsx -o prod/` → `view prod/ /sheet[1] values` / `structure` |
| 一次性改并输出新文件 | `edit in.xlsx /sheet[1]/cell[A1] set --prop value=x -o out.xlsx` |
| 深度编辑 | `unpack` → 直接改 `xl/worksheets/*.json` → `repack` |
| 回环一致性 | `unpack a.xlsx -o p/ && repack p/ -o b.xlsx` |

## 相关文档

- JSON 输入格式 → 本目录 `../SKILL.md`、`schema.json`
- 架构与扩展 → 仓库 `README.md`、`src/`
