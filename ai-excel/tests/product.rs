mod common;

use json2xlsx::model::{Cell, Image, Row, Sheet, Size, Workbook};
use json2xlsx::{generate, is_product_dir, parse, repack, unpack};
use serde_json::json;

#[test]
fn unpack_repack_roundtrip() {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Data".into(),
            tab_color: Some("112233".into()),
            merges: vec!["A1:B1".into()],
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("标题")),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("B2".into()),
                        value: Some(json!(3.5)),
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

    let xlsx = common::temp_xlsx_path();
    generate(&wb, &xlsx).unwrap();

    let dir = common::temp_dir("product");
    let r = unpack(&xlsx, &dir).unwrap();
    assert_eq!(r.sheets, 1);
    assert!(is_product_dir(&dir));
    assert!(dir.join("workbook.json").is_file());
    assert!(dir.join("xl/worksheets/sheet1.json").is_file());

    let out = common::temp_xlsx_path();
    repack(&dir, &out).unwrap();
    let parsed = parse(&out).unwrap();
    assert_eq!(parsed, wb.normalized());

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&xlsx);
    let _ = std::fs::remove_file(&out);
}

#[test]
fn unpack_extracts_media_and_repacks() {
    let gif = common::write_test_gif("prodimg");
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "S".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![Cell {
                    reference: Some("A1".into()),
                    value: Some(json!("x")),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            images: vec![Image {
                src: gif.display().to_string(),
                anchor: Some("C3".into()),
                size: Some(Size { w: 1.0, h: 1.0 }),
                offset: None,
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    let xlsx = common::temp_xlsx_path();
    generate(&wb, &xlsx).unwrap();

    let dir = common::temp_dir("prodimg");
    unpack(&xlsx, &dir).unwrap();
    assert!(dir.join("xl/media/image1.gif").is_file());

    let out = common::temp_xlsx_path();
    repack(&dir, &out).unwrap();
    let parsed = parse(&out).unwrap();
    assert_eq!(parsed.sheets[0].images.len(), 1);
    assert_eq!(parsed.sheets[0].images[0].src, "xl/media/image1.gif");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&xlsx);
    let _ = std::fs::remove_file(&out);
    let _ = std::fs::remove_file(&gif);
}
