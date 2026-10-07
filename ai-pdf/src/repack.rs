//! 逆向生成：产物目录（document.json + pages/*.json + media/ + fonts/）→ PDF。
//!
//! 三阶段构建：
//! 1. 扫描：遍历所有页元素，收集字体（内嵌字体资源优先）、图片、图案与着色；
//! 2. 建对象：标准 14（Type1 零嵌入）/ 内嵌字体资源（CIDFontType2 + FontFile2 或
//!    CIDFontType0 + FontFile3/OpenType）/ 系统字体回退 / 图片（PNG 像素流、
//!    JPEG DCTDecode、无法解码的按原滤镜直通）/ 图案与着色对象；
//! 3. 生成内容流：文本（BT/Tf/Tm/Tj，支持旋转）、矩形/路径（re/m/l/c/h）、
//!    图案填充（/Pattern cs /Pn scn）、渐变（sh）、图片（cm + Do）、透明度（gs）。

use crate::fonts::{self, EmbeddedFont, FontFormat};
use crate::model::{
    DocumentModel, Element, FontResource, ImagePassthrough, PageModel, PatternDef, ShadingDef,
};
use anyhow::{bail, Result};
use lopdf::{Dictionary, Object, Stream, StringFormat};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// repack 结果统计。
pub struct RepackResult {
    pub pages: usize,
    pub images: usize,
    pub fonts: Vec<String>,
    pub warnings: Vec<String>,
}

/// 产物目录 → PDF。
pub fn repack(dir: &Path, out: &Path) -> Result<RepackResult> {
    let model: DocumentModel = serde_json::from_str(
        &std::fs::read_to_string(dir.join("document.json"))
            .map_err(|e| anyhow::anyhow!("read document.json: {e}"))?,
    )
    .map_err(|e| anyhow::anyhow!("parse document.json: {e}"))?;
    repack_model(&model, dir, out)
}

/// 内嵌字体资源（fonts/font-NNN.json + 字体文件）。
struct FontAsset {
    name: String,
    file: PathBuf,
    format: FontFormat,
    map: BTreeMap<u32, u16>,
    widths: BTreeMap<u16, i64>,
    /// 简单字体的 /Encoding 名（Type1C 直通重建）
    encoding: Option<String>,
    /// 原 FontDescriptor 度量（Type1C 直通重建）
    metrics: Option<crate::model::FontMetrics>,
    /// Type3 直通数据
    type3: Option<crate::model::Type3Data>,
    /// 产物根（读非 Identity 的 CIDToGIDMap 等附属文件）
    dir: PathBuf,
    /// 非 Identity 的 CIDToGIDMap 码流文件（相对产物根）
    cid_to_gid: Option<String>,
    /// 原 /W 数组与 /DW（非 Identity 映射时按 CID 索引，原样回写）
    w_array: Option<serde_json::Value>,
    dw: Option<i64>,
    /// 自定义编码 CMap 文件（Type0 /Encoding 为 CMap 流）
    encoding_cmap: Option<String>,
    /// /Encoding 字典的 /Differences（Type1 直通回写）
    encoding_differences: Option<serde_json::Value>,
    /// Type1 /FontFile 的 /Length1
    length1: Option<i64>,
    /// 简单字体的 /Widths（码位 → 1/1000 em）
    code_widths: Option<std::collections::BTreeMap<u16, i64>>,
    /// 简单字体的 unicode → 源码位（宽度回写用）
    code_map: Option<std::collections::BTreeMap<u32, u16>>,
    /// 字体程序 cmap 不可用：按简单字体 + 原码位重建
    simple_rebuild: bool,
}

