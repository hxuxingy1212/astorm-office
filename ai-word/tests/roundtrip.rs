//! 回环测试：JSON → DOCX → JSON

use json2docx::model::blocks::*;
use json2docx::model::{Document, Meta, Part};

fn tmp(ext: &str) -> String {
    let dir = std::env::temp_dir().join(format!("json2docx-test-{}-{}", std::process::id(), ext));
    dir.to_string_lossy().to_string()
}

fn sample_doc() -> Document {
    let mut doc = Document {
        meta: Meta {
            title: Some("测试论文".to_string()),
            author: Some("张三".to_string()),
            ..Default::default()
        },
        template: None,
        ..Default::default()
    };
    let mut part = Part::default();
    part.blocks.push(Block::Heading {
        level: 1,
        text: "第一章 绪论".to_string(),
        numbering: None,
        align: None,
        runs: None,
    });
    part.blocks.push(Block::Paragraph(Paragraph {
        text: Some("本文提出了一种方法。".to_string()),
        first_line_indent: Some(24.0),
        ..Default::default()
    }));
    part.blocks.push(Block::Paragraph(Paragraph {
        runs: Some(vec![
            Run {
                text: "加粗".to_string(),
                bold: Some(true),
                ..Default::default()
            },
            Run::plain("普通"),
            Run {
                text: "链接".to_string(),
                hyperlink: Some("https://example.com".to_string()),
                ..Default::default()
            },
        ]),
        ..Default::default()
    }));
    part.blocks.push(Block::List(ListBlock {
        ordered: true,
        items: vec![
            ListItem {
                blocks: vec![Block::Paragraph(Paragraph::text("第一点"))],
                level: 0,
                ..Default::default()
            },
            ListItem {
                blocks: vec![Block::Paragraph(Paragraph::text("第二点"))],
                level: 0,
                ..Default::default()
            },
        ],
    }));
    part.blocks.push(Block::Table(TableBlock {
        header_row: true,
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
        widths: None,
        font_size: None,
        align: None,
        ..Default::default()
    }));
    part.blocks.push(Block::Formula(FormulaBlock {
        latex: "E = mc^2".to_string(),
        display: true,
        number: None,
    }));
    part.blocks.push(Block::Code(CodeBlock {
        lang: None,
        text: "fn main() {\n    println!(\"hi\");\n}".to_string(),
    }));
    part.blocks.push(Block::Quote(QuoteBlock {
        text: "引用一段话".to_string(),
    }));
    doc.parts = vec![part];
    doc
}

fn flatten(doc: &Document) -> Vec<Block> {
    doc.parts.iter().flat_map(|p| p.blocks.clone()).collect()
}

/// 公式 LaTeX 归一（忽略空白与花括号差异）
fn norm_latex(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace() && *c != '{' && *c != '}')
        .collect()
}

fn normalize(blocks: Vec<Block>) -> Vec<Block> {
    blocks
        .into_iter()
        .map(|b| match b {
            Block::Formula(mut f) => {
                f.latex = norm_latex(&f.latex);
                Block::Formula(f)
            }
            other => other,
        })
        .collect()
}

