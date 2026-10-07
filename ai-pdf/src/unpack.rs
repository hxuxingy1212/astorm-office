//! 正向解析：PDF → 产物目录（document.json + pages/*.json + media/ + fonts/）。
//!
//! 设计对齐 ai-ppt（json2pptx）：产物目录是 unpack 与 repack 的统一载体，
//! 按页拆分 JSON、媒体实体放 media/、内嵌字体放 fonts/、元素坐标一律 pt（y 向上）。
//!
//! 采集能力：
//! - 文本：基线/字号/颜色/字体/旋转；同行同款连续 run 合并，字宽优先用源字体度量
//! - 图片：滤镜链解码（Flate/LZW/ASCII85/ASCIIHex/RunLength + Predictor）重建 PNG；
//!   DCT/JPX/JBIG2/CCITT 等按原滤镜直通；SMask 一并提取
//! - 矢量：矩形（re）、路径（m/l/c/h，保留贝塞尔控制点）、裁剪路径不落元素
//! - 颜色：rg/RG/g/G/k/K + cs/sc/scn（ICCBased/CalRGB/CalGray/Indexed/Separation）
//! - 表单（Form XObject）递归、平铺图案（PatternType 1）递归、渐变（sh）
//! - 透明度（ExtGState ca/CA）、注释外观流（AP/N）

use crate::images::{self, CsSpec, Decoded};
use crate::model::{
    DocumentModel, Element, FontResource, ImagePassthrough, Meta, PageModel, PathSegment,
    PatternDef, ShadingDef, ShadingStop,
};
use crate::text::{decode_string, fix_radicals, resolve_font, tokenize, FontDecoder, Mat, Tok};
use crate::util::round_f;
use anyhow::Result;
use lopdf::{Dictionary, Document as PdfDocument, Object};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// unpack 结果统计。
pub struct UnpackResult {
    pub pages: usize,
    pub images: usize,
    pub warnings: Vec<String>,
    /// 字体元数据文件（相对产物根）
    pub fonts: Vec<String>,
}

/// PDF → 产物目录。
pub fn unpack(pdf_path: &Path, out_dir: &Path) -> Result<UnpackResult> {
    let mut doc = PdfDocument::load(pdf_path)
        .map_err(|e| anyhow::anyhow!("load {}: {e}", pdf_path.display()))?;
    if doc.is_encrypted() {
        // 空用户口令的加密 PDF（RC4/AES-128 常见）：lopdf 可直接解密，
        // 产物按明文重建（视觉一致）。口令非空则仍不支持
        if doc.decrypt("").is_err() {
            anyhow::bail!("document is encrypted with a non-empty password — unsupported");
        }
    }
    normalize_brotli_streams(&mut doc);
    fix_object_stream_versions(&mut doc);
    let raw = std::fs::read(pdf_path).ok();
    unpack_doc_with_raw(&doc, out_dir, raw.as_deref())
}

/// lopdf 0.34 聚合对象流时「只增不替换」（reader.rs 注释，issue #160）：
/// 增量更新过的文件里同一对象在旧、新对象流各存一份时，先遍历到的旧版本
/// 被保留，页面数据过期（pdfbox/AcroFormsBasicFields 的 /Annots 由 26 项变
/// 18 项，表单域外观全丢）。这里按参考表重新解析容器对象流并覆盖。
fn fix_object_stream_versions(doc: &mut PdfDocument) {
    let mut by_container: std::collections::BTreeMap<u32, Vec<u32>> =
        std::collections::BTreeMap::new();
    for (id, entry) in doc.reference_table.entries.iter() {
        if let lopdf::xref::XrefEntry::Compressed { container, .. } = entry {
            by_container.entry(*container).or_default().push(*id);
        }
    }
    for (container, ids) in by_container {
        let Ok(obj) = doc.get_object((container, 0)).cloned() else {
            continue;
        };
        let Ok(stream) = obj.as_stream().cloned() else {
            continue;
        };
        let mut stream = stream;
        let Ok(os) = lopdf::ObjectStream::new(&mut stream) else {
            continue;
        };
        for n in ids {
            // 对象流内对象一律 generation 0
            if let Some(o) = os.objects.get(&(n, 0)) {
                doc.objects.insert((n, 0), o.clone());
            }
        }
    }
}

/// itext 写出的 /BrotliDecode 是非标准滤镜（MuPDF 支持、lopdf 不支持）。
/// 入口处把「单滤镜 BrotliDecode、无 DecodeParms」的流解码后去掉滤镜，
/// 内容流/外观流/图片随后都走既有路径；其余（滤镜链、带预测器）保持原样，
/// 图片仍可由直通路径原样嵌入。
pub fn normalize_brotli_streams(doc: &mut PdfDocument) -> usize {
    use std::io::Read;
    let ids: Vec<(u32, u16)> = doc.objects.keys().cloned().collect();
    let mut n = 0;
    for id in ids {
        let Some(Object::Stream(s)) = doc.objects.get_mut(&id) else {
            continue;
        };
        let is_brotli = matches!(
            s.dict.get(b"Filter"),
            Ok(Object::Name(name)) if name.as_slice() == b"BrotliDecode"
        );
        if !is_brotli || s.dict.has(b"DecodeParms") {
            continue;
        }
        let raw = s.content.clone();
        let mut out = Vec::new();
        let mut dec = brotli::Decompressor::new(std::io::Cursor::new(&raw), 4096);
        if dec.read_to_end(&mut out).is_err() {
            continue;
        }
        s.dict.remove(b"Filter");
        s.dict.set("Length", Object::Integer(out.len() as i64));
        s.content = out;
        n += 1;
    }
    n
}

pub fn unpack_doc(doc: &PdfDocument, out_dir: &Path) -> Result<UnpackResult> {
    unpack_doc_with_raw(doc, out_dir, None)
}

/// 从原始文件字节中提取无 /Length 的流（lopdf 解析不了这类流）。
fn raw_stream_bytes(raw: &[u8], obj_id: (u32, u16)) -> Option<Vec<u8>> {
    let needle = format!("{} {} obj", obj_id.0, obj_id.1);
    let pos = raw
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())?;
    let rest = &raw[pos..];
    let sidx = rest.windows(6).position(|w| w == b"stream")?;
    let mut i = sidx + 6;
    if i < rest.len() && rest[i] == b'\r' {
        i += 1;
    }
    if i < rest.len() && rest[i] == b'\n' {
        i += 1;
    }
    let end = rest[i..].windows(9).position(|w| w == b"endstream")?;
    Some(rest[i..i + end].to_vec())
}

pub fn unpack_doc_with_raw(
    doc: &PdfDocument,
    out_dir: &Path,
    raw: Option<&[u8]>,
) -> Result<UnpackResult> {
    std::fs::create_dir_all(out_dir.join("pages"))?;
    std::fs::create_dir_all(out_dir.join("media"))?;
    std::fs::create_dir_all(out_dir.join("fonts"))?;

    let mut model = DocumentModel {
        meta: Some(extract_meta(doc)),
        ..DocumentModel::default()
    };

    let page_map = doc.get_pages();
    if page_map.is_empty() {
        anyhow::bail!("no pages found — page tree is unreadable");
    }
    let mut warnings = Vec::new();
    let mut image_count = 0usize;
    let mut shared = DocState::default();

    for (pno, page_id) in &page_map {
        let page_no = *pno as usize;
        let page_json = format!("pages/page-{:03}.json", page_no);
        let mut page = PageModel::default();

        if let Some((w, h, mb)) = crate::page_ops::page_mediabox(doc, *page_id) {
            page.width = Some(round_f(w, 2));
            page.height = Some(round_f(h, 2));
            if mb[0].abs() > 1e-6 || mb[1].abs() > 1e-6 {
                page.mediabox = Some([
                    round_f(mb[0], 2),
                    round_f(mb[1], 2),
                    round_f(mb[2], 2),
                    round_f(mb[3], 2),
                ]);
            }
        }
        page.rotation = inherited_u32(doc, *page_id, b"Rotate").unwrap_or(0);
        // CropBox：渲染可见区域，需原样保留（否则内容整体偏移）
        if let Some(obj) = inherited_attr_pub(doc, *page_id, b"CropBox") {
            // CropBox 可能是间接引用，先解引用
            let obj = doc.dereference(obj).map(|(_, o)| o).unwrap_or(obj);
            if let Ok(arr) = obj.as_array() {
                let v: Vec<f64> = arr
                    .iter()
                    .filter_map(|o| match o {
                        Object::Integer(i) => Some(*i as f64),
                        Object::Real(r) => Some(*r as f64),
                        _ => None,
                    })
                    .collect();
                if v.len() == 4 {
                    page.crop = Some([
                        round_f(v[0], 2),
                        round_f(v[1], 2),
                        round_f(v[2], 2),
                        round_f(v[3], 2),
                    ]);
                }
            }
        }

        let mut collector = ElementCollector::new(doc, out_dir, page_no, &mut shared, raw);
        collector.collect_page(*page_id);
        page.elements = std::mem::take(&mut collector.elements);
        image_count += collector.images_extracted;
        for w in &collector.warnings {
            warnings.push(format!("page {pno}: {w}"));
        }
        for rel in &shared.font_files {
            if !model.fonts.contains(rel) {
                model.fonts.push(rel.clone());
            }
        }

        std::fs::write(
            out_dir.join(&page_json),
            serde_json::to_string_pretty(&page)?,
        )?;
        model.pages.push(page_json);
    }

    std::fs::write(
        out_dir.join("document.json"),
        serde_json::to_string_pretty(&model)?,
    )?;

    Ok(UnpackResult {
        pages: page_map.len(),
        images: image_count,
        warnings,
        fonts: model.fonts.clone(),
    })
}

fn extract_meta(doc: &PdfDocument) -> Meta {
    let info = crate::meta::get_info_map(doc);
    let get = |k: &str| info.get(k).and_then(|v| v.as_str()).map(|s| s.to_string());
    Meta {
        title: get("Title"),
        author: get("Author"),
        subject: get("Subject"),
        creator: get("Creator"),
        producer: get("Producer"),
    }
}

/// 沿 Parent 链查找继承属性（供 CropBox 等使用）。
pub fn inherited_attr_pub<'a>(
    doc: &'a PdfDocument,
    page_id: (u32, u16),
    key: &[u8],
) -> Option<&'a Object> {
    let mut cur = page_id;
    for _ in 0..64 {
        let dict = doc.get_dictionary(cur).ok()?;
        if let Ok(v) = dict.get(key) {
            return doc.dereference(v).ok().map(|(_, o)| o);
        }
        cur = dict.get(b"Parent").and_then(Object::as_reference).ok()?;
    }
    None
}

fn inherited_u32(doc: &PdfDocument, page_id: (u32, u16), key: &[u8]) -> Option<u32> {
    let mut cur = page_id;
    for _ in 0..64 {
        let dict = doc.get_dictionary(cur).ok()?;
        if let Ok(v) = dict.get(key) {
            let (_, v) = doc.dereference(v).ok()?;
            return v.as_i64().ok().map(|n| n as u32);
        }
        cur = dict.get(b"Parent").and_then(Object::as_reference).ok()?;
    }
    None
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Rgb(f32, f32, f32);

impl Rgb {
    fn hex(self) -> String {
        format!(
            "#{:02x}{:02x}{:02x}",
            (self.0.clamp(0.0, 1.0) * 255.0) as u8,
            (self.1.clamp(0.0, 1.0) * 255.0) as u8,
            (self.2.clamp(0.0, 1.0) * 255.0) as u8
        )
    }
    fn from_args(args: &[Tok]) -> Self {
        Rgb(
            tok_num(args.first()) as f32,
            tok_num(args.get(1)) as f32,
            tok_num(args.get(2)) as f32,
        )
    }
    fn gray(v: f64) -> Self {
        let v = v as f32;
        Rgb(v, v, v)
    }
    fn cmyk(args: &[Tok]) -> Self {
        let c = tok_num(args.first());
        let m = tok_num(args.get(1));
        let y = tok_num(args.get(2));
        let k = tok_num(args.get(3));
        Rgb(
            ((1.0 - c) * (1.0 - k)) as f32,
            ((1.0 - m) * (1.0 - k)) as f32,
            ((1.0 - y) * (1.0 - k)) as f32,
        )
    }
    fn from_components(comps: &[f64], spec: Option<&CsSpec>) -> Self {
        match spec {
            // 规格未知时按分量数推断（1=灰度 / 3=RGB / 4=CMYK）
            None => match comps.len() {
                4 => Rgb(
                    ((1.0 - comps[0]) * (1.0 - comps[3])) as f32,
                    ((1.0 - comps[1]) * (1.0 - comps[3])) as f32,
                    ((1.0 - comps[2]) * (1.0 - comps[3])) as f32,
                ),
                3 => Rgb(comps[0] as f32, comps[1] as f32, comps[2] as f32),
                _ => Rgb::gray(*comps.first().unwrap_or(&0.0)),
            },
            Some(CsSpec::Gray) => Rgb::gray(*comps.first().unwrap_or(&0.0)),
            Some(CsSpec::CalGray { gamma, white }) => {
                let (r, g, b) =
                    images::calgray_to_srgb(*comps.first().unwrap_or(&0.0), *gamma, white);
                Rgb(r as f32, g as f32, b as f32)
            }
            Some(CsSpec::CalRgb {
                gamma,
                matrix,
                white,
            }) => {
                let (r, g, b) = images::calrgb_to_srgb(
                    *comps.first().unwrap_or(&0.0),
                    *comps.get(1).unwrap_or(&0.0),
                    *comps.get(2).unwrap_or(&0.0),
                    gamma,
                    matrix,
                    white,
                );
                Rgb(r as f32, g as f32, b as f32)
            }
            Some(CsSpec::Rgb) => Rgb(
                *comps.first().unwrap_or(&0.0) as f32,
                *comps.get(1).unwrap_or(&0.0) as f32,
                *comps.get(2).unwrap_or(&0.0) as f32,
            ),
            Some(CsSpec::Cmyk) => {
                let c = *comps.first().unwrap_or(&0.0);
                let m = *comps.get(1).unwrap_or(&0.0);
                let y = *comps.get(2).unwrap_or(&0.0);
                let k = *comps.get(3).unwrap_or(&0.0);
                Rgb(
                    ((1.0 - c) * (1.0 - k)) as f32,
                    ((1.0 - m) * (1.0 - k)) as f32,
                    ((1.0 - y) * (1.0 - k)) as f32,
                )
            }
            // Tinted 由 rgb_from_components 求值（此处不应到达）：退化为分量推断
            Some(CsSpec::Tinted { alt, .. }) => Rgb::from_components(comps, Some(alt.as_ref())),
            Some(CsSpec::Indexed(pal, base)) => {
                let i = *comps.first().unwrap_or(&0.0) as usize;
                match base {
                    1 => {
                        let v = pal.get(i).copied().unwrap_or(0) as f32 / 255.0;
                        Rgb(v, v, v)
                    }
                    _ => Rgb(
                        pal.get(i * 3).copied().unwrap_or(0) as f32 / 255.0,
                        pal.get(i * 3 + 1).copied().unwrap_or(0) as f32 / 255.0,
                        pal.get(i * 3 + 2).copied().unwrap_or(0) as f32 / 255.0,
                    ),
                }
            }
        }
    }
}

/// 正在累积的文本 run：同基线/字号/字体/颜色/旋转的连续 run 合并为一个 Text 元素。
struct TextBuffer {
    y_key: i64,
    x_key: i64,
    size: f64,
    font: String,
    font_id: Option<String>,
    color: String,
    rotation: f64,
    /// 文本基镜像（det<0）
    mirror: bool,
    /// 原始码位（供复杂文种整形文本原样回写）
    codes: Vec<u8>,
    /// 该字体要求原码回写（非 Identity CIDToGIDMap）
    force_codes: bool,
    /// 文本渲染模式与描边色
    render_mode: u8,
    stroke_color: Option<String>,
    alpha: f64,
    blend: Option<String>,
    word_spacing: f64,
    char_spacing: f64,
    h_scale: f64,
    /// 非正交文本基的归一化线性部分（见 Element::Text::tm）
    tm: Option<[f64; 4]>,
    /// 文本所在图形状态的软掩膜（ExtGState /SMask）
    smask: Option<Box<crate::model::SoftMask>>,
    x: f64,
    y: f64,
    text: String,
    /// 估算终点（设备空间），供 run 间隙判空格
    end_x: f64,
}

/// 字体槽位。
struct FontSlot<'a> {
    /// 显示名（去子集前缀）
    display: String,
    /// 文本解码器
    decoder: FontDecoder<'a>,
    /// 内嵌字体资源 id
    font_id: Option<String>,
    /// unicode → 1000 字宽
    widths: Option<Rc<BTreeMap<u32, i64>>>,
    /// Type3 字体字典（CharProcs/FontMatrix）
    type3: Option<&'a Dictionary>,
    /// 非 Identity CIDToGIDMap：文本必须按原始码位回写（GID≠CID）
    raw_codes: bool,
}

/// 资源作用域（页 / 表单 / 图案各一份）。字体按层叠加（后层覆盖前层）。
#[derive(Default, Clone)]
struct Scope<'a> {
    /// 字体层：父作用域在前，自身在后
    font_layers: Vec<Rc<BTreeMap<Vec<u8>, FontSlot<'a>>>>,
    /// 其余资源同样分层（父在前、自身在后）：Form 的 `Scope::clone()` 只是
    /// 克隆若干 Rc，不再深拷贝整张资源表（Illustrator 类文件有上千资源，
    /// 每个 Form 深拷贝一次会让单个文件跑上百秒；pdf.js/issue6961）
    xobject_layers: Vec<Rc<BTreeMap<Vec<u8>, Object>>>,
    colorspace_layers: Vec<Rc<BTreeMap<Vec<u8>, CsSpec>>>,
    pattern_layers: Vec<Rc<BTreeMap<Vec<u8>, Object>>>,
    shading_layers: Vec<Rc<BTreeMap<Vec<u8>, Object>>>,
    extgstate_layers: Vec<Rc<BTreeMap<Vec<u8>, Object>>>,
}

/// 分层查找（后层覆盖前层）。
fn layer_get<'m, V>(layers: &'m [Rc<BTreeMap<Vec<u8>, V>>], name: &[u8]) -> Option<&'m V> {
    layers.iter().rev().find_map(|m| m.get(name))
}

impl<'a> Scope<'a> {
    fn font(&self, name: &[u8]) -> Option<&FontSlot<'a>> {
        self.font_layers.iter().rev().find_map(|m| m.get(name))
    }
    fn xobject(&self, name: &[u8]) -> Option<&Object> {
        layer_get(&self.xobject_layers, name)
    }
    fn colorspace(&self, name: &[u8]) -> Option<&CsSpec> {
        layer_get(&self.colorspace_layers, name)
    }
    fn pattern(&self, name: &[u8]) -> Option<&Object> {
        layer_get(&self.pattern_layers, name)
    }
    fn shading(&self, name: &[u8]) -> Option<&Object> {
        layer_get(&self.shading_layers, name)
    }
    fn extgstate(&self, name: &[u8]) -> Option<&Object> {
        layer_get(&self.extgstate_layers, name)
    }
}

/// 图形状态（q/Q 压栈）。
#[derive(Clone)]
struct GState {
    ctm: Mat,
    /// 内容流入口 CTM（页 = 单位阵；Form = form Matrix ∘ 父 CTM；Type3 = TRM）。
    /// PDF 规定图案 Matrix 映射到所在内容流的默认用户空间，
    /// 即只叠加入口 CTM、不受流内 `cm` 影响（经 MuPDF 最小样例实证：
    /// `q 1 0 0 -1 0 H cm` 内的图案填充位置与无翻转时一致）。
    base: Mat,
    fill: Rgb,
    stroke: Rgb,
    fill_alpha: f64,
    stroke_alpha: f64,
    line_width: f64,
    fill_cs: Option<CsSpec>,
    stroke_cs: Option<CsSpec>,
    /// /Pattern cs + /Pn scn 后的图案名
    fill_pattern: Option<Vec<u8>>,
    font_size: f64,
    font_key: Vec<u8>,
    font_name: String,
    font_id: Option<String>,
    font_widths: Option<Rc<BTreeMap<u32, i64>>>,
    leading: f64,
    char_spacing: f64,
    word_spacing: f64,
    h_scale: f64,
    tm_line: Mat,
    tm_text: Mat,
    /// 当前裁剪框（设备空间近似）
    clip: Option<[f64; 4]>,
    /// 当前裁剪路径（设备空间）
    clip_path: Option<Vec<PathSegment>>,
    /// 文本上升（Ts）
    rise: f64,
    /// 文本渲染模式（Tr）
    render_mode: u8,
    /// 混合模式（/BM）
    blend: Option<String>,
    /// 虚线模式与相位
    dash: Option<Vec<f64>>,
    dash_phase: f64,
    /// 亮度软掩膜的表单对象（ExtGState /SMask /G；元素绘制时解析为 mask 渐变）
    smask_form: Option<Object>,
    /// /SMask 字典的 /S（"Alpha" | "Luminosity"）
    smask_subtype: String,
    /// /SMask 字典的 /BC
    smask_backdrop: Option<Vec<f64>>,
    /// /SMask 字典的 /TR（原样 JSON）
    smask_transfer: Option<serde_json::Value>,
    /// 线端样式（J / ExtGState LC）
    line_cap: f64,
    /// 线连接样式（j / ExtGState LJ）
    line_join: f64,
}

impl Default for GState {
    fn default() -> Self {
        GState {
            ctm: Mat::I,
            base: Mat::I,
            fill: Rgb(0.0, 0.0, 0.0),
            stroke: Rgb(0.0, 0.0, 0.0),
            fill_alpha: 1.0,
            stroke_alpha: 1.0,
            line_width: 1.0,
            fill_cs: None,
            stroke_cs: None,
            fill_pattern: None,
            font_size: 12.0,
            font_key: Vec::new(),
            font_name: "Helvetica".into(),
            font_id: None,
            font_widths: None,
            // PDF 规范：leading 默认 0（未设 TL 时 ' 与 " 的 T* 不移动）
            leading: 0.0,
            char_spacing: 0.0,
            word_spacing: 0.0,
            h_scale: 1.0,
            tm_line: Mat::I,
            tm_text: Mat::I,
            clip: None,
            clip_path: None,
            rise: 0.0,
            render_mode: 0,
            blend: None,
            dash: None,
            dash_phase: 0.0,
            smask_form: None,
            smask_subtype: String::new(),
            smask_backdrop: None,
            smask_transfer: None,
            line_cap: 0.0,
            line_join: 0.0,
        }
    }
}

/// 待绘制的路径。
#[derive(Default)]
struct PathBuilder {
    segs: Vec<PathSegment>,
}

impl PathBuilder {
    fn clear(&mut self) {
        self.segs.clear();
    }
    fn is_empty(&self) -> bool {
        self.segs.is_empty()
    }
    /// 仅一个矩形段时返回 (x,y,w,h)。
    fn as_single_rect(&self) -> Option<(f64, f64, f64, f64)> {
        if self.segs.len() != 1 {
            return None;
        }
        let s = &self.segs[0];
        if s.op != "re" || s.points.len() != 2 {
            return None;
        }
        let (x, y) = (s.points[0][0], s.points[0][1]);
        let (w, h) = (s.points[1][0], s.points[1][1]);
        Some((x, y, w, h))
    }
}

/// 文档级共享状态：媒体/字体序号与缓存（跨页唯一，避免同名覆盖）。
#[derive(Default)]
struct DocState {
    media_idx: usize,
    /// 网格渐变码流序号
    mesh_idx: usize,
    func_idx: usize,
    font_idx: usize,
    /// 已抽取的字体对象 → (资源 id, unicode→字宽, 是否需原码回写)
    #[allow(clippy::type_complexity)]
    font_cache: BTreeMap<(u32, u16), (String, Rc<BTreeMap<u32, i64>>, bool)>,
    /// 字体元数据文件（相对产物根）
    font_files: Vec<String>,
    /// 软掩膜表单序号
    mask_idx: usize,
}

