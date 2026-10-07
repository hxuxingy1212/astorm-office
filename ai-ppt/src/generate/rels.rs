//! 关系文件(.rels)生成模块
//!
//! 生成 OOXML 包中各部件之间的关系文件（.rels）。
//! 关系文件定义了 PPTX 中各 XML 文件之间的引用链接。

use crate::model::elements::*;
use crate::model::Slide;
use crate::utils::constants::rel_types;
use crate::utils::xml::esc_xml;
use std::collections::HashMap;

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// 生成根关系文件 _rels/.rels
pub fn generate_root() -> String {
    format!(
        "{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
         <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"ppt/presentation.xml\"/>\n\
         <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\n\
         <Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/>\n\
         </Relationships>",
        XML_DECL,
    )
}

/// 生成 ppt/_rels/presentation.xml.rels
///
/// rId 布局：rId1=slideMaster，rId2..=slides，随后 notesMaster 与各 notesSlide，
/// 最后 theme/presProps/viewProps/tableStyles（与 presentation.rs 的引用保持一致）。
pub fn generate_presentation_rels(slides: &[Slide]) -> String {
    let slide_count = slides.len() as u32;
    let mut xml = format!("{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n", XML_DECL);
    xml.push_str(&format!(
        "<Relationship Id=\"rId1\" Type=\"{}\" Target=\"slideMasters/slideMaster1.xml\"/>\n",
        rel_types::SLIDE_MASTER
    ));
    for i in 0..slide_count {
        xml.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"{}\" Target=\"slides/slide{}.xml\"/>\n",
            i + 2,
            rel_types::SLIDE,
            i + 1
        ));
    }
    let mut rid = slide_count + 2;
    // notesMaster（固定生成一份）
    xml.push_str(&format!(
        "<Relationship Id=\"rId{}\" Type=\"{}\" Target=\"notesMasters/notesMaster1.xml\"/>\n",
        rid,
        rel_types::NOTES_MASTER
    ));
    rid += 1;
    // 每张有备注的幻灯片对应一个 notesSlide
    for (i, slide) in slides.iter().enumerate() {
        if slide.notes.is_some() {
            xml.push_str(&format!(
                "<Relationship Id=\"rId{}\" Type=\"{}\" Target=\"notesSlides/notesSlide{}.xml\"/>\n",
                rid,
                rel_types::NOTES_SLIDE,
                i + 1
            ));
            rid += 1;
        }
    }
    xml.push_str(&format!(
        "<Relationship Id=\"rId{}\" Type=\"{}\" Target=\"theme/theme1.xml\"/>\n",
        rid,
        rel_types::THEME
    ));
    xml.push_str(&format!(
        "<Relationship Id=\"rId{}\" Type=\"{}\" Target=\"presProps.xml\"/>\n",
        rid + 1,
        rel_types::PRES_PROPS
    ));
    xml.push_str(&format!(
        "<Relationship Id=\"rId{}\" Type=\"{}\" Target=\"viewProps.xml\"/>\n",
        rid + 2,
        rel_types::VIEW_PROPS
    ));
    xml.push_str(&format!(
        "<Relationship Id=\"rId{}\" Type=\"{}\" Target=\"tableStyles.xml\"/>\n",
        rid + 3,
        rel_types::TABLE_STYLES
    ));
    xml.push_str("</Relationships>");
    xml
}

