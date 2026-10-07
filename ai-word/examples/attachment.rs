//! 图片 + 附件（OLE 嵌入对象）示例
//!
//! 运行：cargo run --example attachment

use json2docx::model::blocks::*;
use json2docx::model::{Document, Part};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::Path::new("examples/out");
    std::fs::create_dir_all(dir)?;
    let img = dir.join("asset.png");
    image::RgbImage::from_pixel(96, 64, image::Rgb([46, 116, 181])).save(&img)?;
    let xlsx = dir.join("data.xlsx");
    std::fs::write(&xlsx, b"PK\x03\x04demo-embedded-xlsx")?;

    let mut doc = Document {
        template: Some("report".to_string()),
        ..Default::default()
    };
    let mut part = Part::default();
    part.blocks.push(Block::Heading {
        level: 1,
        text: "项目验收报告".to_string(),
        numbering: None,
        align: Some("center".to_string()),
        runs: None,
    });
    part.blocks.push(Block::Paragraph(Paragraph::text(
        "本报告汇总交付物、验收结论与随附材料。",
    )));
    part.blocks.push(Block::Heading {
        level: 2,
        text: "1. 系统界面".to_string(),
        numbering: None,
        align: None,
        runs: None,
    });
    part.blocks.push(Block::Image(ImageBlock {
        src: img.to_string_lossy().to_string(),
        width: Some(320.0),
        align: Some("center".to_string()),
        caption: Some("图 1  系统主界面".to_string()),
        alt: Some("系统界面".to_string()),
        ..Default::default()
    }));
    part.blocks.push(Block::Heading {
        level: 2,
        text: "2. 数据附件".to_string(),
        numbering: None,
        align: None,
        runs: None,
    });
    part.blocks.push(Block::Paragraph(Paragraph::text(
        "随附测试数据表（双击图标可打开原始文件）：",
    )));
    part.blocks.push(Block::Attachment(AttachmentBlock {
        src: xlsx.to_string_lossy().to_string(),
        name: Some("测试数据.xlsx".to_string()),
        label: Some("测试数据.xlsx".to_string()),
        width: Some(64.0),
        height: Some(64.0),
        ..Default::default()
    }));
    doc.parts = vec![part];

    let r = json2docx::generate(&doc, "examples/out/attachment.docx")?;
    println!("已生成 {} ({} 个分片)", r.path, r.parts);
    Ok(())
}
