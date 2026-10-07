---
name: ai-word
description: 用 json2docx 生成与编辑 Word（.docx）：论文/报告/信函/简历。以产物目录 JSON 为中间形态，repack 构建、view/edit 修改、render 核对。用户要写或改 Word/论文/报告/排版/参考文献时使用，未提 JSON 也应触发。
version: 0.2.0
metadata:
  category: 办公自动化
  tags: docx, word, 论文, 报告, json2docx, 学术写作
compatibility: 需要 Rust 工具链（cargo）或已安装 json2docx；render 输出 HTML 预览
---

# Word 分册（json2docx）

> 通用契约、工作流、CLI 速查、edit 语义与通用约束见统一总纲
> [../../skill/SKILL.md](../../skill/SKILL.md)。本册只含 **Word 格式特有**内容：
> 产物目录结构、JSON 生成规范、模板链路与场景速查。

## 产物目录结构

```text
paper/
├── document.json            # meta/page/theme/styles/template + parts 文件路径数组
└── word/
    ├── parts/
    │   ├── part1.json       # 每章一个文件，含 blocks[]
    │   └── part2.json
    └── media/               # 图片实体（src 相对产物根或 URL）
```

> `document.json` 的 `parts` 也可写成**内联对象数组**（`[{ "blocks": [...] }]`），此时可只用一个文件，直接 `repack`。

## JSON 生成规范

### 单位
- 长度/字号：**pt**（`font_size: 12`）；也可写 `"14pt"`（CLI 的 `--prop` 支持 `12pt`/`0.5cm`/`1.5x`）
- 行距：倍数（`line_spacing: 1.5`）
- 页面：`"A4" | "Letter" | "A3"`；颜色：6 位十六进制**不带 `#`**（`"1F3864"`）

### document.json

```json
{
  "meta": { "title": "…", "author": "…", "subject": "…", "keywords": ["…"] },
  "page": { "size": "A4", "orientation": "portrait",
            "margins": { "top": 72, "bottom": 72, "left": 90, "right": 90 } },
  "theme": { "major_font": "Times New Roman", "minor_font": "Times New Roman",
             "east_asia_font": "宋体", "heading_font": "黑体" },
  "template": "academic-paper",
  "parts": ["word/parts/part1.json", "word/parts/part2.json"]
}
```

### 块类型（18 种）

