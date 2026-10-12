# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_dataset/Sayi__poi-tl__20ccff67b7db52baf2dc252263d7fe38ec3778493cdc5c68ee4f76aa4923c328.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：A4（595×842 pt），方向：portrait
- 页边距（上/下/左/右，pt）：72 / 72 / 90 / 90

## 字体

- 西文：仿宋_GB2312，中文：仿宋_GB2312，标题：Calibri Light
- 文档中出现的字体（频次）：仿宋_GB2312×3

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Normal | 16 | - | - | - | 2.30 | - | - / - | 仿宋_GB2312 / 仿宋_GB2312 |

## 观察

- 正文西文字体：仿宋_GB2312；中文字体：仿宋_GB2312
- 标题字体：Calibri Light
- 正文字号：16 pt
- 正文行距：2.30 倍

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
