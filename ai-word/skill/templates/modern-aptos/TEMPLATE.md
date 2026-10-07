# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_dataset/mirrors__pandoc__e304901308f7b6325bd3403fce0ec3183b5ce23a361e4cb9ed08986b5b56ef34.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：A4（595×842 pt），方向：portrait
- 页边距（上/下/左/右，pt）：72 / 72 / 90 / 90

## 字体

- 西文：Aptos，中文：，标题：Aptos Display

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Heading1 | 20 | - | #0F4761 | - | - | - | 18 / 4 | - / - |
| Heading2 | 16 | - | #0F4761 | - | - | - | 8 / 4 | - / - |
| Heading3 | 14 | - | #0F4761 | - | - | - | 8 / 4 | - / - |
| Heading4 | 12 | - | #0F4761 | - | - | - | 4 / 2 | - / - |
| Heading5 | 12 | - | #0F4761 | - | - | - | 4 / 2 | - / - |
| Heading6 | 12 | - | #595959 | - | - | - | 2 / - | - / - |
| Normal | 12 | - | - | - | - | - | 9 / 9 | Aptos /  |

## 观察

- 正文西文字体：Aptos；中文字体：
- 标题字体：Aptos Display
- 正文字号：12 pt
- 一级标题字号：20 pt
- 标题颜色：#0F4761

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
