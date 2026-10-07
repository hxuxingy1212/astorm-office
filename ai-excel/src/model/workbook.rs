use schemars::JsonSchema;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::cell::{Cell, CellStyleRef};
use super::is_false;
use super::sheet::Sheet;
use super::style::{Font, StyleDef};
use crate::utils::a1::{make_cell_ref, parse_cell_ref};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct NamedRange {
    pub name: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Workbook {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub date1904: bool,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub styles: HashMap<String, StyleDef>,
    /// Workbook default font (styles.xml `fonts[0]`); Calibri 11 when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_font: Option<Font>,
    /// Active sheet index (0-based) in the workbook view.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tab: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named_ranges: Vec<NamedRange>,
    pub sheets: Vec<Sheet>,
    /// 原样保留的 `xl/theme/theme1.xml`（形状/图表里的 `schemeClr` 依赖它）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
}

impl Workbook {
    /// Return a canonical copy with refs, row indices, cell types and date
    /// number formats filled in. `parse` always emits fully-specified output,
    /// so tests compare against `normalized()`.
    pub fn normalized(&self) -> Workbook {
        let mut wb = self.clone();
        if wb
            .default_font
            .as_ref()
            .map(is_calibri_default)
            .unwrap_or(true)
        {
            wb.default_font = None;
        }
        for sheet in &mut wb.sheets {
            if sheet.direction.as_deref() == Some("ltr") {
                sheet.direction = None;
            }
            let clear_freeze =
                matches!(&sheet.freeze, Some(f) if matches!(parse_cell_ref(f), Ok((1, 1))));
            if clear_freeze {
                sheet.freeze = None;
            }
            for (ri, row) in sheet.rows.iter_mut().enumerate() {
                if row.index.is_none() {
                    row.index = Some((ri + 1) as u32);
                }
                let row_idx = row.index.unwrap();
                let mut next_col: u32 = 1;
                for cell in &mut row.cells {
                    let col = match &cell.reference {
                        Some(r) => {
                            let (col, _) = parse_cell_ref(r).unwrap_or((next_col, row_idx));
                            col
                        }
                        None => {
                            cell.reference = Some(make_cell_ref(next_col, row_idx));
                            next_col
                        }
                    };
                    next_col = col + 1;
                    if cell.cell_type.is_none() {
                        cell.cell_type = cell.infer_type();
                    }
                    if cell.cell_type.as_deref() == Some("date") && cell.number_format.is_none() {
                        cell.number_format = Some("yyyy-mm-dd".into());
                    }
                    if let Some(CellStyleRef::Inline(s)) = &mut cell.style {
                        if let Some(b) = &mut s.border {
                            b.expand();
                        }
                    }
                }
            }
        }
        wb
    }
}

/// Best-effort value of a cell as a display string (used by view/render).
pub fn cell_display(cell: &Cell) -> String {
    if let Some(f) = &cell.formula {
        return format!("={f}");
    }
    match &cell.value {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

/// Convenience: resolve a named style against the workbook table.
pub fn resolve_style<'a>(wb: &'a Workbook, cell: &'a Cell) -> Option<StyleDef> {
    match &cell.style {
        Some(CellStyleRef::Named(name)) => wb.styles.get(name).cloned(),
        Some(CellStyleRef::Inline(s)) => Some(s.clone()),
        None => None,
    }
}

/// Whether a font is the implicit Calibri 11 default (so it need not be stored).
pub(crate) fn is_calibri_default(f: &Font) -> bool {
    f.name.as_deref() == Some("Calibri")
        && f.size == Some(11.0)
        && !f.bold
        && !f.italic
        && !f.strike
        && f.underline.is_none()
        && f.color.is_none()
}