/// 生成单张幻灯片的 rels 文件
///
/// 返回 (rels_xml, 图片 src → rId 映射, 超链接 URL → rId 映射)
#[allow(clippy::type_complexity)] // 返回三张关系表组合，拆类型别名收益低
pub fn generate_slide_rels(
    slide: &Slide,
    image_ids: &[(String, String)],
    chart_targets: &[String],
    comment_target: Option<&str>,
    media_ids: &[(String, String, bool)],
    opaque_rels: &[Vec<OpaqueRel>],
) -> (
    String,
    HashMap<String, String>,
    HashMap<String, String>,
    Vec<String>,
    HashMap<String, String>,
    Vec<Vec<(String, String)>>,
) {
    let mut r_id_map = HashMap::new();
    let mut hyperlink_map = HashMap::new();
    let mut chart_rids = Vec::new();
    let mut media_map = HashMap::new();
    let mut opaque_maps: Vec<Vec<(String, String)>> = Vec::new();
    let mut xml = format!("{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n", XML_DECL);
    xml.push_str(&format!(
        "<Relationship Id=\"rId1\" Type=\"{}\" Target=\"../slideLayouts/slideLayout1.xml\"/>\n",
        rel_types::SLIDE_LAYOUT
    ));

    let mut r_id_counter = 2u32;
    for (img_src, media_path) in image_ids {
        let rid = format!("rId{}", r_id_counter);
        r_id_map.insert(img_src.clone(), rid.clone());
        xml.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"../media/{}\"/>\n",
            rid,
            rel_types::IMAGE,
            media_path
        ));
        r_id_counter += 1;
    }

    // 图表关系：Target 指向 ../charts/chartN.xml
    for target in chart_targets {
        let rid = format!("rId{}", r_id_counter);
        chart_rids.push(rid.clone());
        xml.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"/>\n",
            rid,
            rel_types::CHART,
            target
        ));
        r_id_counter += 1;
    }

    // 视频/音频关系
    for (src, media_path, is_video) in media_ids {
        let rid = format!("rId{}", r_id_counter);
        media_map.insert(src.clone(), rid.clone());
        xml.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"../media/{}\"/>\n",
            rid,
            if *is_video {
                rel_types::VIDEO
            } else {
                rel_types::AUDIO
            },
            media_path
        ));
        r_id_counter += 1;
    }

    // 批注关系
    if let Some(target) = comment_target {
        xml.push_str(&format!(
            "<Relationship Id=\"rId{}\" Type=\"{}\" Target=\"{}\"/>\n",
            r_id_counter,
            rel_types::COMMENT,
            target
        ));
        r_id_counter += 1;
    }

    // 不透明对象关系（OLE/SmartArt/3D/zoom）：按元素顺序分配新 rId
    for rels in opaque_rels {
        let mut m = Vec::new();
        for rel in rels {
            let rid = format!("rId{}", r_id_counter);
            r_id_counter += 1;
            m.push((rel.old_id.clone(), rid.clone()));
            let target_mode = if rel.external {
                " TargetMode=\"External\""
            } else {
                ""
            };
            xml.push_str(&format!(
                "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{}/>\n",
                rid,
                rel.rel_type,
                esc_xml(&rel.target),
                target_mode
            ));
        }
        opaque_maps.push(m);
    }

    collect_slide_hyperlinks(slide, &mut r_id_counter, &mut xml, &mut hyperlink_map);

    xml.push_str("</Relationships>");
    (
        xml,
        r_id_map,
        hyperlink_map,
        chart_rids,
        media_map,
        opaque_maps,
    )
}

/// 收集幻灯片内全部超链接关系（图片 + 文本 run），URL 去重
fn collect_slide_hyperlinks(
    slide: &Slide,
    r_id_counter: &mut u32,
    xml: &mut String,
    hyperlink_map: &mut HashMap<String, String>,
) {
    for el in &slide.elements {
        collect_element_hyperlinks(el, r_id_counter, xml, hyperlink_map);
    }
}

fn collect_element_hyperlinks(
    el: &Element,
    r_id_counter: &mut u32,
    xml: &mut String,
    hyperlink_map: &mut HashMap<String, String>,
) {
    match el {
        Element::Image(img) => {
            if let Some(href) = &img.hyperlink {
                add_hyperlink(href, r_id_counter, xml, hyperlink_map);
            }
        }
        Element::Text(t) => collect_text_hyperlinks(&t.text, r_id_counter, xml, hyperlink_map),
        Element::Shape(s) => {
            if let Some(text) = &s.text {
                collect_text_hyperlinks(text, r_id_counter, xml, hyperlink_map);
            }
        }
        Element::Group(grp) => {
            for child in &grp.children {
                collect_element_hyperlinks(child, r_id_counter, xml, hyperlink_map);
            }
        }
        _ => {}
    }
}

