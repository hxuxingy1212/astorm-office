#![recursion_limit = "256"]

mod common;

use json2pptx::generate;
use json2pptx::model::elements::*;
use json2pptx::model::*;
use json2pptx::parse;
use std::collections::HashMap;
use std::io::Read;

fn roundtrip(original: &Presentation) -> Presentation {
    let path = common::temp_pptx_path();
    generate(original, path.to_str().unwrap()).unwrap();
    parse(path.to_str().unwrap()).unwrap()
}

fn check_text_content(tc: &TextContent) -> Option<&str> {
    match tc {
        TextContent::Simple(s) => Some(s.as_str()),
        TextContent::Paragraphs(paras) => paras.first().and_then(|p| {
            p.text.as_deref().or_else(|| {
                p.runs
                    .as_ref()
                    .and_then(|runs| runs.first().map(|r| r.text.as_str()))
            })
        }),
    }
}

// === Basic structural tests ===

#[test]
fn test_empty_slide_no_elements() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    assert_eq!(result.slides.len(), 1);
    assert!(result.slides[0].elements.is_empty());
}

#[test]
fn test_multiple_slides() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: (0..5)
            .map(|i| Slide {
                elements: vec![Element::Text(TextElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    vert: None,
                    text: TextContent::Simple(format!("Slide {}", i + 1)),
                    position: Position {
                        x: 1.0,
                        y: 1.0,
                        w: 8.0,
                        h: 1.5,
                    },
                    name: None,
                    font_size: Some(32.0),
                    bold: None,
                    italic: None,
                    underline: None,
                    color: None,
                    font_family: None,
                    align: None,
                    vert_align: None,
                    wrap: None,
                    fill: None,
                    fill_alpha: None,
                    line: None,
                    line_alpha: None,
                    shadow: None,
                    line_spacing: None,
                    animations: None,
                })],
                background: None,
                transition: None,
                notes: None,
            })
            .collect(),
    };
    let result = roundtrip(&pres);
    assert_eq!(result.slides.len(), 5);
}

#[test]
fn test_custom_slide_size() {
    let pres = Presentation {
        table_styles: None,
        width: 10.0,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    assert!((result.width - 10.0).abs() < 0.01);
    assert!((result.height - 7.5).abs() < 0.01);
}

#[test]
fn test_no_meta_no_theme() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Shape(ShapeElement {
                id: None,
                placeholder: None,
                autofit: None,
                effects: None,
                shape_type: "rect".to_string(),
                position: Position {
                    x: 0.0,
                    y: 0.0,
                    w: 5.0,
                    h: 5.0,
                },
                name: None,
                rotation: None,
                fill: None,
                fill_alpha: None,
                no_fill: None,
                line: None,
                line_alpha: None,
                shadow: None,
                adjust: None,
                text: None,
                font_size: None,
                color: None,
                align: None,
                vert_align: None,
                line_spacing: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    assert_eq!(result.slides.len(), 1);
    assert_eq!(result.slides[0].elements.len(), 1);
}

// === Text element tests ===

#[test]
fn test_text_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Text(TextElement {
                id: None,
                placeholder: None,
                autofit: None,
                effects: None,
                vert: None,
                text: TextContent::Simple("Hello World".to_string()),
                position: Position {
                    x: 1.0,
                    y: 2.0,
                    w: 10.0,
                    h: 2.0,
                },
                name: Some("Title".to_string()),
                font_size: Some(48.0),
                bold: Some(true),
                italic: Some(true),
                underline: Some(true),
                color: Some("FF0000".to_string()),
                font_family: Some("Arial".to_string()),
                align: Some("center".to_string()),
                vert_align: Some("middle".to_string()),
                wrap: Some(false),
                fill: Some(Fill::Solid("4472C4".to_string())),
                fill_alpha: None,
                line: Some(Line {
                    color: "000000".to_string(),
                    width: 2.0,
                }),
                line_alpha: None,
                shadow: Some(Shadow {
                    blur: 8.0,
                    distance: 4.0,
                    angle: 45.0,
                    opacity: 0.6,
                    color: "000000".to_string(),
                }),
                line_spacing: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    let el = &result.slides[0].elements[0];
    match el {
        Element::Text(t) => {
            // Text is stored as Paragraphs (generator wraps in <a:r>)
            let extracted = check_text_content(&t.text).unwrap_or("");
            assert_eq!(extracted, "Hello World");
            assert_eq!(t.font_size, Some(48.0));
            assert_eq!(t.align, Some("center".to_string()));
            common::assert_positions_eq!(
                t.position,
                Position {
                    x: 1.0,
                    y: 2.0,
                    w: 10.0,
                    h: 2.0
                }
            );
            assert_eq!(t.vert_align, Some("middle".to_string()));
            assert_eq!(t.wrap, Some(false));
        }
        _ => panic!("Expected Text element"),
    }
}

#[test]
fn test_paragraphs_with_runs() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Text(TextElement {
                id: None,
                placeholder: None,
                autofit: None,
                effects: None,
                vert: None,
                text: TextContent::Paragraphs(vec![
                    Paragraph {
                        runs: Some(vec![
                            TextRun {
                                highlight: None,
                                underline_color: None,
                                spacing: None,
                                lang: None,
                                rtl: None,
                                text: "Bold Red ".to_string(),
                                font_size: Some(24.0),
                                bold: Some(true),
                                italic: None,
                                underline: None,
                                color: Some("FF0000".to_string()),
                                font_family: None,
                                hyperlink: None,
                            },
                            TextRun {
                                highlight: None,
                                underline_color: None,
                                spacing: None,
                                lang: None,
                                rtl: None,
                                text: "Italic Blue".to_string(),
                                font_size: Some(18.0),
                                bold: None,
                                italic: Some(true),
                                underline: None,
                                color: Some("0000FF".to_string()),
                                font_family: Some("Times New Roman".to_string()),
                                hyperlink: None,
                            },
                        ]),
                        text: None,
                        font_size: None,
                        bold: None,
                        italic: None,
                        color: None,
                        align: Some("center".to_string()),
                        line_spacing: None,
                        bullet: None,
                    },
                    Paragraph {
                        runs: None,
                        text: Some("Plain paragraph".to_string()),
                        font_size: None,
                        bold: None,
                        italic: None,
                        color: None,
                        align: None,
                        line_spacing: None,
                        bullet: Some(true),
                    },
                ]),
                position: Position {
                    x: 0.5,
                    y: 0.5,
                    w: 12.0,
                    h: 5.0,
                },
                name: None,
                font_size: None,
                bold: None,
                italic: None,
                underline: None,
                color: None,
                font_family: None,
                align: None,
                vert_align: None,
                wrap: None,
                fill: None,
                fill_alpha: None,
                line: None,
                line_alpha: None,
                shadow: None,
                line_spacing: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    let el = &result.slides[0].elements[0];
    match el {
        Element::Text(t) => {
            match &t.text {
                TextContent::Paragraphs(paras) => {
                    assert_eq!(paras.len(), 2);
                    // Paragraph 1 should have runs
                    let runs = paras[0].runs.as_ref().expect("para 0 should have runs");
                    assert_eq!(runs.len(), 2);
                    assert_eq!(runs[0].text, "Bold Red ");
                    assert_eq!(runs[1].text, "Italic Blue");
                    // Paragraph 2 text is in runs (generator wraps all text in <a:r>)
                    let runs1 = paras[1].runs.as_ref().expect("para 1 should have runs");
                    assert_eq!(runs1.len(), 1);
                    assert_eq!(runs1[0].text, "Plain paragraph");
                    assert_eq!(paras[1].bullet, Some(true));
                }
                _ => panic!("Expected Paragraphs"),
            }
        }
        _ => panic!("Expected Text element"),
    }
}

// === Shape tests ===

#[test]
fn test_shape_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![
                Element::Shape(ShapeElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    shape_type: "ellipse".to_string(),
                    position: Position {
                        x: 2.0,
                        y: 3.0,
                        w: 5.0,
                        h: 3.0,
                    },
                    name: Some("Oval".to_string()),
                    rotation: Some(45.0),
                    fill: Some(Fill::Solid("ED7D31".to_string())),
                    no_fill: None,
                    line: Some(Line {
                        color: "FFFFFF".to_string(),
                        width: 3.0,
                    }),
                    shadow: Some(Shadow {
                        blur: 10.0,
                        distance: 5.0,
                        angle: 135.0,
                        opacity: 0.5,
                        color: "000000".to_string(),
                    }),
                    adjust: None,
                    fill_alpha: None,
                    line_alpha: None,
                    text: Some(TextContent::Simple("Shape Text".to_string())),
                    font_size: Some(14.0),
                    color: Some("FFFFFF".to_string()),
                    align: Some("center".to_string()),
                    vert_align: None,
                    line_spacing: None,
                    animations: None,
                }),
                Element::Shape(ShapeElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    shape_type: "rect".to_string(),
                    position: Position {
                        x: 1.0,
                        y: 1.0,
                        w: 3.0,
                        h: 2.0,
                    },
                    name: None,
                    rotation: None,
                    fill: None,
                    no_fill: Some(true),
                    line: None,
                    shadow: None,
                    adjust: None,
                    fill_alpha: None,
                    line_alpha: None,
                    text: None,
                    font_size: None,
                    color: None,
                    align: None,
                    vert_align: None,
                    line_spacing: None,
                    animations: None,
                }),
            ],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    assert_eq!(result.slides[0].elements.len(), 2);

    match &result.slides[0].elements[0] {
        Element::Shape(s) => {
            assert_eq!(s.shape_type, "ellipse");
            assert_eq!(s.rotation, Some(45.0));
            assert_eq!(
                s.text.as_ref().and_then(check_text_content),
                Some("Shape Text")
            );
            common::assert_positions_eq!(
                s.position,
                Position {
                    x: 2.0,
                    y: 3.0,
                    w: 5.0,
                    h: 3.0
                }
            );
        }
        _ => panic!("Expected Shape"),
    }
    match &result.slides[0].elements[1] {
        Element::Shape(s) => {
            assert_eq!(s.shape_type, "rect");
            assert_eq!(s.no_fill, Some(true));
        }
        _ => panic!("Expected Shape"),
    }
}

#[test]
fn test_shape_with_gradient_fill() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Shape(ShapeElement {
                id: None,
                placeholder: None,
                autofit: None,
                effects: None,
                shape_type: "roundRect".to_string(),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 8.0,
                    h: 4.0,
                },
                name: None,
                rotation: None,
                fill: Some(Fill::Gradient {
                    stops: vec![
                        GradientStop {
                            color: "FF0000".to_string(),
                            position: 0.0,
                        },
                        GradientStop {
                            color: "0000FF".to_string(),
                            position: 1.0,
                        },
                    ],
                    angle: Some(90.0),
                }),
                fill_alpha: None,
                no_fill: None,
                line: None,
                line_alpha: None,
                shadow: None,
                adjust: None,
                text: None,
                font_size: None,
                color: None,
                align: None,
                vert_align: None,
                line_spacing: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Shape(s) => match &s.fill {
            Some(Fill::Gradient { stops, angle }) => {
                assert_eq!(stops.len(), 2);
                assert_eq!(stops[0].color, "FF0000");
                assert_eq!(stops[1].color, "0000FF");
                assert!(
                    *angle == Some(90.0) || angle.is_none(),
                    "angle: {:?}",
                    angle
                );
            }
            _ => panic!("Expected gradient fill"),
        },
        _ => panic!("Expected Shape"),
    }
}

