# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_dataset/Sayi__poi-tl__8b6d7919efd1d7acd3e082cbd4c1cbb61fbfc456cc4af4ed949a7b1a2cb47b87.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：A4（595×842 pt），方向：portrait
- 页边距（上/下/左/右，pt）：72 / 72 / 90 / 90

## 字体

- 西文：微软雅黑，中文：宋体，标题：微软雅黑
- 文档中出现的字体（频次）：微软雅黑×6

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Heading1 | 22 | 否 | - | justify | 2.40 | - | 17 / 16.5 | 微软雅黑 / 微软雅黑 |
| Heading2 | 16 | - | - | justify | 1.50 | - | 7 / 7 | 微软雅黑 / 微软雅黑 |
| Heading3 | 15 | - | - | justify | 1.50 | - | 1 / 1 | 微软雅黑 / 微软雅黑 |
| Heading4 | 14 | 是 | - | justify | 1.50 | - | 2 / 2.5 | 微软雅黑 / 微软雅黑 |
| Heading5 | 12 | - | - | justify | 1.50 | - | 2 / 2.5 | 微软雅黑 / 微软雅黑 |
| Normal | 10.5 | - | - | - | - | - | - / - | 微软雅黑 / 宋体 |

## 观察

- 正文西文字体：微软雅黑；中文字体：宋体
- 标题字体：微软雅黑
- 正文字号：10.5 pt
- 一级标题字号：22 pt

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
