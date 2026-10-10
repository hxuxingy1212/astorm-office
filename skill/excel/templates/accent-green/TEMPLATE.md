# Excel 样式模板：DrawingSizeInPx

由 json2xlsx `template extract` 从存量 Excel 提取，供大模型据此生成同类风格的表格。

## 概览

- 工作表：icons、International URLs、International Settings、DropdownSizer、Instructions、Images、International Translations、Data Validation、International Data、Example、Data Definitions、Template、Browse Data、Valid Values、Dropdown Lists、AttributePTDMAP
- 默认行高：12.75pt
- 表头：`International Settings` 第 1 行

## 配色（按出现频次）

| 颜色 | 用途 | 次数 |
|---|---|---|
| `FFFFFF` | fill | 465 |
| `92D050` | fill | 169 |
| `FCD5B4` | fill | 121 |
| `CC9999` | fill | 115 |
| `FF0000` | font | 87 |
| `8DB4E2` | fill | 35 |
| `BBA680` | fill | 35 |
| `FFFF00` | fill | 32 |
| `0FFF66` | fill | 28 |
| `F8A45E` | fill | 25 |
| `FFCC66` | fill | 24 |
| `CCCC99` | fill | 20 |
| `FFCC99` | fill | 17 |
| `FF0000` | font | 16 |
| `C0C0C0` | fill | 9 |
| `ABD27F` | fill | 8 |

## 字体

| 字体 | 字号 | 加粗 | 颜色 | 次数 |
|---|---|---|---|---|
| Verdana | 10pt | 否 | - | 2446 |
| Calibri | 11pt | 否 | - | 669 |
| Calibri | 11pt | 是 | - | 348 |
| Verdana | 10pt | 是 | - | 229 |
| Verdana | 10pt | 否 | - | 140 |
| Arial | 10pt | 否 | - | 32 |
| Verdana | 11pt | 否 | - | 29 |
| Calibri | 10pt | 否 | - | 24 |
| Calibri | 11pt | 否 | FF0000 | 16 |
| Verdana | 11pt | 是 | - | 12 |
| Verdana | 10pt | 是 | - | 8 |
| Calibri | 11pt | 否 | - | 3 |
| Verdana | 18pt | 否 | - | 1 |
| Calibri | 20pt | 否 | - | 1 |
| Verdana | 10pt | 否 | - | 1 |

## 表头样式（JSON）

```json
{
  "font": {
    "bold": true,
    "name": "Verdana",
    "size": 10.0
  }
}
```

表头列名：`false`、`Show the Create Templates Toolbar`

## 数字格式

- `@`
- `mmmm\ d\,\ yyyy\ \-\ h:mm:ss\ AM/PM;@`
- `mmmm\ d\,\ yyyy\ h:mmAM/PM`

## 列布局（字符宽度）

1:9, 2:9, 3:9, 4:9, 5:9, 6:9, 7:9, 8:9, 9:9, 10:9, 11:9, 12:9, 13:9, 14:9, 15:9, 16:9, 17:9, 18:9, 19:9, 20:9, 21:9, 22:9, 23:9, 24:9, 25:9, 26:9, 27:9, 28:9, 29:9, 30:9, 31:9, 32:9, 33:9, 34:9, 35:9, 36:9, 37:9, 38:9, 39:9, 40:9, 41:9, 42:9, 43:9, 44:9, 45:9, 46:9, 47:9, 48:9, 49:9, 50:9, 51:9, 52:9, 53:9, 54:9, 55:9, 56:9, 57:9, 58:9, 59:9, 60:9, 61:9, 62:9, 63:9, 64:9, 65:9, 66:9, 67:9, 68:9, 69:9, 70:9

## 如何生成同类文件

1. 复制 `skeleton/` 产物目录（已保留表头与全部样式，数据清空）。
2. 按业务填写 `xl/worksheets/sheetN.json` 的数据行（表头样式可引用 `template.json.header.style`）。
3. 用本工具重建并验收：

```bash
json2xlsx repack skeleton/ -o out.xlsx
json2xlsx render skeleton/ -o preview.html
```

## 模板 JSON

完整结构化模板见 `template.json`（含 `palette` / `fonts` / `header` / `columns` / `number_formats`），可直接被生成流程引用。

## 使用建议（最佳实践）

1. **中文/字体**：本模板默认字体若为拉丁字体（如 Calibri），生成含中文的内容时请给表头与数据 `font.name` 指定中文字体（如 `微软雅黑`、`PingFang SC`、`等线`），或设置工作簿 `default_font`；否则部分环境（如 LibreOffice 无 CJK 回退）会显示空白。
2. **数字格式**：金额用 `#,##0`/`#,##0.00`，比率用 `0.0%`，日期用 `yyyy-mm-dd`；务必显式设置，避免裸浮点（否则会出现 `13.100000000000001%`）。
3. **对齐**：数值列右对齐、文本列左对齐、表头居中。
4. **强调/合计行**：加粗 + 顶部细边框；深色填充配浅色文字。
5. **列宽**：按内容显式设置（中文按约 2 个字符宽估算），避免内容被截断。
6. **图表**：放在数据右侧或下方；需要打印时设置 `print_area`，避免图表/宽表把内容挤到多页。
7. **验收**：生成后务必用 `render` 目视核对（HTML 预览含合并单元格与图表；LibreOffice headless 对中文可能无回退字体）。

## 备注

- 检测到表头：International Settings 第 1 行（保持该行样式即可得到一致外观）
- 生成同类文件时：写产物目录 → repack → render 验收。