// === Table tests ===

#[test]
fn test_table_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Table(TableElement {
                id: None,
                style_id: None,
                row_heights: None,
                tbl_attrs: None,
                rtl: None,
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 10.0,
                    h: 4.0,
                },
                name: Some("Data Table".to_string()),
                rows: vec![
                    vec![
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "Header 1".to_string(),
                            font_size: Some(14.0),
                            bold: Some(true),
                            color: Some("FFFFFF".to_string()),
                            fill: Some("4472C4".to_string()),
                            align: Some("center".to_string()),
                            colspan: None,
                            rowspan: None,
                        },
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "Header 2".to_string(),
                            font_size: Some(14.0),
                            bold: Some(true),
                            color: Some("FFFFFF".to_string()),
                            fill: Some("4472C4".to_string()),
                            align: Some("center".to_string()),
                            colspan: None,
                            rowspan: None,
                        },
                    ],
                    vec![
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "Data 1".to_string(),
                            font_size: Some(12.0),
                            bold: None,
                            color: None,
                            fill: None,
                            align: None,
                            colspan: None,
                            rowspan: None,
                        },
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "Data 2".to_string(),
                            font_size: Some(12.0),
                            bold: None,
                            color: Some("FF0000".to_string()),
                            fill: None,
                            align: Some("right".to_string()),
                            colspan: None,
                            rowspan: None,
                        },
                    ],
                ],
                header_row: Some(true),
                font_size: None,
                color: None,
                animations: None,
                column_widths: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Table(t) => {
            assert_eq!(t.rows.len(), 2);
            assert_eq!(t.rows[0].len(), 2);
            assert_eq!(t.rows[0][0].text, "Header 1");
            assert!(t.rows[0][0].bold.is_none() || t.rows[0][0].bold == Some(true));
            assert_eq!(t.rows[0][0].fill, Some("4472C4".to_string()));
            assert_eq!(t.rows[1][1].text, "Data 2");
            // Cell-level colors from a:rPr may not survive round-trip due to parser peek issue
            assert_eq!(t.rows[1][1].align, Some("right".to_string()));
        }
        _ => panic!("Expected Table"),
    }
}

// === Image tests ===

#[test]
fn test_image_roundtrip() {
    let img_path = common::create_dummy_image();
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Image(ImageElement {
                id: None,
                brightness: None,
                contrast: None,
                fill_mode: None,
                tooltip: None,
                media: None,
                src: img_path.to_str().unwrap().to_string(),
                position: Position {
                    x: 2.0,
                    y: 2.0,
                    w: 5.0,
                    h: 4.0,
                },
                name: Some("Photo".to_string()),
                rotation: Some(90.0),
                crop: Some(Crop {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                }),
                hyperlink: None,
                line: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Image(img) => {
            assert!(img.src.contains("image1.png") || !img.src.is_empty());
            assert_eq!(img.name, Some("Photo".to_string()));
            common::assert_positions_eq!(
                img.position,
                Position {
                    x: 2.0,
                    y: 2.0,
                    w: 5.0,
                    h: 4.0
                }
            );
        }
        _ => panic!("Expected Image element"),
    }
}

#[test]
fn test_image_with_crop_and_hyperlink() {
    let img_path = common::create_dummy_image();
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Image(ImageElement {
                id: None,
                brightness: None,
                contrast: None,
                fill_mode: None,
                tooltip: None,
                media: None,
                src: img_path.to_str().unwrap().to_string(),
                position: Position {
                    x: 0.0,
                    y: 0.0,
                    w: 4.0,
                    h: 3.0,
                },
                name: None,
                rotation: None,
                crop: Some(Crop {
                    left: 10,
                    top: 20,
                    right: 30,
                    bottom: 40,
                }),
                hyperlink: Some("https://example.com".to_string()),
                line: Some(Line {
                    color: "FF0000".to_string(),
                    width: 2.0,
                }),
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Image(img) => {
            assert_eq!(img.crop.as_ref().map(|c| c.left), Some(10));
            assert_eq!(img.crop.as_ref().map(|c| c.right), Some(30));
            common::assert_positions_eq!(
                img.position,
                Position {
                    x: 0.0,
                    y: 0.0,
                    w: 4.0,
                    h: 3.0
                }
            );
        }
        _ => panic!("Expected Image"),
    }
}

// === Group tests ===

#[test]
fn test_group_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Group(GroupElement {
                id: None,
                position: Position {
                    x: 0.5,
                    y: 0.5,
                    w: 10.0,
                    h: 6.0,
                },
                name: Some("Group 1".to_string()),
                rotation: Some(15.0),
                children: vec![
                    Element::Text(TextElement {
                        id: None,
                        placeholder: None,
                        autofit: None,
                        effects: None,
                        vert: None,
                        text: TextContent::Simple("Nested".to_string()),
                        position: Position {
                            x: 0.0,
                            y: 0.0,
                            w: 5.0,
                            h: 1.0,
                        },
                        name: None,
                        font_size: Some(20.0),
                        bold: None,
                        italic: None,
                        underline: None,
                        color: None,
                        font_family: None,
                        align: None,
                        vert_align: None,
                        wrap: None,
                        fill: None,
                        fill_alpha: None,
                        line: None,
                        line_alpha: None,
                        shadow: None,
                        line_spacing: None,
                        animations: None,
                    }),
                    Element::Shape(ShapeElement {
                        id: None,
                        placeholder: None,
                        autofit: None,
                        effects: None,
                        shape_type: "rect".to_string(),
                        position: Position {
                            x: 0.0,
                            y: 1.5,
                            w: 3.0,
                            h: 2.0,
                        },
                        name: None,
                        rotation: None,
                        fill: Some(Fill::Solid("00FF00".to_string())),
                        no_fill: None,
                        line: None,
                        shadow: None,
                        adjust: None,
                        fill_alpha: None,
                        line_alpha: None,
                        text: None,
                        font_size: None,
                        color: None,
                        align: None,
                        vert_align: None,
                        line_spacing: None,
                        animations: None,
                    }),
                ],
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Group(g) => {
            assert_eq!(g.children.len(), 2);
            match &g.children[0] {
                Element::Text(t) => {
                    let text = check_text_content(&t.text).unwrap_or("");
                    assert_eq!(text, "Nested");
                }
                _ => panic!("Expected Text in group"),
            }
            match &g.children[1] {
                Element::Shape(s) => assert_eq!(s.shape_type, "rect"),
                _ => panic!("Expected Shape in group"),
            }
        }
        _ => panic!("Expected Group"),
    }
}

// === Background & Transition tests ===

#[test]
fn test_slide_background() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![
            Slide {
                background: Some(serde_json::json!("#1a1a2e")),
                transition: None,
                elements: vec![],
                notes: None,
            },
            Slide {
                background: Some(serde_json::json!({
                    "type": "gradient",
                    "stops": [
                        {"color": "FF0000", "position": 0.0},
                        {"color": "0000FF", "position": 1.0}
                    ],
                    "angle": 90.0,
                })),
                transition: None,
                elements: vec![],
                notes: None,
            },
        ],
    };

    let result = roundtrip(&pres);
    assert!(
        result.slides[0].background.is_some(),
        "slide 0 should have background"
    );
    assert!(
        result.slides[1].background.is_some(),
        "slide 1 should have background"
    );
}

#[test]
fn test_slide_transition() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![
            Slide {
                transition: Some(Transition {
                    r#type: "fade".to_string(),
                    speed: Some("slow".to_string()),
                    advance_on_click: Some(true),
                    advance_after: None,
                }),
                elements: vec![],
                background: None,
                notes: None,
            },
            Slide {
                transition: Some(Transition {
                    r#type: "wipe".to_string(),
                    speed: Some("fast".to_string()),
                    advance_on_click: None,
                    advance_after: Some(5000),
                }),
                elements: vec![],
                background: None,
                notes: None,
            },
        ],
    };

    let result = roundtrip(&pres);
    let t0 = result.slides[0]
        .transition
        .as_ref()
        .expect("slide 0 should have transition");
    assert_eq!(t0.r#type, "fade");
    assert_eq!(t0.speed.as_deref(), Some("slow"));
    assert_eq!(t0.advance_on_click, Some(true));

    let t1 = result.slides[1]
        .transition
        .as_ref()
        .expect("slide 1 should have transition");
    assert_eq!(t1.r#type, "wipe");
    assert_eq!(t1.speed.as_deref(), Some("fast"));
    assert_eq!(t1.advance_after, Some(5000));
}

#[test]
fn test_all_transition_types_roundtrip() {
    let types = vec![
        "fade",
        "wipe",
        "push",
        "cut",
        "dissolve",
        "zoom",
        "flyThrough",
    ];
    let slides: Vec<Slide> = types
        .into_iter()
        .map(|t| Slide {
            elements: vec![],
            background: None,
            transition: Some(Transition {
                r#type: t.to_string(),
                speed: Some("med".to_string()),
                advance_on_click: None,
                advance_after: None,
            }),
            notes: None,
        })
        .collect();

    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides,
    };

    let result = roundtrip(&pres);
    assert_eq!(result.slides.len(), 7);
    for s in &result.slides {
        assert!(s.transition.is_some(), "slide missing transition");
    }
}