pub fn repack_model(model: &DocumentModel, dir: &Path, out: &Path) -> Result<RepackResult> {
    if model.pages.is_empty() {
        bail!("document.json has no pages");
    }
    // 载入全部页
    let mut pages: Vec<PageModel> = Vec::with_capacity(model.pages.len());
    for page_file in &model.pages {
        let page_path = dir.join(page_file);
        let page: PageModel = serde_json::from_str(
            &std::fs::read_to_string(&page_path)
                .map_err(|e| anyhow::anyhow!("read {}: {e}", page_path.display()))?,
        )
        .map_err(|e| anyhow::anyhow!("parse {}: {e}", page_file))?;
        pages.push(page);
    }

    let mut warnings = Vec::new();

    // 内嵌字体资源
    let mut assets: BTreeMap<String, FontAsset> = BTreeMap::new();
    for font_file in &model.fonts {
        let path = dir.join(font_file);
        let res: FontResource = match std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(r) => r,
            None => {
                warnings.push(format!("font resource {font_file} unreadable — ignored"));
                continue;
            }
        };
        let file = dir.join(&res.file);
        // Type3 直通没有字体程序文件（CharProcs 单独存放）；
        // 非内嵌 CID 字体（cidnone）同样没有程序，靠 /W + 阅读器替换渲染
        let no_program = res.file.is_empty() || res.format == "cidnone";
        if res.type3.is_none() && !no_program && !file.exists() {
            warnings.push(format!(
                "font file {} missing — system font fallback",
                res.file
            ));
            continue;
        }
        assets.insert(
            res.id.clone(),
            FontAsset {
                name: res.name.clone(),
                file,
                format: FontFormat::from_str(&res.format),
                map: res.map.clone(),
                widths: res.widths.clone(),
                encoding: res.encoding.clone(),
                metrics: res.metrics.clone(),
                type3: res.type3.clone(),
                dir: dir.to_path_buf(),
                cid_to_gid: res.cid_to_gid.clone(),
                w_array: res.w_array.clone(),
                dw: res.dw,
                encoding_cmap: res.encoding_cmap.clone(),
                encoding_differences: res.encoding_differences.clone(),
                length1: res.length1,
                code_widths: res.code_widths.clone(),
                code_map: res.code_map.clone(),
                simple_rebuild: res.simple_rebuild,
            },
        );
    }

    // ── 阶段 1：扫描 ──
    let mut font_keys: BTreeMap<String, Vec<char>> = BTreeMap::new();
    let mut image_srcs: Vec<String> = Vec::new();
    for page in &pages {
        for el in &page.elements {
            match el {
                Element::Text {
                    font,
                    text,
                    font_id,
                    ..
                } => {
                    let key = text_font_key(font, text, font_id.as_deref());
                    font_keys.entry(key).or_default().extend(text.chars());
                }
                Element::Image { src, .. } => image_srcs.push(src.clone()),
                _ => {}
            }
        }
    }
    // Type3 直通的字形图片由 CharProcs 引用（不在页面元素里），同样需要嵌入
    for asset in assets.values() {
        if let Some(t3) = &asset.type3 {
            for src in t3.xobjects.values() {
                if !image_srcs.contains(src) {
                    image_srcs.push(src.clone());
                }
            }
        }
    }
    // 软掩膜表单内部引用的图片（软阴影位图）同样要嵌入
    for page in &pages {
        let mut ms: Vec<crate::model::SoftMask> = Vec::new();
        collect_masks(&page.elements, &mut ms);
        for m in ms {
            for src in m.images.values() {
                if !image_srcs.contains(src) {
                    image_srcs.push(src.clone());
                }
            }
        }
    }

    // ── 阶段 2：建对象 ──
    let mut doc = lopdf::Document::new();
    doc.version = "1.7".into();
    let pages_id = doc.add_object(Object::Dictionary(Dictionary::new()));
    let catalog = {
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Catalog".into()));
        d.set("Pages", Object::Reference(pages_id));
        d
    };
    let catalog_id = doc.add_object(Object::Dictionary(catalog));
    doc.trailer.set("Root", Object::Reference(catalog_id));

    let mut next_res = 0u32;
    let mut font_entries: BTreeMap<String, FontEntry> = BTreeMap::new();
    let mut descriptions: Vec<String> = Vec::new();

    for (key, chars) in &font_keys {
        next_res += 1;
        let res_name = format!("F{next_res}");
        if let Some(asset) = key.strip_prefix("res:").and_then(|id| assets.get(id)) {
            // Type3 直通：需在图片嵌入之后构建，跳过此轮
            if asset.type3.is_some() {
                continue;
            }
            // 简单字体（有 unicode→码位映射，说明源 PDF 是简单字体）：按原
            // /Encoding + /Widths 重建为简单字体，文本按原码回写。子集 TrueType
            // 常无可解析 cmap，按 CID 重建会整页 .notdef（pdf.js/pdkids）
            if asset.simple_rebuild {
                match build_simple_font(&mut doc, asset) {
                    Ok(id) => {
                        descriptions.push(asset.name.clone());
                        font_entries.insert(
                            key.clone(),
                            FontEntry {
                                res_name,
                                object_id: id,
                                is_cid: false,
                                is_symbol: false,
                                type3_codes: None,
                                embedded: None,
                            },
                        );
                        continue;
                    }
                    Err(e) => {
                        warnings.push(format!(
                            "font '{}' simple rebuild failed ({e}) — continue as CID",
                            asset.name
                        ));
                    }
                }
            }
            // 裸 CFF（Type1C）/ Type1：以简单 Type1 字体直通嵌入（原 /Widths + /Encoding）
            if matches!(asset.format, FontFormat::Type1C | FontFormat::Type1) {
                match build_type1_font(&mut doc, asset) {
                    Ok(id) => {
                        descriptions.push(asset.name.clone());
                        font_entries.insert(
                            key.clone(),
                            FontEntry {
                                res_name,
                                object_id: id,
                                is_cid: false,
                                is_symbol: false,
                                type3_codes: None,
                                embedded: None,
                            },
                        );
                        continue;
                    }
                    Err(e) => {
                        warnings.push(format!(
                            "font '{}' cannot be embedded ({e}) — system fallback",
                            asset.name
                        ));
                    }
                }
            }
            match load_asset_font(asset, &mut warnings) {
                Some(font) => {
                    let font = Arc::new(font);
                    let id = build_cid_font(&mut doc, &font, chars, &mut descriptions)?;
                    font_entries.insert(
                        key.clone(),
                        FontEntry {
                            res_name,
                            object_id: id,
                            is_cid: true,
                            is_symbol: false,
                            type3_codes: None,
                            embedded: Some(font),
                        },
                    );
                    continue;
                }
                None => {
                    // 资源不可用 → 按显示名回退系统字体
                    let display = asset.name.clone();
                    let family = display
                        .strip_prefix("system:")
                        .unwrap_or(&display)
                        .to_string();
                    let font = fonts::load_system_font(&family).map_err(|e| {
                        anyhow::anyhow!("repack needs font '{family}' for non-ASCII text: {e}")
                    })?;
                    let font = Arc::new(font);
                    let id = build_cid_font(&mut doc, &font, chars, &mut descriptions)?;
                    font_entries.insert(
                        key.clone(),
                        FontEntry {
                            res_name,
                            object_id: id,
                            is_cid: true,
                            is_symbol: false,
                            type3_codes: None,
                            embedded: Some(font),
                        },
                    );
                    continue;
                }
            }
        }
        if let Some(name) = key.strip_prefix("std:") {
            let mut d = Dictionary::new();
            d.set("Type", Object::Name(b"Font".into()));
            d.set("Subtype", Object::Name(b"Type1".into()));
            d.set("BaseFont", Object::Name(name.as_bytes().to_vec()));
            // Symbol/ZapfDingbats 使用内建编码；声明 WinAnsiEncoding 会让
            // 码位整体错位（fpdf2 zapfdingbats：'!' 由 ✁ 变成 ✂ 一类）
            if name != "Symbol" && name != "ZapfDingbats" {
                d.set("Encoding", Object::Name(b"WinAnsiEncoding".into()));
            }
            let id = doc.add_object(Object::Dictionary(d));
            descriptions.push(name.to_string());
            font_entries.insert(
                key.clone(),
                FontEntry {
                    res_name,
                    object_id: id,
                    is_cid: false,
                    embedded: None,
                    is_symbol: name == "Symbol" || name == "ZapfDingbats",
                    type3_codes: None,
                },
            );
            continue;
        }
        // cid:<family> → 系统字体
        let family = key.strip_prefix("cid:").unwrap_or(key).to_string();
        let font = fonts::load_system_font(&family)
            .map_err(|e| anyhow::anyhow!("repack needs font '{family}' for non-ASCII text: {e}"))?;
        let font = Arc::new(font);
        let id = build_cid_font(&mut doc, &font, chars, &mut descriptions)?;
        font_entries.insert(
            key.clone(),
            FontEntry {
                res_name,
                object_id: id,
                is_cid: true,
                is_symbol: false,
                type3_codes: None,
                embedded: Some(font),
            },
        );
    }

    // 图片：直通参数与 SMask 按 src 建索引
    let mut passthrough_by_src: BTreeMap<String, &ImagePassthrough> = BTreeMap::new();
    let mut smask_by_src: BTreeMap<String, (&String, Option<&ImagePassthrough>)> = BTreeMap::new();
    for page in &pages {
        for el in &page.elements {
            if let Element::Image {
                src,
                passthrough: Some(p),
                ..
            } = el
            {
                passthrough_by_src.insert(src.clone(), p);
            }
            if let Element::Image {
                src,
                smask: Some(sm),
                smask_params,
                ..
            } = el
            {
                smask_by_src.insert(src.clone(), (sm, smask_params.as_deref()));
            }
        }
    }
    // Type3 字形图片的 SMask（不在页面元素里）
    for asset in assets.values() {
        if let Some(t3) = &asset.type3 {
            for (src, sm) in &t3.xobject_smasks {
                smask_by_src.entry(src.clone()).or_insert((sm, None));
            }
        }
    }

    // 图片
    let mut image_entries: BTreeMap<String, (String, (u32, u16))> = BTreeMap::new();
    let mut img_next = 0u32;
    for src in &image_srcs {
        let key = src.clone();
        if image_entries.contains_key(&key) {
            continue;
        }
        let img_path = resolve_src(dir, src).map_err(|e| anyhow::anyhow!("{e}"))?;
        img_next += 1;
        let res_name = format!("Im{img_next}");
        let ext = img_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let id = if let Some(pt) = passthrough_by_src.get(&key) {
            // 原码流 + 原滤镜直通（JBIG2/CCITT/JPX/混合滤镜链等）
            let smask = match &pt.smask {
                Some(smask_src) => {
                    let smask_path =
                        resolve_src(dir, smask_src).map_err(|e| anyhow::anyhow!("{e}"))?;
                    let own = pt.smask_params.as_deref();
                    let smask_pt = own.or_else(|| passthrough_by_src.get(smask_src).copied());
                    let sid = embed_image_file(&mut doc, &smask_path, smask_pt, &mut warnings, dir);
                    apply_smask_matte(&mut doc, sid, smask_pt);
                    Some(sid)
                }
                None => None,
            };
            match embed_passthrough(&mut doc, &img_path, pt, smask, dir) {
                Ok(id) => id,
                Err(e) => {
                    warnings.push(format!("{e} — replaced by placeholder"));
                    placeholder_image(&mut doc)?
                }
            }
        } else {
            match ext.as_str() {
                "jpg" | "jpeg" => match embed_jpeg(&mut doc, &img_path) {
                    Ok(id) => id,
                    Err(e) => {
                        warnings.push(format!("{e} — replaced by placeholder"));
                        placeholder_image(&mut doc)?
                    }
                },
                "png" => embed_png(&mut doc, &img_path, &mut warnings)?,
                "jp2" | "jpx" => match embed_passthrough(
                    &mut doc,
                    &img_path,
                    &ImagePassthrough {
                        filter: vec!["JPXDecode".into()],
                        decode_parms: Vec::new(),
                        color_space: "DeviceRGB".into(),
                        icc_components: 0,
                        bpc: 8,
                        width: 0,
                        height: 0,
                        decode: None,
                        image_mask: false,
                        smask: None,
                        smask_params: None,
                        matte: None,
                        globals: None,
                    },
                    None,
                    dir,
                ) {
                    Ok(id) => id,
                    Err(e) => {
                        warnings.push(format!("{e} — replaced by placeholder"));
                        placeholder_image(&mut doc)?
                    }
                },
                other => {
                    warnings.push(format!(
                        "image '{src}' type .{other} is not embeddable — replaced by placeholder"
                    ));
                    placeholder_image(&mut doc)?
                }
            }
        };
        // 附加软掩膜（PNG/JPEG/直通路径统一处理）
        if let Some((smask_src, smask_pt)) = smask_by_src.get(&key).copied() {
            if let Ok(smask_path) = resolve_src(dir, smask_src) {
                let pt = smask_pt.or_else(|| passthrough_by_src.get(smask_src).copied());
                let sid = embed_image_file(&mut doc, &smask_path, pt, &mut warnings, dir);
                apply_smask_matte(&mut doc, sid, pt);
                if let Ok(Object::Stream(st)) = doc.get_object_mut(id) {
                    st.dict.set("SMask", Object::Reference(sid));
                }
            }
        }
        image_entries.insert(key, (res_name, id));
    }

    // Type3 直通字体：CharProcs 引用图片 XObject，需在图片嵌入之后构建
    for (asset_id, asset) in assets.iter() {
        let Some(t3) = &asset.type3 else { continue };
        // font_entries 的键是字体引用键（res:<id>），不是资源 id
        let key = format!("res:{asset_id}");
        if font_entries.contains_key(&key) {
            continue;
        }
        next_res += 1;
        let res_name = format!("F{next_res}");
        match build_type3_font(&mut doc, asset, t3, &image_entries, dir) {
            Ok(id) => {
                descriptions.push(asset.name.clone());
                font_entries.insert(
                    key.clone(),
                    FontEntry {
                        res_name,
                        object_id: id,
                        is_cid: false,
                        embedded: None,
                        is_symbol: false,
                        type3_codes: Some(Arc::new(asset.map.clone())),
                    },
                );
            }
            Err(e) => warnings.push(format!("type3 font '{}' rebuild failed: {e}", asset.name)),
        }
    }

    // ── 阶段 3：页对象 + 内容流 ──
    let mut kids: Vec<(u32, u16)> = Vec::new();
    for page in &pages {
        let width = page.width.unwrap_or(model.page_size.width);
        let height = page.height.unwrap_or(model.page_size.height);

        let mut resources = Dictionary::new();
        let fonts_in_page: BTreeMap<&String, &FontEntry> = font_entries
            .iter()
            .filter(|(k, _)| page_uses_font(page, k))
            .collect();
        if !fonts_in_page.is_empty() {
            let mut fd = Dictionary::new();
            for entry in fonts_in_page.values() {
                fd.set(entry.res_name.clone(), Object::Reference(entry.object_id));
            }
            resources.set("Font", Object::Dictionary(fd));
        }
        type ImageEntry<'a> = (&'a String, &'a (String, (u32, u16)));
        let images_in_page: Vec<ImageEntry> = image_entries
            .iter()
            .filter(|(k, _)| {
                page.elements
                    .iter()
                    .any(|el| matches!(el, Element::Image { src, .. } if src == *k))
            })
            .collect();
        if !images_in_page.is_empty() {
            let mut xo = Dictionary::new();
            for (_, (res_name, id)) in &images_in_page {
                xo.set(res_name.clone(), Object::Reference(*id));
            }
            resources.set("XObject", Object::Dictionary(xo));
        }
        // 图案与着色
        let mut pattern_no = 0;
        let mut shading_no = 0;
        let mut patterns = Dictionary::new();
        let mut shadings = Dictionary::new();
        for el in &page.elements {
            if let Element::PatternRect { pattern, .. } = el {
                pattern_no += 1;
                let name = format!("P{pattern_no}");
                let id = build_tiling_pattern(
                    &mut doc,
                    pattern,
                    &font_entries,
                    &image_entries,
                    &mut warnings,
                )?;
                patterns.set(name, Object::Reference(id));
            }
            if let Element::Shading(def) = el {
                shading_no += 1;
                let name = format!("Sh{shading_no}");
                let id = match &def.mesh {
                    Some(mesh) => build_mesh_shading(&mut doc, mesh, dir)?,
                    None => build_shading(&mut doc, def, dir)?,
                };
                shadings.set(name, Object::Reference(id));
            }
        }
        if !patterns.is_empty() {
            resources.set("Pattern", Object::Dictionary(patterns));
        }
        if !shadings.is_empty() {
            resources.set("Shading", Object::Dictionary(shadings));
        }
        // 透明度与混合模式（键 = 填充 ca、描边 CA、混合模式三元组，
        // 支持逐属性透明度如 ExtGState ca≠CA）
        let mut gs_keys: Vec<GsKey> = Vec::new();
        let mut masks: Vec<crate::model::SoftMask> = Vec::new();
        collect_masks(&page.elements, &mut masks);
        collect_gs_keys(&page.elements, &masks, &mut gs_keys);
        let mut gs_names: BTreeMap<GsKey, String> = BTreeMap::new();
        let mut gs = Dictionary::new();
        {
            for (i, key) in gs_keys.iter().enumerate() {
                let name = format!("GS{}", i + 1);
                let mut d = Dictionary::new();
                d.set("Type", Object::Name(b"ExtGState".into()));
                let a = key.0 as f64 / 1e6;
                let sa = key.1 as f64 / 1e6;
                if (a - 1.0).abs() > 1e-9 {
                    d.set("ca", Object::Real(a as f32));
                }
                if (sa - 1.0).abs() > 1e-9 {
                    d.set("CA", Object::Real(sa as f32));
                }
                if let Some(bm) = &key.2 {
                    d.set("BM", Object::Name(bm.as_bytes().to_vec()));
                }
                // ExtGState /SMask：掩膜表单原样挂回（内容 + 图片资源 + BBox/Matrix）
                if let Some(mi) = key.3 {
                    if let Some(m) = masks.get(mi) {
                        if let Ok(gid) = build_softmask_form(&mut doc, m, dir, &image_entries) {
                            let mut smd = Dictionary::new();
                            smd.set("Type", Object::Name(b"Mask".into()));
                            smd.set(
                                "S",
                                Object::Name(if m.subtype.eq_ignore_ascii_case("Alpha") {
                                    b"Alpha".to_vec()
                                } else {
                                    b"Luminosity".to_vec()
                                }),
                            );
                            smd.set("G", Object::Reference(gid));
                            if let Some(bc) = &m.backdrop {
                                smd.set(
                                    "BC",
                                    Object::Array(
                                        bc.iter().map(|v| Object::Real(*v as f32)).collect(),
                                    ),
                                );
                            }
                            if let Some(tr) = &m.transfer {
                                smd.set("TR", json_to_object(tr));
                            }
                            d.set("SMask", Object::Dictionary(smd));
                        }
                    }
                }
                let id = doc.add_object(Object::Dictionary(d));
                gs.set(name.clone(), Object::Reference(id));
                gs_names.insert(key.clone(), name);
            }
        }
        // 亮度软掩膜：每个带 mask 的着色一个 ExtGState（含 SMask 表单）。
        // 组内着色同样要建，名字全局编号，按层记录索引 → 名字
        let mut mask_seq = 0usize;
        let mut level = collect_level(&page.elements, &mut mask_seq);
        for (idx, name, def, alpha, stroke_alpha) in level.mask_defs.iter() {
            let _ = idx;
            let id = build_mask_extgstate(&mut doc, def, *alpha, *stroke_alpha)?;
            gs.set(name.clone(), Object::Reference(id));
        }
        if !gs.is_empty() {
            resources.set("ExtGState", Object::Dictionary(gs));
        }
        // 透明组：Group 元素 → Form XObject（保留 /Group 隔离/撞色语义）。
        // 递归构建：组内再嵌套组（pdf.js/knockout_nested_group_alpha）时，
        // 子层表单名要进入父层内容与资源
        let mut form_seq = 0usize;
        let mut form_names: BTreeMap<usize, String> = BTreeMap::new();
        let mut forms = Dictionary::new();
        build_group_level(
            &mut doc,
            &page.elements,
            &resources,
            &font_entries,
            &image_entries,
            &gs_names,
            &mut level,
            &masks,
            &mut form_seq,
            &mut forms,
            &mut form_names,
            &mut warnings,
        )?;
        // 与页面图片的 /XObject 合并（直接覆盖会丢掉 Im* 引用，
        // pdf.js/transfer_maps 的 /Im1 Do 解析失败）
        if !forms.is_empty() {
            let mut xo = resources
                .get(b"XObject")
                .ok()
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default();
            for (k, v) in forms.iter() {
                xo.set(k.clone(), v.clone());
            }
            resources.set("XObject", Object::Dictionary(xo));
        }

        let content = build_content(
            page,
            &font_entries,
            &image_entries,
            &gs_names,
            &level.mask_names,
            &form_names,
            &masks,
            &mut warnings,
        )?;
        let content_id = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), content)));

        let mut page_dict = Dictionary::new();
        page_dict.set("Type", Object::Name(b"Page".into()));
        page_dict.set("Parent", Object::Reference(pages_id));
        match page.mediabox {
            Some(mb) => page_dict.set(
                "MediaBox",
                Object::Array(mb.iter().map(|v| Object::Real(*v as f32)).collect()),
            ),
            None => page_dict.set(
                "MediaBox",
                Object::Array(vec![
                    Object::Real(0.0),
                    Object::Real(0.0),
                    Object::Real(width as f32),
                    Object::Real(height as f32),
                ]),
            ),
        }
        if page.rotation > 0 {
            page_dict.set("Rotate", Object::Integer(page.rotation as i64));
        }
        if let Some(crop) = page.crop {
            page_dict.set(
                "CropBox",
                Object::Array(crop.iter().map(|v| Object::Real(*v as f32)).collect()),
            );
        }
        page_dict.set("Resources", Object::Dictionary(resources));
        page_dict.set("Contents", Object::Reference(content_id));
        let page_id = doc.add_object(Object::Dictionary(page_dict));
        kids.push(page_id);
    }

    let pages_dict = doc.get_object_mut(pages_id).and_then(Object::as_dict_mut)?;
    pages_dict.set("Type", Object::Name(b"Pages".into()));
    pages_dict.set(
        "Kids",
        Object::Array(kids.iter().map(|id| Object::Reference(*id)).collect()),
    );
    pages_dict.set("Count", Object::Integer(kids.len() as i64));

    // Info
    if let Some(meta) = &model.meta {
        let mut info = Dictionary::new();
        let put = |d: &mut Dictionary, k: &str, v: &Option<String>| {
            if let Some(s) = v {
                d.set(
                    k.to_string(),
                    Object::String(s.as_bytes().to_vec(), StringFormat::Literal),
                );
            }
        };
        put(&mut info, "Title", &meta.title);
        put(&mut info, "Author", &meta.author);
        put(&mut info, "Subject", &meta.subject);
        put(&mut info, "Creator", &meta.creator);
        info.set(
            "Producer".to_string(),
            Object::String(b"json2pdf".to_vec(), StringFormat::Literal),
        );
        let info_id = doc.add_object(Object::Dictionary(info));
        doc.trailer.set("Info", Object::Reference(info_id));
    }

    doc.save(out)?;
    descriptions.sort();
    Ok(RepackResult {
        pages: kids.len(),
        images: image_entries.len(),
        fonts: descriptions,
        warnings,
    })
}

fn load_asset_font(asset: &FontAsset, warnings: &mut Vec<String>) -> Option<EmbeddedFont> {
    // 非内嵌 CID 字体没有字体程序文件（cidnone），无需读取
    let data = if asset.format == FontFormat::CidNoFile || asset.file.as_os_str().is_empty() {
        Vec::new()
    } else {
        match std::fs::read(&asset.file) {
            Ok(d) => d,
            Err(e) => {
                warnings.push(format!(
                    "font file {} unreadable: {e}",
                    asset.file.display()
                ));
                return None;
            }
        }
    };
    let cid_to_gid = asset
        .cid_to_gid
        .as_ref()
        .and_then(|rel| std::fs::read(asset.dir.join(rel)).ok());
    let encoding_cmap = asset
        .encoding_cmap
        .as_ref()
        .and_then(|rel| std::fs::read(asset.dir.join(rel)).ok());
    match fonts::from_resource(
        &asset.name,
        data,
        asset.format,
        &asset.map,
        &asset.widths,
        cid_to_gid,
        asset.w_array.clone(),
        asset.dw,
        encoding_cmap,
        asset.code_map.as_ref().unwrap_or(&BTreeMap::new()),
        asset.code_widths.as_ref().unwrap_or(&BTreeMap::new()),
    ) {
        Ok(f) => Some(f),
        Err(e) => {
            warnings.push(format!(
                "font '{}' cannot be embedded ({e}) — system fallback",
                asset.name
            ));
            None
        }
    }
}

fn element_blend(el: &Element) -> Option<&str> {
    match el {
        Element::Text { blend, .. }
        | Element::Rect { blend, .. }
        | Element::PatternRect { blend, .. }
        | Element::Path { blend, .. }
        | Element::Image { blend, .. }
        | Element::Group { blend, .. } => blend.as_deref(),
        _ => None,
    }
}

