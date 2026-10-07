//! Style-template extraction: distill an existing workbook into a reusable
//! style template (palette, fonts, number formats, header style, layout) plus
//! an LLM-facing Markdown description and a fill-in skeleton workbook.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::model::style::StyleDef;
use crate::model::workbook::{cell_display, resolve_style, Workbook};
use crate::model::{Cell, Sheet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaletteColor {
    pub hex: String,
    pub count: u32,
    /// `fill` | `font` | `tab`.
    pub usage: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FontUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ColumnSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeaderSpec {
    pub sheet: String,
    pub row: u32,
    pub style: StyleDef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Template {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_font: Option<crate::model::style::Font>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub palette: Vec<PaletteColor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<FontUsage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub number_formats: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header: Option<HeaderSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<ColumnSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_col_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_row_height: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tab_colors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sheets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// Filled-in attributes of a style (ignoring unset ones) so styles can be
/// counted and compared.
fn style_fingerprint(s: &StyleDef) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        serde_json::to_string(s).ok()
    }
}

/// Extract a reusable style template from a workbook.
pub fn extract_template(wb: &Workbook, name: &str) -> Template {
    let mut num_formats: HashMap<String, u32> = HashMap::new();
    let mut fills: HashMap<String, u32> = HashMap::new();
    let mut font_colors: HashMap<String, u32> = HashMap::new();
    let mut tabs: Vec<String> = Vec::new();
    let mut fonts_seen: HashMap<String, FontUsage> = HashMap::new();
    let mut sheets = Vec::new();

    for sheet in &wb.sheets {
        sheets.push(sheet.name.clone());
        if let Some(c) = &sheet.tab_color {
            let h = normalize(c);
            if !tabs.contains(&h) {
                tabs.push(h);
            }
        }
        for row in &sheet.rows {
            for cell in &row.cells {
                if let Some(nf) = &cell.number_format {
                    *num_formats.entry(nf.clone()).or_insert(0) += 1;
                }
                let Some(s) = resolve_style(wb, cell) else {
                    continue;
                };
                if let Some(fill) = &s.fill {
                    *fills.entry(normalize(fill)).or_insert(0) += 1;
                }
                if let Some(font) = &s.font {
                    if let Some(c) = &font.color {
                        *font_colors.entry(normalize(c)).or_insert(0) += 1;
                    }
                    let key = style_fingerprint(&StyleDef {
                        font: Some(font.clone()),
                        ..Default::default()
                    })
                    .unwrap_or_default();
                    let entry = fonts_seen.entry(key).or_insert(FontUsage {
                        name: font.name.clone(),
                        size: font.size,
                        bold: font.bold,
                        color: font.color.clone().map(|c| normalize(&c)),
                        count: 0,
                    });
                    entry.count += 1;
                }
                if let Some(nf) = &s.number_format {
                    *num_formats.entry(nf.clone()).or_insert(0) += 1;
                }
            }
        }
    }

    // palette
    let mut palette: Vec<PaletteColor> = Vec::new();
    for (hex, count) in fills.iter().chain(font_colors.iter()) {
        let usage = if fills.contains_key(hex) && !font_colors.contains_key(hex) {
            "fill"
        } else if font_colors.contains_key(hex) {
            "font"
        } else {
            "fill"
        };
        palette.push(PaletteColor {
            hex: hex.clone(),
            count: *count,
            usage: usage.into(),
        });
    }
    palette.sort_by(|a, b| b.count.cmp(&a.count).then(a.hex.cmp(&b.hex)));
    palette.truncate(16);

    let mut fonts: Vec<FontUsage> = fonts_seen.into_values().collect();
    fonts.sort_by_key(|f| std::cmp::Reverse(f.count));

    let mut nf: Vec<String> = num_formats.keys().cloned().collect();
    nf.sort();

    // header detection: first row (<=5) where >= half the cells are bold/filled
    let header = detect_header(wb);

    // columns / defaults from the sheet with the most cells
    let (columns, default_col_width, default_row_height) = primary_layout(wb);

    let mut notes = Vec::new();
    if let Some(h) = &header {
        notes.push(format!(
            "检测到表头：{} 第 {} 行（保持该行样式即可得到一致外观）",
            h.sheet, h.row
        ));
    }
    if let Some(df) = &wb.default_font {
        notes.push(format!(
            "工作簿默认字体：{} {}",
            df.name.clone().unwrap_or_default(),
            df.size.map(|s| s.to_string()).unwrap_or_default()
        ));
    }
    notes.push("生成同类文件时：写产物目录 → repack → render 验收。".into());

    Template {
        name: name.to_string(),
        source: None,
        default_font: wb.default_font.clone(),
        palette,
        fonts,
        number_formats: nf,
        header,
        columns,
        default_col_width,
        default_row_height,
        tab_colors: tabs,
        sheets,
        notes,
    }
}

fn detect_header(wb: &Workbook) -> Option<HeaderSpec> {
    for sheet in &wb.sheets {
        let mut rows: Vec<&crate::model::Row> = sheet.rows.iter().collect();
        rows.sort_by_key(|r| r.index.unwrap_or(0));
        for row in rows {
            let idx = row.index.unwrap_or(0);
            if idx == 0 || idx > 5 {
                continue;
            }
            let total = row.cells.len();
            if total == 0 {
                continue;
            }
            let styled = row
                .cells
                .iter()
                .filter(|c| {
                    resolve_style(wb, c)
                        .map(|s| {
                            s.fill.is_some() || s.font.as_ref().map(|f| f.bold).unwrap_or(false)
                        })
                        .unwrap_or(false)
                })
                .count();
            if styled * 2 >= total {
                let mut style = StyleDef::default();
                let mut values = Vec::new();
                for c in &row.cells {
                    if let Some(s) = resolve_style(wb, c) {
                        if s.fill.is_some() && style.fill.is_none() {
                            style.fill = s.fill.clone();
                        }
                        if let Some(f) = &s.font {
                            let e = style.font.get_or_insert_with(Default::default);
                            if f.bold {
                                e.bold = true;
                            }
                            if e.color.is_none() {
                                e.color = f.color.clone();
                            }
                            if e.name.is_none() {
                                e.name = f.name.clone();
                            }
                            if e.size.is_none() {
                                e.size = f.size;
                            }
                        }
                    }
                    values.push(cell_display(c));
                }
                return Some(HeaderSpec {
                    sheet: sheet.name.clone(),
                    row: idx,
                    style,
                    values,
                });
            }
        }
    }
    None
}

fn primary_layout(wb: &Workbook) -> (Vec<ColumnSpec>, Option<f64>, Option<f64>) {
    let mut best: Option<&Sheet> = None;
    let mut best_cells = 0usize;
    for sheet in &wb.sheets {
        let n: usize = sheet.rows.iter().map(|r| r.cells.len()).sum();
        if n > best_cells {
            best_cells = n;
            best = Some(sheet);
        }
    }
    let Some(sheet) = best else {
        return (Vec::new(), None, None);
    };
    let columns = sheet
        .columns
        .iter()
        .map(|c| ColumnSpec {
            width: c.width,
            hidden: c.hidden,
        })
        .collect();
    (columns, sheet.default_col_width, sheet.default_row_height)
}

/// Produce a skeleton workbook that keeps the header row and all styling but
/// clears data values, ready for the model to fill in.
pub fn skeleton_workbook(wb: &Workbook, header_row: u32) -> Workbook {
    let mut out = wb.clone();
    for sheet in &mut out.sheets {
        for row in &mut sheet.rows {
            let idx = row.index.unwrap_or(0);
            if idx <= header_row {
                continue;
            }
            for cell in &mut row.cells {
                cell.value = None;
                cell.formula = None;
                cell.cell_type = None;
                cell.comment = None;
                cell.link = None;
                cell.link_tooltip = None;
                cell.runs.clear();
            }
        }
    }
    out
}

/// Render an LLM-facing Markdown description of the template.
pub fn render_markdown(t: &Template) -> String {
    let mut md = String::new();
    md.push_str(&format!("# Excel 样式模板：{}\n\n", t.name));
    md.push_str(
        "由 json2xlsx `template extract` 从存量 Excel 提取，供大模型据此生成同类风格的表格。\n\n",
    );

    md.push_str("## 概览\n\n");
    md.push_str(&format!("- 工作表：{}\n", t.sheets.join("、")));
    if let Some(df) = &t.default_font {
        md.push_str(&format!(
            "- 默认字体：`{}` {}pt\n",
            df.name.clone().unwrap_or_default(),
            df.size.unwrap_or(11.0)
        ));
    }
    if let Some(dr) = t.default_row_height {
        md.push_str(&format!("- 默认行高：{dr}pt\n"));
    }
    if let Some(h) = &t.header {
        md.push_str(&format!("- 表头：`{}` 第 {} 行\n", h.sheet, h.row));
    }
    md.push('\n');

    md.push_str("## 配色（按出现频次）\n\n");
    md.push_str("| 颜色 | 用途 | 次数 |\n|---|---|---|\n");
    for p in &t.palette {
        md.push_str(&format!(
            "| `{hex}` | {usage} | {count} |\n",
            hex = p.hex,
            usage = p.usage,
            count = p.count
        ));
    }
    md.push('\n');

    if !t.fonts.is_empty() {
        md.push_str("## 字体\n\n| 字体 | 字号 | 加粗 | 颜色 | 次数 |\n|---|---|---|---|---|\n");
        for f in &t.fonts {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                f.name.clone().unwrap_or_else(|| "-".into()),
                f.size
                    .map(|s| format!("{s}pt"))
                    .unwrap_or_else(|| "-".into()),
                if f.bold { "是" } else { "否" },
                f.color.clone().unwrap_or_else(|| "-".into()),
                f.count
            ));
        }
        md.push('\n');
    }

    if let Some(h) = &t.header {
        md.push_str("## 表头样式（JSON）\n\n```json\n");
        md.push_str(&format!(
            "{}\n",
            serde_json::to_string_pretty(&h.style).unwrap_or_default()
        ));
        md.push_str("```\n\n");
        if !h.values.is_empty() {
            md.push_str(&format!("表头列名：`{}`\n\n", h.values.join("`、`")));
        }
    }

    if !t.number_formats.is_empty() {
        md.push_str("## 数字格式\n\n");
        for nf in &t.number_formats {
            md.push_str(&format!("- `{nf}`\n"));
        }
        md.push('\n');
    }

    if !t.columns.is_empty() {
        md.push_str("## 列布局（字符宽度）\n\n");
        let widths: Vec<String> = t
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let w = c
                    .width
                    .map(|w| format!("{w}"))
                    .unwrap_or_else(|| "-".into());
                if c.hidden {
                    format!("{}:{w}(隐藏)", i + 1)
                } else {
                    format!("{}:{w}", i + 1)
                }
            })
            .collect();
        md.push_str(&format!("{}\n\n", widths.join(", ")));
    }

    md.push_str("## 如何生成同类文件\n\n");
    md.push_str("1. 复制 `skeleton/` 产物目录（已保留表头与全部样式，数据清空）。\n");
    md.push_str("2. 按业务填写 `xl/worksheets/sheetN.json` 的数据行（表头样式可引用 `template.json.header.style`）。\n");
    md.push_str("3. 用本工具重建并验收：\n\n```bash\n");
    md.push_str("json2xlsx repack skeleton/ -o out.xlsx\n");
    md.push_str("json2xlsx render skeleton/ -o preview.html\n```\n\n");

    md.push_str("## 模板 JSON\n\n完整结构化模板见 `template.json`（含 `palette` / `fonts` / `header` / `columns` / `number_formats`），可直接被生成流程引用。\n");

    md.push_str("\n## 使用建议（最佳实践）\n\n");
    md.push_str("1. **中文/字体**：本模板默认字体若为拉丁字体（如 Calibri），生成含中文的内容时请给表头与数据 `font.name` 指定中文字体（如 `微软雅黑`、`PingFang SC`、`等线`），或设置工作簿 `default_font`；否则部分环境（如 LibreOffice 无 CJK 回退）会显示空白。\n");
    md.push_str("2. **数字格式**：金额用 `#,##0`/`#,##0.00`，比率用 `0.0%`，日期用 `yyyy-mm-dd`；务必显式设置，避免裸浮点（否则会出现 `13.100000000000001%`）。\n");
    md.push_str("3. **对齐**：数值列右对齐、文本列左对齐、表头居中。\n");
    md.push_str("4. **强调/合计行**：加粗 + 顶部细边框；深色填充配浅色文字。\n");
    md.push_str("5. **列宽**：按内容显式设置（中文按约 2 个字符宽估算），避免内容被截断。\n");
    md.push_str("6. **图表**：放在数据右侧或下方；需要打印时设置 `print_area`，避免图表/宽表把内容挤到多页。\n");
    md.push_str("7. **验收**：生成后务必用 `render` 目视核对（HTML 预览含合并单元格与图表；LibreOffice headless 对中文可能无回退字体）。\n");
    if !t.notes.is_empty() {
        md.push_str("\n## 备注\n\n");
        for n in &t.notes {
            md.push_str(&format!("- {n}\n"));
        }
    }
    md
}