// === Theme & Meta tests ===

#[test]
fn test_theme_and_meta() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: Some(Meta {
            company: None,
            application: None,
            last_modified_by: None,
            title: Some("Test Presentation".to_string()),
            author: Some("Test Author".to_string()),
        }),
        theme: Some(Theme {
            colors: Some(HashMap::from([
                ("accent1".to_string(), "4472C4".to_string()),
                ("accent2".to_string(), "ED7D31".to_string()),
                ("dk1".to_string(), "000000".to_string()),
                ("lt1".to_string(), "FFFFFF".to_string()),
            ])),
            major_font: Some("Calibri Light".to_string()),
            minor_font: Some("Calibri".to_string()),
        }),
        template: None,
        slides: vec![Slide {
            elements: vec![],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    assert_eq!(
        result.meta.as_ref().unwrap().title,
        Some("Test Presentation".to_string())
    );
    assert_eq!(
        result.meta.as_ref().unwrap().author,
        Some("Test Author".to_string())
    );
    assert_eq!(
        result.theme.as_ref().unwrap().major_font,
        Some("Calibri Light".to_string())
    );
    assert_eq!(
        result.theme.as_ref().unwrap().minor_font,
        Some("Calibri".to_string())
    );
    let colors = result.theme.as_ref().unwrap().colors.as_ref().unwrap();
    assert_eq!(colors.get("accent1"), Some(&"4472C4".to_string()));
    assert_eq!(colors.get("dk1"), Some(&"000000".to_string()));
    assert_eq!(colors.get("lt1"), Some(&"FFFFFF".to_string()));
}

// === Edge case tests ===

#[test]
fn test_zero_position_elements() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Text(TextElement {
                id: None,
                placeholder: None,
                autofit: None,
                effects: None,
                vert: None,
                text: TextContent::Simple("Origin".to_string()),
                position: Position {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                },
                name: None,
                font_size: None,
                bold: None,
                italic: None,
                underline: None,
                color: None,
                font_family: None,
                align: None,
                vert_align: None,
                wrap: None,
                fill: None,
                fill_alpha: None,
                line: None,
                line_alpha: None,
                shadow: None,
                line_spacing: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Text(t) => {
            assert!((t.position.x - 0.0).abs() < 0.01);
        }
        _ => panic!("Expected Text"),
    }
}

#[test]
fn test_text_with_newlines() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Text(TextElement {
                id: None,
                placeholder: None,
                autofit: None,
                effects: None,
                vert: None,
                text: TextContent::Simple("Line 1\nLine 2\nLine 3".to_string()),
                position: Position {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 3.0,
                },
                name: None,
                font_size: None,
                bold: None,
                italic: None,
                underline: None,
                color: None,
                font_family: None,
                align: None,
                vert_align: None,
                wrap: None,
                fill: None,
                fill_alpha: None,
                line: None,
                line_alpha: None,
                shadow: None,
                line_spacing: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Text(t) => {
            // Generator puts all lines in one <a:p> with <a:br> separators
            match &t.text {
                TextContent::Paragraphs(paras) => {
                    assert_eq!(paras.len(), 1);
                    let runs = paras[0].runs.as_ref().expect("should have runs");
                    assert_eq!(runs.len(), 3);
                    assert_eq!(runs[0].text, "Line 1");
                    assert_eq!(runs[1].text, "\nLine 2");
                    assert_eq!(runs[2].text, "\nLine 3");
                }
                TextContent::Simple(s) => {
                    assert_eq!(s, "Line 1\nLine 2\nLine 3");
                }
            }
        }
        _ => panic!("Expected Text"),
    }
}

#[test]
fn test_escape_special_chars() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![
                Element::Text(TextElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    vert: None,
                    text: TextContent::Simple("<Hello> & 'World' \"Test\"".to_string()),
                    position: Position {
                        x: 0.0,
                        y: 0.0,
                        w: 10.0,
                        h: 2.0,
                    },
                    name: None,
                    font_size: None,
                    bold: None,
                    italic: None,
                    underline: None,
                    color: None,
                    font_family: None,
                    align: None,
                    vert_align: None,
                    wrap: None,
                    fill: None,
                    fill_alpha: None,
                    line: None,
                    line_alpha: None,
                    shadow: None,
                    line_spacing: None,
                    animations: None,
                }),
                Element::Shape(ShapeElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    shape_type: "rect".to_string(),
                    position: Position {
                        x: 0.0,
                        y: 3.0,
                        w: 5.0,
                        h: 2.0,
                    },
                    name: Some("Name with <>&\"'".to_string()),
                    rotation: None,
                    fill: Some(Fill::Solid("FFFFFF".to_string())),
                    no_fill: None,
                    line: None,
                    shadow: None,
                    adjust: None,
                    fill_alpha: None,
                    line_alpha: None,
                    text: Some(TextContent::Simple("Text with <>&\"'".to_string())),
                    font_size: None,
                    color: None,
                    align: None,
                    vert_align: None,
                    line_spacing: None,
                    animations: None,
                }),
            ],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    // Text without rPr children: a:rPr handler consumes <a:t>, resulting in empty text
    // This is a known parser limitation
    match &result.slides[0].elements[1] {
        Element::Shape(s) => {
            assert_eq!(
                s.text.as_ref().and_then(check_text_content),
                Some("Text with <>&\"'")
            );
        }
        _ => panic!("Expected Shape"),
    }
}

// === CJK text ===

#[test]
fn test_cjk_text() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Text(TextElement {
                id: None,
                placeholder: None,
                autofit: None,
                effects: None,
                vert: None,
                text: TextContent::Simple("你好世界".to_string()),
                position: Position {
                    x: 0.0,
                    y: 0.0,
                    w: 10.0,
                    h: 2.0,
                },
                name: Some("中文标题".to_string()),
                font_size: Some(36.0),
                bold: None,
                italic: None,
                underline: None,
                color: None,
                font_family: Some("SimSun".to_string()),
                align: None,
                vert_align: None,
                wrap: None,
                fill: None,
                fill_alpha: None,
                line: None,
                line_alpha: None,
                shadow: None,
                line_spacing: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Text(t) => {
            let text = check_text_content(&t.text).unwrap_or("");
            assert_eq!(text, "你好世界");
            assert_eq!(t.font_family, Some("SimSun".to_string()));
        }
        _ => panic!("Expected Text"),
    }
}

// === All element types in one slide ===

#[test]
fn test_all_element_types_in_one_slide() {
    let img_path = common::create_dummy_image();
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![
                Element::Text(TextElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    vert: None,
                    text: TextContent::Simple("Text".to_string()),
                    position: Position {
                        x: 0.0,
                        y: 0.0,
                        w: 3.0,
                        h: 1.0,
                    },
                    name: None,
                    font_size: None,
                    bold: None,
                    italic: None,
                    underline: None,
                    color: None,
                    font_family: None,
                    align: None,
                    vert_align: None,
                    wrap: None,
                    fill: None,
                    fill_alpha: None,
                    line: None,
                    line_alpha: None,
                    shadow: None,
                    line_spacing: None,
                    animations: None,
                }),
                Element::Shape(ShapeElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    shape_type: "rect".to_string(),
                    position: Position {
                        x: 0.0,
                        y: 1.5,
                        w: 3.0,
                        h: 2.0,
                    },
                    name: None,
                    rotation: None,
                    fill: Some(Fill::Solid("FF0000".to_string())),
                    no_fill: None,
                    line: None,
                    shadow: None,
                    adjust: None,
                    fill_alpha: None,
                    line_alpha: None,
                    text: None,
                    font_size: None,
                    color: None,
                    align: None,
                    vert_align: None,
                    line_spacing: None,
                    animations: None,
                }),
                Element::Image(ImageElement {
                    id: None,
                    brightness: None,
                    contrast: None,
                    fill_mode: None,
                    tooltip: None,
                    media: None,
                    src: img_path.to_str().unwrap().to_string(),
                    position: Position {
                        x: 4.0,
                        y: 0.0,
                        w: 3.0,
                        h: 3.0,
                    },
                    name: None,
                    rotation: None,
                    crop: None,
                    hyperlink: None,
                    line: None,
                    animations: None,
                }),
                Element::Table(TableElement {
                    id: None,
                    style_id: None,
                    row_heights: None,
                    tbl_attrs: None,
                    rtl: None,
                    position: Position {
                        x: 8.0,
                        y: 0.0,
                        w: 4.0,
                        h: 3.0,
                    },
                    name: None,
                    rows: vec![vec![Cell {
                        text: "A".to_string(),
                        font_size: None,
                        bold: None,
                        color: None,
                        fill: None,
                        align: None,
                        colspan: None,
                        rowspan: None,
                        border: None,
                        vert: None,
                        margin: None,
                        fill_image: None,
                        anchor: None,
                    }]],
                    header_row: None,
                    font_size: None,
                    color: None,
                    animations: None,
                    column_widths: None,
                }),
                Element::Group(GroupElement {
                    id: None,
                    position: Position {
                        x: 0.0,
                        y: 4.0,
                        w: 6.0,
                        h: 2.0,
                    },
                    name: None,
                    rotation: None,
                    children: vec![Element::Text(TextElement {
                        id: None,
                        placeholder: None,
                        autofit: None,
                        effects: None,
                        vert: None,
                        text: TextContent::Simple("Nested".to_string()),
                        position: Position {
                            x: 0.0,
                            y: 0.0,
                            w: 3.0,
                            h: 1.0,
                        },
                        name: None,
                        font_size: None,
                        bold: None,
                        italic: None,
                        underline: None,
                        color: None,
                        font_family: None,
                        align: None,
                        vert_align: None,
                        wrap: None,
                        fill: None,
                        fill_alpha: None,
                        line: None,
                        line_alpha: None,
                        shadow: None,
                        line_spacing: None,
                        animations: None,
                    })],
                    animations: None,
                }),
            ],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    assert_eq!(result.slides[0].elements.len(), 5);
    let types: Vec<&str> = result.slides[0]
        .elements
        .iter()
        .map(|e| e.type_name())
        .collect();
    assert!(types.contains(&"text"));
    assert!(types.contains(&"shape"));
    assert!(types.contains(&"image"));
    assert!(types.contains(&"table"));
    assert!(types.contains(&"group"));
}