fn element_alpha(el: &Element) -> f64 {
    match el {
        Element::Text { alpha, .. }
        | Element::Rect { alpha, .. }
        | Element::PatternRect { alpha, .. }
        | Element::Path { alpha, .. }
        | Element::Image { alpha, .. }
        | Element::Group { alpha, .. } => *alpha,
        Element::Shading(def) => def.alpha,
        _ => 1.0,
    }
}

/// 描边不透明度（仅 Rect/Path 支持逐属性；其余跟随填充 alpha）
fn element_stroke_alpha(el: &Element) -> Option<f64> {
    match el {
        Element::Rect { stroke_alpha, .. } | Element::Path { stroke_alpha, .. } => *stroke_alpha,
        // 组的 ca/CA 作用在组合成（Do）上
        Element::Group { alpha, .. } => Some(*alpha),
        _ => None,
    }
}

/// 元素的 ExtGState 键（填充 ca、描边 CA、混合模式）。
/// ExtGState 键：填充 ca、描边 CA、混合模式、软掩膜索引（None = 无掩膜）。
type GsKey = (i64, i64, Option<String>, Option<usize>);

fn element_gs_key_idx(el: &Element, masks: &[crate::model::SoftMask]) -> GsKey {
    let (a, sa, b) = element_gs_key(el);
    let m = element_smask(el).and_then(|sm| masks.iter().position(|x| x.content == sm.content));
    (a, sa, b, m)
}

fn element_smask(el: &Element) -> Option<&crate::model::SoftMask> {
    match el {
        Element::Text { smask, .. }
        | Element::Rect { smask, .. }
        | Element::Path { smask, .. }
        | Element::Group { smask, .. } => smask.as_deref(),
        _ => None,
    }
}

fn element_gs_key(el: &Element) -> (i64, i64, Option<String>) {
    let a = element_alpha(el);
    let sa = element_stroke_alpha(el).unwrap_or(a);
    let b = element_blend(el).map(|s| s.to_string());
    ((a * 1e6).round() as i64, (sa * 1e6).round() as i64, b)
}

fn page_uses_font(page: &PageModel, key: &str) -> bool {
    page.elements.iter().any(|el| match el {
        Element::Text {
            font,
            text,
            font_id,
            ..
        } => *key == text_font_key(font, text, font_id.as_deref()),
        _ => false,
    })
}

/// Unicode → WinAnsi 字节（标准 14 字体用 WinAnsiEncoding 写字节串）。
///
/// 覆盖 ASCII、Latin-1 补充区与 0x80-0x9F 段的排版符号（引号/破折号/省略号等）。
pub(crate) fn winansi_byte(c: char) -> Option<u8> {
    let cp = c as u32;
    if cp < 0x80 || (0xA0..=0xFF).contains(&cp) {
        return Some(cp as u8);
    }
    Some(match c {
        '\u{20AC}' => 0x80, // €
        '\u{201A}' => 0x82, // ‚
        '\u{0192}' => 0x83, // ƒ
        '\u{201E}' => 0x84, // „
        '\u{2026}' => 0x85, // …
        '\u{2020}' => 0x86, // †
        '\u{2021}' => 0x87, // ‡
        '\u{02C6}' => 0x88, // ˆ
        '\u{2030}' => 0x89, // ‰
        '\u{0160}' => 0x8A, // Š
        '\u{2039}' => 0x8B, // ‹
        '\u{0152}' => 0x8C, // Œ
        '\u{017D}' => 0x8E, // Ž
        '\u{2018}' => 0x91, // ‘
        '\u{2019}' => 0x92, // ’
        '\u{201C}' => 0x93, // “
        '\u{201D}' => 0x94, // ”
        '\u{2022}' => 0x95, // •
        '\u{2013}' => 0x96, // –
        '\u{2014}' => 0x97, // —
        '\u{02DC}' => 0x98, // ˜
        '\u{2122}' => 0x99, // ™
        '\u{0161}' => 0x9A, // š
        '\u{203A}' => 0x9B, // ›
        '\u{0153}' => 0x9C, // œ
        '\u{017E}' => 0x9E, // ž
        '\u{0178}' => 0x9F, // Ÿ
        _ => return None,
    })
}

/// WinAnsi 可编码：标准 14 字体可直接用 WinAnsiEncoding 写字节串。
fn winansi_ok(text: &str) -> bool {
    text.chars().all(|c| winansi_byte(c).is_some())
}

fn text_font_key(font: &str, text: &str, font_id: Option<&str>) -> String {
    if let Some(id) = font_id {
        return format!("res:{id}");
    }
    let needs_cid = !crate::model::is_standard_font(font) || !winansi_ok(text);
    if needs_cid {
        format!("cid:{}", font.strip_prefix("system:").unwrap_or(font))
    } else {
        format!("std:{}", standard_name(font))
    }
}

/// 标准 14 名映射（别名 → 基族；已是标准 14 则原样）。
fn standard_name(font: &str) -> String {
    if crate::model::is_standard_font(font) {
        return font.to_string();
    }
    let f = font.to_lowercase();
    let base = if f.contains("times") || f.contains("song") || f.contains("serif") {
        "Times-Roman"
    } else if f.contains("courier") || f.contains("mono") {
        "Courier"
    } else {
        "Helvetica"
    };
    base.to_string()
}

struct FontEntry {
    res_name: String,
    object_id: (u32, u16),
    is_cid: bool,
    embedded: Option<Arc<EmbeddedFont>>,
    /// Symbol/ZapfDingbats：内建编码，文本按 Latin1 原样写字节
    is_symbol: bool,
    /// Type3 直通：unicode→码（文本按原码写字节）
    type3_codes: Option<Arc<BTreeMap<u32, u16>>>,
}

fn resolve_src(dir: &Path, src: &str) -> Result<PathBuf> {
    let p = Path::new(src);
    if p.is_absolute() {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        bail!("image src not found: {src}");
    }
    let resolved = dir.join(p);
    if resolved.exists() {
        Ok(resolved)
    } else {
        bail!("image src not found: {src}")
    }
}

// ── 字体嵌入 ──────────────────────────────────────────────────────────────

/// 构建 CID 字体对象树（只覆盖 chars 中出现的字形）。
fn build_cid_font(
    doc: &mut lopdf::Document,
    font: &Arc<EmbeddedFont>,
    chars: &[char],
    descriptions: &mut Vec<String>,
) -> Result<(u32, u16)> {
    // 已用 cid（= gid）集合与反向映射
    let mut used: BTreeMap<u16, u32> = BTreeMap::new(); // gid → unicode
    let mut widths: BTreeMap<u16, i64> = BTreeMap::new(); // gid → 1/1000 em
    for &c in chars {
        if let Some(&gid) = font.cmap.get(&(c as u32)) {
            used.insert(gid, c as u32);
            // 原 /Widths 声明的码宽优先（可能不同于字体程序自带 advance，
            // 如零宽空格；pdf.js/issue6894 的 "Arial Bold" 应为 "ArialBold"）
            let w = font
                .code_map
                .get(&(c as u32))
                .and_then(|code| font.code_widths.get(code))
                .copied()
                .unwrap_or_else(|| font.glyph_width_1000(gid));
            widths.insert(gid, w);
        }
    }

    // 字体文件（Flate）
    let compressed = {
        use std::io::Write;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&font.data)?;
        enc.finish()?
    };
    let mut ff_dict = Dictionary::new();
    // 无字体程序：只回写 CIDSystemInfo + /W + /Encoding，交由阅读器替换
    if font.format == FontFormat::CidNoFile {
        return build_nofile_cid_font(doc, font, &widths, &used, descriptions);
    }
    let (ff_key, ff_subtype, font_subtype) = match font.format {
        FontFormat::CidNoFile => unreachable!(),
        FontFormat::TrueType => ("FontFile2", None, "CIDFontType2"),
        FontFormat::OpenType => ("FontFile3", Some("OpenType"), "CIDFontType0"),
        // Type1C 不构建 CID 字体（走 build_type1_font）
        FontFormat::Type1C => bail!("Type1C fonts are embedded as simple Type1 fonts"),
        FontFormat::CidCff => ("FontFile3", Some("CIDFontType0C"), "CIDFontType0"),
        FontFormat::Type1 => bail!("Type1 fonts are embedded as simple Type1 fonts"),
    };
    // 修复：compressed 是 zlib 数据，必须声明 /Filter /FlateDecode，
    // 否则阅读器把压缩字节直接当字体交给 FreeType（"unknown file format"，
    // 静默回退替代字体）。依据：评测语料 12% 样本字体流为 Flate 压缩，
    // 重建件渲染出现 FT_New_Memory_Face 错误；补声明后 fonts_otf 2.90→0.00。
    ff_dict.set("Filter", Object::Name(b"FlateDecode".to_vec()));
    if font.format == FontFormat::TrueType {
        ff_dict.set("Length1", Object::Integer(font.data.len() as i64));
    } else {
        ff_dict.set(
            "Subtype",
            Object::Name(ff_subtype.unwrap().as_bytes().to_vec()),
        );
        ff_dict.set("Length1", Object::Integer(font.data.len() as i64));
    }
    let fontfile_id = doc.add_object(Object::Stream(Stream::new(ff_dict, compressed)));

    let bbox = font.bbox_1000();
    let mut fd = Dictionary::new();
    fd.set("Type", Object::Name(b"FontDescriptor".into()));
    fd.set("FontName", Object::Name(font.name.as_bytes().to_vec()));
    fd.set("Flags", Object::Integer(4)); // symbolic
    fd.set(
        "FontBBox",
        Object::Array(bbox.iter().map(|v| Object::Integer(*v)).collect()),
    );
    fd.set("ItalicAngle", Object::Real(font.italic_angle));
    fd.set("Ascent", Object::Integer(font.scale_1000(font.ascent)));
    fd.set("Descent", Object::Integer(font.scale_1000(font.descent)));
    fd.set(
        "CapHeight",
        Object::Integer(font.scale_1000(font.cap_height)),
    );
    fd.set("StemV", Object::Real(80.0));
    fd.set(ff_key, Object::Reference(fontfile_id));
    let fd_id = doc.add_object(Object::Dictionary(fd));

    // W：cid [w] 对（1/1000 em）
    let mut w_arr: Vec<Object> = Vec::new();
    for (gid, w) in &widths {
        w_arr.push(Object::Integer(*gid as i64));
        w_arr.push(Object::Array(vec![Object::Integer(*w)]));
    }

    let mut cidsys = Dictionary::new();
    cidsys.set(
        "Registry",
        Object::String(b"Adobe".to_vec(), StringFormat::Literal),
    );
    cidsys.set(
        "Ordering",
        Object::String(b"Identity".to_vec(), StringFormat::Literal),
    );
    cidsys.set("Supplement", Object::Integer(0));

    let mut descendant = Dictionary::new();
    descendant.set("Type", Object::Name(b"Font".into()));
    descendant.set("Subtype", Object::Name(font_subtype.as_bytes().to_vec()));
    descendant.set("BaseFont", Object::Name(font.name.as_bytes().to_vec()));
    descendant.set("CIDSystemInfo", Object::Dictionary(cidsys));
    descendant.set("FontDescriptor", Object::Reference(fd_id));
    // 非 Identity CIDToGIDMap：W 数组按 CID 索引，与原始码位配套原样回写
    match (&font.w_array, font.dw) {
        (Some(w), Some(dw)) => {
            descendant.set("DW", Object::Integer(dw));
            descendant.set("W", json_to_object(w));
        }
        _ => {
            descendant.set("DW", Object::Integer(1000));
            descendant.set("W", Object::Array(w_arr));
        }
    }
    // CIDFontType0 规范上不用 /CIDToGIDMap，但部分生成器（pdf.js issue7901）
    // 会写入且阅读器按它取字形——存在时一并回写
    {
        // 原字体是非 Identity 映射时必须原样带上：原始码位回写（复杂文种
        // 整形）依赖它指向正确字形（fpdf2 bidi_arabic）
        match &font.cid_to_gid {
            Some(map) => {
                let compressed = {
                    use std::io::Write;
                    let mut enc =
                        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                    enc.write_all(map)?;
                    enc.finish()?
                };
                let mut md = Dictionary::new();
                md.set("Filter", Object::Name(b"FlateDecode".into()));
                let mid = doc.add_object(Object::Stream(Stream::new(md, compressed)));
                descendant.set("CIDToGIDMap", Object::Reference(mid));
            }
            None => descendant.set("CIDToGIDMap", Object::Name(b"Identity".into())),
        }
    }
    let descendant_id = doc.add_object(Object::Dictionary(descendant));

    let cmap_str = fonts::build_to_unicode(&used);
    let tu_id = doc.add_object(Object::Stream(Stream::new(
        Dictionary::new(),
        cmap_str.into_bytes(),
    )));

    let mut type0 = Dictionary::new();
    type0.set("Type", Object::Name(b"Font".into()));
    type0.set("Subtype", Object::Name(b"Type0".into()));
    type0.set("BaseFont", Object::Name(font.name.as_bytes().to_vec()));
    // 自定义编码 CMap：原样回写（Identity-H 会把 1 字节码也当 2 字节）
    match &font.encoding_cmap {
        Some(cmap) => {
            let compressed = {
                use std::io::Write;
                let mut enc =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                enc.write_all(cmap)?;
                enc.finish()?
            };
            let mut cd = Dictionary::new();
            cd.set("Filter", Object::Name(b"FlateDecode".into()));
            let cid = doc.add_object(Object::Stream(Stream::new(cd, compressed)));
            type0.set("Encoding", Object::Reference(cid));
        }
        None => type0.set("Encoding", Object::Name(b"Identity-H".into())),
    }
    type0.set(
        "DescendantFonts",
        Object::Array(vec![Object::Reference(descendant_id)]),
    );
    type0.set("ToUnicode", Object::Reference(tu_id));
    descriptions.push(font.name.clone());
    Ok(doc.add_object(Object::Dictionary(type0)))
}

// ── 图案与着色 ────────────────────────────────────────────────────────────