/// 收集段落 run 中的超链接
fn collect_text_hyperlinks(
    tc: &TextContent,
    r_id_counter: &mut u32,
    xml: &mut String,
    hyperlink_map: &mut HashMap<String, String>,
) {
    if let TextContent::Paragraphs(paras) = tc {
        for para in paras {
            if let Some(runs) = &para.runs {
                for run in runs {
                    if let Some(href) = &run.hyperlink {
                        add_hyperlink(href, r_id_counter, xml, hyperlink_map);
                    }
                }
            }
        }
    }
}

/// 登记一条外部超链接关系（相同 URL 复用同一 rId）
fn add_hyperlink(
    href: &str,
    r_id_counter: &mut u32,
    xml: &mut String,
    hyperlink_map: &mut HashMap<String, String>,
) {
    if hyperlink_map.contains_key(href) {
        return;
    }
    let rid = format!("rId{}", r_id_counter);
    *r_id_counter += 1;
    hyperlink_map.insert(href.to_string(), rid.clone());
    xml.push_str(&format!(
        "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\" TargetMode=\"External\"/>\n",
        rid,
        rel_types::HYPERLINK,
        esc_xml(href)
    ));
}

/// 生成 ppt/notesMasters/_rels/notesMaster1.xml.rels
pub fn generate_notes_master_rels() -> String {
    format!(
        "{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
         <Relationship Id=\"rId1\" Type=\"{}\" Target=\"../theme/theme1.xml\"/>\n\
         </Relationships>",
        XML_DECL,
        rel_types::THEME
    )
}

/// 生成 ppt/notesSlides/_rels/notesSlideN.xml.rels
///
/// 备注幻灯片引用备注母版与对应的幻灯片。
pub fn generate_notes_slide_rels(slide_idx: usize) -> String {
    format!(
        "{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
         <Relationship Id=\"rId1\" Type=\"{}\" Target=\"../notesMasters/notesMaster1.xml\"/>\n\
         <Relationship Id=\"rId2\" Type=\"{}\" Target=\"../slides/slide{}.xml\"/>\n\
         </Relationships>",
        XML_DECL,
        rel_types::NOTES_MASTER,
        rel_types::SLIDE,
        slide_idx + 1
    )
}

pub fn generate_slide_master_rels() -> String {
    format!(
        "{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
         <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/>\n\
         <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"../theme/theme1.xml\"/>\n\
         </Relationships>",
        XML_DECL,
    )
}

pub fn generate_slide_layout_rels() -> String {
    format!(
        "{}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
         <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"../slideMasters/slideMaster1.xml\"/>\n\
         </Relationships>",
        XML_DECL,
    )
}

fn get_bg_image_src(slide: &Slide) -> Option<String> {
    match &slide.background {
        Some(serde_json::Value::Object(obj)) => {
            if obj.get("type").and_then(|v| v.as_str()) == Some("image") {
                obj.get("src")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

pub fn collect_image_sources(slide: &Slide, images: &mut Vec<String>) {
    for el in &slide.elements {
        collect_image_from_element(el, images);
    }
    if let Some(bg_src) = get_bg_image_src(slide) {
        if !images.contains(&bg_src) {
            images.push(bg_src);
        }
    }
}

fn push_unique(images: &mut Vec<String>, src: &str) {
    if !images.contains(&src.to_string()) {
        images.push(src.to_string());
    }
}

fn collect_image_from_element(el: &Element, images: &mut Vec<String>) {
    match el {
        Element::Image(img) => push_unique(images, &img.src),
        Element::Shape(s) => {
            if let Some(Fill::Image { src }) = &s.fill {
                push_unique(images, src);
            }
        }
        Element::Text(t) => {
            if let Some(Fill::Image { src }) = &t.fill {
                push_unique(images, src);
            }
        }
        Element::Table(tbl) => {
            for row in &tbl.rows {
                for cell in row {
                    if let Some(src) = &cell.fill_image {
                        push_unique(images, src);
                    }
                }
            }
        }
        Element::Group(grp) => {
            for child in &grp.children {
                collect_image_from_element(child, images);
            }
        }
        _ => {}
    }
}

pub fn get_slide_images(slide: &Slide) -> Vec<String> {
    let mut result = Vec::new();
    for el in &slide.elements {
        collect_image_from_element(el, &mut result);
    }
    if let Some(bg_src) = get_bg_image_src(slide) {
        if !result.contains(&bg_src) {
            result.push(bg_src);
        }
    }
    result
}
