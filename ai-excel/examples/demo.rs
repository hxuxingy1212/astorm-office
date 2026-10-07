//! Generate a demo workbook for manual / external validation.
//!
//! Usage: cargo run --example demo -- /tmp/demo.xlsx

use json2xlsx::model::cell::CellStyleRef;
use json2xlsx::model::style::{Alignment, Font, StyleDef};
use json2xlsx::model::{Cell, Column, Meta, Row, Sheet, Workbook};

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/json2xlsx_demo.xlsx".to_string());

    let header = StyleDef {
        font: Some(Font {
            bold: true,
            color: Some("FFFFFF".into()),
            ..Default::default()
        }),
        fill: Some("4472C4".into()),
        alignment: Some(Alignment {
            horizontal: Some("center".into()),
            ..Default::default()
        }),
        ..Default::default()
    };

    let wb = Workbook {
        meta: Some(Meta {
            title: Some("Demo Report".into()),
            author: Some("json2xlsx".into()),
        }),
        sheets: vec![Sheet {
            name: "Report".into(),
            tab_color: Some("4472C4".into()),
            freeze: Some("A2".into()),
            columns: vec![
                Column {
                    width: Some(20.0),
                    ..Default::default()
                },
                Column {
                    width: Some(14.0),
                    ..Default::default()
                },
            ],
            merges: vec!["A1:B1".into()],
            rows: vec![
                Row {
                    index: Some(1),
                    height: Some(20.0),
                    cells: vec![Cell {
                        reference: Some("A1".into()),
                        value: Some(serde_json::json!("季度营收汇总")),
                        style: Some(CellStyleRef::Inline(header)),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![
                        Cell {
                            reference: Some("A2".into()),
                            value: Some(serde_json::json!("Q1")),
                            ..Default::default()
                        },
                        Cell {
                            reference: Some("B2".into()),
                            value: Some(serde_json::json!(1234.5)),
                            number_format: Some("#,##0.00".into()),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                Row {
                    index: Some(3),
                    cells: vec![
                        Cell {
                            reference: Some("A3".into()),
                            value: Some(serde_json::json!("合计")),
                            ..Default::default()
                        },
                        Cell {
                            reference: Some("B3".into()),
                            formula: Some("SUM(B2:B2)".into()),
                            number_format: Some("#,##0.00".into()),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
                Row {
                    index: Some(4),
                    cells: vec![Cell {
                        reference: Some("A4".into()),
                        cell_type: Some("date".into()),
                        value: Some(serde_json::json!("2026-01-31")),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };

    let result = json2xlsx::generate(&wb, std::path::Path::new(&path)).expect("generate");
    println!("generated: {} ({} sheet)", result.path, result.sheets);
}
