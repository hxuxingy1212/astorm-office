//! 综合示例：覆盖脚注/尾注/批注/修订/交叉引用/合并单元格/浮动图/公式/静态目录等
//!
//! 运行：cargo run --example features

use json2docx::model::blocks::*;
use json2docx::model::style::{PageSetup, Style};
use json2docx::model::{Document, Meta, Part, Section};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 准备一张真实图片供插图使用
    let out_dir = std::path::Path::new("examples/out");
    std::fs::create_dir_all(out_dir)?;
    let img_path = out_dir.join("asset.png");
    let img = image::RgbImage::from_pixel(64, 48, image::Rgb([46, 116, 181]));
    img.save(&img_path)?;

    let mut doc = Document {
        meta: Meta {
            title: Some("功能总览".to_string()),
            author: Some("json2docx".to_string()),
            subject: Some("feature showcase".to_string()),
            keywords: vec!["演示".to_string()],
        },
        template: Some("report".to_string()),
        ..Default::default()
    };
    doc.styles.insert(
        "Normal".to_string(),
        Style {
            font_size: Some(11.0),
            line_spacing: Some(1.3),
            ..Default::default()
        },
    );
    doc.watermark = Some("DRAFT".to_string());
    doc.page = PageSetup {
        size: "A4".to_string(),
        columns: None,
        ..Default::default()
    };

    let mut p1 = Part::default();
    p1.section = Some(Section {
        header: Some("功能总览".to_string()),
        footer_page_number: Some(true),
        page_number_start: Some(1),
        ..Default::default()
    });
    p1.blocks.push(Block::Heading {
        level: 1,
        text: "功能总览".to_string(),
        numbering: None,
        align: None,
        runs: None,
    });
    p1.blocks.push(Block::Paragraph(Paragraph::text(
        "本示例演示 json2docx 的主要能力。",
    )));
    p1.blocks.push(Block::Toc(TocBlock {
        title: "目录".to_string(),
        levels: Some(vec![1, 2, 3]),
        is_static: Some(true),
        entries: Vec::new(),
    }));

    // 行内混排 + 脚注 + 尾注 + 批注 + 修订 + 书签
    p1.blocks.push(Block::Paragraph(Paragraph {
        runs: Some(vec![
            Run {
                text: "加粗".to_string(),
                bold: Some(true),
                ..Default::default()
            },
            Run::plain("、"),
            Run {
                text: "链接".to_string(),
                hyperlink: Some("https://example.com".to_string()),
                ..Default::default()
            },
            Run::plain("、"),
            Run {
                text: "脚注".to_string(),
                footnote: Some("这是一条脚注说明。".to_string()),
                ..Default::default()
            },
            Run::plain("、"),
            Run {
                text: "尾注".to_string(),
                endnote: Some("这是一条尾注。".to_string()),
                ..Default::default()
            },
            Run::plain("、"),
            Run {
                text: "批注".to_string(),
                comment: Some("审阅意见：请补充数据来源。".to_string()),
                comment_author: Some("审阅者".to_string()),
                ..Default::default()
            },
            Run::plain("、"),
            Run {
                text: "新增".to_string(),
                revision: Some("ins".to_string()),
                revision_author: Some("Alice".to_string()),
                ..Default::default()
            },
            Run {
                text: "删除".to_string(),
                revision: Some("del".to_string()),
                revision_author: Some("Alice".to_string()),
                ..Default::default()
            },
        ]),
        ..Default::default()
    }));

    // 多级列表
    p1.blocks.push(Block::List(ListBlock {
        ordered: true,
        items: vec![
            ListItem {
                blocks: vec![Block::Paragraph(Paragraph::text("一级项"))],
                level: 0,
                ..Default::default()
            },
            ListItem {
                blocks: vec![Block::Paragraph(Paragraph::text("二级项"))],
                level: 1,
                ..Default::default()
            },
        ],
    }));

    // 合并单元格表格 + 自动编号题注
    let cell = |s: &str| TableCell {
        blocks: vec![Block::Paragraph(Paragraph::text(s))],
        ..Default::default()
    };
    p1.blocks.push(Block::Table(TableBlock {
        header_row: true,
        widths: Some(vec![120.0, 180.0]),
        caption: Some("合并单元格与题注".to_string()),
        rows: vec![
            TableRow {
                cells: vec![TableCell {
                    colspan: Some(2),
                    ..cell("跨两列标题")
                }],
                ..Default::default()
            },
            TableRow {
                cells: vec![
                    TableCell {
                        rowspan: Some(2),
                        ..cell("跨两行")
                    },
                    cell("第二列上"),
                ],
                ..Default::default()
            },
            TableRow {
                cells: vec![cell("第二列下")],
                ..Default::default()
            },
        ],
        ..Default::default()
    }));

    // 浮动图片
    p1.blocks.push(Block::Image(ImageBlock {
        src: img_path.to_string_lossy().to_string(),
        width: Some(80.0),
        height: Some(60.0),
        align: None,
        caption: Some("浮动图片".to_string()),
        alt: Some("示例图".to_string()),
        wrap: Some("square".to_string()),
        x: Some(320.0),
        y: Some(360.0),
        behind_text: Some(false),
        crop: None,
        rotation: None,
    }));

    // 公式与代码
    p1.blocks.push(Block::Formula(FormulaBlock {
        latex: "\\sum_{i=1}^{n} x_i^2".to_string(),
        display: true,
        number: Some("(1)".to_string()),
    }));
    // 文本框
    p1.blocks.push(Block::TextBox(TextBoxBlock {
        text: "提示：本段为文本框内容。".to_string(),
        width: Some(260.0),
        height: Some(48.0),
        fill: Some("FFF2CC".to_string()),
        line: Some("C00000".to_string()),
        align: Some("center".to_string()),
        wrap: None,
        x: None,
        y: None,
        font_size: Some(10.0),
        color: Some("333333".to_string()),
        vml: None,
    }));
    // 原生柱状图
    p1.blocks.push(Block::Chart(ChartBlock {
        chart_type: "column".to_string(),
        title: Some("季度营收".to_string()),
        categories: vec!["Q1".to_string(), "Q2".to_string(), "Q3".to_string()],
        series: vec![ChartSeries {
            name: "营收".to_string(),
            values: vec![120.0, 150.0, 180.0],
            ..Default::default()
        }],
        width: Some(360.0),
        height: Some(200.0),
        align: None,
        ..Default::default()
    }));
    p1.blocks.push(Block::Code(CodeBlock {
        lang: Some("rust".to_string()),
        text: "fn main() {\n    println!(\"hello\");\n}".to_string(),
    }));
    p1.blocks.push(Block::Quote(QuoteBlock {
        text: "引用一段话。".to_string(),
    }));

    // 参考文献
    p1.blocks.push(Block::Bibliography(BibliographyBlock {
        style: "GB/T 7714".to_string(),
        title: Some("参考文献".to_string()),
        entries: vec![BibEntry {
            kind: Some("article".to_string()),
            authors: vec!["张三".to_string()],
            title: "示例文献".to_string(),
            container: Some("示例期刊".to_string()),
            year: Some(2024),
            ..Default::default()
        }],
    }));

    doc.parts = vec![p1];
    let r = json2docx::generate(&doc, "examples/out/features.docx")?;
    println!("已生成 {} ({} 个分片)", r.path, r.parts);
    Ok(())
}
