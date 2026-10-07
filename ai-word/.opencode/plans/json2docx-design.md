# ai-world（json2docx）— 技术方案 v3

**JSON ↔ DOCX（Word）双向转换 Rust 库 + CLI，供 Agent 生成与编辑专业长文档（论文、报告等）**

> 立项目录：`ai-world`。
> **定位**：完全沿用 **ai-ppt / json2pptx** 的 **JSON 优先 + 产物目录 + 无状态 CLI** 架构与命令行形态；
> 把 PPTX 换成 DOCX，把"页/元素"换成"章/块"。
> **OfficeCLI** 只作为**具体实现细节**的参考（OOXML 处理、样式继承、域、编号、公式、学术排版、常见坑、渲染核对），不改变 ai-ppt 的 CLI 与 JSON 优先路线。

---

## 1. 项目目标

1. **JSON → DOCX**：根据 JSON 描述生成完整 `.docx`。
2. **DOCX → JSON**：解析现有 `.docx` 为 JSON（双向回环）。
3. **产物目录**：与 ai-ppt 同构的可编辑中间产物（`document.json` + 按章分片的 `word/parts/*.json` + `word/media/`）。
4. **无状态 CLI**：命令之间不共享隐藏状态，输入输出全部由参数显式指定。
5. **AI 友好**：JSON 简洁线性、无 OOXML 噪音，适合 LLM 直接撰写长文。
6. **面向专业文档**：标题层级、自动编号、TOC、页眉页脚/页码、图/表/公式题注、交叉引用、参考文献（含 GB/T 7714）、中英文字体、分节/多栏。
7. **零 Office 依赖**：OOXML 全部手工生成/解析；预览/导出走内建渲染或可选外部引擎。

非目标（首期）：修订/批注完整往返、艺术字/DrawingML 图形可编辑往返、Word 排版级分页复刻。

---

## 2. 架构总览（JSON 优先，对齐 ai-ppt）

```
┌──────────────┐      ┌───────────────┐      ┌─────────────┐
│  JSON 描述    │ ──▶  │  OOXML 部件   │ ──▶  │  DOCX 文件  │
│  (serde)     │ gen  │  (String)     │ zip  │  (.zip/OPC) │
└──────────────┘      └───────────────┘      └─────────────┘
       ▲                                             │
       └────────────────── parse ────────────────────┘
```

- **生成**：JSON → Rust 模型（serde）→ WordprocessingML 字符串（quick-xml）→ OPC ZIP → `.docx`
- **解析**：`.docx` → ZIP 解压 → XML 流式解析（quick-xml Reader）→ Rust 模型 → JSON

核心设计原则（继承 ai-ppt）：

1. **JSON 是契约**：LLM 生成/编辑的是 JSON；`.docx` 只是 JSON 的一种落地格式。
2. **单一数据模型**：生成与解析共享 `model`，保证 JSON → DOCX → JSON 回环。
3. **产物目录**：`presentation.json` → 换成 `document.json`；`ppt/slides/*.json` → 换成 `word/parts/*.json`。
4. **无状态 CLI**：`view`/`edit`/`render` 可直接操作产物目录，或对 `.docx` 一次性解包到临时目录（结束自动删除）。
5. **零 Office 依赖**：手工 OOXML。

> 与 ai-ppt 唯一的架构差异：**文档是线性流（块顺序 + 样式），不是分页画布（绝对坐标）**。因此 JSON 不设 `position`，改以 **块（block）+ 行内（run）+ 样式（style）** 建模。

---

## 3. 与 ai-ppt 的逐项映射

