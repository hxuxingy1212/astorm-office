//! Debug helper: generate an xlsx with an image and a chart.
//! Usage: cargo run --example drawing_demo -- /tmp/out.xlsx

use json2xlsx::model::{Cell, Chart, Image, Row, Sheet, Size, Workbook};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or("/tmp/drawing_demo.xlsx".into());
    let gif = std::env::args().nth(2).unwrap_or("/tmp/test.gif".into());
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Data".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![Cell {
                    reference: Some("A1".into()),
                    value: Some(serde_json::json!("x")),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            images: vec![Image {
                src: gif,
                anchor: Some("C3".into()),
                size: Some(Size { w: 2.0, h: 1.5 }),
                offset: None,
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
    json2xlsx::generate(&wb, std::path::Path::new(&out)).unwrap();
    println!("wrote {out}");
}
