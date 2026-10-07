mod common;

use json2xlsx::model::{Cell, Chart, Image, Row, Sheet, Size, Workbook};
use json2xlsx::{generate, parse};
use serde_json::json;

#[test]
fn image_roundtrip() {
    let gif = common::write_test_gif("img");
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![Cell {
                    reference: Some("A1".into()),
                    value: Some(json!("logo")),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            images: vec![Image {
                src: gif.display().to_string(),
                anchor: Some("C3".into()),
                size: Some(Size { w: 2.0, h: 1.5 }),
                offset: None,
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let path = common::temp_xlsx_path();
    generate(&wb, &path).unwrap();

    // The drawing/media parts must exist.
    let f = std::fs::File::open(&path).unwrap();
    let zip = zip::ZipArchive::new(f).unwrap();
    let names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
    assert!(
        names.iter().any(|n| n == "xl/drawings/drawing1.xml"),
        "{names:?}"
    );
    assert!(
        names.iter().any(|n| n == "xl/media/image1.gif"),
        "{names:?}"
    );
    assert!(
        names
            .iter()
            .any(|n| n == "xl/worksheets/_rels/sheet1.xml.rels"),
        "{names:?}"
    );

    let parsed = parse(&path).unwrap();
    let sheet = &parsed.sheets[0];
    assert_eq!(sheet.images.len(), 1);
    assert_eq!(sheet.images[0].src, "xl/media/image1.gif");
    assert_eq!(sheet.images[0].anchor.as_deref(), Some("C3"));

    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(&gif);
}

#[test]
fn multi_series_chart_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Data".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("Quarter")),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("B1".into()),
                        value: Some(json!("2025")),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("C1".into()),
                        value: Some(json!("2026")),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            charts: vec![Chart {
                chart_type: "column".into(),
                data_range: Some("B2:C4".into()),
                categories: Some("A2:A4".into()),
                title: Some("YoY".into()),
                anchor: Some("E2".into()),
                size: Some(Size { w: 6.0, h: 4.0 }),
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let path = common::temp_xlsx_path();
    generate(&wb, &path).unwrap();

    let f = std::fs::File::open(&path).unwrap();
    let mut zip = zip::ZipArchive::new(f).unwrap();
    let mut chart = String::new();
    {
        use std::io::Read;
        zip.by_name("xl/charts/chart1.xml")
            .unwrap()
            .read_to_string(&mut chart)
            .unwrap();
    }
    assert_eq!(chart.matches("<c:ser>").count(), 2, "{chart}");

    let parsed = parse(&path).unwrap();
    assert_eq!(
        parsed.sheets[0].charts[0].data_range.as_deref(),
        Some("B2:C4")
    );
    let _ = std::fs::remove_file(&path);
}

#[test]
fn chart_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Data".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("Q")),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("B1".into()),
                        value: Some(json!("V")),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            charts: vec![Chart {
                chart_type: "column".into(),
                data_range: Some("B2:B4".into()),
                categories: Some("A2:A4".into()),
                title: Some("Revenue".into()),
                anchor: Some("E2".into()),
                size: Some(Size { w: 6.0, h: 4.0 }),
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
    let names: Vec<String> = zip.file_names().map(|s| s.to_string()).collect();
    assert!(
        names.iter().any(|n| n == "xl/charts/chart1.xml"),
        "{names:?}"
    );

    let parsed = parse(&path).unwrap();
    let chart = &parsed.sheets[0].charts[0];
    assert_eq!(chart.chart_type, "column");
    assert_eq!(chart.data_range.as_deref(), Some("B2:B4"));
    assert_eq!(chart.categories.as_deref(), Some("A2:A4"));
    assert_eq!(chart.title.as_deref(), Some("Revenue"));
    assert_eq!(chart.anchor.as_deref(), Some("E2"));

    let _ = std::fs::remove_file(&path);
}

#[test]
fn enhanced_chart_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Data".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("Q")),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("B1".into()),
                        value: Some(json!("2025")),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("C1".into()),
                        value: Some(json!("2026")),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            charts: vec![Chart {
                chart_type: "column".into(),
                data_range: Some("B2:C3".into()),
                categories: Some("A2:A3".into()),
                title: Some("营收".into()),
                legend: Some("bottom".into()),
                category_axis_title: Some("季度".into()),
                value_axis_title: Some("金额".into()),
                show_values: true,
                colors: vec!["FF0000".into(), "00AA00".into()],
                anchor: Some("E2".into()),
                size: Some(Size { w: 7.0, h: 4.5 }),
                offset: None,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };

    let path = common::temp_xlsx_path();
    generate(&wb, &path).unwrap();
    let parsed = parse(&path).unwrap();
    let chart = &parsed.sheets[0].charts[0];
    assert_eq!(chart.chart_type, "column");
    assert_eq!(chart.data_range.as_deref(), Some("B2:C3"));
    assert_eq!(chart.legend.as_deref(), Some("bottom"));
    assert_eq!(chart.category_axis_title.as_deref(), Some("季度"));
    assert_eq!(chart.value_axis_title.as_deref(), Some("金额"));
    assert!(chart.show_values);
    assert_eq!(
        chart.colors,
        vec!["FF0000".to_string(), "00AA00".to_string()]
    );
    let _ = std::fs::remove_file(&path);
}
