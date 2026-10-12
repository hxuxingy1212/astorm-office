# 文档风格模板说明

> 由 `json2docx extract` 从 `/tmp/docx_dataset/Sayi__poi-tl__40fbe5955c50f7bef4616b61291a90609e8d65dc155960ebb192f06245b9b154.docx` 提取。
> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。

## 页面

- 纸张：A4（595×842 pt），方向：portrait
- 页边距（上/下/左/右，pt）：72 / 72 / 90 / 90

## 字体

- 西文：Times New Roman，中文：宋体，标题：Helvetica
- 文档中出现的字体（频次）：Helvetica×2、Times New Roman×1

## 段落样式

| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |
|------|---------|------|------|------|------|-------------|--------------|--------------|
| Heading1 | 13.5 | 是 | #5B9BD5 | - | 1.80 | 27.1 | 15 / - | Helvetica / 宋体 |
| Normal | 13.5 | - | #333333 | - | 1.80 | 27 | 15 / - | Times New Roman / 宋体 |

## 观察

- 正文西文字体：Times New Roman；中文字体：宋体
- 标题字体：Helvetica
- 正文字号：13.5 pt
- 一级标题字号：13.5 pt
- 正文行距：1.80 倍
- 正文首行缩进：27 pt
- 标题颜色：#5B9BD5

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