/// 页面元素采集器。
struct ElementCollector<'a> {
    doc: &'a PdfDocument,
    out_dir: PathBuf,
    #[allow(dead_code)]
    page_no: usize,
    elements: Vec<Element>,
    images_extracted: usize,
    warnings: Vec<String>,
    shared: &'a mut DocState,
    /// 原始文件字节（补偿 lopdf 对无 /Length 流的解析失败）
    raw: Option<&'a [u8]>,
    /// 已抽取的图片对象 → media 相对路径
    image_cache: BTreeMap<(u32, u16), String>,
    /// 已抽取图片的直通参数（媒体路径 → 参数）
    passthrough_cache: BTreeMap<String, ImagePassthrough>,
    /// XObject 图片 id → 预览 PNG 路径（JPX/JBIG2/CCITT 解码产物）
    preview_cache: BTreeMap<(u32, u16), Option<String>>,
    /// 已抽取的软掩膜（表单对象 id → SoftMask）
    smask_element_cache: BTreeMap<(u32, u16), Box<crate::model::SoftMask>>,
    /// 已抽取图片的 SMask（对象 id → (路径, 参数)）
    #[allow(clippy::type_complexity)]
    smask_cache: BTreeMap<(u32, u16), (Option<String>, Option<Box<ImagePassthrough>>)>,
    /// 图案嵌套深度（防环）
    pattern_depth: usize,
    /// 当前文本 run
    text_buf: Option<TextBuffer>,
}

impl<'a> ElementCollector<'a> {
    fn new(
        doc: &'a PdfDocument,
        out_dir: &Path,
        page_no: usize,
        shared: &'a mut DocState,
        raw: Option<&'a [u8]>,
    ) -> Self {
        ElementCollector {
            doc,
            out_dir: out_dir.to_path_buf(),
            page_no,
            elements: Vec::new(),
            images_extracted: 0,
            warnings: Vec::new(),
            shared,
            raw,
            image_cache: BTreeMap::new(),
            passthrough_cache: BTreeMap::new(),
            preview_cache: BTreeMap::new(),
            smask_cache: BTreeMap::new(),
            smask_element_cache: BTreeMap::new(),
            pattern_depth: 0,
            text_buf: None,
        }
    }

    fn collect_page(&mut self, page_id: (u32, u16)) {
        let scope = self.build_page_scope(page_id);
        let mut content = self.doc.get_page_content(page_id).unwrap_or_default();
        if content.is_empty() {
            content = self.page_content_from_raw(page_id);
        }
        let mut state = GState::default();
        // 不预设裁剪：MediaBox 不是裁剪（页面框原点非零时会把内容误裁），
        // 仅 `W n` 显式设置裁剪区域
        self.run_content(&content, &scope, &mut state, 0);

        // 注释外观流（AP/N）
        self.run_annotations(page_id, &scope, 0);
    }

    /// 页内容流：lopdf 解析为空时（无 /Length 等）从原始字节提取。
    fn page_content_from_raw(&mut self, page_id: (u32, u16)) -> Vec<u8> {
        let Some(raw) = self.raw else {
            return Vec::new();
        };
        let Ok(dict) = self.doc.get_dictionary(page_id) else {
            return Vec::new();
        };
        let Ok(contents) = dict.get(b"Contents") else {
            return Vec::new();
        };
        let mut ids: Vec<(u32, u16)> = Vec::new();
        match self.doc.dereference(contents) {
            Ok((_, Object::Array(arr))) => {
                for o in arr {
                    if let Ok(id) = o.as_reference() {
                        ids.push(id);
                    }
                }
            }
            _ => {
                if let Ok(id) = contents.as_reference() {
                    ids.push(id);
                }
            }
        }
        let mut out = Vec::new();
        for id in ids {
            if let Some(mut b) = raw_stream_bytes(raw, id) {
                out.append(&mut b);
                out.push(b'\n');
            }
        }
        if !out.is_empty() {
            self.warnings
                .push("content stream lacked /Length — recovered from raw bytes".into());
        }
        out
    }

