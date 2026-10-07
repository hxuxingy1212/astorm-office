//! CLI 集成测试：7 子命令端到端

use json2docx::model::blocks::*;
use json2docx::model::{Document, Part};
use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_json2docx")
}

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("json2docx-test-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn make_docx(path: &std::path::Path) {
    let mut doc = Document::default();
    let mut part = Part::default();
    part.blocks.push(Block::Heading {
        level: 1,
        text: "第一章".to_string(),
        numbering: None,
        align: None,
        runs: None,
    });
    part.blocks
        .push(Block::Paragraph(Paragraph::text("原始正文")));
    doc.parts = vec![part];
    json2docx::generate(&doc, path.to_str().unwrap()).unwrap();
}

#[test]
fn templates_lists_presets() {
    let out = Command::new(bin()).arg("templates").output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("academic-paper"));
    assert!(s.contains("report"));
}

#[test]
fn unpack_view_edit_repack_flow() {
    let dir = tmpdir("flow");
    std::fs::create_dir_all(&dir).unwrap();
    let docx = dir.join("in.docx");
    make_docx(&docx);

    // unpack
    let prod = dir.join("prod");
    let out = Command::new(bin())
        .args([
            "unpack",
            docx.to_str().unwrap(),
            "-o",
            prod.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(prod.join("document.json").exists());

    // view layout
    let out = Command::new(bin())
        .args(["view", prod.to_str().unwrap(), "/part[1]", "layout"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("原始正文"));

    // edit set (product dir, in place)
    let out = Command::new(bin())
        .args([
            "edit",
            prod.to_str().unwrap(),
            "/part[1]/paragraph[1]",
            "set",
            "--prop",
            "text=修改后正文",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // repack
    let out_docx = dir.join("out.docx");
    let out = Command::new(bin())
        .args([
            "repack",
            prod.to_str().unwrap(),
            "-o",
            out_docx.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed = json2docx::parse(out_docx.to_str().unwrap()).unwrap();
    let text = parsed
        .all_blocks()
        .map(|b| b.plain_text())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("修改后正文"));
}

#[test]
fn edit_docx_without_output_fails() {
    let dir = tmpdir("guard");
    std::fs::create_dir_all(&dir).unwrap();
    let docx = dir.join("in.docx");
    make_docx(&docx);
    let out = Command::new(bin())
        .args([
            "edit",
            docx.to_str().unwrap(),
            "/part[1]/paragraph[1]",
            "set",
            "--prop",
            "text=x",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn extract_template_command() {
    let dir = tmpdir("extract");
    std::fs::create_dir_all(&dir).unwrap();
    let docx = dir.join("in.docx");
    make_docx(&docx);
    let out = dir.join("tpl");
    let res = Command::new(bin())
        .args([
            "extract",
            docx.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        res.status.success(),
        "{}",
        String::from_utf8_lossy(&res.stderr)
    );
    assert!(out.join("template.json").exists());
    assert!(out.join("TEMPLATE.md").exists());
    let md = std::fs::read_to_string(out.join("TEMPLATE.md")).unwrap();
    assert!(md.contains("段落样式"));
}

#[test]
fn chart_and_textbox_flow() {
    let dir = tmpdir("feat");
    std::fs::create_dir_all(&dir).unwrap();
    let doc_json = dir.join("document.json");
    std::fs::write(
        &doc_json,
        r#"{
          "watermark": "DRAFT",
          "parts": [
            { "blocks": [
              { "type": "heading", "level": 1, "text": "报告" },
              { "type": "chart", "chart_type": "column", "title": "季度",
                "categories": ["Q1","Q2"], "series": [ { "name":"营收", "values":[10,20] } ] },
              { "type": "textbox", "text": "提示文字", "fill": "FFF2CC" }
            ] }
          ]
        }"#,
    )
    .unwrap();

    let out_docx = dir.join("out.docx");
    let out = Command::new(bin())
        .args([
            "repack",
            dir.to_str().unwrap(),
            "-o",
            out_docx.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed = json2docx::parse(out_docx.to_str().unwrap()).unwrap();
    let types: Vec<&str> = parsed.all_blocks().map(|b| b.type_name()).collect();
    assert!(types.contains(&"chart"));
    assert!(types.contains(&"textbox"));
    assert!(types.contains(&"heading"));
}

#[test]
fn render_html_output() {
    let dir = tmpdir("render");
    std::fs::create_dir_all(&dir).unwrap();
    let docx = dir.join("in.docx");
    make_docx(&docx);
    let html = dir.join("out.html");
    let out = Command::new(bin())
        .args([
            "render",
            docx.to_str().unwrap(),
            "/part[1]",
            "-o",
            html.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = std::fs::read_to_string(&html).unwrap();
    assert!(s.contains("原始正文"));
    assert!(s.contains("<h1>"));
}
