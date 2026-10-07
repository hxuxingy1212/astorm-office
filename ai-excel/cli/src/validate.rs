//! 结构校验：对解析出的 Workbook 做静态检查，输出问题列表（JSON）。
//!
//! 与 json2pptx 的 `validate` 对齐：只读、无副作用，问题非空时 CLI 以
//! 退出码 3（EXIT_ISSUES）结束。

use serde_json::{json, Value};
use std::collections::HashSet;

use json2xlsx::model::cell::{Cell, CellStyleRef};
use json2xlsx::model::Workbook;
use json2xlsx::utils::a1::parse_cell_ref;

/// 执行校验，返回问题列表（元素形如 `{"path": ..., "issue": ...}`）
pub fn run(wb: &Workbook) -> Vec<Value> {
    let mut issues = Vec::new();

    let mut seen_names: HashSet<String> = HashSet::new();
    for (si, sheet) in wb.sheets.iter().enumerate() {
        let sp = format!("/sheet[{}]", si + 1);
        if sheet.name.trim().is_empty() {
            issues.push(json!({ "path": sp, "issue": "工作表名为空" }));
        }
        if !seen_names.insert(sheet.name.to_lowercase()) {
            issues.push(json!({ "path": sp, "issue": format!("工作表名重复: {}", sheet.name) }));
        }
        if let Some(f) = &sheet.freeze {
            if parse_cell_ref(f).is_err() {
                issues.push(json!({ "path": sp, "issue": format!("冻结窗格引用非法: {f}") }));
            }
        }
        if let Some(range) = &sheet.auto_filter {
            if !range.contains(':') {
                issues.push(json!({ "path": sp, "issue": format!("auto_filter 应为区域引用（A1:B2）: {range}") }));
            }
        }

        for (ri, row) in sheet.rows.iter().enumerate() {
            let row_idx = row.index.unwrap_or((ri + 1) as u32);
            let rp = format!("{sp}/row[{}]", row_idx);
            if row_idx == 0 {
                issues.push(json!({ "path": rp, "issue": "行号必须 ≥ 1" }));
            }
            for cell in &row.cells {
                check_cell(wb, &sp, row_idx, cell, &mut issues);
            }
        }
    }

    for nr in &wb.named_ranges {
        if nr.reference.trim().is_empty() {
            issues.push(json!({
                "path": format!("/named_range[{}]", nr.name),
                "issue": format!("命名范围 {} 的引用为空", nr.name),
            }));
        }
    }

    issues
}

fn check_cell(wb: &Workbook, sheet_path: &str, row_idx: u32, cell: &Cell, issues: &mut Vec<Value>) {
    let reference = cell.reference.clone().unwrap_or_else(|| "?".to_string());
    let path = format!("{sheet_path}/cell[{reference}]（row {row_idx}）");

    if cell.reference.is_some() {
        if let Err(e) = parse_cell_ref(&reference) {
            issues.push(json!({ "path": path, "issue": format!("单元格引用非法: {e}") }));
            return;
        }
    }
    if let Some(f) = &cell.formula {
        if f.trim_start().starts_with('=') {
            issues.push(json!({
                "path": path,
                "issue": "公式不应以 = 开头（模型约定公式不带 =，生成端会自动补）",
            }));
        }
    }
    if let Some(CellStyleRef::Named(name)) = &cell.style {
        if !wb.styles.contains_key(name) {
            issues.push(json!({ "path": path, "issue": format!("未定义的命名样式: {name}") }));
        }
    }
}
