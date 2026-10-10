# json2docx 命令参考

CLI 采用**无状态**设计：命令之间不共享任何隐藏状态，输入输出由参数显式指定。
15 个子命令：`unpack` / `repack` / `view` / `edit` / `render` / `validate` / `dump` / `raw` / `raw-set` / `schema` / `templates` / `extract` / `merge` / `mcp` / `serve`。
全局 flag：`--json` / `--quiet` / `--verbose` / `-o`；stdout 只输出数据，状态走 stderr；退出码 0/1/2/3（3 = 产物有校验问题）。详见仓库根目录 `docs/cli-conventions.md`。

```bash
cargo run --release --bin json2docx -- <命令>
# 或 cargo install --path cli 后直接 json2docx <命令>
```

---

## 1. unpack — DOCX 解包为产物目录

```bash
json2docx unpack <input.docx> [-o <目录>]
```

```
out/
├── document.json           # meta/page/theme/styles/template + parts 文件路径数组
└── word/
    ├── parts/
    │   ├── part1.json      # 按一级标题分片，每片含 blocks[]
    │   └── part2.json
    └── media/image1.png
```

- 缺省输出目录 = 输入文件名去扩展名。
- 图片 `src` 统一为 `word/media/xxx.ext`（相对产物根）；URL 原样保留。

---

## 2. repack — 产物目录重建 DOCX

```bash
json2docx repack <目录> [-o <output.docx>]
```

- 读取 `document.json` 与各 `word/parts/partN.json`，生成 DOCX。
- **也支持内联**：`document.json` 的 `parts` 可直接内联 Part 对象数组（`[{ "blocks": [...] }]`），此时无需 parts 文件。
- 缺省输出 = 目录名 + `.docx`。
- 生成前自动合并 `template`（academic-paper/report/letter/resume/memo）。

---

## 3. view — 只读视图

```bash
json2docx view <输入> /part[N] text|layout
```

输入可为 `.docx`（一次性解包到临时目录，结束自动删除）或产物目录。

- `text`：每块 `{type, text}` + `media[]`。
- `layout`：块结构树（heading/paragraph/list/table/image/formula/…）。

```bash
json2docx view thesis.docx /part[1] layout
```

---

## 4. edit — 精准修改

> 段落稳定寻址：`/part[1]/paragraph[@paraId=XXXX]`（w14:paraId，unpack 自原始 docx 保留、repack 回写，插删段落不漂移）。

```bash
json2docx edit <输入> <路径> get|set|add|remove [--prop k=v]... [--type T] [-o <输出.docx>]
```

**语义**
- 输入为**产物目录**：原地修改，之后 `repack`。
- 输入为 **.docx**：写操作（set/add/remove）必须带 `-o`，一次性 unpack→改→repack，不回写原文件；`get` 无需 `-o`。

**路径**：`/part[N]/type[M]`，1 起、按类型计数。

| 操作 | 示例 |
|------|------|
| get | `edit prod/ /part[1]/paragraph[2] get` |
| set（段落） | `edit prod/ /part[1]/paragraph[2] set --prop text=新正文 --prop align=justify` |
| set（标题） | `edit prod/ /part[1]/heading[1] set --prop text=新标题 --prop level=1` |
| set（分片） | `edit prod/ /part[1] set --prop header="第 1 章" --prop footer_page_number=true` |
| add | `edit prod/ /part[1] add --type formula --prop latex="E=mc^2" --prop display=true` |
| add（指定位置） | `edit prod/ /part[1] add --type quote --prop text=… --prop index=2` |
| remove | `edit prod/ /part[1]/quote[1] remove` |
| 一次性输出 | `edit in.docx /part[1]/heading[1] set --prop text=新标题 -o out.docx` |

**add `--type` 支持**：`heading` `paragraph` `list` `table` `image` `formula` `code` `quote` `caption` `toc` `bibliography` `chart` `textbox` `shape` `attachment` `page_break`。

