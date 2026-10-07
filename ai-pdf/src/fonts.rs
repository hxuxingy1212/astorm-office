//! 系统字体嵌入 — repack 逆向生成 PDF 时的 CJK/非 ASCII 文本支撑。
//!
//! 方案：TrueType 字体整档嵌入为 CIDFontType2（Identity-H + CIDToGIDMap
//! Identity + ToUnicode），CID 即字形索引。ttc 集合字体抽取单个 sfnt
//! （表重排 + 校验和重算）。CFF/OTF（'OTTO'）不支持 FontFile2 嵌入，跳过。
//!
//! 查找候选（按序探测，取第一个可解析的 TrueType）：
//! - 请求 family 名映射到候选文件列表；
//! - 无匹配时落到各平台默认 CJK 字体。

use anyhow::{bail, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 字体轮廓格式（决定 FontFile2 还是 FontFile3/OpenType 嵌入）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontFormat {
    /// TrueType（glyf 轮廓）→ CIDFontType2 + FontFile2
    TrueType,
    /// OpenType/CFF（OTTO）→ CIDFontType0 + FontFile3 /OpenType
    OpenType,
    /// 裸 CFF（FontFile3 /Subtype /Type1C）→ 简单 Type1 字体 + FontFile3 /Type1C
    /// （tiff-parser 无法解析裸 CFF，度量与宽度取自源 PDF 的 /Widths）
    Type1C,
    /// Type1 /FontFile（PFA/PFB）→ 简单 Type1 字体直通（原编码 + 原 /Widths）
    Type1,
    /// 非内嵌 CID 字体（无字体程序）：保留 CIDSystemInfo + /W + /Encoding，
    /// 由阅读器按其替换规则选字形
    CidNoFile,
    /// CID-keyed CFF（FontFile3 /Subtype /CIDFontType0C）→ CIDFontType0 直通。
    /// 字形选择由字体内置 charset（CID→GID）决定，原码回写即得同字形，
    /// 宽度取源 PDF 的 /W（ttf-parser 不支持裸 CFF）
    CidCff,
}

impl FontFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            FontFormat::TrueType => "truetype",
            FontFormat::OpenType => "opentype",
            FontFormat::Type1C => "type1c",
            FontFormat::CidCff => "cidcff",
            FontFormat::Type1 => "type1",
            FontFormat::CidNoFile => "cidnone",
        }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> FontFormat {
        if s.eq_ignore_ascii_case("cidnone") {
            FontFormat::CidNoFile
        } else if s.eq_ignore_ascii_case("type1") {
            FontFormat::Type1
        } else if s.eq_ignore_ascii_case("cidcff") {
            FontFormat::CidCff
        } else if s.eq_ignore_ascii_case("type1c") {
            FontFormat::Type1C
        } else if s.eq_ignore_ascii_case("opentype") || s.eq_ignore_ascii_case("cff") {
            FontFormat::OpenType
        } else {
            FontFormat::TrueType
        }
    }
}

/// 已解析的可嵌入字体。
pub struct EmbeddedFont {
    /// BaseFont 名（如 "STHeiti-Medium"）
    pub name: String,
    /// 单个 sfnt 字体文件（从 ttc 抽取后）
    pub data: Vec<u8>,
    pub format: FontFormat,
    pub units_per_em: u16,
    /// unicode → glyph id
    pub cmap: BTreeMap<u32, u16>,
    /// glyph id → advance（字体单位）
    pub advances: BTreeMap<u16, u16>,
    /// glyph id → 字宽（1/1000 em）；来自源 PDF 的 W 数组时优先使用
    pub widths_1000: BTreeMap<u16, i64>,
    pub ascent: i16,
    pub descent: i16,
    pub bbox: [i16; 4],
    pub italic_angle: f32,
    pub cap_height: i16,
    /// 简单字体的 unicode → 源 PDF 码位（原 /Widths 按码位索引；
    /// 字形 id 仍取字体程序 cmap，二者不能混用）
    pub code_map: BTreeMap<u32, u16>,
    /// 简单字体的原 /Widths（码位 → 1/1000 em）
    pub code_widths: BTreeMap<u16, i64>,
    /// 非 Identity 的 CIDToGIDMap 码流（原码回写时原样嵌入）
    pub cid_to_gid: Option<Vec<u8>>,
    /// 原 /W 数组与 /DW（按 CID 索引，原样回写）
    pub w_array: Option<serde_json::Value>,
    pub dw: Option<i64>,
    /// 自定义编码 CMap（Type0 /Encoding 为 CMap 流时原样回写）
    pub encoding_cmap: Option<Vec<u8>>,
}

