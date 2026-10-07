//! 模板数据填充：把 `{{key}}` 占位符替换为 JSON 数据，生成新工作簿

use office_core::merge::{fill, fill_opt, lookup};

use crate::model::cell::Cell;
use crate::model::Workbook;

/// 按 JSON 数据替换工作簿内 `{{key}}` 占位符，返回替换次数。
///
/// 覆盖：单元格值/公式/批注/富文本 run、工作表页眉页脚。
pub fn merge_workbook(wb: &mut Workbook, data: &serde_json::Value) -> usize {
    let mut n = 0;
    for sheet in &mut wb.sheets {
        for row in &mut sheet.rows {
            for cell in &mut row.cells {
                merge_cell(cell, data, &mut n);
            }
        }
        if let Some(ps) = &mut sheet.print {
            for hf in [
                &mut ps.odd_header,
                &mut ps.odd_footer,
                &mut ps.even_header,
                &mut ps.even_footer,
                &mut ps.first_header,
                &mut ps.first_footer,
            ] {
                fill_opt(hf, data, &mut n);
            }
        }
    }
    n
}

fn merge_cell(cell: &mut Cell, data: &serde_json::Value, n: &mut usize) {
    if let Some(serde_json::Value::String(s)) = &cell.value {
        if let Some(replaced) = replace_only(s, data, n) {
            cell.value = Some(serde_json::Value::String(replaced));
        }
    }
    if let Some(f) = &mut cell.formula {
        *f = fill(f, data, n);
    }
    fill_opt(&mut cell.comment, data, n);
    for run in &mut cell.runs {
        run.text = fill(&run.text, data, n);
    }
}

/// 仅当确有命中时返回新字符串（避免给非字符串值引入变化）
fn replace_only(s: &str, data: &serde_json::Value, n: &mut usize) -> Option<String> {
    if !s.contains("{{") {
        return None;
    }
    let before = *n;
    let out = fill(s, data, n);
    if *n > before {
        Some(out)
    } else {
        None
    }
}

/// 供 MCP/CLI 校验数据键是否可解析（未使用的导出保留兼容）
#[allow(dead_code)]
pub(crate) fn has_key(data: &serde_json::Value, key: &str) -> bool {
    lookup(data, key).is_some()
}