| ai-ppt（PPTX） | ai-world（DOCX） |
|----------------|------------------|
| `Presentation`（width/height/meta/theme/slides） | `Document`（meta/page/theme/styles/template/parts） |
| `Slide`（background/transition/elements/notes） | `Part`（section 覆盖 / blocks / header / footer） |
| `Element` 枚举（text/shape/image/table/group…） | `Block` 枚举（heading/paragraph/list/table/image/formula/toc…） |
| 元素含 `position{x,y,w,h}`（英寸） | 块为线性流，**无 position**；用段落属性（缩进/间距/对齐） |
| 单位：英寸 / pt / 度 | 单位：**pt**（长度/字号）、mm 或命名纸张（页面）、1.5x（行距） |
| 绝对定位画布 | 流式排版（由 Word 引擎分页） |
| 顶层分片：`ppt/slides/slideN.json` | 顶层分片：`word/parts/partN.json`（按章） |
| 路径：`/slide[N]/type[M]` | 路径：`/part[N]/type[M]`（+ 容器下钻） |
| `view text\|layout` | `view text\|layout`（同两模式） |
| `edit get/set/add/remove` | `edit get/set/add/remove`（同四操作） |
| `render` 幻灯片 → PNG | `render` 文档 → PNG/PDF（逐页或整篇） |
| 7 子命令 `unpack/repack/view/edit/render/schema/templates` | **完全相同** |
| `skill/SKILL.md` + `references/` | 同结构，增加 `academic-paper` 场景层 |

---

## 4. 依赖选择

| 用途 | Crate | 理由 |
|------|-------|------|
| OPC/ZIP | `zip` 2.x | 与 ai-ppt 一致 |
| XML 生成/解析 | `quick-xml` 0.36+ | 流式、宽容模式 |
| JSON | `serde` + `serde_json` | 标准 |
| 图片 | `image` | 像素/DPI → wp:extent |
| 远程图片 | `ureq` 2 | URL 图片（同 ai-ppt） |
| CLI | `clap` 4.x | 子命令 |
| 渲染预览 | 内建 HTML/native + 无头浏览器或 `soffice` | 核对排版 |
| 公式 | 自研 LaTeX↔OMML（白名单子集） | 论文能力 |
| schema 文档 | 静态 JSON（`include_str!`） | `schema` 命令输出 |
| 单元换算 | 自研 `utils/units.rs` | pt/twip/half-point/EMU 精确整数比 |

**不直接用现成 docx crate**：生成强、回环/可编辑产物弱。沿用 ai-ppt 自研 OOXML 路线，`docx-rs` 仅作写法参考。

---

## 5. DOCX（WordprocessingML）包结构与单位

### 5.1 包结构

```
[Content_Types].xml
_rels/.rels
docProps/core.xml, app.xml
word/
  document.xml            # w:body → w:p / w:tbl / w:sectPr
  _rels/document.xml.rels # 图片/超链接/页眉页脚/脚注关系
  styles.xml              # docDefaults + 段落/字符样式（Heading1..9、Title、Caption…）
  numbering.xml           # abstractNum + num（列表/自动编号）
  settings.xml            # updateFields、compat、默认字号
  fontTable.xml
  theme/theme1.xml
  webSettings.xml
  header1.xml / footer1.xml (+rels)
  footnotes.xml / endnotes.xml
  comments.xml
  media/imageN.*
```

### 5.2 正文关键节点

| 节点 | 作用 |
|------|------|
| `w:p` / `w:pPr` | 段落 / 段落属性（`pStyle`、`jc`、`ind`、`spacing`、`numPr`、`pBdr`、`pageBreakBefore`、`keepNext`） |
| `w:r` / `w:rPr` | 运行 / 运行属性（`b/i/u/strike`、`color`、`sz`(半磅)、`rFonts`(ascii/hAnsi/eastAsia/cs)、`highlight`、`vertAlign`、`shd`） |
| `w:t` / `w:br` / `w:tab` | 文本 / 换行 / 制表符 |
| `w:hyperlink` | 超链接（`r:id` 或 `w:anchor`） |
| `w:tbl/tr/tc` | 表格（`gridSpan`、`vMerge`、`tcW`） |
| `w:sectPr` | 节：页面尺寸/页边距/栏数/页眉页脚引用 |
| `w:fldChar`+`w:instrText` | 域：TOC / PAGE / NUMPAGES / SEQ / PAGEREF / REF |
| `m:oMath` / `m:oMathPara` | OMML 公式（行内 / 独立块） |

### 5.3 单位（集中在 `utils/units.rs`）

| 关系 | 换算 |
|------|------|
| 1 pt = 20 twip | 位置/页边距/缩进/间距 |
| 1 pt = 12700 EMU | DrawingML/图片 |
| 1 pt = 2 half-point | 字号（`w:sz`） |
| 边框 1 pt = 8 eighth-point | 边框 `w:sz` |
| 行距：1 倍 = `w:line=240 lineRule=auto` | 倍数 ×240 |
| A4 = 11906×16838 twip；Letter = 12240×15840 twip | 1in = 1440 twip |