/// 平铺图案（PatternType 1）：图块内容用元素模型递归生成。
fn build_tiling_pattern(
    doc: &mut lopdf::Document,
    def: &PatternDef,
    fonts: &BTreeMap<String, FontEntry>,
    images: &BTreeMap<String, (String, (u32, u16))>,
    warnings: &mut Vec<String>,
) -> Result<(u32, u16)> {
    let mut content = String::new();
    for el in &def.elements {
        emit_element(
            &mut content,
            el,
            usize::MAX,
            fonts,
            images,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &[],
            &Vec::new(),
            warnings,
            true,
        );
    }
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"Pattern".into()));
    d.set("PatternType", Object::Integer(1));
    d.set("PaintType", Object::Integer(1));
    d.set("TilingType", Object::Integer(1));
    d.set(
        "BBox",
        Object::Array(def.bbox.iter().map(|v| Object::Real(*v as f32)).collect()),
    );
    d.set("XStep", Object::Real(def.x_step as f32));
    d.set("YStep", Object::Real(def.y_step as f32));
    if let Some(m) = def.matrix {
        d.set(
            "Matrix",
            Object::Array(m.iter().map(|v| Object::Real(*v as f32)).collect()),
        );
    }
    let mut res = Dictionary::new();
    if !fonts.is_empty() {
        let mut fd = Dictionary::new();
        for e in fonts.values() {
            fd.set(e.res_name.clone(), Object::Reference(e.object_id));
        }
        res.set("Font", Object::Dictionary(fd));
    }
    if !images.is_empty() {
        let mut xo = Dictionary::new();
        for (name, id) in images.values() {
            xo.set(name.clone(), Object::Reference(*id));
        }
        res.set("XObject", Object::Dictionary(xo));
    }
    d.set("Resources", Object::Dictionary(res));
    Ok(doc.add_object(Object::Stream(Stream::new(d, content.into_bytes()))))
}

/// 着色字典（ShadingType 2/3 + 拼接函数）。
/// 非内嵌 CID 字体：无字体程序，只保留 /W /DW /CIDSystemInfo /Encoding，
/// 由阅读器按其替换规则选字形（原 PDF 亦然）。
fn build_nofile_cid_font(
    doc: &mut lopdf::Document,
    font: &Arc<EmbeddedFont>,
    widths: &BTreeMap<u16, i64>,
    used: &BTreeMap<u16, u32>,
    descriptions: &mut Vec<String>,
) -> Result<(u32, u16)> {
    let mut cidsys = Dictionary::new();
    cidsys.set(
        "Registry",
        Object::String(b"Adobe".to_vec(), StringFormat::Literal),
    );
    cidsys.set(
        "Ordering",
        Object::String(b"Identity".to_vec(), StringFormat::Literal),
    );
    cidsys.set("Supplement", Object::Integer(0));
    let mut descendant = Dictionary::new();
    descendant.set("Type", Object::Name(b"Font".into()));
    descendant.set("Subtype", Object::Name(b"CIDFontType2".into()));
    descendant.set("BaseFont", Object::Name(font.name.as_bytes().to_vec()));
    descendant.set("CIDSystemInfo", Object::Dictionary(cidsys));
    // 不写 FontDescriptor：源 PDF 亦然；带描述符但无 FontFile 时阅读器会
    // 当作「内嵌但损坏」而不做替换（MuPDF 直接不画字形）
    match (&font.w_array, font.dw) {
        (Some(w), Some(dw)) => {
            descendant.set("DW", Object::Integer(dw));
            descendant.set("W", json_to_object(w));
        }
        _ => {
            descendant.set("DW", Object::Integer(1000));
            descendant.set(
                "W",
                Object::Array(
                    widths
                        .iter()
                        .flat_map(|(g, w)| {
                            vec![
                                Object::Integer(*g as i64),
                                Object::Array(vec![Object::Integer(*w)]),
                            ]
                        })
                        .collect(),
                ),
            );
        }
    }
    if let Some(map) = &font.cid_to_gid {
        let compressed = {
            use std::io::Write;
            let mut enc =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            enc.write_all(map)?;
            enc.finish()?
        };
        let mut md = Dictionary::new();
        md.set("Filter", Object::Name(b"FlateDecode".into()));
        let mid = doc.add_object(Object::Stream(Stream::new(md, compressed)));
        descendant.set("CIDToGIDMap", Object::Reference(mid));
    } else {
        descendant.set("CIDToGIDMap", Object::Name(b"Identity".into()));
    }
    let descendant_id = doc.add_object(Object::Dictionary(descendant));
    let mut type0 = Dictionary::new();
    type0.set("Type", Object::Name(b"Font".into()));
    type0.set("Subtype", Object::Name(b"Type0".into()));
    type0.set("BaseFont", Object::Name(font.name.as_bytes().to_vec()));
    match &font.encoding_cmap {
        Some(cmap) => {
            let compressed = {
                use std::io::Write;
                let mut enc =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                enc.write_all(cmap)?;
                enc.finish()?
            };
            let mut cd = Dictionary::new();
            cd.set("Filter", Object::Name(b"FlateDecode".into()));
            let cid = doc.add_object(Object::Stream(Stream::new(cd, compressed)));
            type0.set("Encoding", Object::Reference(cid));
        }
        None => type0.set("Encoding", Object::Name(b"Identity-H".into())),
    }
    type0.set(
        "DescendantFonts",
        Object::Array(vec![Object::Reference(descendant_id)]),
    );
    // 不写 /ToUnicode：源 PDF 亦然。CID 即原字体的字形索引，阅读器替换字体后
    // 直接按 CID 取字形；带 ToUnicode 会改走「CID→字符→替换字体字形」，
    // 而源码位并无可用的字符映射（pdf.js/issue15977_reduced 会渲染成乱码）
    let _ = used;
    descriptions.push(font.name.clone());
    Ok(doc.add_object(Object::Dictionary(type0)))
}

/// 软掩膜表单：内容原样 + /BBox /Matrix /Group + 内部图片资源。
fn build_softmask_form(
    doc: &mut lopdf::Document,
    m: &crate::model::SoftMask,
    dir: &Path,
    images: &BTreeMap<String, (String, (u32, u16))>,
) -> Result<(u32, u16)> {
    let content = std::fs::read(dir.join(&m.content))?;
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".into()));
    d.set("Subtype", Object::Name(b"Form".into()));
    d.set("FormType", Object::Integer(1));
    if m.bbox.len() >= 4 {
        d.set(
            "BBox",
            Object::Array(m.bbox.iter().map(|v| Object::Real(*v as f32)).collect()),
        );
    }
    if m.matrix.len() >= 6 {
        d.set(
            "Matrix",
            Object::Array(m.matrix.iter().map(|v| Object::Real(*v as f32)).collect()),
        );
    }
    if let Some(cs) = &m.group_cs {
        let mut g = Dictionary::new();
        g.set("Type", Object::Name(b"Group".into()));
        g.set("S", Object::Name(b"Transparency".into()));
        g.set("CS", Object::Name(cs.as_bytes().to_vec()));
        d.set("Group", Object::Dictionary(g));
    }
    let mut res = Dictionary::new();
    if !m.images.is_empty() {
        let mut xo = Dictionary::new();
        for (name, src) in &m.images {
            if let Some((res_name, id)) = images.get(src) {
                xo.set(name.clone(), Object::Reference(*id));
                let _ = res_name;
            }
        }
        res.set("XObject", Object::Dictionary(xo));
    }
    if !m.shadings.is_empty() {
        let mut shd = Dictionary::new();
        for (name, def) in &m.shadings {
            let id = match &def.mesh {
                Some(mesh) => build_mesh_shading(doc, mesh, dir)?,
                None => build_shading(doc, def, dir)?,
            };
            shd.set(name.clone(), Object::Reference(id));
        }
        res.set("Shading", Object::Dictionary(shd));
    }
    if !m.extgstates.is_empty() {
        let mut gs = Dictionary::new();
        for (name, val) in &m.extgstates {
            let o = json_to_object(val);
            let id = doc.add_object(o);
            gs.set(name.clone(), Object::Reference(id));
        }
        res.set("ExtGState", Object::Dictionary(gs));
    }
    if !res.is_empty() {
        d.set("Resources", Object::Dictionary(res));
    }
    Ok(doc.add_object(Object::Stream(Stream::new(d, content))))
}

/// 简单字体（TrueType/OpenType/CFF）：原 /Encoding + 按码位的 /Widths 重建，
/// 文本按原始码位回写 —— 字形由字体程序 + 编码决定，与源 PDF 同构。
fn build_simple_font(doc: &mut lopdf::Document, asset: &FontAsset) -> Result<(u32, u16)> {
    let data = std::fs::read(&asset.file)?;
    let compressed = {
        use std::io::Write;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&data)?;
        enc.finish()?
    };
    let (ff_key, ff_subtype) = match asset.format {
        FontFormat::TrueType => ("FontFile2", None),
        FontFormat::OpenType => ("FontFile3", Some("OpenType")),
        FontFormat::Type1C => ("FontFile3", Some("Type1C")),
        _ => ("FontFile2", None),
    };
    let mut ff = Dictionary::new();
    ff.set("Filter", Object::Name(b"FlateDecode".into()));
    if let Some(st) = ff_subtype {
        ff.set("Subtype", Object::Name(st.as_bytes().to_vec()));
    }
    ff.set("Length1", Object::Integer(data.len() as i64));
    let ff_id = doc.add_object(Object::Stream(Stream::new(ff, compressed)));

    let m = asset.metrics.clone().unwrap_or(crate::model::FontMetrics {
        ascent: 800,
        descent: -200,
        cap_height: 700,
        italic_angle: 0.0,
        flags: 32,
        bbox: [-200, -300, 1200, 1000],
    });
    let mut fd = Dictionary::new();
    fd.set("Type", Object::Name(b"FontDescriptor".into()));
    fd.set("FontName", Object::Name(asset.name.as_bytes().to_vec()));
    fd.set("Flags", Object::Integer(m.flags));
    fd.set(
        "FontBBox",
        Object::Array(m.bbox.iter().map(|v| Object::Integer(*v)).collect()),
    );
    fd.set("ItalicAngle", Object::Real(m.italic_angle as f32));
    fd.set("Ascent", Object::Integer(m.ascent));
    fd.set("Descent", Object::Integer(m.descent));
    fd.set("CapHeight", Object::Integer(m.cap_height));
    fd.set("StemV", Object::Real(80.0));
    fd.set(ff_key, Object::Reference(ff_id));
    let fd_id = doc.add_object(Object::Dictionary(fd));

    // /Widths 按码位（源 PDF 声明值优先，缺省 0）
    let (first, last) = (
        asset.widths.keys().min().copied().unwrap_or(32) as i64,
        asset.widths.keys().max().copied().unwrap_or(255) as i64,
    );
    let widths: Vec<Object> = (first..=last)
        .map(|c| Object::Integer(asset.widths.get(&(c as u16)).copied().unwrap_or(0)))
        .collect();
    let mut f = Dictionary::new();
    f.set("Type", Object::Name(b"Font".into()));
    f.set("Subtype", Object::Name(b"TrueType".into()));
    f.set("BaseFont", Object::Name(asset.name.as_bytes().to_vec()));
    f.set("FirstChar", Object::Integer(first));
    f.set("LastChar", Object::Integer(last));
    f.set("Widths", Object::Array(widths));
    match &asset.encoding_differences {
        Some(diff) => {
            let mut ed = Dictionary::new();
            ed.set("Type", Object::Name(b"Encoding".into()));
            ed.set(
                "BaseEncoding",
                Object::Name(
                    asset
                        .encoding
                        .as_deref()
                        .unwrap_or("WinAnsiEncoding")
                        .as_bytes()
                        .to_vec(),
                ),
            );
            ed.set("Differences", json_to_object(diff));
            f.set("Encoding", Object::Dictionary(ed));
        }
        None => f.set(
            "Encoding",
            Object::Name(
                asset
                    .encoding
                    .as_deref()
                    .unwrap_or("WinAnsiEncoding")
                    .as_bytes()
                    .to_vec(),
            ),
        ),
    }
    f.set("FontDescriptor", Object::Reference(fd_id));
    Ok(doc.add_object(Object::Dictionary(f)))
}

/// FunctionType 0 采样函数：参数 + 采样码流原样写回（/Filter /FlateDecode）。
fn build_func0(
    doc: &mut lopdf::Document,
    f0: &crate::model::Func0Data,
    dir: &Path,
) -> Result<(u32, u16)> {
    let raw = std::fs::read(dir.join(&f0.data))?;
    if raw.is_empty() {
        bail!("empty func0 data");
    }
    let compressed = {
        use std::io::Write;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&raw)?;
        enc.finish()?
    };
    let reals = |v: &[f64]| -> Object {
        Object::Array(v.iter().map(|x| Object::Real(*x as f32)).collect())
    };
    let mut fd = Dictionary::new();
    fd.set("FunctionType", Object::Integer(0));
    fd.set(
        "Size",
        Object::Array(f0.size.iter().map(|v| Object::Integer(*v)).collect()),
    );
    fd.set("BitsPerSample", Object::Integer(f0.bits_per_sample));
    fd.set("Order", Object::Integer(1));
    fd.set(
        "Domain",
        Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
    );
    if !f0.range.is_empty() {
        fd.set("Range", reals(&f0.range));
    }
    if !f0.encode.is_empty() {
        fd.set("Encode", reals(&f0.encode));
    }
    if !f0.decode.is_empty() {
        fd.set("Decode", reals(&f0.decode));
    }
    fd.set("Filter", Object::Name(b"FlateDecode".into()));
    Ok(doc.add_object(Object::Stream(Stream::new(fd, compressed))))
}