#[test]
fn roundtrip_basic() {
    let doc = sample_doc();
    let path = tmp("basic.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(parsed.meta.title.as_deref(), Some("测试论文"));
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn roundtrip_unpack_repack() {
    let doc = sample_doc();
    let docx = tmp("ur.docx");
    json2docx::generate(&doc, &docx).expect("generate");

    let dir = tmp("ur_product");
    let _ = std::fs::remove_dir_all(&dir);
    let r = json2docx::unpack(&docx, &dir).expect("unpack");
    assert!(std::path::Path::new(&dir).join("document.json").exists());
    assert!(r.parts >= 1);

    let packed = tmp("ur_repacked.docx");
    json2docx::repack(&dir, &packed).expect("repack");
    let parsed = json2docx::parse(&packed).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn math_roundtrip() {
    let formulas = [
        "E = mc^2",
        "\\frac{a}{b}",
        "\\sqrt{x}",
        "\\sum_{i=1}^{n} x_i",
        "x_{t+1}^2",
    ];
    let mut doc = Document::default();
    let mut part = Part::default();
    for latex in formulas {
        part.blocks.push(Block::Formula(FormulaBlock {
            latex: latex.to_string(),
            display: true,
            number: None,
        }));
    }
    doc.parts = vec![part];

    let path = tmp("math.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));

    // 希腊字母/符号转 Unicode
    let mut g = Document::default();
    let mut gp = Part::default();
    gp.blocks.push(Block::Formula(FormulaBlock {
        latex: "\\lambda + \\alpha \\le \\infty".to_string(),
        display: true,
        number: None,
    }));
    g.parts = vec![gp];
    let gp_path = tmp("math2.docx");
    json2docx::generate(&g, &gp_path).expect("generate");
    let gparsed = json2docx::parse(&gp_path).expect("parse");
    let latex = gparsed
        .all_blocks()
        .find_map(|b| match b {
            Block::Formula(f) => Some(f.latex.clone()),
            _ => None,
        })
        .unwrap();
    assert!(
        latex.contains('λ') && latex.contains('α') && latex.contains('≤') && latex.contains('∞')
    );
}

#[test]
fn table_merge_roundtrip() {
    fn p(s: &str) -> TableCell {
        TableCell {
            blocks: vec![Block::Paragraph(Paragraph::text(s))],
            ..Default::default()
        }
    }
    let rowspan_table = TableBlock {
        header_row: false,
        widths: None,
        rows: vec![
            TableRow {
                cells: vec![
                    TableCell {
                        rowspan: Some(2),
                        ..p("A")
                    },
                    p("B"),
                ],
                ..Default::default()
            },
            TableRow {
                cells: vec![p("C")],
                ..Default::default()
            },
        ],
        caption: None,
        font_size: None,
        align: None,
        ..Default::default()
    };
    let colspan_table = TableBlock {
        header_row: false,
        widths: None,
        rows: vec![
            TableRow {
                cells: vec![TableCell {
                    colspan: Some(2),
                    ..p("H")
                }],
                ..Default::default()
            },
            TableRow {
                cells: vec![p("x"), p("y")],
                ..Default::default()
            },
        ],
        caption: None,
        font_size: None,
        align: None,
        ..Default::default()
    };

    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![
        Block::Table(rowspan_table),
        Block::Table(colspan_table),
    ])];
    let path = tmp("merge.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn list_level_roundtrip() {
    let li = |s: &str, level: u8| ListItem {
        blocks: vec![Block::Paragraph(Paragraph::text(s))],
        level,
        ..Default::default()
    };
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![Block::List(ListBlock {
        ordered: false,
        items: vec![li("A", 0), li("B", 1), li("C", 1), li("D", 0)],
    })])];
    let path = tmp("listlvl.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn textbox_roundtrip() {
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![Block::TextBox(TextBoxBlock {
        text: "注意：这是一段说明文字。".to_string(),
        width: Some(200.0),
        height: Some(50.0),
        fill: Some("FFF2CC".to_string()),
        line: Some("C00000".to_string()),
        align: Some("center".to_string()),
        wrap: None,
        x: None,
        y: None,
        font_size: Some(10.0),
        color: Some("333333".to_string()),
        vml: None,
    })])];
    let path = tmp("textbox.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn chart_roundtrip() {
    use std::io::Read;
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![
        Block::Chart(ChartBlock {
            chart_type: "bar".to_string(),
            title: Some("季度销售".to_string()),
            categories: vec!["Q1".to_string(), "Q2".to_string(), "Q3".to_string()],
            series: vec![ChartSeries {
                name: "营收".to_string(),
                values: vec![30.0, 50.0, 80.0],
                ..Default::default()
            }],
            width: Some(360.0),
            height: Some(200.0),
            align: None,
            ..Default::default()
        }),
        Block::Chart(ChartBlock {
            chart_type: "pie".to_string(),
            title: None,
            categories: vec!["A".to_string(), "B".to_string()],
            series: vec![ChartSeries {
                name: "占比".to_string(),
                values: vec![60.0, 40.0],
                ..Default::default()
            }],
            width: None,
            height: None,
            align: None,
            ..Default::default()
        }),
    ])];

    let path = tmp("chart.docx");
    json2docx::generate(&doc, &path).expect("generate");

    let file = std::fs::File::open(&path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    assert!(names.contains(&"word/charts/chart1.xml".to_string()));
    assert!(names.contains(&"word/charts/chart2.xml".to_string()));
    let mut c1 = String::new();
    zip.by_name("word/charts/chart1.xml")
        .unwrap()
        .read_to_string(&mut c1)
        .unwrap();
    assert!(c1.contains("<c:barChart>"));
    assert!(c1.contains("季度销售"));

    let parsed = json2docx::parse(&path).expect("parse");
    let charts: Vec<&ChartBlock> = parsed
        .all_blocks()
        .filter_map(|b| match b {
            Block::Chart(c) => Some(c),
            _ => None,
        })
        .collect();
    assert_eq!(charts.len(), 2);
    assert_eq!(charts[0].chart_type, "bar");
    assert_eq!(charts[0].title.as_deref(), Some("季度销售"));
    assert_eq!(charts[0].categories, vec!["Q1", "Q2", "Q3"]);
    assert_eq!(charts[0].series[0].values, vec![30.0, 50.0, 80.0]);
    assert_eq!(charts[1].chart_type, "pie");
    assert_eq!(charts[1].series[0].values, vec![60.0, 40.0]);
}

