//! CLI integration tests: spawn the built binary and exercise the workflow.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use json2xlsx::model::{Cell, Row, Sheet, Workbook};
use serde_json::json;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_json2xlsx")
}

fn unique(tag: &str, ext: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "json2xlsx_cli_{tag}_{}_{}.{ext}",
        std::process::id(),
        n
    ))
}

fn run(args: &[&str]) -> (bool, String) {
    let out = Command::new(bin()).args(args).output().expect("spawn cli");
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), s)
}

fn make_sample(path: &Path) {
    let wb = Workbook {
        sheets: vec![Sheet {
            name: "Sheet1".into(),
            rows: vec![Row {
                index: Some(1),
                cells: vec![
                    Cell {
                        reference: Some("A1".into()),
                        value: Some(json!("hello")),
                        ..Default::default()
                    },
                    Cell {
                        reference: Some("B1".into()),
                        value: Some(json!(2.5)),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    };
    json2xlsx::generate(&wb, path).unwrap();
}

#[test]
fn unpack_view_edit_repack() {
    let xlsx = unique("in", "xlsx");
    make_sample(&xlsx);
    let dir = unique("prod", "d");
    let out = unique("out", "xlsx");

    let (ok, msg) = run(&[
        "unpack",
        xlsx.to_str().unwrap(),
        "-o",
        dir.to_str().unwrap(),
    ]);
    assert!(ok, "{msg}");
    assert!(dir.join("workbook.json").is_file());

    let (ok, msg) = run(&[
        "view",
        dir.to_str().unwrap(),
        "/sheet[1]/cell[A1]",
        "values",
    ]);
    assert!(ok, "{msg}");
    assert!(msg.contains("hello"), "{msg}");

    let (ok, msg) = run(&[
        "edit",
        dir.to_str().unwrap(),
        "/sheet[1]/cell[A1]",
        "set",
        "--prop",
        "value=changed",
        "--prop",
        "font.bold=true",
    ]);
    assert!(ok, "{msg}");

    let (ok, msg) = run(&[
        "edit",
        dir.to_str().unwrap(),
        "/sheet[1]",
        "add",
        "--type",
        "row",
        "--prop",
        "index=2",
    ]);
    assert!(ok, "{msg}");

    let (ok, msg) = run(&[
        "edit",
        dir.to_str().unwrap(),
        "/sheet[1]/cell[A2]",
        "set",
        "--prop",
        "value=row2",
    ]);
    assert!(ok, "{msg}");

    let (ok, msg) = run(&["repack", dir.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(ok, "{msg}");

    let wb = json2xlsx::parse(&out).unwrap();
    let cell = &wb.sheets[0].rows[0].cells[0];
    assert_eq!(cell.value, Some(json!("changed")));

    let (ok, msg) = run(&[
        "render",
        dir.to_str().unwrap(),
        "-o",
        unique("r", "html").to_str().unwrap(),
    ]);
    assert!(ok, "{msg}");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&xlsx);
    let _ = std::fs::remove_file(&out);
}

#[test]
fn template_extract_writes_bundle() {
    let xlsx = unique("tpl", "xlsx");
    make_sample(&xlsx);
    let dir = unique("tplout", "d");
    let (ok, msg) = run(&[
        "template",
        "extract",
        xlsx.to_str().unwrap(),
        "-o",
        dir.to_str().unwrap(),
    ]);
    assert!(ok, "{msg}");
    assert!(dir.join("template.json").is_file(), "{msg}");
    assert!(dir.join("TEMPLATE.md").is_file(), "{msg}");
    assert!(dir.join("skeleton/workbook.json").is_file(), "{msg}");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&xlsx);
}

#[test]
fn write_to_xlsx_without_output_fails() {
    let xlsx = unique("in2", "xlsx");
    make_sample(&xlsx);
    let (ok, msg) = run(&[
        "edit",
        xlsx.to_str().unwrap(),
        "/sheet[1]/cell[A1]",
        "set",
        "--prop",
        "value=x",
    ]);
    assert!(!ok, "expected failure: {msg}");
    assert!(msg.contains("-o"), "{msg}");
    let _ = std::fs::remove_file(&xlsx);
}