**JSON 单位约定（对齐 ai-ppt 的"数值 + 固定单位"风格）**：长度与字号一律 **pt**（如 `font_size: 12`、`first_line_indent: 24`、`space_after: 6`）；页面用命名 `"A4"|"Letter"` 或 mm；行距用倍数（`1.5`）。CLI 的 `--prop` 额外接受单位限定串（`12pt`/`0.5cm`/`1.5x`）以提升 Agent 体验，解析层归一为 pt。

---

## 6. JSON 数据模型

### 6.1 顶层 `document.json`（对应 ai-ppt 的 `presentation.json`）

```json
{
  "meta": { "title": "…", "author": "…", "subject": "计算机科学", "keywords": ["大模型", "检索"] },
  "page": {
    "size": "A4",
    "orientation": "portrait",
    "margins": { "top": 72, "bottom": 72, "left": 90, "right": 90, "header": 42, "footer": 42 }
  },
  "theme": {
    "major_font": "Times New Roman", "minor_font": "Times New Roman",
    "east_asia_font": "宋体", "heading_font": "黑体",
    "colors": { "heading": "1F3864", "link": "0563C1" }
  },
  "styles": {
    "Normal": { "font_size": 12, "line_spacing": 1.5, "first_line_indent": 24, "align": "justify" }
  },
  "template": "academic-paper",
  "parts": ["word/parts/00-front-matter.json", "word/parts/01-chapter1.json", "word/parts/99-references.json"]
}
```

- `meta/page/theme/styles` 均可省略，由 `template` 派生。
- `parts` 为产物内相对路径数组，按序拼接为正文（分片见 §7）。

### 6.2 分片 `word/parts/partN.json`（对应 ai-ppt 的 `slideN.json`）

```json
{
  "section": { "page": { "size": "A4" }, "header": "第 1 章 绪论", "footer_page_number": true },
  "blocks": [ /* Block[] */ ]
}
```

### 6.3 Block 类型（serde `tag = "type"`，同 ai-ppt `Element` 手法）

```jsonc
{ "type": "heading",   "level": 1, "text": "第一章 绪论", "numbering": true }
{ "type": "paragraph", "text": "本文提出……", "style": "Normal", "align": "justify",
  "first_line_indent": 24, "line_spacing": 1.5 }
{ "type": "paragraph", "runs": [
    { "text": "关键词：", "bold": true },
    { "text": "多模态；检索", "color": "333333" },
    { "text": "官网", "hyperlink": "https://example.com" } ] }
{ "type": "list", "ordered": false, "items": [
    { "blocks": [ { "type": "paragraph", "runs": [ {"text": "要点一"} ] } ] } ] }
{ "type": "table", "header_row": true, "widths": [2,3,3],
  "rows": [ { "cells": [ { "blocks": [ {"type":"paragraph","text":"方法","align":"center","bold":true} ] } ] } ],
  "caption": "表 1  实验对比" }
{ "type": "image", "src": "word/media/fig1.png", "width": 300, "align": "center",
  "caption": "图 1  系统架构", "alt": "系统架构图" }
{ "type": "formula", "latex": "E=mc^2", "display": true, "number": "(1)" }
{ "type": "code", "lang": "python", "text": "def f():\n    pass" }
{ "type": "quote", "text": "引用的段落" }
{ "type": "toc", "title": "目录", "levels": [1,2,3] }
{ "type": "bibliography", "style": "GB/T 7714", "entries": [
    { "type": "article", "authors": ["张三"], "title": "…", "journal": "…", "year": 2024 } ] }
{ "type": "page_break" }
```

**Run 字段**：`text`（必填）、`bold`、`italic`、`underline`、`strike`、`color`、`highlight`、`font_family`、`font_size`、`superscript`/`subscript`、`code`、`hyperlink`。

### 6.4 可选：解析时保留的 OOXML 细节（借鉴 OfficeCLI，不破坏 JSON 简洁性）

为提升回环保真度，解析输出可在节点上附**可选**字段（用户/LLM 通常忽略）：

- `para_id`：段落稳定 ID（`w:pPr/w:p/@w14:paraId`），用于稳定寻址；
- `effective`：只读解析结果（如 `{ "size": 12, "size_src": "/styles/Normal" }`），供 `view`/`get` 呈现继承后的真实格式；
- `raw`：无法归一化的 OOXML 片段（域缓存、修订、DrawingML 图形等），生成时原样回填。

