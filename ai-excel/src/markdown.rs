//! 工作表 / 工作簿 → Markdown（结构感知的纯文本视图）
//!
//! 供 `view <文件> [路径] text` 使用：已用区域渲染成 GFM 表格，
//! 单元格内容取 `cell_display`（有公式时显示 `=公式`，与屏幕上的公式视图一致）。
//! 整本工作簿时每张表一个 `## 表名` 小节。输出是**纯 Markdown**，不带注释标记。
//!
//! 设计取向：只表达结构（行列网格），不表达样式（字体/颜色/列宽/合并区属于
//! layout 视图）。超大区域（> 20000 格）自动退回逐行稀疏列表，避免生成
//! 百万行的表格。

use crate::model::sheet::Sheet;
use crate::model::workbook::Workbook;
use crate::utils::a1::{index_to_col, parse_cell_ref};
use office_core::attachment::file_name_of;
use office_core::PayloadSinks;

/// 超过这个格子数就不再画表格（稀疏列表更省）
const MAX_TABLE_CELLS: usize = 20_000;

/// 单张工作表 → Markdown（已用区域；空表返回空串）
///
/// 全表是主路径：**直接从 rows 拼**（不建 `(行,列) → String` 的 HashMap，
/// 也不给每个格子分配字符串），与 `window_from_grid` 的输出逐字节一致；
/// 只有超大区域（> 20000 格）才回落到稀疏形态那条老路。
pub fn sheet_markdown(sheet: &Sheet) -> String {
    sheet_markdown_with(sheet, PayloadSinks::default())
}

/// 单张工作表 → Markdown（带附件导出：OLE 对象落到指定目录，引用指向导出后的路径）
pub fn sheet_markdown_with(sheet: &Sheet, sinks: PayloadSinks<'_>) -> String {
    let mut out = match used_range_direct(sheet) {
        Some((c1, r1, c2, r2)) => render_window(sheet, c1, r1, c2, r2),
        None => String::new(),
    };
    // 内嵌对象（OLE）：只有带 --attachments（sink）时才渲染——默认输出保持不变
    if let Some(sink) = sinks.attachments {
        for ole in &sheet.ole_objects {
            let name = file_name_of(&ole.src).to_string();
            let link = sink.link_for(&name, &ole.src);
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("[{name}]({link})（附件 · {}）", ole.prog_id));
        }
    }
    out
}

/// 窗口 → Markdown：格子数在上限内画表格，超限退化成逐行稀疏列表。
/// 两种形态都**直接从 rows 拼**（不建 `(行,列)` 映射、不给每格分配字符串）。
fn render_window(sheet: &Sheet, c1: u32, r1: u32, c2: u32, r2: u32) -> String {
    let cells = (c2 - c1 + 1) as usize * (r2 - r1 + 1) as usize;
    if cells > MAX_TABLE_CELLS {
        sparse_direct(sheet, c1, r1, c2, r2)
    } else {
        table_direct(sheet, c1, r1, c2, r2)
    }
}

/// 已用区域（无分配扫描；判空只看字段，不生成显示串）
fn used_range_direct(sheet: &Sheet) -> Option<(u32, u32, u32, u32)> {
    let mut c1 = u32::MAX;
    let mut r1 = u32::MAX;
    let mut c2 = 0u32;
    let mut r2 = 0u32;
    let mut any = false;
    for (ri, row) in sheet.rows.iter().enumerate() {
        let r = row.index.unwrap_or(ri as u32 + 1);
        for (ci, cell) in row.cells.iter().enumerate() {
            if !has_display(cell) {
                continue;
            }
            let c = col_of(cell, ci);
            any = true;
            c1 = c1.min(c);
            r1 = r1.min(r);
            c2 = c2.max(c);
            r2 = r2.max(r);
        }
    }
    any.then_some((c1, r1, c2, r2))
}