impl EmbeddedFont {
    /// 文本 → CID 字节流（每字形 2 字节 BE）。缺字形 → gid 0。
    pub fn encode_text(&self, text: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(text.len() * 2);
        for c in text.chars() {
            let gid = self.cmap.get(&(c as u32)).copied().unwrap_or(0);
            out.extend_from_slice(&gid.to_be_bytes());
        }
        out
    }

    /// 字形宽度，PDF 规范单位（1/1000 em）。
    pub fn glyph_width_1000(&self, gid: u16) -> i64 {
        if let Some(&w) = self.widths_1000.get(&gid) {
            return w;
        }
        let raw = self
            .advances
            .get(&gid)
            .copied()
            .unwrap_or(self.units_per_em / 2) as i64;
        raw * 1000 / self.units_per_em.max(1) as i64
    }

    /// 字体单位 → 1/1000 em（FontDescriptor 各度量字段）。
    pub fn scale_1000(&self, v: i16) -> i64 {
        v as i64 * 1000 / self.units_per_em.max(1) as i64
    }

    pub fn bbox_1000(&self) -> [i64; 4] {
        [
            self.scale_1000(self.bbox[0]),
            self.scale_1000(self.bbox[1]),
            self.scale_1000(self.bbox[2]),
            self.scale_1000(self.bbox[3]),
        ]
    }
}

/// 用源 PDF 抽取出的字体文件 + 显式映射构建可嵌入字体。
///
/// 与 `load_font_file` 的区别：cmap 与字宽来自源 PDF（ToUnicode/CIDToGIDMap/W 数组），
/// 不依赖（可能被子集化的）字体自身 cmap，字形与字宽与原文一致。
#[allow(clippy::too_many_arguments)]
pub fn from_resource(
    name: &str,
    data: Vec<u8>,
    format: FontFormat,
    map: &BTreeMap<u32, u16>,
    widths: &BTreeMap<u16, i64>,
    cid_to_gid: Option<Vec<u8>>,
    w_array: Option<serde_json::Value>,
    dw: Option<i64>,
    encoding_cmap: Option<Vec<u8>>,
    code_map: &BTreeMap<u32, u16>,
    code_widths: &BTreeMap<u16, i64>,
) -> Result<EmbeddedFont> {
    // CID-keyed CFF：ttf-parser 不支持裸 CFF，且字形由字体内置 charset
    // 决定（CID→GID 固定），嵌入原件即可；度量/宽度取自源 PDF
    if matches!(
        format,
        FontFormat::CidCff | FontFormat::Type1 | FontFormat::CidNoFile
    ) {
        return Ok(EmbeddedFont {
            name: name.to_string(),
            data,
            format,
            units_per_em: 1000,
            cmap: map.clone(),
            code_map: BTreeMap::new(),
            code_widths: code_widths.clone(),
            advances: BTreeMap::new(),
            widths_1000: widths.clone(),
            ascent: 750,
            descent: -250,
            cid_to_gid,
            w_array,
            dw,
            encoding_cmap,
            bbox: [-200, -300, 1100, 1000],
            italic_angle: 0.0,
            cap_height: 700,
        });
    }
    let face =
        ttf_parser::Face::parse(&data, 0).map_err(|e| anyhow::anyhow!("face parse: {e:?}"))?;
    let units_per_em = face.units_per_em();
    if units_per_em == 0 {
        bail!("invalid units_per_em");
    }
    // 字号映射：简单字体的 unicode→gid 一律取字体程序自身 cmap（子集字体
    // 的字形顺序由程序决定，源 PDF 的 map 只描述 code↔unicode）
    let mut cmap = BTreeMap::new();
    if let Some(tab) = face.tables().cmap {
        for sub in tab.subtables {
            sub.codepoints(|cp| {
                if let Some(gid) = sub.glyph_index(cp) {
                    cmap.entry(cp).or_insert_with(|| gid.0);
                }
            });
        }
    }
    if cmap.is_empty() {
        // 字体程序无可用 cmap（罕见）：退回源 PDF 映射
        cmap = map.clone();
    }
    let mut advances = BTreeMap::new();
    for &gid in cmap.values() {
        if let Some(w) = face.glyph_hor_advance(ttf_parser::GlyphId(gid)) {
            advances.insert(gid, w);
        }
    }
    let (bbox, ascent, descent, italic_angle, cap_height) = (
        face.global_bounding_box(),
        face.typographic_ascender()
            .or(Some(face.ascender()))
            .unwrap_or(800),
        face.typographic_descender()
            .or(Some(face.descender()))
            .unwrap_or(-200),
        face.italic_angle(),
        face.capital_height().unwrap_or(700),
    );
    Ok(EmbeddedFont {
        name: name.to_string(),
        data,
        format,
        units_per_em,
        cmap,
        code_map: code_map.clone(),
        code_widths: code_widths.clone(),
        advances,
        widths_1000: widths.clone(),
        ascent,
        descent,
        cid_to_gid,
        w_array,
        dw,
        encoding_cmap,
        bbox: [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max],
        italic_angle,
        cap_height,
    })
}