> 这三项是"细节参考 OfficeCLI"的地方：JSON 主结构保持 ai-ppt 的简洁，细节靠可选字段承载。

---

## 7. 产物目录与分片

与 ai-ppt 同构，按**章/部分**分片（对应 ai-ppt 按页分片）：

```text
paper/                              # aiworld-cli unpack paper.docx -o paper/
├── document.json                   # meta/page/theme/styles/template + parts[] 文件路径
└── word/
    ├── parts/
    │   ├── 00-front-matter.json    # 封面/摘要/关键词/目录
    │   ├── 01-chapter1.json
    │   └── 99-references.json
    └── media/
        └── image1.png              # 图片实体（src 相对产物根或 URL）
```

- `document.json` 的 `parts` 为**文件路径数组**（同 ai-ppt 的 `slides`）。
- 图片 `src` 用产物根下相对路径（`word/media/x.png` 或自放 `assets/x.png`）或 URL；`repack` 解析为绝对路径打包。
- 分片默认按一级标题（`chapter`）；可选 `section`（按 Word 节）或 `single`（整篇一个文件）。分片只影响布局，不影响语义。

---

## 8. CLI 设计（严格对齐 ai-ppt 的 7 子命令）

二进制 `aiworld-cli`，库 crate `json2docx`。**无状态**：命令间不共享隐藏状态。