/// Normalize a color to 6-hex uppercase.
pub fn normalize(c: &str) -> String {
    let s = c.trim().trim_start_matches('#').to_ascii_uppercase();
    if s.len() == 8 {
        s[2..].to_string()
    } else {
        s
    }
}

/// Convenience: count non-empty cells in a sheet (used by callers).
pub fn non_empty_cells(sheet: &Sheet) -> usize {
    sheet
        .rows
        .iter()
        .flat_map(|r| &r.cells)
        .filter(|c: &&Cell| c.value.is_some() || c.formula.is_some())
        .count()
}

/// Write a template bundle to `dir`: `template.json`, `TEMPLATE.md`, and a
/// `skeleton/` product directory (header + styles kept, data cleared).
pub fn write_template_dir(
    wb: &Workbook,
    dir: &Path,
    name: &str,
    source: Option<&str>,
) -> Result<Template> {
    std::fs::create_dir_all(dir)?;
    let mut t = extract_template(wb, name);
    t.source = source.map(|s| s.to_string());

    std::fs::write(dir.join("template.json"), serde_json::to_string_pretty(&t)?)?;
    std::fs::write(dir.join("TEMPLATE.md"), render_markdown(&t))?;

    let header_row = t.header.as_ref().map(|h| h.row).unwrap_or(1);
    let skeleton = skeleton_workbook(wb, header_row);
    crate::product::save_product(&skeleton, &dir.join("skeleton"))?;

    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::cell::CellStyleRef;
    use crate::model::style::Font;
    use crate::model::{Cell, Column, Row, Sheet};
    use serde_json::json;

    #[test]
    fn extract_and_skeleton() {
        let header = StyleDef {
            font: Some(Font {
                bold: true,
                color: Some("FFFFFF".into()),
                ..Default::default()
            }),
            fill: Some("4472C4".into()),
            ..Default::default()
        };
        let wb = Workbook {
            sheets: vec![Sheet {
                name: "S".into(),
                tab_color: Some("22AA22".into()),
                columns: vec![Column {
                    width: Some(20.0),
                    ..Default::default()
                }],
                rows: vec![
                    Row {
                        index: Some(1),
                        cells: vec![
                            Cell {
                                reference: Some("A1".into()),
                                value: Some(json!("Name")),
                                style: Some(CellStyleRef::Inline(header.clone())),
                                ..Default::default()
                            },
                            Cell {
                                reference: Some("B1".into()),
                                value: Some(json!("Value")),
                                style: Some(CellStyleRef::Inline(header.clone())),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    },
                    Row {
                        index: Some(2),
                        cells: vec![
                            Cell {
                                reference: Some("A2".into()),
                                value: Some(json!("x")),
                                ..Default::default()
                            },
                            Cell {
                                reference: Some("B2".into()),
                                value: Some(json!(42)),
                                number_format: Some("#,##0".into()),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let t = extract_template(&wb, "t");
        assert_eq!(t.header.as_ref().unwrap().row, 1);
        assert_eq!(t.header.as_ref().unwrap().values, vec!["Name", "Value"]);
        assert!(t.palette.iter().any(|p| p.hex == "4472C4"));
        assert!(t.number_formats.contains(&"#,##0".to_string()));
        assert_eq!(t.columns.len(), 1);
        assert!(t.tab_colors.contains(&"22AA22".to_string()));

        let sk = skeleton_workbook(&wb, 1);
        assert_eq!(sk.sheets[0].rows[0].cells[0].value, Some(json!("Name")));
        assert_eq!(sk.sheets[0].rows[1].cells[0].value, None);
        assert_eq!(sk.sheets[0].rows[1].cells[1].value, None);
        assert_eq!(
            sk.sheets[0].rows[1].cells[1].number_format,
            Some("#,##0".into())
        );

        let md = render_markdown(&t);
        assert!(md.contains("4472C4"));
        assert!(md.contains("Name"));
    }
}
