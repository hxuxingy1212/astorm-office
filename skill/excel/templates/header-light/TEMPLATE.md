# Excel 样式模板：cea27c2b_54288-ref

由 json2xlsx `template extract` 从存量 Excel 提取，供大模型据此生成同类风格的表格。

## 概览

- 工作表：MIDCON-PJM、SOCO-ENT、Midmarks
- 默认字体：`Arial` 10pt
- 默认行高：12.75pt
- 表头：`MIDCON-PJM` 第 3 行

## 配色（按出现频次）

| 颜色 | 用途 | 次数 |
|---|---|---|
| `000000` | font | 2606 |
| `FFFF00` | font | 2026 |
| `B7B7B7` | fill | 1234 |
| `C0C0C0` | fill | 918 |
| `FFFF00` | font | 51 |
| `CCFFCC` | fill | 37 |
| `CCFFFF` | fill | 25 |
| `3366FF` | fill | 19 |
| `FF0000` | fill | 17 |
| `38761D` | fill | 14 |
| `1155CC` | fill | 12 |
| `4A86E8` | fill | 7 |
| `FFFFFF` | fill | 5 |
| `99CC00` | fill | 3 |
| `99CCFF` | fill | 3 |
| `B3D580` | fill | 3 |

## 字体

| 字体 | 字号 | 加粗 | 颜色 | 次数 |
|---|---|---|---|---|
| Arial | 10pt | 是 | 000000 | 1010 |
| Arial | 10pt | 是 | 000000 | 1002 |
| Arial | 12pt | 是 | 000000 | 396 |
| Arial | 14pt | 是 | 000000 | 70 |
| Arial | 10pt | 否 | FFFF00 | 50 |
| Arial | 11pt | 是 | 000000 | 50 |
| Arial | 9pt | 是 | 000000 | 33 |
| Arial | 18pt | 是 | 000000 | 16 |
| Arial | 9pt | 是 | 000000 | 8 |
| Arial | 16pt | 是 | 000000 | 6 |
| Arial | 14pt | 否 | 000000 | 6 |
| Arial | 12pt | 是 | 000000 | 6 |
| Arial | 9pt | 否 | 000000 | 1 |
| Arial | 10pt | 否 | FFCC00 | 1 |
| Arial | 12pt | 否 | 000000 | 1 |
| Arial | 11pt | 是 | 000000 | 1 |
| Arial | 12pt | 是 | FFFF00 | 1 |

## 表头样式（JSON）

```json
{
  "font": {
    "bold": true,
    "name": "Arial",
    "size": 18.0,
    "color": "000000"
  },
  "fill": "FFFFFF"
}
```

表头列名：`2012-11-16`、``、``、``、``、``、``、``、``、``、``

## 数字格式

- `#,##0.00`
- `0.00`
- `m/d/yyyy;@`
- `mmm\-yy;@`

## 列布局（字符宽度）

1:16.5703125, 2:6.85546875, 3:-, 4:-, 5:5, 6:7.42578125, 7:6.42578125, 8:4.7109375, 9:6.85546875, 10:-, 11:4.7109375, 12:6.85546875, 13:-, 14:14, 15:7.7109375, 16:6.85546875

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

- 检测到表头：MIDCON-PJM 第 3 行（保持该行样式即可得到一致外观）
- 工作簿默认字体：Arial 10
- 生成同类文件时：写产物目录 → repack → render 验收。
