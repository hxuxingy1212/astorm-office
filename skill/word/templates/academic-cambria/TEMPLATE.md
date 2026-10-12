# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_dataset/jgm__pandoc__d5bde0239367f1fd990fc5219331b1f2f23c021f884718f68493328150892ba8.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：Letter（612×792 pt），方向：portrait
- 页边距（上/下/左/右，pt）：72 / 72 / 90 / 90

## 字体

- 西文：Cambria，中文：，标题：Calibri

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Heading1 | 16 | 是 | #345A8A | - | - | - | 24 / - | - / - |
| Heading2 | 16 | 是 | #4F81BD | - | - | - | 10 / - | - / - |
| Heading3 | 14 | 是 | #4F81BD | - | - | - | 10 / - | - / - |
| Heading4 | 12 | 是 | #4F81BD | - | - | - | 10 / - | - / - |
| Heading5 | 12 | - | #4F81BD | - | - | - | 10 / - | - / - |
| Heading6 | 12 | - | #4F81BD | - | - | - | 10 / - | - / - |
| Normal | 12 | - | - | - | - | - | - / - | Cambria /  |

## 观察

- 正文西文字体：Cambria；中文字体：
- 标题字体：Calibri
- 正文字号：12 pt
- 一级标题字号：16 pt
- 标题颜色：#345A8A

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