| 命令 | 别名 | 功能 |
|------|------|------|
| `aiworld unpack <输入.docx> -o <目录>` | — | DOCX → 产物目录（document.json + word/parts/*.json + word/media/） |
| `aiworld repack <产物目录> -o <输出.docx>` | — | 产物目录 → DOCX（含从零新建的目录） |
| `aiworld view <输入> /part[N] text\|layout` | — | 只读视图：文本+媒体 / 块结构树 |
| `aiworld edit <输入> <路径> get\|set\|add\|remove [-o <输出.docx>]` | — | 精准修改（产物原地改；.docx 写操作需 `-o`） |
| `aiworld render <输入> /part[N] [-o <out.png\|pdf>]` | — | 渲染预览（内建/无头浏览器/soffice） |
| `aiworld schema` | — | 打印 JSON Schema 文档指引 |
| `aiworld templates` | — | 列出文档模板预设（academic-paper / report / letter / resume / memo） |

> `view`/`edit`/`render` 的 `<输入>` 可以是 `.docx`（一次性解包到临时目录，命令结束自动删除）或产物目录（直接读写）。

### 8.1 运行方式（同 ai-ppt）

```bash
cargo run --release --bin aiworld-cli -- unpack paper.docx -o paper/
cargo install --path cli
aiworld-cli unpack paper.docx -o paper/
```

### 8.2 典型工作流

```bash
# 从零生成
aiworld repack paper/ -o paper.docx            # paper/ 由 Agent 直接产出
aiworld render paper.docx /part[1] -o p1.png   # 渲染核对

# 逐章原地微调（产物目录 + edit，无需反复 repack）
aiworld edit paper/ /chapter[1]/paragraph[3] set --prop text="修改后的正文"
aiworld edit paper/ /chapter[1] add --type formula --prop latex="a^2+b^2=c^2" --prop display=true
aiworld repack paper/ -o paper.docx

# 修改他人文档
aiworld unpack thesis.docx -o prod/
aiworld view prod/ /part[1] layout             # 先摸清结构
aiworld edit prod/ /chapter[2]/heading[1] set --prop text="新标题"
aiworld repack prod/ -o fixed.docx

# 一次性修改并输出新文件（原文件不变）
aiworld edit thesis.docx /chapter[1]/paragraph[2] set --prop text="新正文" -o new.docx
```

### 8.3 edit 语义（同 ai-ppt）

- 输入为**产物目录**：原地改对应 `parts/*.json`，之后 `repack`。
- 输入为 **.docx**：写操作（set/add/remove）必须带 `-o <输出.docx>`，一次性 unpack → 改 → repack，**不回写原文件**；只读 `get` 无需 `-o`。

### 8.4 与 OfficeCLI 命令的取舍

明确**不引入** OfficeCLI 的 `create/get/query/set(顶层)/batch/dump/validate/open/close/help/export` 等命令——它们属于 DOM 优先路线，会偏离"JSON 优先 + ai-ppt CLI"。
其中确有价值的能力改用**内部实现**或**归入既有命令**：

| OfficeCLI 能力 | ai-world 落点 |
|----------------|---------------|
| `validate` | `repack`/解析时内部校验；`schema`/skill 说明 |
| `batch` | `edit` 可多次调用（产物原地改），无需单独命令 |
| `dump`（可重放 batch） | 即 `unpack` 产物 JSON（JSON 优先下天然等价） |
| `help <el> --json` | 归入 `schema` 命令与 `skill/references/schema.json` |
| `create/new` | 直接产出产物目录（同 ai-ppt：Agent 落盘即可），不设命令 |
| `export/render` | 统一为 `render` |
| `open/close`（resident） | 首期不做；性能优化留待后续（不改变 CLI 形态） |

---

## 9. 路径语法与元素（ai-ppt 语法，容器名换成文档结构）

路径以 `/` 开头，每段 `类型[索引]`，索引 **1-based**，按类型独立计数（沿用 ai-ppt `path.rs` 规则）。**`/slide[N]` → `/part[N]`**（文档分片，别名 `/chapter[N]`）。

```text
/part[1]                         # 第 1 个分片（章）
/part[2]/heading[1]              # 第 2 章第 1 个标题
/part[1]/paragraph[3]            # 第 1 片第 3 段
/part[2]/table[1]/row[3]/cell[1] # 容器下钻（表格→行→格）
/part[1]/list[1]/item[2]         # 列表下钻
/part[2]/figure[1]/caption       # 图题
```

- 中间节点只能是容器类型（`table`/`list`/`figure` 等），非容器继续下钻报错（同 ai-ppt）。
- 可选稳定 ID 段：`/part[1]/paragraph[@para_id=1A2B]`（解析自 OOXML，见 §6.4）；索引漂移时更稳。
- 错误信息与 ai-ppt 对齐（越界/类型不存在/索引从 1 开始/非容器不可下钻）。

**可用块类型**（`edit add --type` 支持）：`heading` `paragraph` `list` `table` `image` `formula` `code` `quote` `toc` `bibliography` `caption` `page_break`。

---

## 10. 生成管线（JSON → DOCX）

1. **模板合并**：`template` 与 `theme/styles/page` 合并（用户显式优先）。
2. **校验**：至少一个块；引用样式已定义；`add`/`set` 的属性名合法。
3. **收集资源**：`image` 本地路径/URL，读像素与 DPI。
4. **编号池**：预分配 `abstractNumId/numId`（标题自动编号、列表、图表 SEQ 序号）。
5. **生成部件**：
   - `styles.xml`：`docDefaults` + 段落样式（Normal/Heading1-9/Title/Subtitle/Quote/Code/Caption/Reference/TOC）+ 字符样式（Hyperlink/CodeChar/Strong）；
   - `numbering.xml`：多级列表；
   - `document.xml`：按 `parts` 顺序输出 `w:body`，末 `w:sectPr`；
   - `settings.xml`：`<w:updateFields w:val="true"/>`（位置须在 `<w:compat>` 之前）；
   - `header*/footer*.xml`：页眉、页脚、页码域（PAGE/NUMPAGES）；
   - `footnotes.xml`/`endnotes.xml`；
   - 公式：LaTeX→OMML（`m:oMathPara`/`m:oMath`）；
   - `theme1.xml`/`fontTable.xml`/`content_types.xml`/`rels`/`docProps`。
6. **OPC 打包**（zip 顺序：`[Content_Types].xml` 最前）。

关键片段（生成/解析共用）：

```xml
<!-- 段落：样式 + 编号 + 间距 + 缩进 -->
<w:p><w:pPr>
  <w:pStyle w:val="Heading1"/>
  <w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr>
  <w:spacing w:before="240" w:after="240" w:line="360" w:lineRule="auto"/>
  <w:ind w:firstLine="480"/><w:jc w:val="both"/>
</w:pPr>
<w:r><w:rPr>
  <w:rFonts w:ascii="Times New Roman" w:eastAsia="宋体" w:hAnsi="Times New Roman"/>
  <w:b/><w:color w:val="1F3864"/><w:sz w:val="36"/>