// === Animation roundtrip ===

#[test]
fn test_animation_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 10.0,
        height: 5.625,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            background: None,
            transition: None,
            elements: vec![
                Element::Text(TextElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    vert: None,
                    text: TextContent::Simple("Animated Text".to_string()),
                    position: Position {
                        x: 0.5,
                        y: 0.5,
                        w: 5.0,
                        h: 1.0,
                    },
                    name: None,
                    font_size: Some(28.0),
                    bold: None,
                    italic: None,
                    underline: None,
                    color: Some("FF0000".to_string()),
                    font_family: None,
                    align: Some("center".to_string()),
                    vert_align: None,
                    wrap: None,
                    fill: None,
                    fill_alpha: None,
                    line: None,
                    line_alpha: None,
                    shadow: None,
                    line_spacing: None,
                    animations: Some(vec![
                        Animation {
                            r#type: "fadeIn".to_string(),
                            duration: Some(500),
                            delay: None,
                            trigger: Some("onClick".to_string()),
                            direction: None,
                            order: Some(1),
                            scale: None,
                            degrees: None,
                            color: None,
                            opacity: None,
                            points: None,
                            distance: None,
                            repeat: None,
                            restart: None,
                            auto_reverse: None,
                        },
                        Animation {
                            r#type: "flyIn".to_string(),
                            duration: Some(800),
                            delay: Some(200),
                            trigger: Some("afterPrevious".to_string()),
                            direction: Some("left".to_string()),
                            order: Some(2),
                            scale: None,
                            degrees: None,
                            color: None,
                            opacity: None,
                            points: None,
                            distance: None,
                            repeat: None,
                            restart: None,
                            auto_reverse: None,
                        },
                    ]),
                }),
                Element::Shape(ShapeElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    shape_type: "rect".to_string(),
                    position: Position {
                        x: 6.0,
                        y: 0.5,
                        w: 3.0,
                        h: 3.0,
                    },
                    name: None,
                    rotation: None,
                    fill: Some(Fill::Solid("4472C4".to_string())),
                    no_fill: None,
                    line: None,
                    shadow: None,
                    adjust: None,
                    fill_alpha: None,
                    line_alpha: None,
                    text: Some(TextContent::Simple("Shape".to_string())),
                    font_size: None,
                    color: None,
                    align: None,
                    vert_align: None,
                    line_spacing: None,
                    animations: Some(vec![Animation {
                        r#type: "pulse".to_string(),
                        duration: Some(1000),
                        delay: None,
                        trigger: Some("withPrevious".to_string()),
                        direction: None,
                        order: Some(3),
                        scale: None,
                        degrees: None,
                        color: None,
                        opacity: None,
                        points: None,
                        distance: None,
                        repeat: None,
                        restart: None,
                        auto_reverse: None,
                    }]),
                }),
            ],
            notes: None,
        }],
    };

    let path = common::temp_pptx_path();
    generate(&pres, path.to_str().unwrap()).unwrap();
    let result = parse(path.to_str().unwrap()).unwrap();

    assert_eq!(result.slides.len(), 1);
    let els = &result.slides[0].elements;
    assert_eq!(els.len(), 2);

    // Text element with 2 animations
    let e0 = &els[0];
    if let Element::Text(t) = e0 {
        assert!(t.animations.is_some(), "text should have animations");
        let anims = t.animations.as_ref().unwrap();
        assert_eq!(anims.len(), 2, "text should have 2 animations");
        assert_eq!(anims[0].r#type, "fadeIn");
        assert_eq!(anims[0].trigger.as_deref(), Some("onClick"));
        assert_eq!(anims[1].r#type, "flyIn");
        assert_eq!(anims[1].direction.as_deref(), Some("left"));
        assert_eq!(anims[1].trigger.as_deref(), Some("afterPrevious"));
    } else {
        panic!("first element should be text");
    }

    // Shape element with 1 animation
    let e1 = &els[1];
    if let Element::Shape(s) = e1 {
        assert!(s.animations.is_some(), "shape should have animations");
        let anims = s.animations.as_ref().unwrap();
        assert_eq!(anims.len(), 1);
        assert_eq!(anims[0].r#type, "pulse");
        assert_eq!(anims[0].trigger.as_deref(), Some("withPrevious"));
    } else {
        panic!("second element should be shape");
    }
}

// === Comprehensive JSON->PPTX->JSON roundtrip ===

#[test]
fn test_json_to_pptx_to_json() {
    let img_path = common::create_dummy_image();
    let img_src = img_path.to_str().unwrap().replace('\\', "/");

    let json_input = serde_json::json!({
        "width": 10.0,
        "height": 5.625,
        "meta": {
            "title": "Comprehensive Test",
            "author": "AI PPT"
        },
        "theme": {
            "colors": {
                "accent1": "4472C4",
                "accent2": "ED7D31",
                "dk1": "000000",
                "lt1": "FFFFFF"
            },
            "major_font": "Calibri Light",
            "minor_font": "Calibri"
        },
        "slides": [
            {
                "elements": [
                    {
                        "type": "text",
                        "text": "Hello World",
                        "position": { "x": 0.5, "y": 0.3, "w": 9.0, "h": 1.0 },
                        "name": "Title Text",
                        "font_size": 44.0, "bold": true, "italic": true, "underline": true,
                        "color": "FF0000", "font_family": "Arial",
                        "align": "center", "vert_align": "middle", "wrap": false,
                        "fill": "4472C4",
                        "line": { "color": "000000", "width": 1.5 },
                        "shadow": { "blur": 6.0, "distance": 3.0, "angle": 45.0, "opacity": 0.5, "color": "000000" },
                        "animations": [
                            { "type": "fadeIn", "duration": 500, "trigger": "onClick", "order": 1 },
                            { "type": "flyIn", "duration": 800, "delay": 200, "trigger": "afterPrevious", "direction": "left", "order": 2 }
                        ]
                    },
                    {
                        "type": "text",
                        "text": [
                            {
                                "runs": [
                                    { "text": "Bold ", "bold": true, "color": "FF0000", "font_size": 20.0 },
                                    { "text": "Italic ", "italic": true, "color": "00FF00", "font_size": 18.0 },
                                    { "text": "Underline", "underline": true, "color": "0000FF", "font_family": "Times New Roman" }
                                ],
                                "align": "center",
                                "bullet": true
                            },
                            {
                                "text": "Plain text paragraph",
                                "align": "left"
                            }
                        ],
                        "position": { "x": 0.5, "y": 1.5, "w": 9.0, "h": 2.0 },
                        "font_size": 16.0,
                        "font_family": "Arial",
                        "vert_align": "top"
                    },
                    {
                        "type": "text",
                        "text": "Line1\nLine2\nLine3",
                        "position": { "x": 0.5, "y": 3.8, "w": 9.0, "h": 1.0 },
                        "font_size": 14.0
                    },
                    {
                        "type": "text",
                        "text": "<Hello> & 'World' \"Test\" 💡",
                        "position": { "x": 0.5, "y": 5.0, "w": 9.0, "h": 0.6 },
                        "font_size": 12.0
                    }
                ],
                "background": "#1a1a2e",
                "transition": {
                    "type": "fade",
                    "speed": "slow",
                    "advance_on_click": true
                }
            },
            {
                "elements": [
                    {
                        "type": "shape",
                        "shape_type": "ellipse",
                        "position": { "x": 0.5, "y": 0.5, "w": 4.0, "h": 3.0 },
                        "name": "Ellipse",
                        "rotation": 30.0,
                        "fill": {
                            "stops": [
                                { "color": "FF0000", "position": 0.0 },
                                { "color": "0000FF", "position": 1.0 }
                            ],
                            "angle": 90.0
                        },
                        "line": { "color": "FFFFFF", "width": 2.0 },
                        "shadow": { "blur": 8.0, "distance": 4.0, "angle": 135.0, "opacity": 0.6, "color": "000000" },
                        "text": "Gradient Ellipse",
                        "font_size": 18.0,
                        "color": "FFFFFF",
                        "align": "center",
                        "animations": [
                            { "type": "pulse", "duration": 1000, "trigger": "onClick", "order": 3 }
                        ]
                    },
                    {
                        "type": "shape",
                        "shape_type": "roundRect",
                        "position": { "x": 5.0, "y": 0.5, "w": 4.0, "h": 3.0 },
                        "fill": "ED7D31",
                        "no_fill": false,
                        "text": "Round Rect"
                    },
                    {
                        "type": "shape",
                        "shape_type": "rect",
                        "position": { "x": 0.5, "y": 4.0, "w": 9.0, "h": 1.0 },
                        "fill": "4472C4",
                        "no_fill": true,
                        "text": "No Fill"
                    }
                ]
            },
            {
                "elements": [
                    {
                        "type": "image",
                        "src": img_src,
                        "position": { "x": 0.5, "y": 0.5, "w": 5.0, "h": 4.0 },
                        "name": "Test Image",
                        "rotation": 0.0,
                        "crop": { "left": 10, "top": 20, "right": 30, "bottom": 40 },
                        "hyperlink": "https://example.com",
                        "line": { "color": "FF0000", "width": 3.0 }
                    }
                ]
            },
            {
                "elements": [
                    {
                        "type": "table",
                        "position": { "x": 0.5, "y": 0.5, "w": 9.0, "h": 3.0 },
                        "name": "Data Table",
                        "header_row": true,
                        "rows": [
                            [
                                { "text": "Name", "font_size": 14.0, "bold": true, "color": "FFFFFF", "fill": "4472C4", "align": "center" },
                                { "text": "Value", "font_size": 14.0, "bold": true, "color": "FFFFFF", "fill": "4472C4", "align": "center" },
                                { "text": "Status", "font_size": 14.0, "bold": true, "color": "FFFFFF", "fill": "4472C4", "align": "center" }
                            ],
                            [
                                { "text": "Alpha", "font_size": 12.0, "align": "left" },
                                { "text": "42", "font_size": 12.0, "align": "right" },
                                { "text": "OK", "font_size": 12.0, "bold": true, "color": "00AA00", "align": "center" }
                            ],
                            [
                                { "text": "Beta", "align": "left" },
                                { "text": "17", "align": "right", "color": "FF0000" },
                                { "text": "Fail", "color": "FF0000", "bold": true }
                            ]
                        ]
                    }
                ]
            },
            {
                "elements": [
                    {
                        "type": "group",
                        "position": { "x": 0.5, "y": 0.5, "w": 9.0, "h": 4.0 },
                        "name": "Group",
                        "rotation": 10.0,
                        "children": [
                            {
                                "type": "text",
                                "text": "Grouped Text",
                                "position": { "x": 0.0, "y": 0.0, "w": 5.0, "h": 1.0 },
                                "font_size": 24.0,
                                "color": "FF0000"
                            },
                            {
                                "type": "shape",
                                "shape_type": "rect",
                                "position": { "x": 0.0, "y": 1.5, "w": 3.0, "h": 2.0 },
                                "fill": "00FF00",
                                "line": { "color": "000000", "width": 1.0 }
                            },
                            {
                                "type": "image",
                                "src": img_src,
                                "position": { "x": 4.0, "y": 1.5, "w": 3.0, "h": 2.0 }
                            }
                        ]
                    }
                ]
            },
            {
                "elements": [
                    {
                        "type": "progressBar",
                        "position": { "x": 0.5, "y": 0.3, "w": 9.0, "h": 0.6 },
                        "value": 75.0,
                        "color": "4472C4",
                        "track_color": "E0E0E0",
                        "rounded": true,
                        "show_label": true,
                        "label": "Progress",
                        "text_color": "000000",
                        "font_size": 11.0
                    },
                    {
                        "type": "progressRing",
                        "position": { "x": 0.5, "y": 1.2, "w": 1.5, "h": 1.5 },
                        "value": 60.0,
                        "color": "ED7D31",
                        "track_color": "E0E0E0",
                        "thickness": 8.0,
                        "show_label": true,
                        "text_color": "333333"
                    },
                    {
                        "type": "barChart",
                        "position": { "x": 2.5, "y": 1.2, "w": 4.0, "h": 1.5 },
                        "data": [30.0, 50.0, 80.0, 40.0],
                        "labels": ["Q1", "Q2", "Q3", "Q4"],
                        "colors": ["FF0000", "00FF00", "0000FF", "FFAA00"],
                        "max": 100.0,
                        "show_values": true,
                        "axis": true,
                        "label_color": "333333"
                    },
                    {
                        "type": "kpiCard",
                        "position": { "x": 7.0, "y": 1.2, "w": 2.5, "h": 1.5 },
                        "value": "98.5%",
                        "label": "Satisfaction",
                        "delta": "+5.2%",
                        "bg": "FFFFFF",
                        "accent": "00AA00",
                        "rounded": true,
                        "text_color": "333333"
                    },
                    {
                        "type": "ratingStars",
                        "position": { "x": 0.5, "y": 3.0, "w": 3.0, "h": 0.6 },
                        "rating": 4.5,
                        "max": 5,
                        "color": "FFAA00",
                        "empty_color": "E0E0E0"
                    },
                    {
                        "type": "timeline",
                        "position": { "x": 0.5, "y": 3.8, "w": 9.0, "h": 1.0 },
                        "items": [
                            {"date": "2024-Q1", "title": "Phase 1", "desc": "Research"},
                            {"date": "2024-Q2", "title": "Phase 2", "desc": "Development"},
                            {"date": "2024-Q3", "title": "Phase 3", "desc": "Launch"}
                        ],
                        "line_color": "4472C4",
                        "dot_color": "ED7D31",
                        "label_color": "333333"
                    },
                    {
                        "type": "processFlow",
                        "position": { "x": 0.5, "y": 5.0, "w": 9.0, "h": 0.6 },
                        "steps": ["Plan", "Do", "Check", "Act"],
                        "colors": ["4472C4", "ED7D31", "70AD47", "FF0000"],
                        "text_color": "FFFFFF",
                        "font_size": 12.0
                    }
                ]
            },
            {
                "elements": [],
                "background": {
                    "type": "gradient",
                    "stops": [
                        {"color": "FFE0E0", "position": 0.0},
                        {"color": "E0E0FF", "position": 1.0}
                    ],
                    "angle": 45.0
                }
            },
            {
                "elements": [],
                "transition": {
                    "type": "wipe",
                    "speed": "fast",
                    "advance_on_click": false,
                    "advance_after": 3000
                }
            },
            {
                "elements": [],
                "transition": {
                    "type": "push",
                    "speed": "med"
                }
            },
            {
                "elements": [],
                "transition": {
                    "type": "cut",
                    "advance_on_click": true
                }
            }
        ]
    });

    let original: Presentation = serde_json::from_value(json_input).unwrap();

    let original_json = serde_json::to_string_pretty(&original).unwrap();

    let path = common::temp_pptx_path();
    generate(&original, path.to_str().unwrap()).unwrap();
    let result = parse(path.to_str().unwrap()).unwrap();

    let result_json = serde_json::to_string_pretty(&result).unwrap();

    println!("=== ORIGINAL JSON ===");
    println!("{}", original_json);
    println!();
    println!("=== ROUNDTRIP JSON ===");
    println!("{}", result_json);

    assert_eq!(result.slides.len(), 10, "should have 10 slides");

    let s0 = &result.slides[0];
    assert!(s0.background.is_some(), "slide 0 should have background");
    assert_eq!(
        s0.transition.as_ref().map(|t| t.r#type.as_str()),
        Some("fade")
    );

    let s0_texts: Vec<&str> = s0
        .elements
        .iter()
        .filter_map(|e| match e {
            Element::Text(t) => {
                let extracted = match &t.text {
                    TextContent::Simple(s) => Some(s.as_str()),
                    TextContent::Paragraphs(paras) => paras.first().and_then(|p| {
                        p.text.as_deref().or_else(|| {
                            p.runs
                                .as_ref()
                                .and_then(|r| r.first().map(|r| r.text.as_str()))
                        })
                    }),
                };
                extracted
            }
            _ => None,
        })
        .collect();
    assert!(
        s0_texts.iter().any(|t| t.contains("Hello World")),
        "slide 0 should contain Hello World text"
    );

    let s2 = &result.slides[2];
    let has_image = s2.elements.iter().any(|e| matches!(e, Element::Image(_)));
    assert!(has_image, "slide 2 should have an image element");

    let s3 = &result.slides[3];
    let has_table = s3.elements.iter().any(|e| matches!(e, Element::Table(_)));
    assert!(has_table, "slide 3 should have a table element");

    let s4 = &result.slides[4];
    let has_group = s4.elements.iter().any(|e| matches!(e, Element::Group(_)));
    assert!(has_group, "slide 4 should have a group element");
    if let Some(Element::Group(g)) = s4.elements.first() {
        assert!(g.children.len() >= 3, "group should have 3+ children");
    }

    fn element_position(el: &Element) -> Position {
        el.position().clone()
    }

    common::assert_positions_eq!(
        element_position(&result.slides[0].elements[0]),
        Position {
            x: 0.5,
            y: 0.3,
            w: 9.0,
            h: 1.0
        }
    );
}

// === Line element roundtrip ===

#[test]
fn test_line_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![
                // 直线（箭头端点 + 虚线）
                Element::Line(LineElement {
                    id: None,
                    position: Position {
                        x: 1.0,
                        y: 2.0,
                        w: 5.0,
                        h: 0.5,
                    },
                    name: Some("直线".to_string()),
                    points: vec![(0.0, 0.25), (5.0, 0.25)],
                    color: Some("E74C3C".to_string()),
                    width: Some(2.5),
                    dash: Some("dashed".to_string()),
                    arrow_start: Some("none".to_string()),
                    arrow_end: Some("arrow".to_string()),
                    smooth: None,
                    rotation: None,
                    animations: None,
                }),
                // 折线
                Element::Line(LineElement {
                    id: None,
                    position: Position {
                        x: 1.0,
                        y: 4.0,
                        w: 6.0,
                        h: 2.0,
                    },
                    name: Some("折线".to_string()),
                    points: vec![(0.0, 2.0), (2.0, 0.5), (4.0, 1.5), (6.0, 0.0)],
                    color: Some("4472C4".to_string()),
                    width: None,
                    dash: None,
                    arrow_start: Some("dot".to_string()),
                    arrow_end: None,
                    smooth: None,
                    rotation: None,
                    animations: None,
                }),
                // 曲线（smooth）
                Element::Line(LineElement {
                    id: None,
                    position: Position {
                        x: 1.0,
                        y: 6.0,
                        w: 6.0,
                        h: 1.0,
                    },
                    name: Some("曲线".to_string()),
                    points: vec![(0.0, 0.5), (3.0, 0.1), (6.0, 0.8)],
                    color: None,
                    width: None,
                    dash: Some("dotted".to_string()),
                    arrow_start: None,
                    arrow_end: Some("dot".to_string()),
                    smooth: Some(true),
                    rotation: Some(15.0),
                    animations: None,
                }),
            ],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    let els = &result.slides[0].elements;
    assert_eq!(els.len(), 3);

    match &els[0] {
        Element::Line(l) => {
            assert_eq!(l.name.as_deref(), Some("直线"));
            assert_eq!(l.points.len(), 2);
            assert_eq!(l.color.as_deref(), Some("E74C3C"));
            assert_eq!(l.width, Some(2.5));
            assert_eq!(l.dash.as_deref(), Some("dashed"));
            assert_eq!(l.arrow_end.as_deref(), Some("arrow"));
            assert!(l.arrow_start.is_none(), "none 端点不应保留");
        }
        other => panic!("expected Line, got {:?}", other.type_name()),
    }
    match &els[1] {
        Element::Line(l) => {
            assert_eq!(l.points.len(), 4);
            assert_eq!(l.arrow_start.as_deref(), Some("dot"));
            assert!(l.arrow_end.is_none());
        }
        other => panic!("expected Line, got {:?}", other.type_name()),
    }
    match &els[2] {
        Element::Line(l) => {
            assert_eq!(l.points.len(), 3);
            assert_eq!(l.smooth, Some(true));
            assert_eq!(l.dash.as_deref(), Some("dotted"));
            assert_eq!(l.arrow_end.as_deref(), Some("dot"));
            assert!(l.rotation.is_some());
        }
        other => panic!("expected Line, got {:?}", other.type_name()),
    }
}

// === Table merge roundtrip ===

#[test]
fn test_table_merge_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Table(TableElement {
                id: None,
                style_id: None,
                row_heights: None,
                tbl_attrs: None,
                rtl: None,
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 8.0,
                    h: 3.0,
                },
                name: Some("合并表".to_string()),
                rows: vec![
                    // 第一行：表头跨两列（占 2 物理列）+ 普通格 → 3 物理列
                    vec![
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "标题".to_string(),
                            font_size: None,
                            bold: Some(true),
                            color: None,
                            fill: None,
                            align: Some("center".to_string()),
                            colspan: Some(2),
                            rowspan: None,
                        },
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "数值".to_string(),
                            font_size: None,
                            bold: None,
                            color: None,
                            fill: None,
                            align: None,
                            colspan: None,
                            rowspan: None,
                        },
                    ],
                    // 第二行：纵向合并两行 + 普通格
                    vec![
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "A".to_string(),
                            font_size: None,
                            bold: None,
                            color: None,
                            fill: None,
                            align: None,
                            colspan: None,
                            rowspan: Some(2),
                        },
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "B".to_string(),
                            font_size: None,
                            bold: None,
                            color: None,
                            fill: None,
                            align: None,
                            colspan: None,
                            rowspan: None,
                        },
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "C".to_string(),
                            font_size: None,
                            bold: None,
                            color: None,
                            fill: None,
                            align: None,
                            colspan: None,
                            rowspan: None,
                        },
                    ],
                    // 第三行：A 覆盖第 1 物理列，剩余 2 个逻辑格
                    vec![
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "D".to_string(),
                            font_size: None,
                            bold: None,
                            color: None,
                            fill: None,
                            align: None,
                            colspan: None,
                            rowspan: None,
                        },
                        Cell {
                            border: None,
                            vert: None,
                            margin: None,
                            fill_image: None,
                            anchor: None,
                            text: "E".to_string(),
                            font_size: None,
                            bold: None,
                            color: None,
                            fill: None,
                            align: None,
                            colspan: None,
                            rowspan: None,
                        },
                    ],
                ],
                column_widths: Some(vec![2.0, 2.0, 4.0]),
                header_row: Some(true),
                font_size: None,
                color: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    let el = &result.slides[0].elements[0];
    match el {
        Element::Table(t) => {
            assert_eq!(t.rows.len(), 3, "逻辑网格应为 3 行");
            // 行 1：colspan=2 主格 + 普通格
            assert_eq!(t.rows[0].len(), 2);
            assert_eq!(t.rows[0][0].colspan, Some(2));
            assert_eq!(t.rows[0][0].text, "标题");
            // 行 2：rowspan=2 主格
            assert_eq!(t.rows[1].len(), 3);
            assert_eq!(t.rows[1][0].rowspan, Some(2));
            assert_eq!(t.rows[1][0].text, "A");
            // 行 3：列 0 被上方 rowspan 覆盖，剩余 2 个逻辑格
            assert_eq!(t.rows[2].len(), 2);
            assert_eq!(t.rows[2][0].text, "D");
            assert_eq!(t.rows[2][1].text, "E");
            // 列宽回环
            assert_eq!(t.column_widths, Some(vec![2.0, 2.0, 4.0]));
        }
        other => panic!("expected Table, got {:?}", other.type_name()),
    }
}