```jsonc
// 标题
{ "type": "heading", "level": 1, "text": "第一章 绪论" }

// 段落（可用 text 简写，或用 runs 做行内混排）
{ "type": "paragraph", "text": "本文提出……", "first_line_indent": 24, "line_spacing": 1.5, "align": "justify" }
{ "type": "paragraph", "runs": [
    { "text": "关键词：", "bold": true },
    { "text": "多模态；检索", "color": "333333" },
    { "text": "官网", "hyperlink": "https://example.com" } ] }

// 列表
{ "type": "list", "ordered": true, "items": [
    { "blocks": [ { "type": "paragraph", "text": "第一点" } ] } ] }

// 表格（图题在图下方、表题在表上方；表样式/边框/行高/单元格对齐均可选）
{ "type": "table", "header_row": true, "widths": [120, 120], "width": 240, "layout": "fixed",
  "style": "LightShading", "border": { "style": "single", "color": "808080", "width": 0.5 },
  "caption": "表 1  实验对比",
  "rows": [ { "height": 28, "height_rule": "atLeast", "cells": [
      { "blocks": [ { "type": "paragraph", "text": "方法", "align": "center" } ],
        "fill": "F2F2F2", "valign": "center", "margin": 4.0 } ] } ] }

// 图片（wrap 非 inline 即浮动）
{ "type": "image", "src": "word/media/fig1.png", "width": 300, "align": "center",
  "caption": "图 1  系统架构", "alt": "系统架构图" }
{ "type": "image", "src": "word/media/logo.png", "wrap": "square", "x": 300, "y": 200 }

// 公式（display 独立成块）
{ "type": "formula", "latex": "E = mc^2", "display": true, "number": "(1)" }

// 原生图表（series 可带 color；legend/x_title/y_title/y_min/y_max/data_labels/grouping 可选）
{ "type": "chart", "chart_type": "column", "title": "季度营收",
  "grouping": "stacked", "legend": "r", "x_title": "季度", "y_title": "万元",
  "y_min": 0, "y_max": 200, "data_labels": true,
  "categories": ["Q1", "Q2", "Q3"],
  "series": [ { "name": "营收", "values": [120, 150, 180], "color": "4472C4" } ] }

// 文本框
{ "type": "textbox", "text": "提示文字", "fill": "FFF2CC", "line": "C00000",
  "width": 200, "height": 50, "align": "center" }

// 形状/线条（预设几何：rect/roundRect/ellipse/rightArrow/line/...）
{ "type": "shape", "shape_type": "rightArrow", "width": 80, "height": 40,
  "fill": "4472C4", "line": "2E5496", "text": "流程", "text_color": "FFFFFF" }

// 附件（OLE 嵌入对象，icon 为预览图）
{ "type": "attachment", "src": "word/embeddings/book.xlsx", "name": "book.xlsx",
  "prog_id": "Excel.Sheet.12", "icon": "word/media/preview.png" }

// 内容控件（SDT）
{ "type": "sdt", "sdt_type": "dropDownList", "alias": "状态", "tag": "status",
  "items": ["进行中", "已完成"],
  "blocks": [ { "type": "paragraph", "text": "进行中" } ] }

// SmartArt（保留原始 XML；diagram.texts 可编辑并回写数据部件）
{ "type": "raw", "xml": "…原样 drawing XML…",
  "rels": [ { "id": "rId5", "rel_type": "…/diagramData", "target": "diagrams/data1.xml" } ],
  "diagram": { "data": "word/diagrams/data1.xml", "texts": ["步骤一", "步骤二"] } }

// 其他
{ "type": "code", "lang": "python", "text": "def f():\n    pass" }
{ "type": "quote", "text": "引用的段落" }
{ "type": "caption", "text": "图 1  说明", "of": "figure" }
{ "type": "page_break" }
{ "type": "toc", "title": "目录", "levels": [1, 2, 3] }
{ "type": "toc", "title": "目录", "static": true }
{ "type": "bibliography", "style": "GB/T 7714", "entries": [
    { "kind": "article", "authors": ["张三"], "title": "…", "container": "…", "year": 2024 } ] }
```

**Run 字段**：`text`、`bold`、`italic`、`underline`、`strike`、`color`、`highlight`、`shading`、`font_family`/`east_asia_font`/`cs_font`（ascii/东亚/复杂脚本字体）、`font_size`、`superscript`、`subscript`、`code`、`hyperlink`（`#` 开头为内部书签）、`footnote`/`endnote`、`bookmark`、`ref_target`+`pageref`、`comment`+`comment_author`、`revision`+`revision_author`、`field`（通用域：`IF`/`DOCPROPERTY`/`STYLEREF`/`MERGEFIELD`/`DATE` 等）、`form_field`（`kind`: text/checkbox/dropdown）、`symbol`（`Font:Char`）、`caps`/`small_caps`、`spacing`（字间距 pt）、`ruby`（注音）、`lang`/`lang_ea`、`rtl`、`image`（行内图片）、`sdt`（行内内容控件）、`outline`/`shadow`/`emboss`/`imprint`。

```jsonc
// 带脚注的正文
{ "type": "paragraph", "runs": [
    { "text": "该结论见相关综述" },
    { "text": "（脚注）", "footnote": "Zhang et al. 2024." } ] }
// 交叉引用：先给标题/图注加书签，再在正文引用
{ "type": "paragraph", "runs": [ { "text": "图 1", "bookmark": "fig1" } ] }
{ "type": "paragraph", "runs": [ { "text": "图 1", "ref_target": "fig1", "pageref": true } ] }
```

