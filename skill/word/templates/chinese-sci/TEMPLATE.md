# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_corpus/templates/Achuan__sci-nonum.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：A4（595×842 pt），方向：portrait
- 页边距（上/下/左/右，pt）：72 / 72 / 90 / 90

## 字体

- 西文：Times New Roman，中文：宋体，标题：Times New Roman
- 文档中出现的字体（频次）：Times New Roman×36、Consolas×1

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Code | 11 | 是 | #007020 | - | 1.50 | - | - / - | Consolas / 宋体 |
| Heading1 | 18 | 是 | - | center | 1.50 | - | 24 / - | Times New Roman / 黑体 |
| Heading2 | 16 | 是 | - | - | 1.50 | - | 15 / - | Times New Roman / 黑体 |
| Heading3 | 14 | 是 | - | - | 1.50 | - | 10 / - | Times New Roman / 黑体 |
| Heading4 | 12 | 是 | - | - | 1.50 | - | 10 / - | Times New Roman / 黑体 |
| Heading5 | 12 | 是 | - | - | 1.50 | - | 10 / - | Times New Roman / 黑体 |
| Heading6 | 12 | 是 | - | - | 1.50 | - | 10 / - | Times New Roman / 黑体 |
| Normal | 12 | - | - | - | 2.00 | 24 | 9 / 3 | Times New Roman / 宋体 |

## 观察

- 正文西文字体：Times New Roman；中文字体：宋体
- 标题字体：Times New Roman
- 正文字号：12 pt
- 一级标题字号：18 pt
- 正文行距：2.00 倍
- 正文首行缩进：24 pt

## 如何使用

把 `template.json` 中的 `page` / `theme` / `styles` 合并进新文档的 `document.json`（用户显式字段优先）：

```json
{
  "page": { ... },
  "theme": { ... },
  "styles": { "Normal": { ... }, "Heading1": { ... } },
  "parts": [ { "blocks": [ { "type": "heading", "level": 1, "text": "标题" } ] } ]
}
```

```bash
json2docx repack mydoc/ -o mydoc.docx
json2docx render mydoc.docx / -o preview.png
```
