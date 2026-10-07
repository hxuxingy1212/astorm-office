//! PPTX 解析模块
//!
//! 将 .pptx 文件解析为 JSON 数据模型。
//! 工作流程：解压 ZIP -> 解析 XML -> 构建数据模型。

pub mod animation;
pub mod background;
pub mod chart;
pub mod comment;
pub mod grp_sp;
pub mod inherit;
pub mod pic;
pub mod slide;
pub mod table;
pub mod theme;

use crate::model::elements::Position;
use crate::model::Meta;
use crate::model::Presentation;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

/// PPTX 解包结果
pub struct UnpackResult {
    /// 输出目录路径
    pub path: String,
    /// 幻灯片数量
    pub slides: u32,
}

/// 将 PPTX 解包为可编辑中间产物（包镜像结构）
///
/// 产物结构：
/// ```text
/// out/
/// ├── presentation.json       # 顶层结构（meta/theme/尺寸），slides 为文件路径数组
/// └── ppt/
///     ├── slides/slideN.json  # 每张幻灯片一个 JSON（可直接编辑）
///     └── media/imageN.*      # 媒体文件实体（保留包内路径）
/// ```
///
/// 图片 src 统一改写为 `ppt/media/...`（相对产物根），URL 原样保留。
pub fn unpack(input_path: &str, output_dir: &str) -> Result<UnpackResult, crate::error::Error> {
    // 旧版 .ppt（OLE2/CFB）：自动转换成 .pptx 后再解包（媒体一并来自转换产物）
    if crate::legacy::is_cfb(std::path::Path::new(input_path)) {
        let tmp = std::env::temp_dir().join(format!(
            "json2pptx-legacy-unpack-{}-{}.pptx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        crate::legacy::ppt_to_pptx(std::path::Path::new(input_path), &tmp)?;
        let r = unpack(&tmp.to_string_lossy(), output_dir);
        let _ = std::fs::remove_file(&tmp);
        return r;
    }
    let mut model = parse(input_path)?;
    let out_dir = Path::new(output_dir);
    std::fs::create_dir_all(out_dir.join("ppt/slides"))?;

    // 1. 提取 ppt/media/* 到 out/ppt/media/（保留包内路径）
    let file = std::fs::File::open(input_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        // 跳过目录条目（旧版 Office/WPS 会在 ZIP 中写 0 字节目录项）
        if name.ends_with('/') {
            continue;
        }
        if name.starts_with("ppt/media/") {
            let target_path = out_dir.join(&name);
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            std::fs::write(&target_path, &bytes)?;
        }
    }

    // 2. 改写图片 src：../media/xxx → ppt/media/xxx（URL 保留）
    let to_package_path = |src: &str| -> String {
        if src.starts_with("http://") || src.starts_with("https://") {
            return src.to_string();
        }
        // 归一化：去掉前导 '/' 与任意数量的 "../"
        let mut s = src.trim_start_matches('/');
        while let Some(rest) = s.strip_prefix("../") {
            s = rest;
        }
        if s.starts_with("ppt/") {
            s.to_string()
        } else {
            format!("ppt/{}", s)
        }
    };
    for slide in &mut model.slides {
        for el in &mut slide.elements {
            match el {
                crate::model::elements::Element::Image(img) => {
                    img.src = to_package_path(&img.src);
                }
                crate::model::elements::Element::Shape(s) => {
                    if let Some(crate::model::elements::Fill::Image { src }) = &mut s.fill {
                        *src = to_package_path(src);
                    }
                }
                crate::model::elements::Element::Text(t) => {
                    if let Some(crate::model::elements::Fill::Image { src }) = &mut t.fill {
                        *src = to_package_path(src);
                    }
                }
                crate::model::elements::Element::Table(tbl) => {
                    for row in &mut tbl.rows {
                        for c in row.iter_mut() {
                            if let Some(s) = &mut c.fill_image {
                                *s = to_package_path(s);
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
                    bg.insert(
                        "src".to_string(),
                        serde_json::Value::String(to_package_path(src)),
                    );
                }
            }
        }
    }

    // 3. 逐张写 slideN.json
    let mut slide_paths = Vec::new();
    for (i, slide) in model.slides.iter().enumerate() {
        let rel = format!("ppt/slides/slide{}.json", i + 1);
        let json = serde_json::to_string_pretty(slide)?;
        std::fs::write(out_dir.join(&rel), json)?;
        slide_paths.push(rel);
    }

    // 4. 写 presentation.json（slides 为文件路径数组）
    let mut pres = serde_json::to_value(&model)?;
    pres["slides"] = serde_json::Value::Array(
        slide_paths
            .iter()
            .map(|p| serde_json::Value::String(p.clone()))
            .collect(),
    );
    let pres_json = serde_json::to_string_pretty(&pres)?;
    std::fs::write(out_dir.join("presentation.json"), pres_json)?;

    Ok(UnpackResult {
        path: output_dir.to_string(),
        slides: model.slides.len() as u32,
    })
}

/// 提取 PPTX 中的媒体文件到指定目录
///
/// 将 ppt/media/ 中的图片提取到输出目录，
/// 并更新模型中图片路径为本地绝对路径。
pub fn extract_media(
    model: &mut Presentation,
    pptx_path: &str,
    output_dir: &str,
) -> Result<(), crate::error::Error> {
    let file = std::fs::File::open(pptx_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let out_dir = Path::new(output_dir);

    // 提取 ppt/media/ 目录下的所有文件
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        // 跳过目录条目
        if name.ends_with('/') {
            continue;
        }
        if name.starts_with("ppt/media/") {
            let rel_path = name.strip_prefix("ppt/").unwrap_or(&name);
            let target_path = out_dir.join(rel_path);
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes)?;
            std::fs::write(&target_path, &bytes)?;
        }
    }

    let abs_base = out_dir
        .canonicalize()
        .unwrap_or_else(|_| out_dir.to_path_buf());

    // 更新模型中图片元素的路径为绝对路径
    for slide in &mut model.slides {
        for el in &mut slide.elements {
            if let crate::model::elements::Element::Image(ref mut img) = el {
                if !img.src.starts_with("http://") && !img.src.starts_with("https://") {
                    let clean = normalize_part_path("", &img.src);
                    let new_src = abs_base.join(clean);
                    img.src = new_src.to_str().unwrap_or(&img.src).to_string();
                }
            }
        }
        // 更新背景图片路径
        if let Some(serde_json::Value::Object(ref mut bg)) = slide.background {
            if bg.get("type").and_then(|v| v.as_str()) == Some("image") {
                if let Some(src) = bg.get("src").and_then(|v| v.as_str()) {
                    if !src.starts_with("http://") && !src.starts_with("https://") {
                        let clean = normalize_part_path("", src);
                        let new_src = abs_base.join(clean);
                        bg.insert(
                            "src".to_string(),
                            serde_json::Value::String(new_src.to_str().unwrap_or(src).to_string()),
                        );
                    }
                }
            }
        }
    }

    Ok(())
}

/// 同时适用于开始/结束标签的“原始限定名”提供者
pub(crate) trait HasRawName {
    fn raw_name(&self) -> Vec<u8>;
}
impl HasRawName for quick_xml::events::BytesStart<'_> {
    fn raw_name(&self) -> Vec<u8> {
        self.name().as_ref().to_vec()
    }
}
impl HasRawName for quick_xml::events::BytesEnd<'_> {
    fn raw_name(&self) -> Vec<u8> {
        self.name().as_ref().to_vec()
    }
}

/// 取限定名的局部名并映射回规范前缀（a/p）
pub(crate) fn tag_name(raw: &[u8]) -> String {
    let local = match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    };
    let local = String::from_utf8_lossy(local);
    match &*local {
        // drawingml
        "alpha" | "avLst" | "blip" | "blipFill" | "bodyPr" | "br" | "buChar"
        | "clrScheme" | "ea" | "effectLst" | "ext" | "fontScheme" | "gd" | "gradFill"
        | "gridCol" | "gs" | "gsLst" | "hMerge" | "headEnd" | "hlinkClick" | "latin"
        | "lin" | "ln" | "lnSpc" | "lnTo" | "majorFont" | "minorFont" | "moveTo"
        | "noFill" | "off" | "outerShdw" | "p" | "pPr" | "prstDash" | "prstGeom"
        | "pt" | "quadBezTo" | "r" | "rPr" | "schemeClr" | "solidFill" | "spcPct"
        | "srcRect" | "srgbClr" | "t" | "tailEnd" | "tbl" | "tblGrid" | "tblPr"
        | "blur" | "fillOverlay" | "glow" | "innerShdw" | "softEdge" | "reflection"
        | "pattFill" | "fgClr" | "bgClr" | "highlight" | "uLn" | "lum" | "tile"
        | "stretch" | "tblStyle" | "lnL" | "lnR" | "lnT" | "lnB" | "videoFile"
        | "audioFile" | "lumMod" | "lumOff" | "tint" | "shade" | "alphaMod"
        | "alphaOff" | "normAutofit" | "spAutoFit" | "noAutofit" | "spcPts"
        | "buNone" | "buAutoNum"
        | "tc" | "tcPr" | "tr" | "vMerge" | "xfrm"
        // 主题配色方案的条目名（clrScheme 的子元素）
        | "dk1" | "lt1" | "dk2" | "lt2" | "accent1" | "accent2" | "accent3"
        | "accent4" | "accent5" | "accent6" | "hlink" | "folHlink" | "sysClr"
        | "prstClr" => format!("a:{}", local),
        // presentationml
        "animClr" | "animEffect" | "animMotion" | "animRot" | "animScale" | "bg"
        | "bgPr" | "cBhvr" | "cNvPr" | "cNvSpPr" | "cTn" | "childTnLst" | "cond"
        | "cxnSp" | "fltVal" | "graphicFrame" | "grpSp" | "grpSpPr" | "notesId"
        | "nvCxnSpPr" | "nvSpPr" | "par" | "ph" | "pic" | "seq" | "set" | "sldId" | "sldSz"
        | "sp" | "spPr" | "spTgt" | "spTree" | "stCondLst" | "tgtEl" | "timing"
        | "to" | "transition" | "txBody" => format!("p:{}", local),
        // Dublin Core（docProps/core.xml）
        "title" | "creator" => format!("dc:{}", local),
        _ => local.to_string(),
    }
}

/// 取元素标签名（命名空间无关），开始/结束标签通用
pub(crate) fn tag<T: HasRawName>(e: &T) -> String {
    tag_name(&e.raw_name())
}

/// 完整关系信息（类型/目标/是否外部）
struct RelFull {
    target: String,
    rel_type: String,
    external: bool,
}

/// 解析 .rels：Id → RelFull
fn parse_rels_full(xml: &str) -> HashMap<String, RelFull> {
    let mut map = HashMap::new();
    if xml.is_empty() {
        return map;
    }
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e))
                if (String::from_utf8_lossy(e.name().as_ref()) == "Relationship"
                    || e.local_name().as_ref() == b"Relationship") =>
            {
                let mut id = String::new();
                let mut target = String::new();
                let mut rel_type = String::new();
                let mut external = false;
                for a in e.attributes().flatten() {
                    let v = a
                        .unescape_value()
                        .unwrap_or(std::borrow::Cow::Borrowed(""))
                        .to_string();
                    match a.key.as_ref() {
                        b"Id" => id = v,
                        b"Target" => target = v,
                        b"Type" => rel_type = v,
                        b"TargetMode" => external = v == "External",
                        _ => {}
                    }
                }
                if !id.is_empty() && !target.is_empty() {
                    map.insert(
                        id,
                        RelFull {
                            target,
                            rel_type,
                            external,
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    map
}

/// 从元素 XML 中收集以 r: 开头且值为 rIdN 的属性值
fn referenced_rel_ids(xml: &str) -> Vec<String> {
    let mut ids = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                for a in e.attributes().flatten() {
                    if a.key.as_ref().starts_with(b"r:") {
                        let v = String::from_utf8_lossy(&a.value).to_string();
                        if v.starts_with("rId") && !ids.contains(&v) {
                            ids.push(v);
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    ids
}

/// 解析 [Content_Types].xml → (扩展名默认类型, 部件覆盖类型)
fn parse_content_types(xml: &str) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut defaults = HashMap::new();
    let mut overrides = HashMap::new();
    if xml.is_empty() {
        return (defaults, overrides);
    }
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name().as_ref().to_vec();
                let mut ext = String::new();
                let mut part = String::new();
                let mut ct = String::new();
                for a in e.attributes().flatten() {
                    let v = String::from_utf8_lossy(&a.value).to_string();
                    match a.key.as_ref() {
                        b"Extension" => ext = v,
                        b"PartName" => part = v,
                        b"ContentType" => ct = v,
                        _ => {}
                    }
                }
                if local == b"Default" && !ext.is_empty() {
                    defaults.insert(ext.to_lowercase(), ct);
                } else if local == b"Override" && !part.is_empty() {
                    overrides.insert(part, ct);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    (defaults, overrides)
}

fn content_type_for(
    path: &str,
    defaults: &HashMap<String, String>,
    overrides: &HashMap<String, String>,
) -> String {
    if let Some(ct) = overrides.get(&format!("/{}", path)) {
        return ct.clone();
    }
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    defaults
        .get(&ext)
        .cloned()
        .unwrap_or_else(|| "application/octet-stream".to_string())
}

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

/// 解析不透明元素引用的关系与部件（含传递闭包）
fn resolve_opaque<R: std::io::Read + std::io::Seek>(
    elements: &mut [crate::model::elements::Element],
    slide_rels: &HashMap<String, RelFull>,
    defaults: &HashMap<String, String>,
    overrides: &HashMap<String, String>,
    archive: &mut zip::ZipArchive<R>,
) {
    use base64::Engine as _;
    for el in elements.iter_mut() {
        let opaque = match el {
            crate::model::elements::Element::Opaque(o) => o,
            crate::model::elements::Element::Group(g) => {
                resolve_opaque(&mut g.children, slide_rels, defaults, overrides, archive);
                continue;
            }
            _ => continue,
        };
        let mut rels = Vec::new();
        let mut queue: Vec<String> = Vec::new();
        // 候选：XML 中引用的 rId + 所有 diagram 相关关系（含 XML 未引用的预渲染 drawing）
        let mut candidate_ids = referenced_rel_ids(&opaque.xml);
        for (id, r) in slide_rels.iter() {
            if r.rel_type.contains("/diagram") && !candidate_ids.contains(id) {
                candidate_ids.push(id.clone());
            }
        }
        for old_id in candidate_ids {
            if let Some(r) = slide_rels.get(&old_id) {
                rels.push(crate::model::elements::OpaqueRel {
                    old_id: old_id.clone(),
                    rel_type: r.rel_type.clone(),
                    target: r.target.clone(),
                    external: r.external,
                });
                if !r.external {
                    let path = normalize_part_path("ppt/slides/", &r.target);
                    if !queue.contains(&path) {
                        queue.push(path);
                    }
                }
            }
        }
        // 传递闭包读取部件
        let mut parts = Vec::new();
        let mut seen = std::collections::HashSet::new();
        while let Some(path) = queue.pop() {
            if !seen.insert(path.clone()) {
                continue;
            }
            let mut bytes = Vec::new();
            if let Ok(mut f) = archive.by_name(&path) {
                if f.read_to_end(&mut bytes).is_err() {
                    continue;
                }
            } else {
                continue;
            }
            let ct = content_type_for(&path, defaults, overrides);
            let rels_path = rels_part_path(&path);
            let mut part_rels_xml = None;
            if let Ok(mut rf) = archive.by_name(&rels_path) {
                let mut s = String::new();
                if rf.read_to_string(&mut s).is_ok() {
                    // 继续向上收集子部件
                    let dir = std::path::Path::new(&path)
                        .parent()
                        .map(|d| d.to_string_lossy().to_string())
                        .unwrap_or_default();
                    for sub in parse_rels_full(&s).values() {
                        if !sub.external {
                            let sub_path = normalize_part_path(&dir, &sub.target);
                            if !seen.contains(&sub_path) {
                                queue.push(sub_path);
                            }
                        }
                    }
                    part_rels_xml = Some(s);
                }
            }
            parts.push(crate::model::elements::OpaquePart {
                path,
                content_type: ct,
                data: base64::engine::general_purpose::STANDARD.encode(&bytes),
                rels_xml: part_rels_xml,
            });
        }
        opaque.rels = rels;
        opaque.parts = parts;
    }
}

/// 从元素 XML 中提取全部文本（a:t / m:t），用于查询与预览
pub(crate) fn extract_all_text(xml: &str) -> Option<String> {
    let mut out = String::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut in_text = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if local == "t" {
                    in_text = true;
                }
            }
            Ok(Event::Empty(_)) => {}
            Ok(Event::Text(ref t)) if in_text => {
                if let Ok(txt) = t.unescape() {
                    out.push_str(&txt);
                }
            }
            Ok(Event::End(ref e)) if e.local_name().as_ref() == b"t" => {
                in_text = false;
                out.push(' ');
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    let trimmed = out.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// 解析 .pptx 文件为 Presentation 数据模型
/// 从 p:cNvPr 节点属性中解析元素名称
pub(crate) fn parse_name_attr(e: &quick_xml::events::BytesStart) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.as_ref() == b"name" {
            Some(String::from_utf8_lossy(&a.value).to_string())
        } else {
            None
        }
    })
}

/// 从 p:cNvPr 节点属性中解析稳定 ID（@id 寻址用）
pub(crate) fn parse_id_attr(e: &quick_xml::events::BytesStart) -> Option<u32> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == b"id")
        .and_then(|a| String::from_utf8_lossy(&a.value).parse::<u32>().ok())
        .filter(|v| *v > 0)
}

/// 从 a:off / a:ext 节点属性中解析位置（英寸），写入 position
pub(crate) fn parse_pos_attrs(e: &quick_xml::events::BytesStart, position: &mut Position) {
    for a in e.attributes().flatten() {
        let k = a.key.as_ref();
        let Ok(v) = String::from_utf8_lossy(&a.value).parse::<i64>() else {
            continue;
        };
        match k {
            b"x" => position.x = crate::utils::constants::emu_to_inch(v),
            b"y" => position.y = crate::utils::constants::emu_to_inch(v),
            b"cx" => position.w = crate::utils::constants::emu_to_inch(v),
            b"cy" => position.h = crate::utils::constants::emu_to_inch(v),
            _ => {}
        }
    }
}

pub fn parse(input_path: &str) -> Result<Presentation, crate::error::Error> {
    // 旧版 .ppt（OLE2/CFB）自动识别：先转 .pptx（写入临时文件）再解析
    if crate::legacy::is_cfb(std::path::Path::new(input_path)) {
        let tmp = std::env::temp_dir().join(format!(
            "json2pptx-legacy-{}-{}.pptx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        crate::legacy::ppt_to_pptx(std::path::Path::new(input_path), &tmp)?;
        let pres = parse(&tmp.to_string_lossy())?;
        let _ = std::fs::remove_file(&tmp);
        return Ok(pres);
    }

    let path = Path::new(input_path);
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)?;

    // 读取 presentation.xml 获取幻灯片尺寸
    let mut pres_xml = String::new();
    archive
        .by_name("ppt/presentation.xml")?
        .read_to_string(&mut pres_xml)?;
    let (width, height) = parse_presentation_size(&pres_xml)?;

    // 读取文档元信息
    let mut core_xml = String::new();
    if let Ok(mut f) = archive.by_name("docProps/core.xml") {
        f.read_to_string(&mut core_xml)?;
    }
    let mut meta = parse_core_xml(&core_xml);
    // 扩展属性（docProps/app.xml）
    let mut app_xml = String::new();
    if let Ok(mut f) = archive.by_name("docProps/app.xml") {
        f.read_to_string(&mut app_xml)?;
    }
    parse_app_xml(&app_xml, &mut meta);

    // 读取主题（theme1 缺失时回退到任意 themeN）
    let mut theme_xml = String::new();
    let mut theme_name = "ppt/theme/theme1.xml".to_string();
    if archive.by_name("ppt/theme/theme1.xml").is_err() {
        for i in 0..archive.len() {
            if let Ok(entry) = archive.by_index(i) {
                let n = entry.name().to_string();
                if n.starts_with("ppt/theme/") && n.ends_with(".xml") && !n.contains(".rels") {
                    theme_name = n;
                    break;
                }
            }
        }
    }
    if let Ok(mut f) = archive.by_name(&theme_name) {
        f.read_to_string(&mut theme_xml)?;
    }
    let theme_val = theme::parse(&theme_xml);

    // 预提取所有图表部件（path → xml），供幻灯片解析时按关系引用
    let mut charts: HashMap<String, String> = HashMap::new();
    for i in 0..archive.len() {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        // 图表部件可能位于 ppt/charts/ 或 ppt/slides/charts/ 等任意 charts 目录
        if name.starts_with("ppt/")
            && name.contains("/charts/")
            && name.ends_with(".xml")
            && !name.contains(".rels")
        {
            drop(entry);
            let mut cxml = String::new();
            archive.by_name(&name)?.read_to_string(&mut cxml)?;
            charts.insert(name, cxml);
        }
    }

    // 内容类型表（供不透明部件推导类型）
    let mut ct_xml = String::new();
    if let Ok(mut f) = archive.by_name("[Content_Types].xml") {
        f.read_to_string(&mut ct_xml)?;
    }
    let (ct_defaults, ct_overrides) = parse_content_types(&ct_xml);

    // 批注作者表
    let mut authors_xml = String::new();
    if let Ok(mut f) = archive.by_name("ppt/commentAuthors.xml") {
        f.read_to_string(&mut authors_xml)?;
    }
    let comment_authors = comment::parse_authors(&authors_xml);

    // 解析母版与版式，构建占位符继承上下文（供幻灯片回填位置/字号/颜色）
    let mut master_xml = String::new();
    if let Ok(mut f) = archive.by_name("ppt/slideMasters/slideMaster1.xml") {
        f.read_to_string(&mut master_xml)?;
    }
    let master_inh = inherit::parse_master(&master_xml);
    // 版式缓存：部件路径 → 已叠加母版的继承上下文
    let mut layout_cache: HashMap<String, inherit::Inheritance> = HashMap::new();

    // 逐张解析幻灯片；ZIP 条目顺序不等于幻灯片顺序，先收集再按编号排序
    let mut collected: Vec<(usize, crate::model::Slide)> = Vec::new();
    let count = archive.len();
    for i in 0..count {
        let entry = archive.by_index(i)?;
        let name = entry.name().to_string();
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") && !name.contains(".rels")
        {
            drop(entry);
            let mut sxml = String::new();
            archive.by_name(&name)?.read_to_string(&mut sxml)?;
            let rels_name = format!(
                "ppt/slides/_rels/{}",
                Path::new(&name).file_name().unwrap().to_str().unwrap()
            );
            let mut rxml = String::new();
            if let Ok(mut f) = archive.by_name(&rels_name.replace(".xml", ".xml.rels")) {
                f.read_to_string(&mut rxml)?;
            }
            // 幻灯片 → 版式：从 rels 中找 slideLayout 关系
            let inherit_ctx = find_layout_target(&rxml).and_then(|target| {
                let lpath = normalize_part_path("ppt/slides/", &target);
                if let Some(cached) = layout_cache.get(&lpath) {
                    return Some(cached.clone());
                }
                let mut lxml = String::new();
                archive
                    .by_name(&lpath)
                    .ok()?
                    .read_to_string(&mut lxml)
                    .ok()?;
                let mut inh = inherit::parse_layout(&lxml);
                inh.overlay_master(&master_inh);
                layout_cache.insert(lpath, inh.clone());
                Some(inh)
            });
            let mut slide = slide::parse(&sxml, &rxml, inherit_ctx.as_ref(), &charts);
            // 批注：按 rels 引用加载并追加到元素列表
            if let Some(target) = find_comment_target(&rxml) {
                let cpath = normalize_part_path("ppt/slides/", &target);
                if let Ok(mut f) = archive.by_name(&cpath) {
                    let mut cxml = String::new();
                    f.read_to_string(&mut cxml)?;
                    slide.elements.extend(
                        comment::parse_comments(&cxml, &comment_authors)
                            .into_iter()
                            .map(crate::model::elements::Element::Comment),
                    );
                }
            }
            // 不透明富对象：解析引用的关系与部件
            let slide_rels_full = parse_rels_full(&rxml);
            resolve_opaque(
                &mut slide.elements,
                &slide_rels_full,
                &ct_defaults,
                &ct_overrides,
                &mut archive,
            );
            let slide_no = name
                .trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
                .parse::<usize>()
                .unwrap_or(collected.len() + 1);
            collected.push((slide_no, slide));
        }
    }
    // 按 presentation.xml 的 sldIdLst 顺序排序（回退为幻灯片文件编号）
    let mut pres_rels_xml = String::new();
    if let Ok(mut f) = archive.by_name("ppt/_rels/presentation.xml.rels") {
        f.read_to_string(&mut pres_rels_xml)?;
    }
    let pres_rels = parse_rels_map(&pres_rels_xml);
    let mut rank: HashMap<usize, usize> = HashMap::new();
    for (r, (_id, rid)) in parse_sld_id_lst(&pres_xml).iter().enumerate() {
        if let Some(target) = pres_rels.get(rid) {
            if let Some(no) = target
                .trim_start_matches('/')
                .strip_prefix("slides/slide")
                .and_then(|s| s.strip_suffix(".xml"))
                .and_then(|s| s.parse::<usize>().ok())
            {
                rank.insert(no, r);
            }
        }
    }
    collected.sort_by_key(|(no, _)| rank.get(no).copied().unwrap_or(usize::MAX / 2 + *no));
    let mut slides: Vec<crate::model::Slide> = Vec::new();
    let mut slide_indices: Vec<(usize, usize)> = Vec::new();
    for (no, s) in collected {
        slide_indices.push((slides.len(), no));
        slides.push(s);
    }

    // 演讲者备注：presentation.xml 的 notesIdLst（slideId → rId）→ presentation.xml.rels 定位 notesSlide 部件
    let notes_id_map = parse_notes_id_lst(&pres_xml);
    if !notes_id_map.is_empty() {
        let mut pres_rels_xml = String::new();
        if let Ok(mut f) = archive.by_name("ppt/_rels/presentation.xml.rels") {
            f.read_to_string(&mut pres_rels_xml)?;
        }
        let pres_rels = parse_rels_map(&pres_rels_xml);
        // sldId id → 幻灯片编号（通过 sldId 的 rId 与 rels Target 解析）
        let mut id_to_slide_no: HashMap<u32, usize> = HashMap::new();
        for (id, rid) in parse_sld_id_lst(&pres_xml) {
            if let Some(target) = pres_rels.get(&rid) {
                if let Some(no) = target
                    .trim_start_matches('/')
                    .strip_prefix("slides/slide")
                    .and_then(|s| s.strip_suffix(".xml"))
                    .and_then(|s| s.parse::<usize>().ok())
                {
                    id_to_slide_no.insert(id, no);
                }
            }
        }
        for (idx, slide_no) in &slide_indices {
            let slide_id = id_to_slide_no
                .iter()
                .find(|(_, v)| **v == *slide_no)
                .map(|(k, _)| *k);
            if let Some(sid) = slide_id {
                if let Some(rid) = notes_id_map.get(&sid) {
                    if let Some(target) = pres_rels.get(rid) {
                        let notes_path = format!("ppt/{}", target.trim_start_matches('/'));
                        if let Ok(mut f) = archive.by_name(&notes_path) {
                            let mut nxml = String::new();
                            f.read_to_string(&mut nxml)?;
                            slides[*idx].notes = parse_notes_slide_text(&nxml);
                        }
                    }
                }
            }
        }
    }

    // 表格样式表（保留内置样式/banding）
    let mut table_styles = None;
    if let Ok(mut f) = archive.by_name("ppt/tableStyles.xml") {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() && !s.trim().is_empty() {
            table_styles = Some(s);
        }
    }

    Ok(Presentation {
        width,
        height,
        meta: Some(meta),
        template: None,
        theme: Some(theme_val),
        slides,
        table_styles,
    })
}

/// 从幻灯片 rels 中查找 slideLayout 目标
fn find_layout_target(rels_xml: &str) -> Option<String> {
    parse_rels_map(rels_xml)
        .into_values()
        .find(|t| t.contains("slideLayout"))
}

/// 从幻灯片 rels 中查找批注目标（comments/commentN.xml）
fn find_comment_target(rels_xml: &str) -> Option<String> {
    parse_rels_map(rels_xml)
        .into_values()
        .find(|t| t.contains("comments/comment"))
}

/// 将关系 Target 相对路径规范化为包内路径（如 `ppt/slideLayouts/slideLayout1.xml`）
pub(crate) fn normalize_part_path(base_dir: &str, target: &str) -> String {
    if let Some(abs) = target.strip_prefix('/') {
        return abs.to_string();
    }
    let mut parts: Vec<&str> = base_dir
        .trim_end_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

/// 解析 presentation.xml 的 notesIdLst：slideId（256 起）→ 关系 rId
fn parse_notes_id_lst(xml: &str) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let name = tag(e).to_string();
                if name == "p:notesId" {
                    let mut id = 0u32;
                    let mut rid = String::new();
                    for attr in e.attributes().flatten() {
                        let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let v = String::from_utf8_lossy(&attr.value).to_string();
                        if k == "id" {
                            id = v.parse().unwrap_or(0);
                        }
                        if k == "r:id" {
                            rid = v;
                        }
                    }
                    if id > 0 && !rid.is_empty() {
                        map.insert(id, rid);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    map
}

/// 解析 .rels 文件：关系 Id → Target 映射
fn parse_rels_map(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let name = tag(e).to_string();
                if name == "Relationship" {
                    let mut id = String::new();
                    let mut target = String::new();
                    for attr in e.attributes().flatten() {
                        let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        // 属性值需反转义实体（如 URL 中的 &amp;）
                        let v = attr
                            .unescape_value()
                            .unwrap_or_else(|_| {
                                std::borrow::Cow::Borrowed(
                                    std::str::from_utf8(&attr.value).unwrap_or(""),
                                )
                            })
                            .to_string();
                        if k == "Id" {
                            id = v.clone();
                        }
                        if k == "Target" {
                            target = v;
                        }
                    }
                    if !id.is_empty() && !target.is_empty() {
                        map.insert(id, target);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    map
}

/// 解析 presentation.xml 的 sldIdLst：slideId → 关系 rId
fn parse_sld_id_lst(xml: &str) -> Vec<(u32, String)> {
    let mut list = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let name = tag(e).to_string();
                if name == "p:sldId" {
                    let mut id = 0u32;
                    let mut rid = String::new();
                    for attr in e.attributes().flatten() {
                        let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let v = String::from_utf8_lossy(&attr.value).to_string();
                        if k == "id" {
                            id = v.parse().unwrap_or(0);
                        }
                        if k == "r:id" {
                            rid = v;
                        }
                    }
                    if id > 0 && !rid.is_empty() {
                        list.push((id, rid));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    list
}

/// 提取 notesSlide 正文文本（body 占位框内容，段落按行连接）
fn parse_notes_slide_text(xml: &str) -> Option<String> {
    let mut paras: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_p = false;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = tag(e).to_string();
                if name == "a:p" {
                    in_p = true;
                    current.clear();
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = tag(e).to_string();
                if name == "a:br" && in_p {
                    current.push('\n');
                }
            }
            Ok(Event::Text(ref t)) if in_p => {
                let text = t
                    .unescape()
                    .unwrap_or_else(|_| std::str::from_utf8(t.as_ref()).unwrap_or("").into())
                    .to_string();
                current.push_str(&text);
            }
            Ok(Event::End(ref e)) => {
                let name = tag(e).to_string();
                if name == "a:p" {
                    in_p = false;
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        paras.push(trimmed);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    if paras.is_empty() {
        None
    } else {
        Some(paras.join("\n"))
    }
}

fn parse_presentation_size(xml: &str) -> Result<(f64, f64), crate::error::Error> {
    let mut width = 13.333;
    let mut height = 7.5;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let name = tag(e).to_string();
                if name == "p:sldSz" {
                    for attr in e.attributes().flatten() {
                        let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let v = String::from_utf8_lossy(&attr.value).to_string();
                        if k == "cx" {
                            if let Ok(cx) = v.parse::<i64>() {
                                width = crate::utils::constants::emu_to_inch(cx);
                            }
                        }
                        if k == "cy" {
                            if let Ok(cy) = v.parse::<i64>() {
                                height = crate::utils::constants::emu_to_inch(cy);
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
        buf.clear();
    }
    Ok((width, height))
}

fn parse_core_xml(xml: &str) -> Meta {
    let mut meta = Meta {
        title: None,
        author: None,
        company: None,
        application: None,
        last_modified_by: None,
    };
    if xml.is_empty() {
        return meta;
    }
    let mut in_title = false;
    let mut in_creator = false;
    let mut in_last_modified_by = false;
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = tag(e).to_string();
                if name == "dc:title" {
                    in_title = true;
                }
                if name == "dc:creator" {
                    in_creator = true;
                }
                if name == "cp:lastModifiedBy" || name == "lastModifiedBy" {
                    in_last_modified_by = true;
                }
            }
            Ok(Event::Text(ref t)) => {
                let text = String::from_utf8_lossy(t.as_ref()).to_string();
                if in_title {
                    meta.title = Some(text.clone());
                }
                if in_creator {
                    meta.author = Some(text.clone());
                }
                if in_last_modified_by {
                    meta.last_modified_by = Some(text);
                }
            }
            Ok(Event::End(ref e)) => {
                let name = tag(e).to_string();
                if name == "dc:title" {
                    in_title = false;
                }
                if name == "dc:creator" {
                    in_creator = false;
                }
                if name == "cp:lastModifiedBy" || name == "lastModifiedBy" {
                    in_last_modified_by = false;
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    meta
}

/// 解析 docProps/app.xml 中扩展属性（Company / Application）
fn parse_app_xml(xml: &str, meta: &mut Meta) {
    if xml.is_empty() {
        return;
    }
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut field: Option<&str> = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                field = match e.local_name().as_ref() {
                    b"Company" => Some("company"),
                    b"Application" => Some("application"),
                    _ => None,
                };
            }
            Ok(Event::Text(ref t)) => {
                if let Some(f) = field {
                    let v = t
                        .unescape()
                        .unwrap_or_else(|_| std::str::from_utf8(t.as_ref()).unwrap_or("").into())
                        .to_string();
                    match f {
                        "company" => meta.company = Some(v),
                        "application" => meta.application = Some(v),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                if matches!(e.local_name().as_ref(), b"Company" | b"Application") {
                    field = None;
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;

    #[test]
    fn normalize_relative_target() {
        assert_eq!(
            normalize_part_path("ppt/slides/", "../slideLayouts/slideLayout2.xml"),
            "ppt/slideLayouts/slideLayout2.xml"
        );
        assert_eq!(
            normalize_part_path("ppt/slides/", "/ppt/slideMasters/slideMaster1.xml"),
            "ppt/slideMasters/slideMaster1.xml"
        );
    }

    #[test]
    fn finds_layout_target() {
        let rels = r#"<Relationships xmlns="r">
          <Relationship Id="rId1" Type=".../slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
          <Relationship Id="rId2" Type=".../image" Target="../media/image1.png"/>
        </Relationships>"#;
        assert_eq!(
            find_layout_target(rels).as_deref(),
            Some("../slideLayouts/slideLayout1.xml")
        );
    }
}