/// 有内容可显示？（与 `cell_display` 的判空一致，但不分配字符串）
fn has_display(cell: &crate::model::cell::Cell) -> bool {
    if cell.formula.is_some() {
        return true;
    }
    match &cell.value {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// 单元格列号（优先取 ref，否则按行内序号）
fn col_of(cell: &crate::model::cell::Cell, ci: usize) -> u32 {
    cell.reference
        .as_deref()
        .and_then(|x| parse_cell_ref(x).ok())
        .map(|(c, _)| c)
        .unwrap_or(ci as u32 + 1)
}

/// 全表表格直出：行序扫描 + 列位填充，**逐格零分配**（显示文本直接写进输出缓冲）
fn table_direct(sheet: &Sheet, c1: u32, r1: u32, c2: u32, r2: u32) -> String {
    let cols = (c2 - c1 + 1) as usize;
    // 行按索引升序（与 grid 版一致：同索引后来者覆盖）
    let mut rows: Vec<(u32, &crate::model::sheet::Row)> = sheet
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| (row.index.unwrap_or(i as u32 + 1), row))
        .collect();
    rows.sort_by_key(|(r, _)| *r);

    let mut out = String::with_capacity(cols * (r2 - r1 + 1) as usize * 6 + 32);
    let mut cursor = 0usize;
    for r in r1..=r2 {
        // 跳过更早的行；同索引的连续行取最后一个（grid 的覆盖语义）
        let mut row: Option<&crate::model::sheet::Row> = None;
        while cursor < rows.len() {
            let (idx, cand) = rows[cursor];
            if idx < r {
                cursor += 1;
                continue;
            }
            if idx == r {
                row = Some(cand);
                cursor += 1;
                continue;
            }
            break;
        }
        // 该行要写的格子：窗口内且有内容，按列升序
        let mut items: Vec<(u32, &crate::model::cell::Cell)> = Vec::new();
        if let Some(row) = row {
            for (ci, cell) in row.cells.iter().enumerate() {
                if !has_display(cell) {
                    continue;
                }
                let c = col_of(cell, ci);
                if c < c1 || c > c2 {
                    continue;
                }
                items.push((c, cell));
            }
            items.sort_by_key(|(c, _)| *c);
        }
        push_row(&mut out, &items, c1, c2);
        if r == r1 {
            out.push('|');
            for i in 0..cols {
                if i > 0 {
                    out.push_str(" | ");
                } else {
                    out.push(' ');
                }
                out.push_str("---");
            }
            out.push_str(" |\n");
        }
    }
    out.trim_end().to_string()
}

/// A1 风格的单元格引用
fn cell_ref(col: u32, row: u32) -> String {
    format!("{}{}", index_to_col(col), row)
}

/// 稀疏形态：逐行列出有内容的单元格（`- A1: 值; B2: 值`），超限窗口用
fn sparse_direct(sheet: &Sheet, c1: u32, r1: u32, c2: u32, r2: u32) -> String {
    let mut rows: Vec<(u32, &crate::model::sheet::Row)> = sheet
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| (row.index.unwrap_or(i as u32 + 1), row))
        .collect();
    rows.sort_by_key(|(r, _)| *r);

    let mut out = String::new();
    let mut cursor = 0usize;
    for r in r1..=r2 {
        let mut row: Option<&crate::model::sheet::Row> = None;
        while cursor < rows.len() {
            let (idx, cand) = rows[cursor];
            if idx < r {
                cursor += 1;
                continue;
            }
            if idx == r {
                row = Some(cand);
                cursor += 1;
                continue;
            }
            break;
        }
        let Some(row) = row else { continue };
        let mut items: Vec<(u32, &crate::model::cell::Cell)> = Vec::new();
        for (ci, cell) in row.cells.iter().enumerate() {
            if !has_display(cell) {
                continue;
            }
            let c = col_of(cell, ci);
            if c < c1 || c > c2 {
                continue;
            }
            items.push((c, cell));
        }
        if items.is_empty() {
            continue;
        }
        items.sort_by_key(|(c, _)| *c);
        out.push_str("- ");
        for (i, (c, cell)) in items.iter().enumerate() {
            if i > 0 {
                out.push_str("; ");
            }
            out.push_str(&cell_ref(*c, r));
            out.push_str(": ");
            write_display(&mut out, cell);
        }
        out.push('\n');
    }
    out.trim_end().to_string() // 与表格形态一致：末尾不留空行
}

