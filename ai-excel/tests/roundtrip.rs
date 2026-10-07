mod common;

use json2xlsx::model::cell::{CellStyleRef, Run};
use json2xlsx::model::style::{Alignment, Border, Font, StyleDef};
use json2xlsx::model::{
    Cell, Column, ConditionalFormat, DataValidation, FilterColumn, Meta, NamedRange, PrintSettings,
    Row, Sheet, Table, Workbook,
};
use json2xlsx::{generate, parse};
use serde_json::json;

/// generate then parse, returning the parsed workbook.
fn roundtrip(wb: &Workbook) -> Workbook {
    let path = common::temp_xlsx_path();
    generate(wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    parsed
}

fn cell(reference: &str, value: serde_json::Value) -> Cell {
    Cell {
        reference: Some(reference.into()),
        value: Some(value),
        ..Default::default()
    }
}

fn assert_roundtrip(wb: &Workbook) {
    let parsed = roundtrip(wb);
    assert_eq!(parsed, wb.normalized());
}

#[test]
fn basic_values() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![
                        cell("A1", json!("hello")),
                        cell("B1", json!(42)),
                        cell("C1", json!(3.5)),
                        cell("D1", json!(true)),
                    ],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![cell("A2", json!("世界")), cell("B2", json!(-7))],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn multiple_sheets_and_meta() {
    let wb = Workbook {
        meta: Some(Meta {
            title: Some("报表".into()),
            author: Some("ai-excel".into()),
        }),
        sheets: vec![
            Sheet {
                name: "Summary".into(),
                rows: vec![Row {
                    index: Some(1),
                    cells: vec![cell("A1", json!("Total"))],
                    ..Default::default()
                }],
                ..Default::default()
            },
            Sheet {
                name: "Data".into(),
                hidden: true,
                rows: vec![Row {
                    index: Some(1),
                    cells: vec![cell("A1", json!(1)), cell("A2", json!(2))],
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn formulas() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    cell("A1", json!(10)),
                    cell("A2", json!(20)),
                    Cell {
                        reference: Some("A3".into()),
                        formula: Some("SUM(A1:A2)".into()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    // 生成时会对无缓存值的公式求值，故断言解析回的缓存值。
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let a3 = parsed.sheets[0].rows[0]
        .cells
        .iter()
        .find(|c| c.reference.as_deref() == Some("A3"))
        .unwrap();
    assert_eq!(a3.formula.as_deref(), Some("SUM(A1:A2)"));
    assert_eq!(a3.value, Some(json!(30)));
}

#[test]
fn number_formats_and_styles() {
    let style = StyleDef {
        font: Some(Font {
            bold: true,
            color: Some("FF0000".into()),
            size: Some(12.0),
            ..Default::default()
        }),
        fill: Some("FFFF00".into()),
        alignment: Some(Alignment {
            horizontal: Some("center".into()),
            wrap_text: true,
            ..Default::default()
        }),
        border: Some(Border {
            all: Some("thin".into()),
            color: Some("808080".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("标题")),
                        style: Some(CellStyleRef::Inline(style)),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("B1".into()),
                        value: Some(json!(1234.5)),
                        number_format: Some("#,##0.00".into()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn layout_features() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            tab_color: Some("4472C4".into()),
            freeze: Some("B2".into()),
            auto_filter: Some("A1:D10".into()),
            columns: vec![
                Column {
                    width: Some(18.0),
                    ..Default::default()
                },
                Column {
                    hidden: true,
                    ..Default::default()
                },
            ],
            merges: vec!["A1:C1".into()],
            rows: vec![Row {
                index: Some(1),
                height: Some(22.0),
                cells: vec![cell("A1", json!("Merged title"))],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn dates_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        cell_type: Some("date".into()),
                        value: Some(json!("2026-01-31")),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("A2".into()),
                        cell_type: Some("date".into()),
                        value: Some(json!("1900-03-01")),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn date1904() {
    let wb = Workbook {
        date1904: true,
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![Cell {
                    reference: Some("A1".into()),
                    cell_type: Some("date".into()),
                    value: Some(json!("2020-06-15")),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn named_ranges() {
    let wb = Workbook {
        named_ranges: vec![NamedRange {
            name: "Tax".into(),
            reference: "Sheet1!$B$1".into(),
        }],
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![cell("B1", json!(0.13))],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn hyperlinks_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("site")),
                        link: Some("https://example.com".into()),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("A2".into()),
                        value: Some(json!("jump")),
                        link: Some("#Sheet1!B2".into()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn rich_text_runs_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![Cell {
                    reference: Some("A1".into()),
                    cell_type: Some("richtext".into()),
                    value: Some(json!("BoldRed plain")),
                    runs: vec![
                        Run {
                            text: "BoldRed ".into(),
                            bold: true,
                            color: Some("FF0000".into()),
                            ..Default::default()
                        },
                        Run {
                            text: "plain".into(),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn data_validation_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Form".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![cell("A1", json!("level"))],
                ..Default::default()
            }],
            validations: vec![
                DataValidation {
                    range: "A1:A10".into(),
                    validation_type: "list".into(),
                    values: vec!["低".into(), "中".into(), "高".into()],
                    allow_blank: true,
                    show_error: true,
                    error_title: Some("无效值".into()),
                    error_message: Some("请从下拉中选择".into()),
                    ..Default::default()
                },
                DataValidation {
                    range: "B1:B10".into(),
                    validation_type: "whole".into(),
                    operator: Some("between".into()),
                    formula1: Some("1".into()),
                    formula2: Some("10".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn conditional_format_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "CF".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![cell("A1", json!(1))],
                ..Default::default()
            }],
            conditional_formats: vec![
                ConditionalFormat {
                    range: "B2:B10".into(),
                    rule_type: "cellIs".into(),
                    operator: Some("greaterThan".into()),
                    formulas: vec!["100".into()],
                    font: Some(Font {
                        bold: true,
                        color: Some("FF0000".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ConditionalFormat {
                    range: "C2:C10".into(),
                    rule_type: "colorScale".into(),
                    colors: vec!["63BE7B".into(), "FFEB84".into(), "F8696B".into()],
                    ..Default::default()
                },
                ConditionalFormat {
                    range: "D2:D10".into(),
                    rule_type: "dataBar".into(),
                    color: Some("638EC6".into()),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn protection_and_print_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "P".into(),
            protect: true,
            print: Some(PrintSettings {
                orientation: Some("landscape".into()),
                fit_to_page: true,
                fit_to_width: Some(1),
                fit_to_height: Some(1),
                ..Default::default()
            }),
            rows: vec![Row {
                index: Some(1),
                cells: vec![cell("A1", json!("x"))],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn table_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Scores".into(),
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![cell("A1", json!("Name")), cell("B1", json!("Score"))],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![cell("A2", json!("Alice")), cell("B2", json!(90))],
                    ..Default::default()
                },
                Row {
                    index: Some(3),
                    cells: vec![cell("A3", json!("Bob")), cell("B3", json!(85))],
                    ..Default::default()
                },
            ],
            tables: vec![Table {
                range: "A1:B3".into(),
                name: Some("Scores".into()),
                style: Some("TableStyleMedium9".into()),
                show_header_row: true,
                show_banded_rows: true,
                columns: vec!["Name".into(), "Score".into()],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn print_area_and_titles_roundtrip() {
    let wb = Workbook {
        named_ranges: vec![NamedRange {
            name: "Rate".into(),
            reference: "R!$B$1".into(),
        }],
        sheets: vec![Sheet {
            name: "R".into(),
            print_area: Some("A1:C20".into()),
            print_title_rows: Some("1:1".into()),
            print_title_cols: Some("A:A".into()),
            rows: vec![Row {
                index: Some(1),
                cells: vec![cell("A1", json!("x"))],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn comments_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Notes".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("x")),
                        comment: Some("第一条备注".into()),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("B2".into()),
                        value: Some(json!(1)),
                        comment: Some("second note".into()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn view_outline_and_diagonal_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "V".into(),
            gridlines: Some(false),
            headings: Some(false),
            zoom: Some(150),
            show_formulas: true,
            tab_selected: true,
            columns: vec![
                Column {
                    width: Some(12.0),
                    outline_level: Some(1),
                    ..Default::default()
                },
                Column {
                    hidden: true,
                    collapsed: true,
                    ..Default::default()
                },
            ],
            rows: vec![
                Row {
                    index: Some(1),
                    outline_level: Some(1),
                    cells: vec![Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("x")),
                        style: Some(CellStyleRef::Inline(StyleDef {
                            border: Some(Border {
                                diagonal: Some("thin".into()),
                                diagonal_up: true,
                                diagonal_color: Some("FF0000".into()),
                                ..Default::default()
                            }),
                            ..Default::default()
                        })),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    hidden: true,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn conditional_format_extras_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "CF2".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![cell("A1", json!(1))],
                ..Default::default()
            }],
            conditional_formats: vec![
                ConditionalFormat {
                    range: "A1:A10".into(),
                    rule_type: "containsText".into(),
                    text: Some("急".into()),
                    font: Some(Font {
                        bold: true,
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                ConditionalFormat {
                    range: "B1:B10".into(),
                    rule_type: "top10".into(),
                    rank: Some(5),
                    ..Default::default()
                },
                ConditionalFormat {
                    range: "C1:C10".into(),
                    rule_type: "duplicateValues".into(),
                    fill: Some("FFC7CE".into()),
                    ..Default::default()
                },
                ConditionalFormat {
                    range: "D1:D10".into(),
                    rule_type: "aboveAverage".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn autofilter_criteria_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "F".into(),
            auto_filter: Some("A1:C10".into()),
            filter_columns: vec![
                FilterColumn {
                    col_id: 0,
                    values: vec!["A".into(), "B".into()],
                    ..Default::default()
                },
                FilterColumn {
                    col_id: 2,
                    operator: Some("greaterThan".into()),
                    criteria: Some("100".into()),
                    ..Default::default()
                },
            ],
            rows: vec![Row {
                index: Some(1),
                cells: vec![cell("A1", json!("k"))],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn hyperlink_tooltip_and_run_extras_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "L".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("site")),
                        link: Some("https://example.com".into()),
                        link_tooltip: Some("打开官网".into()),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("A2".into()),
                        cell_type: Some("richtext".into()),
                        value: Some(json!("x2 y2")),
                        runs: vec![
                            Run {
                                text: "x".into(),
                                strike: true,
                                underline: Some("single".into()),
                                vert_align: Some("superscript".into()),
                                ..Default::default()
                            },
                            Run {
                                text: "2 y".into(),
                                ..Default::default()
                            },
                            Run {
                                text: "2".into(),
                                vert_align: Some("subscript".into()),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn conditional_format_iconset_and_more_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "CF3".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![cell("A1", json!(1))],
                ..Default::default()
            }],
            conditional_formats: vec![
                ConditionalFormat {
                    range: "A1:A10".into(),
                    rule_type: "iconSet".into(),
                    icon_style: Some("3TrafficLights1".into()),
                    show_value: Some(true),
                    ..Default::default()
                },
                ConditionalFormat {
                    range: "B1:B10".into(),
                    rule_type: "beginsWith".into(),
                    text: Some("A".into()),
                    ..Default::default()
                },
                ConditionalFormat {
                    range: "C1:C10".into(),
                    rule_type: "timePeriod".into(),
                    time_period: Some("thisMonth".into()),
                    ..Default::default()
                },
                ConditionalFormat {
                    range: "D1:D10".into(),
                    rule_type: "aboveAverage".into(),
                    above_average: Some(true),
                    equal_average: true,
                    std_dev: Some(2),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn workbook_layout_fidelity_roundtrip() {
    let wb = Workbook {
        active_tab: Some(1),
        default_font: Some(Font {
            name: Some("等线".into()),
            size: Some(11.0),
            ..Default::default()
        }),
        sheets: vec![
            Sheet {
                name: "A".into(),
                base_col_width: Some(8.0),
                default_col_width: Some(9.0),
                default_row_height: Some(14.4),
                print: Some(PrintSettings {
                    orientation: Some("landscape".into()),
                    margin_left: Some(0.7),
                    margin_top: Some(0.75),
                    ..Default::default()
                }),
                rows: vec![Row {
                    index: Some(1),
                    cells: vec![cell("A1", json!("x"))],
                    ..Default::default()
                }],
                ..Default::default()
            },
            Sheet {
                name: "B".into(),
                rows: vec![Row {
                    index: Some(1),
                    cells: vec![cell("A1", json!(1))],
                    ..Default::default()
                }],
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn column_row_style_and_gray125_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            columns: vec![Column {
                width: Some(12.0),
                style: Some(StyleDef {
                    fill: Some("D9D9D9".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            rows: vec![
                Row {
                    index: Some(1),
                    style: Some(StyleDef {
                        font: Some(Font {
                            bold: true,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    cells: vec![cell("A1", json!("h"))],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![Cell {
                        reference: Some("A2".into()),
                        value: Some(json!(1)),
                        style: Some(CellStyleRef::Inline(StyleDef {
                            fill: Some("GRAY125".into()),
                            ..Default::default()
                        })),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn error_values() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![Cell {
                    reference: Some("A1".into()),
                    cell_type: Some("error".into()),
                    value: Some(json!("#REF!")),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn empty_sheet() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Blank".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn generated_file_is_a_valid_zip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![cell("A1", json!("x"))],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).unwrap();
    let f = std::fs::File::open(&path).unwrap();
    let zip = zip::ZipArchive::new(f).unwrap();
    assert!(zip.file_names().any(|n| n == "xl/workbook.xml"));
    assert!(zip.file_names().any(|n| n == "xl/worksheets/sheet1.xml"));
    let _ = std::fs::remove_file(&path);
}

#[test]
fn sparkline_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            sparklines: vec![
                json2xlsx::model::sheet::Sparkline {
                    kind: Some("line".into()),
                    data_range: "A1:A10".into(),
                    location: "B1".into(),
                    color: Some("FF0000".into()),
                    show_high: true,
                    show_last: true,
                    ..Default::default()
                },
                json2xlsx::model::sheet::Sparkline {
                    kind: Some("column".into()),
                    data_range: "A1:A10".into(),
                    location: "B2".into(),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn formula_values_evaluated() {
    let mk = |r: &str, f: &str| Cell {
        reference: Some(r.into()),
        formula: Some(f.into()),
        ..Default::default()
    };
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![cell("A1", json!(2)), cell("B1", json!(3))],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![
                        mk("A2", "SUM(A1:B1)"),
                        mk("B2", "AVERAGE(A1:B1)"),
                        mk("C2", "IF(A2>4,\"big\",\"small\")"),
                        mk("D2", "A1*B1"),
                        mk("E2", "A1/B1^2"),
                        mk("F2", "ROUND(A1/B1,2)"),
                        mk("G2", "SUM(A1:B1)+D2"),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let got = |r: &str| {
        parsed.sheets[0].rows[1]
            .cells
            .iter()
            .find(|c| c.reference.as_deref() == Some(r))
            .and_then(|c| c.value.clone())
    };
    assert_eq!(got("A2"), Some(json!(5)));
    assert_eq!(got("B2"), Some(json!(2.5)));
    assert_eq!(got("C2"), Some(json!("big")));
    assert_eq!(got("D2"), Some(json!(6)));
    assert_eq!(got("G2"), Some(json!(11)));
}

#[test]
fn pivot_and_slicer_generate() {
    use json2xlsx::model::sheet::{PivotTable, PivotValue, Slicer};
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Data".into(),
            pivot_tables: vec![PivotTable {
                name: Some("P1".into()),
                source: "Data!A1:B5".into(),
                position: Some("D1".into()),
                rows: vec!["Region".into()],
                values: vec![PivotValue {
                    field: "Amount".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            slicers: vec![Slicer {
                field: "Region".into(),
                anchor: Some("D8:F18".into()),
                ..Default::default()
            }],
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![cell("A1", json!("Region")), cell("B1", json!("Amount"))],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![cell("A2", json!("N")), cell("B2", json!(10))],
                    ..Default::default()
                },
                Row {
                    index: Some(3),
                    cells: vec![cell("A3", json!("S")), cell("B3", json!(20))],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    assert_eq!(parsed.sheets.len(), 1);
}

#[test]
fn formula_more_functions() {
    let mk = |r: &str, f: &str| Cell {
        reference: Some(r.into()),
        formula: Some(f.into()),
        ..Default::default()
    };
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![cell("A1", json!("K")), cell("B1", json!("V"))],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![cell("A2", json!("a")), cell("B2", json!(1))],
                    ..Default::default()
                },
                Row {
                    index: Some(3),
                    cells: vec![cell("A3", json!("b")), cell("B3", json!(2))],
                    ..Default::default()
                },
                Row {
                    index: Some(4),
                    cells: vec![cell("A4", json!("c")), cell("B4", json!(3))],
                    ..Default::default()
                },
                Row {
                    index: Some(5),
                    cells: vec![
                        mk("D1", "SUMIFS(B2:B4,A2:A4,\">=b\")"),
                        mk("E1", "VLOOKUP(\"b\",A2:B4,2)"),
                        mk("F1", "INDEX(A2:A4,2)"),
                        mk("G1", "MATCH(\"c\",A2:A4,0)"),
                        mk("H1", "MEDIAN(B2:B4)"),
                        mk("I1", "SUMPRODUCT(B2:B4,B2:B4)"),
                        mk("J1", "ROUND(PMT(0.05,3,-100),2)"),
                        mk("K1", "MAXIFS(B2:B4,A2:A4,\"<>a\")"),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let got = |r: &str| {
        parsed.sheets[0].rows[4]
            .cells
            .iter()
            .find(|c| c.reference.as_deref() == Some(r))
            .and_then(|c| c.value.clone())
    };
    assert_eq!(got("D1"), Some(json!(5)));
    assert_eq!(got("E1"), Some(json!(2)));
    assert_eq!(got("F1"), Some(json!("b")));
    assert_eq!(got("G1"), Some(json!(3)));
    assert_eq!(got("H1"), Some(json!(2)));
    assert_eq!(got("I1"), Some(json!(14)));
    assert_eq!(got("K1"), Some(json!(3)));
}

#[test]
fn background_and_header_footer() {
    let gif = common::write_test_gif("bg");
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            background: Some(gif.display().to_string()),
            print: Some(PrintSettings {
                odd_header: Some("&CMy Report".into()),
                odd_footer: Some("&P / &N".into()),
                ..Default::default()
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let s = &parsed.sheets[0];
    assert!(
        s.background
            .as_deref()
            .map(|b| b.starts_with("xl/media/"))
            .unwrap_or(false),
        "背景图未保留: {:?}",
        s.background
    );
    let p = s.print.as_ref().expect("print");
    assert_eq!(p.odd_header.as_deref(), Some("&CMy Report"));
    assert_eq!(p.odd_footer.as_deref(), Some("&P / &N"));
}

#[test]
fn shape_generate() {
    use json2xlsx::model::sheet::Shape;
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            drawing_shapes: vec![Shape {
                geometry: Some("ellipse".into()),
                anchor: Some("B2".into()),
                size: Some(json2xlsx::model::Size { w: 2.0, h: 1.0 }),
                fill: Some("FF0000".into()),
                line_color: Some("000000".into()),
                text: Some("Hi".into()),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let shapes = &parsed.sheets[0].shapes;
    assert_eq!(shapes.len(), 1);
    assert!(shapes[0].contains("ellipse"), "{}", shapes[0]);
    assert!(shapes[0].contains("FF0000"));
    assert!(shapes[0].contains("Hi"));
}

#[test]
fn sort_state_roundtrip() {
    use json2xlsx::model::sheet::{SortCondition, SortState};
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            auto_filter: Some("A1:B4".into()),
            sort: Some(SortState {
                range: "A2:B4".into(),
                conditions: vec![SortCondition {
                    column: "B2:B4".into(),
                    descending: true,
                    sort_on: Some("value".into()),
                }],
            }),
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}

#[test]
fn ole_object_generate() {
    use json2xlsx::model::sheet::OleObject;
    let dir = std::env::temp_dir();
    let bin = dir.join(format!("json2xlsx_ole_{}.bin", std::process::id()));
    std::fs::write(&bin, b"FAKEOLE").unwrap();
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            ole_objects: vec![OleObject {
                prog_id: "Package".into(),
                src: bin.display().to_string(),
                anchor: Some("C2".into()),
                size: None,
                icon: None,
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let f = std::fs::File::open(&path).unwrap();
    let mut z = zip::ZipArchive::new(f).unwrap();
    let names: Vec<String> = (0..z.len())
        .map(|i| z.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(
        names.iter().any(|n| n.contains("embeddings/oleObject")),
        "{names:?}"
    );
    let mut sheet = String::new();
    std::io::Read::read_to_string(
        &mut z.by_name("xl/worksheets/sheet1.xml").unwrap(),
        &mut sheet,
    )
    .unwrap();
    assert!(sheet.contains("<oleObjects>"));
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&bin);
}

#[test]
fn formula_engine_broad() {
    let mk = |r: &str, f: &str| Cell {
        reference: Some(r.into()),
        formula: Some(f.into()),
        ..Default::default()
    };
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![
                        cell("A1", json!(10)),
                        cell("A2", json!(20)),
                        cell("A3", json!(30)),
                    ],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![
                        mk("B1", "CEILING(2.3,1)"),
                        mk("B2", "MROUND(7,3)"),
                        mk("B3", "GCD(12,18)"),
                        mk("C1", "MID(\"Hello\",2,3)"),
                        mk("C2", "FIND(\"l\",\"Hello\")"),
                        mk("C3", "SUBSTITUTE(\"a-b-c\",\"-\",\"+\")"),
                        mk("D1", "REPT(\"ab\",3)"),
                        mk("D2", "ROWS(A1:A3)"),
                        mk("D3", "COLUMNS(A1:B1)"),
                        mk("E1", "IFS(A1>5,\"big\",TRUE,\"small\")"),
                        mk("E2", "SWITCH(A2,20,\"twenty\",\"other\")"),
                        mk("E3", "SLN(1000,100,3)"),
                        mk("F1", "EDATE(DATE(2024,1,31),1)"),
                        mk("F2", "DATE(2024,2,29)"),
                        mk("F3", "DAYS(DATE(2024,3,1),DATE(2024,2,28))"),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let got = |r: &str| {
        parsed.sheets[0].rows[1]
            .cells
            .iter()
            .find(|c| c.reference.as_deref() == Some(r))
            .and_then(|c| c.value.clone())
    };
    assert_eq!(got("B1"), Some(json!(3)));
    assert_eq!(got("B2"), Some(json!(6)));
    assert_eq!(got("B3"), Some(json!(6)));
    assert_eq!(got("C1"), Some(json!("ell")));
    assert_eq!(got("C2"), Some(json!(3)));
    assert_eq!(got("C3"), Some(json!("a+b+c")));
    assert_eq!(got("D1"), Some(json!("ababab")));
    assert_eq!(got("D2"), Some(json!(3)));
    assert_eq!(got("D3"), Some(json!(2)));
    assert_eq!(got("E1"), Some(json!("big")));
    assert_eq!(got("E2"), Some(json!("twenty")));
    assert_eq!(got("F1"), got("F2"));
    assert_eq!(got("F3"), Some(json!(2)));
}

#[test]
fn formula_engine_stats() {
    let mk = |r: &str, f: &str| Cell {
        reference: Some(r.into()),
        formula: Some(f.into()),
        ..Default::default()
    };
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![
                        cell("A1", json!(1)),
                        cell("A2", json!(2)),
                        cell("A3", json!(3)),
                        cell("A4", json!(4)),
                    ],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![
                        mk("B1", "COMBIN(5,2)"),
                        mk("B2", "PERMUT(4,2)"),
                        mk("B3", "ROMAN(9)"),
                        mk("B4", "ARABIC(\"IX\")"),
                        mk("C1", "PERCENTILE.INC(A1:A4,0.5)"),
                        mk("C2", "GEOMEAN(1,4)"),
                        mk("C3", "ISNUMBER(A1)"),
                        mk("C4", "NETWORKDAYS(DATE(2024,1,1),DATE(2024,1,7))"),
                        mk("D1", "CORREL(A1:A4,A1:A4)"),
                        mk("D2", "SUMXMY2(A1:A4,A1:A4)"),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let got = |r: &str| {
        parsed.sheets[0].rows[1]
            .cells
            .iter()
            .find(|c| c.reference.as_deref() == Some(r))
            .and_then(|c| c.value.clone())
    };
    assert_eq!(got("B1"), Some(json!(10)));
    assert_eq!(got("B2"), Some(json!(12)));
    assert_eq!(got("B3"), Some(json!("IX")));
    assert_eq!(got("B4"), Some(json!(9)));
    assert_eq!(got("C1"), Some(json!(2.5)));
    assert_eq!(got("C2"), Some(json!(2)));
    assert_eq!(got("C3"), Some(json!(true)));
    assert_eq!(got("C4"), Some(json!(5)));
    assert!((got("D1").unwrap().as_f64().unwrap() - 1.0).abs() < 1e-9);
    assert_eq!(got("D2"), Some(json!(0)));
}

#[test]
fn formula_engine_third() {
    let mk = |r: &str, f: &str| Cell {
        reference: Some(r.into()),
        formula: Some(f.into()),
        ..Default::default()
    };
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![
                        cell("A1", json!(10)),
                        cell("A2", json!(20)),
                        cell("A3", json!(30)),
                    ],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![
                        mk("B1", "SUBTOTAL(9,A1:A3)"),
                        mk("B2", "SUBTOTAL(1,A1:A3)"),
                        mk("B3", "TYPE(A1)"),
                        mk("C1", "HEX2DEC(\"FF\")"),
                        mk("C2", "DEC2BIN(5)"),
                        mk("C3", "DOLLARDE(1.02,16)"),
                        mk("D1", "DAYS360(DATE(2024,1,1),DATE(2024,3,1))"),
                        mk("D2", "GESTEP(5,4)"),
                        mk("D3", "DELTA(3,3)"),
                    ],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let got = |r: &str| {
        parsed.sheets[0].rows[1]
            .cells
            .iter()
            .find(|c| c.reference.as_deref() == Some(r))
            .and_then(|c| c.value.clone())
    };
    assert_eq!(got("B1"), Some(json!(60)));
    assert_eq!(got("B2"), Some(json!(20)));
    assert_eq!(got("B3"), Some(json!(1)));
    assert_eq!(got("C1"), Some(json!(255)));
    assert_eq!(got("C2"), Some(json!("101")));
    assert_eq!(got("C3"), Some(json!(1.125)));
    assert_eq!(got("D1"), Some(json!(60)));
    assert_eq!(got("D2"), Some(json!(1)));
    assert_eq!(got("D3"), Some(json!(1)));
}

#[test]
fn formula_engine_distributions() {
    let formulas = [
        ("A2", "NORM.S.DIST(0,TRUE)", 0.5),
        ("B2", "NORM.S.DIST(1.96,TRUE)", 0.975),
        ("C2", "NORM.S.INV(0.975)", 1.959964),
        ("D2", "ROUND(BINOM.DIST(2,4,0.5,FALSE),6)", 0.375),
        ("E2", "ROUND(POISSON.DIST(2,2,FALSE),5)", 0.27067),
        ("F2", "GAMMA(5)", 24.0),
        ("G2", "ROUND(GAMMALN(5),5)", 3.17805),
        ("H2", "ROUND(EXPON.DIST(1,1,TRUE),4)", 0.6321),
        ("I2", "ROUND(BETA.DIST(0.5,1,1,TRUE),6)", 0.5),
        ("J2", "ROUND(CHISQ.DIST(3.84,1,TRUE),3)", 0.95),
    ];
    let cells: Vec<Cell> = formulas
        .iter()
        .map(|(r, f, _)| Cell {
            reference: Some((*r).into()),
            formula: Some((*f).into()),
            ..Default::default()
        })
        .collect();
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![Row {
                index: Some(2),
                cells,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    for (r, _, expect) in &formulas {
        let got = parsed.sheets[0].rows[0]
            .cells
            .iter()
            .find(|c| c.reference.as_deref() == Some(*r))
            .and_then(|c| c.value.clone())
            .and_then(|v| v.as_f64());
        let got = got.unwrap_or_else(|| panic!("{r} no value"));
        assert!((got - expect).abs() < 1e-3, "{r}: got {got} want {expect}");
    }
}

#[test]
fn formula_engine_dist5() {
    let formulas = [
        ("A1", "STANDARDIZE(5,3,2)", 1.0),
        ("B1", "GAUSS(0)", 0.0),
        ("C1", "ROUND(T.DIST.RT(2.228,10),3)", 0.025),
        ("D1", "ROUND(T.INV(0.975,10),3)", 2.228),
        ("E1", "ROUND(CHISQ.INV.RT(0.05,1),3)", 3.841),
        ("F1", "ROUND(F.INV(0.95,1,1),1)", 161.4),
        ("G1", "ROUND(PDURATION(0.1,100,200),2)", 7.27),
        ("H1", "ROUND(RRI(2,100,121),6)", 0.1),
        ("I1", "ROUND(HYPGEOM.DIST(1,4,3,10,FALSE),6)", 0.5),
        ("J1", "ROUND(NEGBINOM.DIST(0,1,0.5,FALSE),6)", 0.5),
    ];
    let cells: Vec<Cell> = formulas
        .iter()
        .map(|(r, f, _)| Cell {
            reference: Some((*r).into()),
            formula: Some((*f).into()),
            ..Default::default()
        })
        .collect();
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![Row {
                index: Some(1),
                cells,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    for (r, _, expect) in &formulas {
        let got = parsed.sheets[0].rows[0]
            .cells
            .iter()
            .find(|c| c.reference.as_deref() == Some(*r))
            .and_then(|c| c.value.clone())
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| panic!("{r} no value"));
        assert!((got - expect).abs() < 1e-2, "{r}: got {got} want {expect}");
    }
}

#[test]
fn formula_engine_arrays() {
    let mk = |r: &str, f: &str| Cell {
        reference: Some(r.into()),
        formula: Some(f.into()),
        ..Default::default()
    };
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![
                        mk("A1", "SEQUENCE(3)"),
                        mk("C1", "ROW()"),
                        mk("C2", "COLUMN()"),
                        mk("C3", "ROWS(A1:A3)"),
                        mk("E1", "TRANSPOSE(SEQUENCE(3,1))"),
                        mk("P1", ""),
                    ],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![mk("E3", "OFFSET(A1,1,0)"), mk("P2", "")],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    // 填矩阵数据 P1:Q2 = [[1,2],[3,4]]
    let mut wb = wb;
    wb.sheets[0].rows[0]
        .cells
        .retain(|c| c.formula.is_none() || c.reference.as_deref() != Some("P1"));
    wb.sheets[0].rows[0].cells.push(cell("P1", json!(1)));
    wb.sheets[0].rows[0].cells.push(cell("Q1", json!(2)));
    wb.sheets[0].rows[1]
        .cells
        .retain(|c| c.formula.is_none() || c.reference.as_deref() != Some("P2"));
    wb.sheets[0].rows[1].cells.push(cell("P2", json!(3)));
    wb.sheets[0].rows[1].cells.push(cell("Q2", json!(4)));
    wb.sheets[0].rows.push(Row {
        index: Some(4),
        cells: vec![
            mk("S1", "MDETERM(P1:Q2)"),
            mk("V1", "FREQUENCY(P1:Q2,P1:P2)"),
            mk("W1", "SORT(P1:P2,1,-1)"),
            mk("X1", "UNIQUE(A1:A3)"),
            mk("Z1", "TAKE(A1:A3,2)"),
        ],
        ..Default::default()
    });

    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let all: Vec<(String, serde_json::Value)> = parsed.sheets[0]
        .rows
        .iter()
        .flat_map(|r| {
            r.cells
                .iter()
                .filter_map(|c| Some((c.reference.clone()?, c.value.clone()?)))
        })
        .collect();
    let got = |r: &str| all.iter().find(|(k, _)| k == r).map(|(_, v)| v.clone());
    assert_eq!(got("A1"), Some(json!(1)));
    assert_eq!(got("A2"), Some(json!(2)));
    assert_eq!(got("A3"), Some(json!(3)));
    assert_eq!(got("C1"), Some(json!(1)));
    assert_eq!(got("C2"), Some(json!(3)));
    assert_eq!(got("C3"), Some(json!(3)));
    assert_eq!(got("E1"), Some(json!(1)));
    assert_eq!(got("F1"), Some(json!(2)));
    assert_eq!(got("G1"), Some(json!(3)));
    assert_eq!(got("E3"), Some(json!(2)));
    assert_eq!(got("S1"), Some(json!(-2)));
    assert_eq!(got("V1"), Some(json!(1)));
    assert_eq!(got("V2"), Some(json!(2)));
    assert_eq!(got("W1"), Some(json!(3)));
    assert_eq!(got("Z2"), Some(json!(2)));
}

#[test]
fn formula_engine_lambda() {
    let mk = |r: &str, f: &str| Cell {
        reference: Some(r.into()),
        formula: Some(f.into()),
        ..Default::default()
    };
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![
                Row {
                    index: Some(1),
                    cells: vec![
                        mk("A1", "LET(x,5,x*2)"),
                        mk("B1", "MAP(SEQUENCE(3),LAMBDA(v,v*10))"),
                        mk("C1", "REDUCE(0,SEQUENCE(3),LAMBDA(a,b,a+b))"),
                        mk("D1", "MAKEARRAY(2,2,LAMBDA(r,c,r*c))"),
                        mk("F1", "BYROW(SEQUENCE(4),LAMBDA(rw,SUM(rw)))"),
                        mk("G1", "SCAN(0,SEQUENCE(3),LAMBDA(a,b,a+b))"),
                    ],
                    ..Default::default()
                },
                Row {
                    index: Some(2),
                    cells: vec![mk("A2", "LET(x,1,y,2,x+y)")],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }],
        ..Default::default()
    };
    let path = common::temp_xlsx_path();
    generate(&wb, &path).expect("generate");
    let parsed = parse(&path).expect("parse");
    let _ = std::fs::remove_file(&path);
    let all: Vec<(String, serde_json::Value)> = parsed.sheets[0]
        .rows
        .iter()
        .flat_map(|r| {
            r.cells
                .iter()
                .filter_map(|c| Some((c.reference.clone()?, c.value.clone()?)))
        })
        .collect();
    let got = |r: &str| all.iter().find(|(k, _)| k == r).map(|(_, v)| v.clone());
    assert_eq!(got("A1"), Some(json!(10)));
    assert_eq!(got("A2"), Some(json!(3)));
    assert_eq!(got("B1"), Some(json!(10)));
    assert_eq!(got("B2"), Some(json!(20)));
    assert_eq!(got("B3"), Some(json!(30)));
    assert_eq!(got("C1"), Some(json!(6)));
    assert_eq!(got("D1"), Some(json!(1)));
    assert_eq!(got("E1"), Some(json!(2)));
    assert_eq!(got("D2"), Some(json!(2)));
    assert_eq!(got("E2"), Some(json!(4)));
    assert_eq!(got("F4"), Some(json!(4)));
    assert_eq!(got("G3"), Some(json!(6)));
}

#[test]
fn comment_on_empty_cell_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Notes".into(),
            rows: vec![Row {
                index: Some(3),
                cells: vec![Cell {
                    reference: Some("C3".into()),
                    comment: Some("note on empty".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&wb);
}