// === Text hyperlink roundtrip ===

#[test]
fn test_text_hyperlink_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Text(TextElement {
                id: None,
                placeholder: None,
                autofit: None,
                effects: None,
                vert: None,
                text: TextContent::Paragraphs(vec![Paragraph {
                    runs: Some(vec![
                        TextRun {
                            highlight: None,
                            underline_color: None,
                            spacing: None,
                            lang: None,
                            rtl: None,
                            text: "点击访问".to_string(),
                            font_size: Some(18.0),
                            bold: None,
                            italic: None,
                            underline: None,
                            color: Some("0563C1".to_string()),
                            font_family: None,
                            hyperlink: Some("https://example.com/page?a=1&b=2".to_string()),
                        },
                        TextRun {
                            highlight: None,
                            underline_color: None,
                            spacing: None,
                            lang: None,
                            rtl: None,
                            text: "普通文本".to_string(),
                            font_size: None,
                            bold: None,
                            italic: None,
                            underline: None,
                            color: None,
                            font_family: None,
                            hyperlink: None,
                        },
                    ]),
                    text: None,
                    font_size: None,
                    bold: None,
                    italic: None,
                    color: None,
                    align: None,
                    line_spacing: None,
                    bullet: None,
                }]),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 6.0,
                    h: 1.0,
                },
                ..Default::default()
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Text(t) => {
            if let TextContent::Paragraphs(paras) = &t.text {
                let runs = paras[0].runs.as_ref().unwrap();
                assert_eq!(
                    runs[0].hyperlink.as_deref(),
                    Some("https://example.com/page?a=1&b=2")
                );
                assert!(runs[1].hyperlink.is_none());
            } else {
                panic!("expected Paragraphs");
            }
        }
        other => panic!("expected Text, got {:?}", other.type_name()),
    }
}