**段落/表格/节 扩展属性**：段落 `border_box`（四边盒式边框）、`tabs`（`{pos,align,leader}`，leader 支持 `dot`/`dash`）、`rtl`（`w:bidi`）、`auto_space_de`/`auto_space_dn`、`drop_cap`；表格 `width`/`width_pct`/`layout`/`rtl`/`valign`/`height`+`height_rule`；节 `section.page_border`、`doc_grid`、`rtl`/`rtl_gutter`、`gutter`、`section_type`、`title_page`。

## 模板填充与服务化命令（Word 特有用法）

```bash
# 模板数据填充：替换 {{key}}（支持 a.b 嵌套）
json2docx merge <输入.docx|产物目录> -d '{"client":"Acme","order":{"id":"A-100"}}' -o out.docx

# MCP 服务（stdio JSON-RPC）：工具 unpack/repack/view/validate/merge/edit/render/schema
json2docx mcp

# 常驻编辑：加载一次，stdin 逐行 JSON（edit/view/merge/issues/warnings/save/quit）
json2docx serve <输入>

# 结构校验：repack 自动校验；view 输出 issues[] 与 warnings[]（空段落/缺 alt/标题跳级/占位符/缺目录）
```

## 存量模板与风格提取

当用户提供**样张**(任意 docx)、或希望"照这个风格来"时，先提取风格模板：

```bash
json2docx extract 样张.docx -o tpl/     # → tpl/template.json + tpl/TEMPLATE.md
```

- `template.json` 含 `page` / `theme` / `styles`，直接合并进新文档的 `document.json` 即可套用同风格。
- `TEMPLATE.md` 是面向大模型的说明（字体/字号/颜色/行距/缩进/对齐），据此撰写正文。
- 也可直接选用 [templates/INDEX.md](templates/INDEX.md) 中的存量模板（学术/中文公文/商务/企业/技术等）。

> 生成时于 `document.json` 提供 `page` / `theme` / `styles`，用户显式字段优先于 `template` 预设。

## 设计原则

1. **层级清晰**：Title → Heading1 → Heading2 → 正文；不要一墙 `Normal` 段落。
2. **字号体系**：正文小四/Times 12pt；H1 ≥18pt 粗、H2 14pt 粗、H3 12pt 粗；同页字号层级 ≤4 档。
3. **间距用属性**：用 `space_before`/`space_after`，不要用空段落。
4. **中英混排**：中文用宋体、西文 Times New Roman、标题黑体（模板已内置）。
5. **题注铁律**：图题在图下方，表题在表上方。
6. **公式规范**：正文中的变量/希腊字母/下标走 `formula`，不要写成纯文本。
7. **对齐显式化**：标题居中用 `heading.align="center"`，正文 `align="justify"`，落款/日期/签名 `align="right"`，表格默认居中（`table.align`），不要依赖模板默认。详见 [references/design-guide.md](references/design-guide.md) §4。

## 场景速查（结构 + 版式）

按用户意图选模板并组织结构；对齐/缩进按 [references/design-guide.md](references/design-guide.md) §4 显式设置。

