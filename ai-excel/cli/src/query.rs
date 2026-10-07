//! 元素查询：只读返回节点或匹配项（与 json2pptx 的 `query` 动词对齐）。
//!
//! 支持的选择器：
//! ```text
//! sheets                    # （缺省）列出全部工作表摘要
//! sheet[N]                  # 第 N 个工作表的完整 JSON
//! sheet[name=X]             # 按名（不区分大小写）取工作表
//! sheet:empty               # 没有任何单元格的工作表
//! cell:contains("T")        # 跨表查找值/公式包含 T 的单元格
//! ```

use serde_json::{json, Value};

use json2xlsx::model::Workbook;
use json2xlsx::utils::a1::{make_cell_ref, parse_cell_ref};
use office_core::CliError;

/// 本模块统一错误类型
type Result<T> = std::result::Result<T, CliError>;

/// 执行查询，返回 JSON 结果
pub fn run(wb: &Workbook, selector: &str) -> Result<Value> {
    let sel = selector.trim();
    if sel.is_empty() || sel == "sheets" || sel == "*" {
        return Ok(json!({
            "sheets": wb.sheets.iter().enumerate()
                .map(|(i, s)| json!({
                    "index": i + 1,
                    "name": s.name,
                    "rows": s.rows.len(),
                    "cells": s.rows.iter().map(|r| r.cells.len()).sum::<usize>(),
                }))
                .collect::<Vec<_>>(),
        }));
    }

    if let Some(rest) = sel.strip_prefix("sheet[") {
        let inner = rest
            .strip_suffix(']')
            .ok_or_else(|| CliError::new(format!("选择器缺少 ]: {sel}")))?;
        if let Ok(idx) = inner.parse::<usize>() {
            if idx == 0 {
                return Err(CliError::new("sheet 索引从 1 开始"));
            }
            let sheet = wb.sheets.get(idx - 1).ok_or_else(|| {
                CliError::new(format!(
                    "工作表索引越界: {idx}（共 {} 个）",
                    wb.sheets.len()
                ))
            })?;
            return Ok(serde_json::to_value(sheet)?);
        }
        if let Some(name) = inner.strip_prefix("name=") {
            let sheet = wb
                .sheets
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(name))
                .ok_or_else(|| CliError::new(format!("找不到工作表: {name}")))?;
            return Ok(serde_json::to_value(sheet)?);
        }
        return Err(CliError::new(format!("sheet[] 内支持索引或 name=X: {sel}")));
    }

    if sel == "sheet:empty" {
        return Ok(json!({
            "sheets": wb.sheets.iter().enumerate()
                .filter(|(_, s)| s.rows.iter().all(|r| r.cells.is_empty()))
                .map(|(i, s)| json!({ "index": i + 1, "name": s.name }))
                .collect::<Vec<_>>(),
        }));
    }

    if let Some(arg) = sel.strip_prefix("cell:contains(") {
        let needle = arg
            .strip_suffix(')')
            .ok_or_else(|| CliError::new(format!("cell:contains 缺少右括号: {sel}")))?
            .trim()
            .trim_matches('"');
        let mut hits = Vec::new();
        for (si, sheet) in wb.sheets.iter().enumerate() {
            for row in &sheet.rows {
                let row_idx = row.index.unwrap_or(0);
                let mut next_col: u32 = 1;
                for cell in &row.cells {
                    let col: u32 = match &cell.reference {
                        Some(r) => match parse_cell_ref(r) {
                            Ok((c, _)) => c,
                            Err(_) => next_col,
                        },
                        None => next_col,
                    };
                    next_col = col + 1;
                    let hay = match (&cell.value, &cell.formula) {
                        (Some(Value::String(s)), _) => s.clone(),
                        (Some(v), _) => v.to_string(),
                        (None, Some(f)) => f.clone(),
                        (None, None) => continue,
                    };
                    if !needle.is_empty() && hay.contains(needle) {
                        hits.push(json!({
                            "path": format!("/sheet[{}]/cell[{}]", si + 1, make_cell_ref(col, row_idx)),
                            "sheet": sheet.name,
                            "value": cell.value,
                            "formula": cell.formula,
                        }));
                    }
                }
            }
        }
        return Ok(json!({ "matches": hits.len(), "cells": hits }));
    }

    Err(CliError::new(format!("未知选择器: {sel}"))
        .suggest("可用: sheets / sheet[N] / sheet[name=X] / sheet:empty / cell:contains(\"文本\")"))
}