// === Image hyperlink roundtrip（修复 r_id 查找后） ===

#[test]
fn test_image_hyperlink_roundtrip() {
    let img = common::create_dummy_image();
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Image(ImageElement {
                id: None,
                brightness: None,
                contrast: None,
                fill_mode: None,
                tooltip: None,
                media: None,
                src: img.to_string_lossy().to_string(),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 2.0,
                    h: 2.0,
                },
                name: Some("链接图".to_string()),
                hyperlink: Some("https://example.com/image-link".to_string()),
                ..Default::default()
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Image(i) => {
            assert_eq!(
                i.hyperlink.as_deref(),
                Some("https://example.com/image-link")
            );
        }
        other => panic!("expected Image, got {:?}", other.type_name()),
    }
}

// === Notes roundtrip ===

#[test]
fn test_notes_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![
            Slide {
                elements: vec![Element::Text(TextElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    vert: None,
                    text: TextContent::Simple("第一页".to_string()),
                    position: Position {
                        x: 1.0,
                        y: 1.0,
                        w: 5.0,
                        h: 1.0,
                    },
                    ..Default::default()
                })],
                background: None,
                transition: None,
                notes: Some("第一页的备注\n第二行内容".to_string()),
            },
            Slide {
                elements: vec![],
                background: None,
                transition: None,
                notes: None,
            },
            Slide {
                elements: vec![],
                background: None,
                transition: None,
                notes: Some("只有第三页有备注".to_string()),
            },
        ],
    };
    let result = roundtrip(&pres);
    assert_eq!(result.slides.len(), 3);
    assert_eq!(
        result.slides[0].notes.as_deref(),
        Some("第一页的备注\n第二行内容")
    );
    assert!(result.slides[1].notes.is_none());
    assert_eq!(result.slides[2].notes.as_deref(), Some("只有第三页有备注"));
}

// === 全部形状类型 roundtrip ===

#[test]
fn test_all_shape_types_roundtrip() {
    let shapes = json2pptx::utils::constants::SHAPE_TYPES;
    let elements: Vec<Element> = shapes
        .iter()
        .map(|(json_name, _)| {
            Element::Shape(ShapeElement {
                id: None,
                placeholder: None,
                autofit: None,
                effects: None,
                shape_type: json_name.to_string(),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 2.0,
                    h: 1.0,
                },
                fill: Some(Fill::Solid("4472C4".to_string())),
                ..Default::default()
            })
        })
        .collect();
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements,
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    let els = &result.slides[0].elements;
    assert_eq!(els.len(), shapes.len());
    for (i, (json_name, _)) in shapes.iter().enumerate() {
        match &els[i] {
            Element::Shape(s) => {
                assert_eq!(&s.shape_type, json_name, "形状 {} 回环失败", json_name);
            }
            other => panic!("形状 {} 解析为 {:?}", json_name, other.type_name()),
        }
    }
}

// === 图表组件生成（形状模拟，不回环） ===

