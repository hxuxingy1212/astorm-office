//! Structured table (ListObject) OOXML generation.

use serde_json::Value;

use crate::error::Result;
use crate::model::sheet::{Sheet, Table};
use crate::utils::a1::{make_cell_ref, parse_cell_ref, parse_range};
use crate::utils::constants::NS_MAIN;
use crate::utils::xml::esc_xml;

/// Build a `xl/tables/tableN.xml` part. Returns `(file_name, xml)`.
pub fn table_xml(
    sheet: &Sheet,
    table: &Table,
    table_no: usize,
    table_id: usize,
) -> Result<(String, String)> {
    let ((c1, r1), (c2, r2)) = parse_range(&table.range)?;
    let header = table.show_header_row;
    let name = table
        .name
        .clone()
        .unwrap_or_else(|| format!("Table{table_no}"));

    let mut cols: Vec<String> = Vec::new();
    for c in c1..=c2 {
        let idx = (c - c1) as usize;
        let provided = table.columns.get(idx).cloned().filter(|s| !s.is_empty());
        let derived = if header {
            header_name(sheet, c, r1)
        } else {
            None
        };
        let mut n = provided
            .or(derived)
            .unwrap_or_else(|| format!("Column{}", idx + 1));
        while cols.contains(&n) {
            n.push('_');
        }
        cols.push(n);
    }

    let header_row_count = if header { 1 } else { 0 };
    let totals_attr = if table.totals_row {
        " totalsRowCount=\"1\""
    } else {
        ""
    };
    let insert_attr = if table.insert_row {
        " insertRow=\"1\""
    } else {
        ""
    };
    // 自动筛选区域不含汇总行。
    let filter_ref = if table.totals_row && r2 > r1 {
        format!("{}:{}", make_cell_ref(c1, r1), make_cell_ref(c2, r2 - 1))
    } else {
        table.range.clone()
    };
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    xml.push_str(&format!(
        "<table xmlns=\"{NS_MAIN}\" id=\"{table_id}\" name=\"{}\" displayName=\"{}\" ref=\"{}\" headerRowCount=\"{header_row_count}\"{totals_attr}{insert_attr}>",
        esc_xml(&name),
        esc_xml(&name),
        esc_xml(&table.range)
    ));
    xml.push_str(&format!("<autoFilter ref=\"{}\"/>", esc_xml(&filter_ref)));
    xml.push_str(&format!("<tableColumns count=\"{}\">", cols.len()));
    for (i, n) in cols.iter().enumerate() {
        let total = table.totals.get(i);
        let calc = table.calculated_formulas.get(i).filter(|s| !s.is_empty());
        let mut attrs = String::new();
        if let Some(t) = total {
            if let Some(l) = &t.label {
                attrs.push_str(&format!(" totalsRowLabel=\"{}\"", esc_xml(l)));
            }
            if let Some(f) = &t.function {
                attrs.push_str(&format!(" totalsRowFunction=\"{}\"", esc_xml(f)));
            }
        }
        let has_children = total.and_then(|t| t.formula.as_ref()).is_some() || calc.is_some();
        if has_children {
            xml.push_str(&format!(
                "<tableColumn id=\"{}\" name=\"{}\"{attrs}>",
                i + 1,
                esc_xml(n)
            ));
            if let Some(c) = calc {
                xml.push_str(&format!(
                    "<calculatedColumnFormula>{}</calculatedColumnFormula>",
                    esc_xml(c)
                ));
            }
            if let Some(f) = total.and_then(|t| t.formula.as_ref()) {
                xml.push_str(&format!(
                    "<totalsRowFormula>{}</totalsRowFormula>",
                    esc_xml(f)
                ));
            }
            xml.push_str("</tableColumn>");
        } else {
            xml.push_str(&format!(
                "<tableColumn id=\"{}\" name=\"{}\"{attrs}/>",
                i + 1,
                esc_xml(n)
            ));
        }
    }
    xml.push_str("</tableColumns>");
    let style_attr = match &table.style {
        Some(s) => format!(" name=\"{}\"", esc_xml(s)),
        None => String::new(),
    };
    xml.push_str(&format!(
        "<tableStyleInfo{style_attr} showFirstColumn=\"{}\" showLastColumn=\"{}\" showRowStripes=\"{}\" showColumnStripes=\"{}\"/>",
        b01(table.show_first_column),
        b01(table.show_last_column),
        b01(table.show_banded_rows),
        b01(table.show_banded_columns)
    ));
    xml.push_str("</table>");

    Ok((format!("table{table_no}.xml"), xml))
}

fn header_name(sheet: &Sheet, col: u32, row: u32) -> Option<String> {
    for r in &sheet.rows {
        if r.index != Some(row) {
            continue;
        }
        for cell in &r.cells {
            if let Some((cc, _)) = cell
                .reference
                .as_deref()
                .and_then(|x| parse_cell_ref(x).ok())
            {
                if cc == col {
                    let s = match &cell.value {
                        Some(Value::String(s)) => s.clone(),
                        Some(v) => v.to_string(),
                        None => String::new(),
                    };
                    if !s.is_empty() {
                        return Some(s);
                    }
                }
            }
        }
    }
    None
}

fn b01(b: bool) -> &'static str {
    if b {
        "1"
    } else {
        "0"
    }
}
