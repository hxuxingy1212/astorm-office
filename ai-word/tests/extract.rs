//! 模板提取测试：从生成的文档中提取页面/主题/段落样式

use json2docx::model::blocks::*;
use json2docx::model::{Document, Part};

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(format!("json2docx-extract-{}-{}", std::process::id(), name))
        .to_string_lossy()
        .to_string()
}

#[test]
fn extract_academic_style() {
    let mut doc = Document {
        template: Some("academic-paper".to_string()),
        ..Default::default()
    };
    doc.parts = vec![Part::new(vec![
        Block::Heading {
            level: 1,
            text: "绪论".to_string(),
            numbering: None,
            align: None,
            runs: None,
        },
        Block::Paragraph(Paragraph::text("正文段落内容。")),
    ])];
    let path = tmp("tpl.docx");
    json2docx::generate(&doc, &path).expect("generate");

    let ext = json2docx::extract_template(&path).expect("extract");
    let normal = ext.styles.get("Normal").expect("Normal");
    assert_eq!(normal.font_size, Some(12.0));
    assert_eq!(normal.font_family.as_deref(), Some("Times New Roman"));
    assert_eq!(normal.east_asia_font.as_deref(), Some("宋体"));

    let h1 = ext.styles.get("Heading1").expect("Heading1");
    assert_eq!(h1.bold, Some(true));
    assert_eq!(h1.font_size, Some(18.0));

    assert_eq!(ext.theme.minor_font.as_deref(), Some("Times New Roman"));
    assert_eq!(ext.theme.east_asia_font.as_deref(), Some("宋体"));

    let md = json2docx::describe_template(&ext, "tpl.docx");
    assert!(md.contains("Heading1"));
    assert!(md.contains("Times New Roman"));
    assert!(md.contains("## 段落样式"));
}

#[test]
fn extract_report_style() {
    let mut doc = Document {
        template: Some("report".to_string()),
        ..Default::default()
    };
    doc.parts = vec![Part::new(vec![Block::Paragraph(Paragraph::text(
        "报告正文",
    ))])];
    let path = tmp("rep.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let ext = json2docx::extract_template(&path).expect("extract");
    let normal = ext.styles.get("Normal").expect("Normal");
    assert_eq!(normal.font_family.as_deref(), Some("Calibri"));
    assert_eq!(normal.font_size, Some(11.0));
}