fn build_shading(doc: &mut lopdf::Document, def: &ShadingDef, dir: &Path) -> Result<(u32, u16)> {
    let mut d = Dictionary::new();
    d.set(
        "ShadingType",
        Object::Integer(if def.kind == "radial" { 3 } else { 2 }),
    );
    d.set("ColorSpace", Object::Name(b"DeviceRGB".into()));
    d.set(
        "Coords",
        Object::Array(def.coords.iter().map(|v| Object::Real(*v as f32)).collect()),
    );
    if !def.extend.is_empty() {
        d.set(
            "Extend",
            Object::Array(def.extend.iter().map(|v| Object::Boolean(*v)).collect()),
        );
    }
    // FunctionType 0 直通：采样码流原样写回（按色标抽样会漏条纹）
    if let Some(f0) = &def.func0 {
        if let Ok(fid) = build_func0(doc, f0, dir) {
            d.set("Function", Object::Reference(fid));
            return Ok(doc.add_object(Object::Dictionary(d)));
        }
    }
    // 函数：2 个色标用 type 2，多色标用 type 3 拼接
    let stops = &def.stops;
    let rgb = |hex: &str| -> Vec<Object> {
        let (r, g, b) = color_operands(hex).unwrap_or((0.0, 0.0, 0.0));
        vec![Object::Real(r), Object::Real(g), Object::Real(b)]
    };
    let func = if stops.len() <= 2 {
        let c0 = stops
            .first()
            .map(|s| rgb(&s.color))
            .unwrap_or_else(|| vec![Object::Real(0.0); 3]);
        let c1 = stops
            .last()
            .map(|s| rgb(&s.color))
            .unwrap_or_else(|| vec![Object::Real(1.0); 3]);
        let mut f = Dictionary::new();
        f.set("FunctionType", Object::Integer(2));
        f.set(
            "Domain",
            Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
        );
        f.set("C0", Object::Array(c0));
        f.set("C1", Object::Array(c1));
        f.set("N", Object::Integer(1));
        doc.add_object(Object::Dictionary(f))
    } else {
        let mut subs: Vec<Object> = Vec::new();
        for pair in stops.windows(2) {
            let mut f = Dictionary::new();
            f.set("FunctionType", Object::Integer(2));
            f.set(
                "Domain",
                Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
            );
            f.set("C0", Object::Array(rgb(&pair[0].color)));
            f.set("C1", Object::Array(rgb(&pair[1].color)));
            f.set("N", Object::Integer(1));
            subs.push(Object::Reference(doc.add_object(Object::Dictionary(f))));
        }
        let bounds: Vec<Object> = stops[1..stops.len() - 1]
            .iter()
            .map(|s| Object::Real(s.offset as f32))
            .collect();
        let encode: Vec<Object> = (0..subs.len())
            .flat_map(|_| [Object::Real(0.0), Object::Real(1.0)])
            .collect();
        let mut f = Dictionary::new();
        f.set("FunctionType", Object::Integer(3));
        f.set(
            "Domain",
            Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
        );
        f.set("Functions", Object::Array(subs));
        f.set("Bounds", Object::Array(bounds));
        f.set("Encode", Object::Array(encode));
        doc.add_object(Object::Dictionary(f))
    };
    d.set("Function", Object::Reference(func));
    Ok(doc.add_object(Object::Dictionary(d)))
}

// ── 图片嵌入 ──────────────────────────────────────────────────────────────

/// 小写滤镜名 → PDF 规范名（滤镜名大小写敏感）。
fn canonical_filter(name: &str) -> String {
    match name.to_lowercase().as_str() {
        "flatedecode" => "FlateDecode",
        "lzwdecode" => "LZWDecode",
        "asciihexdecode" => "ASCIIHexDecode",
        "ascii85decode" => "ASCII85Decode",
        "runlengthdecode" => "RunLengthDecode",
        "dctdecode" => "DCTDecode",
        "jpxdecode" => "JPXDecode",
        "jbig2decode" => "JBIG2Decode",
        "ccittfaxdecode" => "CCITTFaxDecode",
        "crypt" => "Crypt",
        _other => return name.to_string(),
    }
    .to_string()
}

/// 简单 Type1 字体（裸 CFF）：/Subtype /Type1 + FontFile3 /Type1C +
/// 原 /Widths 与 /Encoding。文本按 WinAnsi 字节写出（源字体为 WinAnsi 编码）。
fn build_type1_font(doc: &mut lopdf::Document, asset: &FontAsset) -> Result<(u32, u16)> {
    let data = std::fs::read(&asset.file)?;
    let compressed = {
        use std::io::Write;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&data)?;
        enc.finish()?
    };
    let is_type1 = asset.format == FontFormat::Type1;
    let mut ff = Dictionary::new();
    ff.set("Filter", Object::Name(b"FlateDecode".into()));
    if is_type1 {
        // 真正的 Type1 程序（PFA/PFB）：/Length1 为明文段长度，缺省取全长
        ff.set(
            "Length1",
            Object::Integer(asset.length1.unwrap_or(data.len() as i64)),
        );
    } else {
        ff.set("Subtype", Object::Name(b"Type1C".into()));
        ff.set("Length1", Object::Integer(data.len() as i64));
    }
    let ff_id = doc.add_object(Object::Stream(Stream::new(ff, compressed)));

    let m = asset.metrics.clone().unwrap_or(crate::model::FontMetrics {
        ascent: 800,
        descent: -200,
        cap_height: 700,
        italic_angle: 0.0,
        flags: 32,
        bbox: [-200, -300, 1200, 1000],
    });
    let mut fd = Dictionary::new();
    fd.set("Type", Object::Name(b"FontDescriptor".into()));
    fd.set("FontName", Object::Name(asset.name.as_bytes().to_vec()));
    fd.set("Flags", Object::Integer(m.flags));
    fd.set(
        "FontBBox",
        Object::Array(m.bbox.iter().map(|v| Object::Integer(*v)).collect()),
    );
    fd.set("ItalicAngle", Object::Real(m.italic_angle as f32));
    fd.set("Ascent", Object::Integer(m.ascent));
    fd.set("Descent", Object::Integer(m.descent));
    fd.set("CapHeight", Object::Integer(m.cap_height));
    fd.set("StemV", Object::Real(80.0));
    if is_type1 {
        fd.set("FontFile", Object::Reference(ff_id));
    } else {
        fd.set("FontFile3", Object::Reference(ff_id));
    }
    let fd_id = doc.add_object(Object::Dictionary(fd));

    let first = asset.widths.keys().min().copied().unwrap_or(32);
    let last = asset.widths.keys().max().copied().unwrap_or(255);
    let widths: Vec<Object> = (first..=last)
        .map(|c| Object::Integer(asset.widths.get(&c).copied().unwrap_or(0)))
        .collect();

    let mut f = Dictionary::new();
    f.set("Type", Object::Name(b"Font".into()));
    f.set("Subtype", Object::Name(b"Type1".into()));
    f.set("BaseFont", Object::Name(asset.name.as_bytes().to_vec()));
    f.set("FirstChar", Object::Integer(first as i64));
    f.set("LastChar", Object::Integer(last as i64));
    f.set("Widths", Object::Array(widths));
    // /Encoding：有 /Differences 时回写成字典，否则用基名
    match &asset.encoding_differences {
        Some(diff) => {
            let mut ed = Dictionary::new();
            ed.set("Type", Object::Name(b"Encoding".into()));
            ed.set(
                "BaseEncoding",
                Object::Name(
                    asset
                        .encoding
                        .as_deref()
                        .unwrap_or("StandardEncoding")
                        .as_bytes()
                        .to_vec(),
                ),
            );
            ed.set("Differences", json_to_object(diff));
            f.set("Encoding", Object::Dictionary(ed));
        }
        None => f.set(
            "Encoding",
            Object::Name(
                asset
                    .encoding
                    .as_deref()
                    .unwrap_or("WinAnsiEncoding")
                    .as_bytes()
                    .to_vec(),
            ),
        ),
    }
    f.set("FontDescriptor", Object::Reference(fd_id));
    Ok(doc.add_object(Object::Dictionary(f)))
}

/// Type3 直通：CharProcs 码流 + 图片 XObject 资源 + 原编码/宽度。
fn build_type3_font(
    doc: &mut lopdf::Document,
    asset: &FontAsset,
    t3: &crate::model::Type3Data,
    images: &BTreeMap<String, (String, (u32, u16))>,
    dir: &Path,
) -> Result<(u32, u16)> {
    // CharProcs：字形名 → 码流（路径相对产物根）
    let mut procs = Dictionary::new();
    for (name, rel) in &t3.charprocs {
        let data =
            std::fs::read(dir.join(rel)).map_err(|e| anyhow::anyhow!("charproc {name}: {e}"))?;
        let mut d = Dictionary::new();
        d.set("Length", Object::Integer(data.len() as i64));
        procs.set(
            name.as_bytes().to_vec(),
            Object::Reference(doc.add_object(Object::Stream(Stream::new(d, data)))),
        );
    }
    // 资源：图片 XObject
    let mut resources = Dictionary::new();
    if !t3.xobjects.is_empty() {
        let mut xo = Dictionary::new();
        for (name, src) in &t3.xobjects {
            if let Some((_, id)) = images.get(src) {
                xo.set(name.as_bytes().to_vec(), Object::Reference(*id));
            }
        }
        resources.set("XObject", Object::Dictionary(xo));
    }
    // 编码：/Differences
    let mut encoding = Dictionary::new();
    encoding.set("Type", Object::Name(b"Encoding".into()));
    let mut diffs: Vec<Object> = Vec::new();
    let mut prev: Option<u16> = None;
    for (code, name) in &t3.differences {
        if prev.map(|p| p + 1 != *code).unwrap_or(true) {
            diffs.push(Object::Integer(*code as i64));
        }
        diffs.push(Object::Name(name.as_bytes().to_vec()));
        prev = Some(*code);
    }
    if !diffs.is_empty() {
        encoding.set("Differences", Object::Array(diffs));
    }
    let mut f = Dictionary::new();
    f.set("Type", Object::Name(b"Font".into()));
    f.set("Subtype", Object::Name(b"Type3".into()));
    f.set("Name", Object::Name(asset.name.as_bytes().to_vec()));
    f.set(
        "FontMatrix",
        Object::Array(t3.matrix.iter().map(|v| Object::Real(*v as f32)).collect()),
    );
    f.set(
        "FontBBox",
        Object::Array(t3.bbox.iter().map(|v| Object::Integer(*v)).collect()),
    );
    f.set("CharProcs", Object::Dictionary(procs));
    f.set("Encoding", Object::Dictionary(encoding));
    f.set("FirstChar", Object::Integer(t3.first_char));
    f.set("LastChar", Object::Integer(t3.last_char));
    f.set(
        "Widths",
        Object::Array(t3.widths.iter().map(|w| Object::Integer(*w)).collect()),
    );
    f.set("Resources", Object::Dictionary(resources));
    Ok(doc.add_object(Object::Dictionary(f)))
}

/// 网格渐变（ShadingType 4-7）：原码流 + 原参数直通。
fn build_mesh_shading(
    doc: &mut lopdf::Document,
    mesh: &crate::model::MeshShading,
    dir: &Path,
) -> Result<(u32, u16)> {
    let raw = std::fs::read(dir.join(&mesh.data))?;
    let compressed = {
        use std::io::Write;
        let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&raw)?;
        enc.finish()?
    };
    let mut d = Dictionary::new();
    d.set("ShadingType", Object::Integer(mesh.shading_type));
    // 分量数必须与源一致：固定 DeviceRGB 会让 CMYK/灰度网格的顶点流错位
    d.set(
        "ColorSpace",
        Object::Name(mesh.color_space.as_bytes().to_vec()),
    );
    d.set(
        "BitsPerCoordinate",
        Object::Integer(mesh.bits_per_coordinate),
    );
    d.set("BitsPerComponent", Object::Integer(mesh.bits_per_component));
    d.set("BitsPerFlag", Object::Integer(mesh.bits_per_flag));
    if !mesh.decode.is_empty() {
        d.set(
            "Decode",
            Object::Array(
                mesh.decode
                    .iter()
                    .map(|v| Object::Real(*v as f32))
                    .collect(),
            ),
        );
    }
    if let Some(bg) = &mesh.background {
        d.set(
            "Background",
            Object::Array(bg.iter().map(|v| Object::Real(*v as f32)).collect()),
        );
    }
    // 着色函数（字典直写；流则嵌入码流 + 原字典）
    if let Some(f) = &mesh.func {
        d.set("Function", json_to_object(f));
    }
    if let Some(fs) = &mesh.func_stream {
        if let Ok(raw) = std::fs::read(dir.join(&fs.data)) {
            let compressed = {
                use std::io::Write;
                let mut enc =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                enc.write_all(&raw)?;
                enc.finish()?
            };
            let mut fd = match json_to_object(&fs.dict) {
                Object::Dictionary(x) => x,
                _ => Dictionary::new(),
            };
            fd.set("Filter", Object::Name(b"FlateDecode".into()));
            let fid = doc.add_object(Object::Stream(Stream::new(fd, compressed)));
            d.set("Function", Object::Reference(fid));
        }
    }
    d.set("Filter", Object::Name(b"FlateDecode".into()));
    Ok(doc.add_object(Object::Stream(Stream::new(d, compressed))))
}

/// DeviceGray 渐变（亮度软掩膜用；取色标的 R 分量作为灰度）。
fn build_shading_gray(doc: &mut lopdf::Document, def: &ShadingDef) -> Result<(u32, u16)> {
    let mut d = Dictionary::new();
    d.set(
        "ShadingType",
        Object::Integer(if def.kind == "radial" { 3 } else { 2 }),
    );
    d.set("ColorSpace", Object::Name(b"DeviceGray".into()));
    d.set(
        "Coords",
        Object::Array(def.coords.iter().map(|v| Object::Real(*v as f32)).collect()),
    );
    if !def.extend.is_empty() {
        d.set(
            "Extend",
            Object::Array(def.extend.iter().map(|v| Object::Boolean(*v)).collect()),
        );
    }
    let gray = |hex: &str| -> Vec<Object> {
        let (r, _, _) = color_operands(hex).unwrap_or((0.0, 0.0, 0.0));
        vec![Object::Real(r)]
    };
    let stops = &def.stops;
    let func = if stops.len() <= 2 {
        let c0 = stops
            .first()
            .map(|s| gray(&s.color))
            .unwrap_or_else(|| vec![Object::Real(0.0)]);
        let c1 = stops
            .last()
            .map(|s| gray(&s.color))
            .unwrap_or_else(|| vec![Object::Real(1.0)]);
        let mut f = Dictionary::new();
        f.set("FunctionType", Object::Integer(2));
        f.set(
            "Domain",
            Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
        );
        f.set("C0", Object::Array(c0));
        f.set("C1", Object::Array(c1));
        f.set("N", Object::Integer(1));
        doc.add_object(Object::Dictionary(f))
    } else {
        let mut subs: Vec<Object> = Vec::new();
        for pair in stops.windows(2) {
            let mut f = Dictionary::new();
            f.set("FunctionType", Object::Integer(2));
            f.set(
                "Domain",
                Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
            );
            f.set("C0", Object::Array(gray(&pair[0].color)));
            f.set("C1", Object::Array(gray(&pair[1].color)));
            f.set("N", Object::Integer(1));
            subs.push(Object::Reference(doc.add_object(Object::Dictionary(f))));
        }
        let bounds: Vec<Object> = stops[1..stops.len() - 1]
            .iter()
            .map(|s| Object::Real(s.offset as f32))
            .collect();
        let encode: Vec<Object> = (0..subs.len())
            .flat_map(|_| [Object::Real(0.0), Object::Real(1.0)])
            .collect();
        let mut f = Dictionary::new();
        f.set("FunctionType", Object::Integer(3));
        f.set(
            "Domain",
            Object::Array(vec![Object::Real(0.0), Object::Real(1.0)]),
        );
        f.set("Functions", Object::Array(subs));
        f.set("Bounds", Object::Array(bounds));
        f.set("Encode", Object::Array(encode));
        doc.add_object(Object::Dictionary(f))
    };
    d.set("Function", Object::Reference(func));
    Ok(doc.add_object(Object::Dictionary(d)))
}

