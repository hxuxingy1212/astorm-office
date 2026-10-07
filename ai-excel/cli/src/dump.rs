//! 序列化为可回放的编辑指令（batch JSON），与 json2pptx 的 `dump` 对齐。
//!
//! 输出约定：
//! ```json
//! { "version": 1, "commands": [
//!   { "op": "add", "path": "/sheet", "type": "sheet", "props": { "name": "Data" } },
//!   { "op": "set", "path": "/sheet[1]", "props": { "freeze": "A2" } },
//!   { "op": "add", "path": "/sheet[1]/cell[A1]", "type": "cell", "props": { "value": "合计" } }
//! ] }
//! ```
//! props 为 JSON 对象（非 `--prop k=v` 字符串），便于「读取样例 → 修改 → 回放」。

use serde_json::{json, Map, Value};

use json2xlsx::model::cell::CellStyleRef;
use json2xlsx::model::Workbook;

/// dump 时保留在 `set /sheet[N]` props 里的工作表级字段（其余复杂成员单独出 add 指令）
const SHEET_PROPS: &[&str] = &[
    "name",
    "tab_color",
    "hidden",
    "freeze",
    "direction",
    "gridlines",
    "headings",
    "zoom",
    "default_col_width",
    "default_row_height",
    "base_col_width",
    "show_formulas",
    "auto_filter",
    "merges",
    "protect",
];

/// 生成可回放指令
pub fn run(wb: &Workbook) -> Value {
    let mut commands = Vec::new();
    for (i, sheet) in wb.sheets.iter().enumerate() {
        let sp = format!("/sheet[{}]", i + 1);

        commands.push(json!({
            "op": "add",
            "path": "/sheet",
            "type": "sheet",
            "props": { "name": sheet.name },
        }));

        // 工作表级属性（排除单独成指令的成员）
        if let Ok(Value::Object(map)) = serde_json::to_value(sheet) {
            let mut props = Map::new();
            for key in SHEET_PROPS {
                if let Some(v) = map.get(*key) {
                    if !v.is_null() {
                        props.insert((*key).to_string(), v.clone());
                    }
                }
            }
            if !props.is_empty() {
                commands.push(json!({ "op": "set", "path": sp, "props": props }));
            }
        }

        // 单元格
        for row in &sheet.rows {
            let row_idx = row.index.unwrap_or(0);
            let mut next_col: u32 = 1;
            for cell in &row.cells {
                let col = match &cell.reference {
                    Some(r) => json2xlsx::utils::a1::parse_cell_ref(r)
                        .map(|(c, _)| c)
                        .unwrap_or(next_col),
                    None => next_col,
                };
                next_col = col + 1;
                let reference = json2xlsx::utils::a1::make_cell_ref(col, row_idx);
                let mut props = Map::new();
                props.insert("ref".into(), json!(reference));
                if let Some(v) = &cell.value {
                    props.insert("value".into(), v.clone());
                }
                if let Some(f) = &cell.formula {
                    props.insert("formula".into(), json!(f));
                }
                if let Some(t) = cell.infer_type() {
                    props.insert("type".into(), json!(t));
                }
                if let Some(nf) = &cell.number_format {
                    props.insert("number_format".into(), json!(nf));
                }
                if let Some(style) = &cell.style {
                    match style {
                        CellStyleRef::Named(name) => {
                            props.insert("style".into(), json!(name));
                        }
                        CellStyleRef::Inline(def) => {
                            if let Ok(v) = serde_json::to_value(def) {
                                props.insert("style".into(), v);
                            }
                        }
                    }
                }
                if let Some(link) = &cell.link {
                    props.insert("link".into(), json!(link));
                }
                if let Some(comment) = &cell.comment {
                    props.insert("comment".into(), json!(comment));
                }
                commands.push(json!({
                    "op": "add",
                    "path": format!("{sp}/cell[{reference}]"),
                    "type": "cell",
                    "props": props,
                }));
            }
        }

        // 图表 / 图片 / 表格 / 形状
        for chart in &sheet.charts {
            push_add(&mut commands, &sp, "chart", chart);
        }
        for image in &sheet.images {
            push_add(&mut commands, &sp, "image", image);
        }
        for table in &sheet.tables {
            push_add(&mut commands, &sp, "table", table);
        }
        for shape in &sheet.drawing_shapes {
            push_add(&mut commands, &sp, "shape", shape);
        }
    }

    json!({ "version": 1, "commands": commands })
}

fn push_add<T: serde::Serialize>(
    commands: &mut Vec<Value>,
    sheet_path: &str,
    etype: &str,
    value: &T,
) {
    if let Ok(props) = serde_json::to_value(value) {
        commands.push(json!({
            "op": "add",
            "path": sheet_path,
            "type": etype,
            "props": props,
        }));
    }
}
