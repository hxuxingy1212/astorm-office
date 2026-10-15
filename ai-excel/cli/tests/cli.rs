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

/// 写一个最小产物目录（workbook.json + 两张表），用于验证 markdown 视图
fn write_product(dir: &Path) {
    std::fs::create_dir_all(dir.join("xl/worksheets")).unwrap();
    std::fs::write(
        dir.join("workbook.json"),
        r#"{"sheets": ["xl/worksheets/sheet1.json", "xl/worksheets/sheet2.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("xl/worksheets/sheet1.json"),
        r#"{"name": "汇总", "merges": ["A1:B1"], "rows": [
            {"index": 1, "cells": [{"ref":"A1","value":"月份"},{"ref":"B1","value":"收入"}]},
            {"index": 2, "cells": [{"ref":"A2","value":"1月"},{"ref":"B2","value":100}]},
            {"index": 3, "cells": [{"ref":"A3","value":"合计"},{"ref":"B3","formula":"SUM(B2:B2)"}]}
        ]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("xl/worksheets/sheet2.json"),
        r#"{"name": "空表", "rows": []}"#,
    )
    .unwrap();
}

#[test]
fn view_text_emits_markdown_tables() {
    let dir = unique("md_prod", "d");
    write_product(&dir);
    let d = dir.to_str().unwrap();
    let run_view = |args: &[&str]| -> (bool, String, String) {
        let mut all = vec!["view", d];
        all.extend_from_slice(args);
        let out = Command::new(bin()).args(&all).output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    };
    // --json：结构化信封（markdown + range 等）
    let view = |args: &[&str]| -> (bool, serde_json::Value, String) {
        let mut all = args.to_vec();
        all.push("--json");
        let (ok, stdout, stderr) = run_view(&all);
        let v = serde_json::from_str(&stdout).unwrap_or(serde_json::Value::Null);
        (ok, v, stderr)
    };

    // 默认（不加 --json）：stdout 直出 Markdown 表格，且无注释标记
    let (ok, md, err) = run_view(&["/sheet[1]", "text"]);
    assert!(ok, "{err}");
    assert!(md.starts_with("| 月份 | 收入 |\n| --- | --- |"), "{md}");
    assert!(md.contains("| 合计 | =SUM(B2:B2) |"), "{md}");
    assert!(!md.contains("<!--"), "纯 Markdown 不带注释标记：{md}");

    // 整本（`/`）默认也是 Markdown：每张表一个小节
    let (ok, md, err) = run_view(&["/", "text"]);
    assert!(ok, "{err}");
    assert!(md.contains("## 汇总") && md.contains("## 空表"), "{md}");

    // 单表（--json）：GFM 表格 + 公式显示为 =公式
    let (ok, v, err) = view(&["/sheet[1]", "text"]);
    assert!(ok, "{err}");
    assert_eq!(v["sheet"], "汇总");
    assert_eq!(v["range"], "A1:B3");
    let md = v["markdown"].as_str().unwrap();
    assert!(md.starts_with("| 月份 | 收入 |\n| --- | --- |"), "{md}");

    // 区域视图：限定窗口的表格
    let (ok, v, err) = view(&["/sheet[1]/range[A1:B2]", "text"]);
    assert!(ok, "{err}");
    assert_eq!(v["range"], "A1:B2");
    assert!(
        v["markdown"].as_str().unwrap().contains("| 1月 | 100 |"),
        "{v}"
    );

    // 单元格视图：不套 markdown（单节点没有 markdown），默认即输出完整字段 JSON
    let (ok, stdout, err) = run_view(&["/sheet[1]/cell[B3]", "text"]);
    assert!(ok, "{err}");
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(v["formula"], "SUM(B2:B2)");
    assert_eq!(v["ref"], "B3");

    // 整本（`/`）--json：结构化信封
    let (ok, v, err) = view(&["/", "text"]);
    assert!(ok, "{err}");
    assert_eq!(v["sheets"], 2);
    assert!(v["markdown"].as_str().unwrap().contains("## 汇总"));

    // 整本不支持 layout
    let (ok, _v, err) = view(&["/", "layout"]);
    assert!(!ok, "整本 layout 应当报错");
    assert!(err.contains("不支持"), "{err}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn view_text_exports_ole_attachments() {
    let dir = unique("attach_prod", "d");
    std::fs::create_dir_all(dir.join("xl/worksheets")).unwrap();
    std::fs::create_dir_all(dir.join("xl/embeddings")).unwrap();
    let payload = b"PK\x03\x04-embedded-docx";
    std::fs::write(dir.join("xl/embeddings/oleObject1.bin"), payload).unwrap();
    std::fs::write(
        dir.join("workbook.json"),
        r#"{"sheets": ["xl/worksheets/sheet1.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("xl/worksheets/sheet1.json"),
        r#"{"name": "报表", "rows": [
            {"index": 1, "cells": [{"ref": "A1", "value": "指标"}, {"ref": "B1", "value": "值"}]},
            {"index": 2, "cells": [{"ref": "A2", "value": "收入"}, {"ref": "B2", "value": 928.3}]}
        ], "ole_objects": [{"prog_id": "Word.Document.12", "src": "xl/embeddings/oleObject1.bin"}]}"#,
    )
    .unwrap();
    let d = dir.to_str().unwrap();
    let att = dir.join("out");

    // 默认：表格照旧，不渲染附件行
    let (ok, out) = run(&["view", d, "/sheet[1]", "text"]);
    assert!(ok, "{out}");
    assert!(!out.contains("附件"), "默认不应出现附件行：{out}");

    // --attachments：导出 OLE 载荷，表格后追加附件行指向新路径
    let (ok, out) = run(&[
        "view",
        d,
        "/sheet[1]",
        "text",
        "--attachments",
        att.to_str().unwrap(),
    ]);
    assert!(ok, "{out}");
    let exported = att.join("oleObject1.bin");
    assert!(exported.is_file(), "附件应已导出：{out}");
    assert_eq!(std::fs::read(&exported).unwrap(), payload);
    assert!(
        out.contains(&format!("[oleObject1.bin]({})", exported.display())),
        "{out}"
    );
    assert!(out.contains("（附件 · Word.Document.12）"), "{out}");

    // 二进制输入（.xlsx）：解析侧现在会读 ole_objects，载荷从包里抽到临时目录再导出
    let xlsx = dir.join("book.xlsx");
    let (ok, msg) = run(&["repack", d, "-o", xlsx.to_str().unwrap()]);
    assert!(ok, "{msg}");
    let att2 = dir.join("out2");
    let (ok, out) = run(&[
        "view",
        xlsx.to_str().unwrap(),
        "/sheet[1]",
        "text",
        "--attachments",
        att2.to_str().unwrap(),
    ]);
    assert!(ok, "{out}");
    let exported2 = att2.join("oleObject1.bin");
    assert!(exported2.is_file(), "二进制输入也应导出载荷：{out}");
    assert_eq!(std::fs::read(&exported2).unwrap(), payload);
    assert!(
        out.contains(&format!("[oleObject1.bin]({})", exported2.display())),
        "{out}"
    );

    // 载荷不在（src 指向不存在的文件）：保留原引用、不报成功
    std::fs::remove_file(dir.join("xl/embeddings/oleObject1.bin")).unwrap();
    let mut all = vec![
        "view",
        d,
        "/sheet[1]",
        "text",
        "--attachments",
        att.to_str().unwrap(),
    ];
    all.push("--json");
    let p = std::process::Command::new(bin())
        .args(&all)
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&p.stdout).unwrap();
    assert_eq!(v["attachments"][0]["kind"], "missing", "{v}");

    let _ = std::fs::remove_dir_all(&dir);
}