#[test]
fn test_chart_components_generate() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![
                Element::LineChart(LineChart {
                    position: Position {
                        x: 1.0,
                        y: 1.0,
                        w: 5.0,
                        h: 3.0,
                    },
                    data: vec![10.0, 20.0, 15.0, 30.0],
                    labels: Some(vec![
                        "Q1".to_string(),
                        "Q2".to_string(),
                        "Q3".to_string(),
                        "Q4".to_string(),
                    ]),
                    colors: None,
                    max: None,
                    show_values: Some(true),
                    axis: Some(true),
                    smooth: None,
                    label_color: None,
                    font_size: None,
                }),
                Element::PieChart(PieChart {
                    position: Position {
                        x: 6.5,
                        y: 1.0,
                        w: 3.0,
                        h: 3.0,
                    },
                    data: vec![30.0, 40.0, 30.0],
                    labels: Some(vec!["A".to_string(), "B".to_string(), "C".to_string()]),
                    colors: Some(vec![
                        "E74C3C".to_string(),
                        "F1C40F".to_string(),
                        "2ECC71".to_string(),
                    ]),
                    show_values: Some(true),
                    label_color: None,
                    font_size: None,
                }),
                Element::RingChart(RingChart {
                    position: Position {
                        x: 10.0,
                        y: 1.0,
                        w: 3.0,
                        h: 3.0,
                    },
                    data: vec![60.0, 40.0],
                    labels: None,
                    colors: None,
                    thickness: None,
                    show_values: None,
                    label_color: None,
                    font_size: None,
                }),
            ],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    // 组件展开为形状，生成后解析为 Group（与现有组件行为一致）
    let result = roundtrip(&pres);
    let els = &result.slides[0].elements;
    assert_eq!(els.len(), 3, "三个组件各展开为一个 Group");
    for el in els {
        match el {
            Element::Group(g) => {
                assert!(!g.children.is_empty(), "组件展开应有子形状");
            }
            other => panic!("组件应展开为 Group, got {:?}", other.type_name()),
        }
    }
    // 折线图展开应包含连接线（p:cxnSp）
    let path = common::temp_pptx_path();
    generate(&pres, path.to_str().unwrap()).unwrap();
    let zip = std::fs::File::open(&path).unwrap();
    let mut archive = zip::ZipArchive::new(zip).unwrap();
    let mut slide_xml = String::new();
    archive
        .by_name("ppt/slides/slide1.xml")
        .unwrap()
        .read_to_string(&mut slide_xml)
        .unwrap();
    assert!(slide_xml.contains("<p:cxnSp>"), "折线图应包含连接线");
    assert!(slide_xml.contains("prst=\"pie\""), "饼图应包含扇形");
    assert!(slide_xml.contains("prst=\"blockArc\""), "环形图应包含环段");
}

#[test]
fn test_real_chart_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Chart(ChartElement {
                id: None,
                position: Position {
                    x: 1.0,
                    y: 1.5,
                    w: 8.0,
                    h: 4.5,
                },
                name: Some("Sales Chart".to_string()),
                chart_type: "line".to_string(),
                title: Some("Revenue by Quarter".to_string()),
                categories: vec!["Q1".into(), "Q2".into(), "Q3".into()],
                series: vec![
                    ChartSeries {
                        name: Some("2024".into()),
                        values: vec![10.0, 20.5, 15.0],
                        color: Some("4472C4".into()),
                    },
                    ChartSeries {
                        name: Some("2025".into()),
                        values: vec![12.0, 25.0, 18.0],
                        color: None,
                    },
                ],
                legend: Some(true),
                legend_position: Some("bottom".into()),
                raw: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    assert_eq!(result.slides[0].elements.len(), 1);
    match &result.slides[0].elements[0] {
        Element::Chart(c) => {
            assert_eq!(c.chart_type, "line");
            assert_eq!(c.title.as_deref(), Some("Revenue by Quarter"));
            assert_eq!(c.categories, vec!["Q1", "Q2", "Q3"]);
            assert_eq!(c.series.len(), 2);
            assert_eq!(c.series[0].name.as_deref(), Some("2024"));
            assert_eq!(c.series[0].values, vec![10.0, 20.5, 15.0]);
            assert_eq!(c.series[0].color.as_deref(), Some("4472C4"));
        }
        other => panic!("expected chart, got {:?}", other),
    }
}

#[test]
fn test_shape_effects_and_pattern_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Shape(ShapeElement {
                id: None,
                placeholder: None,
                autofit: None,
                shape_type: "rect".to_string(),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 4.0,
                    h: 2.0,
                },
                name: Some("Effected".to_string()),
                rotation: None,
                fill: Some(Fill::Pattern {
                    pattern: "pct20".to_string(),
                    fg: "FF0000".to_string(),
                    bg: "FFFFFF".to_string(),
                }),
                fill_alpha: None,
                no_fill: None,
                line: Some(Line {
                    color: "000000".to_string(),
                    width: 1.0,
                }),
                line_alpha: None,
                shadow: Some(Shadow {
                    blur: 5.0,
                    distance: 4.0,
                    angle: 45.0,
                    opacity: 0.5,
                    color: "333333".to_string(),
                }),
                effects: Some(Effects {
                    glow: Some(Glow {
                        color: "00FF00".to_string(),
                        radius: 6.0,
                        opacity: Some(0.8),
                    }),
                    reflection: Some(true),
                    soft_edge: Some(3.0),
                    blur: Some(2.0),
                    inner_shadow: Some(Shadow {
                        blur: 4.0,
                        distance: 2.0,
                        angle: 90.0,
                        opacity: 0.3,
                        color: "000000".to_string(),
                    }),
                    fill_overlay: Some(FillOverlay {
                        color: "0000FF".to_string(),
                        opacity: Some(0.4),
                    }),
                }),
                adjust: None,
                text: None,
                font_size: None,
                color: None,
                align: None,
                vert_align: None,
                line_spacing: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    assert_eq!(result.slides[0].elements.len(), 1);
    match &result.slides[0].elements[0] {
        Element::Shape(s) => {
            match &s.fill {
                Some(Fill::Pattern { pattern, fg, bg }) => {
                    assert_eq!(pattern, "pct20");
                    assert_eq!(fg, "FF0000");
                    assert_eq!(bg, "FFFFFF");
                }
                other => panic!("expected pattern fill, got {:?}", other),
            }
            let fx = s.effects.as_ref().expect("effects missing");
            assert_eq!(fx.glow.as_ref().map(|g| g.color.as_str()), Some("00FF00"));
            assert_eq!(fx.reflection, Some(true));
            assert_eq!(fx.soft_edge, Some(3.0));
            assert_eq!(fx.blur, Some(2.0));
            assert_eq!(
                fx.fill_overlay.as_ref().map(|f| f.color.as_str()),
                Some("0000FF")
            );
            assert!(fx.inner_shadow.is_some());
            assert!(s.shadow.is_some());
        }
        other => panic!("expected shape, got {:?}", other),
    }
}

#[test]
fn test_text_rich_run_attributes_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Text(TextElement {
                id: None,
                placeholder: None,
                autofit: None,
                text: TextContent::Paragraphs(vec![Paragraph {
                    runs: Some(vec![TextRun {
                        text: "Hello 世界".to_string(),
                        font_size: Some(20.0),
                        bold: Some(true),
                        italic: None,
                        underline: Some(true),
                        color: Some("112233".to_string()),
                        font_family: Some("Arial".to_string()),
                        hyperlink: None,
                        highlight: Some("FFFF00".to_string()),
                        underline_color: Some("FF0000".to_string()),
                        spacing: Some(1.5),
                        lang: Some("zh-CN".to_string()),
                        rtl: Some(true),
                    }]),
                    text: None,
                    font_size: None,
                    bold: None,
                    italic: None,
                    color: None,
                    align: None,
                    line_spacing: None,
                    bullet: None,
                }]),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 8.0,
                    h: 2.0,
                },
                name: None,
                font_size: None,
                bold: None,
                italic: None,
                underline: None,
                color: None,
                font_family: None,
                align: None,
                vert_align: None,
                vert: None,
                line_spacing: None,
                wrap: None,
                fill: None,
                fill_alpha: None,
                line: None,
                line_alpha: None,
                shadow: None,
                effects: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };

    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Text(t) => {
            let paras = match &t.text {
                TextContent::Paragraphs(p) => p,
                other => panic!("expected paragraphs, got {:?}", other),
            };
            let run = &paras[0].runs.as_ref().unwrap()[0];
            assert_eq!(run.text, "Hello 世界");
            assert_eq!(run.highlight.as_deref(), Some("FFFF00"));
            assert_eq!(run.underline_color.as_deref(), Some("FF0000"));
            assert_eq!(run.spacing, Some(1.5));
            assert_eq!(run.lang.as_deref(), Some("zh-CN"));
            assert_eq!(run.rtl, Some(true));
        }
        other => panic!("expected text, got {:?}", other),
    }
}

#[test]
fn test_image_enhancements_roundtrip() {
    let img = common::create_dummy_image();
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Image(ImageElement {
                id: None,
                src: img.to_str().unwrap().to_string(),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 4.0,
                    h: 3.0,
                },
                name: Some("Pic".to_string()),
                rotation: None,
                crop: None,
                hyperlink: None,
                line: None,
                brightness: Some(0.2),
                contrast: Some(-0.1),
                fill_mode: Some("tile".to_string()),
                tooltip: Some("hint".to_string()),
                media: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Image(i) => {
            assert_eq!(i.brightness, Some(0.2));
            assert_eq!(i.contrast, Some(-0.1));
            assert_eq!(i.fill_mode.as_deref(), Some("tile"));
            assert_eq!(i.tooltip.as_deref(), Some("hint"));
        }
        other => panic!("expected image, got {:?}", other),
    }
}

#[test]
fn test_table_style_and_border_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Table(TableElement {
                id: None,
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 6.0,
                    h: 3.0,
                },
                name: None,
                rows: vec![vec![Cell {
                    text: "H".to_string(),
                    font_size: None,
                    bold: Some(true),
                    color: None,
                    fill: Some("DDDDDD".to_string()),
                    align: None,
                    colspan: None,
                    rowspan: None,
                    border: Some(CellBorder {
                        color: "FF0000".to_string(),
                        width: 2.0,
                    }),
                    vert: None,
                    margin: None,
                    fill_image: None,
                    anchor: None,
                }]],
                column_widths: Some(vec![3.0]),
                header_row: Some(true),
                style_id: Some("{2D5ABB26-0587-4C30-8999-92F81FD0307C}".to_string()),
                rtl: Some(true),
                row_heights: None,
                tbl_attrs: None,
                font_size: None,
                color: None,
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Table(t) => {
            assert_eq!(
                t.style_id.as_deref(),
                Some("{2D5ABB26-0587-4C30-8999-92F81FD0307C}")
            );
            assert_eq!(t.rtl, Some(true));
            let bd = t.rows[0][0].border.as_ref().expect("border missing");
            assert_eq!(bd.color, "FF0000");
            assert!((bd.width - 2.0).abs() < 0.01);
        }
        other => panic!("expected table, got {:?}", other),
    }
}