</w:rPr><w:t xml:space="preserve">第一章 绪论</w:t></w:r></w:p>

<!-- 域五段式（PAGE / TOC / SEQ / PAGEREF 同构） -->
<w:r><w:fldChar w:fldCharType="begin"/></w:r>
<w:r><w:instrText xml:space="preserve"> PAGE </w:instrText></w:r>
<w:r><w:fldChar w:fldCharType="separate"/></w:r>
<w:r><w:t>1</w:t></w:r>
<w:r><w:fldChar w:fldCharType="end"/></w:r>
```

---

## 11. 解析管线（DOCX → JSON）

1. 解压（跳过 `customXml/` 与未知部件）。
2. 元信息：`core.xml`→meta；首/末 `sectPr`→page；`theme1.xml`/`styles.xml`→theme/styles。
3. **样式继承展开**（OfficeCLI 细节参考）：构建 `styleId→Style`，沿 `basedOn` 链与 `docDefaults` 计算有效属性；用于 `view`/`get` 呈现与可选 `effective` 字段。
4. **遍历 `w:body`**：
   - `pStyle` → heading/paragraph/quote/code/caption/toc；
   - `numPr` → `list`（`ilvl` 嵌套、`numId` 有序/无序）；
   - `w:r` → run（`br`→`\n`、`tab`→`\t`、hyperlink 读 rels）；
   - `w:tbl` → table（`gridSpan`/`vMerge`/`tcW`）；
   - `fldChar/instrText` → 域（TOC/PAGE/SEQ/PAGEREF）；
   - `m:oMath` → formula（OMML→LaTeX 或 `raw`）。
5. **资源提取**：`word/media/*` → 产物 `word/media/`，`src` 改写为相对路径。
6. **归一化**：合并相邻同 `rPr` 的 run；未知/复杂内容入 `raw`。
7. **分片**：按 §7 规则写 `parts/*.json` + `document.json`。

**回环保真边界**：

- 可回环：段落/run 格式、标题层级、列表、表格、图片、题注、结构化公式、页眉页脚文本、页码开关、页面设置、样式定义。
- 近似/归一化：复杂域缓存值、修订、批注、艺术字/DrawingML 图形 → `raw` 或降级。
- 不保证：Word 引擎分页位置、断字、行内对象精确布局。

---

## 12. 从 OfficeCLI 借鉴的实现细节（不改 JSON/CLI 路线）

这些是"具体实现参考"，用于提高质量与保真度：

| 主题 | 借鉴点 |
|------|--------|
| 样式继承 | 回环最大难点。解析沿 `docDefaults→basedOn→直接格式`，对外可选暴露 `effective.*` 与来源指针 |
| 编号系统 | `abstractNum/num/ilvl/numId` 预分配；标题自动编号建议**手写前缀**（`numId` 跨 Heading 复用脆弱） |
| 域字段 | 五段式 `fldChar/instrText/separate/result/end`；生成写占位 + `updateFields` |
| SEQ 缓存陷阱 | 生成的多 SEQ 域默认缓存同值；需专门写入递增缓存值（OfficeCLI `recalcFields=seq` 等价做法） |
| 公式子集 | `\left(...\right)`+上下标会 crash、`\mathcal` 产出非法 OMML → 白名单 + 降级 |
| 单位解析 | `--prop` 接受 `12pt/0.5cm/1.5x`，内部归一为 pt（宽进严出） |
| 稳定 ID | 段落 `@paraId`、图形 `@id`、书签 `@name`；JSON 可选携带，编辑优先稳定 ID |
| 渲染核对 | 生成后渲染 HTML/截图/PDF 逐页核对（render→look→fix）——这是 ai-ppt `render` 的文档版 |
| 校验 | 生成/打包时内部做 OOXML 结构校验（相当于 OfficeCLI `validate`，但不新增命令） |
| 常见坑 | 见 §13.4 |

---

## 13. 论文场景专项（模板 `academic-paper`）

结构骨架：封面 → 摘要/Abstract → 关键词 → 目录 → 正文各章 → 结论 → 参考文献 → 致谢 → 附录。

### 13.1 引用格式（中英双轨）

| 格式 | 正文形态 | 参考表排序 | 适用 |
|------|----------|-----------|------|
| GB/T 7714（中文论文默认） | 顺序编码 `[1]` / 著者-出版年 | 引用顺序 | 中文期刊/学位论文 |
| APA 7 | `(Smith, 2024)` | 作者字母序 | 社科 |
| Chicago 17 Notes-Bib | 上标脚注 | 作者字母序 | 人文 |
| IEEE | `[1]` | 首次引用顺序 | 工程 |
| MLA 9 | `(Smith 412)` | 作者字母序 | 文学 |

- 参考文献段一律**悬挂缩进**：`indent=720 hanging_indent=720`（**不要** `first_line_indent=-720`）。
- 引用标记一律 live 域（User 引用锚点），避免硬编码。

### 13.2 公式（OMML）

- 行内 `m:oMath`；独立块 `m:oMathPara`。
- **规则**：正文中一切数学变量/希腊字母/上下标都走公式元素，禁止以纯文本写 `lambda_1`/`x_{t+1}`。
- 公式编号：右对齐独立段 `(1)`，或 `SEQ` 域自动编号。

### 13.3 图/表/交叉引用与版式

- **题注位置铁律**：**图题在图下方，表题在表上方**（APA/Chicago/IEEE/MLA/GB 一致）。
- `SEQ Figure` / `SEQ Table` 自动计数 + `PAGEREF` 交叉引用（注意缓存值陷阱）。
- 图片必须 `alt`；`view`/解析可检查缺失。
- 字体：正文小四/Times 12pt；标题分级（H1≥18pt 粗、H2 14pt 粗、H3 12pt 粗斜）。
- 行距 1.5×/2×；页边距 1in；摘要块式（无首行缩进）；关键词斜体。
- IEEE 双栏：`section type=continuous columns=2`；正文后**必须**再加分节 `columns=1` 恢复单栏；摘要单栏。

### 13.4 常见坑（来自 OfficeCLI 实战）

| 坑 | 正确做法 |
|----|----------|
| 页面分页不触发 | `pageBreakBefore` 与独立 `break` **双保险**（不同查看器失灵点相反） |
| 分节后索引漂移 | 每次 `add section` 会插入一个空段落，后续 `p[N]` +1，需重新索引 |
| 多栏不恢复 | 双栏节之后必须显式加 `columns=1` 节，否则参考文献也两栏 |
| 悬挂缩进写法 | `indent=720 hanging_indent=720`，非负首行缩进 |
| 页码假象 | 用 live PAGE 域，判断按 `fldChar` 结构而非文字 |
| TOC 未更新 | 生成后需更新域（Word F9 或容器侧刷新）；必要时静态 TOC 兜底 |
| 空段落做间距 | 用 `space_before/space_after`，不用空段 |
| 表格题注位置 | 表题在表上方，图题在图下方 |

---

## 14. Skill 与交付核对

### 14.1 Skill 目录（沿用 ai-ppt 布局）

```
skill/
├── SKILL.md                     # 触发条件 + 工作流 + JSON 规范 + 排版原则
└── references/
    ├── schema.json              # 完整 JSON Schema（draft 2020-12，字段/枚举/默认值权威）
    ├── cli.md                   # CLI 全参考（unpack/repack/view/edit/render/schema/templates）
    ├── doc-templates.md         # 文档模板定义（版式/字体/默认值）
    ├── design-guide.md          # 排版与视觉层级原则
    └── writing-guide.md         # 中文学术写作规范（结构/措辞/引用/图表）
```

### 14.2 工作流（对齐 ai-ppt）

```
理解需求 → 选模板/风格 → 产出产物目录 JSON → repack 成 docx
        → render 导出 PNG/PDF 逐页核对 → 迭代 edit → 交付
```

**生成后必须渲染核对**，不要只凭命令退出码交付（ai-ppt 同样要求）。

### 14.3 交付自检（可写入 Skill 的检查清单）

| 检查 | 手段 |
|------|------|
| 结构/大纲 | `view <doc> /part[N] layout` 或对全文遍历；标题层级无跳级 |
| 占位泄漏 | 文本中无 `$x$`/`{{}}`/`<TODO>`/`lorem`/"Update field to see" |
| 页码 | 有页脚时是 live PAGE 域 |
| 引用往返 | 正文引用数 ≤ 文献条目数；顺序编码比对最大编号 |
| 图表编号 | SEQ 域存在且缓存值递增互异 |
| 视觉 | `render` 逐页核对顺序、摘要唯一、图表编号、公式渲染、多栏 |

> 这些自检**不新增 CLI 命令**，由 Skill 通过 `view`/`render`/`repack` 组合完成（必要时加 `--json` 便于脚本判断）。

---

## 15. 项目结构

```
ai-world/
├── Cargo.toml                 # workspace root（members: cli）
├── src/
│   ├── lib.rs                 # generate/parse/unpack/repack/Error
│   ├── error.rs
│   ├── model/                 # serde 数据模型（JSON 优先）
│   │   ├── document.rs        # Document（meta/page/theme/styles/template/parts）
│   │   ├── part.rs            # Part（section/blocks）
│   │   ├── blocks/            # heading/paragraph/list/table/image/formula/toc/bibliography…
│   │   ├── style.rs / page.rs / template.rs
│   ├── generate/              # JSON → DOCX
│   │   ├── mod.rs(document.xml)/styles.rs/numbering.rs/fields.rs/math.rs/header_footer.rs
│   │   ├── content_types.rs/rels.rs/theme.rs/doc_props.rs
│   ├── parse/                 # DOCX → JSON
│   │   ├── mod.rs/extract_media/document.rs/styles.rs/numbering.rs/fields.rs/math.rs/table.rs
│   ├── render/                # HTML/native/外部引擎 → PNG/PDF
│   └── utils/                 # units.rs/constants.rs/xml.rs/image.rs
├── cli/                       # aiworld-cli（clap）：7 子命令 + path/product/view/edit/render
├── skill/                     # SKILL.md + references/（含 academic-paper 内容）
├── examples/  tests/  docs/
```

**复用 ai-ppt**：workspace/cli 布局、`error.rs`、`TempGuard` 无状态输入、`product.rs` 产物目录、`path.rs` 路径解析、`view.rs`/`edit.rs` 框架、`schema`/`templates` 子命令、skill 布局、回环测试组织。
**借鉴 OfficeCLI**：§12 的 OOXML 实现细节与 §13.4 的坑。

---

## 16. 实施阶段

| Phase | 内容 | 验收 |
|-------|------|------|
| P1 | 骨架：workspace + utils/units + model（Document/Part/Block/Run/Style/Page） | `cargo build` 通过 |
| P2 | 最小生成：空白 A4 docx + 段落/标题 | Word/LibreOffice 可开 |
| P3 | 段落/run/样式/标题 + 解析回环 | 富文本双向等价 |
| P4 | 列表/表格/图片/题注 + 解析 | 复杂块回环 |
| P5 | 页面/分节/页眉页脚/页码 + 解析 | 页面结构回环 |
| P6 | 域(TOC/PAGE/SEQ/PAGEREF)/脚注/OMML/交叉引用 | 论文要素可用 |
| P7 | CLI：unpack/repack/view/edit/render/schema/templates 端到端 | 7 命令可用 |
| P8 | skill（docx + academic-paper）+ academic-paper 模板 + 渲染核对 | Agent 能生成论文并核对 PDF |

---

## 17. 已知限制

| 限制 | 说明 |
|------|------|
| 分页 | Word 引擎决定，无法绝对控制；以渲染核对 |
| 公式 | 完整 LaTeX↔OMML 复杂，首期白名单子集或降级图片 |
| 既有文档保真 | 修订/批注/艺术字/复杂浮动对象保留或降级，不保证可编辑往返 |
| TOC/页码 | 依赖域更新；部分查看器需手动 F9，必要时静态 TOC 兜底 |
| PDF 预览 | 纯 Rust 渲染保真有限；默认内建 HTML 或 `soffice`，需本机安装 |

---

## 18. 参考

- **ai-ppt / json2pptx**：架构与 CLI 主线（JSON 优先、产物目录、无状态 CLI、`unpack/repack/view/edit/render/schema/templates`、Skill、回环测试）。
- **OfficeCLI**：仅实现细节（样式继承、域、编号、公式、稳定 ID、单位解析、渲染核对、学术排版、常见坑）。
- 规范：ECMA-376 Part 1（WordprocessingML）/ Part 2（OPC）。
- 实现参考（仅写法）：`docx-rs`、`docx-rust`、`rdocx`、`python-docx`。