/// 判断字体文件格式（'OTTO' → OpenType/CFF；'ttcf'/0x00010000/'true' → TrueType）。
pub fn font_format_of(data: &[u8]) -> Option<FontFormat> {
    if data.len() < 4 {
        return None;
    }
    match &data[..4] {
        b"OTTO" => Some(FontFormat::OpenType),
        [0x00, 0x01, 0x00, 0x00] | b"true" | b"ttcf" => Some(FontFormat::TrueType),
        b"wOFF" | b"wOF2" => None,
        _ => None,
    }
}

/// 平台默认 CJK 字体候选（TrueType only）。
fn default_candidates() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if cfg!(target_os = "macos") {
        v.push(PathBuf::from(
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ));
        v.push(PathBuf::from("/System/Library/Fonts/STHeiti Medium.ttc"));
        v.push(PathBuf::from("/System/Library/Fonts/STHeiti Light.ttc"));
        v.push(PathBuf::from(
            "/System/Library/Fonts/Supplemental/Songti.ttc",
        ));
    } else if cfg!(target_os = "windows") {
        for f in ["msyh.ttc", "msyh.ttf", "simhei.ttf", "simsun.ttc"] {
            v.push(PathBuf::from(format!("C:\\Windows\\Fonts\\{f}")));
        }
    } else {
        for p in [
            "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
            "/usr/share/fonts/truetype/arphic/uming.ttc",
            "/usr/share/fonts/truetype/arphic/ukai.ttc",
        ] {
            v.push(PathBuf::from(p));
        }
    }
    v
}

