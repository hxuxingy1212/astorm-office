# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_dataset/bokuweb__docx-rs__0c6090cafe83700de673d37b51956b85bc4af4e81f0b1bbd21810d7ad24ae094.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：A4（595×842 pt），方向：portrait
- 页边距（上/下/左/右，pt）：99.2 / 85 / 85 / 85

## 字体

- 西文：游明朝，中文：，标题：游ゴシック Light
- 文档中出现的字体（频次）：Meiryo UI×2

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Heading2 | 12 | - | - | - | 1.30 | - | 18 / - | - / Meiryo UI |
| Heading4 | 10.5 | - | - | - | 1.30 | - | - / - | Meiryo UI / Meiryo UI |
| Heading5 | 10.5 | - | - | - | 1.30 | - | - / - | - / Meiryo UI |
| Normal | 10.5 | - | - | - | - | - | - / - | 游明朝 /  |

## 观察

- 正文西文字体：游明朝；中文字体：
- 标题字体：游ゴシック Light
- 正文字号：10.5 pt

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
