//! Parse structured table (ListObject) parts back into the model.

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::model::sheet::{Table, TableTotal};

pub fn parse_tables(xmls: &[String]) -> Vec<Table> {
    xmls.iter().filter_map(|x| parse_table(x)).collect()
}

pub fn parse_table(xml: &str) -> Option<Table> {
    let mut reader = Reader::from_str(xml);

    let mut range: Option<String> = None;
    let mut name: Option<String> = None;
    let mut style: Option<String> = None;
    let mut show_header = true;
    let mut banded_rows = false;
    let mut banded_cols = false;
    let mut first_col = false;
    let mut last_col = false;
    let mut columns: Vec<String> = Vec::new();
    let mut totals: Vec<TableTotal> = Vec::new();
    let mut calc: Vec<String> = Vec::new();
    let mut totals_row = false;
    let mut insert_row = false;
    let mut cur: Option<usize> = None;
    let mut text_tag: Option<u8> = None; // 1 = calculatedColumnFormula, 2 = totalsRowFormula
    let mut buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let n = e.local_name().as_ref().to_vec();
                // 跳过带命名空间前缀的扩展元素（如 <x14:table>），避免其同名
                // local name 覆盖主命名空间元素的属性。
                if e.name().as_ref().contains(&b':') {
                    continue;
                }
                let attrs: Vec<(String, String)> = e
                    .attributes()
                    .filter_map(|a| a.ok())
                    .map(|a| {
                        (
                            String::from_utf8_lossy(a.key.local_name().as_ref()).to_string(),
                            a.unescape_value()
                                .map(|v| v.into_owned())
                                .unwrap_or_default(),
                        )
                    })
                    .collect();
                let g = |k: &str| {
                    attrs
                        .iter()
                        .find(|(kk, _)| kk == k)
                        .map(|(_, v)| v.as_str())
                };
                match n.as_slice() {
                    b"table" => {
                        range = g("ref").map(|s| s.to_string());
                        name = g("name").map(|s| s.to_string());
                        show_header = g("headerRowCount").map(|v| v != "0").unwrap_or(true);
                        totals_row = matches!(g("totalsRowCount"), Some(v) if v != "0");
                        insert_row = matches!(g("insertRow"), Some("1") | Some("true"));
                    }
                    b"tableColumn" => {
                        columns.push(g("name").unwrap_or_default().to_string());
                        totals.push(TableTotal {
                            label: g("totalsRowLabel").map(|s| s.to_string()),
                            function: g("totalsRowFunction").map(|s| s.to_string()),
                            formula: None,
                        });
                        calc.push(String::new());
                        cur = Some(columns.len() - 1);
                    }
                    b"calculatedColumnFormula" => {
                        text_tag = Some(1);
                        buf.clear();
                    }
                    b"totalsRowFormula" => {
                        text_tag = Some(2);
                        buf.clear();
                    }
                    b"tableStyleInfo" => {
                        style = g("name").map(|s| s.to_string());
                        banded_rows = matches!(g("showRowStripes"), Some("1") | Some("true"));
                        banded_cols = matches!(g("showColumnStripes"), Some("1") | Some("true"));
                        first_col = matches!(g("showFirstColumn"), Some("1") | Some("true"));
                        last_col = matches!(g("showLastColumn"), Some("1") | Some("true"));
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) if text_tag.is_some() => {
                if let Ok(t) = e.unescape() {
                    buf.push_str(&t);
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"calculatedColumnFormula" => {
                    if let Some(i) = cur {
                        calc[i] = buf.clone();
                    }
                    text_tag = None;
                }
                b"totalsRowFormula" => {
                    if let Some(i) = cur {
                        totals[i].formula = Some(buf.clone());
                    }
                    text_tag = None;
                }
                b"tableColumn" => cur = None,
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    let range = range?;
    // 若没有任何汇总/计算列信息，则不序列化这两个字段。
    if totals.iter().all(|t| t.is_empty()) {
        totals.clear();
    }
    if calc.iter().all(|c| c.is_empty()) {
        calc.clear();
    }
    Some(Table {
        range,
        name,
        style,
        show_header_row: show_header,
        insert_row,
        show_banded_rows: banded_rows,
        show_banded_columns: banded_cols,
        show_first_column: first_col,
        show_last_column: last_col,
        columns,
        totals_row,
        totals,
        calculated_formulas: calc,
    })
}
