//! CSV/TSV → Workbook 导入（RFC4180：引号、转义、嵌入换行）。

use std::path::Path;

use json2xlsx::model::cell::CellStyleRef;
use json2xlsx::model::style::{Alignment, Font, StyleDef};
use json2xlsx::model::{Cell, Column, Row, Sheet, Workbook};
use json2xlsx::Error;
use json2xlsx::Result;
use serde_json::Value;

/// 解析 CSV 文本为记录（字段）列表。
fn parse_csv(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let mut field = String::new();
    let mut record = Vec::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' {
            in_quotes = true;
        } else if c == delim {
            record.push(std::mem::take(&mut field));
        } else if c == '\n' {
            record.push(std::mem::take(&mut field));
            records.push(std::mem::take(&mut record));
        } else if c == '\r' {
            // 忽略（配合 \n）
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    // 去掉末尾空行
    while records
        .last()
        .map(|r| r.len() == 1 && r[0].is_empty())
        .unwrap_or(false)
    {
        records.pop();
    }
    records
}

fn detect_delim(text: &str) -> char {
    let line = text.lines().next().unwrap_or("");
    let cands = [',', '\t', ';', '|'];
    cands
        .iter()
        .copied()
        .max_by_key(|c| line.matches(*c).count())
        .filter(|c| line.matches(*c).count() > 0)
        .unwrap_or(',')
}

fn typed(s: &str) -> Value {
    let t = s.trim();
    if t.is_empty() {
        return Value::String(String::new());
    }
    if let Ok(i) = t.parse::<i64>() {
        return Value::Number(i.into());
    }
    if let Ok(f) = t.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return Value::Number(n);
        }
    }
    match t.to_ascii_lowercase().as_str() {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(s.to_string()),
    }
}

pub fn csv_to_workbook(
    input: &Path,
    delim: Option<&str>,
    header: bool,
    sheet_name: Option<String>,
) -> Result<Workbook> {
    let text = std::fs::read_to_string(input)
        .map_err(|e| Error::InvalidInput(format!("读取 {}: {e}", input.display())))?;
    let delim = match delim {
        Some("\\t") | Some("tab") => '\t',
        Some(s) => s.chars().next().unwrap_or(','),
        None => detect_delim(&text),
    };
    let records = parse_csv(&text, delim);
    let ncols = records.iter().map(|r| r.len()).max().unwrap_or(0);

    let header_style = StyleDef {
        font: Some(Font {
            bold: true,
            ..Default::default()
        }),
        fill: Some("D9D9D9".into()),
        alignment: Some(Alignment {
            horizontal: Some("center".into()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut widths = vec![8usize; ncols];
    let mut rows = Vec::new();
    for (ri, rec) in records.iter().enumerate() {
        let rnum = (ri + 1) as u32;
        let mut cells = Vec::new();
        for (ci, val) in rec.iter().enumerate() {
            let col = (ci + 1) as u32;
            let reference = json2xlsx::utils::a1::make_cell_ref(col, rnum);
            let is_header = header && ri == 0;
            let cell = if is_header {
                Cell {
                    reference: Some(reference),
                    value: Some(Value::String(val.clone())),
                    style: Some(CellStyleRef::Inline(header_style.clone())),
                    ..Default::default()
                }
            } else {
                Cell {
                    reference: Some(reference),
                    value: Some(typed(val)),
                    ..Default::default()
                }
            };
            // 列宽推断（中文按 2 计）
            let w: usize = val
                .chars()
                .map(|c| if (c as u32) > 0x2E80 { 2 } else { 1 })
                .sum::<usize>()
                + 2;
            if ci < widths.len() {
                widths[ci] = widths[ci].max(w);
            }
            cells.push(cell);
        }
        rows.push(Row {
            index: Some(rnum),
            cells,
            ..Default::default()
        });
    }

    let columns = widths
        .into_iter()
        .map(|w| Column {
            width: Some(w.clamp(8, 60) as f64),
            ..Default::default()
        })
        .collect();

    let name = sheet_name.unwrap_or_else(|| "Sheet1".into());
    let sheet = Sheet {
        name,
        columns,
        rows,
        freeze: if header { Some("A2".into()) } else { None },
        ..Default::default()
    };
    Ok(Workbook {
        sheets: vec![sheet],
        ..Default::default()
    })
}
