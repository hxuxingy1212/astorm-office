//! unpack / repack 集成测试
//!
//! 验证 PPTX → unpack（按幻灯片拆分的 JSON 中间产物）→ repack → PPTX 的完整链路。

mod common;

use json2pptx::model::elements::*;
use json2pptx::model::*;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

/// 构造带图片的演示文稿模型（元素图 + 背景图）
fn build_pres_with_images(img: &str) -> Presentation {
    Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: Some(Meta {
            company: None,
            application: None,
            last_modified_by: None,
            title: Some("Unpack Test".to_string()),
            author: Some("json2pptx".to_string()),
        }),
        template: None,
        theme: Some(Theme {
            colors: Some(HashMap::from([(
                "accent1".to_string(),
                "4472C4".to_string(),
            )])),
            major_font: Some("Calibri Light".to_string()),
            minor_font: Some("Calibri".to_string()),
        }),
        slides: vec![
            Slide {
                background: Some(serde_json::json!({ "type": "image", "src": img })),
                transition: None,
                elements: vec![
                    Element::Text(TextElement {
                        placeholder: None,
                        autofit: None,
                        effects: None,
                        vert: None,
                        text: TextContent::Simple("第一页".to_string()),
                        position: Position {
                            x: 1.0,
                            y: 1.0,
                            w: 8.0,
                            h: 1.0,
                        },
                        ..Default::default()
                    }),
                    Element::Image(ImageElement {
                        brightness: None,
                        contrast: None,
                        fill_mode: None,
                        tooltip: None,
                        media: None,
                        src: img.to_string(),
                        position: Position {
                            x: 5.0,
                            y: 3.0,
                            w: 2.0,
                            h: 2.0,
                        },
                        ..Default::default()
                    }),
                ],
                notes: None,
            },
            Slide {
                background: None,
                transition: None,
                elements: vec![Element::Text(TextElement {
                    placeholder: None,
                    autofit: None,
                    effects: None,
                    vert: None,
                    text: TextContent::Simple("第二页".to_string()),
                    position: Position {
                        x: 1.0,
                        y: 1.0,
                        w: 8.0,
                        h: 1.0,
                    },
                    ..Default::default()
                })],
                notes: None,
            },
        ],
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("json2pptx_unpack_test")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn test_unpack_structure_and_media() {
    let img = common::create_dummy_image();
    let pptx = common::temp_pptx_path();
    let pres = build_pres_with_images(img.to_str().unwrap());
    json2pptx::generate(&pres, pptx.to_str().unwrap()).unwrap();

    let out = temp_dir("structure");
    let r = json2pptx::unpack(pptx.to_str().unwrap(), out.to_str().unwrap()).unwrap();
    assert_eq!(r.slides, 2);

    // presentation.json：slides 为路径数组
    let pres_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("presentation.json")).unwrap())
            .unwrap();
    let slides = pres_json["slides"].as_array().unwrap();
    assert_eq!(slides.len(), 2);
    assert_eq!(slides[0], "ppt/slides/slide1.json");
    assert_eq!(slides[1], "ppt/slides/slide2.json");
    assert_eq!(pres_json["width"], 13.333);
    assert_eq!(pres_json["meta"]["title"], "Unpack Test");

    // 每张幻灯片一个 JSON
    let slide1: Slide =
        serde_json::from_str(&std::fs::read_to_string(out.join("ppt/slides/slide1.json")).unwrap())
            .unwrap();
    let slide2: Slide =
        serde_json::from_str(&std::fs::read_to_string(out.join("ppt/slides/slide2.json")).unwrap())
            .unwrap();
    assert_eq!(slide1.elements.len(), 2);
    assert_eq!(slide2.elements.len(), 1);

    // 图片 src 改写为包内路径
    match &slide1.elements[1] {
        Element::Image(img_el) => {
            assert!(
                img_el.src.starts_with("ppt/media/"),
                "src 应为包内路径，实际: {}",
                img_el.src
            );
        }
        _ => panic!("预期图片元素"),
    }
    // 背景图 src 同样改写
    let bg = slide1.background.as_ref().unwrap();
    assert_eq!(bg["type"], "image");
    assert!(bg["src"].as_str().unwrap().starts_with("ppt/media/"));

    // 媒体文件实体存在且字节一致
    let media_dir = out.join("ppt/media");
    let files: Vec<_> = std::fs::read_dir(&media_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(files.len(), 1, "应提取 1 个媒体文件");
    let extracted = std::fs::read(files[0].path()).unwrap();
    let original = std::fs::read(&img).unwrap();
    assert_eq!(extracted, original, "媒体文件字节应一致");
}