**常用 `--prop`**
- heading：`text` `level`
- paragraph：`text` `style` `align` `bold` `italic` `font_size` `color` `font_family` `first_line_indent` `indent` `hanging_indent` `line_spacing` `space_before` `space_after`
- image：`src` `width` `height` `alt` `caption` `align`
- formula：`latex` `display` `number`
- 分片：`header` `footer_page_number` `page_size` `orientation`

值支持单位限定串：`12pt` / `0.5cm` / `1.5x` / `150%`。

---

## 5. render — 渲染预览

```bash
json2docx render <输入> /part[N] [-o <out.html|pdf|png>] [--format auto|html|pdf|png]
json2docx render <输入> /             # 整篇渲染（所有分片）
```

输出 HTML 预览（便于 render→look→fix）；`.pdf` / `.png` 经 LibreOffice 或无头 Chrome 导出。
缺省 `<输入名>_part[N].html`（整篇为 `<输入名>_all.html`）。

---

## 6. extract — 提取风格样式模板

```bash
json2docx extract <输入.docx> [-o <目录>]
```

从一份 Word 文档提取页面/字体/段落样式，产出：

```
tpl/
├── template.json    # page / theme / styles（可直接合并进新文档的 document.json）
└── TEMPLATE.md      # 面向大模型的说明：字体、字号、颜色、行距、缩进、对齐、用法
```

- 会解析 `styles.xml` 的样式继承链（`basedOn` + `docDefaults`），并结合文档中段落/run 的实际格式聚合。
- 缺省输出目录 `<文件名>_template/`。
- 典型用法：用户上传样张 → `extract` → 把 `page`/`theme`/`styles` 合并进新文档 → `repack`。
- 存量模板见 [../templates/INDEX.md](../templates/INDEX.md)。

## 7. validate — 结构校验

```bash
json2docx validate a.docx        # 或产物目录；有问题时 stdout 输出 issues 并退出码 3
```

## 8. dump — 导出可回放编辑指令

```bash
json2docx dump paper/            # {"version":1,"commands":[...]}，可回放 set/add
```

## 9. raw / raw-set — 原始 OPC 部件读写

```bash
json2docx raw a.docx word/document.xml > doc.xml            # 读取（字节原样）
json2docx raw-set a.docx word/document.xml --file doc.xml -o b.docx   # -o 必填，不覆盖原文件
echo '<x/>' | json2docx raw-set a.docx customXml/i1.xml --file - -o b.docx
```

## 10. merge / serve / mcp

```bash
json2docx merge tpl.docx -d '{"name":"张三"}' -o out.docx   # {{key}} 数据填充
json2docx serve a.docx                                      # 常驻：stdin 逐行 JSON 操作
json2docx mcp                                               # MCP stdio JSON-RPC 服务
```

## 11. schema / 12. templates

```bash
json2docx schema        # 打印 JSON 字段指引；--json 输出机器可读 JSON Schema
json2docx templates     # 列出 5 套内置模板；--json 输出结构化
```

---

## 常见组合

| 场景 | 命令 |
|------|------|
| 从零写论文 | 写 `paper/document.json` + parts → `repack paper/ -o paper.docx` |
| 单文件起步 | `document.json` 内联 parts → `repack dir/ -o a.docx` |
| 看结构 | `view a.docx /part[1] layout` |
| 逐章微调 | `unpack` → 多次 `edit prod/ …` → `repack` |
| 改他人文档（不改原件） | `edit a.docx /part[1]/paragraph[1] set --prop text=… -o b.docx` |
| 渲染核对 | `render a.docx /part[1] -o p1.html` |

## 能力边界

- `view` / `edit` 支持 `/part[N]/type[M]`，并支持容器下钻：
  - 表格：`/part[N]/table[1]/row[2]/cell[1]/paragraph[1]`
  - 列表：`/part[N]/list[1]/item[2]/paragraph[1]`
