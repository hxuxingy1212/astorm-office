# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_corpus/poi/61470.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：Letter（612×792 pt），方向：portrait
- 页边距（上/下/左/右，pt）：72 / 72 / 72 / 72

## 字体

- 西文：SimSun，中文：MS Mincho，标题：Calibri Light
- 文档中出现的字体（频次）：MS Mincho×1、SimSun×1

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Normal | 8 | - | - | - | - | - | - / - | SimSun / MS Mincho |

## 观察

- 正文西文字体：SimSun；中文字体：MS Mincho
- 标题字体：Calibri Light
- 正文字号：8 pt

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