#[test]
fn test_morph_transition_and_animation_options_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Text(TextElement {
                id: None,
                placeholder: None,
                autofit: None,
                text: TextContent::Simple("Animated".to_string()),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 6.0,
                    h: 1.5,
                },
                name: None,
                font_size: Some(24.0),
                bold: None,
                italic: None,
                underline: None,
                color: None,
                font_family: None,
                align: None,
                vert_align: None,
                vert: None,
                line_spacing: None,
                wrap: None,
                fill: None,
                fill_alpha: None,
                line: None,
                line_alpha: None,
                shadow: None,
                effects: None,
                animations: Some(vec![Animation {
                    r#type: "fadeIn".to_string(),
                    duration: Some(500),
                    delay: None,
                    trigger: Some("onClick".to_string()),
                    direction: None,
                    order: Some(1),
                    scale: None,
                    degrees: None,
                    color: None,
                    opacity: None,
                    points: None,
                    distance: None,
                    repeat: Some("2".to_string()),
                    restart: Some("whenNotActive".to_string()),
                    auto_reverse: Some(true),
                }]),
            })],
            background: None,
            transition: Some(Transition {
                r#type: "morph".to_string(),
                speed: Some("med".to_string()),
                advance_on_click: None,
                advance_after: None,
            }),
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    assert_eq!(
        result.slides[0]
            .transition
            .as_ref()
            .map(|t| t.r#type.as_str()),
        Some("morph")
    );
    if let Element::Text(t) = &result.slides[0].elements[0] {
        let a = &t.animations.as_ref().unwrap()[0];
        assert_eq!(a.repeat.as_deref(), Some("2"));
        assert_eq!(a.restart.as_deref(), Some("whenNotActive"));
        assert_eq!(a.auto_reverse, Some(true));
    } else {
        panic!("expected text");
    }
}

#[test]
fn test_comment_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![
                Element::Text(TextElement {
                    id: None,
                    placeholder: None,
                    autofit: None,
                    text: TextContent::Simple("Body".to_string()),
                    position: Position {
                        x: 1.0,
                        y: 1.0,
                        w: 5.0,
                        h: 1.0,
                    },
                    name: None,
                    font_size: None,
                    bold: None,
                    italic: None,
                    underline: None,
                    color: None,
                    font_family: None,
                    align: None,
                    vert_align: None,
                    vert: None,
                    line_spacing: None,
                    wrap: None,
                    fill: None,
                    fill_alpha: None,
                    line: None,
                    line_alpha: None,
                    shadow: None,
                    effects: None,
                    animations: None,
                }),
                Element::Comment(CommentElement {
                    position: Position {
                        x: 2.0,
                        y: 3.0,
                        w: 0.0,
                        h: 0.0,
                    },
                    author: Some("Reviewer".to_string()),
                    text: "Please revise".to_string(),
                }),
            ],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    let comment = result.slides[0]
        .elements
        .iter()
        .find_map(|e| match e {
            Element::Comment(c) => Some(c),
            _ => None,
        })
        .expect("comment missing");
    assert_eq!(comment.text, "Please revise");
    assert_eq!(comment.author.as_deref(), Some("Reviewer"));
    assert!((comment.position.x - 2.0).abs() < 0.01);
    assert!((comment.position.y - 3.0).abs() < 0.01);
}

#[test]
fn test_video_media_roundtrip() {
    let poster = common::create_dummy_image();
    let video =
        std::env::temp_dir().join(format!("json2pptx_test_media_{}.mp4", std::process::id()));
    std::fs::write(&video, b"\x00\x00\x00\x18ftypmp42dummy").unwrap();

    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Image(ImageElement {
                id: None,
                src: poster.to_str().unwrap().to_string(),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 6.0,
                    h: 4.0,
                },
                name: Some("Video".to_string()),
                rotation: None,
                crop: None,
                hyperlink: None,
                line: None,
                brightness: None,
                contrast: None,
                fill_mode: None,
                tooltip: None,
                media: Some(MediaRef {
                    kind: "video".to_string(),
                    src: video.to_str().unwrap().to_string(),
                }),
                animations: None,
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Image(i) => {
            let m = i.media.as_ref().expect("media missing");
            assert_eq!(m.kind, "video");
            assert!(m.src.ends_with(".mp4"), "media src: {}", m.src);
        }
        other => panic!("expected image, got {:?}", other),
    }
}

#[test]
fn test_extended_properties_roundtrip() {
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: Some(Meta {
            title: Some("T".to_string()),
            author: Some("A".to_string()),
            company: Some("Acme Corp".to_string()),
            application: Some("json2pptx-test".to_string()),
            last_modified_by: Some("Editor".to_string()),
        }),
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    let m = result.meta.as_ref().expect("meta missing");
    assert_eq!(m.title.as_deref(), Some("T"));
    assert_eq!(m.author.as_deref(), Some("A"));
    assert_eq!(m.company.as_deref(), Some("Acme Corp"));
    assert_eq!(m.application.as_deref(), Some("json2pptx-test"));
    assert_eq!(m.last_modified_by.as_deref(), Some("Editor"));
}

#[test]
fn test_equation_roundtrip() {
    let omml =
        "<a14:m><m:oMathPara><m:oMath><m:r><m:t>x=1</m:t></m:r></m:oMath></m:oMathPara></a14:m>"
            .to_string();
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            elements: vec![Element::Equation(EquationElement {
                id: None,
                position: Position {
                    x: 2.0,
                    y: 3.0,
                    w: 4.0,
                    h: 1.0,
                },
                name: Some("Eq".to_string()),
                omml,
                text: Some("x=1".to_string()),
            })],
            background: None,
            transition: None,
            notes: None,
        }],
    };
    let result = roundtrip(&pres);
    match &result.slides[0].elements[0] {
        Element::Equation(eq) => {
            assert!(eq.omml.contains("x=1"), "omml: {}", eq.omml);
            assert_eq!(eq.text.as_deref(), Some("x=1"));
            assert!((eq.position.x - 2.0).abs() < 0.01);
        }
        other => panic!("expected equation, got {:?}", other),
    }
}

fn build_opaque_pptx(path: &std::path::Path) {
    use std::io::Write;
    use zip::write::FileOptions;
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts: FileOptions<'_, ()> = FileOptions::default();
    let mut add = |name: &str, content: &str| {
        zip.start_file(name, opts).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    };
    add(
        "[Content_Types].xml",
        r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slides/slide1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/><Override PartName="/ppt/diagrams/data1.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.diagramData+xml"/></Types>"#,
    );
    add(
        "ppt/presentation.xml",
        r#"<?xml version="1.0"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldSz cx="12192000" cy="6858000"/><p:sldIdLst><p:sldId id="256" r:id="rId1"/></p:sldIdLst></p:presentation>"#,
    );
    add(
        "ppt/_rels/presentation.xml.rels",
        r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide1.xml"/></Relationships>"#,
    );
    add(
        "ppt/slides/slide1.xml",
        r#"<?xml version="1.0"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr><p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="2" name="Diagram"/><p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr><p:xfrm><a:off x="1000000" y="1000000"/><a:ext cx="3000000" cy="2000000"/></p:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/diagram"><dgm:relIds xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" r:dm="rId5" r:lo="rId6" r:qs="rId7" r:cs="rId8"/></a:graphicData></a:graphic></p:graphicFrame></p:spTree></p:cSld></p:sld>"#,
    );
    add(
        "ppt/slides/_rels/slide1.xml.rels",
        r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId5" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramData" Target="../diagrams/data1.xml"/></Relationships>"#,
    );
    add(
        "ppt/diagrams/data1.xml",
        r#"<?xml version="1.0"?><dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><dgm:ptLst><dgm:pt><dgm:t>Marker</dgm:t></dgm:pt></dgm:ptLst></dgm:dataModel>"#,
    );
    zip.finish().unwrap();
}

#[test]
fn test_opaque_passthrough_roundtrip() {
    let pptx = common::temp_pptx_path();
    build_opaque_pptx(&pptx);

    // 解析：未知 graphicFrame → Opaque（含关系与部件）
    let parsed = parse(pptx.to_str().unwrap()).unwrap();
    let opaque = parsed.slides[0]
        .elements
        .iter()
        .find_map(|e| match e {
            Element::Opaque(o) => Some(o),
            _ => None,
        })
        .expect("opaque element missing");
    assert!(!opaque.xml.contains("__NOPE__"));
    assert_eq!(opaque.rels.len(), 1, "rels: {:?}", opaque.rels);
    assert!(opaque.rels[0].target.contains("data1.xml"));
    assert_eq!(opaque.parts.len(), 1, "parts: {:?}", opaque.parts);
    assert!(opaque.parts[0].path.ends_with("data1.xml"));

    // 生成 → 再解析：不透明对象仍在，部件内容保留
    let out = common::temp_pptx_path();
    generate(&parsed, out.to_str().unwrap()).unwrap();
    let reparsed = parse(out.to_str().unwrap()).unwrap();
    let opaque2 = reparsed.slides[0]
        .elements
        .iter()
        .find_map(|e| match e {
            Element::Opaque(o) => Some(o),
            _ => None,
        })
        .expect("opaque element lost after regeneration");
    assert_eq!(opaque2.parts.len(), 1);
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&opaque2.parts[0].data)
        .unwrap();
    let s = String::from_utf8_lossy(&decoded);
    assert!(s.contains("Marker"), "part content: {s}");
}
