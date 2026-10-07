//! 生成一份示例论文并解析回 JSON

use json2docx::model::blocks::*;
use json2docx::model::{Document, Meta, Part};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut doc = Document {
        meta: Meta {
            title: Some("基于大模型的多模态检索方法研究".to_string()),
            author: Some("张三".to_string()),
            subject: Some("计算机科学".to_string()),
            keywords: vec!["大模型".to_string(), "多模态".to_string()],
        },
        template: Some("academic-paper".to_string()),
        ..Default::default()
    };

    let mut part = Part::default();
    part.blocks.push(Block::Heading {
        level: 1,
        text: "第一章 绪论".to_string(),
        numbering: Some(true),
        align: None,
        runs: None,
    });
    part.blocks.push(Block::Paragraph(Paragraph {
        text: Some("本文提出了一种基于大模型的多模态检索方法。".to_string()),
        first_line_indent: Some(24.0),
        ..Default::default()
    }));
    part.blocks.push(Block::List(ListBlock {
        ordered: true,
        items: vec![
            ListItem {
                blocks: vec![Block::Paragraph(Paragraph::text("研究背景"))],
                level: 0,
                ..Default::default()
            },
            ListItem {
                blocks: vec![Block::Paragraph(Paragraph::text("主要贡献"))],
                level: 0,
                ..Default::default()
            },
        ],
    }));
    part.blocks.push(Block::Table(TableBlock {
        header_row: true,
        widths: Some(vec![120.0, 120.0]),
        rows: vec![
            TableRow {
                cells: vec![
                    TableCell {
                        blocks: vec![Block::Paragraph(Paragraph::text("方法"))],
                        ..Default::default()
                    },
                    TableCell {
                        blocks: vec![Block::Paragraph(Paragraph::text("准确率"))],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            TableRow {
                cells: vec![
                    TableCell {
                        blocks: vec![Block::Paragraph(Paragraph::text("Ours"))],
                        ..Default::default()
                    },
                    TableCell {
                        blocks: vec![Block::Paragraph(Paragraph::text("92.3%"))],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        ],
        caption: Some("表 1  实验对比".to_string()),
        ..Default::default()
    }));
    part.blocks.push(Block::Formula(FormulaBlock {
        latex: "E = mc^2".to_string(),
        display: true,
        number: Some("(1)".to_string()),
    }));
    part.blocks.push(Block::Bibliography(BibliographyBlock {
        style: "GB/T 7714".to_string(),
        title: Some("参考文献".to_string()),
        entries: vec![BibEntry {
            kind: Some("article".to_string()),
            authors: vec!["张三".to_string()],
            title: "多模态检索综述".to_string(),
            container: Some("计算机学报".to_string()),
            year: Some(2024),
            ..Default::default()
        }],
    }));
    doc.parts = vec![part];

    let result = json2docx::generate(&doc, "examples/out/simple.docx")?;
    println!("已生成 {} ({} 分片)", result.path, result.parts);

    let parsed = json2docx::parse("examples/out/simple.docx")?;
    println!("{}", serde_json::to_string_pretty(&parsed)?);
    Ok(())
}