/// 亮度软掩膜：DeviceGray 表单（内容 `q /ShM sh Q`）+ ExtGState
/// `/SMask << /S /Luminosity /G form >>`，可选叠加元素自身的 ca/CA。
fn build_mask_extgstate(
    doc: &mut lopdf::Document,
    mask: &ShadingDef,
    fill_alpha: f64,
    stroke_alpha: f64,
) -> Result<(u32, u16)> {
    let sh_id = build_shading_gray(doc, mask)?;
    let bbox = mask.bbox.unwrap_or([0.0, 0.0, 1.0, 1.0]);
    let mut sh_dict = Dictionary::new();
    sh_dict.set("ShM", Object::Reference(sh_id));
    let mut res = Dictionary::new();
    res.set("Shading", Object::Dictionary(sh_dict));

    let content = b"q /ShM sh Q".to_vec();
    let mut fd = Dictionary::new();
    fd.set("Type", Object::Name(b"XObject".into()));
    fd.set("Subtype", Object::Name(b"Form".into()));
    fd.set("FormType", Object::Integer(1));
    fd.set(
        "BBox",
        Object::Array(bbox.iter().map(|v| Object::Real(*v as f32)).collect()),
    );
    fd.set(
        "Matrix",
        Object::Array(
            [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
                .iter()
                .map(|v| Object::Real(*v as f32))
                .collect(),
        ),
    );
    let mut group = Dictionary::new();
    group.set("Type", Object::Name(b"Group".into()));
    group.set("S", Object::Name(b"Transparency".into()));
    group.set("CS", Object::Name(b"DeviceGray".into()));
    fd.set("Group", Object::Dictionary(group));
    fd.set("Resources", Object::Dictionary(res));
    fd.set("Length", Object::Integer(content.len() as i64));
    let form_id = doc.add_object(Object::Stream(Stream::new(fd, content)));

    let mut sm = Dictionary::new();
    sm.set("S", Object::Name(b"Luminosity".into()));
    sm.set("G", Object::Reference(form_id));
    let mut gs = Dictionary::new();
    gs.set("Type", Object::Name(b"ExtGState".into()));
    gs.set("SMask", Object::Dictionary(sm));
    if (fill_alpha - 1.0).abs() > 1e-9 {
        gs.set("ca", Object::Real(fill_alpha as f32));
    }
    if (stroke_alpha - 1.0).abs() > 1e-9 {
        gs.set("CA", Object::Real(stroke_alpha as f32));
    }
    Ok(doc.add_object(Object::Dictionary(gs)))
}

/// 把 SMask 的 /Matte 写回其流字典（1-4 分量预乘底色）。
fn apply_smask_matte(doc: &mut lopdf::Document, sid: (u32, u16), pt: Option<&ImagePassthrough>) {
    let Some(matte) = pt.and_then(|p| p.matte.as_ref()) else {
        return;
    };
    if matte.is_empty() {
        return;
    }
    if let Ok(Object::Stream(st)) = doc.get_object_mut(sid) {
        st.dict.set(
            "Matte",
            Object::Array(matte.iter().map(|v| Object::Real(*v as f32)).collect()),
        );
    }
}

/// 按原滤镜链直通嵌入（码流原样保留）。
fn embed_passthrough(
    doc: &mut lopdf::Document,
    src: &Path,
    pt: &ImagePassthrough,
    smask: Option<(u32, u16)>,
    root: &Path,
) -> Result<(u32, u16)> {
    let data = std::fs::read(src)?;
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".into()));
    d.set("Subtype", Object::Name(b"Image".into()));
    // 宽高缺失时尝试从码流解析
    let (mut w, mut h) = (pt.width, pt.height);
    if (w <= 0 || h <= 0) && pt.filter.iter().any(|f| f == "DCTDecode") {
        if let Some((jw, jh, _)) = jpeg_dimensions(&data) {
            w = jw;
            h = jh;
        }
    }
    if (w <= 0 || h <= 0) && pt.filter.iter().any(|f| f == "JPXDecode") {
        if let Some((jw, jh)) = jp2_dimensions(&data) {
            w = jw;
            h = jh;
        }
    }
    if w <= 0 || h <= 0 {
        // 尺寸不可知（畸形字典）→ 用 PNG 解码器兜底，再退回占位图
        if let Ok(id) = embed_png(doc, src, &mut Vec::new()) {
            return Ok(id);
        }
        bail!(
            "image {}: cannot determine dimensions for passthrough",
            src.display()
        );
    }
    d.set("Width", Object::Integer(w));
    d.set("Height", Object::Integer(h));
    let cs = if pt.icc_components == 1 {
        "DeviceGray".to_string()
    } else if pt.icc_components == 3 {
        "DeviceRGB".to_string()
    } else if pt.icc_components == 4 {
        "DeviceCMYK".to_string()
    } else {
        pt.color_space.clone()
    };
    if !pt.image_mask {
        d.set("ColorSpace", Object::Name(cs.as_bytes().to_vec()));
        d.set("BitsPerComponent", Object::Integer(pt.bpc));
    } else {
        d.set("ImageMask", Object::Boolean(true));
    }
    if let Some(dec) = &pt.decode {
        d.set(
            "Decode",
            Object::Array(dec.iter().map(|v| Object::Real(*v as f32)).collect()),
        );
    }
    // 滤镜链
    let names: Vec<Object> = pt
        .filter
        .iter()
        .map(|f| Object::Name(canonical_filter(f).into_bytes()))
        .collect();
    if names.len() == 1 {
        d.set("Filter", names.into_iter().next().unwrap());
    } else if !names.is_empty() {
        d.set("Filter", Object::Array(names));
    }
    let mut parms: Vec<Object> = pt.decode_parms.iter().map(json_to_object).collect();
    // JBIG2 全局段：重新嵌入为独立码流并挂回 DecodeParms
    if let Some(grel) = &pt.globals {
        if let Ok(gpath) = resolve_src(root, grel) {
            if let Ok(gdata) = std::fs::read(&gpath) {
                let gid = doc.add_object(Object::Stream(Stream::new(Dictionary::new(), gdata)));
                if parms.is_empty() {
                    parms.push(Object::Null);
                }
                if let Some(Object::Dictionary(pd)) = parms.first_mut() {
                    pd.set("JBIG2Globals", Object::Reference(gid));
                } else {
                    let mut pd = Dictionary::new();
                    pd.set("JBIG2Globals", Object::Reference(gid));
                    parms[0] = Object::Dictionary(pd);
                }
            }
        }
    }
    if parms.len() == 1 {
        d.set("DecodeParms", parms.into_iter().next().unwrap());
    } else if !parms.is_empty() {
        d.set("DecodeParms", Object::Array(parms));
    }
    if let Some(smask) = smask {
        d.set("SMask", Object::Reference(smask));
    }
    Ok(doc.add_object(Object::Stream(Stream::new(d, data))))
}

/// 按扩展名选择嵌入方式（供 SMask 复用）；失败时回退占位图并告警。
fn embed_image_file(
    doc: &mut lopdf::Document,
    path: &Path,
    pt: Option<&ImagePassthrough>,
    warnings: &mut Vec<String>,
    root: &Path,
) -> (u32, u16) {
    if let Some(pt) = pt {
        if let Ok(id) = embed_passthrough(doc, path, pt, None, root) {
            return id;
        }
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let r = match ext.as_str() {
        "png" => embed_png(doc, path, warnings),
        "jpg" | "jpeg" => embed_jpeg(doc, path),
        "jp2" | "jpx" => embed_passthrough(
            doc,
            path,
            &ImagePassthrough {
                filter: vec!["JPXDecode".into()],
                decode_parms: Vec::new(),
                color_space: "DeviceRGB".into(),
                icc_components: 0,
                bpc: 8,
                width: 0,
                height: 0,
                decode: None,
                image_mask: false,
                smask: None,
                smask_params: None,
                matte: None,
                globals: None,
            },
            None,
            root,
        ),
        _ => {
            warnings.push(format!(
                "smask {}: unsupported extension .{ext}",
                path.display()
            ));
            return placeholder_image(doc).unwrap_or((0, 0));
        }
    };
    match r {
        Ok(id) => id,
        Err(e) => {
            warnings.push(format!("smask {}: {e} — placeholder used", path.display()));
            placeholder_image(doc).unwrap_or((0, 0))
        }
    }
}

/// serde_json::Value → lopdf::Object（仅直通 DecodeParms 用到的子集）。
fn json_to_object(v: &serde_json::Value) -> Object {
    match v {
        serde_json::Value::Null => Object::Null,
        serde_json::Value::Bool(b) => Object::Boolean(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Object::Integer(i)
            } else {
                Object::Real(n.as_f64().unwrap_or(0.0) as f32)
            }
        }
        serde_json::Value::String(s) => Object::Name(s.as_bytes().to_vec()),
        serde_json::Value::Array(a) => Object::Array(a.iter().map(json_to_object).collect()),
        serde_json::Value::Object(o) => {
            let mut d = Dictionary::new();
            for (k, val) in o {
                d.set(k.clone(), json_to_object(val));
            }
            Object::Dictionary(d)
        }
    }
}

/// JPEG 原样嵌入（DCTDecode）；尺寸从 SOF 标记解析。
fn embed_jpeg(doc: &mut lopdf::Document, src: &Path) -> Result<(u32, u16)> {
    let data = std::fs::read(src)?;
    let (w, h, comps) = jpeg_dimensions(&data)
        .ok_or_else(|| anyhow::anyhow!("not a valid JPEG: {}", src.display()))?;
    let cs = match comps {
        1 => "DeviceGray",
        3 => "DeviceRGB",
        4 => "DeviceCMYK",
        _ => bail!("unsupported JPEG component count {comps}"),
    };
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".into()));
    d.set("Subtype", Object::Name(b"Image".into()));
    d.set("Width", Object::Integer(w));
    d.set("Height", Object::Integer(h));
    d.set("ColorSpace", Object::Name(cs.as_bytes().to_vec()));
    d.set("BitsPerComponent", Object::Integer(8));
    d.set("Filter", Object::Name(b"DCTDecode".into()));
    if cs == "DeviceCMYK" {
        // Adobe CMYK JPEG 反相
        d.set(
            "Decode",
            Object::Array(vec![
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(0.0),
                Object::Real(1.0),
                Object::Real(0.0),
            ]),
        );
    }
    Ok(doc.add_object(Object::Stream(Stream::new(d, data))))
}

/// 最小 JPEG 扫描：SOF0/1/2 取宽高与分量数。
fn jpeg_dimensions(data: &[u8]) -> Option<(i64, i64, u8)> {
    let mut i = 2usize;
    while i + 9 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = data[i + 1];
        if (0xD8..=0xD9).contains(&marker) || marker == 0x01 {
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        if (0xC0..=0xC2).contains(&marker) {
            let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as i64;
            let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as i64;
            return Some((w, h, data[i + 9]));
        }
        i += 2 + len;
    }
    None
}

/// JP2 尺寸：扫描 ihdr box（宽高在 box 内 8/12 字节处）。
fn jp2_dimensions(data: &[u8]) -> Option<(i64, i64)> {
    let mut i = 0usize;
    while i + 8 <= data.len() {
        let len = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        let tag = &data[i + 4..i + 8];
        if tag == b"ihdr" && i + 16 <= data.len() {
            let h =
                u32::from_be_bytes([data[i + 8], data[i + 9], data[i + 10], data[i + 11]]) as i64;
            let w =
                u32::from_be_bytes([data[i + 12], data[i + 13], data[i + 14], data[i + 15]]) as i64;
            return Some((w, h));
        }
        if len < 8 {
            break;
        }
        i += len;
    }
    None
}

/// PNG 解码 → 像素流（RGB/Gray；带 alpha 拆 SMask）。
fn embed_png(
    doc: &mut lopdf::Document,
    src: &Path,
    warnings: &mut Vec<String>,
) -> Result<(u32, u16)> {
    let data = std::fs::read(src)?;
    let decoder = png::Decoder::new(std::io::Cursor::new(&data));
    let mut reader = decoder
        .read_info()
        .map_err(|e| anyhow::anyhow!("png {}: {e}", src.display()))?;
    let info = reader.info().clone();
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let frame = reader
        .next_frame(&mut buf)
        .map_err(|e| anyhow::anyhow!("png {}: {e}", src.display()))?;
    let (w, h) = (info.width as i64, info.height as i64);
    let pixels_len = buf.len();
    let pixels = buf[..pixels_len.min(buf.len())].to_vec();

    use png::{BitDepth, ColorType};
    match (frame.color_type, frame.bit_depth) {
        (ColorType::Rgb, BitDepth::Eight) => embed_pixels(doc, w, h, 3, &pixels, None),
        (ColorType::Grayscale, BitDepth::Eight) => embed_pixels(doc, w, h, 1, &pixels, None),
        (ColorType::Rgba, BitDepth::Eight) => {
            let n = (w * h) as usize;
            let mut rgb = Vec::with_capacity(n * 3);
            let mut alpha = Vec::with_capacity(n);
            for px in pixels.chunks_exact(4) {
                rgb.extend_from_slice(&px[..3]);
                alpha.push(px[3]);
            }
            let smask = embed_smask(doc, w, h, &alpha)?;
            embed_pixels(doc, w, h, 3, &rgb, Some(smask))
        }
        (ColorType::GrayscaleAlpha, BitDepth::Eight) => {
            let n = (w * h) as usize;
            let mut gray = Vec::with_capacity(n);
            let mut alpha = Vec::with_capacity(n);
            for px in pixels.chunks_exact(2) {
                gray.push(px[0]);
                alpha.push(px[1]);
            }
            let smask = embed_smask(doc, w, h, &alpha)?;
            embed_pixels(doc, w, h, 1, &gray, Some(smask))
        }
        (ct, bd) => {
            warnings.push(format!(
                "png {src:?} uses {ct:?}/{bd:?} — not embeddable, replaced by placeholder"
            ));
            placeholder_image(doc)
        }
    }
}

fn embed_pixels(
    doc: &mut lopdf::Document,
    w: i64,
    h: i64,
    comps: u8,
    pixels: &[u8],
    smask: Option<(u32, u16)>,
) -> Result<(u32, u16)> {
    use std::io::Write;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(pixels)?;
    let compressed = enc.finish()?;
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".into()));
    d.set("Subtype", Object::Name(b"Image".into()));
    d.set("Width", Object::Integer(w));
    d.set("Height", Object::Integer(h));
    d.set(
        "ColorSpace",
        Object::Name(
            if comps == 1 {
                b"DeviceGray".as_slice()
            } else {
                b"DeviceRGB"
            }
            .into(),
        ),
    );
    d.set("BitsPerComponent", Object::Integer(8));
    d.set("Filter", Object::Name(b"FlateDecode".into()));
    if let Some(smask) = smask {
        d.set("SMask", Object::Reference(smask));
    }
    Ok(doc.add_object(Object::Stream(Stream::new(d, compressed))))
}

fn embed_smask(doc: &mut lopdf::Document, w: i64, h: i64, alpha: &[u8]) -> Result<(u32, u16)> {
    use std::io::Write;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(alpha)?;
    let compressed = enc.finish()?;
    let mut d = Dictionary::new();
    d.set("Type", Object::Name(b"XObject".into()));
    d.set("Subtype", Object::Name(b"Image".into()));
    d.set("Width", Object::Integer(w));
    d.set("Height", Object::Integer(h));
    d.set("ColorSpace", Object::Name(b"DeviceGray".into()));
    d.set("BitsPerComponent", Object::Integer(8));
    d.set("Filter", Object::Name(b"FlateDecode".into()));
    Ok(doc.add_object(Object::Stream(Stream::new(d, compressed))))
}

/// 不可嵌入图片的占位：1x1 白图。
fn placeholder_image(doc: &mut lopdf::Document) -> Result<(u32, u16)> {
    embed_pixels(doc, 1, 1, 1, &[255u8], None)
}

// ── 内容流生成 ────────────────────────────────────────────────────────────

fn color_operands(hex: &str) -> Option<(f32, f32, f32)> {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&h[0..2], 16).ok()? as f32 / 255.0,
        u8::from_str_radix(&h[2..4], 16).ok()? as f32 / 255.0,
        u8::from_str_radix(&h[4..6], 16).ok()? as f32 / 255.0,
    ))
}

