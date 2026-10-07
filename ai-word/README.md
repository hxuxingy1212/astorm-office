# ai-word（json2docx）

**JSON ↔ DOCX（Word）双向转换的 Rust 库与命令行工具，面向 Agent 生成与编辑专业长文档（论文、报告、信函、简历等）。**

`ai-word` 以 **JSON 优先 + 产物目录 + 无状态 CLI** 为设计主线（对齐 [ai-ppt / json2pptx](https://gitee.com)），
直接手工生成与解析 OOXML（WordprocessingML），**零 Office 运行时依赖**。
你既可以用 JSON 从零描述一份文档再编译为 `.docx`，也可以把任意 `.docx` 解析回可编辑的 JSON 产物目录。

---

## 目录

- [特性](#特性)
- [架构](#架构)
- [安装与构建](#安装与构建)
- [快速开始](#快速开始)
- [CLI 命令](#cli-命令)
- [路径语法](#路径语法)
- [JSON 数据模型](#json-数据模型)
- [单位](#单位)
- [模板](#模板)
- [导出与预览](#导出与预览)
- [Word 特性覆盖](#word-特性覆盖)
- [示例](#示例)
- [测试](#测试)
- [Agent 技能包](#agent-技能包)
- [项目结构](#项目结构)
- [已知限制](#已知限制)
- [许可证](#许可证)

---

## 特性

| 类别 | 能力 |
|------|------|
| **双向转换** | JSON → DOCX 生成；DOCX → JSON 解析；产物目录 `unpack` / `repack` 回环 |
| **文本** | 多段落、多 run 行内混排、加粗/斜体/下划线/删除线、颜色、高亮、底纹、上下标、字体/字号、超链接、换行 |
| **段落** | 对齐、首行缩进、左右缩进、悬挂缩进、行距、段前段后间距、与下段同页、段中不分页、段前分页、段落底纹、下边框（分隔线）、制表位 |
| **标题与目录** | 多级标题、标题分级、live `TOC` 域、静态目录（`toc.static = true`） |
| **列表** | 有序 / 无序、多级缩进（level） |
| **表格** | 任意行列、表头、合并单元格（`colspan`/`rowspan`）、列宽、单元格底纹、自动编号题注 |
| **图片** | 内联与浮动（`wrap` + `x`/`y`/`behind_text` → `wp:anchor`）、题注、alt |
| **公式** | LaTeX ↔ OMML（上下标、`\frac`、`\sqrt`、`\sum`/`\int`、希腊字母、常用符号、`\left...\right`） |
| **图表** | 原生 OOXML 图表（column/bar/line/pie/doughnut/area，生成 `chartN.xml`） |
| **文本框/形状** | DrawingML `wps` 文本框（填充、边框、字号、颜色，内联或浮动） |
| **引用与注释** | 脚注、尾注、批注（`comments.xml`）、修订（`w:ins`/`w:del`）、交叉引用（`REF`/`PAGEREF` + 书签）、参考文献（GB/T 7714 等） |
| **页面与节** | 纸张/方向/页边距、分节（逐节 `sectPr`）、多栏、页码与起始、首页不同/奇偶页不同的页眉页脚、`第 X 页 / 共 Y 页`、水印 |
| **导出** | `render` 输出 HTML；`pdf` / `png` 经 LibreOffice 或无头 Chrome 导出（缺失引擎自动回退） |
| **Agent 友好** | 简洁 JSON 模型、产物目录、无状态 CLI、完整技能包（`skill/`） |

---

## 架构

```
┌──────────────┐      ┌───────────────┐      ┌─────────────┐
│  JSON 描述    │ ──▶  │  OOXML 部件   │ ──▶  │  DOCX 文件  │
│  (serde)     │ gen  │  (String)     │ zip  │  (.zip/OPC) │
└──────────────┘      └───────────────┘      └─────────────┘
       ▲                                             │
       └────────────────── parse ────────────────────┘
```

- **生成**：JSON → Rust 模型（serde）→ WordprocessingML 字符串 → OPC ZIP → `.docx`
- **解析**：`.docx` → ZIP 解压 → XML 流式解析（quick-xml）→ Rust 模型 → JSON
- **产物目录**：`document.json` + 按章分片的 `word/parts/*.json` + `word/media/`，是 CLI 与 Agent 的统一载体。

---

## 安装与构建

要求：Rust 1.75+。

> 本项目是 **astorm-office** monorepo 的成员（与 ai-excel / ai-ppt 共享 workspace 与 `office-core`）；
> 下列命令均在**仓库根目录**执行（单项目开发也可 `cd ai-word` 后用同样的相对参数）。

```bash
git clone <astorm-office 仓库>
cd astorm-office
cargo build -p json2docx -p json2docx-cli --release   # 仅构建本项目
cargo build --release                                  # 构建默认成员（三个项目）
cargo test -p json2docx -p json2docx-cli               # 本项目测试
```

全局安装 CLI：

```bash
cargo install --path cli
json2docx --help
```

---

## 快速开始

```bash
# 1) 从零写一份文档：产出产物目录（可内联或分片）
#    （Agent 通常直接写 JSON；也可 unpack 一份已有 docx 作为起点）

# 2) 产物目录 → DOCX
json2docx repack paper/ -o paper.docx

# 3) 查看结构
json2docx view paper.docx /part[1] layout

# 4) 修改
json2docx edit paper/ /part[1]/paragraph[1] set --prop text="修改后的正文"

# 5) 重新构建
json2docx repack paper/ -o paper.docx

# 6) 渲染预览 / 导出
json2docx render paper.docx / -o paper.png
```

---

## CLI 命令

二进制名 `json2docx`，库 crate 名 `json2docx`。**无状态设计**：命令之间不共享隐藏状态，输入输出全部由参数显式指定。

| 命令 | 功能 |
|------|------|
| `json2docx unpack <in.docx> -o <dir>` | DOCX → 产物目录（`document.json` + 分片 JSON + 媒体） |
| `json2docx repack <dir> -o <out.docx>` | 产物目录 → DOCX（含从零新建、或 `parts` 内联的目录）；自动校验，有问题退出码 3 |
| `json2docx view <输入> <路径> text\|layout` | 只读视图：文本+媒体 / 块结构树（附 `issues`/`warnings`） |
| `json2docx edit <输入> <路径> get\|set\|add\|remove [-o <out.docx>]` | 精准修改 |
| `json2docx render <输入> /part[N] \| / [-o <out.html\|pdf\|png>]` | 渲染预览 / 导出（`/` = 整篇） |
| `json2docx validate <输入>` | 结构校验（.docx 或产物目录），有问题退出码 3 |
| `json2docx dump <输入>` | 导出可回放的编辑指令（batch JSON） |
| `json2docx raw <输入> <部件>` / `raw-set <输入> <部件> --file F -o <out>` | 原始 OPC 部件字节级读写（`raw-set` 不覆盖原文件） |
| `json2docx extract <in.docx> -o <dir>` | **从 DOCX 提取风格样式模板**（`template.json` + `TEMPLATE.md`） |
| `json2docx schema [--json]` | 打印 JSON 字段指引；`--json` 输出机器可读 JSON Schema |
| `json2docx templates [--json]` | 列出文档模板（`--json` 输出结构化） |
| `json2docx merge <输入> -d '<JSON>' [-o]` | 模板数据填充：把 `{{key}}` 替换为 JSON 数据 |
| `json2docx serve <输入>` / `json2docx mcp` | 常驻编辑服务（stdin JSON 行）/ MCP stdio 服务 |

全局 flag（三个 CLI 一致）：`--json`（写盘命令输出结果 JSON、状态与错误结构化为 JSON 行）、`--quiet`、`--verbose`、`-o/--output`。stdout 只输出数据，状态/进度走 stderr；退出码 `0` 成功、`1` 运行时错误、`2` 用法错误、`3` 产物有校验问题。详见仓库根目录 `docs/cli-conventions.md`。

### edit 语义

- 输入为**产物目录** → **原地修改**，之后 `repack` 重建；
- 输入为 **.docx** → 写操作（set/add/remove）**必须带 `-o <输出.docx>`**，一次性 unpack→改→repack，**不回写原文件**；只读 `get` 无需 `-o`。

### render 格式

`-o` 的扩展名决定输出格式，也可用 `--format auto|html|pdf|png`：`.html` 内置预览、`.pdf`/`.png` 经 LibreOffice（`soffice`）或无头 Chrome 导出。

---

## 路径语法

以 `/` 开头，每段 `类型[索引]`，索引 **从 1 开始**、按类型独立计数；`part` 的别名是 `chapter`。

```text
/part[1]                                        # 第 1 个分片
/part[2]/heading[1]                             # 第 2 片第 1 个标题
/part[1]/paragraph[3]                           # 第 1 片第 3 段
/part[1]/table[1]/row[2]/cell[1]/paragraph[1]   # 表格单元格内段落
/part[1]/list[1]/item[2]/paragraph[1]           # 列表项内段落
```

`edit add --type` 支持：`heading` `paragraph` `list` `table` `image` `formula` `code` `quote` `caption` `toc` `bibliography` `chart` `textbox` `page_break`。

---

## JSON 数据模型

### 顶层 `document.json`

```json
{
  "meta": { "title": "示例文档", "author": "作者", "subject": "主题", "keywords": ["关键词"] },
  "page": { "size": "A4", "orientation": "portrait",
            "margins": { "top": 72, "bottom": 72, "left": 90, "right": 90 } },
  "theme": { "major_font": "Times New Roman", "minor_font": "Times New Roman",
             "east_asia_font": "宋体", "heading_font": "黑体" },
  "template": "academic-paper",
  "watermark": "DRAFT",
  "parts": ["word/parts/part1.json", "word/parts/part2.json"]
}
```

> `parts` 也支持**内联对象数组**（`[{ "blocks": [...] }]`），此时单个 `document.json` 即可。

### 分片 `word/parts/partN.json`

```json
{ "section": { "header": "页眉", "footer_page_number": true, "footer_format": "page_of" },
  "blocks": [ /* 见下 */ ] }
```

### 块类型

```jsonc
// 标题
{ "type": "heading", "level": 2, "text": "1.1 研究背景" }

// 段落（text 简写）
{ "type": "paragraph", "text": "正文……", "align": "justify",
  "first_line_indent": 24, "line_spacing": 1.5, "keep_next": true }

// 段落（runs 行内混排）
{ "type": "paragraph", "runs": [
    { "text": "关键词：", "bold": true },
    { "text": "多模态；检索", "color": "333333" },
    { "text": "官网", "hyperlink": "https://example.com" },
    { "text": "脚注", "footnote": "这是脚注内容" },
    { "text": "图 1", "bookmark": "fig1" },
    { "text": "图 1", "ref_target": "fig1", "pageref": true } ] }

// 列表
{ "type": "list", "ordered": true, "items": [
    { "blocks": [ { "type": "paragraph", "text": "第一点" } ], "level": 0 } ] }

// 表格（图题在图下方、表题在表上方；caption 未手动编号时插入 live SEQ 域）
{ "type": "table", "header_row": true, "widths": [120, 120],
  "caption": "实验对比",
  "rows": [ { "cells": [
      { "blocks": [ { "type": "paragraph", "text": "方法", "align": "center" } ], "colspan": 2 } ] } ] }

// 图片（wrap 非 inline 即浮动）
{ "type": "image", "src": "word/media/fig1.png", "width": 300, "caption": "系统架构", "alt": "架构图" }
{ "type": "image", "src": "word/media/logo.png", "wrap": "square", "x": 320, "y": 200 }

// 公式
{ "type": "formula", "latex": "L = L_{ret} + \\lambda L_{align}", "display": true, "number": "(1)" }

// 原生图表
{ "type": "chart", "chart_type": "column", "title": "季度营收",
  "categories": ["Q1", "Q2", "Q3"], "series": [ { "name": "营收", "values": [120, 150, 180] } ] }

// 文本框
{ "type": "textbox", "text": "提示", "fill": "FFF2CC", "line": "C00000", "width": 200, "height": 50 }

// 其他
{ "type": "code", "lang": "rust", "text": "fn main() {}" }
{ "type": "quote", "text": "引用" }
{ "type": "caption", "text": "图 1  说明", "of": "figure" }
{ "type": "toc", "title": "目录", "levels": [1, 2, 3] }
{ "type": "toc", "title": "目录", "static": true }
{ "type": "bibliography", "style": "GB/T 7714", "entries": [
    { "kind": "article", "authors": ["张三"], "title": "…", "container": "…", "year": 2024 } ] }
{ "type": "page_break" }
```

### Run（行内）字段

`text`（必填）、`bold`、`italic`、`underline`、`strike`、`color`、`highlight`、`shading`、`font_family`、`font_size`、`superscript`、`subscript`、`code`、`hyperlink`、`footnote`、`endnote`、`comment`（+`comment_author`）、`revision`（`ins`/`del`，+`revision_author`）、`bookmark`、`ref_target`、`pageref`。

完整字段以 `json2docx schema` 或 [skill/references/schema.json](skill/references/schema.json) 为准。

---

## 单位

| 量 | 单位 |
|----|------|
| 长度 / 字号 | **pt**（也接受 `12pt`、`0.5cm`、`1.5x`、`150%`） |
| 行距 | 倍数（`1.5`） |
| 页面 | `A4` / `Letter` / `A3` / `custom` |
| 颜色 | 6 位十六进制，**不带 `#`**（`"1F3864"`） |

---

## 模板

`template` 内置 5 套，仅提供默认值（JSON 中显式字段优先）：

| 名称 | 说明 |
|------|------|
| `academic-paper` | 学术论文：Times New Roman/宋体、1.5 倍行距、首行缩进、标题分级 |
| `report` | 商务报告：Calibri、1.15 倍行距 |
| `letter` | 信函：Garamond、单倍行距 |
| `resume` | 简历：Calibri、紧凑行距 |
| `memo` | 备忘录：简洁排版 |

```bash
json2docx templates
```

---

## 风格模板提取

给定任意 `.docx`，可提取其**风格样式模板**，用于生成同风格的其它文档：

```bash
json2docx extract 样张.docx -o tpl/
# tpl/template.json  → page / theme / styles
# tpl/TEMPLATE.md     → 面向大模型的说明（字体/字号/颜色/行距/缩进/对齐 + 用法）
```

- 解析 `styles.xml` 的样式继承链（`basedOn` + `docDefaults`），并聚合文档中段落/run 的实际格式。
- 用法：把 `page` / `theme` / `styles` 合并进新文档的 `document.json` → `repack` 生成 → `render` 核对。
- 仓库内 `skill/templates/` 提供从真实文档提取的**存量模板库**（21 套：中/英/日文，学术/公文/商务/企业/技术/设计/简历/合同/纪要等），见 [skill/templates/INDEX.md](skill/templates/INDEX.md)。
- 批量采集与提取：`./scripts/collect_docs.sh`（多仓库 docx，已汇集 **1431 份**并全部通过回环校验）→ `./scripts/collect_templates.sh`。

---

## 导出与预览

```bash
json2docx render doc.docx /part[1] -o page.png     # 单分片 → PNG
json2docx render doc.docx /         -o all.pdf     # 整篇 → PDF
json2docx render doc.docx /         -o all.html    # 整篇 → HTML 预览
```

- 优先使用 LibreOffice（`soffice`）转换；缺失时回退到无头 Chrome（`--screenshot` / `--print-to-pdf`）。
- HTML 预览内嵌本地图片（base64 data URI），并使用 SVG 近似渲染图表、Unicode 近似渲染公式。

---

## Word 特性覆盖

- **文字**：加粗/斜体/下划线/删除线、颜色、高亮/底纹、上下标、字体字号、超链接、换行
- **段落**：对齐、缩进（首行/悬挂/左右）、行距、间距、keep-next / keep-lines / 段前分页、底纹、下边框、制表位
- **结构**：多级标题、目录（live/静态）、多级列表、表格（合并/列宽/表头/题注）、图片（内联/浮动/题注）、公式、图表、文本框
- **引用**：脚注、尾注、批注、修订、交叉引用、参考文献
- **页面**：纸张/页边距、分节、多栏、页码（含起始与 `共 Y 页`）、首页/奇偶页眉页脚、水印

---

## 示例

```bash
cargo run --example simple      # 基础块（标题/段落/列表/表格/公式）
cargo run --example paper       # 学术论文（摘要/目录/章节/图表/参考文献/脚注）
cargo run --example features    # 全量特性（注释/修订/交叉引用/合并单元格/浮动图/图表/文本框/水印/静态目录）
cargo run --example attachment  # 附件与嵌入对象
```

生成结果位于 `examples/out/`。

---

## 测试

```bash
cargo test                  # 库回环 + CLI 集成测试
./scripts/e2e.sh            # 端到端：示例生成 + XML 校验 + CLI 全链路（unpack/view/edit/repack/render）
./scripts/corpus.sh         # 真实语料回归：下载 Apache POI / python-docx 测试 docx 并逐份 unpack→repack 校验
./scripts/visual_diff.sh    # 视觉对比：原始 vs 回环 渲染 PNG 逐张比较
./scripts/collect_docs.sh      # 采集语料（多仓库 docx，去重汇集到 /tmp/docx_dataset）
./scripts/collect_templates.sh # 批量模板提取：对语料逐份 extract，产出 templates_catalog/
./scripts/visual_roundtrip.py # 视觉回环：原始 vs unpack→repack 渲染 PNG 像素对比（找问题）
```

> 视觉对比默认用 `textutil`(HTML)+Chrome 保证确定性，`VISUAL_RENDERER=qlmanage` 可切换系统 QuickLook 渲染器（更保真，仅首页）。
> `textutil` 不解析自定义样式/字体，其视觉差异多为渲染器所致；语料**文本保真度**以 `corpus.sh` 输出为准（44 份语料 ≥0.9，唯一差异为修订显示）。

---

## Agent 技能包

[`skill/`](skill) 为 Agent 提供即插即用的技能：

| 文件 | 内容 |
|------|------|
| [skill/SKILL.md](skill/SKILL.md) | 触发条件、工作流、JSON 规范、块示例、写作与排版原则 |
| [skill/references/schema.json](skill/references/schema.json) | 完整 JSON Schema（draft 2020-12） |
| [skill/references/cli.md](skill/references/cli.md) | CLI 全参考与组合用法 |
| [skill/references/doc-templates.md](skill/references/doc-templates.md) | 5 套模板细则 |
| [skill/references/writing-guide.md](skill/references/writing-guide.md) | 学术写作规范（结构/引用/图表/公式） |
| [skill/references/design-guide.md](skill/references/design-guide.md) | 排版设计规范 |

---

## 项目结构

```
ai-word/
├── Cargo.toml
├── src/
│   ├── model/       # JSON 数据模型：Document/Part/Block/Run/Style/Page/Theme/Template
│   ├── generate/    # JSON → DOCX：document/styles/numbering/settings/theme/chart/math + OPC 打包
│   ├── parse/       # DOCX → JSON：XML 树解析 + unpack/repack 产物目录
│   ├── extract/     # 风格模板提取：styles.xml 继承解析 + 段落格式聚合 + TEMPLATE.md
│   └── utils/       # 单位换算 / XML 辅助 / 图片
├── cli/             # json2docx（clap，15 子命令）
├── skill/           # Agent 技能包（含 templates/ 存量风格模板库）
├── examples/        # simple / paper / features
├── tests/           # 回环 / CLI / 语料回归测试
└── scripts/         # e2e.sh / corpus.sh / visual_diff.sh
```

---

## 已知限制

- 分页位置由 Word 引擎决定，无法绝对控制；以渲染预览核对。
- 回环会**重生成样式与主题**，不保留源文档的自定义样式/字体模板；内容与结构保留。
- 非目标：艺术字/WordArt、`.doc` 旧版本格式、修订的"接受/拒绝"操作。

---

## 许可证

MIT
