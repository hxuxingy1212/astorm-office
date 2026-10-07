# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_dataset/mirrors__Open-XML-SDK__a4fa38264959e183825ab4fd7bb04b1caa07540f94f29485a1d13f64301cc875.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：Letter（612×792 pt），方向：portrait
- 页边距（上/下/左/右，pt）：54 / 54 / 72 / 72

## 字体

- 西文：Verdana，中文：Wingdings，标题：Trebuchet MS
- 文档中出现的字体（频次）：Verdana×285、Times New Roman×59、Wingdings×45、Trebuchet MS×30、Courier New×24、Symbol×9、Arial×1

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Heading1 | 24 | 是 | #E18F8F | - | - | - | 5 / 5 | Trebuchet MS / Times New Roman |
| Heading2 | 16 | 是 | #8080C0 | - | - | - | 5 / 5 | Trebuchet MS / Times New Roman |
| Heading3 | 13.5 | 是 | #000000 | - | - | 34.5 | 5 / 5 | Trebuchet MS / - |
| Heading4 | - | 是 | #000000 | - | - | - | 5 / 5 | Verdana / Times New Roman |
| Normal | 9 | - | #808080 | - | - | 21 | - / - | Verdana / Wingdings |
| TOC1 | 10.5 | 否 | - | - | - | - | 6 / 6 | Verdana / Times New Roman |

## 观察

- 正文西文字体：Verdana；中文字体：Wingdings
- 标题字体：Trebuchet MS
- 正文字号：9 pt
- 一级标题字号：24 pt
- 正文首行缩进：21 pt
- 标题颜色：#E18F8F

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