fn fmt(v: f32) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[allow(clippy::too_many_arguments)]
fn build_content(
    page: &PageModel,
    fonts: &BTreeMap<String, FontEntry>,
    images: &BTreeMap<String, (String, (u32, u16))>,
    gs_names: &BTreeMap<GsKey, String>,
    mask_names: &BTreeMap<usize, String>,
    form_names: &BTreeMap<usize, String>,
    masks: &[crate::model::SoftMask],
    warnings: &mut Vec<String>,
) -> Result<Vec<u8>> {
    let mut s = String::new();
    let mut pattern_no = 0;
    let mut shading_no = 0;
    let mut open_clips = 0usize;
    for (idx, el) in page.elements.iter().enumerate() {
        let mut extra: Vec<String> = Vec::new();
        if matches!(el, Element::PatternRect { .. }) {
            pattern_no += 1;
            extra.push(format!("P{pattern_no}"));
        }
        if matches!(el, Element::Shading(_)) {
            shading_no += 1;
            extra.push(format!("Sh{shading_no}"));
        }
        if matches!(el, Element::Group { .. }) {
            // 组 → 表单名（缺失时 emit_element 会跳过该组）
            if let Some(n) = form_names.get(&idx) {
                extra.push(n.clone());
            }
        }
        emit_element(
            &mut s, el, idx, fonts, images, gs_names, mask_names, masks, &extra, warnings, false,
        );
        // Tr 7 文本未闭合的 q：记录数量
        if matches!(el, Element::Text { render_mode: 7, .. }) {
            open_clips += 1;
        }
    }
    for _ in 0..open_clips {
        s.push_str("Q\n");
    }
    Ok(s.into_bytes())
}

/// 递归收集 ExtGState 键（元素 + 组，组自身也要 ca/CA/BM）。
fn collect_gs_keys(elements: &[Element], masks: &[crate::model::SoftMask], out: &mut Vec<GsKey>) {
    for el in elements {
        let key = element_gs_key_idx(el, masks);
        let (a, sa, b) = (&key.0, &key.1, &key.2);
        // 有软掩膜的键即使 alpha=1/无混合也必须建 ExtGState
        if ((*a - 1_000_000).abs() > 1
            || (*sa - 1_000_000).abs() > 1
            || b.is_some()
            || key.3.is_some())
            && !out.contains(&key)
        {
            out.push(key);
        }
        if let Element::Group { children, .. } = el {
            collect_gs_keys(children, masks, out);
        }
    }
}

/// 每层元素树的软掩膜信息（索引 → 名字 / 待建定义）。
#[derive(Default)]
struct LevelMasks {
    /// 该层元素索引 → ExtGState 名
    mask_names: BTreeMap<usize, String>,
    /// 待创建的掩膜 ExtGState（名字, 定义, ca, CA）
    mask_defs: Vec<(usize, String, Box<ShadingDef>, f64, f64)>,
    /// 该层组元素索引 → 子层信息
    children: BTreeMap<usize, LevelMasks>,
}

/// 递归扫描一层元素：分配全局 GSM 编号并记录各层掩膜映射。
/// 收集元素树里的软掩膜（去重，供 ExtGState 复用）。
fn collect_masks(elements: &[Element], out: &mut Vec<crate::model::SoftMask>) {
    for el in elements {
        if let Some(m) = element_smask(el) {
            if !out.iter().any(|x| x.content == m.content) {
                out.push(m.clone());
            }
        }
        if let Element::Group { children, .. } = el {
            collect_masks(children, out);
        }
    }
}

fn collect_level(elements: &[Element], seq: &mut usize) -> LevelMasks {
    let mut level = LevelMasks::default();
    for (idx, el) in elements.iter().enumerate() {
        match el {
            Element::Shading(def) => {
                if let Some(mask) = &def.mask {
                    *seq += 1;
                    let name = format!("GSM{seq}");
                    let a = element_alpha(el);
                    let sa = element_stroke_alpha(el).unwrap_or(a);
                    level
                        .mask_defs
                        .push((idx, name.clone(), mask.clone(), a, sa));
                    level.mask_names.insert(idx, name);
                }
            }
            Element::Group { children, .. } => {
                let sub = collect_level(children, seq);
                level.children.insert(idx, sub);
            }
            _ => {}
        }
    }
    level
}

/// 递归构建一层的 Group 表单（先子后父），把表单引用写入 `forms`。
#[allow(clippy::too_many_arguments)]
fn build_group_level(
    doc: &mut lopdf::Document,
    elements: &[Element],
    page_res: &Dictionary,
    font_entries: &BTreeMap<String, FontEntry>,
    image_entries: &BTreeMap<String, (String, (u32, u16))>,
    gs_names: &BTreeMap<GsKey, String>,
    level: &mut LevelMasks,
    all_masks: &[crate::model::SoftMask],
    form_seq: &mut usize,
    forms: &mut Dictionary,
    form_names: &mut BTreeMap<usize, String>,
    warnings: &mut Vec<String>,
) -> Result<()> {
    for (idx, el) in elements.iter().enumerate() {
        let Element::Group {
            children,
            bbox,
            group,
            matrix,
            ..
        } = el
        else {
            continue;
        };
        // 子层先建：子内容要引用更深层的表单
        let mut child_forms = Dictionary::new();
        let mut child_names: BTreeMap<usize, String> = BTreeMap::new();
        if let Some(sub) = level.children.get_mut(&idx) {
            build_group_level(
                doc,
                children,
                page_res,
                font_entries,
                image_entries,
                gs_names,
                sub,
                all_masks,
                form_seq,
                &mut child_forms,
                &mut child_names,
                warnings,
            )?;
        }
        let sub_masks = level
            .children
            .get(&idx)
            .map(|s| s.mask_names.clone())
            .unwrap_or_default();
        // 本层资源 = 页面资源 + 子层表单
        let mut res = page_res.clone();
        if !child_forms.is_empty() {
            let mut xo = res
                .get(b"XObject")
                .ok()
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default();
            for (k, v) in child_forms.iter() {
                xo.set(k.clone(), v.clone());
            }
            res.set("XObject", Object::Dictionary(xo));
        }
        let tmp = PageModel {
            elements: children.clone(),
            ..Default::default()
        };
        let mut fwarn = Vec::new();
        let fcontent = build_content(
            &tmp,
            font_entries,
            image_entries,
            gs_names,
            &sub_masks,
            &child_names,
            all_masks,
            &mut fwarn,
        )?;
        warnings.extend(fwarn);
        let mut fd = Dictionary::new();
        fd.set("Type", Object::Name(b"XObject".into()));
        fd.set("Subtype", Object::Name(b"Form".into()));
        fd.set("FormType", Object::Integer(1));
        fd.set(
            "BBox",
            Object::Array(bbox.iter().map(|v| Object::Real(*v as f32)).collect()),
        );
        if let Some(g) = group {
            let mut gd = Dictionary::new();
            gd.set("Type", Object::Name(b"Group".into()));
            gd.set("S", Object::Name(b"Transparency".into()));
            if g.isolated {
                gd.set("I", Object::Boolean(true));
            }
            if g.knockout {
                gd.set("K", Object::Boolean(true));
            }
            if let Some(cs) = &g.cs {
                gd.set("CS", Object::Name(cs.as_bytes().to_vec()));
            }
            fd.set("Group", Object::Dictionary(gd));
        }
        fd.set("Resources", Object::Dictionary(res));
        // 放置矩阵由内容流的 `{matrix} cm` 施加（子元素坐标为组空间）；
        // 表单自身保持单位 Matrix，否则重复平移（组会偏出原位）
        let _ = matrix;
        fd.set(
            "Matrix",
            Object::Array(vec![
                Object::Integer(1),
                Object::Integer(0),
                Object::Integer(0),
                Object::Integer(1),
                Object::Integer(0),
                Object::Integer(0),
            ]),
        );
        let id = doc.add_object(Object::Stream(Stream::new(fd, fcontent)));
        *form_seq += 1;
        let name = format!("FM{form_seq}");
        forms.set(name.clone(), Object::Reference(id));
        form_names.insert(idx, name);
        // 子层表单名同样进父层内容可见范围（父层内容里也引用它们）
        for (k, v) in child_forms.iter() {
            forms.set(k.clone(), v.clone());
        }
    }
    Ok(())
}

/// 输出虚线设置（`[] 0 d` = 实线）。
fn emit_dash(s: &mut String, dash: Option<&[f64]>, phase: f64) {
    match dash {
        Some(d) if !d.is_empty() => {
            let items: Vec<String> = d.iter().map(|v| fmt(*v as f32)).collect();
            s.push_str(&format!("[{}] {} d ", items.join(" "), fmt(phase as f32)));
        }
        _ => s.push_str("[] 0 d "),
    }
}

/// 输出路径段（m/l/c/h/re）。
fn emit_segments(s: &mut String, segments: &[crate::model::PathSegment]) {
    for seg in segments {
        match seg.op.as_str() {
            "m" if !seg.points.is_empty() => {
                let p = seg.points[0];
                s.push_str(&format!("{} {} m ", fmt(p[0] as f32), fmt(p[1] as f32)));
            }
            "l" if !seg.points.is_empty() => {
                let p = seg.points[0];
                s.push_str(&format!("{} {} l ", fmt(p[0] as f32), fmt(p[1] as f32)));
            }
            "c" if seg.points.len() >= 3 => {
                let p1 = seg.points[0];
                let p2 = seg.points[1];
                let p3 = seg.points[2];
                s.push_str(&format!(
                    "{} {} {} {} {} {} c ",
                    fmt(p1[0] as f32),
                    fmt(p1[1] as f32),
                    fmt(p2[0] as f32),
                    fmt(p2[1] as f32),
                    fmt(p3[0] as f32),
                    fmt(p3[1] as f32)
                ));
            }
            "re" if seg.points.len() >= 2 => {
                let (x, y) = (seg.points[0][0], seg.points[0][1]);
                let (w, h) = (seg.points[1][0], seg.points[1][1]);
                s.push_str(&format!(
                    "{} {} {} {} re ",
                    fmt(x as f32),
                    fmt(y as f32),
                    fmt(w as f32),
                    fmt(h as f32)
                ));
            }
            "h" => s.push_str("h "),
            _ => {}
        }
    }
}