/// 写一行：`| a | b |`（缺列补空），与 `cells.join(" | ")` 的旧写法逐字节一致
fn push_row(out: &mut String, items: &[(u32, &crate::model::cell::Cell)], c1: u32, c2: u32) {
    out.push('|');
    let mut col = c1;
    let mut first = true;
    for (c, cell) in items {
        while col < *c {
            push_cell_sep(out, &mut first);
            col += 1;
        }
        push_cell_sep(out, &mut first);
        write_display(out, cell);
        col = c + 1;
    }
    while col <= c2 {
        push_cell_sep(out, &mut first);
        col += 1;
    }
    out.push_str(" |\n");
}

fn push_cell_sep(out: &mut String, first: &mut bool) {
    if *first {
        out.push(' ');
        *first = false;
    } else {
        out.push_str(" | ");
    }
}

/// 显示文本直接写进输出缓冲：与 `escape(cell_display(cell))` 逐字节一致，
/// 但正常内容（无 `|`、无换行、无需 trim）不做任何分配
fn write_display(out: &mut String, cell: &crate::model::cell::Cell) {
    let text = display_str(cell);
    let text = text.trim();
    if text.contains('|') || text.contains('\n') {
        for ch in text.chars() {
            match ch {
                '|' => out.push_str("\\|"),
                '\n' => out.push(' '),
                c => out.push(c),
            }
        }
    } else {
        out.push_str(text);
    }
}

/// 与 `cell_display` 同义，但能借用时不复制（有公式 → `=公式`）
fn display_str<'a>(cell: &'a crate::model::cell::Cell) -> std::borrow::Cow<'a, str> {
    if let Some(f) = &cell.formula {
        return std::borrow::Cow::Owned(format!("={f}"));
    }
    match &cell.value {
        Some(serde_json::Value::String(s)) => std::borrow::Cow::Borrowed(s.as_str()),
        Some(serde_json::Value::Bool(b)) => std::borrow::Cow::Owned(b.to_string()),
        Some(serde_json::Value::Number(n)) => std::borrow::Cow::Owned(n.to_string()),
        Some(serde_json::Value::Null) | None => std::borrow::Cow::Borrowed(""),
        Some(other) => std::borrow::Cow::Owned(other.to_string()),
    }
}

/// 整个工作簿 → Markdown（每张表一个 `## 表名` 小节）
pub fn workbook_markdown(wb: &Workbook) -> String {
    workbook_markdown_with(wb, PayloadSinks::default())
}

/// 整个工作簿 → Markdown（带附件导出）
pub fn workbook_markdown_with(wb: &Workbook, sinks: PayloadSinks<'_>) -> String {
    let mut out = String::new();
    for (i, s) in wb.sheets.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("## {}\n\n", s.name));
        out.push_str(&sheet_markdown_with(s, sinks));
    }
    out
}

/// 已用区域（最小/最大行列），空表返回 None
pub fn used_range(sheet: &Sheet) -> Option<(u32, u32, u32, u32)> {
    used_range_direct(sheet)
}

