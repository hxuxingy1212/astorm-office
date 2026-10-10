# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_dataset/mirrors__Open-XML-SDK__168cd476d909307451aa6bea0295c48e9c69aaf5b6d675b09224ff8a7ead8889.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：Letter（612×792 pt），方向：portrait
- 页边距（上/下/左/右，pt）：72 / 72 / 72 / 72

## 字体

- 西文：Times New Roman，中文：Times New Roman，标题：Palatino Linotype
- 文档中出现的字体（频次）：Times New Roman×287、Palatino Linotype×229、Garamond×27

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Caption | 10 | 是 | - | - | - | - | 6 / 6 | Palatino Linotype / Times New Roman |
| Heading1 | 12 | 是 | - | center | - | - | 12 / 12 | Palatino Linotype / Times New Roman |
| Heading3 | 14 | 是 | - | center | - | - | 12 / 12 | Palatino Linotype / Times New Roman |
| Heading5 | 16 | 是 | - | - | - | - | 12 / 3 | Garamond / Times New Roman |
| Normal | 12 | - | - | - | - | - | 6 / 12 | Times New Roman / Times New Roman |
| TOC1 | 12 | - | - | - | - | - | - / 12 | Times New Roman / MS Mincho |

## 观察

- 正文西文字体：Times New Roman；中文字体：Times New Roman
- 标题字体：Palatino Linotype
- 正文字号：12 pt
- 一级标题字号：12 pt

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