- 公式 `latex` 会转换为真正的 OMML（上下标、`\frac`、`\sqrt`、`\sum`/`\int`、希腊字母、常用符号）。
- 图/表题注未手动编号时自动插入 live `SEQ` 域（Word 打开自动重算）；已以「图/表/Figure/Table」开头则原样保留。
- 支持分节与多栏：在分片 `section.page.columns` 指定栏数；显式声明 `section` 的分片会生成分节符，分节类型用 `section.section_type`（`continuous`/`nextPage`/`evenPage`/`oddPage`）。页面边框用 `section.page.page_border`（`{style,color,width}`，pt）。
- 支持逐节差异化页眉/页脚：各分片 `section.header`（文本）或 `section.header_blocks`/`footer_blocks`（块，保留多段/对齐/表格格式）生成独立部件，`footer_page_number` 控制页码；`section.page_number_start` 设置页码起始，`section.page_number_format` 设置页码格式（decimal/upperRoman/lowerLetter…）。
- 表格支持合并单元格（`colspan`/`rowspan`）与列宽 `widths`；显式宽度 `width`（pt，`tblW dxa`）、宽度百分比 `width_pct`（`tblW type=pct`）、布局 `layout`（`fixed`/`autofit`）；表样式 `style`、整体边框 `border`（`style`/`color`/`width`；`style:"none"` 无边框）、缩进 `indent`；行 `height`+`height_rule`（`exact`/`atLeast`）；单元格 `valign`（`top`/`center`/`bottom`）、`fill`、`border`、`margin`（pt）。
- 列表编号保真：支持 `w:lvlOverride/startOverride`；列表项 `num_format`（`decimal`/`lowerLetter`/`upperLetter`/`lowerRoman`/`upperRoman`/`bullet`）、`lvl_text`（如 `%1)`、`•`）、`start`、`num_font`；非默认时生成独立 `numbering.xml` 定义，保持符号/编号样式。
- 表单域：`run.form_field`（`kind`: text/checkbox/dropdown，`name`/`checked`/`default`/`items`），生成 `w:ffData`。
- 同段多图：无文本段落中的 VML 图与 DrawingML 图会分别输出为独立 `image` 块；VML 图片填充（`v:fill r:id`）也识别为图片。
- 脚本字体槽：`run.font_family`/`east_asia_font`/`cs_font`（w:rFonts ascii/eastAsia/cs）；表格 `table.rtl`（`w:bidiVisual`）。
- i18n/RTL：`run.lang`/`run.lang_ea`（BCP-47）、`run.rtl`、`paragraph.rtl`（`w:bidi`）、`section.page.rtl`/`rtl_gutter`。
- 通用域：`run.field`（如 `IF ...`、`DOCPROPERTY Title`、`STYLEREF 1`、`MERGEFIELD name`、`DATE`）保留指令 + 缓存结果，生成五段式域。
- 内容控件（SDT）：`sdt` 块（`sdt_type`/`alias`/`tag`/`placeholder`/`lock`/`items` + `blocks`）；行内控件属性落到 `run.sdt`。
- SmartArt（`dgm:`）：保留原始 drawing XML + 关系并透传 `word/diagrams/*`，**原样渲染**（`raw` 块）；其 `raw.diagram.texts` 为**可编辑**文本，修改后回写 diagram 数据部件。
- 未建模部件透传：`document.json` 的 `passthrough`（路径/内容类型/Base64）原样保留 diagrams、customXml、字体、glossary 等，repack 时写回。
- 表单结构（Blocks）内联：`run.footnote` / `run.endnote` 生成脚注/尾注；`run.bookmark` + `run.ref_target`（`pageref`）生成交叉引用域；`run.comment` 生成批注；`run.revision`（`ins`/`del`）生成修订。
- 行内文本属性：`run.bold`/`italic`/`underline`/`strike`/`color`/`highlight`/`shading`/`font_family`/`font_size`/`superscript`/`subscript`；`run.caps`（全大写）、`run.small_caps`（小型大写）、`run.spacing`（字间距 pt）、`run.symbol`（符号 `Font:Char`，如 `Wingdings:F0B7`，经 `w:sym` 保真）。
- 目录支持 live `TOC` 域或静态目录（JSON 中 `{"type":"toc","static":true}`）。
- `render` 输出 HTML；`-o *.pdf` / `*.png` 经 LibreOffice 或无头 Chrome 导出，本地图片自动内嵌为 base64 data URI（相对产物根解析）。
- 图片支持内联与浮动：`image.wrap`（`square`/`tight`/`topAndBottom`/`none`）+ `x`/`y`（pt）+ `behind_text`；裁剪 `image.crop`（[左,上,右,下] 百分比）与旋转 `image.rotation`（度）。
- MCP 服务：`json2docx mcp`（stdio JSON-RPC，工具：unpack/repack/view/validate/merge/edit/render/schema）。
- 常驻编辑：`json2docx serve <输入>`（stdin 逐行 JSON 操作，内存保持文档）。
- 数据填充：`merge <输入> -d '{"k":"v"}' -o out.docx`，替换段落/表格/形状/页眉页脚中的 `{{key}}`（支持 `a.b` 嵌套）。
- 超链接：`run.hyperlink` 以 `#` 开头视为文档内书签锚点（`w:hyperlink w:anchor`），否则按外部关系生成。
- 原生图表：`chart` 块（`chart_type` + `categories` + `series{name,values,color}`）生成 `chartN.xml` 并可回环；支持 `legend`（b/t/l/r/none）、`x_title`/`y_title`、`y_min`/`y_max`、`data_labels`、`grouping`（clustered/stacked/percentStacked）；系列数值/类别兼容 `numRef/strRef` 缓存。
- 文本框/形状：`textbox` 块（`fill`/`line`/`font_size`/`color`，内联或浮动）；`shape` 块（`shape_type` 预设几何 + 可选 `text`/`fill`/`line`/`rotation`/`wrap`+`x`/`y`）。
- 水印：顶层 `watermark`；页面背景色：顶层 `background`（6 位 hex，自动写入 `displayBackgroundShape`）；段落扩展：`keep_next`/`keep_lines`/`page_break_before`/`shading`/`border`（下边框）/`border_box`（四边盒式）/`tabs`/`drop_cap`（首字下沉行数）；运行 `shading`。
- 运行注音 `ruby`（`text` 为基底、`ruby` 为注音）；文字效果 `outline`/`shadow`/`emboss`/`imprint`；艺术字（WordArt `v:textpath`）文本提取为 `shape.text`。
- 页脚 `footer_format: "page_of"`（第 X 页 / 共 Y 页）。
- 保真边界（已知损失）：SmartArt/图表的**布局视觉**（文本已保留）、`dgm:`/ActiveX/墨迹等**未建模 part 不随包透传**（repack 从 JSON 重建，未知部件与其关系会被丢弃）；`.doc` 旧格式不支持。

## 13. batch / help — 指令回放与能力速查（三 CLI 统一）

### batch：回放 dump 风格指令

```bash
json2docx dump book.xlsx > cmds.json          # 或手写指令
json2docx batch book/ --input-file cmds.json  # 产物目录原地回放
json2docx batch book.xlsx --input-file c.json -o new.xlsx   # 文件输入必须 -o
```

- 默认**遇错即停**；`--force` 跳过错误继续；任一步失败退出码 1。
- 结果 JSON：`{"ok","applied","failed","steps":[{"index","op","path","ok","result"/"error","suggestion"}]}`。
- dump → batch 是保真闭环：对同一文档回放 dump 输出，语义不变（同名 add sheet / 按 ref 的 add cell 均幂等）。

### help：能力速查（不确定属性时先查，不要猜）

```bash
json2docx help            # 概览：元素 + add 类型 + view 模式 + batch op
json2docx help cell       # 单元素全部可 set/add 属性（含示例）
json2docx help cell --json  # 机器可读能力 schema（与实现同源）
```

### 未知输入的自愈建议

未知属性/类型/取值/越界错误一律带 `建议:`（--json 时 `error.suggestion`）：最近匹配（`是否想用 "bold"？`）+ 合法取值/范围。