    /// 页资源作用域：祖先 → 页面内联（后者覆盖同名）。
    fn build_page_scope(&mut self, page_id: (u32, u16)) -> Scope<'a> {
        let (inline, ids) = self
            .doc
            .get_page_resources(page_id)
            .unwrap_or((None, Vec::new()));
        let mut scope = Scope::default();
        // 祖先 /Resources（远祖先先合并，近祖先与页面内联覆盖）：lopdf 的
        // get_page_resources 在 /Resources 挂在 /Pages 节点时返回空，
        // 会让图案/字体/ExtGState 全丢（pdf.js/issue17065）
        let mut chain: Vec<Dictionary> = Vec::new();
        let mut cur = page_id;
        for _ in 0..64 {
            let Ok(dict) = self.doc.get_dictionary(cur) else {
                break;
            };
            let parent = dict.get(b"Parent").and_then(Object::as_reference).ok();
            let Some(p) = parent else { break };
            if let Ok(pd) = self.doc.get_dictionary(p) {
                if let Ok(res) = pd.get(b"Resources") {
                    let ro = self
                        .doc
                        .dereference(res)
                        .map(|(_, o)| o.clone())
                        .unwrap_or_else(|_| res.clone());
                    if let Ok(rd) = ro.as_dict() {
                        chain.push(rd.clone());
                    }
                }
            }
            cur = p;
        }
        for res in chain.iter().rev() {
            self.merge_resources(&mut scope, res);
        }
        for id in ids.iter().rev() {
            if let Ok(dict) = self.doc.get_dictionary(*id) {
                let d = dict.clone();
                self.merge_resources(&mut scope, &d);
            }
        }
        if let Some(res) = inline {
            let d = res.clone();
            self.merge_resources(&mut scope, &d);
        }
        scope
    }

    fn merge_resources(&mut self, scope: &mut Scope<'a>, res: &Dictionary) {
        // 字体（新层）
        if let Ok(fonts) = self.deref_dict(res, b"Font") {
            let mut layer: BTreeMap<Vec<u8>, FontSlot<'a>> = BTreeMap::new();
            let entries: Vec<(Vec<u8>, Object)> =
                fonts.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            for (name, obj) in entries {
                // 解码器借用文档内的字体字典（生命周期 'a）：间接引用直接借，
                // 内联字典（/Font << /f1 << … >> >>，非引用）克隆后泄漏换取 'a —
                // 否则整页字体全被跳过、文本回落 Helvetica（pdf.js/bug946506）
                let font: &'a Dictionary = match obj.as_reference() {
                    Ok(id) => match self.doc.get_dictionary(id) {
                        Ok(d) => d,
                        Err(_) => continue,
                    },
                    Err(_) => match obj.as_dict() {
                        Ok(d) => Box::leak(Box::new(d.clone())),
                        Err(_) => continue,
                    },
                };
                // /BaseFont 可能是 `#XX` 转义的 GBK 名（老式中文 PDF 的「宋体」）
                let base = crate::text::basefont_display(font);
                let base = if base.is_empty() {
                    "Helvetica".to_string()
                } else {
                    base
                };
                let display = strip_subset_prefix(&base).to_string();
                let decoder = resolve_font(font, self.doc);
                let (mut font_id, mut widths, raw_codes) =
                    self.extract_font_resource(font, &display);
                // 简单字体没有 W 数组，宽度取自 /Widths（经解码器折算到 unicode）
                if widths.as_ref().map(|w| w.is_empty()).unwrap_or(true) {
                    let w = Self::simple_font_unicode_widths(font, &decoder, self.doc);
                    if !w.is_empty() {
                        widths = Some(Rc::new(w));
                    }
                }
                let is_type3 = font
                    .get(b"Subtype")
                    .and_then(Object::as_name_str)
                    .unwrap_or("")
                    == "Type3";
                // Type3 位图字体（CharProcs 引用图片）：字体直通而非扁平化
                let mut type3_passthrough = false;
                if is_type3 {
                    if let Some(fid) = self.extract_type3_font(font, &display) {
                        font_id = Some(fid);
                        widths = Some(Rc::new(Self::simple_font_unicode_widths(
                            font, &decoder, self.doc,
                        )));
                        type3_passthrough = true;
                    }
                }
                layer.insert(
                    name,
                    FontSlot {
                        display,
                        decoder,
                        font_id,
                        widths,
                        type3: if is_type3 && !type3_passthrough {
                            Some(font)
                        } else {
                            None
                        },
                        raw_codes,
                    },
                );
            }
            if !layer.is_empty() {
                scope.font_layers.push(Rc::new(layer));
            }
        }
        // XObject（值可能是间接引用或内联字典，统一存对象）
        if let Ok(xo) = self.deref_dict(res, b"XObject") {
            if !xo.is_empty() {
                let layer: BTreeMap<Vec<u8>, Object> =
                    xo.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                scope.xobject_layers.push(Rc::new(layer));
            }
        }
        // 颜色空间
        if let Ok(cs) = self.deref_dict(res, b"ColorSpace") {
            let mut layer: BTreeMap<Vec<u8>, CsSpec> = BTreeMap::new();
            for (name, obj) in cs.iter() {
                if let Some(spec) = self.resolve_cs_spec(obj, 0) {
                    layer.insert(name.clone(), spec);
                }
            }
            if !layer.is_empty() {
                scope.colorspace_layers.push(Rc::new(layer));
            }
        }
        // 图案 / 着色 / ExtGState
        if let Ok(p) = self.deref_dict(res, b"Pattern") {
            if !p.is_empty() {
                let layer: BTreeMap<Vec<u8>, Object> =
                    p.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                scope.pattern_layers.push(Rc::new(layer));
            }
        }
        if let Ok(s) = self.deref_dict(res, b"Shading") {
            if !s.is_empty() {
                let layer: BTreeMap<Vec<u8>, Object> =
                    s.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                scope.shading_layers.push(Rc::new(layer));
            }
        }
        if let Ok(g) = self.deref_dict(res, b"ExtGState") {
            if !g.is_empty() {
                let layer: BTreeMap<Vec<u8>, Object> =
                    g.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                scope.extgstate_layers.push(Rc::new(layer));
            }
        }
    }

    /// 读取数值字段（自动解引用间接引用）。
    fn num_field(&self, dict: &Dictionary, key: &[u8]) -> Option<i64> {
        let v = dict.get(key).ok()?;
        match self.doc.dereference(v) {
            Ok((_, o)) => obj_i64(o),
            Err(_) => None,
        }
    }

    /// 取字典字段并解引用（字段本身可能是间接引用）。
    fn deref_dict(&self, res: &Dictionary, key: &[u8]) -> Result<Dictionary, ()> {
        match self.doc.dereference(res.get(key).map_err(|_| ())?) {
            Ok((_, Object::Dictionary(d))) => Ok(d.clone()),
            _ => Err(()),
        }
    }

    /// `cs`/`CS` 操作数的颜色空间名 → 规格：先查资源表，再按设备色空间名直解。
    fn cs_by_name(&self, name: &[u8], scope: &Scope<'a>) -> Option<CsSpec> {
        if let Some(spec) = scope.colorspace(name) {
            return Some(spec.clone());
        }
        match name {
            b"DeviceGray" | b"G" => Some(CsSpec::Gray),
            b"CalGray" => Some(self.calgray_spec(None)),
            b"DeviceCMYK" | b"CMYK" => Some(CsSpec::Cmyk),
            b"DeviceRGB" | b"RGB" | b"CalRGB" | b"Lab" => Some(CsSpec::Rgb),
            // 图案色空间：分量由后续 scn 决定，按 RGB 处理
            b"Pattern" => Some(CsSpec::Rgb),
            _ => None,
        }
    }

    /// CalGray 字典 → CsSpec::CalGray（/Gamma 默认 1，/WhitePoint 默认 D65）。
    fn calgray_spec(&self, dict_obj: Option<&Object>) -> CsSpec {
        let mut gamma = 1.0f64;
        let mut white = [0.9505f64, 1.0, 1.089];
        let dict = dict_obj
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok().cloned());
        if let Some(d) = &dict {
            gamma = d
                .get(b"Gamma")
                .ok()
                .and_then(|o| o.as_float().ok().map(|v| v as f64))
                .unwrap_or(1.0);
            if let Ok((_, o)) = d.get(b"WhitePoint").and_then(|o| self.doc.dereference(o)) {
                if let Some(a) = obj_f64_array(o) {
                    for (i, v) in a.iter().take(3).enumerate() {
                        white[i] = *v;
                    }
                }
            }
        }
        CsSpec::CalGray { gamma, white }
    }

    /// CalRGB 字典 → CsSpec::CalRgb（/Gamma 默认 1 1 1，/Matrix 默认单位阵，
    /// /WhitePoint 默认 D65）。线性分量若直接当 sRGB 用会明显偏暗。
    fn calrgb_spec(&self, dict_obj: Option<&Object>) -> CsSpec {
        let dict = dict_obj
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok().cloned());
        let mut gamma = [1.0f64; 3];
        let mut matrix = [1.0f64, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let mut white = [0.9505f64, 1.0, 1.089];
        if let Some(d) = &dict {
            if let Ok(g) = d
                .get(b"Gamma")
                .and_then(|o| self.doc.dereference(o))
                .map(|(_, o)| o)
            {
                if let Ok(a) = g.as_array() {
                    for (i, v) in a.iter().take(3).enumerate() {
                        gamma[i] = v.as_float().unwrap_or(1.0) as f64;
                    }
                }
            }
            if let Ok(m) = d
                .get(b"Matrix")
                .and_then(|o| self.doc.dereference(o))
                .map(|(_, o)| o)
            {
                if let Ok(a) = m.as_array() {
                    for (i, v) in a.iter().take(9).enumerate() {
                        matrix[i] = v.as_float().unwrap_or(0.0) as f64;
                    }
                }
            }
            if let Ok(w) = d
                .get(b"WhitePoint")
                .and_then(|o| self.doc.dereference(o))
                .map(|(_, o)| o)
            {
                if let Ok(a) = w.as_array() {
                    for (i, v) in a.iter().take(3).enumerate() {
                        white[i] = v.as_float().unwrap_or(white[i] as f32) as f64;
                    }
                }
            }
        }
        CsSpec::CalRgb {
            gamma,
            matrix,
            white,
        }
    }

    /// 颜色空间对象 → 规格（ICCBased / CalRGB / CalGray / Indexed / Separation / Device*）。
    fn resolve_cs_spec(&self, obj: &Object, depth: usize) -> Option<CsSpec> {
        if depth > 4 {
            return None;
        }
        match self.doc.dereference(obj) {
            Ok((_, Object::Name(n))) => match n.as_slice() {
                b"DeviceGray" | b"G" | b"Indexed" => Some(CsSpec::Gray),
                b"CalGray" => Some(self.calgray_spec(None)),
                b"DeviceCMYK" | b"CMYK" => Some(CsSpec::Cmyk),
                _ => Some(CsSpec::Rgb),
            },
            Ok((_, Object::Array(arr))) => {
                // 数组包裹的颜色空间（如 DeviceN 备选空间写法 `[ /CalRGB <<…>> ]`）
                if arr.len() == 1 {
                    if let Some(first) = arr.first() {
                        if matches!(first, Object::Array(_) | Object::Name(_)) {
                            if let Some(spec) = self.resolve_cs_spec(first, depth + 1) {
                                return Some(spec);
                            }
                        }
                    }
                }
                let head = arr.first().and_then(|o| o.as_name_str().ok()).unwrap_or("");
                match head {
                    "ICCBased" => {
                        let n = arr
                            .get(1)
                            .and_then(|o| self.doc.dereference(o).ok())
                            .and_then(|(_, o)| o.as_stream().ok())
                            .and_then(|s| s.dict.get(b"N").and_then(Object::as_i64).ok())
                            .unwrap_or(3);
                        match n {
                            1 => Some(CsSpec::Gray),
                            4 => Some(CsSpec::Cmyk),
                            _ => Some(CsSpec::Rgb),
                        }
                    }
                    // CalRGB 是线性 RGB：必须带 Gamma/Matrix/WhitePoint 转 sRGB
                    "CalRGB" => Some(self.calrgb_spec(arr.get(1))),
                    "Lab" => Some(CsSpec::Rgb),
                    "CalGray" => Some(self.calgray_spec(arr.get(1))),
                    "Indexed" | "I" => {
                        // [/Indexed base hival lookup]
                        let base = arr
                            .get(1)
                            .and_then(|o| self.resolve_cs_spec(o, depth + 1))
                            .unwrap_or(CsSpec::Rgb);
                        let base_comps = match base {
                            CsSpec::Gray => 1,
                            _ => 3,
                        };
                        let lookup = arr
                            .get(3)
                            .and_then(|o| self.doc.dereference(o).ok())
                            .map(|(_, o)| match o {
                                Object::String(s, _) => s.clone(),
                                Object::Stream(s) => s.get_plain_content().unwrap_or_default(),
                                _ => Vec::new(),
                            })
                            .unwrap_or_default();
                        Some(CsSpec::Indexed(lookup, base_comps))
                    }
                    "Separation" => {
                        // 带 tint transform 时保留函数（由 rgb_from_components 求值）；
                        // 否则退化为灰度近似
                        if let Some(t) = self.tint_spec(arr, 1) {
                            return Some(t);
                        }
                        Some(CsSpec::Gray)
                    }
                    "DeviceN" => {
                        let n = arr
                            .get(1)
                            .and_then(|o| self.doc.dereference(o).ok())
                            .and_then(|(_, o)| o.as_array().ok())
                            .map(|a| a.len())
                            .unwrap_or(1);
                        if let Some(t) = self.tint_spec(arr, n) {
                            return Some(t);
                        }
                        match n {
                            1 => Some(CsSpec::Gray),
                            3 => Some(CsSpec::Rgb),
                            4 => Some(CsSpec::Cmyk),
                            _ => Some(CsSpec::Gray),
                        }
                    }
                    _ => Some(CsSpec::Rgb),
                }
            }
            Ok((_, Object::Dictionary(d))) => {
                // 直接是 ICC 流字典
                let n = d.get(b"N").and_then(Object::as_i64).unwrap_or(3);
                match n {
                    1 => Some(CsSpec::Gray),
                    4 => Some(CsSpec::Cmyk),
                    _ => Some(CsSpec::Rgb),
                }
            }
            _ => None,
        }
    }

    /// 主解释循环（页内容流 / 表单 / 图案 / 注释外观共用）。
    fn run_content(&mut self, content: &[u8], scope: &Scope<'a>, state: &mut GState, depth: usize) {
        if depth > 12 {
            self.warnings
                .push("form/pattern nesting too deep — truncated".into());
            return;
        }
        let ops = tokenize(content);
        let mut stack: Vec<GState> = Vec::new();
        let mut path = PathBuilder::default();
        let mut pending_clip = false;

        for op in &ops {
            let args = &op.operands;
            match op.operator.as_str() {
                "q" => stack.push(state.clone()),
                "Q" => {
                    if let Some(s) = stack.pop() {
                        *state = s;
                    }
                }
                "cm" if args.len() >= 6 => {
                    state.ctm = state.ctm.transform(Mat::from_operands(args))
                }
                "BT" => {
                    self.flush_text();
                    state.tm_line = Mat::I;
                    state.tm_text = Mat::I;
                }
                "ET" => self.flush_text(),
                "Tf" if args.len() >= 2 => {
                    if let Tok::Name(name) = &args[0] {
                        let size = tok_num(args.get(1)).abs().max(0.1);
                        let (display, font_id, widths) = match scope.font(name) {
                            Some(f) => (f.display.clone(), f.font_id.clone(), f.widths.clone()),
                            None => ("Helvetica".to_string(), None, None),
                        };
                        if size != state.font_size
                            || display != state.font_name
                            || name != &state.font_key
                        {
                            self.flush_text();
                        }
                        state.font_size = size;
                        state.font_key = name.clone();
                        state.font_name = display;
                        state.font_id = font_id;
                        state.font_widths = widths;
                    }
                }
                "TL" => state.leading = tok_num(args.first()),
                "Tc" => state.char_spacing = tok_num(args.first()),
                "Ts" => state.rise = tok_num(args.first()),
                // 文本渲染模式（1 描边 / 2 填充+描边 / 3 不可见；4-7 含裁剪
                // 近似按填充处理）
                "Tr" => state.render_mode = tok_num(args.first()).clamp(0.0, 7.0) as u8,
                "Tw" => state.word_spacing = tok_num(args.first()),
                "Tz" => state.h_scale = tok_num(args.first()) / 100.0,
                "Td" | "TD" => {
                    let (tx, ty) = (tok_num(args.first()), tok_num(args.get(1)));
                    if op.operator == "TD" {
                        state.leading = -ty;
                    }
                    let (dx, dy) = state.tm_line.apply_linear(tx, ty);
                    state.tm_line.e += dx;
                    state.tm_line.f += dy;
                    state.tm_text = state.tm_line;
                }
                "Tm" if args.len() >= 6 => {
                    state.tm_line = Mat::from_operands(args);
                    state.tm_text = state.tm_line;
                }
                "T*" => {
                    let (dx, dy) = state.tm_line.apply_linear(0.0, -state.leading);
                    state.tm_line.e += dx;
                    state.tm_line.f += dy;
                    state.tm_text = state.tm_line;
                }
                "Tj" | "'" | "\"" if !args.is_empty() => {
                    if op.operator == "'" || op.operator == "\"" {
                        let (dx, dy) = state.tm_line.apply_linear(0.0, -state.leading);
                        state.tm_line.e += dx;
                        state.tm_line.f += dy;
                        state.tm_text = state.tm_line;
                    }
                    if op.operator == "\"" {
                        state.word_spacing = tok_num(args.first());
                        state.char_spacing = tok_num(args.get(1));
                    }
                    if let Some(bytes) = tok_str(args.last()) {
                        self.show_text(bytes, state, scope);
                    }
                }
                "TJ" if !args.is_empty() => {
                    if let Tok::Array(arr) = &args[0] {
                        for el in arr {
                            match el {
                                Tok::Str(bytes) => self.show_text(bytes, state, scope),
                                Tok::Num(k) => {
                                    // 字距（千分之 em）：大间隙判词间空格
                                    let delta = -*k / 1000.0 * state.font_size * state.h_scale;
                                    if delta > 0.28 * state.font_size {
                                        self.append_space();
                                    }
                                    let (dx, dy) = state.tm_text.apply_linear(delta, 0.0);
                                    state.tm_text.e += dx;
                                    state.tm_text.f += dy;
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "rg" if args.len() >= 3 => self.set_fill(state, Rgb::from_args(args)),
                "g" if !args.is_empty() => self.set_fill(state, Rgb::gray(tok_num(args.first()))),
                "k" if args.len() >= 4 => self.set_fill(state, Rgb::cmyk(args)),
                "RG" if args.len() >= 3 => state.stroke = Rgb::from_args(args),
                "G" if !args.is_empty() => state.stroke = Rgb::gray(tok_num(args.first())),
                "K" if args.len() >= 4 => state.stroke = Rgb::cmyk(args),
                "cs" if !args.is_empty() => {
                    if let Tok::Name(n) = &args[0] {
                        state.fill_cs = self.cs_by_name(n, scope);
                        state.fill_pattern = if scope.pattern(n).is_some() {
                            Some(n.clone())
                        } else {
                            None
                        };
                    }
                }
                "CS" if !args.is_empty() => {
                    if let Tok::Name(n) = &args[0] {
                        state.stroke_cs = self.cs_by_name(n, scope);
                    }
                }
                "sc" | "scn" | "SC" | "SCN" if !args.is_empty() => {
                    let is_fill = op.operator == "sc" || op.operator == "scn";
                    if let Some(Tok::Name(n)) = args.first() {
                        // /Pn scn → 图案
                        if is_fill && scope.pattern(n).is_some() {
                            state.fill_pattern = Some(n.clone());
                            continue;
                        }
                    }
                    let comps: Vec<f64> = args
                        .iter()
                        .filter_map(|t| match t {
                            Tok::Num(v) => Some(*v),
                            _ => None,
                        })
                        .collect();
                    let spec = if is_fill {
                        state.fill_cs.clone()
                    } else {
                        state.stroke_cs.clone()
                    };
                    let c = self.rgb_from_components(&comps, spec.as_ref());
                    if is_fill {
                        state.fill_pattern = None;
                        self.set_fill(state, c);
                    } else {
                        state.stroke = c;
                    }
                }
                "gs" if !args.is_empty() => {
                    if let Tok::Name(n) = &args[0] {
                        if let Some(obj) = scope.extgstate(n) {
                            if let Ok((_, Object::Dictionary(d))) = self.doc.dereference(obj) {
                                let d = d.clone();
                                if let Ok(v) = d.get(b"ca") {
                                    if let Some(f) = obj_f64(v) {
                                        state.fill_alpha = f.clamp(0.0, 1.0);
                                    }
                                }
                                if let Ok(v) = d.get(b"CA") {
                                    if let Some(f) = obj_f64(v) {
                                        state.stroke_alpha = f.clamp(0.0, 1.0);
                                    }
                                }
                                // 线宽/端点/连接/虚线（ExtGState 可覆盖图形状态）
                                if let Ok(v) = d.get(b"LW") {
                                    if let Some(f) = obj_f64(v) {
                                        state.line_width = f.max(0.0);
                                    }
                                }
                                if let Ok(v) = d.get(b"LC") {
                                    if let Some(f) = obj_f64(v) {
                                        state.line_cap = f.clamp(0.0, 2.0);
                                    }
                                }
                                if let Ok(v) = d.get(b"LJ") {
                                    if let Some(f) = obj_f64(v) {
                                        state.line_join = f.clamp(0.0, 2.0);
                                    }
                                }
                                // 软掩膜：记住表单与元数据，绘制元素时随元素输出
                                if let Ok(v) = d.get(b"SMask") {
                                    // /SMask 值可能是字典，也可能是「字典在流上」的
                                    // 间接对象（tika/testPDF_angles 的 57 0 R 是流）
                                    let sm_dict = match self.doc.dereference(v) {
                                        Ok((_, Object::Dictionary(sm))) => Some(sm.clone()),
                                        Ok((_, Object::Stream(st))) => Some(st.dict.clone()),
                                        _ => None,
                                    };
                                    if let Some(sm) = sm_dict {
                                        state.smask_subtype = sm
                                            .get(b"S")
                                            .and_then(Object::as_name_str)
                                            .unwrap_or("Luminosity")
                                            .to_string();
                                        state.smask_backdrop = sm
                                            .get(b"BC")
                                            .ok()
                                            .and_then(|o| self.doc.dereference(o).ok())
                                            .and_then(|(_, o)| obj_f64_array(o));
                                        // /TR 可为字典（FunctionType 2/3）或数组
                                        // /TR 可为字典（FunctionType 2/3）或数组；
                                        // 注意要序列化「解引用后」的对象（引用会变 null）
                                        state.smask_transfer = sm.get(b"TR").ok().and_then(|o| {
                                            match self.doc.dereference(o) {
                                                Ok((_, obj @ Object::Dictionary(_)))
                                                | Ok((_, obj @ Object::Array(_))) => {
                                                    Some(object_to_json(obj))
                                                }
                                                _ => None,
                                            }
                                        });
                                        match sm.get(b"G") {
                                            Ok(g) => {
                                                state.smask_form = Some(g.clone());
                                            }
                                            // /G 缺失（/S /Alpha 时掩膜即图形状态本身）
                                            _ => state.smask_form = None,
                                        }
                                    } else {
                                        state.smask_form = None;
                                    }
                                }
                                if let Ok(v) = d.get(b"D") {
                                    if let Ok((_, Object::Array(arr))) = self.doc.dereference(v) {
                                        let arr = arr.clone();
                                        let dash: Vec<f64> = arr
                                            .first()
                                            .and_then(|o| self.doc.dereference(o).ok())
                                            .and_then(|(_, o)| obj_f64_array(o))
                                            .unwrap_or_default();
                                        let phase = arr
                                            .get(1)
                                            .and_then(|o| self.doc.dereference(o).ok())
                                            .and_then(|(_, o)| obj_f64(o))
                                            .unwrap_or(0.0);
                                        state.dash =
                                            if dash.is_empty() || dash.iter().all(|v| *v == 0.0) {
                                                None
                                            } else {
                                                Some(dash)
                                            };
                                        state.dash_phase = phase;
                                    }
                                }
                                if let Ok(v) = d.get(b"BM") {
                                    let name = match v {
                                        Object::Name(n) => String::from_utf8_lossy(n).to_string(),
                                        Object::Array(a) => a
                                            .first()
                                            .and_then(|o| o.as_name_str().ok())
                                            .unwrap_or("")
                                            .to_string(),
                                        _ => String::new(),
                                    };
                                    if !name.is_empty() && name != "Normal" && name != "Compatible"
                                    {
                                        state.blend = Some(name);
                                    } else {
                                        state.blend = None;
                                    }
                                }
                            }
                        }
                    }
                }
                "d" if !args.is_empty() => {
                    let pattern = match args.first() {
                        Some(Tok::Array(arr)) => arr
                            .iter()
                            .filter_map(|t| match t {
                                Tok::Num(v) => Some(*v),
                                _ => None,
                            })
                            .collect::<Vec<f64>>(),
                        _ => Vec::new(),
                    };
                    state.dash = if pattern.is_empty() {
                        None
                    } else {
                        Some(pattern)
                    };
                    state.dash_phase = tok_num(args.get(1));
                }
                "w" if !args.is_empty() => {
                    let w = tok_num(args.first()).max(0.0);
                    if (w - state.line_width).abs() > 1e-6 {
                        self.flush_text();
                    }
                    state.line_width = w;
                }
                // 线端/连接样式（虚线外观由此决定，如 50-50 dash 圆帽）
                "J" if !args.is_empty() => state.line_cap = tok_num(args.first()).clamp(0.0, 2.0),
                "j" if !args.is_empty() => {
                    state.line_join = tok_num(args.first()).clamp(0.0, 2.0);
                }
                "re" if args.len() >= 4 => {
                    self.flush_text();
                    let (x, y, w, h) = (
                        tok_num(args.first()),
                        tok_num(args.get(1)),
                        tok_num(args.get(2)),
                        tok_num(args.get(3)),
                    );
                    path.segs.push(PathSegment {
                        op: "re".into(),
                        points: vec![[x, y], [w, h]],
                    });
                }
                "m" if args.len() >= 2 => {
                    self.flush_text();
                    path.segs.push(PathSegment {
                        op: "m".into(),
                        points: vec![[tok_num(args.first()), tok_num(args.get(1))]],
                    });
                }
                "l" if args.len() >= 2 => {
                    path.segs.push(PathSegment {
                        op: "l".into(),
                        points: vec![[tok_num(args.first()), tok_num(args.get(1))]],
                    });
                }
                "c" if args.len() >= 6 => {
                    path.segs.push(PathSegment {
                        op: "c".into(),
                        points: vec![
                            [tok_num(args.first()), tok_num(args.get(1))],
                            [tok_num(args.get(2)), tok_num(args.get(3))],
                            [tok_num(args.get(4)), tok_num(args.get(5))],
                        ],
                    });
                }
                "v" if args.len() >= 4 => {
                    // 首控制点 = 当前点：近似为直线到第二控制点再到终点
                    path.segs.push(PathSegment {
                        op: "c".into(),
                        points: vec![
                            path.segs
                                .last()
                                .and_then(|s| s.points.last().copied())
                                .unwrap_or([0.0, 0.0]),
                            [tok_num(args.first()), tok_num(args.get(1))],
                            [tok_num(args.get(2)), tok_num(args.get(3))],
                        ],
                    });
                }
                "y" if args.len() >= 4 => {
                    // 末控制点 = 终点
                    path.segs.push(PathSegment {
                        op: "c".into(),
                        points: vec![
                            [tok_num(args.first()), tok_num(args.get(1))],
                            [tok_num(args.get(2)), tok_num(args.get(3))],
                            [tok_num(args.get(2)), tok_num(args.get(3))],
                        ],
                    });
                }
                "h" => path.segs.push(PathSegment {
                    op: "h".into(),
                    points: Vec::new(),
                }),
                "W" | "W*" => pending_clip = true,
                "n" => {
                    if pending_clip {
                        self.apply_clip(state, &path);
                    }
                    pending_clip = false;
                    path.clear();
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" => {
                    if !path.is_empty() {
                        self.flush_text();
                        self.paint_path(&path, state, scope, op.operator.as_str(), depth);
                    }
                    path.clear();
                }
                "Do" if !args.is_empty() => {
                    if let Tok::Name(name) = &args[0] {
                        if let Some(obj) = scope.xobject(name) {
                            let obj = obj.clone();
                            self.do_xobject(&obj, state, scope, depth);
                        }
                    }
                }
                "sh" if !args.is_empty() => {
                    if let Tok::Name(name) = &args[0] {
                        if let Some(obj) = scope.shading(name) {
                            let obj = obj.clone();
                            self.paint_shading(&obj, state);
                        }
                    }
                }
                "BI" => {
                    // 内联图像：操作数 [dict, data]
                    if let (Some(Tok::Dict(dict)), Some(data)) =
                        (args.first(), tok_str(args.get(1)))
                    {
                        self.flush_text();
                        let dict = dict.clone();
                        let data = data.to_vec();
                        self.push_inline_image(&dict, &data, state);
                    }
                }
                _ => {}
            }
        }
        self.flush_text();
    }

    fn set_fill(&mut self, state: &mut GState, c: Rgb) {
        if state.fill != c {
            self.flush_text();
        }
        state.fill = c;
        state.fill_pattern = None;
    }

    fn apply_clip(&mut self, state: &mut GState, path: &PathBuilder) {
        // 规范化（re 展开、孤立 m 剔除）后取包围盒，保证与 clip_path 一致
        let segs = normalized_path_segs(&path.segs, &state.ctm);
        let Some(bb) = segments_bbox(&segs) else {
            return;
        };
        state.clip = Some(match state.clip {
            Some(c) => [
                c[0].max(bb[0]),
                c[1].max(bb[1]),
                c[2].min(bb[2]),
                c[3].min(bb[3]),
            ],
            None => bb,
        });
        if !segs.is_empty() {
            state.clip_path = Some(segs);
        }
    }

    /// ExtGState /SMask 的表单 → SoftMask（按表单对象 id 缓存）。
    /// 掩膜表单内容与其中的图片落盘一次，重建时原样挂回 ExtGState。
    fn capture_smask(&mut self, state: &GState) -> Option<Box<crate::model::SoftMask>> {
        let form = state.smask_form.clone()?;
        let Ok((id, Object::Stream(st))) = self.doc.dereference(&form) else {
            return None;
        };
        if let Some(id) = id {
            if let Some(m) = self.smask_element_cache.get(&id) {
                return Some(m.clone());
            }
        }
        let dict = st.dict.clone();
        let content = st.get_plain_content().unwrap_or_default();
        if content.is_empty() {
            return None;
        }
        self.shared.mask_idx += 1;
        let rel = format!("masks/mask-{:03}.bin", self.shared.mask_idx);
        if std::fs::create_dir_all(self.out_dir.join("masks")).is_err()
            || std::fs::write(self.out_dir.join(&rel), &content).is_err()
        {
            return None;
        }
        let f64_arr = |k: &[u8]| -> Vec<f64> {
            dict.get(k)
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| obj_f64_array(o))
                .unwrap_or_default()
        };
        let group_cs = dict
            .get(b"Group")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok().cloned())
            .and_then(|g| {
                g.get(b"CS")
                    .ok()
                    .and_then(|o| o.as_name_str().ok())
                    .map(str::to_string)
            });
        // 掩膜表单内的图片（软阴影常用灰度位图）
        let mut images: BTreeMap<String, String> = BTreeMap::new();
        if let Some(res) = dict
            .get(b"Resources")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok())
            .cloned()
        {
            if let Ok(xo) = self.deref_dict(&res, b"XObject") {
                let entries: Vec<(Vec<u8>, Object)> =
                    xo.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                for (name, obj) in entries {
                    let Ok((Some(oid), Object::Stream(_))) = self.doc.dereference(&obj) else {
                        continue;
                    };
                    let Ok(st) = self.doc.get_object(oid).and_then(Object::as_stream) else {
                        continue;
                    };
                    // 只取图片 XObject（表单要按结构重建，不能当图片解码）
                    let is_image = st
                        .dict
                        .get(b"Subtype")
                        .and_then(Object::as_name_str)
                        .map(|s| s == "Image")
                        .unwrap_or(false);
                    if !is_image {
                        continue;
                    }
                    let d = st.dict.clone();
                    if let Some((src, ..)) = self.save_image_full(oid, &d, state) {
                        images.insert(String::from_utf8_lossy(&name).to_string(), src);
                    }
                }
            }
        }
        let mut extgstates: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        if let Some(res) = dict
            .get(b"Resources")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok())
            .cloned()
        {
            if let Ok(gs) = self.deref_dict(&res, b"ExtGState") {
                for (k, v) in gs.iter() {
                    extgstates.insert(String::from_utf8_lossy(k).to_string(), object_to_json(v));
                }
            }
        }
        // 掩膜表单内的渐变（渐变掩膜很常见）：按表单空间重建
        let mut shadings: BTreeMap<String, Box<crate::model::ShadingDef>> = BTreeMap::new();
        if let Some(res) = dict
            .get(b"Resources")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok())
            .cloned()
        {
            if let Ok(sh) = self.deref_dict(&res, b"Shading") {
                let entries: Vec<(Vec<u8>, Object)> =
                    sh.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                for (name, obj) in entries {
                    if let Ok(def) = self.shading_def(&obj, Mat::I, None, None, 1.0) {
                        shadings.insert(String::from_utf8_lossy(&name).to_string(), Box::new(def));
                    }
                }
            }
        }
        let mask = Box::new(crate::model::SoftMask {
            subtype: state.smask_subtype.clone(),
            content: rel,
            bbox: f64_arr(b"BBox"),
            matrix: f64_arr(b"Matrix"),
            group_cs,
            images,
            extgstates,
            shadings,
            backdrop: state.smask_backdrop.clone(),
            transfer: state.smask_transfer.clone(),
        });
        if let Some(id) = id {
            self.smask_element_cache.insert(id, mask.clone());
        }
        Some(mask)
    }

    /// 绘制路径（矩形 → Rect；其余 → Path）。裁剪路径不会走到这里。
    fn paint_path(
        &mut self,
        path: &PathBuilder,
        state: &GState,
        scope: &Scope<'a>,
        op: &str,
        depth: usize,
    ) {
        let is_fill = matches!(op, "f" | "F" | "f*" | "B" | "B*" | "b" | "b*");
        let is_stroke = matches!(op, "S" | "s" | "B" | "B*" | "b" | "b*");
        let even_odd = op.ends_with('*');
        let fill_color = if is_fill { Some(state.fill) } else { None };
        let stroke_color = if is_stroke { Some(state.stroke) } else { None };

        // 图案填充
        if is_fill {
            if let Some(pname) = &state.fill_pattern {
                if let Some(obj) = scope.pattern(pname) {
                    let obj = obj.clone();
                    if self.paint_pattern(&obj, path, state, scope, depth) {
                        return;
                    }
                }
            }
        }

        if let Some((x, y, w, h)) = path.as_single_rect() {
            let origin = state.ctm.apply(x, y);
            let rot = state.ctm.rotation_deg();
            let (mut dx, mut dy) = if rot.abs() < 1e-9 {
                (
                    origin.0.min(origin.0 + (w * state.ctm.a)),
                    origin.1.min(origin.1 + (h * state.ctm.d)),
                )
            } else {
                (origin.0, origin.1)
            };
            let (mut dw, mut dh) = if rot.abs() < 1e-9 {
                ((w * state.ctm.a).abs(), (h * state.ctm.d).abs())
            } else {
                (w * state.ctm.x_scale(), h * state.ctm.y_scale())
            };
            if dw.abs() < 0.01 && dh.abs() < 0.01 {
                return;
            }
            // 裁剪框约束（轴对齐时按交集裁剪）
            if rot.abs() < 1e-9 {
                if let Some(c) = state.clip {
                    let (x0, y0, x1, y1) = (dx, dy, dx + dw, dy + dh);
                    let (nx0, ny0) = (x0.max(c[0]), y0.max(c[1]));
                    let (nx1, ny1) = (x1.min(c[2]), y1.min(c[3]));
                    if nx1 - nx0 <= 0.01 || ny1 - ny0 <= 0.01 {
                        return;
                    }
                    dx = nx0;
                    dy = ny0;
                    dw = nx1 - nx0;
                    dh = ny1 - ny0;
                }
            }
            let cap_smask = self.capture_smask(state);
            self.elements.push(Element::Rect {
                x: round_f(dx, 2),
                y: round_f(dy, 2),
                w: round_f(dw, 2),
                h: round_f(dh, 2),
                fill: fill_color.map(|c| c.hex()),
                stroke: stroke_color.map(|c| c.hex()),
                line_width: round_f(state.line_width * state.ctm.x_scale(), 2),
                rotation: round_f(rot, 3),
                alpha: round_f(state.fill_alpha, 4),
                stroke_alpha: if (state.stroke_alpha - state.fill_alpha).abs() > 1e-6 {
                    Some(round_f(state.stroke_alpha, 4))
                } else {
                    None
                },
                blend: state.blend.clone(),
                smask: cap_smask,
            });
            return;
        }

        // 一般路径：转换 re 段为折线
        let mut segs: Vec<PathSegment> = Vec::new();
        for s in &path.segs {
            match s.op.as_str() {
                "re" => {
                    let (x, y) = (s.points[0][0], s.points[0][1]);
                    let (w, h) = (s.points[1][0], s.points[1][1]);
                    segs.push(PathSegment {
                        op: "m".into(),
                        points: vec![[x, y]],
                    });
                    segs.push(PathSegment {
                        op: "l".into(),
                        points: vec![[x + w, y]],
                    });
                    segs.push(PathSegment {
                        op: "l".into(),
                        points: vec![[x + w, y + h]],
                    });
                    segs.push(PathSegment {
                        op: "l".into(),
                        points: vec![[x, y + h]],
                    });
                    segs.push(PathSegment {
                        op: "h".into(),
                        points: Vec::new(),
                    });
                }
                _ => segs.push(s.clone()),
            }
        }
        let has_points = segs.iter().any(|s| !s.points.is_empty());
        if !has_points {
            return;
        }
        // 路径点已按 CTM 变换到设备空间
        let mut out_segs: Vec<PathSegment> = Vec::new();
        for s in segs {
            let pts: Vec<[f64; 2]> = s
                .points
                .iter()
                .map(|p| {
                    let (x, y) = state.ctm.apply(p[0], p[1]);
                    [round_f(x, 2), round_f(y, 2)]
                })
                .collect();
            out_segs.push(PathSegment {
                op: s.op,
                points: pts,
            });
        }
        let cap_smask = self.capture_smask(state);
        self.elements.push(Element::Path {
            segments: out_segs,
            fill: fill_color.map(|c| c.hex()),
            stroke: stroke_color.map(|c| c.hex()),
            line_width: round_f(state.line_width * state.ctm.x_scale(), 2),
            even_odd,
            // 虚线长度随 CTM 缩放（坐标已烘焙到设备空间）
            dash: state.dash.as_ref().map(|d| {
                d.iter()
                    .map(|v| round_f(v * state.ctm.x_scale(), 2))
                    .collect()
            }),
            dash_phase: round_f(state.dash_phase * state.ctm.x_scale(), 2),
            line_cap: round_f(state.line_cap, 2),
            line_join: round_f(state.line_join, 2),
            clip: state.clip_path.clone(),
            alpha: round_f(state.fill_alpha, 4),
            // 描边透明度与填充不同时单独记录（如 ExtGState ca≠CA）
            stroke_alpha: if (state.stroke_alpha - state.fill_alpha).abs() > 1e-6 {
                Some(round_f(state.stroke_alpha, 4))
            } else {
                None
            },
            blend: state.blend.clone(),
            smask: cap_smask,
        });
    }

    /// 网格渐变（ShadingType 4-7）：解码后的码流写入 shadings/ 直通保留，
    /// 重建时用 matrix 把着色空间放回页面（顶点坐标不重写）。
    fn paint_mesh_shading(
        &mut self,
        obj: &Object,
        state: &GState,
        placement: Mat,
        bbox: Option<[f64; 4]>,
        clip: Option<Vec<PathSegment>>,
    ) {
        let Ok((_, sobj)) = self.doc.dereference(obj) else {
            return;
        };
        let dict = match sobj {
            Object::Stream(s) => s.dict.clone(),
            Object::Dictionary(d) => d.clone(),
            _ => return,
        };
        let content = match sobj {
            Object::Stream(s) => s.get_plain_content().unwrap_or_default(),
            _ => Vec::new(),
        };
        if content.is_empty() {
            return;
        }
        // 着色空间 → 设备空间：placement 之上再叠 shading 字典自身 /Matrix
        let m = dict
            .get(b"Matrix")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o))
            .filter(|v| v.len() >= 6)
            .map(|v| {
                placement.transform(Mat {
                    a: v[0],
                    b: v[1],
                    c: v[2],
                    d: v[3],
                    e: v[4],
                    f: v[5],
                })
            })
            .unwrap_or(placement);
        self.shared.mesh_idx += 1;
        let name = format!("mesh-{:03}.bin", self.shared.mesh_idx);
        if std::fs::create_dir_all(self.out_dir.join("shadings")).is_err() {
            return;
        }
        if std::fs::write(self.out_dir.join("shadings").join(&name), &content).is_err() {
            return;
        }
        let num = |k: &[u8], d: i64| dict.get(k).and_then(Object::as_i64).ok().unwrap_or(d);
        let mesh = crate::model::MeshShading {
            shading_type: num(b"ShadingType", 4),
            bits_per_coordinate: num(b"BitsPerCoordinate", 8),
            bits_per_component: num(b"BitsPerComponent", 8),
            bits_per_flag: num(b"BitsPerFlag", 2),
            decode: dict
                .get(b"Decode")
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| obj_f64_array(o))
                .unwrap_or_default(),
            background: dict
                .get(b"Background")
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| obj_f64_array(o)),
            data: format!("shadings/{name}"),
            matrix: [
                round_f(m.a, 6),
                round_f(m.b, 6),
                round_f(m.c, 6),
                round_f(m.d, 6),
                round_f(m.e, 2),
                round_f(m.f, 2),
            ],
            color_space: {
                // 分量数必须与原空间一致（否则顶点流解析错位 → 彩虹碎片）
                let n = match dict.get(b"ColorSpace").map(|o| self.resolve_cs_spec(o, 0)) {
                    Ok(Some(crate::images::CsSpec::Gray)) => 1,
                    Ok(Some(crate::images::CsSpec::Cmyk)) => 4,
                    Ok(Some(crate::images::CsSpec::Indexed(..))) => 1,
                    Ok(Some(crate::images::CsSpec::Tinted { n, .. })) => n,
                    Ok(Some(_)) => 3,
                    _ => 3,
                };
                let named = dict
                    .get(b"ColorSpace")
                    .ok()
                    .and_then(|o| o.as_name_str().ok())
                    .map(str::to_string);
                match (named, n) {
                    (Some(name), _) => name,
                    (None, 1) => "DeviceGray".to_string(),
                    (None, 4) => "DeviceCMYK".to_string(),
                    _ => "DeviceRGB".to_string(),
                }
            },
            func: None,
            func_stream: None,
        };
        // /Function 原样保留：缺失会让渲染器按分量数误解析颜色流
        let (mesh_func, mesh_func_stream) = match dict.get(b"Function") {
            Ok(f) => match self.doc.dereference(f) {
                Ok((_, Object::Dictionary(fd))) => {
                    (Some(object_to_json(&Object::Dictionary(fd.clone()))), None)
                }
                Ok((_, Object::Stream(fs))) => {
                    let data = fs.get_plain_content().unwrap_or_default();
                    if data.is_empty() {
                        (None, None)
                    } else {
                        self.shared.func_idx += 1;
                        let frel = format!("shadings/fn-{:03}.bin", self.shared.func_idx);
                        let mut fd = fs.dict.clone();
                        fd.remove(b"Filter");
                        fd.remove(b"DecodeParms");
                        fd.remove(b"Length");
                        if std::fs::write(self.out_dir.join(&frel), &data).is_ok() {
                            (
                                None,
                                Some(crate::model::FuncStreamData {
                                    dict: object_to_json(&Object::Dictionary(fd)),
                                    data: frel,
                                }),
                            )
                        } else {
                            (None, None)
                        }
                    }
                }
                _ => (None, None),
            },
            Err(_) => (None, None),
        };
        let mesh = crate::model::MeshShading {
            func: mesh_func,
            func_stream: mesh_func_stream,
            ..mesh
        };
        self.elements.push(Element::Shading(ShadingDef {
            kind: "mesh".into(),
            coords: Vec::new(),
            stops: Vec::new(),
            extend: Vec::new(),
            bbox,
            clip,
            alpha: round_f(state.fill_alpha, 4),
            mask: None,
            mesh: Some(Box::new(mesh)),
            func0: None,
            matrix: None,
        }));
    }

    /// 平铺图案填充：把图案图块内容递归解析为元素列表。
    fn paint_pattern(
        &mut self,
        obj: &Object,
        path: &PathBuilder,
        state: &GState,
        scope: &Scope<'a>,
        depth: usize,
    ) -> bool {
        let Ok((_, pobj)) = self.doc.dereference(obj) else {
            return false;
        };
        let (dict, content) = match pobj {
            Object::Stream(s) => (s.dict.clone(), s.get_plain_content().unwrap_or_default()),
            Object::Dictionary(d) => (d.clone(), Vec::new()),
            _ => return false,
        };
        let ptype = dict
            .get(b"PatternType")
            .and_then(Object::as_i64)
            .unwrap_or(1);
        if ptype == 2 {
            // 着色图案：解析为 Shading 元素，裁剪区域 = 被填充路径
            if let Ok(sh) = dict.get(b"Shading") {
                let sh = sh.clone();
                let matrix = dict
                    .get(b"Matrix")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .filter(|v| v.len() >= 6)
                    .map(|v| Mat {
                        a: v[0],
                        b: v[1],
                        c: v[2],
                        d: v[3],
                        e: v[4],
                        f: v[5],
                    });
                // 填充路径规范化后作为裁剪区域（re 展开、孤立 m 剔除，
                // bbox 与输出段一致）
                let segs = normalized_path_segs(&path.segs, &state.ctm);
                let bbox = segments_bbox(&segs);
                // 退化路径（如 `0 0 m h B`，零面积）不绘制：否则着色图案
                // 会因缺裁剪铺满整页（fpdf2 text_fill_gradient）
                if segs.is_empty() || bbox.is_none() {
                    return true;
                }
                // 图案 Matrix 映射到所在内容流的默认用户空间：只叠加入口
                // CTM，不受流内 cm 影响（MuPDF 最小样例实证；此前叠
                // state.ctm 造成双重翻转，渐变轴落到页外被裁没）
                let placement = state.base.transform(matrix.unwrap_or(Mat::I));
                // 已有显式裁剪（如 Type3 字形轮廓）且比填充路径更小时，用它裁剪
                let clip = match (&state.clip_path, &bbox) {
                    (Some(cp), Some(fb)) => {
                        let cbb = segments_bbox(cp);
                        let area = |b: &[f64; 4]| (b[2] - b[0]).max(0.0) * (b[3] - b[1]).max(0.0);
                        match cbb {
                            Some(cb) if area(&cb) < area(fb) => Some((cb, cp.clone())),
                            _ => Some((*fb, segs)),
                        }
                    }
                    (Some(cp), None) => segments_bbox(cp).map(|cb| (cb, cp.clone())),
                    (None, Some(fb)) => Some((*fb, segs)),
                    (None, None) => None,
                };
                match clip {
                    Some((bb, cl)) => self.paint_shading_ex(&sh, state, Some((bb, cl)), placement),
                    None => self.paint_shading_ex(&sh, state, None, placement),
                }
                return true;
            }
            return false;
        }
        if ptype != 1 || content.is_empty() || self.pattern_depth >= 2 {
            return false;
        }
        // 图块内容用独立作用域解析
        let mut inner = scope.clone();
        if let Some(res) = dict
            .get(b"Resources")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok())
            .cloned()
        {
            self.merge_resources(&mut inner, &res);
        }

        let bbox = dict
            .get(b"BBox")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o))
            .unwrap_or(vec![0.0, 0.0, 1.0, 1.0]);
        if bbox.len() < 4 {
            return false;
        }
        let x_step = dict
            .get(b"XStep")
            .ok()
            .and_then(obj_f64)
            .unwrap_or(bbox[2] - bbox[0]);
        let y_step = dict
            .get(b"YStep")
            .ok()
            .and_then(obj_f64)
            .unwrap_or(bbox[3] - bbox[1]);
        if x_step.abs() < 1e-6 || y_step.abs() < 1e-6 {
            return false;
        }
        let matrix = dict
            .get(b"Matrix")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o))
            .and_then(|v| {
                if v.len() >= 6 {
                    Some([v[0], v[1], v[2], v[3], v[4], v[5]])
                } else {
                    None
                }
            });

        // 图块内容：以单位状态在图案空间解释
        let saved = std::mem::take(&mut self.elements);
        self.pattern_depth += 1;
        let mut inner_state = GState::default();
        inner_state.fill = state.fill;
        inner_state.stroke = state.stroke;
        self.run_content(&content, &inner, &mut inner_state, depth + 1);
        self.pattern_depth -= 1;
        let inner_elements = std::mem::replace(&mut self.elements, saved);

        // 图案空间 → 页面空间：Matrix 后再经当前 CTM
        let m = matrix.unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
        let combined = state.ctm.transform(Mat {
            a: m[0],
            b: m[1],
            c: m[2],
            d: m[3],
            e: m[4],
            f: m[5],
        });
        let mapped: Vec<Element> = inner_elements
            .into_iter()
            .map(|e| map_element(e, &combined))
            .collect();

        let Some((x, y, w, h)) = path.as_single_rect() else {
            return false;
        };
        let origin = state.ctm.apply(x, y);
        let dw = (w * state.ctm.x_scale()).abs();
        let dh = (h * state.ctm.y_scale()).abs();
        let bbox_dev = [
            origin.0.min(origin.0 + dw),
            origin.1,
            origin.0.max(origin.0 + dw),
            origin.1 + dh,
        ];
        self.elements.push(Element::PatternRect {
            x: round_f(bbox_dev[0], 2),
            y: round_f(bbox_dev[1], 2),
            w: round_f(dw, 2),
            h: round_f(dh, 2),
            pattern: PatternDef {
                bbox: [bbox[0], bbox[1], bbox[2], bbox[3]],
                x_step,
                y_step,
                matrix: matrix.map(|_m| {
                    let t = combined;
                    [t.a, t.b, t.c, t.d, t.e, t.f]
                }),
                elements: mapped,
            },
            alpha: round_f(state.fill_alpha, 4),
            blend: state.blend.clone(),
        });
        true
    }

    /// 渐变（sh）：解析类型/坐标/函数为色标。
    fn paint_shading(&mut self, obj: &Object, state: &GState) {
        // sh 操作符：着色坐标位于当前用户空间，随流内 cm 变换
        // （MuPDF 实证：翻转 cm 内的 sh 渐变随之翻转）
        self.paint_shading_ex(obj, state, None, state.ctm);
    }

    /// 渐变（带裁剪区域与已合成的着色空间矩阵 placement：
    /// `sh` 操作符 = 当前 CTM；着色图案 = 入口 CTM ∘ 图案 Matrix）。
    /// 从 shading 对象构造 ShadingDef（设备空间）；页面/图案/软掩膜共用。
    fn shading_def(
        &mut self,
        obj: &Object,
        placement: Mat,
        bbox: Option<[f64; 4]>,
        clip: Option<Vec<PathSegment>>,
        alpha: f64,
    ) -> Result<ShadingDef, String> {
        let (_, sobj) = self
            .doc
            .dereference(obj)
            .map_err(|_| "shading dereference failed".to_string())?;
        let dict = match sobj {
            Object::Stream(s) => s.dict.clone(),
            Object::Dictionary(d) => d.clone(),
            _ => return Err("shading is not a dictionary".into()),
        };
        // shading 字典自身的 /Matrix（若存在）在 placement 之上再叠一层
        let m = dict
            .get(b"Matrix")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o))
            .filter(|v| v.len() >= 6)
            .map(|v| {
                placement.transform(Mat {
                    a: v[0],
                    b: v[1],
                    c: v[2],
                    d: v[3],
                    e: v[4],
                    f: v[5],
                })
            })
            .unwrap_or(placement);
        let stype = dict
            .get(b"ShadingType")
            .and_then(Object::as_i64)
            .unwrap_or(0);
        if !(2..=3).contains(&stype) {
            return Err(format!("shading type {stype} unsupported — skipped"));
        }
        let coords = dict
            .get(b"Coords")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o))
            .unwrap_or_default();
        if coords.len() < 4 {
            return Err("shading coords missing".into());
        }
        // 径向渐变在非相似变换下会被压成椭圆：半径无法单独表达，改为记录
        // 着色空间坐标 + 矩阵（重建时 `cm` 后再 `sh`）
        let anisotropic = {
            let xs = m.x_scale();
            let ys = (m.c * m.c + m.d * m.d).sqrt();
            let shear = (m.a * m.c + m.b * m.d).abs() / (xs * ys).max(1e-9);
            (xs - ys).abs() / xs.max(ys).max(1e-9) > 1e-3 || shear > 1e-3
        };
        let radial_matrix = (stype == 3 && anisotropic).then_some([
            round_f(m.a, 6),
            round_f(m.b, 6),
            round_f(m.c, 6),
            round_f(m.d, 6),
            round_f(m.e, 2),
            round_f(m.f, 2),
        ]);
        let dev_coords: Vec<f64> = if radial_matrix.is_some() {
            coords.iter().map(|v| round_f(*v, 3)).collect()
        } else if stype == 2 {
            let (x0, y0) = m.apply(coords[0], coords[1]);
            let (x1, y1) = m.apply(coords[2], coords[3]);
            vec![
                round_f(x0, 2),
                round_f(y0, 2),
                round_f(x1, 2),
                round_f(y1, 2),
            ]
        } else {
            let (x0, y0) = m.apply(coords[0], coords[1]);
            let (x1, y1) = m.apply(coords[3], coords[4]);
            vec![
                round_f(x0, 2),
                round_f(y0, 2),
                round_f(coords[2] * m.x_scale(), 2),
                round_f(x1, 2),
                round_f(y1, 2),
                round_f(coords[5] * m.x_scale(), 2),
            ]
        };
        let cs_spec = dict
            .get(b"ColorSpace")
            .ok()
            .map(|o| self.resolve_cs_spec(o, 0))
            .unwrap_or(Some(CsSpec::Rgb));
        let func = dict
            .get(b"Function")
            .ok()
            .cloned()
            .ok_or("shading function missing")?;
        let stops = self
            .shading_stops(&func, cs_spec.as_ref(), 0)
            .ok_or("shading function unsupported — skipped")?;
        if stops.len() < 2 {
            return Err("shading stops < 2".into());
        }
        let extend = dict
            .get(b"Extend")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_array().ok())
            .map(|a| a.iter().map(|v| v.as_bool().unwrap_or(false)).collect())
            .unwrap_or_else(|| vec![false, false]);
        // FunctionType 0（采样函数）原样直通：按色标抽样会漏掉条纹
        let func0 = self.extract_func0(&func);
        Ok(ShadingDef {
            kind: if stype == 3 {
                "radial".into()
            } else {
                "axial".into()
            },
            coords: dev_coords,
            stops,
            extend,
            bbox,
            clip,
            alpha,
            mask: None,
            mesh: None,
            func0,
            matrix: radial_matrix,
        })
    }

    /// FunctionType 0 采样函数 → shadings/fn-NNN.bin（重建时原样写回）。
    fn extract_func0(&mut self, func: &Object) -> Option<Box<crate::model::Func0Data>> {
        let stream = match self.doc.dereference(func) {
            Ok((_, Object::Stream(s))) => s.clone(),
            _ => return None,
        };
        if stream
            .dict
            .get(b"FunctionType")
            .and_then(Object::as_i64)
            .ok()
            != Some(0)
        {
            return None;
        }
        let data = stream.get_plain_content().ok()?;
        if data.is_empty() {
            return None;
        }
        let arr = |doc: &lopdf::Document, k: &[u8]| -> Vec<f64> {
            stream
                .dict
                .get(k)
                .ok()
                .and_then(|o| doc.dereference(o).ok())
                .and_then(|(_, o)| obj_f64_array(o))
                .unwrap_or_default()
        };
        let size: Vec<i64> = match stream.dict.get(b"Size") {
            Ok(Object::Array(a)) => a.iter().filter_map(|v| v.as_i64().ok()).collect(),
            Ok(o) => o.as_i64().ok().map(|v| vec![v]).unwrap_or_default(),
            Err(_) => return None,
        };
        if size.is_empty() {
            return None;
        }
        let bits = stream
            .dict
            .get(b"BitsPerSample")
            .and_then(Object::as_i64)
            .unwrap_or(8);
        self.shared.func_idx += 1;
        let rel = format!("shadings/fn-{:03}.bin", self.shared.func_idx);
        std::fs::create_dir_all(self.out_dir.join("shadings")).ok()?;
        std::fs::write(self.out_dir.join(&rel), &data).ok()?;
        let (encode, decode, range) = (
            arr(self.doc, b"Encode"),
            arr(self.doc, b"Decode"),
            arr(self.doc, b"Range"),
        );
        Some(Box::new(crate::model::Func0Data {
            size,
            bits_per_sample: bits,
            encode,
            decode,
            range,
            data: rel,
        }))
    }

    fn paint_shading_ex(
        &mut self,
        obj: &Object,
        state: &GState,
        clip: Option<([f64; 4], Vec<PathSegment>)>,
        placement: Mat,
    ) {
        let (bbox, clip_segs) = match clip {
            Some((bb, segs)) => (
                Some([
                    round_f(bb[0], 2),
                    round_f(bb[1], 2),
                    round_f(bb[2], 2),
                    round_f(bb[3], 2),
                ]),
                Some(segs),
            ),
            None => (
                state.clip.map(|c| {
                    [
                        round_f(c[0], 2),
                        round_f(c[1], 2),
                        round_f(c[2], 2),
                        round_f(c[3], 2),
                    ]
                }),
                state.clip_path.clone(),
            ),
        };
        // 网格渐变（ShadingType 4-7）：码流直通（顶点在着色空间内）
        if let Ok((_, sobj)) = self.doc.dereference(obj) {
            let stype = match sobj {
                Object::Stream(s) => s.dict.get(b"ShadingType").and_then(Object::as_i64).ok(),
                Object::Dictionary(d) => d.get(b"ShadingType").and_then(Object::as_i64).ok(),
                _ => None,
            };
            if matches!(stype, Some(4..=7)) {
                self.paint_mesh_shading(obj, state, placement, bbox, clip_segs);
                return;
            }
        }
        match self.shading_def(
            obj,
            placement,
            bbox,
            clip_segs,
            round_f(state.fill_alpha, 4),
        ) {
            Ok(mut def) => {
                // 亮度软掩膜：把掩膜表单内的渐变作为 mask 一并记录
                if let Some(form) = state.smask_form.clone() {
                    def.mask = self.mask_gradient(&form, state).map(Box::new);
                }
                self.elements.push(Element::Shading(def));
            }
            Err(w) => {
                if w.contains("unsupported") {
                    self.warnings.push(w);
                }
            }
        }
    }

    /// 亮度软掩膜表单（/SMask /G）内的渐变，设备空间。
    /// 表单内容通常是 `/Pattern cs /Pn scn … re f`；取其中的 PatternType 2
    /// 或 /Shading。表单 Matrix 缺省为身份，即表单空间 = 应用 gs 时的用户空间。
    fn mask_gradient(&mut self, form: &Object, state: &GState) -> Option<ShadingDef> {
        let (_, fobj) = self.doc.dereference(form).ok()?;
        let stream = fobj.as_stream().ok()?;
        let dict = stream.dict.clone();
        // 表单 BBox → 设备空间（掩膜覆盖范围）
        let bbox = dict
            .get(b"BBox")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o))
            .filter(|v| v.len() >= 4)
            .map(|v| {
                let (x0, y0) = state.ctm.apply(v[0], v[1]);
                let (x1, y1) = state.ctm.apply(v[2], v[3]);
                [
                    round_f(x0.min(x1), 2),
                    round_f(y0.min(y1), 2),
                    round_f(x0.max(x1), 2),
                    round_f(y0.max(y1), 2),
                ]
            });
        let res = dict
            .get(b"Resources")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok())
            .cloned()?;
        if let Ok(pats) = self.deref_dict(&res, b"Pattern") {
            for (_, pobj) in pats.iter() {
                let Ok((_, pd)) = self.doc.dereference(pobj) else {
                    continue;
                };
                let Ok(pdict) = pd.as_dict() else { continue };
                if pdict
                    .get(b"PatternType")
                    .and_then(Object::as_i64)
                    .unwrap_or(1)
                    != 2
                {
                    continue;
                }
                let Ok(sh) = pdict.get(b"Shading") else {
                    continue;
                };
                let pm = pdict
                    .get(b"Matrix")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .filter(|v| v.len() >= 6)
                    .map(|v| Mat {
                        a: v[0],
                        b: v[1],
                        c: v[2],
                        d: v[3],
                        e: v[4],
                        f: v[5],
                    })
                    .unwrap_or(Mat::I);
                // 图案 Matrix 映射到表单内容流的默认用户空间 = gs 生效时的用户空间
                if let Ok(d) = self.shading_def(sh, state.ctm.transform(pm), bbox, None, 1.0) {
                    return Some(d);
                }
            }
        }
        if let Ok(shs) = self.deref_dict(&res, b"Shading") {
            for (_, sh) in shs.iter() {
                if let Ok(d) = self.shading_def(sh, state.ctm, bbox, None, 1.0) {
                    return Some(d);
                }
            }
        }
        None
    }

    /// 着色函数 → 色标（type 2/3；type 0 采样；type 4 不支持）。
    fn shading_stops(
        &self,
        func: &Object,
        cs: Option<&CsSpec>,
        depth: usize,
    ) -> Option<Vec<ShadingStop>> {
        if depth > 4 {
            return None;
        }
        let (_, fobj) = self.doc.dereference(func).ok()?;
        let dict = match fobj {
            Object::Stream(s) => s.dict.clone(),
            Object::Dictionary(d) => d.clone(),
            _ => return None,
        };
        let ftype = dict
            .get(b"FunctionType")
            .and_then(Object::as_i64)
            .unwrap_or(-1);
        let content = match fobj {
            Object::Stream(s) => s.get_plain_content().unwrap_or_default(),
            _ => Vec::new(),
        };
        let eval = |t: f64| -> Option<String> {
            let out = self.eval_function(&dict, &content, t, cs, depth)?;
            Some(Rgb(out.0, out.1, out.2).hex())
        };
        match ftype {
            2 => {
                let c0 = dict
                    .get(b"C0")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_else(|| vec![0.0]);
                let c1 = dict
                    .get(b"C1")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_else(|| vec![1.0]);
                let n = dict.get(b"N").and_then(Object::as_i64).unwrap_or(1).max(1) as i32;
                let mut stops = Vec::new();
                for i in 0..=4 {
                    let t = i as f64 / 4.0;
                    let comps: Vec<f64> = c0
                        .iter()
                        .zip(c1.iter())
                        .map(|(a, b)| a + (b - a) * t.powf(n as f64))
                        .collect();
                    let c = self.rgb_from_components(&comps, cs);
                    stops.push(ShadingStop {
                        offset: t,
                        color: c.hex(),
                    });
                }
                Some(stops)
            }
            3 => {
                let subs = dict
                    .get(b"Functions")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| o.as_array().ok())
                    .cloned()
                    .unwrap_or_default();
                let bounds = dict
                    .get(b"Bounds")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_default();
                if subs.is_empty() {
                    return None;
                }
                let mut stops = Vec::new();
                let n_sub = subs.len();
                for (i, _sub) in subs.iter().enumerate() {
                    let lo = if i == 0 {
                        0.0
                    } else {
                        bounds.get(i - 1).copied().unwrap_or(0.0)
                    };
                    let hi = if i + 1 == n_sub {
                        1.0
                    } else {
                        bounds.get(i).copied().unwrap_or(1.0)
                    };
                    for k in 0..=2 {
                        let t = lo + (hi - lo) * (k as f64 / 2.0);
                        let color = eval(t).unwrap_or_else(|| "#000000".into());
                        stops.push(ShadingStop { offset: t, color });
                    }
                }
                Some(stops)
            }
            4 => {
                // PostScript 计算器：通过 eval 均匀采样 16 点
                let mut stops = Vec::new();
                for i in 0..=16 {
                    let t = i as f64 / 16.0;
                    let color = eval(t).unwrap_or_else(|| "#000000".into());
                    stops.push(ShadingStop { offset: t, color });
                }
                Some(stops)
            }
            0 => {
                let n = dict
                    .get(b"Size")
                    .and_then(Object::as_i64)
                    .unwrap_or(0)
                    .max(2) as usize;
                let mut stops = Vec::new();
                for i in 0..=n.min(16) {
                    let t = i as f64 / n.min(16) as f64;
                    let color = eval(t).unwrap_or_else(|| "#000000".into());
                    stops.push(ShadingStop { offset: t, color });
                }
                Some(stops)
            }
            _ => None,
        }
    }

    fn eval_function(
        &self,
        dict: &Dictionary,
        content: &[u8],
        t: f64,
        cs: Option<&CsSpec>,
        depth: usize,
    ) -> Option<(f32, f32, f32)> {
        if depth > 4 {
            return None;
        }
        let ftype = dict
            .get(b"FunctionType")
            .and_then(Object::as_i64)
            .unwrap_or(-1);
        match ftype {
            4 => {
                // PostScript 计算器：码流即程序，输入在栈上，输出取栈顶 n_out
                let out = images::eval_ps_calculator(content, &[t])?;
                let n_out = dict
                    .get(b"Range")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .map(|v| (v.len() / 2).max(1))
                    .unwrap_or(3);
                if out.len() < n_out {
                    return None;
                }
                let vals = &out[out.len() - n_out..];
                let comps: Vec<f64> = if n_out == 1 {
                    vec![vals[0], vals[0], vals[0]]
                } else {
                    vals.to_vec()
                };
                let c = self.rgb_from_components(&comps, cs);
                Some((c.0, c.1, c.2))
            }
            0 => {
                // 采样函数：Size + BitsPerSample + Encode/Decode + 码流样本
                // （此前未实现 → 输出全黑，pdf.js/issue6113 78.4）
                let size: Vec<usize> = dict
                    .get(b"Size")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| o.as_array().ok())
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_i64().ok())
                            .map(|v| v.max(1) as usize)
                            .collect()
                    })
                    .unwrap_or_default();
                if size.is_empty() {
                    return None;
                }
                let bps = dict
                    .get(b"BitsPerSample")
                    .and_then(Object::as_i64)
                    .unwrap_or(8)
                    .max(1) as usize;
                let encode = dict
                    .get(b"Encode")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_default();
                let decode = dict
                    .get(b"Decode")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_default();
                // 单维输入（Size 只有一项）取最近样本；输出分量数由 Decode 推断
                let m = size[0].max(1);
                let lo_enc = encode.first().copied().unwrap_or(0.0);
                let hi_enc = encode.get(1).copied().unwrap_or((m - 1) as f64);
                let enc = (lo_enc + (hi_enc - lo_enc) * t)
                    .round()
                    .clamp(0.0, (m - 1) as f64) as usize;
                // 分量数：优先 /Decode，其次 /Range（4 输出 = CMYK 的 Separation
                // tint 很常见，按 3 推断会让采样跨距错位 → 颜色乱，pdfplumber/issue-316）
                let range = dict
                    .get(b"Range")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_default();
                let n_comp = if decode.len() >= 2 {
                    decode.len() / 2
                } else if range.len() >= 2 {
                    range.len() / 2
                } else {
                    3
                };
                let mut comps = Vec::with_capacity(n_comp);
                for c in 0..n_comp {
                    let idx = enc * n_comp + c;
                    let raw = match bps {
                        8 => content.get(idx).copied().unwrap_or(0) as f64 / 255.0,
                        16 => {
                            let hi = content.get(idx * 2).copied().unwrap_or(0) as f64;
                            let lo = content.get(idx * 2 + 1).copied().unwrap_or(0) as f64;
                            (hi * 256.0 + lo) / 65535.0
                        }
                        1 | 2 | 4 => {
                            let bit = idx * bps;
                            let byte = content.get(bit / 8).copied().unwrap_or(0);
                            let shift = 8 - bps - (bit % 8);
                            let v = (byte >> shift) & ((1u16 << bps) - 1) as u8;
                            v as f64 / ((1u16 << bps) - 1) as f64
                        }
                        _ => 0.0,
                    };
                    let dlo = decode.get(c * 2).copied().unwrap_or(0.0);
                    let dhi = decode.get(c * 2 + 1).copied().unwrap_or(1.0);
                    comps.push(dlo + (dhi - dlo) * raw);
                }
                let c = self.rgb_from_components(&comps, cs);
                Some((c.0, c.1, c.2))
            }
            2 => {
                let c0 = dict
                    .get(b"C0")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_else(|| vec![0.0]);
                let c1 = dict
                    .get(b"C1")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_else(|| vec![1.0]);
                let n = dict.get(b"N").and_then(Object::as_i64).unwrap_or(1).max(1) as i32;
                let comps: Vec<f64> = c0
                    .iter()
                    .zip(c1.iter())
                    .map(|(a, b)| a + (b - a) * t.powf(n as f64))
                    .collect();
                let c = self.rgb_from_components(&comps, cs);
                Some((c.0, c.1, c.2))
            }
            3 => {
                let subs = dict
                    .get(b"Functions")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| o.as_array().ok())
                    .cloned()
                    .unwrap_or_default();
                let bounds = dict
                    .get(b"Bounds")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_default();
                let encode = dict
                    .get(b"Encode")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_default();
                if subs.is_empty() {
                    return None;
                }
                let mut idx = subs.len() - 1;
                for (i, b) in bounds.iter().enumerate() {
                    if t < *b {
                        idx = i;
                        break;
                    }
                }
                let lo = if idx == 0 { 0.0 } else { bounds[idx - 1] };
                let hi = bounds.get(idx).copied().unwrap_or(1.0);
                let local = if (hi - lo).abs() < 1e-12 {
                    0.0
                } else {
                    (t - lo) / (hi - lo)
                };
                let (e0, e1) = (
                    encode.get(idx * 2).copied().unwrap_or(0.0),
                    encode.get(idx * 2 + 1).copied().unwrap_or(1.0),
                );
                let t2 = e0 + (e1 - e0) * local;
                let sub = subs[idx].clone();
                let (_, sdict) = self.doc.dereference(&sub).ok()?;
                let (sc, sd) = match sdict {
                    Object::Stream(s) => {
                        (s.get_plain_content().unwrap_or_default(), s.dict.clone())
                    }
                    Object::Dictionary(d) => (Vec::new(), d.clone()),
                    _ => return None,
                };
                self.eval_function(&sd, &sc, t2, cs, depth + 1)
            }
            _ => None,
        }
    }

    /// 内联图像（BI/ID/EI）。
    fn push_inline_image(&mut self, dict: &Dictionary, data: &[u8], state: &GState) {
        let width = dict
            .get(b"W")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        let height = dict
            .get(b"H")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(0);
        let bpc = dict
            .get(b"BPC")
            .ok()
            .and_then(|o| o.as_i64().ok())
            .unwrap_or(8);
        let image_mask = dict
            .get(b"IM")
            .ok()
            .and_then(|o| o.as_bool().ok())
            .unwrap_or(false);
        let filters: Vec<String> = dict
            .get(b"F")
            .ok()
            .map(|o| match o {
                Object::Name(n) => vec![String::from_utf8_lossy(n).to_lowercase()],
                Object::Array(a) => a
                    .iter()
                    .filter_map(|x| x.as_name_str().ok().map(|s| s.to_lowercase()))
                    .collect(),
                _ => Vec::new(),
            })
            .unwrap_or_default()
            // 行内图像的过滤名是缩写（Fl/CCF/…），只在内联语境合法；
            // 产物统一成完整名，重建为 XObject 时才可解析
            .into_iter()
            .map(|f| match f.as_str() {
                "ahx" => "asciihexdecode".to_string(),
                "a85" => "ascii85decode".to_string(),
                "lzw" => "lzwdecode".to_string(),
                "fl" => "flatedecode".to_string(),
                "rl" => "runlengthdecode".to_string(),
                "ccf" => "ccittfaxdecode".to_string(),
                "dct" => "dctdecode".to_string(),
                other => other.to_string(),
            })
            .collect();
        let parms: Vec<Value> = dict
            .get(b"DP")
            .ok()
            .map(|o| vec![object_to_json(o)])
            .unwrap_or_default();
        // 内联语境用缩写 /D（同 /Decode）；ImageMask + [1 0] 决定掩膜极性
        let decode_arr: Option<Vec<f64>> = dict
            .get(b"D")
            .or_else(|_| dict.get(b"Decode"))
            .ok()
            .and_then(|o| match o {
                Object::Array(a) => Some(
                    a.iter()
                        .filter_map(|x| x.as_float().ok().map(|v| v as f64))
                        .collect::<Vec<f64>>(),
                ),
                _ => None,
            })
            .filter(|v| !v.is_empty());
        let cs = dict.get(b"CS").ok().and_then(|o| match o.as_name_str() {
            Ok("G") => Some(CsSpec::Gray),
            Ok("CMYK") => Some(CsSpec::Cmyk),
            Ok("RGB") => Some(CsSpec::Rgb),
            _ => self.resolve_cs_spec(o, 0),
        });
        let mask_fill = {
            let c = state.fill;
            Some((
                (c.0.clamp(0.0, 1.0) * 255.0) as u8,
                (c.1.clamp(0.0, 1.0) * 255.0) as u8,
                (c.2.clamp(0.0, 1.0) * 255.0) as u8,
            ))
        };
        // 颜色键遮罩（/Mask 数组；内联图像少见，同样支持）
        let color_key: Option<Vec<f64>> = dict
            .get(b"Mask")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o));
        let decoded = images::decode_image(
            data,
            &filters,
            &parms,
            width,
            height,
            bpc,
            cs.as_ref(),
            image_mask,
            mask_fill,
            decode_arr.as_deref(),
            color_key.as_deref(),
            None,
        );
        self.shared.media_idx += 1;
        let idx = self.shared.media_idx;
        match decoded {
            Decoded::Png(bytes) => {
                let name = format!("img-{idx:03}.png");
                if std::fs::write(self.out_dir.join("media").join(&name), bytes).is_ok() {
                    self.images_extracted += 1;
                    self.push_image_element(&format!("media/{name}"), state, None);
                }
            }
            Decoded::Passthrough {
                ext,
                bytes,
                filter,
                decode_parms,
            } => {
                let name = format!("img-{idx:03}.{ext}");
                // 内联图无法被浏览器直接解码（JPX/JBIG2/CCITT）：同样生成预览 PNG
                let preview = if images::preview::has_decoder(&filter) {
                    images::preview::decode_preview_png(
                        &filter,
                        &decode_parms,
                        &bytes,
                        None,
                        width,
                        height,
                        decode_arr.as_deref(),
                    )
                    .and_then(|png| {
                        let pname = format!("img-{idx:03}.preview.png");
                        std::fs::write(self.out_dir.join("media").join(&pname), png)
                            .ok()
                            .map(|_| format!("media/{pname}"))
                    })
                } else {
                    None
                };
                if std::fs::write(self.out_dir.join("media").join(&name), bytes).is_ok() {
                    self.images_extracted += 1;
                    // JBIG2Globals 是独立码流（/DecodeParms 里的间接引用）：
                    // 落到 media 并记录，重建时再挂回 DecodeParms
                    let mut globals = None;
                    if filter.iter().any(|f| f == "jbig2decode") {
                        let gobj = dict
                            .get(b"DecodeParms")
                            .ok()
                            .and_then(|o| self.doc.dereference(o).ok())
                            .and_then(|(_, o)| o.as_dict().ok().cloned())
                            .and_then(|pd| pd.get(b"JBIG2Globals").ok().cloned());
                        if let Some(gobj) = gobj {
                            let bytes = self
                                .doc
                                .dereference(&gobj)
                                .ok()
                                .and_then(|(_, o)| o.as_stream().ok())
                                .and_then(|st| st.get_plain_content().ok());
                            if let Some(b) = bytes {
                                let gname = format!("{name}.jb2g");
                                if std::fs::write(self.out_dir.join("media").join(&gname), b)
                                    .is_ok()
                                {
                                    globals = Some(format!("media/{gname}"));
                                }
                            }
                        }
                    }
                    let mut decode_parms = decode_parms;
                    // globals 已单独保存，DecodeParms 里的 null 占位清掉
                    if globals.is_some() {
                        for v in decode_parms.iter_mut() {
                            if let Some(o) = v.as_object_mut() {
                                o.remove("JBIG2Globals");
                            }
                        }
                    }
                    let pt = ImagePassthrough {
                        filter,
                        decode_parms: decode_parms.clone(),
                        color_space: cs_name(&cs),
                        icc_components: 0,
                        bpc,
                        width,
                        height,
                        decode: None,
                        image_mask,
                        smask: None,
                        smask_params: None,
                        matte: None,
                        globals,
                    };
                    self.push_image_element_ex(
                        &format!("media/{name}"),
                        state,
                        Some(pt),
                        None,
                        None,
                        preview,
                    );
                }
            }
        }
    }

    /// 图片元素放置（旋转/缩放按 CTM 解析）。
    fn push_image_element(
        &mut self,
        src: &str,
        state: &GState,
        passthrough: Option<ImagePassthrough>,
    ) {
        self.push_image_element_ex(src, state, passthrough, None, None, None);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_image_element_ex(
        &mut self,
        src: &str,
        state: &GState,
        passthrough: Option<ImagePassthrough>,
        smask: Option<String>,
        smask_params: Option<Box<ImagePassthrough>>,
        preview: Option<String>,
    ) {
        let (x0, y0) = state.ctm.apply(0.0, 0.0);
        let rot = state.ctm.rotation_deg();
        let w = state.ctm.x_scale();
        let h = state.ctm.y_scale();
        if w < 0.5 || h < 0.5 {
            return;
        }
        self.elements.push(Element::Image {
            src: src.to_string(),
            preview,
            x: round_f(x0, 2),
            y: round_f(y0, 2),
            w: round_f(w, 2),
            h: round_f(h, 2),
            rotation: round_f(rot, 3),
            alpha: round_f(state.fill_alpha, 4),
            blend: state.blend.clone(),
            clip: state.clip_path.clone(),
            smask,
            smask_params,
            passthrough,
        });
    }

    fn do_xobject(&mut self, obj: &Object, state: &GState, scope: &Scope<'a>, depth: usize) {
        let Ok((id, xobj)) = self.doc.dereference(obj) else {
            return;
        };
        let stream = match xobj {
            Object::Stream(s) => s,
            _ => return,
        };
        let subtype = stream
            .dict
            .get(b"Subtype")
            .and_then(Object::as_name_str)
            .unwrap_or("");
        match subtype {
            "Image" => {
                self.flush_text();
                let Some(id) = id else { return };
                let dict = stream.dict.clone();
                let (src, smask, smask_params, preview) = if let Some(src) =
                    self.image_cache.get(&id).cloned()
                {
                    let info = self.smask_cache.get(&id).cloned().unwrap_or((None, None));
                    let pv = self.preview_cache.get(&id).cloned().unwrap_or(None);
                    (src, info.0, info.1, pv)
                } else {
                    let Some((src, smask, sp, pv)) = self.save_image_full(id, &dict, state) else {
                        return;
                    };
                    self.image_cache.insert(id, src.clone());
                    self.smask_cache.insert(id, (smask.clone(), sp.clone()));
                    self.preview_cache.insert(id, pv.clone());
                    (src, smask, sp, pv)
                };
                self.push_image_element_ex(
                    &src,
                    state,
                    self.passthrough_of(&id),
                    smask,
                    smask_params,
                    preview,
                );
            }
            "Form" => {
                let dict = stream.dict.clone();
                let mut content = stream.get_plain_content().unwrap_or_default();
                if content.is_empty() {
                    if let (Some(raw), Some(oid)) = (self.raw, id) {
                        if let Some(b) = raw_stream_bytes(raw, oid) {
                            content = b;
                        }
                    }
                }
                // 表单矩阵 + 当前 CTM
                let m = dict
                    .get(b"Matrix")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .filter(|v| v.len() >= 6)
                    .map(|v| Mat {
                        a: v[0],
                        b: v[1],
                        c: v[2],
                        d: v[3],
                        e: v[4],
                        f: v[5],
                    })
                    .unwrap_or(Mat::I);

                if let Some(res) = dict
                    .get(b"Resources")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| o.as_dict().ok())
                    .cloned()
                {
                    let mut inner = scope.clone();
                    self.merge_resources(&mut inner, &res);
                }
                // 透明组 Form（/Group /S /Transparency）：保留组语义。
                // /Group 常是间接引用，判定时必须解引用（否则整族表单退化成
                // 普通 Form：组 alpha/BM/软掩膜全部丢失，tika/testPDF_angles）
                if dict
                    .get(b"Group")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| o.as_dict().ok())
                    .map(|g| {
                        g.get(b"S").ok().and_then(|o| o.as_name_str().ok()) == Some("Transparency")
                    })
                    .unwrap_or(false)
                {
                    let saved = std::mem::take(&mut self.elements);
                    let mut inner_state = state.clone();
                    // 子元素在组空间：CTM = Form Matrix（缺省单位阵）
                    inner_state.ctm = m;
                    inner_state.base = m;
                    inner_state.tm_line = Mat::I;
                    inner_state.tm_text = Mat::I;
                    // Do 时刻的 ca/CA/BM 作用于「组合成」（组作为整体绘制时），
                    // 不属于组内元素：否则子元素与组各应用一次（knockout 双重 alpha）
                    inner_state.fill_alpha = 1.0;
                    inner_state.stroke_alpha = 1.0;
                    inner_state.blend = None;
                    let mut inner = scope.clone();
                    if let Some(res) = dict
                        .get(b"Resources")
                        .ok()
                        .and_then(|o| self.doc.dereference(o).ok())
                        .and_then(|(_, o)| o.as_dict().ok())
                        .cloned()
                    {
                        self.merge_resources(&mut inner, &res);
                    }
                    self.run_content(&content, &inner, &mut inner_state, depth + 1);
                    let children = std::mem::replace(&mut self.elements, saved);
                    let gdict = dict
                        .get(b"Group")
                        .ok()
                        .and_then(|o| self.doc.dereference(o).ok())
                        .and_then(|(_, o)| o.as_dict().ok())
                        .cloned();
                    let (iso, kn, cs_name) = gdict
                        .as_ref()
                        .map(|g| {
                            (
                                g.get(b"I")
                                    .ok()
                                    .and_then(|o| o.as_bool().ok())
                                    .unwrap_or(false),
                                // /K 允许布尔或整数（/K true 与 /K 1 等价）
                                g.get(b"K")
                                    .ok()
                                    .map(|o| match o {
                                        Object::Boolean(b) => *b,
                                        other => {
                                            other.as_i64().ok().map(|v| v != 0).unwrap_or(false)
                                        }
                                    })
                                    .unwrap_or(false),
                                g.get(b"CS")
                                    .ok()
                                    .and_then(|o| o.as_name_str().ok())
                                    .map(str::to_string),
                            )
                        })
                        .unwrap_or((false, false, None));
                    let bbox = dict
                        .get(b"BBox")
                        .ok()
                        .and_then(|o| self.doc.dereference(o).ok())
                        .and_then(|(_, o)| obj_f64_array(o))
                        .filter(|v| v.len() >= 4)
                        .map(|v| {
                            [
                                round_f(v[0], 2),
                                round_f(v[1], 2),
                                round_f(v[2], 2),
                                round_f(v[3], 2),
                            ]
                        })
                        .unwrap_or([0.0, 0.0, 0.0, 0.0]);
                    self.flush_text();
                    let cap_smask = self.capture_smask(state);
                    self.elements.push(Element::Group {
                        // 放置矩阵：Do 时的 CTM（组空间→页面空间）
                        matrix: [
                            round_f(state.ctm.a, 6),
                            round_f(state.ctm.b, 6),
                            round_f(state.ctm.c, 6),
                            round_f(state.ctm.d, 6),
                            round_f(state.ctm.e, 2),
                            round_f(state.ctm.f, 2),
                        ],
                        bbox,
                        group: Some(crate::model::GroupInfo {
                            isolated: iso,
                            knockout: kn,
                            cs: cs_name,
                        }),
                        children,
                        alpha: round_f(state.fill_alpha, 4),
                        smask: cap_smask,
                        blend: state.blend.clone(),
                    });
                    return;
                }
                let mut inner_state = state.clone();
                inner_state.ctm = state.ctm.transform(m);
                // Form 入口即该内容流的默认用户空间
                inner_state.base = inner_state.ctm;
                inner_state.tm_line = Mat::I;
                inner_state.tm_text = Mat::I;
                let mut inner = scope.clone();
                if let Some(res) = dict
                    .get(b"Resources")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| o.as_dict().ok())
                    .cloned()
                {
                    self.merge_resources(&mut inner, &res);
                }
                self.run_content(&content, &inner, &mut inner_state, depth + 1);
            }
            _ => {}
        }
    }

    fn passthrough_of(&self, id: &(u32, u16)) -> Option<ImagePassthrough> {
        let src = self.image_cache.get(id)?;
        self.passthrough_cache.get(src).cloned()
    }

    /// 图片落盘（滤镜链解码 / 原码流直通 + SMask）。
    fn save_image(&mut self, id: (u32, u16), dict: &Dictionary, state: &GState) -> Option<String> {
        let (src, ..) = self.save_image_full(id, dict, state)?;
        Some(src)
    }

    #[allow(clippy::type_complexity)]
    fn save_image_full(
        &mut self,
        id: (u32, u16),
        dict: &Dictionary,
        state: &GState,
    ) -> Option<(
        String,
        Option<String>,
        Option<Box<ImagePassthrough>>,
        Option<String>,
    )> {
        let stream = self.doc.get_object(id).and_then(Object::as_stream).ok()?;
        let raw = stream.content.clone();
        let filters: Vec<String> = dict
            .get(b"Filter")
            .map(|f| match f {
                Object::Name(n) => vec![String::from_utf8_lossy(n).to_lowercase()],
                Object::Array(arr) => arr
                    .iter()
                    .filter_map(|o| o.as_name_str().ok().map(|s| s.to_lowercase()))
                    .collect(),
                _ => Vec::new(),
            })
            .unwrap_or_default();
        let parms: Vec<Value> = match dict.get(b"DecodeParms") {
            Ok(Object::Array(arr)) => arr
                .iter()
                .map(|o| {
                    let (_, o) = self.doc.dereference(o).unwrap_or((None, o));
                    object_to_json(o)
                })
                .collect(),
            Ok(o) => {
                let (_, o) = self.doc.dereference(o).unwrap_or((None, o));
                vec![object_to_json(o)]
            }
            Err(_) => Vec::new(),
        };
        let width = self.num_field(dict, b"Width").unwrap_or(0);
        let height = self.num_field(dict, b"Height").unwrap_or(0);
        let bpc = self.num_field(dict, b"BitsPerComponent").unwrap_or(8);
        let image_mask = dict
            .get(b"ImageMask")
            .and_then(Object::as_bool)
            .unwrap_or(false);
        let cs = if image_mask {
            None
        } else {
            dict.get(b"ColorSpace")
                .ok()
                .map(|o| self.resolve_cs_spec(o, 0))
                .unwrap_or(Some(CsSpec::Gray))
        };
        let decode = dict
            .get(b"Decode")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o));
        let mask_fill = {
            let c = state.fill;
            Some((
                (c.0.clamp(0.0, 1.0) * 255.0) as u8,
                (c.1.clamp(0.0, 1.0) * 255.0) as u8,
                (c.2.clamp(0.0, 1.0) * 255.0) as u8,
            ))
        };

        // SMask（记录其直通参数，重建时一并还原）
        let (smask_src, smask_params) =
            match dict.get(b"SMask").ok().and_then(|o| o.as_reference().ok()) {
                Some(sm_id) => {
                    let sdict = self
                        .doc
                        .get_object(sm_id)
                        .and_then(Object::as_stream)
                        .ok()
                        .map(|s| s.dict.clone());
                    match sdict {
                        Some(sdict) => {
                            let mut st = state.clone();
                            st.fill = Rgb(1.0, 1.0, 1.0);
                            let src = self.save_image(sm_id, &sdict, &st);
                            let mut params = src
                                .as_ref()
                                .and_then(|s| self.passthrough_cache.get(s).cloned())
                                .map(Box::new);
                            // /Matte 挂在 SMask 流字典上：阅读器用它对父图像做
                            // 解预乘，丢失会让颜色整体偏移（pdfium/matte.pdf）。
                            // SMask 被解码为 PNG 时没有直通参数，合成仅承载
                            // matte 的最小结构（repack 按扩展名走像素嵌入，
                            // 只取其中的 matte 写回流字典）
                            let matte = sdict
                                .get(b"Matte")
                                .ok()
                                .and_then(|o| self.doc.dereference(o).ok())
                                .and_then(|(_, o)| obj_f64_array(o));
                            if let Some(p) = params.as_mut() {
                                p.matte = matte;
                            } else if let (Some(_), Some(m)) = (&src, matte) {
                                params = Some(Box::new(ImagePassthrough {
                                    filter: Vec::new(),
                                    decode_parms: Vec::new(),
                                    color_space: "DeviceGray".into(),
                                    icc_components: 0,
                                    bpc: 8,
                                    width: 0,
                                    height: 0,
                                    decode: None,
                                    image_mask: false,
                                    smask: None,
                                    smask_params: None,
                                    matte: Some(m),
                                    globals: None,
                                }));
                            }
                            (src, params)
                        }
                        None => (None, None),
                    }
                }
                None => (None, None),
            };

        // 颜色键遮罩：/Mask 数组（min/max 逐分量）
        let color_key: Option<Vec<f64>> = dict
            .get(b"Mask")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o));
        let tint_spec = match &cs {
            Some(CsSpec::Tinted {
                n, content, alt, ..
            }) => Some(images::TintSpec {
                n: *n,
                code: content.clone(),
                alt: (**alt).clone(),
            }),
            _ => None,
        };
        let decoded = images::decode_image(
            &raw,
            &filters,
            &parms,
            width,
            height,
            bpc,
            cs.as_ref(),
            image_mask,
            mask_fill,
            decode.as_deref(),
            color_key.as_deref(),
            tint_spec.as_ref(),
        );
        self.shared.media_idx += 1;
        let idx = self.shared.media_idx;
        match decoded {
            Decoded::Png(bytes) => {
                let name = format!("img-{idx:03}.png");
                std::fs::write(self.out_dir.join("media").join(&name), bytes).ok()?;
                self.images_extracted += 1;
                Some((format!("media/{name}"), smask_src, smask_params, None))
            }
            Decoded::Passthrough {
                ext,
                bytes,
                filter,
                decode_parms,
            } => {
                let name = format!("img-{idx:03}.{ext}");
                std::fs::write(self.out_dir.join("media").join(&name), bytes).ok()?;
                self.images_extracted += 1;
                let src = format!("media/{name}");
                // JBIG2Globals 是独立码流（/DecodeParms 里的间接引用，JSON 化
                // 会变成 null）：存成 media 附属文件，重建时挂回 DecodeParms
                let mut globals = None;
                let mut globals_bytes: Option<Vec<u8>> = None;
                if filter.iter().any(|f| f == "jbig2decode") {
                    let gobj = dict
                        .get(b"DecodeParms")
                        .ok()
                        .and_then(|o| self.doc.dereference(o).ok())
                        .and_then(|(_, o)| o.as_dict().ok().cloned())
                        .and_then(|pd| pd.get(b"JBIG2Globals").ok().cloned());
                    if let Some(gobj) = gobj {
                        let gbytes = self
                            .doc
                            .dereference(&gobj)
                            .ok()
                            .and_then(|(_, o)| o.as_stream().ok())
                            .and_then(|st| st.get_plain_content().ok());
                        if let Some(b) = gbytes {
                            let gname = format!("img-{idx:03}.jb2g");
                            if std::fs::write(self.out_dir.join("media").join(&gname), &b).is_ok() {
                                globals = Some(format!("media/{gname}"));
                                globals_bytes = Some(b);
                            }
                        }
                    }
                }
                // 原图无法被浏览器直接解码（JPX/JBIG2/CCITT）：解码出像素生成
                // 预览 PNG，供 render / web 预览使用（repack 仍走原字节）
                let preview = images::preview::decode_preview_png(
                    &filter,
                    &decode_parms,
                    std::fs::read(self.out_dir.join("media").join(&name))
                        .ok()
                        .as_deref()
                        .unwrap_or(&[]),
                    globals_bytes.as_deref(),
                    width,
                    height,
                    decode.as_deref(),
                )
                .and_then(|png| {
                    let pname = format!("img-{idx:03}.preview.png");
                    std::fs::write(self.out_dir.join("media").join(&pname), png)
                        .ok()
                        .map(|_| format!("media/{pname}"))
                });
                let mut decode_parms = decode_parms;
                if globals.is_some() {
                    for v in decode_parms.iter_mut() {
                        if let Some(o) = v.as_object_mut() {
                            o.remove("JBIG2Globals");
                        }
                    }
                }
                let pt = ImagePassthrough {
                    filter,
                    decode_parms,
                    color_space: cs_name(&cs),
                    icc_components: icc_components(&cs),
                    bpc,
                    width,
                    height,
                    decode,
                    image_mask,
                    smask: smask_src.clone(),
                    smask_params: smask_params.clone(),
                    matte: None,
                    globals,
                };
                self.passthrough_cache.insert(src.clone(), pt);
                Some((src, smask_src, smask_params, preview))
            }
        }
    }

    /// 解码一段文本并合并进当前 run（Type3 走字形过程解释）。
    fn show_text(&mut self, bytes: &[u8], state: &mut GState, scope: &Scope<'a>) {
        if let Some(slot) = scope.font(&state.font_key) {
            if let Some(t3) = slot.type3 {
                self.flush_text();
                self.show_type3(bytes, state, scope, t3, 0);
                return;
            }
        }
        let decoded = {
            let decoder = scope.font(&state.font_key).map(|f| &f.decoder);
            match decoder {
                Some(d) => decode_string(d, bytes),
                None => bytes.iter().map(|&b| b as char).collect::<String>(),
            }
        };
        let decoded = fix_radicals(decoded).trim_end_matches('\0').to_string();
        if decoded.is_empty() {
            return;
        }
        let tm_dev = state.ctm.transform(state.tm_text);
        // 文本上升：基线沿文本 y 方向偏移
        let (x, y) = tm_dev.apply(0.0, state.rise);
        let size_dev = state.font_size * tm_dev.x_scale();
        // 镜像基（det<0）：rotation_deg 只覆盖正常定向，det<0 时按
        // R(θ)·diag(1,-1) 分解，θ = atan2(b, a)，repack 据此还原 Tm
        let det = tm_dev.a * tm_dev.d - tm_dev.b * tm_dev.c;
        let mirror = det < -1e-9;
        let rotation = if mirror {
            round_f(tm_dev.b.atan2(tm_dev.a).to_degrees(), 3)
        } else {
            round_f(tm_dev.rotation_deg(), 3)
        };
        // 非正交基（剪切 / 两轴缩放不等，如 Tm `1.4 -0.7 0.7 0.7`）：
        // (size, rotation, mirror) 只能表达正交基，会把字形拉高
        // （pdf.js/bug946506）。此时额外记录归一化线性部分，repack 直接用
        let x_scale = tm_dev.x_scale();
        let model = if mirror {
            let (c, s) = (rotation.to_radians().cos(), rotation.to_radians().sin());
            [c, s, s, -c]
        } else {
            let (c, s) = (rotation.to_radians().cos(), rotation.to_radians().sin());
            [c, s, -s, c]
        };
        let tm_norm: Option<[f64; 4]> = if x_scale > 1e-9 {
            let norm = [
                tm_dev.a / x_scale,
                tm_dev.b / x_scale,
                tm_dev.c / x_scale,
                tm_dev.d / x_scale,
            ];
            let dev = norm
                .iter()
                .zip(model.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0f64, f64::max);
            if dev > 1e-3 {
                Some([
                    round_f(norm[0], 6),
                    round_f(norm[1], 6),
                    round_f(norm[2], 6),
                    round_f(norm[3], 6),
                ])
            } else {
                None
            }
        } else {
            None
        };
        // 裁剪外文本直接丢弃：外观流常用 `W n` 把远超 BBox 的长文本裁掉
        // （pdfbox/AcroFormsBasicFields 的域外观含 2 万行、y 到 -32000），
        // 全量保留会把整页画花。判定按基线原点 + 一个字号余量
        if let Some(c) = state.clip {
            let m = size_dev.max(4.0);
            if x < c[0] - m || x > c[2] + m || y < c[1] - m || y > c[3] + m {
                return;
            }
        }
        let y_key = (y * 4.0).round() as i64;
        let x_key = (x * 2.0).round() as i64;
        let color = state.fill.hex();

        let need_new = match &self.text_buf {
            Some(buf) => {
                buf.y_key != y_key
                    || (buf.size - size_dev).abs() > 0.05
                    || buf.font != state.font_name
                    || buf.color != color
                    || (buf.rotation - rotation).abs() > 0.05
                    || buf.mirror != mirror
                    || buf.render_mode != state.render_mode
                    || buf.stroke_color != Some(state.stroke.hex())
                    || buf.alpha != state.fill_alpha
                    || (buf.word_spacing - state.word_spacing).abs() > 1e-6
                    || (buf.char_spacing - state.char_spacing).abs() > 1e-6
                    || (buf.h_scale - state.h_scale).abs() > 1e-6
                    || buf.tm != tm_norm
                    || buf.x_key != x_key
            }
            None => true,
        };
        // 非 Identity CIDToGIDMap 的字体：所有文本按原始码位回写
        let force_codes = scope
            .font(&state.font_key)
            .map(|f| f.raw_codes)
            .unwrap_or(false);
        let width_em = self.text_width_em(&decoded, state);
        if need_new {
            self.flush_text();
            let cap_smask = self.capture_smask(state);
            self.text_buf = Some(TextBuffer {
                y_key,
                x_key,
                size: size_dev,
                font: state.font_name.clone(),
                font_id: state.font_id.clone(),
                color,
                rotation,
                mirror,
                alpha: state.fill_alpha,
                blend: state.blend.clone(),
                word_spacing: state.word_spacing,
                char_spacing: state.char_spacing,
                h_scale: state.h_scale,
                tm: tm_norm,
                smask: cap_smask,

                x,
                y,
                text: decoded.clone(),
                end_x: x + width_em * size_dev,
                codes: bytes.to_vec(),
                force_codes,
                render_mode: state.render_mode,
                stroke_color: Some(state.stroke.hex()),
            });
        } else {
            let buf = self.text_buf.as_mut().unwrap();
            if x - buf.end_x > 0.2 * size_dev && !buf.text.ends_with(' ') {
                buf.text.push(' ');
            }
            buf.text.push_str(&decoded);
            buf.end_x = x + width_em * size_dev;
            buf.codes.extend_from_slice(bytes);
        }
        // 隐式笔步进（含 Tc/Tw/Tz）：走与元素定位同一套宽度
        // （字体 /Widths 或标准 14 AFM），避免行内逐词漂移
        let mut adv = self.text_width_em(&decoded, state) * state.font_size;
        let n_chars = decoded.chars().count() as f64;
        adv += state.char_spacing * n_chars;
        adv += state.word_spacing * decoded.chars().filter(|c| *c == ' ').count() as f64;
        adv *= state.h_scale;
        let (dx, dy) = state.tm_text.apply_linear(adv, 0.0);
        state.tm_text.e += dx;
        state.tm_text.f += dy;
    }

    /// Type3 字体：逐个字形解释 CharProc（字形自带颜色/矢量/文本）。
    fn show_type3(
        &mut self,
        bytes: &[u8],
        state: &mut GState,
        scope: &Scope<'a>,
        font: &'a Dictionary,
        depth: usize,
    ) {
        if depth > 6 {
            return;
        }
        // 码 → 字形名（/Encoding /Differences）
        let mut names: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
        if let Ok(Object::Dictionary(enc)) = font.get_deref(b"Encoding", self.doc) {
            if let Ok(Object::Array(diff)) = enc.get_deref(b"Differences", self.doc) {
                let mut code: i64 = -1;
                for item in diff {
                    match item {
                        Object::Integer(n) => code = *n,
                        Object::Real(r) => code = *r as i64,
                        Object::Name(n) => {
                            if (0..256).contains(&code) {
                                names.insert(code as u8, n.clone());
                            }
                            code += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        let charprocs = self.deref_dict(font, b"CharProcs").ok();
        let font_matrix = font
            .get(b"FontMatrix")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o))
            .filter(|v| v.len() >= 6)
            .map(|v| Mat {
                a: v[0],
                b: v[1],
                c: v[2],
                d: v[3],
                e: v[4],
                f: v[5],
            })
            .unwrap_or(Mat {
                a: 0.001,
                b: 0.0,
                c: 0.0,
                d: 0.001,
                e: 0.0,
                f: 0.0,
            });
        // 字形资源
        let mut inner = scope.clone();
        if let Ok(res) = self.deref_dict(font, b"Resources") {
            self.merge_resources(&mut inner, &res);
        }
        let first_char = self.num_field(font, b"FirstChar").unwrap_or(0);
        let widths: Vec<i64> = font
            .get(b"Widths")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_array().ok())
            .map(|a| a.iter().map(|v| obj_i64(v).unwrap_or(0)).collect())
            .unwrap_or_default();

        for &code in bytes {
            let name = names.get(&code).cloned();
            let proc = match (&charprocs, &name) {
                (Some(cp), Some(n)) => cp.get(n).ok().cloned(),
                _ => None,
            };
            let Some(proc) = proc else {
                continue;
            };
            let (_, obj) = match self.doc.dereference(&proc) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let stream = match obj {
                Object::Stream(s) => s,
                _ => continue,
            };
            let content = stream.get_plain_content().unwrap_or_default();
            // 字形位移（d0/d1 的 wx；缺省用 /Widths）
            let mut wx = widths
                .get(code as usize - first_char as usize)
                .copied()
                .unwrap_or(0) as f64;
            for op in tokenize(&content) {
                if (op.operator == "d0" || op.operator == "d1") && !op.operands.is_empty() {
                    wx = tok_num(op.operands.first());
                    break;
                }
            }
            if wx == 0.0 {
                wx = 1000.0;
            }
            // 字形设备矩阵：CTM × Tm × [Tfs] × FontMatrix
            let size = state.font_size;
            let trm = state
                .ctm
                .transform(state.tm_text)
                .transform(Mat {
                    a: size,
                    b: 0.0,
                    c: 0.0,
                    d: size,
                    e: 0.0,
                    f: 0.0,
                })
                .transform(font_matrix);
            let mut gs = state.clone();
            gs.ctm = trm;
            gs.base = trm;
            gs.tm_line = Mat::I;
            gs.tm_text = Mat::I;
            gs.font_size = 1.0;
            gs.clip = state.clip;
            self.run_content(&content, &inner, &mut gs, depth + 1);
            // 笔步进：wx × FontMatrix × Tfs
            let adv = wx * font_matrix.a * size;
            let (dx, dy) = state.tm_text.apply_linear(adv, 0.0);
            state.tm_text.e += dx;
            state.tm_text.f += dy;
        }
    }

    /// 文本宽度（em）：优先用源字体度量，缺省用 Helvetica 近似表。
    /// 简单字体（Type1/TrueType + /Widths）的 unicode→字宽（1/1000 em）。
    /// 缺失时 unpack 步进退回通用估算表，逐字位置漂移
    /// （pdfcpu/readAndUpdatePage：Arial 子集应 3111/1000 em，估算仅约 2494）
    fn simple_font_unicode_widths(
        font: &Dictionary,
        decoder: &crate::text::FontDecoder<'_>,
        doc: &PdfDocument,
    ) -> BTreeMap<u32, i64> {
        let mut out = BTreeMap::new();
        let first = font.get(b"FirstChar").and_then(Object::as_i64).unwrap_or(0);
        let widths = font
            .get(b"Widths")
            .ok()
            .and_then(|o| doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_array().ok())
            .cloned();
        let Some(widths) = widths else { return out };
        for (i, w) in widths.iter().enumerate() {
            let Some(wv) = obj_f64(w) else { continue };
            if wv <= 0.0 {
                continue;
            }
            let code = first + i as i64;
            if !(0..=255).contains(&code) {
                continue;
            }
            let uni = crate::text::decode_string(decoder, &[code as u8]);
            if let Some(c) = uni.chars().next() {
                out.insert(c as u32, wv.round() as i64);
            }
        }
        out
    }

    fn text_width_em(&self, text: &str, state: &GState) -> f64 {
        if let Some(widths) = &state.font_widths {
            let mut total = 0i64;
            for c in text.chars() {
                total += widths.get(&(c as u32)).copied().unwrap_or(500);
            }
            return total as f64 / 1000.0;
        }
        // 标准 14：优先用 AFM 精确度量，否则行内多段定位会逐词漂移
        text.chars()
            .map(|c| {
                crate::text::std14_width_1000(&state.font_name, c)
                    .map(|w| w as f64 / 1000.0)
                    .unwrap_or_else(|| crate::text::fallback_width_em(c))
            })
            .sum()
    }

    fn append_space(&mut self) {
        if let Some(buf) = self.text_buf.as_mut() {
            if !buf.text.ends_with(' ') {
                buf.text.push(' ');
            }
        }
    }

    fn flush_text(&mut self) {
        if let Some(buf) = self.text_buf.take() {
            if buf.text.trim().is_empty() {
                return;
            }
            self.elements.push(Element::Text {
                text: buf.text.clone(),
                x: round_f(buf.x, 2),
                y: round_f(buf.y, 2),
                size: round_f(buf.size, 2),
                font: buf.font,
                font_id: buf.font_id,
                color: buf.color,
                rotation: buf.rotation,
                mirror: buf.mirror,
                // 复杂文种（阿拉伯/希伯来/印度系）连写由 shaper 决定：
                // 记录原始码位，重建时原样回写
                render_mode: buf.render_mode,
                stroke_color: if buf.render_mode == 1 || buf.render_mode >= 2 {
                    buf.stroke_color.clone()
                } else {
                    None
                },
                stroke_width: 1.0,
                codes: if (needs_raw_codes(&buf.text) || buf.force_codes) && !buf.codes.is_empty() {
                    Some(
                        buf.codes
                            .iter()
                            .map(|b| format!("{b:02X}"))
                            .collect::<String>(),
                    )
                } else {
                    None
                },
                code_bytes: 1,
                word_spacing: round_f(buf.word_spacing, 3),
                char_spacing: round_f(buf.char_spacing, 3),
                h_scale: round_f(buf.h_scale, 4),
                tm: buf.tm,
                alpha: round_f(buf.alpha, 4),
                blend: buf.blend.clone(),
                smask: buf.smask.clone(),
                bold: false,
                italic: false,
            });
        }
    }

    /// 注释外观流（AP/N）。
    fn run_annotations(&mut self, page_id: (u32, u16), scope: &Scope<'a>, depth: usize) {
        let Ok(dict) = self.doc.get_dictionary(page_id) else {
            return;
        };
        let annots = dict.get(b"Annots").ok().cloned();
        let Some(annots) = annots else { return };
        let arr = match self.doc.dereference(&annots) {
            Ok((_, Object::Array(a))) => a.clone(),
            _ => return,
        };
        for a in arr {
            let Ok((_, Object::Dictionary(ad))) = self.doc.dereference(&a) else {
                continue;
            };
            let ad = ad.clone();
            // 文本标记注释（Highlight/Underline/StrikeOut/Squiggly）多数
            // 没有外观流：按 /QuadPoints + /C 直接绘制（fpdf2
            // highlighted_over_page_break 的黄色高亮）；Highlight 用
            // Multiply 混合，保证黑字仍可见
            let subtype = ad
                .get(b"Subtype")
                .ok()
                .and_then(|o| o.as_name_str().ok())
                .unwrap_or("");
            if matches!(
                subtype,
                "Highlight" | "Underline" | "StrikeOut" | "Squiggly"
            ) {
                let color = ad
                    .get(b"C")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .map(|v| Rgb::from_components(&v, Some(&CsSpec::Rgb)))
                    .unwrap_or(Rgb(1.0, 1.0, 0.0));
                let quads = ad
                    .get(b"QuadPoints")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_default();
                for q in quads.chunks(8) {
                    if q.len() < 8 {
                        break;
                    }
                    let xs = [q[0], q[2], q[4], q[6]];
                    let ys = [q[1], q[3], q[5], q[7]];
                    let x0 = xs.iter().copied().fold(f64::INFINITY, f64::min);
                    let x1 = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    let y0 = ys.iter().copied().fold(f64::INFINITY, f64::min);
                    let y1 = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    let (ry, rh) = match subtype {
                        "Underline" | "Squiggly" => (y0, 1.0),
                        "StrikeOut" => ((y0 + y1) / 2.0, 1.0),
                        _ => (y0, y1 - y0),
                    };
                    self.elements.push(Element::Rect {
                        x: round_f(x0, 2),
                        y: round_f(ry, 2),
                        w: round_f(x1 - x0, 2),
                        h: round_f(rh, 2),
                        fill: Some(color.hex()),
                        stroke: None,
                        line_width: 1.0,
                        rotation: 0.0,
                        alpha: 1.0,
                        stroke_alpha: None,
                        smask: None,
                        blend: if subtype == "Highlight" {
                            Some("Multiply".into())
                        } else {
                            None
                        },
                    });
                }
                continue;
            }
            // FreeText 注释多数无外观流：按 /Rect + /C + /Contents 绘制，
            // /DA 缺失时用阅读器默认（Helv 12 黑字、左上 2pt 内缩）
            // （pdfium/freetext_annotation_without_da）
            if subtype == "FreeText" {
                let rect = ad
                    .get(b"Rect")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_default();
                if rect.len() >= 4 {
                    let (x0, x1) = (rect[0].min(rect[2]), rect[0].max(rect[2]));
                    let (y0, y1) = (rect[1].min(rect[3]), rect[1].max(rect[3]));
                    let color = ad
                        .get(b"C")
                        .ok()
                        .and_then(|o| self.doc.dereference(o).ok())
                        .and_then(|(_, o)| obj_f64_array(o))
                        .map(|v| Rgb::from_components(&v, Some(&CsSpec::Rgb)));
                    let ca = ad
                        .get(b"CA")
                        .ok()
                        .and_then(obj_f64)
                        .unwrap_or(1.0)
                        .clamp(0.0, 1.0);
                    self.elements.push(Element::Rect {
                        x: round_f(x0, 2),
                        y: round_f(y0, 2),
                        w: round_f(x1 - x0, 2),
                        h: round_f(y1 - y0, 2),
                        fill: color.map(|c| c.hex()),
                        stroke: Some("#000000".into()),
                        line_width: 1.0,
                        rotation: 0.0,
                        alpha: round_f(ca, 4),
                        stroke_alpha: None,
                        smask: None,
                        blend: None,
                    });
                    let contents = ad
                        .get(b"Contents")
                        .ok()
                        .and_then(|o| match self.doc.dereference(o) {
                            Ok((_, Object::String(bytes, _))) => {
                                Some(decode_pdf_text_string(bytes))
                            }
                            _ => None,
                        })
                        .unwrap_or_default();
                    if !contents.trim().is_empty() {
                        let (font, size, tcolor) = parse_free_text_da(ad.get(b"DA").ok());
                        // 按 /Rect 宽度自动折行（阅读器合成的外观也是折行的；
                        // 不折行会单行溢出整页，pdfbox/Annotations）
                        let wrapped = wrap_free_text(&contents, &font, size, (x1 - x0) - 4.0);
                        self.elements.push(Element::Text {
                            text: wrapped,
                            x: round_f(x0 + 2.0, 2),
                            y: round_f(y1 - size, 2),
                            size,
                            font,
                            font_id: None,
                            color: tcolor,
                            rotation: 0.0,
                            mirror: false,
                            codes: None,
                            code_bytes: 1,
                            render_mode: 0,
                            stroke_color: None,
                            stroke_width: 1.0,
                            word_spacing: 0.0,
                            char_spacing: 0.0,
                            h_scale: 1.0,
                            tm: None,
                            alpha: 1.0,
                            blend: None,
                            smask: None,
                            bold: false,
                            italic: false,
                        });
                    }
                }
                continue;
            }
            // 只取可见外观流：/AP /N
            let Some(ap) = ad
                .get(b"AP")
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| o.as_dict().ok())
                .cloned()
            else {
                continue;
            };
            let Some(n) = ap.get(b"N").ok().cloned() else {
                continue;
            };
            // N 可能是流、或状态字典（复选框/单选按钮）：按 /AS 选当前状态，
            // 取第一个会把「选中」画成 Off（pdfbox/AcroFormsBasicFields 的勾选丢失）
            let n_obj = match self.doc.dereference(&n) {
                Ok((_, Object::Stream(_))) => n.clone(),
                Ok((_, Object::Dictionary(d))) => {
                    let d = d.clone();
                    let as_name = ad
                        .get(b"AS")
                        .ok()
                        .and_then(|o| o.as_name_str().ok())
                        .map(|s| s.as_bytes().to_vec());
                    let mut pick = as_name
                        .as_deref()
                        .and_then(|name| d.get(name).ok())
                        .filter(|o| matches!(self.doc.dereference(o), Ok((_, Object::Stream(_)))))
                        .cloned();
                    if pick.is_none() {
                        for (_, v) in d.iter() {
                            if let Ok((_, Object::Stream(_))) = self.doc.dereference(v) {
                                pick = Some(v.clone());
                                break;
                            }
                        }
                    }
                    match pick {
                        Some(f) => f,
                        None => continue,
                    }
                }
                _ => continue,
            };
            let Ok((_, Object::Stream(s))) = self.doc.dereference(&n_obj) else {
                continue;
            };
            let content = s.get_plain_content().unwrap_or_default();
            if content.is_empty() {
                continue;
            }
            let form_dict = s.dict.clone();
            let rect = ad
                .get(b"Rect")
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| obj_f64_array(o))
                .unwrap_or_default();
            let bbox = form_dict
                .get(b"BBox")
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| obj_f64_array(o))
                .unwrap_or_default();
            let matrix = form_dict
                .get(b"Matrix")
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| obj_f64_array(o))
                .filter(|v| v.len() >= 6)
                .map(|v| Mat {
                    a: v[0],
                    b: v[1],
                    c: v[2],
                    d: v[3],
                    e: v[4],
                    f: v[5],
                })
                .unwrap_or(Mat::I);
            // 外观流 → 页面空间：先按表单 /Matrix 变换 BBox，再映射到 /Rect
            //（规范 12.5.5 的算法；顺序反了会让内容整体偏移，pdf.js/issue7821
            // 的印章 /Matrix 是平移 [-157.551 -370.547]）
            let placement = if rect.len() >= 4 && bbox.len() >= 4 {
                let corners = [
                    (bbox[0], bbox[1]),
                    (bbox[2], bbox[1]),
                    (bbox[2], bbox[3]),
                    (bbox[0], bbox[3]),
                ];
                let mapped: Vec<(f64, f64)> =
                    corners.iter().map(|(x, y)| matrix.apply(*x, *y)).collect();
                let bx0 = mapped.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
                let bx1 = mapped.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
                let by0 = mapped.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
                let by1 = mapped.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
                let (bw, bh) = (bx1 - bx0, by1 - by0);
                let (rw, rh) = (rect[2] - rect[0], rect[3] - rect[1]);
                let (sx, sy) = if bw.abs() > 1e-9 && bh.abs() > 1e-9 {
                    (rw / bw, rh / bh)
                } else {
                    (1.0, 1.0)
                };
                Mat {
                    a: sx,
                    b: 0.0,
                    c: 0.0,
                    d: sy,
                    e: rect[0] - bx0 * sx,
                    f: rect[1] - by0 * sy,
                }
            } else {
                Mat::I
            };
            let mut inner = scope.clone();
            if let Some(res) = form_dict
                .get(b"Resources")
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| o.as_dict().ok())
                .cloned()
            {
                self.merge_resources(&mut inner, &res);
            }
            self.flush_text();
            let mut st = GState::default();
            // 内容先经表单 /Matrix（映射到变换后 BBox 所在空间），再 placement 到 /Rect
            st.ctm = placement.transform(matrix);
            st.base = st.ctm;
            st.clip = None;
            self.run_content(&content, &inner, &mut st, depth + 1);
        }
    }

    /// 抽取内嵌字体资源（FontFile2/FontFile3）→ fonts/ 下的文件 + 元数据。
    ///
    /// 返回 (字体资源 id, unicode→1000 字宽)。
    /// Type3 位图字体直通提取：CharProcs 码流 + 图片 XObject + 编码/宽度。
    /// 仅当 CharProcs 全部可读、且字体 /Resources 只含图片 XObject 时启用；
    /// 否则返回 None，回退到既有的字形扁平化路径。
    fn extract_type3_font(&mut self, font: &'a Dictionary, display: &str) -> Option<String> {
        let charprocs = font
            .get(b"CharProcs")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok())
            .cloned()?;
        let entries: Vec<(String, Object)> = charprocs
            .iter()
            .map(|(k, v)| (String::from_utf8_lossy(k).to_string(), v.clone()))
            .collect();
        if entries.is_empty() {
            return None;
        }
        // 资源：只接受图片 XObject（其余资源类型回退扁平化）
        let mut xobjects: BTreeMap<String, String> = BTreeMap::new();
        let mut xobject_smasks: BTreeMap<String, String> = BTreeMap::new();
        if let Some(res) = font
            .get(b"Resources")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok())
            .cloned()
        {
            for (k, v) in res.iter() {
                if k.as_slice() != b"XObject" {
                    return None;
                }
                let xo = self.doc.dereference(v).ok()?.1.as_dict().ok()?.clone();
                for (name, obj) in xo.iter() {
                    let (id, d) = match self.doc.dereference(obj) {
                        Ok((Some(id), Object::Stream(s))) => (id, s.dict.clone()),
                        _ => return None,
                    };
                    let is_image = d
                        .get(b"Subtype")
                        .and_then(Object::as_name_str)
                        .map(|s| s == "Image")
                        .unwrap_or(false);
                    if !is_image {
                        return None;
                    }
                    let st = GState::default();
                    let (src, smask, ..) = self.save_image_full(id, &d, &st)?;
                    if let Some(sm) = smask {
                        xobject_smasks.insert(src.clone(), sm);
                    }
                    xobjects.insert(String::from_utf8_lossy(name).to_string(), src);
                }
            }
        }
        // CharProcs 码流落盘
        self.shared.font_idx += 1;
        let fid = format!("font-{:03}", self.shared.font_idx);
        let dir = self.out_dir.join("fonts").join(&fid);
        std::fs::create_dir_all(&dir).ok()?;
        let mut proc_files: Vec<(String, String)> = Vec::new();
        for (name, obj) in &entries {
            let content = match self.doc.dereference(obj) {
                Ok((_, Object::Stream(s))) => s.get_plain_content().unwrap_or_default(),
                _ => Vec::new(),
            };
            if content.is_empty() {
                return None;
            }
            let safe: String = name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let rel = format!("fonts/{fid}/{safe}.bin");
            std::fs::write(self.out_dir.join(&rel), &content).ok()?;
            proc_files.push((name.clone(), rel));
        }
        // 编码（/Differences）→ unicode→码
        let enc_table = font
            .get(b"Encoding")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_dict().ok())
            .and_then(crate::text::differences_table);
        let mut map: BTreeMap<u32, u16> = BTreeMap::new();
        let mut differences: Vec<(u16, String)> = Vec::new();
        if let Some(table) = &enc_table {
            // 记录 Differences 原样（码→字形名）供 repack 回写
            if let Ok((_, Object::Dictionary(encd))) = font
                .get(b"Encoding")
                .map(|o| self.doc.dereference(o))
                .unwrap_or(Err(lopdf::Error::Type))
            {
                if let Ok(diffs) = encd.get(b"Differences").and_then(Object::as_array) {
                    let mut code: u32 = 0;
                    for item in diffs {
                        match item {
                            Object::Integer(n) => code = *n as u32,
                            Object::Real(r) => code = *r as u32,
                            Object::Name(n) => {
                                differences
                                    .push((code as u16, String::from_utf8_lossy(n).to_string()));
                                code += 1;
                            }
                            _ => {}
                        }
                    }
                }
            }
            for (code, ch) in table.iter().enumerate() {
                if let Some(c) = ch {
                    map.entry(*c as u32).or_insert(code as u16);
                }
            }
        }
        let num = |k: &[u8], d: i64| {
            font.get(k)
                .ok()
                .and_then(|o| o.as_i64().ok().or_else(|| obj_f64(o).map(|f| f as i64)))
                .unwrap_or(d)
        };
        let matrix = font
            .get(b"FontMatrix")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o))
            .filter(|v| v.len() >= 6)
            .unwrap_or_else(|| vec![0.001, 0.0, 0.0, 0.001, 0.0, 0.0]);
        let bbox = font
            .get(b"FontBBox")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| obj_f64_array(o))
            .filter(|v| v.len() >= 4)
            .map(|v| [v[0] as i64, v[1] as i64, v[2] as i64, v[3] as i64])
            .unwrap_or([0, 0, 1000, 1000]);
        let first_char = num(b"FirstChar", 0);
        let widths: Vec<i64> = font
            .get(b"Widths")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_array().ok())
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|o| obj_f64(o).unwrap_or(0.0).round() as i64)
            .collect();
        let resource = FontResource {
            id: fid.clone(),
            name: display.to_string(),
            file: String::new(),
            format: "type3".into(),
            units_per_em: 1000,
            map: map.clone(),
            widths: widths
                .iter()
                .enumerate()
                .map(|(i, w)| ((first_char + i as i64) as u16, *w))
                .collect(),
            encoding: None,
            metrics: None,
            cid_to_gid: None,
            w_array: None,
            dw: None,
            encoding_cmap: None,
            encoding_differences: None,
            length1: None,
            code_widths: None,
            code_map: None,
            simple_rebuild: false,
            type3: Some(crate::model::Type3Data {
                charprocs: proc_files,
                xobjects,
                xobject_smasks,
                matrix: [
                    matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5],
                ],
                bbox,
                first_char,
                last_char: num(b"LastChar", first_char + widths.len() as i64 - 1),
                widths,
                differences,
            }),
        };
        let rel_json = format!("fonts/{fid}.json");
        let _ = serde_json::to_string_pretty(&resource)
            .ok()
            .map(|s| std::fs::write(self.out_dir.join(&rel_json), s));
        self.shared.font_files.push(rel_json);
        Some(fid)
    }

    #[allow(clippy::type_complexity)]
    fn extract_font_resource(
        &mut self,
        font: &'a Dictionary,
        display: &str,
    ) -> (Option<String>, Option<Rc<BTreeMap<u32, i64>>>, bool) {
        let is_cid = font
            .get(b"Subtype")
            .and_then(Object::as_name_str)
            .unwrap_or("")
            == "Type0";
        let desc_obj: Option<&'a Object> = if is_cid {
            self.descendant_ref(font)
                .and_then(|d| d.get(b"FontDescriptor").ok())
        } else {
            font.get(b"FontDescriptor").ok()
        };
        // CID 字体可以完全没有 FontDescriptor（非内嵌、无描述符，
        // pdf.js/XiaoBiaoSong）：仍应保留 CID 结构，交给阅读器替换
        let empty_desc = Dictionary::new();
        let (desc_id, desc) = match desc_obj {
            Some(o) => match self.doc.dereference(o) {
                Ok((id, Object::Dictionary(d))) => (id, d),
                _ if is_cid => (None, &empty_desc),
                _ => return (None, None, false),
            },
            None if is_cid => (None, &empty_desc),
            None => return (None, None, false),
        };
        // 去重（必须早于字体程序读取/解压：同一字体在每个 Form 的资源里都会出现，
        // 每次解压几百 KB 会让 Illustrator 类文件卡到几分钟；pdf.js/issue6961）
        if let Some(id) = desc_id {
            if let Some((fid, w, raw)) = self.shared.font_cache.get(&id) {
                return (Some(fid.clone()), Some(w.clone()), *raw);
            }
        }
        // 字体文件
        let mut file: Option<(Vec<u8>, crate::fonts::FontFormat)> = None;
        if let Ok(ff) = desc.get(b"FontFile2") {
            if let Ok((_, Object::Stream(s))) = self.doc.dereference(ff) {
                if let Ok(bytes) = s.get_plain_content() {
                    if !bytes.is_empty() {
                        file = Some((bytes, crate::fonts::FontFormat::TrueType));
                    }
                }
            }
        }
        // Type1 /FontFile（PFA/PFB）：原样直通（字形来自原件，码位按原 /Encoding）
        let mut type1_length1: Option<i64> = None;
        if file.is_none() && !is_cid {
            if let Ok(ff) = desc.get(b"FontFile") {
                if let Ok((_, Object::Stream(s))) = self.doc.dereference(ff) {
                    if let Ok(bytes) = s.get_plain_content() {
                        if !bytes.is_empty() {
                            type1_length1 = s.dict.get(b"Length1").and_then(Object::as_i64).ok();
                            file = Some((bytes, crate::fonts::FontFormat::Type1));
                        }
                    }
                }
            }
        }
        if file.is_none() {
            if let Ok(ff) = desc.get(b"FontFile3") {
                if let Ok((_, Object::Stream(s))) = self.doc.dereference(ff) {
                    let sub = s
                        .dict
                        .get(b"Subtype")
                        .and_then(Object::as_name_str)
                        .unwrap_or("");
                    let format = match sub {
                        "OpenType" => Some(crate::fonts::FontFormat::OpenType),
                        // 裸 CFF：简单 Type1 字体（/Subtype /Type1 + /Widths + /Encoding）
                        "Type1C" if !is_cid => Some(crate::fonts::FontFormat::Type1C),
                        // CID-keyed CFF（CIDFontType0C）：字形由字体内置 charset 决定
                        "CIDFontType0C" if is_cid => Some(crate::fonts::FontFormat::CidCff),
                        _ => None,
                    };
                    if let Some(format) = format {
                        if let Ok(bytes) = s.get_plain_content() {
                            if !bytes.is_empty() {
                                file = Some((bytes, format));
                            }
                        }
                    }
                }
            }
        }
        // 非内嵌 CID 字体（无字体程序但有 /W）：保留 CID 结构（宽度 + CMap +
        // CID→GID），由阅读器按其替换规则渲染——退化成"系统字体 + 原码位"
        // 会让 2 字节 CID 被当单字节码解码（pdf.js/issue15977_reduced）
        let (bytes, format) = match file {
            Some(v) => v,
            None if is_cid => (Vec::new(), crate::fonts::FontFormat::CidNoFile),
            None => return (None, None, false),
        };

        // 非 Identity 的 CIDToGIDMap：原码回写时要原样带上
        let mut cid_to_gid: Option<String> = None;
        let mut w_array: Option<serde_json::Value> = None;
        let mut dw: Option<i64> = None;
        // 自定义编码 CMap（/Encoding 为流）：变长码空间需原样保留
        let mut encoding_cmap: Option<String> = None;
        if is_cid {
            if let Ok((_, Object::Stream(enc))) = font
                .get(b"Encoding")
                .map(|o| self.doc.dereference(o))
                .unwrap_or(Err(lopdf::Error::Type))
            {
                if let Ok(bytes) = enc.get_plain_content() {
                    if !bytes.is_empty() {
                        self.shared.font_idx += 1;
                        let rel = format!("fonts/font-{:03}.cmap", self.shared.font_idx);
                        if std::fs::write(self.out_dir.join(&rel), &bytes).is_ok() {
                            encoding_cmap = Some(rel);
                        }
                        self.shared.font_idx -= 1;
                    }
                }
            }
        }
        if is_cid {
            if let Some(desc) = self.descendant_ref(font) {
                if let Ok((_, Object::Stream(s))) = desc
                    .get(b"CIDToGIDMap")
                    .map(|o| self.doc.dereference(o))
                    .unwrap_or(Err(lopdf::Error::Type))
                {
                    if let Ok(bytes) = s.get_plain_content() {
                        if !bytes.is_empty() {
                            self.shared.font_idx += 1;
                            let rel = format!("fonts/font-{:03}.c2g", self.shared.font_idx);
                            if std::fs::write(self.out_dir.join(&rel), &bytes).is_ok() {
                                cid_to_gid = Some(rel);
                            }
                            self.shared.font_idx -= 1;
                        }
                    }
                }
            }
        }
        // W 数组按 CID 索引，与原始码位配套，一律原样保留：只按 gid 重建
        // 会丢掉 ToUnicode 未映射的 CID（宽度落到 DW，字距整体变宽；
        // pdf.js/issue13193）
        if is_cid {
            if let Some(desc) = self.descendant_ref(font) {
                if let Ok(w) = desc.get(b"W") {
                    w_array = Some(object_to_json(w));
                    dw = desc.get(b"DW").ok().and_then(|o| o.as_i64().ok());
                    if dw.is_none() {
                        dw = Some(1000);
                    }
                } else if let Ok(dwv) = desc.get(b"DW").and_then(Object::as_i64) {
                    dw = Some(dwv);
                }
            }
        }

        // 映射与字宽
        let mut encoding: Option<String> = None;
        let mut encoding_differences: Option<serde_json::Value> = None;
        let mut code_widths: std::collections::BTreeMap<u16, i64> =
            std::collections::BTreeMap::new();
        // 简单字体程序无可解析 cmap：必须原码回写（见下）
        let mut force_raw_codes = false;
        let mut metrics: Option<crate::model::FontMetrics> = None;
        // /Encoding（含 /Differences）对所有简单字体都要原样记录：repack 按
        // 「简单字体 + 原 /Encoding + 原码位」重建，缺 Differences 会整体错位
        if !is_cid {
            if let Ok(enc) = font.get(b"Encoding").map(|o| self.doc.dereference(o)) {
                match enc {
                    Ok((_, Object::Name(n))) => {
                        encoding = Some(String::from_utf8_lossy(n).to_string());
                    }
                    Ok((_, Object::Dictionary(ed))) => {
                        encoding = ed
                            .get(b"BaseEncoding")
                            .ok()
                            .and_then(|o| o.as_name_str().ok())
                            .map(str::to_string);
                        if let Ok((_, d)) =
                            ed.get(b"Differences").and_then(|o| self.doc.dereference(o))
                        {
                            encoding_differences = Some(object_to_json(d));
                        }
                    }
                    _ => {}
                }
            }
        }
        let (map, gid_widths, uni_widths) = if matches!(
            format,
            crate::fonts::FontFormat::Type1C | crate::fonts::FontFormat::Type1
        ) {
            // 简单 Type1（裸 CFF）：/Widths + 解码器给出 unicode↔码；
            // 度量取自原 FontDescriptor（裸 CFF 无法用 ttf-parser 解析）
            let num = |k: &[u8], d: i64| {
                desc.get(k)
                    .ok()
                    .and_then(|o| o.as_i64().ok().or_else(|| obj_f64(o).map(|f| f as i64)))
                    .unwrap_or(d)
            };
            let bbox = desc
                .get(b"FontBBox")
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| obj_f64_array(o))
                .filter(|v| v.len() >= 4)
                .map(|v| [v[0] as i64, v[1] as i64, v[2] as i64, v[3] as i64])
                .unwrap_or([-200, -300, 1200, 1000]);
            metrics = Some(crate::model::FontMetrics {
                ascent: num(b"Ascent", 800),
                descent: num(b"Descent", -200),
                cap_height: num(b"CapHeight", 700),
                italic_angle: num(b"ItalicAngle", 0) as f64,
                flags: num(b"Flags", 32),
                bbox,
            });
            let decoder = resolve_font(font, self.doc);
            let first = font.get(b"FirstChar").and_then(Object::as_i64).unwrap_or(0);
            let mut map: BTreeMap<u32, u16> = BTreeMap::new();
            let mut gid_widths: BTreeMap<u16, i64> = BTreeMap::new();
            let mut uni_widths: BTreeMap<u32, i64> = BTreeMap::new();
            if let Ok((_, Object::Array(arr))) = font
                .get(b"Widths")
                .map(|o| self.doc.dereference(o))
                .unwrap_or(Err(lopdf::Error::Type))
            {
                for (i, w) in arr.iter().enumerate() {
                    let Some(wv) = obj_f64(w) else { continue };
                    let code = first + i as i64;
                    if !(0..=255).contains(&code) {
                        continue;
                    }
                    gid_widths.insert(code as u16, wv.round() as i64);
                    let uni = decode_string(&decoder, &[code as u8]);
                    if let Some(c) = uni.chars().next() {
                        // 同一 unicode 可能对应多个码（Differences 码 + 基本编码码）：
                        // 优先取有非零宽度的那个（子集字体里未映射码宽度为 0）
                        let wr = wv.round() as i64;
                        match map.entry(c as u32) {
                            std::collections::btree_map::Entry::Vacant(v) => {
                                v.insert(code as u16);
                                uni_widths.insert(c as u32, wr);
                            }
                            std::collections::btree_map::Entry::Occupied(mut o) => {
                                if uni_widths.get(&(c as u32)).copied().unwrap_or(0) == 0 && wr != 0
                                {
                                    o.insert(code as u16);
                                    uni_widths.insert(c as u32, wr);
                                }
                            }
                        }
                    }
                }
            }
            (map, gid_widths, uni_widths)
        } else {
            let mut map = self.build_unicode_gid_map(font, is_cid);
            let mut gid_widths = self.build_widths_1000(font, is_cid, &map);
            // 简单字体：/Widths 按码位索引，而 gid 由字体程序决定（子集字体
            // 的字形顺序常被打乱）。这里用解码器给出的 code→unicode 建
            // unicode→code，并保留码位宽度，重建时按它回写 /W
            if map.is_empty()
                && matches!(
                    format,
                    crate::fonts::FontFormat::TrueType | crate::fonts::FontFormat::OpenType
                )
            {
                let decoder = resolve_font(font, self.doc);
                let first = font.get(b"FirstChar").and_then(Object::as_i64).unwrap_or(0);
                if let Ok(w) = font.get(b"Widths") {
                    if let Ok((_, Object::Array(arr))) = self.doc.dereference(w) {
                        let arr = arr.clone();
                        for (k, v) in arr.iter().enumerate() {
                            let code = first + k as i64;
                            if !(0..=255).contains(&code) {
                                continue;
                            }
                            let uni = decode_string(&decoder, &[code as u8]);
                            let Some(c) = uni.chars().next() else {
                                continue;
                            };
                            if c == '\u{FFFD}' {
                                continue;
                            }
                            let wv = obj_i64(v).unwrap_or(500);
                            map.insert(c as u32, code as u16);
                            code_widths.insert(code as u16, wv);
                        }
                    }
                }
                // 按字体程序自身 cmap 折算 gid 宽度。子集字体常常没有可用的
                // cmap（或只有 symbol 子表）——此时按 CID 重建会整页 .notdef，
                // 改为原码回写 + 简单字体（字体程序 + 原 /Encoding 决定字形）
                // 判定程序 cmap 是否可用：能映射到一半以上的码位才算可用
                // （子集字体常有 cmap 表但查不到字符，按 CID 重建会整页 .notdef）
                let mut hits = 0usize;
                let total = map.len().max(1);
                if let Ok(face) = ttf_parser::Face::parse(&bytes, 0) {
                    for (uni, code) in map.clone() {
                        if let Some(gid) = face.glyph_index(char::from_u32(uni).unwrap_or(' ')) {
                            hits += 1;
                            if let Some(w) = code_widths.get(&code) {
                                gid_widths.insert(gid.0, *w);
                            }
                        }
                    }
                }
                if hits * 2 < total {
                    force_raw_codes = true;
                }
            }
            // 度量用：unicode → 1000 字宽
            let mut uni_widths: BTreeMap<u32, i64> = BTreeMap::new();
            for (uni, gid) in &map {
                if let Some(w) = gid_widths.get(gid) {
                    uni_widths.insert(*uni, *w);
                }
            }
            (map, gid_widths, uni_widths)
        };

        self.shared.font_idx += 1;
        let fid = format!("font-{:03}", self.shared.font_idx);
        let ext = match format {
            crate::fonts::FontFormat::TrueType => "ttf",
            crate::fonts::FontFormat::OpenType => "otf",
            crate::fonts::FontFormat::Type1C => "cff",
            crate::fonts::FontFormat::CidCff => "cff",
            crate::fonts::FontFormat::Type1 => "pfa",
            crate::fonts::FontFormat::CidNoFile => "nofile",
        };
        let rel_file = format!("fonts/{fid}.{ext}");
        let rel_json = format!("fonts/{fid}.json");
        if !bytes.is_empty() && std::fs::write(self.out_dir.join(&rel_file), &bytes).is_err() {
            return (None, None, false);
        }
        let units_per_em = ttf_parser::Face::parse(&bytes, 0)
            .map(|f| f.units_per_em())
            .unwrap_or(1000);
        let resource = FontResource {
            id: fid.clone(),
            name: display.to_string(),
            file: if bytes.is_empty() {
                String::new()
            } else {
                rel_file
            },
            format: format.as_str().to_string(),
            units_per_em: if matches!(
                format,
                crate::fonts::FontFormat::Type1C
                    | crate::fonts::FontFormat::CidCff
                    | crate::fonts::FontFormat::Type1
            ) {
                1000
            } else {
                units_per_em
            },
            map: map.clone(),
            widths: gid_widths.clone(),
            encoding,
            metrics,
            type3: None,
            cid_to_gid: cid_to_gid.clone(),
            w_array,
            dw,
            encoding_cmap: encoding_cmap.clone(),
            encoding_differences,
            length1: type1_length1,
            code_map: if code_widths.is_empty() {
                None
            } else {
                Some(map.clone())
            },
            simple_rebuild: force_raw_codes && !is_cid,
            code_widths: if code_widths.is_empty() {
                None
            } else {
                Some(code_widths.clone())
            },
        };
        match serde_json::to_string_pretty(&resource)
            .ok()
            .map(|s| std::fs::write(self.out_dir.join(&rel_json), s))
        {
            Some(Ok(_)) => {
                self.shared.font_files.push(rel_json);
                let w_rc = Rc::new(uni_widths);
                // Type1/Type1C 简单字体：/Widths 按原码索引、/Encoding 原样回写，
                // 只有原码回写才能让码位与宽度对齐（子集字体的 Differences 码常与
                // WinAnsi 码不同；pdf.js/issue7101 按 WinAnsi 重编码导致宽度取到 0）
                // 简单字体（有 code_map）一律原码回写：repack 会按原 /Encoding
                // 重建简单字体，码位必须与源一致（自定义 Differences 下按 WinAnsi
                // 重编码会取错字形，pdf.js/issue6894）
                let raw_codes = cid_to_gid.is_some()
                    || encoding_cmap.is_some()
                    || force_raw_codes
                    || matches!(
                        format,
                        crate::fonts::FontFormat::Type1 | crate::fonts::FontFormat::Type1C
                    );
                if let Some(id) = desc_id {
                    self.shared
                        .font_cache
                        .insert(id, (fid.clone(), w_rc.clone(), raw_codes));
                }
                (Some(fid), Some(w_rc), raw_codes)
            }
            _ => (None, None, false),
        }
    }

    /// 取 Type0 字体的后代 CIDFont 字典（借用文档）。
    fn descendant_ref(&self, font: &'a Dictionary) -> Option<&'a Dictionary> {
        let arr = font.get(b"DescendantFonts").ok()?;
        let (_, arr) = self.doc.dereference(arr).ok()?;
        let first = arr.as_array().ok()?.first()?;
        match first.as_reference() {
            Ok(id) => self.doc.get_dictionary(id).ok(),
            Err(_) => self
                .doc
                .dereference(first)
                .ok()
                .and_then(|(_, o)| o.as_dict().ok()),
        }
    }

    /// unicode → glyph id（CID 字体经 ToUnicode + CIDToGIDMap；简单字体用字体自身 cmap）。
    fn build_unicode_gid_map(&self, font: &'a Dictionary, is_cid: bool) -> BTreeMap<u32, u16> {
        let (_, code_to_cid) = self.font_encoding_cid(font, is_cid);
        let mut map = BTreeMap::new();
        if is_cid {
            // code → unicode
            let mut code_to_unicode: BTreeMap<u32, u32> = BTreeMap::new();
            if let Ok(Object::Stream(s)) = font.get_deref(b"ToUnicode", self.doc) {
                if let Ok(content) = s.get_plain_content() {
                    if let Some((_cb, m)) = crate::text::parse_tounicode_cmap(&content) {
                        for (code, s) in m {
                            if let Some(c) = s.chars().next() {
                                code_to_unicode.insert(code, c as u32);
                            }
                        }
                    }
                }
            }
            if code_to_unicode.is_empty() {
                return map;
            }
            // cid → gid
            let Some(desc) = self.descendant_ref(font) else {
                return map;
            };
            let cid2gid: Option<Vec<u8>> =
                desc.get(b"CIDToGIDMap")
                    .ok()
                    .and_then(|o| match self.doc.dereference(o) {
                        Ok((_, Object::Stream(s))) => s.get_plain_content().ok(),
                        _ => None,
                    });
            for (code, uni) in code_to_unicode {
                // 自定义编码 CMap：code → CID；否则 Identity（CID = code）
                let cid = code_to_cid.get(&code).copied().unwrap_or(code as u16);
                let gid = match &cid2gid {
                    Some(bytes) => {
                        let pos = cid as usize * 2;
                        if pos + 1 < bytes.len() {
                            ((bytes[pos] as u16) << 8) | bytes[pos + 1] as u16
                        } else {
                            cid
                        }
                    }
                    None => cid,
                };
                // ToUnicode 显式声明「无对应字符」（U+0000）：换成私用区占位，
                // 让字宽表仍有条目（否则行长按 0.5em 估算，字距拉宽；
                // pdf.js/issue13193 的数学符号子集）。文本按原始码位回写
                let key = if uni == 0 { 0xE000 + cid as u32 } else { uni };
                map.insert(key, gid);
            }
        }
        map
    }

    /// DeviceN/Separation 的 tint transform：arr = [name, names?, tint, alt]。
    /// 返回带函数的 Tinted 规格（无法解析时 None，调用方退化为近似）。
    fn tint_spec(&self, arr: &[Object], n: usize) -> Option<CsSpec> {
        // 规范顺序：[/Separation name alt tint] / [/DeviceN names alt tint]
        let tint = arr.get(3)?.clone();
        let alt_obj = arr.get(2)?.clone();
        let (_, tdict) = self.doc.dereference(&tint).ok()?;
        let (content, dict) = match tdict {
            Object::Stream(s) => (s.get_plain_content().unwrap_or_default(), s.dict.clone()),
            Object::Dictionary(d) => (Vec::new(), d.clone()),
            _ => return None,
        };
        let ft = dict
            .get(b"FunctionType")
            .and_then(Object::as_i64)
            .unwrap_or(-1);
        if !matches!(ft, 0 | 2 | 3 | 4) {
            return None;
        }
        let alt = self.resolve_cs_spec(&alt_obj, 0)?;
        Some(CsSpec::Tinted {
            n,
            func: Box::new(tint),
            content,
            alt: Box::new(alt),
        })
    }

    /// 带 tint transform 的分量 → RGB（DeviceN/Separation）。
    fn rgb_from_components(&self, comps: &[f64], spec: Option<&CsSpec>) -> Rgb {
        if let Some(CsSpec::Tinted {
            n,
            func,
            content,
            alt,
        }) = spec
        {
            if comps.len() >= *n {
                if let Some(out) = self.eval_tint(func, content, &comps[..*n], 0) {
                    return self.rgb_from_components(&out, Some(alt.as_ref()));
                }
            }
        }
        Rgb::from_components(comps, spec)
    }

    /// tint transform 求值：Type 4 直接执行；0/2/3 单输入按分量逐一求值。
    fn eval_tint(
        &self,
        func: &Object,
        content: &[u8],
        inputs: &[f64],
        depth: usize,
    ) -> Option<Vec<f64>> {
        if depth > 4 {
            return None;
        }
        let (_, fobj) = self.doc.dereference(func).ok()?;
        let dict = match fobj {
            Object::Stream(s) => s.dict.clone(),
            Object::Dictionary(d) => d.clone(),
            _ => return None,
        };
        let ft = dict
            .get(b"FunctionType")
            .and_then(Object::as_i64)
            .unwrap_or(-1);
        match ft {
            4 => {
                let out = images::eval_ps_calculator(content, inputs)?;
                // 输出 = 栈顶 n_out 个值（Range 分量数）；其余是中间量
                let n_out = dict
                    .get(b"Range")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .map(|v| (v.len() / 2).max(1))
                    .unwrap_or(3);
                if out.len() < n_out {
                    return None;
                }
                Some(out[out.len() - n_out..].to_vec())
            }
            // 单输入函数（Separation/DeviceN 常见）：返回**原始输出分量**，
            // 由调用方按备选空间（如 DeviceCMYK）继续转换。此前这里先转成 RGB
            // 再交给备选空间，等于转换两次（pdfplumber/issue-316 黑字变白/品红）
            0 => {
                let t = *inputs.first().unwrap_or(&0.0);
                self.eval_sampled_components(&dict, content, t)
            }
            2 => {
                let c0 = dict
                    .get(b"C0")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_else(|| vec![0.0]);
                let c1 = dict
                    .get(b"C1")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_else(|| vec![1.0]);
                let n = dict.get(b"N").and_then(Object::as_i64).unwrap_or(1).max(1) as i32;
                let t = *inputs.first().unwrap_or(&0.0);
                let out: Vec<f64> = c0
                    .iter()
                    .zip(c1.iter())
                    .map(|(a, b)| a + (b - a) * t.powf(n as f64))
                    .collect();
                if out.is_empty() {
                    None
                } else {
                    Some(out)
                }
            }
            3 => {
                let subs = dict
                    .get(b"Functions")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| o.as_array().ok())
                    .cloned()
                    .unwrap_or_default();
                if subs.is_empty() {
                    return None;
                }
                // 单输入：按 Bounds 选子函数
                let t = *inputs.first().unwrap_or(&0.0);
                let bounds = dict
                    .get(b"Bounds")
                    .ok()
                    .and_then(|o| self.doc.dereference(o).ok())
                    .and_then(|(_, o)| obj_f64_array(o))
                    .unwrap_or_default();
                let idx = bounds
                    .iter()
                    .filter(|b| t > **b)
                    .count()
                    .min(subs.len() - 1);
                let sub = subs[idx].clone();
                let (_, sd) = self.doc.dereference(&sub).ok()?;
                let (sc, sdict) = match sd {
                    Object::Stream(st) => {
                        (st.get_plain_content().unwrap_or_default(), st.dict.clone())
                    }
                    Object::Dictionary(d) => (Vec::new(), d.clone()),
                    _ => return None,
                };
                // 子函数域 [0,1]：把 t 映射到该子区间
                let lo = if idx == 0 { 0.0 } else { bounds[idx - 1] };
                let hi = *bounds.get(idx).unwrap_or(&1.0);
                let local = if (hi - lo).abs() < 1e-9 {
                    0.0
                } else {
                    (t - lo) / (hi - lo)
                };
                self.eval_tint(&sub, &sc, &[local], depth + 1)
                    .or_else(|| self.eval_sampled_components(&sdict, &sc, local))
            }
            _ => None,
        }
    }

    /// FunctionType 0 采样函数 → 原始输出分量（不经过颜色空间转换）。
    fn eval_sampled_components(
        &self,
        dict: &Dictionary,
        content: &[u8],
        t: f64,
    ) -> Option<Vec<f64>> {
        let size: Vec<usize> = dict
            .get(b"Size")
            .ok()
            .and_then(|o| self.doc.dereference(o).ok())
            .and_then(|(_, o)| o.as_array().ok())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_i64().ok())
                    .map(|v| v.max(1) as usize)
                    .collect()
            })
            .unwrap_or_default();
        if size.is_empty() {
            return None;
        }
        let bps = dict
            .get(b"BitsPerSample")
            .and_then(Object::as_i64)
            .unwrap_or(8)
            .max(1) as usize;
        let arr = |k: &[u8]| -> Vec<f64> {
            dict.get(k)
                .ok()
                .and_then(|o| self.doc.dereference(o).ok())
                .and_then(|(_, o)| obj_f64_array(o))
                .unwrap_or_default()
        };
        let encode = arr(b"Encode");
        let decode = arr(b"Decode");
        let range = arr(b"Range");
        let m = size[0].max(1);
        let lo_enc = encode.first().copied().unwrap_or(0.0);
        let hi_enc = encode.get(1).copied().unwrap_or((m - 1) as f64);
        let enc = (lo_enc + (hi_enc - lo_enc) * t)
            .round()
            .clamp(0.0, (m - 1) as f64) as usize;
        let n_comp = if decode.len() >= 2 {
            decode.len() / 2
        } else if range.len() >= 2 {
            range.len() / 2
        } else {
            3
        };
        let mut comps = Vec::with_capacity(n_comp);
        for c in 0..n_comp {
            let idx = enc * n_comp + c;
            let raw = match bps {
                8 => content.get(idx).copied().unwrap_or(0) as f64 / 255.0,
                16 => {
                    let hi = content.get(idx * 2).copied().unwrap_or(0) as f64;
                    let lo = content.get(idx * 2 + 1).copied().unwrap_or(0) as f64;
                    (hi * 256.0 + lo) / 65535.0
                }
                _ => {
                    let bit = idx * bps;
                    let byte = content.get(bit / 8).copied().unwrap_or(0);
                    let shift = 8 - bps - (bit % 8);
                    let v = (byte >> shift) & ((1u16 << bps) - 1) as u8;
                    v as f64 / ((1u16 << bps) - 1) as f64
                }
            };
            let dlo = decode.get(c * 2).copied().unwrap_or(0.0);
            let dhi = decode.get(c * 2 + 1).copied().unwrap_or(1.0);
            comps.push(dlo + (dhi - dlo) * raw);
        }
        Some(comps)
    }

    /// Type0 字体的 /Encoding 若为 CMap 流 → (码空间, code→CID)。
    #[allow(clippy::type_complexity)]
    fn font_encoding_cid(
        &self,
        font: &'a Dictionary,
        is_cid: bool,
    ) -> (Vec<(u32, u32, usize)>, std::collections::HashMap<u32, u16>) {
        if !is_cid {
            return (Vec::new(), std::collections::HashMap::new());
        }
        if let Ok((_, Object::Stream(enc))) = font
            .get(b"Encoding")
            .map(|o| self.doc.dereference(o))
            .unwrap_or(Err(lopdf::Error::Type))
        {
            if let Ok(bytes) = enc.get_plain_content() {
                return crate::text::parse_cmap_cid(&bytes);
            }
        }
        (Vec::new(), std::collections::HashMap::new())
    }

    /// glyph id → 字宽（1/1000 em）：CID 用 W 数组，简单字体用 Widths。
    fn build_widths_1000(
        &self,
        font: &'a Dictionary,
        is_cid: bool,
        map: &BTreeMap<u32, u16>,
    ) -> BTreeMap<u16, i64> {
        let mut widths = BTreeMap::new();
        if is_cid {
            let Some(desc) = self.descendant_ref(font) else {
                return widths;
            };
            let dw = desc.get(b"DW").and_then(Object::as_i64).unwrap_or(1000);
            // W 数组：[c [w…] cfirst clast w …]
            if let Ok(w) = desc.get(b"W") {
                if let Ok((_, Object::Array(arr))) = self.doc.dereference(w) {
                    let arr = arr.clone();
                    let mut i = 0usize;
                    let mut cid_widths: BTreeMap<u16, i64> = BTreeMap::new();
                    while i < arr.len() {
                        let Some(start) = arr[i].as_i64().ok() else {
                            i += 1;
                            continue;
                        };
                        match arr.get(i + 1) {
                            Some(Object::Array(list)) => {
                                for (k, v) in list.iter().enumerate() {
                                    let cid = start as usize + k;
                                    if cid <= 0xFFFF {
                                        cid_widths.insert(cid as u16, obj_i64(v).unwrap_or(dw));
                                    }
                                }
                                i += 2;
                            }
                            Some(_) => {
                                let end = arr.get(i + 1).and_then(obj_i64).unwrap_or(start);
                                let wv = arr.get(i + 2).and_then(obj_i64).unwrap_or(dw);
                                for cid in start..=end {
                                    if cid <= 0xFFFF {
                                        cid_widths.insert(cid as u16, wv);
                                    }
                                }
                                i += 3;
                            }
                            None => break,
                        }
                    }
                    // cid → gid（Identity 时相同）
                    let cid2gid: Option<Vec<u8>> =
                        desc.get(b"CIDToGIDMap")
                            .ok()
                            .and_then(|o| match self.doc.dereference(o) {
                                Ok((_, Object::Stream(s))) => s.get_plain_content().ok(),
                                _ => None,
                            });
                    for (cid, w) in cid_widths {
                        let gid = match &cid2gid {
                            Some(bytes) => {
                                let pos = cid as usize * 2;
                                if pos + 1 < bytes.len() {
                                    ((bytes[pos] as u16) << 8) | bytes[pos + 1] as u16
                                } else {
                                    cid
                                }
                            }
                            None => cid,
                        };
                        widths.insert(gid, w);
                    }
                }
            }
            // 未覆盖的字形用 DW
            for gid in map.values() {
                widths.entry(*gid).or_insert(dw);
            }
        } else {
            // 简单字体：Widths[code - FirstChar]
            let first = font.get(b"FirstChar").and_then(Object::as_i64).unwrap_or(0);
            if let Ok(w) = font.get(b"Widths") {
                if let Ok((_, Object::Array(arr))) = self.doc.dereference(w) {
                    let arr = arr.clone();
                    // code → unicode（用解码器）
                    let decoder = resolve_font(font, self.doc);
                    for (k, v) in arr.iter().enumerate() {
                        let code = (first as usize + k) as u8;
                        let uni = decode_string(&decoder, &[code]);
                        let Some(c) = uni.chars().next() else {
                            continue;
                        };
                        if let Some(&gid) = map.get(&(c as u32)) {
                            widths.insert(gid, obj_i64(v).unwrap_or(500));
                        }
                    }
                }
            }
        }
        widths
    }
}

