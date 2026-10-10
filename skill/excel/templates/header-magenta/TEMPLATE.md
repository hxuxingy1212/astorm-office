# Excel 样式模板：2_40809_input

由 json2xlsx `template extract` 从存量 Excel 提取，供大模型据此生成同类风格的表格。

## 概览

- 工作表：Malaga
- 默认字体：`等线` 12pt
- 默认行高：20.15pt
- 表头：`Malaga` 第 1 行

## 配色（按出现频次）

| 颜色 | 用途 | 次数 |
|---|---|---|
| `D569B6` | font | 949 |
| `8C021B` | font | 603 |
| `D569B6` | font | 32 |

## 字体

| 字体 | 字号 | 加粗 | 颜色 | 次数 |
|---|---|---|---|---|
| Arial Narrow | 16pt | 否 | 8C021B | 542 |
| Arial Narrow | 12pt | 否 | - | 90 |
| Arial Narrow | 14pt | 否 | - | 56 |
| Arial Narrow | 12pt | 否 | 8C021B | 50 |
| Arial Narrow | 11pt | 否 | - | 46 |
| Arial Narrow | 16pt | 否 | - | 46 |
| Arial Narrow | 14pt | 是 | - | 27 |
| Arial Narrow | 16pt | 否 | - | 26 |
| Arial Narrow | 10pt | 否 | - | 19 |
| Arial Narrow | 10pt | 是 | D569B6 | 14 |
| Arial Narrow | 24pt | 是 | - | 13 |
| Arial Narrow | 28pt | 是 | D569B6 | 13 |
| Arial Narrow | 20pt | 否 | - | 7 |
| Arial Narrow | 8pt | 否 | - | 7 |
| Arial Narrow | 20pt | 是 | 8C021B | 5 |
| Arial Narrow | 10pt | 否 | 8C021B | 4 |
| Arial Narrow | 16pt | 是 | - | 3 |
| Arial Narrow | 14pt | 否 | - | 3 |
| Arial Narrow | 16pt | 是 | D569B6 | 3 |
| Arial Narrow | 9pt | 否 | - | 2 |
| Arial Narrow | 14pt | 否 | 8C021B | 2 |
| Arial Narrow | 16pt | 否 | D569B6 | 1 |
| Arial Narrow | 8pt | 是 | D569B6 | 1 |

## 表头样式（JSON）

```json
{
  "font": {
    "bold": true,
    "name": "Arial Narrow",
    "size": 28.0,
    "color": "D569B6"
  },
  "fill": "D569B6"
}
```

表头列名：`Holiday in Malaga, Spain`、``、``、``、``、``、``、``、``、``、``、``、``、`=A1`、``、``、``、``、``、``、``、``

## 数字格式

- `"£"#,##0`
- `"£"#,##0.00`
- `"£"#,##0;[Red]\-"£"#,##0`
- `mm-dd-yy`

## 列布局（字符宽度）

1:4.59765625, 2:5.59765625, 3:24.59765625, 4:7.59765625, 5:7.59765625, 6:7.59765625, 7:3.59765625, 8:4.59765625, 9:5.59765625, 10:24.59765625, 11:7.59765625, 12:7.59765625, 13:7.59765625, 14:9, 15:19.59765625, 16:7.59765625, 17:7.59765625, 18:9, 19:9, 20:9, 21:9, 22:9, 23:9, 24:9, 25:9, 26:9, 27:9, 28:9, 29:9, 30:9, 31:9, 32:9, 33:9, 34:9, 35:9, 36:9

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

- 检测到表头：Malaga 第 1 行（保持该行样式即可得到一致外观）
- 工作簿默认字体：等线 12
- 生成同类文件时：写产物目录 → repack → render 验收。