#[test]
fn test_unpack_without_media() {
    let pptx = common::temp_pptx_path();
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            background: None,
            transition: None,
            elements: vec![Element::Text(TextElement {
                placeholder: None,
                autofit: None,
                effects: None,
                vert: None,
                text: TextContent::Simple("无图".to_string()),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 8.0,
                    h: 1.0,
                },
                ..Default::default()
            })],
            notes: None,
        }],
    };
    json2pptx::generate(&pres, pptx.to_str().unwrap()).unwrap();

    let out = temp_dir("no_media");
    let r = json2pptx::unpack(pptx.to_str().unwrap(), out.to_str().unwrap()).unwrap();
    assert_eq!(r.slides, 1);
    assert!(out.join("presentation.json").exists());
    assert!(out.join("ppt/slides/slide1.json").exists());
    assert!(
        !out.join("ppt/media").exists(),
        "无媒体时不应创建 ppt/media"
    );
}

#[test]
fn test_unpack_url_becomes_local() {
    // URL 图片在 generate 时被下载嵌入 PPTX 内部，
    // unpack 后以包内路径存在（预期行为，无需网络）。
    // 用户编辑产物时把 src 改为 URL，repack 会原样保留（不重写）。
    let img = common::create_dummy_image();
    let pptx = common::temp_pptx_path();
    let pres = Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: None,
        theme: None,
        template: None,
        slides: vec![Slide {
            background: None,
            transition: None,
            elements: vec![Element::Image(ImageElement {
                brightness: None,
                contrast: None,
                fill_mode: None,
                tooltip: None,
                media: None,
                src: img.to_str().unwrap().to_string(),
                position: Position {
                    x: 1.0,
                    y: 1.0,
                    w: 3.0,
                    h: 2.0,
                },
                ..Default::default()
            })],
            notes: None,
        }],
    };
    json2pptx::generate(&pres, pptx.to_str().unwrap()).unwrap();

    let out = temp_dir("localized");
    json2pptx::unpack(pptx.to_str().unwrap(), out.to_str().unwrap()).unwrap();
    let slide1: Slide =
        serde_json::from_str(&std::fs::read_to_string(out.join("ppt/slides/slide1.json")).unwrap())
            .unwrap();
    match &slide1.elements[0] {
        Element::Image(img_el) => {
            assert!(
                img_el.src.starts_with("ppt/media/"),
                "图片应本地化为包内路径，实际: {}",
                img_el.src
            );
        }
        _ => panic!("预期图片元素"),
    }
}

#[test]
fn test_full_roundtrip_unpack_repack() {
    let img = common::create_dummy_image();
    let pptx1 = common::temp_pptx_path();
    let pptx2 = common::temp_pptx_path();
    let pres = build_pres_with_images(img.to_str().unwrap());
    json2pptx::generate(&pres, pptx1.to_str().unwrap()).unwrap();

    // unpack → repack → parse
    let out = temp_dir("roundtrip");
    json2pptx::unpack(pptx1.to_str().unwrap(), out.to_str().unwrap()).unwrap();
    json2pptx::repack(out.to_str().unwrap(), pptx2.to_str().unwrap()).unwrap();
    let reparsed = json2pptx::parse(pptx2.to_str().unwrap()).unwrap();

    // 幻灯片数量与文本内容一致
    assert_eq!(reparsed.slides.len(), 2);
    assert_eq!(
        reparsed.meta.as_ref().and_then(|m| m.title.as_deref()),
        Some("Unpack Test")
    );
    assert_eq!(reparsed.slides[0].elements.len(), 2);
    assert_eq!(reparsed.slides[1].elements.len(), 1);

    // 背景图还原（重建后 parse 输出标准相对路径）
    let bg = reparsed.slides[0].background.as_ref().unwrap();
    assert_eq!(bg["type"], "image");
    assert!(bg["src"].as_str().unwrap().starts_with("../media/"));

    // 重建的 PPTX 中媒体字节一致
    let mut archive = zip::ZipArchive::new(std::fs::File::open(&pptx2).unwrap()).unwrap();
    let mut media_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let name = archive.by_index(i).unwrap().name().to_string();
        if name.starts_with("ppt/media/") {
            media_names.push(name);
        }
    }
    assert_eq!(media_names.len(), 1, "重建 PPTX 应包含 1 个媒体文件");
    let mut bytes = Vec::new();
    archive
        .by_name(&media_names[0])
        .unwrap()
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes, std::fs::read(&img).unwrap(), "媒体字节应一致");
}