| 场景 | 结构 | 版式要点 | 推荐模板 |
|------|------|----------|----------|
| 通知 / 公告 / 公文 | 居中标题 → 主送机关 → 正文（一/二/三）→ 落款 | 标题居中；主送机关顶格；正文首行缩进 2 字、两端对齐；落款右对齐 | `chinese-official` |
| 学术论文 / 开题 / 答辩 | 标题→作者单位→摘要→关键词→目录→引言/方法/实验/结论→参考文献 | 标题/摘要居中；正文两端对齐+缩进 2 字；图表题注居中；参考文献悬挂缩进 | `chinese-sci-numbered`、`academic-cambria` |
| 商业合同 / 协议 | 标题→甲乙方→条款（第一条…）→签署 → 落款 | 标题居中；条款左对齐/两端对齐；签署与日期右对齐 | `corporate-arial`、`classic-times` |
| 简历 / CV | 姓名/联系 → 教育 → 经历 → 技能 | 姓名居中或左；分节标题左；条目用列表；日期右对齐 | `resume`（模板命令）、`modern-calibri` |
| 会议纪要 | 标题（时间/地点/参会）→ 议题与决议 → 待办（含负责人/期限） | 标题居中；待办用表格（列：事项/负责人/期限/状态） | `report-cambria`、`chinese-modern` |
| 产品需求文档 PRD | 背景/目标 → 用户与场景 → 功能需求（表格）→ 非功能 → 里程碑 | 标题左；需求用表格；优先级列右对齐 | `technical-arial`、`modern-aptos` |
| 项目周报 / 月报 | 本期进展 → 数据（表）→ 风险与问题 → 下期计划 | 标题居中或左；数据表格居中；要点列表 | `chinese-modern`、`report-cambria` |
| 经营 / 财务报告 | 摘要 → 分项数据（表/图）→ 结论与建议 | 标题居中；数据表居中+题注；落款右对齐 | `report-cambria`、`chinese-modern` |
| 招投标文件 | 封面 → 目录 → 商务/技术标 → 附件 | 封面标题居中；目录用 `toc`；附件编号清晰 | `corporate-arial`、`chinese-sci` |
| 信函 / 邮件 | 称呼 → 正文 → 此致/敬礼 → 署名日期 | 中文：称呼顶格、此致空两格、敬礼顶格、署名右；英文：日期右、正文左 | `chinese-letter`、`french-letter` |
| 技术 / 接口文档 | 概述 → 鉴权 → 接口清单 → 示例 → 错误码 | 代码块用 `code`；接口用表格；左对齐 | `technical-arial` |

> 若用户提供样张，先 `json2docx extract 样张.docx -o tpl/` 得到同风格模板再套用。

## 学术论文（academic-paper）

- 结构：封面 → 摘要 → 关键词 → 目录 → 正文各章 → 结论 → 参考文献。
- 引用格式：中文论文默认 **GB/T 7714**（顺序编码 `[1]`）；英文可用 APA/IEEE。
- 参考文献用 `bibliography` 块，自动悬挂缩进（`indent=720 hanging=720`）。
- 段落首行缩进 24pt、1.5 倍行距、两端对齐（模板已内置）。
- 图表用 `caption`（图下/表上）；公式用 `formula display=true`。
- 引用注：正文引注用 `run.footnote`（脚注）或 `run.endnote`（尾注）。
- 交叉引用：给图表标题的 run 加 `bookmark`，正文用 `ref_target`（`pageref:true` 显示页码）。
- 目录 `{"type":"toc"}` 默认为 live 域；需要跨查看器可靠（不依赖 F9）时用 `"static": true`。

## Word 特有约束与核对

- 编辑既有文档（unpack→修改→repack）后，建议用 `scripts/visual_roundtrip.py` 做**原图/回环图**视觉核对：内容与页面尺寸保持一致；VML 文本框（w:pict→v:textbox）与 VML/DrawingML 形状线条会分别回流为 `textbox`、`shape` 块，位置/尺寸/填充/边框保留。
- 图片 `src` 为产物根相对路径或 URL，本地文件需真实存在。

## 参考资源

| 文件 | 内容 | 何时读 |
|------|------|--------|
| [references/schema.json](references/schema.json) | 完整 JSON Schema（字段/枚举/默认值权威） | 字段不确定时 |
| [references/cli.md](references/cli.md) | CLI 命令全参考与组合用法 | 执行命令细节不确定时 |
| [references/doc-templates.md](references/doc-templates.md) | 5 套文档模板（字体/版式/默认值） | 选模板或排版前 |
| [references/writing-guide.md](references/writing-guide.md) | 学术写作规范（结构/引用/图表/公式） | 写论文内容前 |
| [scenarios/academic-paper.md](scenarios/academic-paper.md) | 学术论文专项技能（结构约定/公式编号/@paraId 修订/batch） | 需求命中论文/投稿场景时优先 |
| [templates/INDEX.md](templates/INDEX.md) | 存量风格模板库（21 套，含简历/合同/纪要内置场景，按领域映射） | 需要"照某风格生成"时 |