/// family 名 → 候选文件列表（含 family 精确候选 + 平台默认）。
fn candidates_for(family: &str) -> Vec<PathBuf> {
    let fam = family.to_lowercase();
    let mut named: Vec<PathBuf> = Vec::new();

    // 常见 PDF 基础字体别名 → 衬线/无衬线/等宽分类。未命中时兜底是
    // CJK 无衬线字体，会把 NimbusRoman/Times 这类衬线文档整体渲染成
    // 无衬线（qpdf/source2 等 Ghostscript 产物），先按类别找系统字体。
    const SERIF: &[&str] = &[
        "times",
        "nimbusroman",
        "liberationserif",
        "freeserif",
        "dejavuserif",
        "georgia",
        "garamond",
        "palatino",
        "bookantiqua",
        "bookman",
        "century",
        "roman",
        "serif",
        "baskerville",
    ];
    const MONO: &[&str] = &[
        "courier",
        "nimbusmono",
        "liberationmono",
        "freemono",
        "consolas",
        "monaco",
        "menlo",
        "mono",
    ];
    const SANS: &[&str] = &[
        "helvetica",
        "nimbussans",
        "liberationsans",
        "freesans",
        "dejavusans",
        "arial",
        "verdana",
        "tahoma",
        "calibri",
        "segoe",
        "trebuchet",
        "grotesk",
        "sans",
    ];
    let style = if SERIF.iter().any(|k| fam.contains(k)) {
        "serif"
    } else if MONO.iter().any(|k| fam.contains(k)) {
        "mono"
    } else if SANS.iter().any(|k| fam.contains(k)) {
        "sans"
    } else {
        ""
    };

    if cfg!(target_os = "macos") {
        let style_files: &[&str] = match style {
            "serif" => &[
                "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
                "/System/Library/Fonts/Supplemental/Georgia.ttf",
            ],
            "mono" => &[
                "/System/Library/Fonts/Supplemental/Courier New.ttf",
                "/System/Library/Fonts/Menlo.ttc",
            ],
            "sans" => &[
                "/System/Library/Fonts/Supplemental/Arial.ttf",
                "/System/Library/Fonts/Helvetica.ttc",
            ],
            _ => &[],
        };
        // 显式关键字（含 CJK 字体）优先于样式兜底：否则 "Arial Unicode"
        // 会先命中 Arial.ttf（纯拉丁）导致 CJK 编码为空
        let known: &[(&str, &str)] = &[
            ("songti", "/System/Library/Fonts/Supplemental/Songti.ttc"),
            ("stheiti", "/System/Library/Fonts/STHeiti Medium.ttc"),
            ("hei", "/System/Library/Fonts/STHeiti Medium.ttc"),
            ("kaiti", "/System/Library/Fonts/Supplemental/Kaiti.ttc"),
            (
                "arial unicode",
                "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
            ),
            // 常见非内嵌字体名 → 系统同名字体（含度量，避免落到 CJK 兜底字体）
            ("lucida", "/System/Library/Fonts/LucidaGrande.ttc"),
            ("verdana", "/System/Library/Fonts/Supplemental/Verdana.ttf"),
            ("tahoma", "/System/Library/Fonts/Supplemental/Tahoma.ttf"),
            (
                "trebuchet",
                "/System/Library/Fonts/Supplemental/Trebuchet MS.ttf",
            ),
            ("georgia", "/System/Library/Fonts/Supplemental/Georgia.ttf"),
            (
                "arial black",
                "/System/Library/Fonts/Supplemental/Arial Black.ttf",
            ),
            (
                "arial narrow",
                "/System/Library/Fonts/Supplemental/Arial Narrow.ttf",
            ),
            (
                "courier new",
                "/System/Library/Fonts/Supplemental/Courier New.ttf",
            ),
            (
                "times new roman",
                "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
            ),
            ("geneva", "/System/Library/Fonts/Geneva.ttf"),
            // 中文名（老式中文 PDF 的 /BaseFont 常直接是「宋体」「黑体」等）
            ("宋体", "/System/Library/Fonts/Supplemental/Songti.ttc"),
            ("simsun", "/System/Library/Fonts/Supplemental/Songti.ttc"),
            ("黑体", "/System/Library/Fonts/STHeiti Medium.ttc"),
            ("楷体", "/System/Library/Fonts/Supplemental/Kaiti.ttc"),
            ("仿宋", "/System/Library/Fonts/Supplemental/Songti.ttc"),
            ("微软雅黑", "/System/Library/Fonts/STHeiti Medium.ttc"),
            ("细明体", "/System/Library/Fonts/Supplemental/Songti.ttc"),
            ("pingfang", "/System/Library/Fonts/PingFang.ttc"), // CFF，加载时会跳过
            ("hiragino", "/System/Library/Fonts/Hiragino Sans GB.ttc"), // CFF，跳过
        ];
        for (kw, path) in known {
            if fam.contains(kw) {
                named.push(PathBuf::from(path));
            }
        }
        named.extend(style_files.iter().map(PathBuf::from));
    } else if cfg!(target_os = "windows") {
        let style_files: &[&str] = match style {
            "serif" => &[
                "C:\\Windows\\Fonts\\times.ttf",
                "C:\\Windows\\Fonts\\georgia.ttf",
            ],
            "mono" => &[
                "C:\\Windows\\Fonts\\cour.ttf",
                "C:\\Windows\\Fonts\\consola.ttf",
            ],
            "sans" => &[
                "C:\\Windows\\Fonts\\arial.ttf",
                "C:\\Windows\\Fonts\\segoeui.ttf",
            ],
            _ => &[],
        };
        let known: &[(&str, &str)] = &[
            ("yahei", "C:\\Windows\\Fonts\\msyh.ttc"),
            ("微软雅黑", "C:\\Windows\\Fonts\\msyh.ttc"),
            ("simhei", "C:\\Windows\\Fonts\\simhei.ttf"),
            ("黑体", "C:\\Windows\\Fonts\\simhei.ttf"),
            ("simsun", "C:\\Windows\\Fonts\\simsun.ttc"),
            ("宋体", "C:\\Windows\\Fonts\\simsun.ttc"),
        ];
        for (kw, path) in known {
            if fam.contains(kw) {
                named.push(PathBuf::from(path));
            }
        }
        named.extend(style_files.iter().map(PathBuf::from));
    } else {
        let style_files: &[&str] = match style {
            "serif" => &[
                "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
            ],
            "mono" => &[
                "/usr/share/fonts/truetype/liberation/LiberationMono-Regular.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            ],
            "sans" => &[
                "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            ],
            _ => &[],
        };
        named.extend(style_files.iter().map(PathBuf::from));
    }
    named.extend(default_candidates());
    named
}

fn is_truetype_magic(magic: [u8; 4]) -> bool {
    magic == [0x00, 0x01, 0x00, 0x00] || &magic == b"true" || &magic == b"ttcf"
}

/// 加载系统字体（TrueType）。找不到或全部为 CFF 时报错。
pub fn load_system_font(family: &str) -> Result<EmbeddedFont> {
    let candidates = candidates_for(family);
    let mut last_err = String::from("no candidate files found");
    for path in candidates {
        if !path.exists() {
            continue;
        }
        match load_font_file(&path) {
            // 字体名表缺失时用请求的 family 名（BaseFont 只是标识符）
            Ok(mut f) => {
                if f.name == "EmbeddedFont" {
                    f.name = family
                        .replace(' ', "")
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                        .collect();
                    if f.name.is_empty() {
                        f.name = "EmbeddedFont".into();
                    }
                }
                return Ok(f);
            }
            Err(e) => last_err = format!("{}: {e}", path.display()),
        }
    }
    bail!(
        "no embeddable TrueType CJK font found for '{family}' ({last_err}). \
         Install one (macOS: Arial Unicode / STHeiti; Windows: 微软雅黑; Linux: wqy-microhei) \
         or reference it as system:<family> with an available file."
    )
}

/// 加载单个字体文件（ttf 取本体，ttc 抽取第一个覆盖 CJK 的 face）。
pub fn load_font_file(path: &Path) -> Result<EmbeddedFont> {
    let data = std::fs::read(path)?;
    let mut magic = [0u8; 4];
    if data.len() < 4 {
        bail!("file too small");
    }
    magic.copy_from_slice(&data[..4]);
    if !is_truetype_magic(magic) {
        bail!("not a TrueType font (CFF/OTF cannot be embedded as FontFile2)");
    }
    if &magic == b"ttcf" {
        let faces = ttc_face_count(&data)?;
        for index in 0..faces {
            let sfnt = extract_ttc_face(&data, index)?;
            if let Ok(font) = build_embedded(&sfnt) {
                // 优先选覆盖 CJK 的 face
                if font.cmap.keys().any(|&c| (0x4E00..=0x9FFF).contains(&c)) {
                    return Ok(font);
                }
            }
        }
        // 没有覆盖 CJK 的 face → 用第一个可解析的
        let sfnt = extract_ttc_face(&data, 0)?;
        return build_embedded(&sfnt);
    }
    build_embedded(&data)
}

fn ttc_face_count(data: &[u8]) -> Result<usize> {
    if data.len() < 12 {
        bail!("ttc header too small");
    }
    Ok(u32::from_be_bytes([data[8], data[9], data[10], data[11]]) as usize)
}

/// 从 ttc 抽取单个 face：重排表目录 + 重算校验和。
fn extract_ttc_face(data: &[u8], index: usize) -> Result<Vec<u8>> {
    let faces = ttc_face_count(data)?;
    if index >= faces {
        bail!("ttc face index {index} out of range ({faces})");
    }
    let off_pos = 12 + index * 4;
    if off_pos + 4 > data.len() {
        bail!("ttc offset out of range");
    }
    let face_off = u32::from_be_bytes([
        data[off_pos],
        data[off_pos + 1],
        data[off_pos + 2],
        data[off_pos + 3],
    ]) as usize;
    if face_off + 12 > data.len() {
        bail!("face offset out of range");
    }
    let num_tables = u16::from_be_bytes([data[face_off + 4], data[face_off + 5]]) as usize;

    struct TableRec {
        tag: [u8; 4],
        checksum: u32,
        offset: usize,
        length: usize,
    }
    let mut tables: Vec<TableRec> = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let rec = face_off + 12 + i * 16;
        if rec + 16 > data.len() {
            bail!("table record out of range");
        }
        let mut tag = [0u8; 4];
        tag.copy_from_slice(&data[rec..rec + 4]);
        tables.push(TableRec {
            tag,
            checksum: u32::from_be_bytes([
                data[rec + 4],
                data[rec + 5],
                data[rec + 6],
                data[rec + 7],
            ]),
            offset: u32::from_be_bytes([
                data[rec + 8],
                data[rec + 9],
                data[rec + 10],
                data[rec + 11],
            ]) as usize,
            length: u32::from_be_bytes([
                data[rec + 12],
                data[rec + 13],
                data[rec + 14],
                data[rec + 15],
            ]) as usize,
        });
    }
    tables.sort_by_key(|t| t.tag);

    // 重排：表数据 4 字节对齐，紧随表目录
    let dir_size = 12 + tables.len() * 16;
    let mut out: Vec<u8> = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[face_off..face_off + 12]); // sfnt version + numTables 等（后续修 head）
                                                           // 写表目录占位
    out.resize(dir_size, 0);
    for (i, t) in tables.iter().enumerate() {
        if t.offset + t.length > data.len() {
            bail!("table data out of range");
        }
        let mut new_off = out.len() as u32;
        out.extend_from_slice(&data[t.offset..t.offset + t.length]);
        while !out.len().is_multiple_of(4) {
            out.push(0);
        }
        let rec = 12 + i * 16;
        out[rec..rec + 4].copy_from_slice(&t.tag);
        out[rec + 4..rec + 8].copy_from_slice(&t.checksum.to_be_bytes());
        out[rec + 8..rec + 12].copy_from_slice(&new_off.to_be_bytes());
        out[rec + 12..rec + 16].copy_from_slice(&(t.length as u32).to_be_bytes());
        let _ = new_off;
        new_off = 0;
        let _ = new_off;
    }
    // head.checkSumAdjustment（offset 8 in head）：整字体校验和的补码
    if let Some(_head) = tables.iter().find(|t| &t.tag == b"head") {
        // head 新偏移：查目录
        for (i, t) in tables.iter().enumerate() {
            if &t.tag == b"head" {
                let rec = 12 + i * 16;
                let off =
                    u32::from_be_bytes([out[rec + 8], out[rec + 9], out[rec + 10], out[rec + 11]])
                        as usize;
                if off + 12 <= out.len() {
                    out[off + 8..off + 12].copy_from_slice(&[0; 4]);
                    let sum: u32 = out
                        .chunks(4)
                        .map(|c| {
                            let mut b = [0u8; 4];
                            b[..c.len()].copy_from_slice(c);
                            u32::from_be_bytes(b)
                        })
                        .fold(0u32, |a, v| a.wrapping_add(v));
                    let adj = 0xB1B0_AFBAu32.wrapping_sub(sum);
                    out[off + 8..off + 12].copy_from_slice(&adj.to_be_bytes());
                }
                break;
            }
        }
    }
    Ok(out)
}

