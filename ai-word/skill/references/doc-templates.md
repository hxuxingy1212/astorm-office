# 文档模板参考

`json2docx templates` 列出全部模板；在 `document.json` 中用 `"template": "<name>"` 引用。
模板只提供**默认值**：JSON 中显式给出的 `page`/`theme`/`styles` 字段优先。

| 名称 | 显示名 | 适用 |
|------|--------|------|
| `academic-paper` | Academic Paper（学术论文） | 期刊/会议/学位论文 |
| `report` | Business Report（报告） | 汇报、周报、方案 |
| `letter` | Letter（信函） | 正式信函 |
| `resume` | Resume（简历） | 个人简历 |
| `memo` | Memo（备忘录） | 内部备忘 |

---

## academic-paper

- 纸张：A4，纵向；页边距 上下 72pt / 左右 90pt。
- 字体：正文/西文 **Times New Roman**，中文 **宋体**，标题 **黑体**。
- 正文：12pt、1.5 倍行距、首行缩进 24pt。
- 标题：H1 18pt 粗 / H2 15pt 粗 / H3 13pt 粗全部左对齐；H1/H2 带 `outlineLvl`（供目录）。
- 参考文献：悬挂缩进（左 720 twip / 悬挂 720 twip，即 0.5in）。
- 题注：居中、10pt、斜体。

推荐结构：

```
封面(标题/作者/单位) → 摘要 → 关键词 → 目录(toc) → 各章 → 参考文献(bibliography)
```

## report

- A4；正文 **Calibri** 11pt、1.15 倍行距、无首行缩进。
- 标题 H1 20pt 粗，主题色 `1F4E79`。

## letter

- Letter 纸；**Garamond** 11pt、单倍行距、无缩进。

## resume

- Letter 纸；**Calibri** 10.5pt、紧凑行距 1.05、标题色 `2E74B5`。

## memo

- Letter 纸；Calibri 11pt、H1 14pt、标题色 `222222`。

---

## 覆盖模板字段

```json
{
  "template": "academic-paper",
  "page": { "size": "A4", "margins": { "top": 72, "left": 90 } },
  "theme": { "east_asia_font": "思源宋体", "heading_font": "思源黑体" },
  "styles": {
    "Normal": { "font_size": 12, "line_spacing": 1.5, "first_line_indent": 24, "align": "justify" },
    "Heading1": { "font_size": 18, "bold": true, "align": "center" }
  }
}
```

> 默认样式（Normal/Heading1-9/Title/Subtitle/Quote/Code/Caption/Reference/TOCHeading/Hyperlink 等）由生成器内置，`styles` 中同名项会覆盖。


---

## 场景与特性样张

- `skill/templates/` 下 21 套**场景/风格模板**（简历/合同/会议纪要/中英文信函/论文/报告等），`templates/INDEX.md` 有领域映射。
- 运行 `python3 scripts/gen_template_samples.py <目录>` 会为每套模板生成语境内样张，并额外生成 **`feature-showcase.docx`**，演示：段落盒式边框、制表位前导符、堆叠富图表、表格显式宽度/布局、内容控件(SDT)、表单域、通用域、注音(Ruby)、RTL。

> 新块/属性（`sdt`、`attachment`、`raw`+`diagram`、`chart` 富属性、`field`/`form_field`、`ruby`、`rtl` 等）以 `SKILL.md` 与 `references/schema.json` 为准。
