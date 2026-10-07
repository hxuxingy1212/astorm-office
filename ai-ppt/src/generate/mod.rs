//! PPTX 生成模块
//!
//! 将 JSON 数据模型生成为符合 OOXML 标准的 .pptx 文件。
//! 工作流程：应用模板 -> 生成各 XML 部件 -> ZIP 打包。

pub mod animation;
pub mod chart;
pub mod comment;
pub mod components;
pub mod content_types;
pub mod notes;
pub mod presentation;
pub mod rels;
pub mod slide;
pub mod slide_master;
pub mod theme;

use std::io::Write;

use crate::model::template::TemplatePreset;
use crate::model::Presentation;
use crate::utils;
use zip::write::FileOptions;
use zip::ZipWriter;

/// 根据数据模型生成 PPTX 文件
///
/// # 参数
/// - `model`: 演示文稿数据模型
/// - `output_path`: 输出的 .pptx 文件路径
pub fn generate(
    model: &Presentation,
    output_path: &str,
) -> Result<GenerateResult, crate::error::Error> {
    let mut model = model.clone();
    // 应用模板预设（主题、背景）
    if let Some(template_name) = &model.template {
        if let Some(preset) = TemplatePreset::from_name(template_name) {
            if model.theme.is_none() {
                model.theme = Some(preset.to_theme());
            } else if let Some(theme) = model.theme.as_mut() {
                theme.merge(preset.to_theme());
            }
            // 为没有设置背景的幻灯片应用模板默认背景
            if let Some(bg) = preset.bg {
                for slide in &mut model.slides {
                    if slide.background.is_none() {
                        slide.background = Some(serde_json::Value::String(bg.to_string()));
                    }
                }
            }
        }
    }
    let slides = &model.slides;
    let slide_count = slides.len();

    // 收集所有图片源（包括背景图片）
    let mut all_images: Vec<String> = Vec::new();
    for slide in slides {
        rels::collect_image_sources(slide, &mut all_images);
    }

    // 收集视频/音频媒体，分配 ppt/media/mediaN.ext 名称（全局去重）
    let mut media_index_map: std::collections::HashMap<String, (String, bool)> =
        std::collections::HashMap::new();
    let mut media_counter = 0usize;
    for slide in slides {
        for (src, is_video) in slide_media(slide) {
            if !media_index_map.contains_key(&src) {
                media_counter += 1;
                let ext = media_extension(&src);
                media_index_map.insert(
                    src.clone(),
                    (format!("media{}.{}", media_counter, ext), is_video),
                );
            }
        }
    }

    // 收集真实图表元素，分配 chartN.xml 部件编号（按幻灯片/元素遍历顺序）
    let mut chart_parts: Vec<(usize, &crate::model::elements::ChartElement)> = Vec::new();
    let mut per_slide_chart_targets: Vec<Vec<String>> = Vec::new();
    let mut chart_no = 0usize;
    for slide in slides {
        let mut charts_in_slide: Vec<&crate::model::elements::ChartElement> = Vec::new();
        for el in &slide.elements {
            collect_chart_refs(el, &mut charts_in_slide);
        }
        let mut targets = Vec::new();
        for c in charts_in_slide {
            chart_no += 1;
            targets.push(format!("../charts/chart{}.xml", chart_no));
            chart_parts.push((chart_no, c));
        }
        per_slide_chart_targets.push(targets);
    }
    let chart_count = chart_no as u32;

    // 收集批注：全局作者表 + 每张幻灯片的批注部件
    let mut author_names: Vec<String> = Vec::new();
    let mut comment_parts: Vec<(usize, Vec<crate::model::elements::CommentElement>)> = Vec::new();
    let mut per_slide_comment_target: Vec<Option<String>> = Vec::new();
    let mut comment_no = 0usize;
    for slide in slides {
        let comments: Vec<crate::model::elements::CommentElement> = slide
            .elements
            .iter()
            .filter_map(|el| match el {
                crate::model::elements::Element::Comment(c) => Some(c.clone()),
                _ => None,
            })
            .collect();
        if comments.is_empty() {
            per_slide_comment_target.push(None);
            continue;
        }
        for c in &comments {
            let name = c.author.clone().unwrap_or_default();
            if !author_names.contains(&name) {
                author_names.push(name);
            }
        }
        comment_no += 1;
        per_slide_comment_target.push(Some(format!("../comments/comment{}.xml", comment_no)));
        comment_parts.push((comment_no, comments));
    }
    let comment_count = comment_no as u32;
    let author_ids: std::collections::HashMap<String, u32> = author_names
        .iter()
        .enumerate()
        .map(|(i, n)| (n.clone(), i as u32))
        .collect();

    // 收集不透明对象引用的部件（全局去重）
    let mut opaque_parts: std::collections::HashMap<String, (String, String, Option<String>)> =
        std::collections::HashMap::new();
    {
        let mut opaques: Vec<&crate::model::elements::OpaqueElement> = Vec::new();
        for slide in slides {
            for el in &slide.elements {
                collect_opaque(el, &mut opaques);
            }
        }
        for o in opaques {
            for p in &o.parts {
                opaque_parts.entry(p.path.clone()).or_insert_with(|| {
                    (p.content_type.clone(), p.data.clone(), p.rels_xml.clone())
                });
            }
        }
    }
    let opaque_overrides: Vec<(String, String)> = opaque_parts
        .iter()
        .map(|(path, (ct, _, _))| (path.clone(), ct.clone()))
        .collect();

    // 创建 ZIP 文件（.pptx 本质是 ZIP 压缩包）
    let file = std::fs::File::create(output_path)?;
    let mut zip = ZipWriter::new(file);
    let options: FileOptions<'_, ()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // [Content_Types].xml - OOXML 内容类型声明
    let notes_indexes = notes::notes_slide_indexes(slides);
    zip.start_file("[Content_Types].xml", options)?;
    write!(
        zip,
        "{}",
        content_types::generate(
            slide_count as u32,
            &all_images,
            &notes_indexes,
            chart_count,
            comment_count,
            !author_names.is_empty(),
            &opaque_overrides,
        )
    )?;

    // _rels/.rels - 根关系文件
    zip.start_file("_rels/.rels", options)?;
    write!(zip, "{}", rels::generate_root())?;

    // docProps/ - 文档属性（元信息）
    zip.start_file("docProps/core.xml", options)?;
    write!(
        zip,
        "{}",
        presentation::generate_core_props(model.meta.as_ref())
    )?;
    zip.start_file("docProps/app.xml", options)?;
    write!(
        zip,
        "{}",
        presentation::generate_app_props(model.meta.as_ref(), slide_count as u32)
    )?;

    // ppt/presentation.xml - 演示文稿主文件
    zip.start_file("ppt/presentation.xml", options)?;
    write!(zip, "{}", presentation::generate(&model))?;

    // ppt/_rels/presentation.xml.rels - 演示文稿关系文件
    zip.start_file("ppt/_rels/presentation.xml.rels", options)?;
    write!(zip, "{}", rels::generate_presentation_rels(slides))?;

    // ppt/presProps.xml, viewProps.xml, tableStyles.xml - 演示文稿属性
    zip.start_file("ppt/presProps.xml", options)?;
    write!(zip, "{}", presentation::generate_pres_props())?;
    zip.start_file("ppt/viewProps.xml", options)?;
    write!(zip, "{}", presentation::generate_view_props())?;
    zip.start_file("ppt/tableStyles.xml", options)?;
    write!(
        zip,
        "{}",
        model
            .table_styles
            .clone()
            .unwrap_or_else(presentation::generate_table_styles)
    )?;

    // ppt/theme/theme1.xml - 主题定义（颜色、字体）
    zip.start_file("ppt/theme/theme1.xml", options)?;
    write!(zip, "{}", theme::generate(model.theme.as_ref()))?;

    // ppt/slideMasters/ - 幻灯片母版
    zip.start_file("ppt/slideMasters/slideMaster1.xml", options)?;
    write!(zip, "{}", slide_master::generate_slide_master())?;
    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", options)?;
    write!(zip, "{}", rels::generate_slide_master_rels())?;

    // ppt/slideLayouts/ - 幻灯片版式
    zip.start_file("ppt/slideLayouts/slideLayout1.xml", options)?;
    write!(zip, "{}", slide_master::generate_slide_layout())?;
    zip.start_file("ppt/slideLayouts/_rels/slideLayout1.xml.rels", options)?;
    write!(zip, "{}", rels::generate_slide_layout_rels())?;

    // ppt/notesMasters/ - 备注母版（有备注的演示文稿）
    if !notes_indexes.is_empty() {
        zip.start_file("ppt/notesMasters/notesMaster1.xml", options)?;
        write!(zip, "{}", notes::generate_notes_master())?;
        zip.start_file("ppt/notesMasters/_rels/notesMaster1.xml.rels", options)?;
        write!(zip, "{}", rels::generate_notes_master_rels())?;
    }

    // 嵌入视频/音频到 ppt/media/
    for (src, (media_name, _)) in &media_index_map {
        if let Some(bytes) = utils::load_image_bytes(src) {
            zip.start_file(format!("ppt/media/{}", media_name), options)?;
            zip.write_all(&bytes)?;
        }
    }

    // 嵌入图片到 ppt/media/ 目录
    let mut image_index_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for (img_idx, img_src) in all_images.iter().enumerate() {
        if let Some(img_bytes) = utils::load_image_bytes(img_src) {
            let ext = utils::detect_image_extension(img_src, &img_bytes);
            let media_name = format!("image{}.{}", img_idx + 1, ext);
            let media_path = format!("ppt/media/{}", media_name);
            zip.start_file(&media_path, options)?;
            zip.write_all(&img_bytes)?;
            image_index_map.insert(img_src.clone(), media_name);
        }
    }

    let slide_size = (model.width, model.height);

    // ppt/slides/ - 逐张生成幻灯片
    for (i, slide) in slides.iter().enumerate() {
        let slide_images = rels::get_slide_images(slide);
        let image_ids: Vec<(String, String)> = slide_images
            .iter()
            .filter_map(|src| {
                image_index_map
                    .get(src)
                    .map(|media| (src.clone(), media.clone()))
            })
            .collect();

        let chart_targets = &per_slide_chart_targets[i];
        let comment_target = per_slide_comment_target[i].as_deref();
        let media_ids: Vec<(String, String, bool)> = slide_media(slide)
            .into_iter()
            .filter_map(|(src, is_video)| {
                media_index_map
                    .get(&src)
                    .map(|(name, _)| (src.clone(), name.clone(), is_video))
            })
            .collect();
        let mut opaque_rels: Vec<Vec<crate::model::elements::OpaqueRel>> = Vec::new();
        {
            let mut opaques: Vec<&crate::model::elements::OpaqueElement> = Vec::new();
            for el in &slide.elements {
                collect_opaque(el, &mut opaques);
            }
            for o in opaques {
                opaque_rels.push(o.rels.clone());
            }
        }
        let (rels_xml, r_id_map, hyperlink_map, chart_rids, media_map, opaque_maps) =
            rels::generate_slide_rels(
                slide,
                &image_ids,
                chart_targets,
                comment_target,
                &media_ids,
                &opaque_rels,
            );
        let slide_xml = slide::generate(
            slide,
            &r_id_map,
            &hyperlink_map,
            &chart_rids,
            &media_map,
            &opaque_maps,
            slide_size,
        );

        zip.start_file(format!("ppt/slides/slide{}.xml", i + 1), options)?;
        write!(zip, "{}", slide_xml)?;
        zip.start_file(format!("ppt/slides/_rels/slide{}.xml.rels", i + 1), options)?;
        write!(zip, "{}", rels_xml)?;

        // 演讲者备注：有备注的幻灯片生成 notesSlide 部件
        if slide.notes.is_some() {
            let notes_xml = notes::generate_notes_slide(i, slide.notes.as_deref().unwrap_or(""));
            zip.start_file(format!("ppt/notesSlides/notesSlide{}.xml", i + 1), options)?;
            write!(zip, "{}", notes_xml)?;
            zip.start_file(
                format!("ppt/notesSlides/_rels/notesSlide{}.xml.rels", i + 1),
                options,
            )?;
            write!(zip, "{}", rels::generate_notes_slide_rels(i))?;
        }
    }

    // ppt/charts/chartN.xml - 真实图表部件
    for (no, c) in &chart_parts {
        zip.start_file(format!("ppt/charts/chart{}.xml", no), options)?;
        write!(zip, "{}", chart::generate(c))?;
    }

    // 批注作者表与批注部件
    if !author_names.is_empty() {
        zip.start_file("ppt/commentAuthors.xml", options)?;
        write!(zip, "{}", comment::generate_authors(&author_names))?;
    }
    for (no, comments) in &comment_parts {
        zip.start_file(format!("ppt/comments/comment{}.xml", no), options)?;
        write!(zip, "{}", comment::generate_comments(comments, &author_ids))?;
    }

    // 不透明对象引用的部件原样写回
    {
        use base64::Engine as _;
        for (path, (_, data, rels)) in &opaque_parts {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) {
                zip.start_file(path, options)?;
                zip.write_all(&bytes)?;
            }
            if let Some(rx) = rels {
                zip.start_file(rels_part_path(path), options)?;
                write!(zip, "{}", rx)?;
            }
        }
    }

    zip.finish()?;

    Ok(GenerateResult {
        path: output_path.to_string(),
        slides: slide_count as u32,
    })
}