/// 路径段的包围盒。
/// FreeText 注释的 /DA（如 `/Helv 12 Tf 0 g`）→ (字体, 字号, 颜色)。
/// 缺失或无法解析时用阅读器默认 Helvetica 12 黑字。
/// FreeText 注释文本按可用宽度折行（按字体度量逐词累积，超出即换行）。
fn wrap_free_text(text: &str, font: &str, size: f64, max_w: f64) -> String {
    if max_w <= 1.0 {
        return text.to_string();
    }
    let word_w = |w: &str| -> f64 {
        w.chars()
            .map(|c| {
                crate::text::std14_width_1000(font, c)
                    .map(|v| v as f64 / 1000.0)
                    .unwrap_or_else(|| crate::text::fallback_width_em(c))
            })
            .sum::<f64>()
            * size
    };
    let mut lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        let mut cur = String::new();
        let mut cur_w = 0.0;
        for word in para.split_whitespace() {
            let ww = word_w(word);
            let space_w = if cur.is_empty() { 0.0 } else { word_w(" ") };
            if !cur.is_empty() && cur_w + space_w + ww > max_w {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0.0;
            }
            if !cur.is_empty() {
                cur.push(' ');
                cur_w += space_w;
            }
            cur.push_str(word);
            cur_w += ww;
        }
        lines.push(cur);
    }
    lines.join("\n")
}

