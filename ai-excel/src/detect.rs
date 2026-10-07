//! 从数据区自动识别区域/表（detected table）。

use crate::model::sheet::Sheet;
use crate::utils::a1::{make_cell_ref, parse_cell_ref};

/// 工作表已用数据区的包围盒（"A1:C10"）。
pub fn detect_range(sheet: &Sheet) -> Option<String> {
    let (mut min_c, mut min_r, mut max_c, mut max_r) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut found = false;
    for row in &sheet.rows {
        for cell in &row.cells {
            if cell.value.is_none() && cell.formula.is_none() {
                continue;
            }
            if let Some((c, r)) = cell
                .reference
                .as_deref()
                .and_then(|x| parse_cell_ref(x).ok())
            {
                found = true;
                min_c = min_c.min(c);
                min_r = min_r.min(r);
                max_c = max_c.max(c);
                max_r = max_r.max(r);
            }
        }
    }
    if !found {
        return None;
    }
    Some(format!(
        "{}:{}",
        make_cell_ref(min_c, min_r),
        make_cell_ref(max_c, max_r)
    ))
}

/// 数据区首行的列名（缺失则 ColumnN）。
pub fn header_names(sheet: &Sheet, range: &str) -> Vec<String> {
    let Ok(((c1, r1), (c2, _))) = crate::utils::a1::parse_range(range) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for c in c1..=c2 {
        let name = sheet
            .rows
            .iter()
            .find(|row| row.index == Some(r1))
            .and_then(|row| {
                row.cells.iter().find(|cell| {
                    cell.reference
                        .as_deref()
                        .and_then(|x| parse_cell_ref(x).ok())
                        .map(|(cc, _)| cc == c)
                        .unwrap_or(false)
                })
            })
            .and_then(|cell| match &cell.value {
                Some(serde_json::Value::String(s)) => Some(s.clone()),
                Some(v) => Some(v.to_string()),
                None => None,
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("Column{}", c - c1 + 1));
        names.push(name);
    }
    names
}
