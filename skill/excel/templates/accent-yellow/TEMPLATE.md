# Excel 样式模板：autofilter-colors

由 json2xlsx `template extract` 从存量 Excel 提取，供大模型据此生成同类风格的表格。

## 概览

- 工作表：Sheet1
- 默认字体：`Liberation Sans` 11pt
- 默认行高：14.25pt
- 表头：`Sheet1` 第 1 行

## 配色（按出现频次）

| 颜色 | 用途 | 次数 |
|---|---|---|
| `808080` | font | 4 |
| `B3CAC7` | fill | 4 |
| `B7B3CA` | fill | 4 |
| `E8F2A1` | fill | 4 |
| `FFD7D7` | fill | 4 |
| `3465A4` | font | 3 |
| `FFFFFF` | font | 3 |
| `00A933` | font | 2 |
| `D4EA6B` | fill | 2 |
| `FF0000` | font | 2 |
| `FFBF00` | font | 2 |
| `FFDBB6` | fill | 2 |

## 字体

| 字体 | 字号 | 加粗 | 颜色 | 次数 |
|---|---|---|---|---|
| Liberation Sans | 11pt | 否 | 808080 | 4 |
| Liberation Sans | 11pt | 否 | FFFFFF | 3 |
| Liberation Sans | 11pt | 是 | - | 3 |
| Liberation Sans | 11pt | 否 | 3465A4 | 3 |
| Liberation Sans | 11pt | 否 | FF0000 | 2 |
| Liberation Sans | 11pt | 否 | 00A933 | 2 |
| Liberation Sans | 11pt | 否 | FFBF00 | 2 |
| Liberation Sans | 11pt | 否 | - | 1 |

## 表头样式（JSON）

```json
{
  "font": {
    "bold": true,
    "name": "Liberation Sans",
    "size": 11.0
  }
}
```

表头列名：`Background`、`Foreground`、`Both`

## 列布局（字符宽度）

1:10.625, 2:10.625, 3:10.625

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

- 检测到表头：Sheet1 第 1 行（保持该行样式即可得到一致外观）
- 工作簿默认字体：Liberation Sans 11
- 生成同类文件时：写产物目录 → repack → render 验收。