fn parse_free_text_da(da: Option<&Object>) -> (String, f64, String) {
    let mut font = "Helvetica".to_string();
    let mut size = 12.0f64;
    let mut color = "#000000".to_string();
    let Some(Object::String(bytes, _)) = da else {
        return (font, size, color);
    };
    let ops = crate::text::tokenize(bytes);
    for op in &ops {
        match op.operator.as_str() {
            "Tf" => {
                if let Some(Tok::Name(n)) = op.operands.first() {
                    font = match String::from_utf8_lossy(n).as_ref() {
                        "Helv" => "Helvetica".into(),
                        "HeBo" => "Helvetica-Bold".into(),
                        "TiRo" => "Times-Roman".into(),
                        "TiBo" => "Times-Bold".into(),
                        "TiIt" => "Times-Italic".into(),
                        "Cour" => "Courier".into(),
                        "CoBo" => "Courier-Bold".into(),
                        other => other.to_string(),
                    };
                }
                if let Some(v) = op.operands.get(1).and_then(|t| match t {
                    Tok::Num(v) => Some(*v),
                    _ => None,
                }) {
                    if v > 0.0 {
                        size = v;
                    }
                }
            }
            "g" => {
                let v = op.operands.first().and_then(|t| match t {
                    Tok::Num(v) => Some(*v),
                    _ => None,
                });
                if let Some(v) = v {
                    color = Rgb::gray(v).hex();
                }
            }
            "rg" => {
                color = Rgb::from_args(&op.operands).hex();
            }
            _ => {}
        }
    }
    (font, size, color)
}

