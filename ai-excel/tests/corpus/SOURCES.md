# 回归语料（tests/corpus）

精选自多个开源项目的 `.xlsx` 测试夹具，覆盖真实世界边界情况，供
`tests/corpus.rs` 做「解析 → 生成 → 再解析」回归。均为**精简小样本**（合计约 640KB）。

大规模语料（数千份）不入库，用脚本拉取到 `/tmp`：
`./scripts/collect_corpus.sh` → `/tmp/xlsx_more/`，然后 `./scripts/corpus_check.sh`。

## 样本清单

| 文件 | 来源项目 | 覆盖点 |
| --- | --- | --- |
| `phpspreadsheet__utf16be_nobom.xlsx` | PHPOffice/PhpSpreadsheet | 无 BOM 的 UTF-16BE 部件解码 |
| `phpspreadsheet__utf16be_bom.xlsx` | PHPOffice/PhpSpreadsheet | 带 BOM 的 UTF-16BE |
| `phpspreadsheet__backslash_paths.xlsx` | PHPOffice/PhpSpreadsheet | zip 条目使用 `\` 分隔符 |
| `phpspreadsheet__backslash_paths_win.xlsx` | PHPOffice/PhpSpreadsheet | Windows 目录分隔符 |
| `phpspreadsheet__root_level_workbook.xlsx` | PHPOffice/PhpSpreadsheet | workbook 位于根目录（无 `xl/` 前缀） |
| `phpspreadsheet__autofilter_basic.xlsx` | PHPOffice/PhpSpreadsheet | 自动筛选 |
| `calamine__root_package_minimal.xlsx` | tafia/calamine | 根级最小包（部件在根） |
| `calamine__shared_formula_reversed.xlsx` | tafia/calamine | 共享公式引用顺序异常 |
| `calamine__string_ref.xlsx` | tafia/calamine | 字符串引用 |
| `calamine__non_monotonic_si.xlsx` | tafia/calamine | 非单调 sharedStrings 索引 |
| `calamine__formula_issue.xlsx` | tafia/calamine | 公式边界 |
| `umya__indexed_color.xlsx` | MathNya/umya-spreadsheet | indexed 调色板（已知降级项） |
| `exceljs__date1904.xlsx` | exceljs/exceljs | 1904 日期系统 |
| `exceljs__date_issue.xlsx` | exceljs/exceljs | 日期序列值 |
| `exceljs__google_sheets_hidden.xlsx` | exceljs/exceljs | Google Sheets 导出 / 隐藏表 |
| `exceljs__many_columns.xlsx` | exceljs/exceljs | 多列（列范围） |
| `poiji__empty_cells.xlsx` | ozlerhakan/poiji | 空单元格 |
| `poiji__identical_headers.xlsx` | ozlerhakan/poiji | 重复/未知表头 |
| `poiji__number_format.xlsx` | ozlerhakan/poiji | 数字格式 |
| `libxlsxwriter__image_twocell.xlsx` | jmcnamara/libxlsxwriter | `twoCellAnchor` 图片（尺寸/定位） |
| `umya__charts_images_shapes.xlsx` | MathNya/umya-spreadsheet | 多 `twoCellAnchor` 图表+图片+形状 |
| `rust_xlsxwriter__chart_quoted_sheet.xlsx` | jmcnamara/rust_xlsxwriter | 图表引用含带引号/特殊字符的工作表名 |
| `rust_xlsxwriter__chart_union_range.xlsx` | jmcnamara/rust_xlsxwriter | 图表并集区间引用 `(r1,r2)` |
| `rust_xlsxwriter__table_x14_ext.xlsx` | jmcnamara/rust_xlsxwriter | 表格 `<extLst><x14:table>`（扩展元素不得覆盖主表属性） |
| `rust_xlsxwriter__background_with_comments.xlsx` | jmcnamara/rust_xlsxwriter | 工作表背景图 + 批注（sheet 关系 rId 唯一性） |
| `rust_xlsxwriter__comments_empty_cells.xlsx` | jmcnamara/rust_xlsxwriter | 空单元格上的批注（需补建 Cell）；遗留 VML 注释 |
| `poiji__image_jpeg.xlsx` | ozlerhakan/poiji | 图片部件扩展名保留（`.jpeg` 不改名为 `.jpg`） |

## 来源与许可

各样本版权归原作者，均取自其公开仓库的测试资源，遵循对应开源许可：

- PHPOffice/PhpSpreadsheet — MIT
- tafia/calamine — MIT
- MathNya/umya-spreadsheet — MIT
- exceljs/exceljs — MIT
- ozlerhakan/poiji — MIT

仅用于兼容性回归测试。