/// 从 sfnt 数据构建 EmbeddedFont（ttf-parser 提供度量）。
fn build_embedded(sfnt: &[u8]) -> Result<EmbeddedFont> {
    let face =
        ttf_parser::Face::parse(sfnt, 0).map_err(|e| anyhow::anyhow!("face parse: {e:?}"))?;
    let units_per_em = face.units_per_em();
    if units_per_em == 0 {
        bail!("invalid units_per_em");
    }

    let mut cmap = BTreeMap::new();
    if let Some(tab) = face.tables().cmap {
        for sub in tab.subtables {
            sub.codepoints(|cp| {
                if let Some(gid) = sub.glyph_index(cp) {
                    cmap.entry(cp).or_insert_with(|| gid.0);
                }
            });
        }
    }
    if cmap.is_empty() {
        bail!("font has no usable cmap");
    }

    let mut advances = BTreeMap::new();
    for &gid in cmap.values() {
        if let Some(w) = face.glyph_hor_advance(ttf_parser::GlyphId(gid)) {
            advances.insert(gid, w);
        }
    }

    let name = face
        .names()
        .into_iter()
        .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
        .and_then(|n| n.to_string())
        .or_else(|| {
            face.names()
                .into_iter()
                .find(|n| n.name_id == ttf_parser::name_id::FAMILY)
                .and_then(|n| n.to_string())
        })
        .unwrap_or_else(|| "EmbeddedFont".to_string())
        .replace(' ', "");

    let (bbox, ascent, descent, italic_angle, cap_height) = (
        face.global_bounding_box(),
        face.typographic_ascender()
            .or(Some(face.ascender()))
            .unwrap_or(800),
        face.typographic_descender()
            .or(Some(face.descender()))
            .unwrap_or(-200),
        face.italic_angle(),
        face.capital_height().unwrap_or(700),
    );

    Ok(EmbeddedFont {
        name,
        data: sfnt.to_vec(),
        format: FontFormat::TrueType,
        units_per_em,
        cmap,
        code_map: BTreeMap::new(),
        code_widths: BTreeMap::new(),
        advances,
        widths_1000: BTreeMap::new(),
        ascent,
        descent,
        bbox: [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max],
        italic_angle,
        cap_height,
        cid_to_gid: None,
        w_array: None,
        dw: None,
        encoding_cmap: None,
    })
}