/// PDF 文本串（可能 UTF-16BE 带 BOM）→ String。
fn decode_pdf_text_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| ((c[0] as u16) << 8) | *c.get(1).unwrap_or(&0) as u16)
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// 是否需要保留原始码位：复杂文种整形（阿拉伯/希伯来/天城文等）。
fn needs_raw_codes(text: &str) -> bool {
    // U+0000 / 私用区占位 / C0-C1 控制符：ToUnicode 无效或缺失（如只给
    // 空 codespacerange，pdf.js/issue7696 的 CJK 子集），解码结果不可信，
    // 按 unicode 回写会得到豆腐块；改为原码回写交给字体内置映射
    if text.chars().any(|c| {
        let u = c as u32;
        u == 0
            || u == 0xFFFD
            || (0xE000..=0xF8FF).contains(&u)
            || (u < 0x20)
            || (0x7F..=0x9F).contains(&u)
    }) {
        return true;
    }
    text.chars().any(|c| {
        matches!(c as u32,
            0x0590..=0x05FF      // 希伯来
            | 0x0600..=0x06FF    // 阿拉伯
            | 0x0700..=0x074F    // 叙利亚
            | 0x0750..=0x077F
            | 0x08A0..=0x08FF
            | 0x0900..=0x097F    // 天城文
            | 0x0980..=0x0DFF    // 印度系
            | 0x0E00..=0x0E7F    // 泰文
            | 0x0F00..=0x0FFF    // 藏文
            | 0xFB50..=0xFDFF    // 阿拉伯表现形式 A
            | 0xFE70..=0xFEFF    // 阿拉伯表现形式 B
        )
    })
}