/// 生成单个元素的内容流片段（页面与图案图块共用）。
#[allow(clippy::too_many_arguments)]
fn emit_element(
    s: &mut String,
    el: &Element,
    idx: usize,
    fonts: &BTreeMap<String, FontEntry>,
    images: &BTreeMap<String, (String, (u32, u16))>,
    gs_names: &BTreeMap<GsKey, String>,
    mask_names: &BTreeMap<usize, String>,
    masks: &[crate::model::SoftMask],
    extra_names: &[String],
    warnings: &mut Vec<String>,
    in_pattern: bool,
) {
    let key = element_gs_key_idx(el, masks);
    // 带亮度软掩膜的元素用专属 ExtGState（含 /SMask）
    let gs = mask_names
        .get(&idx)
        .cloned()
        .or_else(|| gs_names.get(&key).cloned());
    let open = |s: &mut String| {
        s.push_str("q ");
        if let Some(g) = &gs {
            s.push_str(&format!("/{g} gs "));
        }
    };
    match el {
        Element::Group { matrix, .. } => {
            // Form 已在页面资源阶段注册为 /FMN（含嵌套层）
            let Some(name) = extra_names.first() else {
                return;
            };
            open(s);
            let (a, b, c, d, e, f) = (
                matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5],
            );
            s.push_str(&format!(
                "{} {} {} {} {} {} cm /{name} Do Q\n",
                fmt(a as f32),
                fmt(b as f32),
                fmt(c as f32),
                fmt(d as f32),
                fmt(e as f32),
                fmt(f as f32)
            ));
        }
        Element::Text {
            text,
            x,
            y,
            size,
            font,
            font_id,
            color,
            rotation,
            mirror,
            codes,
            code_bytes,
            render_mode,
            stroke_color,
            stroke_width,
            word_spacing,
            char_spacing,
            h_scale,
            tm,
            ..
        } => {
            let key = text_font_key(font, text, font_id.as_deref());
            let Some(entry) = fonts.get(&key) else {
                warnings.push(format!("font '{font}' was not registered — skipping text"));
                return;
            };
            let (r, g, b) = color_operands(color).unwrap_or((0.0, 0.0, 0.0));
            let size = size.max(0.1);
            let lines: Vec<&str> = text.split('\n').collect();
            let rad = rotation.to_radians();
            let (cos, sin) = (rad.cos() as f32, rad.sin() as f32);
            // 镜像基 R(θ)·diag(1,-1) = [cos sin; sin -cos]（unpack 侧按
            // det<0 分解，这里逆变换；行偏移方向同样取镜像基的 -y 列）
            let (ma, mb, mc, md) = match tm {
                // 非正交基（剪切/两轴缩放不等）：按原始归一化线性部分精确还原
                Some(t) => (t[0] as f32, t[1] as f32, t[2] as f32, t[3] as f32),
                None if *mirror => (cos, sin, sin, -cos),
                None => (cos, sin, -sin, cos),
            };
            for (li, line) in lines.iter().enumerate() {
                if line.is_empty() {
                    continue;
                }
                // 多行沿文本方向的行距偏移（1.2 倍行距，与阅读器合成
                // 注释外观的默认一致）
                let off = (li as f64) * size * 1.2;
                let (lx, ly) = match tm {
                    // 下一行 = 文本 -y 轴方向（通用基）
                    Some(_) => (*x - off * (mc as f64), *y - off * (md as f64)),
                    None if *mirror => (*x - off * (mc as f64), *y + off * (cos as f64)),
                    None => (*x + off * (-sin as f64), *y - off * (cos as f64)),
                };
                open(s);
                s.push_str(&format!(
                    "BT /{} {} Tf {} {} {} rg ",
                    entry.res_name,
                    fmt(size as f32),
                    fmt(r),
                    fmt(g),
                    fmt(b)
                ));
                // 文本渲染模式（1 描边 / 2 填充+描边）与描边属性
                if *render_mode != 0 {
                    if let Some(sc) = stroke_color {
                        let (sr, sg, sb) = color_operands(sc).unwrap_or((0.0, 0.0, 0.0));
                        s.push_str(&format!("{} {} {} RG ", fmt(sr), fmt(sg), fmt(sb)));
                    }
                    s.push_str(&format!(
                        "{} w {} Tr ",
                        fmt(*stroke_width as f32),
                        render_mode
                    ));
                }
                s.push_str(&format!(
                    "{} {} {} {} {} {} Tm ",
                    fmt(ma),
                    fmt(mb),
                    fmt(mc),
                    fmt(md),
                    fmt(lx as f32),
                    fmt(ly as f32)
                ));
                if word_spacing.abs() > 1e-9 {
                    s.push_str(&format!("{} Tw ", fmt(*word_spacing as f32)));
                }
                if char_spacing.abs() > 1e-9 {
                    s.push_str(&format!("{} Tc ", fmt(*char_spacing as f32)));
                }
                if (h_scale - 1.0).abs() > 1e-6 {
                    s.push_str(&format!("{} Tz ", fmt((*h_scale * 100.0) as f32)));
                }
                // Tr 7 文本作为裁剪路径：保持本元素的 q 打开（不写 Q），
                // 让后续元素继承文本裁剪（build_content 末尾统一补 Q）
                let close = if *render_mode == 7 { "" } else { "Q" };
                if let Some(hex) = codes {
                    if !hex.is_empty() && *code_bytes >= 1 {
                        let _ = code_bytes;
                        s.push_str(&format!("<{hex}> Tj ET {close}\n"));
                        continue;
                    }
                }
                if entry.is_cid {
                    let encoder = entry.embedded.as_ref().unwrap();
                    let hex: String = encoder
                        .encode_text(line)
                        .iter()
                        .map(|b| format!("{b:02X}"))
                        .collect();
                    s.push_str(&format!("<{hex}> Tj ET {close}\n"));
                } else {
                    // 标准 14 字体：按 WinAnsi 写字节串；Symbol/ZapfDingbats
                    // 使用内建编码，码位必须原样透传（0x80+ 不能走 WinAnsi 重映射）
                    let bytes: Vec<u8> = if let Some(codes) = &entry.type3_codes {
                        line.chars()
                            .map(|c| codes.get(&(c as u32)).copied().unwrap_or(0) as u8)
                            .collect()
                    } else if entry.is_symbol {
                        line.chars().map(|c| c as u32 as u8).collect()
                    } else {
                        line.chars()
                            .map(|c| winansi_byte(c).unwrap_or(b'?'))
                            .collect()
                    };
                    let mut escaped = String::with_capacity(bytes.len());
                    for b in bytes {
                        match b {
                            b'\\' => escaped.push_str("\\\\"),
                            b'(' => escaped.push_str("\\("),
                            b')' => escaped.push_str("\\)"),
                            b'\n' => escaped.push_str("\\n"),
                            b'\r' => escaped.push_str("\\r"),
                            // 非 ASCII/控制字节用八进制转义，避免被再次 UTF-8 编码
                            0x20..=0x7e => escaped.push(b as char),
                            other => escaped.push_str(&format!("\\{other:03o}")),
                        }
                    }
                    s.push_str(&format!("({escaped}) Tj ET {close}\n"));
                }
            }
        }
        Element::Rect {
            x,
            y,
            w,
            h,
            fill,
            stroke,
            line_width,
            rotation,
            ..
        } => {
            let has_fill = fill.as_deref().and_then(color_operands);
            let has_stroke = stroke.as_deref().and_then(color_operands);
            if has_fill.is_none() && has_stroke.is_none() {
                return;
            }
            open(s);
            if rotation.abs() > 1e-9 {
                let rad = rotation.to_radians();
                let (cos, sin) = (rad.cos() as f32, rad.sin() as f32);
                s.push_str(&format!(
                    "{} {} {} {} {} {} cm ",
                    fmt(cos),
                    fmt(sin),
                    fmt(-sin),
                    fmt(cos),
                    fmt(*x as f32),
                    fmt(*y as f32)
                ));
            } else {
                s.push_str(&format!(
                    "1 0 0 1 {} {} cm ",
                    fmt(*x as f32),
                    fmt(*y as f32)
                ));
            }
            if let Some((r, g, b)) = has_stroke {
                s.push_str(&format!(
                    "{} {} {} RG {} w ",
                    fmt(r),
                    fmt(g),
                    fmt(b),
                    fmt(*line_width as f32)
                ));
            }
            s.push_str(&format!("0 0 {} {} re ", fmt(*w as f32), fmt(*h as f32)));
            match (has_fill, has_stroke) {
                (Some((r, g, b)), None) => {
                    s.push_str(&format!("{} {} {} rg f Q\n", fmt(r), fmt(g), fmt(b)))
                }
                (None, Some(_)) => s.push_str("S Q\n"),
                (Some((r, g, b)), Some(_)) => {
                    s.push_str(&format!("{} {} {} rg B Q\n", fmt(r), fmt(g), fmt(b)))
                }
                (None, None) => unreachable!(),
            }
        }
        Element::PatternRect { x, y, w, h, .. } => {
            let Some(name) = extra_names.first() else {
                return;
            };
            open(s);
            s.push_str(&format!(
                "1 0 0 1 {} {} cm /Pattern cs /{} scn 0 0 {} {} re f Q\n",
                fmt(*x as f32),
                fmt(*y as f32),
                name,
                fmt(*w as f32),
                fmt(*h as f32)
            ));
        }
        Element::Path {
            segments,
            fill,
            stroke,
            line_width,
            even_odd,
            dash,
            dash_phase,
            line_cap,
            line_join,
            clip,
            ..
        } => {
            let has_fill = fill.as_deref().and_then(color_operands);
            let has_stroke = stroke.as_deref().and_then(color_operands);
            if has_fill.is_none() && has_stroke.is_none() {
                return;
            }
            open(s);
            if let Some(segs) = clip {
                if !segs.is_empty() {
                    emit_segments(s, segs);
                    s.push_str("W n ");
                }
            }
            if let Some((r, g, b)) = has_stroke {
                s.push_str(&format!(
                    "{} {} {} RG {} w ",
                    fmt(r),
                    fmt(g),
                    fmt(b),
                    fmt(*line_width as f32)
                ));
            }
            emit_dash(s, dash.as_deref(), *dash_phase);
            // 线端/连接样式（圆帽会把短虚线变成圆点，必须回写）
            if line_cap.abs() > 1e-9 {
                s.push_str(&format!("{} J ", fmt(*line_cap as f32)));
            }
            if line_join.abs() > 1e-9 {
                s.push_str(&format!("{} j ", fmt(*line_join as f32)));
            }
            emit_segments(s, segments);
            match (has_fill, has_stroke) {
                (Some((r, g, b)), None) => {
                    s.push_str(&format!("{} {} {} rg ", fmt(r), fmt(g), fmt(b)));
                    s.push_str(if *even_odd { "f* Q\n" } else { "f Q\n" });
                }
                (None, Some(_)) => s.push_str("S Q\n"),
                (Some((r, g, b)), Some(_)) => {
                    s.push_str(&format!("{} {} {} rg ", fmt(r), fmt(g), fmt(b)));
                    s.push_str(if *even_odd { "B* Q\n" } else { "B Q\n" });
                }
                (None, None) => unreachable!(),
            }
        }
        Element::Polyline {
            points,
            stroke,
            line_width,
            close,
            dash,
            dash_phase,
        } => {
            if points.len() < 2 {
                return;
            }
            open(s);
            if let Some((r, g, b)) = color_operands(stroke) {
                s.push_str(&format!(
                    "{} {} {} RG {} w ",
                    fmt(r),
                    fmt(g),
                    fmt(b),
                    fmt(*line_width as f32)
                ));
            }
            emit_dash(s, dash.as_deref(), *dash_phase);
            for (i, [x, y]) in points.iter().enumerate() {
                if i == 0 {
                    s.push_str(&format!("{} {} m ", fmt(*x as f32), fmt(*y as f32)));
                } else {
                    s.push_str(&format!("{} {} l ", fmt(*x as f32), fmt(*y as f32)));
                }
            }
            if *close {
                s.push('h');
            }
            s.push_str("S Q\n");
        }
        Element::Shading(def) => {
            let Some(name) = extra_names.first() else {
                return;
            };
            open(s);
            if (def.alpha - 1.0).abs() > 1e-6 {
                if let Some(g) = gs_names.get(&(
                    (def.alpha * 1e6).round() as i64,
                    (def.alpha * 1e6).round() as i64,
                    None,
                    None,
                )) {
                    s.push_str(&format!("/{g} gs "));
                }
            }
            match &def.clip {
                Some(segs) if !segs.is_empty() => {
                    emit_segments(s, segs);
                    s.push_str("W n ");
                }
                _ => {
                    if let Some(bbox) = def.bbox {
                        s.push_str(&format!(
                            "{} {} {} {} re W n ",
                            fmt(bbox[0] as f32),
                            fmt(bbox[1] as f32),
                            fmt(bbox[2] as f32),
                            fmt(bbox[3] as f32)
                        ));
                    }
                }
            }
            // 网格渐变的顶点 / 椭圆径向渐变的坐标在着色空间内：裁剪路径已是
            // 设备坐标，必须在 W n 之后再 cm（否则裁剪被二次变换）
            if let Some(m) = &def.matrix {
                s.push_str(&format!(
                    "{} {} {} {} {} {} cm ",
                    fmt(m[0] as f32),
                    fmt(m[1] as f32),
                    fmt(m[2] as f32),
                    fmt(m[3] as f32),
                    fmt(m[4] as f32),
                    fmt(m[5] as f32)
                ));
            }
            if let Some(mesh) = &def.mesh {
                let m = &mesh.matrix;
                s.push_str(&format!(
                    "{} {} {} {} {} {} cm ",
                    fmt(m[0] as f32),
                    fmt(m[1] as f32),
                    fmt(m[2] as f32),
                    fmt(m[3] as f32),
                    fmt(m[4] as f32),
                    fmt(m[5] as f32)
                ));
            }
            s.push_str(&format!("/{name} sh Q\n"));
        }
        Element::Image {
            src,
            x,
            y,
            w,
            h,
            rotation,
            clip,
            ..
        } => {
            let Some((res_name, _)) = images.get(src) else {
                warnings.push(format!("image '{src}' was not registered — skipping"));
                return;
            };
            open(s);
            if let Some(segs) = clip {
                if !segs.is_empty() {
                    emit_segments(s, segs);
                    s.push_str("W n ");
                }
            }
            if rotation.abs() > 1e-9 {
                let rad = rotation.to_radians();
                let (cos, sin) = (rad.cos() as f32, rad.sin() as f32);
                s.push_str(&format!(
                    "{} {} {} {} {} {} cm ",
                    fmt(*w as f32 * cos),
                    fmt(*w as f32 * sin),
                    fmt(-*h as f32 * sin),
                    fmt(*h as f32 * cos),
                    fmt(*x as f32),
                    fmt(*y as f32)
                ));
            } else {
                s.push_str(&format!(
                    "{} 0 0 {} {} {} cm ",
                    fmt(*w as f32),
                    fmt(*h as f32),
                    fmt(*x as f32),
                    fmt(*y as f32)
                ));
            }
            s.push_str(&format!("/{res_name} Do Q\n"));
        }
    }
    let _ = in_pattern;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_parsing() {
        assert_eq!(color_operands("#ff8000"), Some((1.0, 128.0 / 255.0, 0.0)));
        assert_eq!(color_operands("bad"), None);
    }

    #[test]
    fn jpeg_dimensions_reads_sof() {
        // FFD8 + SOF0 段（len=17, prec=8, h=2, w=3, comps=3）
        let mut d = vec![0xFF, 0xD8];
        d.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x02, 0x00, 0x03, 0x03]);
        d.extend_from_slice(&[0u8; 16]);
        assert_eq!(jpeg_dimensions(&d), Some((3, 2, 3)));
    }

    #[test]
    fn fmt_trims() {
        assert_eq!(fmt(1.5), "1.5");
        assert_eq!(fmt(0.0), "0");
        assert_eq!(fmt(-0.25), "-0.25");
    }
}