/// 指定窗口 → Markdown
///
/// 窗口首行作为表头（电子表格的通行约定；区域视图同此）。
/// `view <xlsx> /sheet[1]/range[A1:C10] text` 走的也是这里。
/// 输出是纯 Markdown：合并区这类表格语法表达不了的信息不放注释，
/// 需要时看 `layout` 视图（它给出 merges）。
pub fn window_markdown(sheet: &Sheet, c1: u32, r1: u32, c2: u32, r2: u32) -> String {
    if used_range_direct(sheet).is_none() {
        return String::new();
    }
    let (c1, r1) = (c1.min(c2), r1.min(r2));
    let (c2, r2) = (c2.max(c1), r2.max(r1));
    render_window(sheet, c1, r1, c2, r2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(v: serde_json::Value) -> Sheet {
        serde_json::from_value(v).expect("sheet json")
    }

    #[test]
    fn grid_becomes_table_with_first_row_as_header() {
        let s = sheet(serde_json::json!({
            "name": "经营看板",
            "rows": [
                { "index": 1, "cells": [ { "ref": "A1", "value": "月份" }, { "ref": "B1", "value": "收入" } ] },
                { "index": 2, "cells": [ { "ref": "A2", "value": "1月" }, { "ref": "B2", "value": 12.5 } ] }
            ]
        }));
        let md = sheet_markdown(&s);
        assert!(md.starts_with("| 月份 | 收入 |\n| --- | --- |\n"), "{md}");
        assert!(md.contains("| 1月 | 12.5 |"), "{md}");
    }

    #[test]
    fn gaps_are_filled_with_empty_cells() {
        let s = sheet(serde_json::json!({
            "name": "s",
            "rows": [
                { "index": 1, "cells": [ { "ref": "A1", "value": "左" }, { "ref": "C1", "value": "右" } ] },
                { "index": 2, "cells": [ { "ref": "B2", "value": "中" } ] }
            ]
        }));
        let md = sheet_markdown(&s);
        assert!(md.contains("| 左 |  | 右 |"), "{md}");
        assert!(md.contains("|  | 中 |  |"), "{md}");
    }

    #[test]
    fn formula_shows_as_formula_text() {
        let s = sheet(serde_json::json!({
            "name": "s",
            "rows": [
                { "index": 1, "cells": [ { "ref": "A1", "value": "合计" }, { "ref": "B1", "formula": "SUM(B2:B9)" } ] }
            ]
        }));
        let md = sheet_markdown(&s);
        assert!(md.contains("| 合计 | =SUM(B2:B9) |"), "{md}");
    }

    #[test]
    fn output_is_plain_markdown_without_comments() {
        let s = sheet(serde_json::json!({
            "name": "s",
            "merges": ["A1:B1"],
            "rows": [ { "index": 1, "cells": [ { "ref": "A1", "value": "标题" } ] } ]
        }));
        let md = sheet_markdown(&s);
        assert_eq!(md, "| 标题 |\n| --- |");
        assert!(!md.contains("<!--"), "{md}");
    }

    #[test]
    fn empty_sheet_is_empty_string() {
        let s = sheet(serde_json::json!({ "name": "s", "rows": [] }));
        assert_eq!(sheet_markdown(&s), "");
    }

    #[test]
    fn workbook_marks_sheet_names() {
        let wb: Workbook = serde_json::from_value(serde_json::json!({
            "sheets": [
                { "name": "汇总", "rows": [ { "index": 1, "cells": [ { "ref": "A1", "value": "x" } ] } ] },
                { "name": "明细", "rows": [] }
            ]
        }))
        .unwrap();
        let md = workbook_markdown(&wb);
        assert!(md.starts_with("## 汇总\n\n| x |\n| --- |\n"), "{md}");
        // 空表只留小节标题，不加注释标记
        assert!(md.trim_end().ends_with("## 明细"), "{md}");
    }

    #[test]
    fn sparse_form_matches_grid_semantics() {
        // 稀疏形态：行内按列升序、缺行跳过、末尾不留空行、`|` 转义
        let s = sheet(serde_json::json!({
            "name": "s",
            "rows": [
                { "index": 1, "cells": [ { "ref": "C1", "value": "右" }, { "ref": "A1", "value": "左|竖" } ] },
                { "index": 3, "cells": [ { "ref": "B3", "value": "三" } ] }
            ]
        }));
        let md = window_markdown(&s, 1, 1, 40, 600);
        assert_eq!(
            md, "- A1: 左\\|竖; C1: 右\n- B3: 三",
            "行内按列升序、跳空行、无尾随空行：{md}"
        );
    }

    #[test]
    fn huge_window_falls_back_to_sparse_rows() {
        let s = sheet(serde_json::json!({
            "name": "s",
            "rows": [
                { "index": 1, "cells": [ { "ref": "A1", "value": "起点" } ] },
                { "index": 500, "cells": [ { "ref": "A500", "value": "终点" } ] }
            ]
        }));
        let md = window_markdown(&s, 1, 1, 40, 600);
        assert!(md.contains("- A1: 起点"), "{md}");
        assert!(md.contains("- A500: 终点"), "{md}");
        assert!(!md.contains("<!--"), "{md}");
    }
}