/// 生成 /ToUnicode CMap（cid → unicode；cid 即 gid）。
pub fn build_to_unicode(mapping: &BTreeMap<u16, u32>) -> String {
    let mut out = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<(&u16, &u32)> = mapping.iter().collect();
    for chunk in entries.chunks(100) {
        out.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (cid, cp) in chunk {
            out.push_str(&format!("<{cid:04X}> <{cp:04X}>\n"));
        }
        out.push_str("endbfchar\n");
    }
    out.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    out
}

/// /W 数组：仅列已用 cid 的宽度（c [w] c [w] …）；w 为 1/1000 em。
pub fn build_w_array(used: &BTreeMap<u16, i64>) -> String {
    let mut out = String::from("[");
    for (cid, w) in used {
        out.push_str(&format!(" {cid} [{w}]"));
    }
    out.push_str(" ]");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn macos_cjk_font() -> Option<PathBuf> {
        let p = PathBuf::from("/System/Library/Fonts/Supplemental/Arial Unicode.ttf");
        p.exists().then_some(p)
    }

    #[test]
    fn encode_text_two_bytes_per_glyph() {
        // 合成一个小 cmap 验证编码
        let mut cmap = BTreeMap::new();
        cmap.insert('A' as u32, 3u16);
        cmap.insert(0x4E2D, 100u16);
        let font = EmbeddedFont {
            name: "T".into(),
            data: vec![],
            format: FontFormat::TrueType,
            units_per_em: 1000,
            cmap,
            code_map: BTreeMap::new(),
            code_widths: BTreeMap::new(),
            advances: BTreeMap::new(),
            widths_1000: BTreeMap::new(),
            ascent: 800,
            descent: -200,
            bbox: [0, 0, 1000, 1000],
            italic_angle: 0.0,
            cap_height: 700,
            cid_to_gid: None,
            w_array: None,
            dw: None,
            encoding_cmap: None,
        };
        assert_eq!(font.encode_text("A中"), vec![0, 3, 0, 100]);
        // 缺字形 → gid 0
        assert_eq!(font.encode_text("Z"), vec![0, 0]);
    }

    #[test]
    fn width_scales_to_1000_em() {
        // units_per_em=2048 的字体：原始 1139 → 556（1/1000 em）
        let mut advances = BTreeMap::new();
        advances.insert(75u16, 1139u16);
        let font = EmbeddedFont {
            name: "T".into(),
            data: vec![],
            format: FontFormat::TrueType,
            units_per_em: 2048,
            cmap: BTreeMap::new(),
            code_map: BTreeMap::new(),
            code_widths: BTreeMap::new(),
            advances,
            widths_1000: BTreeMap::new(),
            ascent: 1638,
            descent: -410,
            bbox: [0, -410, 2048, 1638],
            italic_angle: 0.0,
            cap_height: 1409,
            cid_to_gid: None,
            w_array: None,
            dw: None,
            encoding_cmap: None,
        };
        assert_eq!(font.glyph_width_1000(75), 556);
        assert_eq!(font.scale_1000(1638), 799);
        assert_eq!(font.bbox_1000(), [0, -200, 1000, 799]);
        // 显式 1000 单位字宽优先
        let mut widths = BTreeMap::new();
        widths.insert(75u16, 610i64);
        let font2 = EmbeddedFont {
            widths_1000: widths,
            ..font
        };
        assert_eq!(font2.glyph_width_1000(75), 610);
    }

    #[test]
    fn to_unicode_cmap_format() {
        let mut m = BTreeMap::new();
        m.insert(3u16, 'A' as u32);
        m.insert(100u16, 0x4E2D);
        let cmap = build_to_unicode(&m);
        assert!(cmap.contains("<0003> <0041>"));
        assert!(cmap.contains("<0064> <4E2D>")); // 100 = 0x64
        assert!(cmap.contains("beginbfchar"));
    }

    #[test]
    fn w_array_format() {
        let mut w = BTreeMap::new();
        w.insert(3u16, 556i64);
        assert_eq!(build_w_array(&w), "[ 3 [556] ]");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn loads_real_system_font() {
        let Some(path) = macos_cjk_font() else { return };
        let font = load_font_file(&path).expect("Arial Unicode should load");
        assert!(font.cmap.contains_key(&('中' as u32)), "should cover CJK");
        assert!(font.cmap.contains_key(&('A' as u32)));
        assert!(font.units_per_em > 0);
        let gid = font.cmap[&('中' as u32)];
        assert!(
            font.glyph_width_1000(gid) > 900,
            "CJK glyph ~1em wide (1/1000 em)"
        );
        // 编码往返
        let bytes = font.encode_text("中文");
        assert_eq!(bytes.len(), 4);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn extracts_ttc_face() {
        let p = PathBuf::from("/System/Library/Fonts/Supplemental/Songti.ttc");
        if !p.exists() {
            return;
        }
        let font = load_font_file(&p).expect("Songti.ttc should load");
        assert!(font.cmap.contains_key(&('宋' as u32)));
    }
}