fn segments_bbox(segs: &[PathSegment]) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    for s in segs {
        for p in &s.points {
            let e = b.get_or_insert([p[0], p[1], p[0], p[1]]);
            e[0] = e[0].min(p[0]);
            e[1] = e[1].min(p[1]);
            e[2] = e[2].max(p[0]);
            e[3] = e[3].max(p[1]);
        }
    }
    b
}

/// 路径段规范化到设备空间：
/// 1. `re`（本地空间存 [x,y],[w,h] 两点）展开为闭合多边形——直接对 (w,h)
///    做 CTM 点变换在翻转/旋转 CTM 下会得到错误矩形（如 fpdf2 渐变
///    `1 0 0 -1 0 H cm` 下裁剪框整体错位，渐变被裁没）；
/// 2. 丢弃无绘图算子跟随的孤立 `m`（零面积子路径，会撑大包围盒，
///    使图案裁剪框大于真实填充区域）。
///
/// 裁剪/填充轮廓统一走这里，保证 bbox 与输出的段一致。
fn normalized_path_segs(segs: &[PathSegment], ctm: &Mat) -> Vec<PathSegment> {
    let mut local: Vec<PathSegment> = Vec::with_capacity(segs.len());
    for s in segs {
        match s.op.as_str() {
            "re" if s.points.len() >= 2 => {
                let (x, y) = (s.points[0][0], s.points[0][1]);
                let (w, h) = (s.points[1][0], s.points[1][1]);
                local.push(PathSegment {
                    op: "m".into(),
                    points: vec![[x, y]],
                });
                local.push(PathSegment {
                    op: "l".into(),
                    points: vec![[x + w, y]],
                });
                local.push(PathSegment {
                    op: "l".into(),
                    points: vec![[x + w, y + h]],
                });
                local.push(PathSegment {
                    op: "l".into(),
                    points: vec![[x, y + h]],
                });
                local.push(PathSegment {
                    op: "h".into(),
                    points: Vec::new(),
                });
            }
            _ => local.push(s.clone()),
        }
    }
    let consumes = |s: Option<&PathSegment>| {
        matches!(
            s.map(|n| n.op.as_str()),
            Some("l") | Some("c") | Some("v") | Some("y") | Some("re")
        )
    };
    let mut out: Vec<PathSegment> = Vec::with_capacity(local.len());
    for (i, s) in local.iter().enumerate() {
        if s.op == "m" && !consumes(local.get(i + 1)) {
            continue;
        }
        let pts: Vec<[f64; 2]> = s
            .points
            .iter()
            .map(|p| {
                let (x, y) = ctm.apply(p[0], p[1]);
                [round_f(x, 2), round_f(y, 2)]
            })
            .collect();
        out.push(PathSegment {
            op: s.op.clone(),
            points: pts,
        });
    }
    out
}