#[test]
fn paragraph_extras_roundtrip() {
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![
        Block::Paragraph(Paragraph {
            text: Some("分隔线下方的段落".to_string()),
            keep_next: Some(true),
            keep_lines: Some(true),
            page_break_before: Some(true),
            shading: Some("FFF2CC".to_string()),
            border: Some("C00000".to_string()),
            tabs: Some(vec![
                json2docx::model::TabStop {
                    pos: 100.0,
                    ..Default::default()
                },
                json2docx::model::TabStop {
                    pos: 200.0,
                    ..Default::default()
                },
            ]),
            ..Default::default()
        }),
        Block::Paragraph(Paragraph {
            runs: Some(vec![Run {
                text: "底纹文字".to_string(),
                shading: Some("FFFF00".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }),
    ])];
    let path = tmp("pextra.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn footer_page_of_roundtrip() {
    use json2docx::model::Section;
    use std::io::Read;

    let mut doc = Document::default();
    doc.parts = vec![Part {
        section: Some(Section {
            footer_page_number: Some(true),
            footer_format: Some("page_of".to_string()),
            ..Default::default()
        }),
        blocks: vec![Block::Paragraph(Paragraph::text("x"))],
    }];
    let path = tmp("pageof.docx");
    json2docx::generate(&doc, &path).expect("generate");

    let file = std::fs::File::open(&path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut ftr = String::new();
    zip.by_name("word/footer1.xml")
        .unwrap()
        .read_to_string(&mut ftr)
        .unwrap();
    assert!(ftr.contains("NUMPAGES"));

    let parsed = json2docx::parse(&path).expect("parse");
    let sec = parsed.parts[0].section.as_ref().unwrap();
    assert_eq!(sec.footer_format.as_deref(), Some("page_of"));
    assert_eq!(sec.footer_page_number, Some(true));
}

#[test]
fn watermark_generation() {
    use std::io::Read;

    let mut doc = Document::default();
    doc.watermark = Some("机密".to_string());
    doc.parts = vec![Part::new(vec![Block::Paragraph(Paragraph::text("正文"))])];
    let path = tmp("wm.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let file = std::fs::File::open(&path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut hdr = String::new();
    zip.by_name("word/header1.xml")
        .unwrap()
        .read_to_string(&mut hdr)
        .unwrap();
    assert!(hdr.contains("PowerPlusWaterMarkObject"));
    assert!(hdr.contains("机密"));
}

#[test]
fn floating_image_roundtrip() {
    // 生成一张真实图片，验证浮动图（wp:anchor）回环
    let png = tmp("float.png");
    let img = image::RgbImage::from_pixel(4, 4, image::Rgb([200, 30, 30]));
    img.save(&png).expect("save png");

    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![Block::Image(ImageBlock {
        src: png.clone(),
        width: Some(120.0),
        height: Some(60.0),
        align: None,
        caption: None,
        alt: Some("浮动图".to_string()),
        wrap: Some("square".to_string()),
        x: Some(100.0),
        y: Some(80.0),
        behind_text: Some(false),
        crop: None,
        rotation: None,
    })])];

    let path = tmp("float.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    let block = parsed.all_blocks().next().unwrap();
    match block {
        Block::Image(im) => {
            assert_eq!(im.wrap.as_deref(), Some("square"));
            assert!((im.x.unwrap() - 100.0).abs() < 0.5);
            assert!((im.y.unwrap() - 80.0).abs() < 0.5);
            assert_eq!(im.behind_text, Some(false));
            assert!((im.width.unwrap() - 120.0).abs() < 0.5);
            assert_eq!(im.alt.as_deref(), Some("浮动图"));
        }
        other => panic!("expected image, got {:?}", other.type_name()),
    }
}

#[test]
fn block_serde_names() {
    // 每个块的 JSON `type` 标签必须与 type_name() 一致，且可反序列化
    let blocks = vec![
        Block::Heading {
            level: 1,
            text: "h".to_string(),
            numbering: None,
            align: None,
            runs: None,
        },
        Block::Paragraph(Paragraph::text("p")),
        Block::List(ListBlock {
            ordered: true,
            items: vec![ListItem {
                blocks: vec![Block::Paragraph(Paragraph::text("i"))],
                level: 0,
                ..Default::default()
            }],
        }),
        Block::Table(TableBlock {
            rows: vec![TableRow {
                cells: vec![TableCell {
                    blocks: vec![Block::Paragraph(Paragraph::text("c"))],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        Block::Image(ImageBlock {
            src: "x.png".to_string(),
            ..Default::default()
        }),
        Block::Formula(FormulaBlock {
            latex: "x".to_string(),
            display: true,
            number: None,
        }),
        Block::Code(CodeBlock {
            lang: None,
            text: "c".to_string(),
        }),
        Block::Quote(QuoteBlock {
            text: "q".to_string(),
        }),
        Block::Toc(TocBlock {
            title: "目录".to_string(),
            levels: Some(vec![1]),
            is_static: None,
            entries: Vec::new(),
        }),
        Block::Bibliography(BibliographyBlock {
            style: "GB/T 7714".to_string(),
            title: None,
            entries: vec![],
        }),
        Block::Caption(CaptionBlock {
            text: "cap".to_string(),
            of: None,
        }),
        Block::Chart(ChartBlock {
            chart_type: "column".to_string(),
            title: None,
            categories: vec!["A".to_string()],
            series: vec![ChartSeries {
                name: "s".to_string(),
                values: vec![1.0],
                ..Default::default()
            }],
            width: None,
            height: None,
            align: None,
            ..Default::default()
        }),
        Block::TextBox(TextBoxBlock {
            text: "tb".to_string(),
            ..Default::default()
        }),
        Block::Attachment(AttachmentBlock {
            src: "a.xlsx".to_string(),
            ..Default::default()
        }),
        Block::Shape(ShapeBlock {
            shape_type: "rightArrow".to_string(),
            width: Some(80.0),
            height: Some(40.0),
            ..Default::default()
        }),
        Block::PageBreak,
    ];
    for b in &blocks {
        let v = serde_json::to_value(b).expect("serialize");
        assert_eq!(
            v["type"].as_str(),
            Some(b.type_name()),
            "type 标签与 type_name 不一致: {}",
            b.type_name()
        );
        let back: Block = serde_json::from_value(v).expect("deserialize");
        assert_eq!(&back, b, "块回环不一致: {}", b.type_name());
    }
}

#[test]
fn comment_roundtrip() {
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![Block::Paragraph(Paragraph {
        runs: Some(vec![Run {
            text: "被批注的文字".to_string(),
            comment: Some("这里的表述需要更严谨".to_string()),
            comment_author: Some("审阅者".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    })])];
    let path = tmp("comment.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn revision_roundtrip() {
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![Block::Paragraph(Paragraph {
        runs: Some(vec![
            Run {
                text: "新增内容".to_string(),
                revision: Some("ins".to_string()),
                revision_author: Some("Alice".to_string()),
                ..Default::default()
            },
            Run {
                text: "删除内容".to_string(),
                revision: Some("del".to_string()),
                revision_author: Some("Alice".to_string()),
                ..Default::default()
            },
        ]),
        ..Default::default()
    })])];
    let path = tmp("revision.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn footnote_roundtrip() {
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![Block::Paragraph(Paragraph {
        runs: Some(vec![Run {
            text: "正文内容".to_string(),
            footnote: Some("这是一条脚注".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    })])];
    let path = tmp("footnote.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn endnote_roundtrip() {
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![Block::Paragraph(Paragraph {
        runs: Some(vec![Run {
            text: "正文".to_string(),
            endnote: Some("这是一条尾注".to_string()),
            ..Default::default()
        }]),
        ..Default::default()
    })])];
    let path = tmp("endnote.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn cross_ref_roundtrip() {
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![
        Block::Paragraph(Paragraph {
            runs: Some(vec![Run {
                text: "图 1".to_string(),
                bookmark: Some("fig1".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        Block::Paragraph(Paragraph {
            runs: Some(vec![
                Run::plain("见 "),
                Run {
                    text: "图 1".to_string(),
                    ref_target: Some("fig1".to_string()),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        }),
    ])];
    let path = tmp("xref.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(normalize(flatten(&doc)), normalize(flatten(&parsed)));
}

#[test]
fn special_chars_roundtrip() {
    let text = "A & B < C > D \"引号\" 'apos' — dash 😀 中文";
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![
        Block::Paragraph(Paragraph::text(text)),
        Block::Paragraph(Paragraph {
            runs: Some(vec![Run::plain("符号: <&>")]),
            ..Default::default()
        }),
    ])];
    let path = tmp("chars.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(parsed.all_blocks().next().unwrap().plain_text(), text);
}

#[test]
fn page_number_start() {
    use json2docx::model::Section;

    let mut doc = Document::default();
    doc.parts = vec![Part {
        section: Some(Section {
            page_number_start: Some(5),
            ..Default::default()
        }),
        blocks: vec![Block::Paragraph(Paragraph::text("x"))],
    }];
    let path = tmp("pgnum.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(
        parsed.parts[0].section.as_ref().unwrap().page_number_start,
        Some(5)
    );
}

#[test]
fn per_section_header_footer() {
    use json2docx::model::Section;
    use std::io::Read;

    let mut doc = Document::default();
    doc.even_and_odd = Some(true);
    doc.parts = vec![
        Part {
            section: Some(Section {
                header: Some("上半部分".to_string()),
                first_header: Some("封面页眉".to_string()),
                first_footer_page_number: Some(false),
                title_page: Some(true),
                ..Default::default()
            }),
            blocks: vec![Block::Paragraph(Paragraph::text("a"))],
        },
        Part {
            section: Some(Section {
                header: Some("下半部分".to_string()),
                footer_page_number: Some(true),
                even_header: Some("偶数页眉".to_string()),
                even_footer_page_number: Some(true),
                ..Default::default()
            }),
            blocks: vec![Block::Paragraph(Paragraph::text("b"))],
        },
    ];

    let path = tmp("hdrftr.docx");
    json2docx::generate(&doc, &path).expect("generate");

    let file = std::fs::File::open(&path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect();
    // 页眉：上半部分 / 封面页眉 / 下半部分 / 偶数页眉 = 4 个
    assert_eq!(
        names
            .iter()
            .filter(|n| n.starts_with("word/header"))
            .count(),
        4
    );
    // 页脚：default 与 even 共用同一页码部件 = 1 个文件
    assert_eq!(
        names
            .iter()
            .filter(|n| n.starts_with("word/footer"))
            .count(),
        1
    );

    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert_eq!(xml.matches("<w:footerReference").count(), 2);
    assert!(xml.contains("w:type=\"first\""));
    assert!(xml.contains("w:type=\"even\""));
    assert!(xml.contains("<w:titlePg/>"));

    let mut settings = String::new();
    zip.by_name("word/settings.xml")
        .unwrap()
        .read_to_string(&mut settings)
        .unwrap();
    assert!(settings.contains("<w:evenAndOddHeaders/>"));

    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(parsed.parts.len(), 2);
    let p0 = parsed.parts[0].section.as_ref().unwrap();
    assert_eq!(p0.header.as_deref(), Some("上半部分"));
    assert_eq!(p0.first_header.as_deref(), Some("封面页眉"));
    assert_eq!(p0.first_footer_page_number, Some(false));
    let p1 = parsed.parts[1].section.as_ref().unwrap();
    assert_eq!(p1.header.as_deref(), Some("下半部分"));
    assert_eq!(p1.footer_page_number, Some(true));
    assert_eq!(p1.even_header.as_deref(), Some("偶数页眉"));
    assert_eq!(p1.even_footer_page_number, Some(true));
}

#[test]
fn multi_section_columns() {
    use json2docx::model::style::PageSetup;
    use json2docx::model::Section;
    use std::io::Read;

    let mut doc = Document::default();
    doc.page = PageSetup {
        size: "A4".to_string(),
        orientation: "portrait".to_string(),
        ..Default::default()
    };
    // 第一节：单栏（摘要）
    let mut p1 = Part::default();
    p1.blocks
        .push(Block::Paragraph(Paragraph::text("Abstract")));
    p1.section = Some(Section {
        page: Some(PageSetup {
            columns: None,
            ..Default::default()
        }),
        ..Default::default()
    });
    // 第二节：双栏（正文）
    let mut p2 = Part::default();
    p2.blocks
        .push(Block::Paragraph(Paragraph::text("Body two columns")));
    p2.section = Some(Section {
        page: Some(PageSetup {
            columns: Some(2),
            column_space: Some(20.0),
            ..Default::default()
        }),
        ..Default::default()
    });
    doc.parts = vec![p1, p2];

    let path = tmp("cols.docx");
    json2docx::generate(&doc, &path).expect("generate");

    // 校验 XML 有 2 个 sectPr 且含 cols
    let file = std::fs::File::open(&path).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert_eq!(xml.matches("<w:sectPr>").count(), 2);
    assert!(xml.contains("<w:cols w:num=\"2\""));

    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(parsed.parts.len(), 2);
    assert_eq!(
        parsed.parts[1]
            .section
            .as_ref()
            .unwrap()
            .page
            .as_ref()
            .unwrap()
            .columns,
        Some(2)
    );
}

#[test]
fn template_applies_fonts() {
    let mut doc = sample_doc();
    doc.template = Some("academic-paper".to_string());
    let path = tmp("tpl.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    assert_eq!(parsed.theme.minor_font.as_deref(), Some("Times New Roman"));
    assert_eq!(parsed.theme.east_asia_font.as_deref(), Some("宋体"));
}

#[test]
fn repack_inline_parts() {
    // document.json 内联 parts（单文件起步），无 word/parts 文件
    let dir = tmp("inline");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let document = serde_json::json!({
        "meta": { "title": "内联测试" },
        "template": "academic-paper",
        "parts": [
            { "blocks": [
                { "type": "heading", "level": 1, "text": "第一章" },
                { "type": "paragraph", "text": "正文" }
            ] }
        ]
    });
    std::fs::write(
        std::path::Path::new(&dir).join("document.json"),
        serde_json::to_string_pretty(&document).unwrap(),
    )
    .unwrap();

    let out = tmp("inline_out.docx");
    json2docx::repack(&dir, &out).expect("repack");
    let parsed = json2docx::parse(&out).expect("parse");
    let texts: Vec<String> = parsed.all_blocks().map(|b| b.plain_text()).collect();
    assert!(texts.iter().any(|t| t == "第一章"));
    assert!(texts.iter().any(|t| t == "正文"));
}

#[test]
fn image_crop_rotation_and_anchor_link() {
    use std::io::Read;
    let png = tmp("crop.png");
    let img = image::RgbImage::from_pixel(8, 8, image::Rgb([10, 120, 200]));
    img.save(&png).expect("save png");

    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![
        Block::Image(ImageBlock {
            src: png.clone(),
            width: Some(100.0),
            height: Some(50.0),
            crop: Some([5.0, 10.0, 0.0, 0.0]),
            rotation: Some(15.0),
            ..Default::default()
        }),
        Block::Paragraph(Paragraph {
            runs: Some(vec![Run {
                text: "跳转".to_string(),
                hyperlink: Some("#sec1".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }),
    ])];

    let path = tmp("crop.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let f = std::fs::File::open(&path).unwrap();
    let mut z = zip::ZipArchive::new(f).unwrap();
    let mut xml = String::new();
    z.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert!(xml.contains("a:srcRect"), "应含裁剪: {xml}");
    assert!(xml.contains("rot=\"900000\""), "应含旋转: {xml}");
    assert!(
        xml.contains("<w:hyperlink w:anchor=\"sec1\""),
        "应含书签超链接: {xml}"
    );

    let parsed = json2docx::parse(&path).expect("parse");
    let im = parsed
        .all_blocks()
        .find_map(|b| match b {
            Block::Image(i) => Some(i),
            _ => None,
        })
        .expect("image");
    assert_eq!(im.crop, Some([5.0, 10.0, 0.0, 0.0]));
    assert_eq!(im.rotation, Some(15.0));
    let link = parsed.all_blocks().find_map(|b| match b {
        Block::Paragraph(p) => p
            .runs
            .as_ref()
            .and_then(|rs| rs.iter().find_map(|r| r.hyperlink.clone())),
        _ => None,
    });
    assert_eq!(link.as_deref(), Some("#sec1"));
}

#[test]
fn validate_generated_docx() {
    let doc = sample_doc();
    let path = tmp("validate.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let issues = json2docx::validate_docx(std::path::Path::new(&path));
    assert!(issues.is_empty(), "校验应无问题: {issues:?}");
}

#[test]
fn merge_placeholders() {
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![
        Block::Paragraph(Paragraph::text("你好，{{name}}！")),
        Block::Paragraph(Paragraph {
            runs: Some(vec![Run {
                text: "订单：{{order.id}}".to_string(),
                bold: Some(true),
                ..Default::default()
            }]),
            ..Default::default()
        }),
    ])];
    let data = serde_json::json!({ "name": "世界", "order": { "id": "A-100" } });
    let n = json2docx::merge_document(&mut doc, &data);
    assert_eq!(n, 2);
    let texts: Vec<String> = doc
        .all_blocks()
        .flat_map(|b| match b {
            Block::Paragraph(p) => {
                let mut v = Vec::new();
                if let Some(t) = &p.text {
                    v.push(t.clone());
                }
                if let Some(rs) = &p.runs {
                    for r in rs {
                        v.push(r.text.clone());
                    }
                }
                v
            }
            _ => vec![],
        })
        .collect();
    assert!(texts.iter().any(|t| t == "你好，世界！"), "{texts:?}");
    assert!(texts.iter().any(|t| t == "订单：A-100"), "{texts:?}");
}

#[test]
fn chart_rich_roundtrip() {
    let mut doc = Document::default();
    doc.parts = vec![Part::new(vec![Block::Chart(ChartBlock {
        chart_type: "column".to_string(),
        title: Some("营收".to_string()),
        categories: vec!["Q1".to_string(), "Q2".to_string()],
        series: vec![ChartSeries {
            name: "营收".to_string(),
            values: vec![10.0, 20.0],
            color: Some("ED7D31".to_string()),
        }],
        legend: Some("r".to_string()),
        x_title: Some("季度".to_string()),
        y_title: Some("万元".to_string()),
        y_min: Some(0.0),
        y_max: Some(50.0),
        data_labels: Some(true),
        ..Default::default()
    })])];
    let path = tmp("chart_rich.docx");
    json2docx::generate(&doc, &path).expect("generate");
    let parsed = json2docx::parse(&path).expect("parse");
    let c = parsed
        .all_blocks()
        .find_map(|b| match b {
            Block::Chart(c) => Some(c.clone()),
            _ => None,
        })
        .expect("chart");
    assert_eq!(c.legend.as_deref(), Some("r"));
    assert_eq!(c.x_title.as_deref(), Some("季度"));
    assert_eq!(c.y_title.as_deref(), Some("万元"));
    assert_eq!(c.y_max, Some(50.0));
    assert_eq!(c.series[0].color.as_deref(), Some("ED7D31"));
    assert_eq!(c.data_labels, Some(true));
}