/// 从 unpack 产物目录重建 PPTX
///
/// 读取 `presentation.json`（slides 为路径数组）与各 `ppt/slides/slideN.json`，
/// 将图片 src（`ppt/media/...`）重写为相对产物目录的绝对路径（URL 保留）后生成。
pub fn repack(input_dir: &str, output_path: &str) -> Result<GenerateResult, crate::error::Error> {
    use crate::model::elements::Element;
    use crate::model::Slide;

    let base = std::path::Path::new(input_dir);

    // 1. 读 presentation.json（slides 为文件路径数组）
    let pres_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(base.join("presentation.json"))?)?;
    let slide_paths: Vec<String> = pres_json["slides"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 2. 逐张读 slideN.json
    let mut slides = Vec::new();
    for rel in &slide_paths {
        let slide: Slide = serde_json::from_str(&std::fs::read_to_string(base.join(rel))?)?;
        slides.push(slide);
    }

    // 3. 组装模型
    let mut model = Presentation {
        width: pres_json["width"].as_f64().unwrap_or(13.333),
        height: pres_json["height"].as_f64().unwrap_or(7.5),
        meta: pres_json
            .get("meta")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        template: pres_json
            .get("template")
            .and_then(|v| v.as_str().map(String::from)),
        theme: pres_json
            .get("theme")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        slides,
        table_styles: pres_json
            .get("table_styles")
            .and_then(|v| v.as_str().map(String::from)),
    };

    // 4. 重写图片 src 为绝对路径（相对产物目录；URL 保留）
    let abs_base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let absolutize = |s: &mut String| {
        if !s.starts_with("http://") && !s.starts_with("https://") {
            *s = abs_base.join(&*s).to_str().unwrap_or(s).to_string();
        }
    };
    for slide in &mut model.slides {
        for el in &mut slide.elements {
            match el {
                Element::Image(img) => absolutize(&mut img.src),
                Element::Shape(s) => {
                    if let Some(crate::model::elements::Fill::Image { src }) = &mut s.fill {
                        absolutize(src);
                    }
                }
                Element::Text(t) => {
                    if let Some(crate::model::elements::Fill::Image { src }) = &mut t.fill {
                        absolutize(src);
                    }
                }
                Element::Table(tbl) => {
                    for row in &mut tbl.rows {
                        for c in row.iter_mut() {
                            if let Some(s) = &mut c.fill_image {
                                absolutize(s);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(serde_json::Value::Object(ref mut bg)) = slide.background {
            if bg.get("type").and_then(|v| v.as_str()) == Some("image") {
                if let Some(src) = bg.get("src").and_then(|v| v.as_str()) {
                    if !src.starts_with("http://") && !src.starts_with("https://") {
                        let new_src = abs_base.join(src).to_str().unwrap_or(src).to_string();
                        bg.insert("src".to_string(), serde_json::Value::String(new_src));
                    }
                }
            }
        }
    }

    generate(&model, output_path)
}

/// 深度优先收集元素中的图表（与 element_to_xml 遍历顺序一致）
fn collect_chart_refs<'a>(
    el: &'a crate::model::elements::Element,
    out: &mut Vec<&'a crate::model::elements::ChartElement>,
) {
    use crate::model::elements::Element;
    match el {
        Element::Chart(c) => out.push(c),
        Element::Group(g) => {
            for child in &g.children {
                collect_chart_refs(child, out);
            }
        }
        _ => {}
    }
}

/// 深度优先收集不透明富对象（与 element_to_xml 遍历顺序一致）
fn collect_opaque<'a>(
    el: &'a crate::model::elements::Element,
    out: &mut Vec<&'a crate::model::elements::OpaqueElement>,
) {
    use crate::model::elements::Element;
    match el {
        Element::Opaque(o) => out.push(o),
        Element::Group(g) => {
            for c in &g.children {
                collect_opaque(c, out);
            }
        }
        _ => {}
    }
}

/// 部件关系文件路径：ppt/x/y.xml → ppt/x/_rels/y.xml.rels
fn rels_part_path(part_path: &str) -> String {
    let p = std::path::Path::new(part_path);
    let dir = p
        .parent()
        .map(|d| d.to_string_lossy().to_string())
        .unwrap_or_default();
    let file = p
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    format!("{}/_rels/{}.rels", dir, file)
}

/// 收集幻灯片中的媒体（视频/音频）src 及类型，去重保序
fn slide_media(slide: &crate::model::Slide) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = Vec::new();
    for el in &slide.elements {
        collect_media_srcs(el, &mut out);
    }
    out
}

fn collect_media_srcs(el: &crate::model::elements::Element, out: &mut Vec<(String, bool)>) {
    use crate::model::elements::Element;
    match el {
        Element::Image(i) => {
            if let Some(m) = &i.media {
                if !out.iter().any(|(s, _)| s == &m.src) {
                    out.push((m.src.clone(), m.kind == "video"));
                }
            }
        }
        Element::Group(g) => {
            for c in &g.children {
                collect_media_srcs(c, out);
            }
        }
        _ => {}
    }
}

/// 从路径/URL 推断媒体扩展名
fn media_extension(src: &str) -> String {
    let no_query = src.split(['?', '#']).next().unwrap_or(src);
    std::path::Path::new(no_query)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .filter(|e| {
            matches!(
                e.as_str(),
                "mp4"
                    | "m4v"
                    | "mov"
                    | "avi"
                    | "wmv"
                    | "mkv"
                    | "webm"
                    | "mp3"
                    | "m4a"
                    | "wav"
                    | "wma"
                    | "aac"
                    | "ogg"
                    | "flac"
            )
        })
        .unwrap_or_else(|| "bin".to_string())
}

/// PPTX 生成结果
pub struct GenerateResult {
    /// 输出文件路径
    pub path: String,
    /// 生成的幻灯片数量
    pub slides: u32,
}