/// 元素坐标按矩阵映射（图案图块 → 页面空间）。
fn map_element(el: Element, m: &Mat) -> Element {
    let tp = |p: &[f64; 2]| -> [f64; 2] {
        let (x, y) = m.apply(p[0], p[1]);
        [round_f(x, 2), round_f(y, 2)]
    };
    let rot = m.rotation_deg();
    let (sx, sy) = (m.x_scale(), m.y_scale());
    match el {
        Element::Rect {
            x,
            y,
            w,
            h,
            fill,
            stroke,
            line_width,
            rotation,
            alpha,
            stroke_alpha,
            blend,
            smask,
        } => {
            let (nx, ny) = m.apply(x, y);
            Element::Rect {
                x: round_f(nx, 2),
                y: round_f(ny, 2),
                w: round_f(w * sx, 2),
                h: round_f(h * sy, 2),
                fill,
                stroke,
                line_width: round_f(line_width * sx, 2),
                rotation: round_f(rotation + rot, 3),
                alpha,
                stroke_alpha,
                blend,
                smask,
            }
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
            alpha,
            stroke_alpha,
            blend,
            smask,
        } => Element::Path {
            segments: segments
                .into_iter()
                .map(|s| PathSegment {
                    op: s.op,
                    points: s.points.iter().map(tp).collect(),
                })
                .collect(),
            fill,
            stroke,
            line_width: round_f(line_width * sx, 2),
            even_odd,
            dash,
            dash_phase,
            line_cap,
            line_join,
            clip,
            alpha,
            stroke_alpha,
            blend,
            smask,
        },
        Element::Polyline {
            points,
            stroke,
            line_width,
            close,
            dash,
            dash_phase,
        } => Element::Polyline {
            points: points.iter().map(tp).collect(),
            stroke,
            line_width: round_f(line_width * sx, 2),
            close,
            dash,
            dash_phase,
        },
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
            alpha,
            blend,
            smask,
            bold,
            italic,
        } => {
            let (nx, ny) = m.apply(x, y);
            Element::Text {
                render_mode,
                stroke_color,
                stroke_width,
                text,
                x: round_f(nx, 2),
                y: round_f(ny, 2),
                size: round_f(size * sy, 2),
                font,
                font_id,
                color,
                rotation: round_f(rotation + rot, 3),
                mirror,
                codes,
                code_bytes,
                word_spacing,
                char_spacing,
                h_scale,
                tm,
                alpha,
                blend,
                smask,
                bold,
                italic,
            }
        }
        Element::Image {
            src,
            preview,
            x,
            y,
            w,
            h,
            rotation,
            alpha,
            blend,
            clip,
            smask,
            smask_params,
            passthrough,
        } => {
            let (nx, ny) = m.apply(x, y);
            Element::Image {
                src,
                preview,
                x: round_f(nx, 2),
                y: round_f(ny, 2),
                w: round_f(w * sx, 2),
                h: round_f(h * sy, 2),
                rotation: round_f(rotation + rot, 3),
                alpha,
                blend,
                clip,
                smask,
                smask_params,
                passthrough,
            }
        }
        other => other,
    }
}

fn cs_name(cs: &Option<CsSpec>) -> String {
    match cs {
        Some(CsSpec::Gray) => "DeviceGray".into(),
        Some(CsSpec::Cmyk) => "DeviceCMYK".into(),
        _ => "DeviceRGB".into(),
    }
}

/// ICCBased 分量数（用于重建时还原色彩空间）。
fn icc_components(cs: &Option<CsSpec>) -> u8 {
    match cs {
        Some(CsSpec::Gray) => 1,
        Some(CsSpec::Cmyk) => 4,
        _ => 0,
    }
}

fn obj_f64(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        Object::Reference(_) => None,
        _ => None,
    }
}

fn obj_i64(o: &Object) -> Option<i64> {
    match o {
        Object::Integer(i) => Some(*i),
        Object::Real(r) => Some(*r as i64),
        _ => None,
    }
}

fn obj_f64_array(o: &Object) -> Option<Vec<f64>> {
    let arr = o.as_array().ok()?;
    let mut out = Vec::with_capacity(arr.len());
    for v in arr {
        match v {
            Object::Integer(i) => out.push(*i as f64),
            Object::Real(r) => out.push(*r as f64),
            _ => return None,
        }
    }
    Some(out)
}

fn object_to_json(o: &Object) -> Value {
    match o {
        Object::Null => Value::Null,
        Object::Boolean(b) => Value::Bool(*b),
        Object::Integer(i) => json!(i),
        Object::Real(r) => json!(*r as f64),
        Object::Name(n) => Value::String(String::from_utf8_lossy(n).to_string()),
        Object::String(_, _) => Value::Null,
        Object::Array(a) => Value::Array(a.iter().map(object_to_json).collect()),
        Object::Dictionary(d) => {
            let mut m = serde_json::Map::new();
            for (k, v) in d.iter() {
                m.insert(String::from_utf8_lossy(k).to_string(), object_to_json(v));
            }
            Value::Object(m)
        }
        Object::Stream(_) => Value::Null,
        Object::Reference(_) => Value::Null,
    }
}

fn tok_num(t: Option<&Tok>) -> f64 {
    match t {
        Some(Tok::Num(v)) => *v,
        _ => 0.0,
    }
}

fn tok_str(t: Option<&Tok>) -> Option<&[u8]> {
    match t {
        Some(Tok::Str(v)) => Some(v),
        _ => None,
    }
}

/// 去掉子集字体前缀（"ABCDEE+PingFangSC" → "PingFangSC"）。
pub fn strip_subset_prefix(base: &str) -> &str {
    if base.len() > 7 && base.as_bytes()[6] == b'+' {
        &base[7..]
    } else {
        base
    }
}

/// JSON 概要（unpack 的 stdout 摘要）。
pub fn summary(result: &UnpackResult, out_dir: &Path) -> serde_json::Value {
    json!({
        "output_dir": out_dir.display().to_string(),
        "pages": result.pages,
        "images": result.images,
        "fonts": result.fonts.len(),
        "warnings": result.warnings,
    })
}
