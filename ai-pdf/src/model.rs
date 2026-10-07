//! 产物目录数据模型 — 对齐 ai-ppt（json2pptx）的设计思路：
//! serde 模型是单一事实源，`unpack`（PDF → 产物目录）与 `repack`（产物目录 → PDF）
//! 关于它对称。坐标单位一律 pt（PDF 用户空间，y 向上）。
//!
//! 产物目录结构：
//! ```text
//! doc/
//! ├── document.json        # 顶层：version/meta/page_size + pages/fonts 文件路径数组
//! ├── pages/
//! │   ├── page-001.json    # 每页一个 JSON（尺寸/旋转/元素列表）
//! │   └── page-002.json
//! ├── media/               # 图片实体（image 元素 src 指向这里的相对路径）
//! └── fonts/               # 内嵌字体实体 + 元数据（font-NNN.ttf / font-NNN.json）
//! ```

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 顶层文档模型（document.json）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DocumentModel {
    /// 产物目录格式版本
    #[serde(default = "default_version")]
    pub version: u32,
    /// 文档元信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// 默认页面尺寸（pt）；页面可覆盖
    #[serde(default)]
    pub page_size: PageSize,
    /// 页面文件路径数组（相对产物根，如 "pages/page-001.json"）
    pub pages: Vec<String>,
    /// 字体元数据文件路径数组（相对产物根，如 "fonts/font-001.json"）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fonts: Vec<String>,
}

fn default_version() -> u32 {
    2
}

impl Default for DocumentModel {
    fn default() -> Self {
        DocumentModel {
            version: default_version(),
            meta: None,
            page_size: PageSize::default(),
            pages: Vec::new(),
            fonts: Vec::new(),
        }
    }
}

/// 元信息
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub producer: Option<String>,
}

/// 页面尺寸（pt）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct PageSize {
    pub width: f64,
    pub height: f64,
}

impl Default for PageSize {
    fn default() -> Self {
        PageSize {
            width: 595.28,
            height: 841.89,
        } // A4
    }
}

/// 单页模型（pages/page-NNN.json）
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct PageModel {
    /// 页面尺寸（pt）；缺省用顶层 page_size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// 顺时针旋转角度（0/90/180/270）
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation: u32,
    /// 裁剪框（CropBox，绝对坐标 [x0,y0,x1,y1]）；缺省 = 整页
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<[f64; 4]>,
    /// 页面框（MediaBox，绝对坐标）；原点非零时需原样保留
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mediabox: Option<[f64; 4]>,
    /// 页面稳定 ID（unpack 不产生；dump 填充 1..N，batch 回放按 id 原位替换实现幂等）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    /// 页面元素（按内容流出现顺序）
    #[serde(default)]
    pub elements: Vec<Element>,
}

fn is_zero(v: &u32) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn is_one_f(v: &f64) -> bool {
    (*v - 1.0).abs() < 1e-9
}

/// 内嵌字体元数据（fonts/font-NNN.json）。
///
/// 用于在重建时使用**原始字体**而非系统替代字体，保证字形与字宽一致。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FontResource {
    /// 字体资源 id（Text 元素的 font_id 引用它）
    pub id: String,
    /// 显示名（去子集前缀后的 BaseFont）
    pub name: String,
    /// 字体文件（相对产物根；TrueType/OpenType）
    pub file: String,
    /// 文件格式："truetype" | "opentype"
    #[serde(default = "default_font_format")]
    pub format: String,
    /// unitsPerEm
    pub units_per_em: u16,
    /// Unicode 码点 → 字形 id（由源 PDF 的 ToUnicode/Encoding 与 CIDToGIDMap/cmap 推导）
    pub map: BTreeMap<u32, u16>,
    /// 字形 id → 字宽（1/1000 em，PDF 规范单位）
    pub widths: BTreeMap<u16, i64>,
    /// 简单字体的 /Encoding 名（Type1C/Type1 直通重建用，如 WinAnsiEncoding）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    /// /Encoding 为字典时的 /Differences（原样回写，Type1 直通时码位→字形名）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding_differences: Option<serde_json::Value>,
    /// Type1 /FontFile 的 /Length1（明文段长度）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length1: Option<i64>,
    /// 简单字体的 /Widths（码位 → 1/1000 em）。原 PDF 声明的宽度可能不同于
    /// 字体程序自带 advance（如零宽空格），重建时必须回写原值
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_widths: Option<std::collections::BTreeMap<u16, i64>>,
    /// 简单字体的 unicode → 源码位（与 code_widths 配套；字形 id 仍取字体程序）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_map: Option<std::collections::BTreeMap<u32, u16>>,
    /// 字体程序 cmap 不可用（子集字体常见）：必须按「简单字体 + 原 /Encoding
    /// + 原码位」重建，按 CID 重建会整页 .notdef（pdf.js/pdkids）
    #[serde(default, skip_serializing_if = "is_false")]
    pub simple_rebuild: bool,
    /// 原 FontDescriptor 度量（Type1C 直通重建时回写；TTF/OTF 由字体程序自算）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<FontMetrics>,
    /// Type3 字体直通数据（CharProcs 引用图片时不扁平化，见 unpack）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type3: Option<Type3Data>,
    /// 非 Identity 的 /CIDToGIDMap 码流文件（相对产物根）。
    /// 原码回写（codes）时必须原样带上，否则 CID 指向错误字形
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cid_to_gid: Option<String>,
    /// 原 /W 数组与 /DW（仅非 Identity CIDToGIDMap 的 CID 字体记录：
    /// 该数组按 CID 索引，与原始码位配套，不能用 GID 宽度重建）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w_array: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dw: Option<i64>,
    /// 自定义编码 CMap 文件（Type0 字体 /Encoding 为 CMap 流时保留：
    /// 变长码空间必须原样回写，Identity-H 会破坏 1 字节码）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding_cmap: Option<String>,
}

/// Type3 字体直通：CharProcs 码流原样保留，文本按原码写回。
/// 用于位图字形（CharProcs 内 /Do 引用图片）——扁平化为页面图片后，
/// 阅读器的字形缓存栅格化与直接画图存在采样相位差（sbix 字体 5.0/4.3）。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Type3Data {
    /// 字形名 → CharProc 码流文件（相对产物根，如 fonts/font-001/L.bin）
    pub charprocs: Vec<(String, String)>,
    /// CharProc 资源名 → media 相对路径（仅图片 XObject）
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub xobjects: BTreeMap<String, String>,
    /// 字形图片 src → 其 SMask 的 media 相对路径（重建时恢复软掩膜）
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub xobject_smasks: BTreeMap<String, String>,
    /// /FontMatrix [a,b,c,d,e,f]
    pub matrix: [f64; 6],
    /// /FontBBox
    pub bbox: [i64; 4],
    pub first_char: i64,
    pub last_char: i64,
    /// /Widths（码序，1/1000 em）
    pub widths: Vec<i64>,
    /// /Encoding /Differences：码 → 字形名
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub differences: Vec<(u16, String)>,
}

/// FontDescriptor 度量（1/1000 em / 度数）。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FontMetrics {
    pub ascent: i64,
    pub descent: i64,
    pub cap_height: i64,
    pub italic_angle: f64,
    pub flags: i64,
    pub bbox: [i64; 4],
}

fn default_font_format() -> String {
    "truetype".into()
}

/// 图片直通参数：无法解码为 PNG/JPEG 时，原样保留码流并在重建时用同一滤镜嵌入。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ImagePassthrough {
    /// 滤镜链（PDF 名称，按顺序）
    pub filter: Vec<String>,
    /// DecodeParms（按滤镜顺序，缺省 null）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decode_parms: Vec<serde_json::Value>,
    /// 颜色空间名（DeviceGray/DeviceRGB/DeviceCMYK/…）
    pub color_space: String,
    /// ICCBased 分量数（无则 0）
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub icc_components: u8,
    #[serde(default = "default_bpc")]
    pub bpc: i64,
    pub width: i64,
    pub height: i64,
    /// /Decode 数组（如 CMYK JPEG 的反相）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decode: Option<Vec<f64>>,
    /// /ImageMask true（1bit 掩膜，用填充色绘制）
    #[serde(default, skip_serializing_if = "is_false")]
    pub image_mask: bool,
    /// 软掩膜图片路径（media/ 相对路径）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smask: Option<String>,
    /// 软掩膜的直通参数（无法解码时原样嵌入）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smask_params: Option<Box<ImagePassthrough>>,
    /// SMask 流的 /Matte（1-4 分量预乘底色；丢失会让还原颜色整体偏移，
    /// 如 pdfium/matte.pdf 色块由橙黄变灰紫）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matte: Option<Vec<f64>>,
    /// JBIG2 /DecodeParms /JBIG2Globals 码流（media/ 相对路径）。
    /// 丢失会让解码器无法解出任何图像（pdfminer/pdf-with-jbig2 整页空白）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub globals: Option<String>,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}
fn default_one_u8() -> u8 {
    1
}
fn is_one_u8(v: &u8) -> bool {
    *v == 1
}
fn default_mesh_cs() -> String {
    "DeviceRGB".to_string()
}

fn default_bpc() -> i64 {
    8
}

/// 着色（shading）定义。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShadingDef {
    /// "axial"（轴向）| "radial"（径向）
    pub kind: String,
    /// 轴向 [x0,y0,x1,y1]；径向 [x0,y0,r0,x1,y1,r1]
    pub coords: Vec<f64>,
    /// 色标（位置 0-1 → 颜色 "#RRGGBB"）
    pub stops: Vec<ShadingStop>,
    /// extend [start, end]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extend: Vec<bool>,
    /// 绘制区域（裁剪框；缺省 = 整页）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bbox: Option<[f64; 4]>,
    /// 裁剪路径（设备空间；缺省只用 bbox）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<Vec<PathSegment>>,
    /// 不透明度 0-1
    #[serde(default = "default_alpha", skip_serializing_if = "is_one_f")]
    pub alpha: f64,
    /// 亮度软掩膜渐变（ExtGState /SMask /S /Luminosity /G 表单内的渐变，
    /// 设备空间）。重建为 DeviceGray 表单 + /SMask，还原渐变透明度
    /// （fpdf2 gradient_opacity：掩膜白→黑，丢失时整块变实心）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Box<ShadingDef>>,
    /// 网格渐变（ShadingType 4-7）：码流直通，重建时原样写回
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh: Option<Box<MeshShading>>,
    /// 采样函数（FunctionType 0）：几千个采样点直通，重建时原样写回。
    /// 按色标抽样会漏掉条纹（pdf.js/issue14165 的 5120 采样条纹渐变）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub func0: Option<Box<Func0Data>>,
    /// 着色空间 → 设备空间矩阵 [a,b,c,d,e,f]。仅在**非相似变换**（各向异性
    /// 缩放/剪切）时记录：此时 coords 保持着色空间坐标，由矩阵把圆压成椭圆
    /// （径向渐变的半径无法单独表达椭圆；pdf.js/issue7847_radial）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<[f64; 6]>,
}

/// 网格渐变（free-form/lattice Gouraud、Coons、Tensor）：码流直通。
/// 顶点坐标在着色空间内，重建时用 matrix 把着色空间放回页面。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MeshShading {
    pub shading_type: i64,
    pub bits_per_coordinate: i64,
    pub bits_per_component: i64,
    pub bits_per_flag: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decode: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<Vec<f64>>,
    /// 解码后的码流（相对产物根，如 shadings/mesh-001.bin）
    pub data: String,
    /// 着色空间 → 设备空间矩阵 [a,b,c,d,e,f]
    pub matrix: [f64; 6],
    /// 网格着色空间（分量数必须一致，否则顶点流解析错位；
    /// 复杂的 ICC/DeviceN 按分量数折算为设备空间）
    #[serde(default = "default_mesh_cs")]
    pub color_space: String,
    /// /Function（字典形式，如 FunctionType 2/3）原样回写。缺失会让渲染器
    /// 按分量数直接解释颜色，流解析整体错位（pdf.js/coons-allflags-withfunction）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub func: Option<serde_json::Value>,
    /// /Function 为流时的字典与码流（相对产物根）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub func_stream: Option<FuncStreamData>,
}

/// 网格着色函数的流形式（FunctionType 0/4 等）。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FuncStreamData {
    pub dict: serde_json::Value,
    pub data: String,
}

/// ExtGState `/SMask` 软掩膜：掩膜表单原样直通（内容 + BBox/Matrix + 内部图片）。
/// 缺失时掩膜区域会被画成不透明（pdf.js/smask_* 整块实心、
/// tika/testPDF_angles 的软阴影变成实心黑条）。
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct SoftMask {
    /// "Alpha" | "Luminosity"
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subtype: String,
    /// 掩膜表单内容（相对产物根，如 masks/mask-001.bin，已解码）
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bbox: Vec<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matrix: Vec<f64>,
    /// 表单 /Group 的 /CS（色彩空间名，可空）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_cs: Option<String>,
    /// 掩膜表单内引用的图片：资源名 → media 路径
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub images: std::collections::BTreeMap<String, String>,
    /// 掩膜表单 /Resources /ExtGState（原样，供表单内容里的 /GSn gs 解析）
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub extgstates: std::collections::BTreeMap<String, serde_json::Value>,
    /// 掩膜表单 /Resources /Shading（渐变掩膜；按着色空间坐标重建）
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub shadings: std::collections::BTreeMap<String, Box<ShadingDef>>,
    /// /BC 背景色（掩膜外的取值）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backdrop: Option<Vec<f64>>,
    /// /TR 传递函数（JSON 原样）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transfer: Option<serde_json::Value>,
}

/// FunctionType 0 采样函数的直通数据。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Func0Data {
    /// /Size（多维时为多个，重建时原样写回）
    pub size: Vec<i64>,
    pub bits_per_sample: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub encode: Vec<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decode: Vec<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub range: Vec<f64>,
    /// 采样码流（相对产物根，如 shadings/fn-001.bin）
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShadingStop {
    pub offset: f64,
    pub color: String,
}

/// 平铺图案定义（PatternType 1）：以元素列表描述图块内容。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PatternDef {
    /// 图块边界 [x0,y0,x1,y1]
    pub bbox: [f64; 4],
    pub x_step: f64,
    pub y_step: f64,
    /// 图案空间 → 页面空间矩阵 [a,b,c,d,e,f]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<[f64; 6]>,
    /// 图块内的元素
    pub elements: Vec<Element>,
}

/// 透明组/Form XObject 的 /Group 字典（保留隔离与撞色语义）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct GroupInfo {
    /// /I（隔离组）
    #[serde(default, skip_serializing_if = "is_false")]
    pub isolated: bool,
    /// /K（撞色组）
    #[serde(default, skip_serializing_if = "is_false")]
    pub knockout: bool,
    /// /CS 颜色空间名（如 DeviceRGB；缺失 = 继承）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cs: Option<String>,
}

/// 页面元素。坐标一律 pt、y 向上；text 的 y 为基线。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Element {
    /// 文本
    Text {
        /// 文本内容（\n 表示强制换行，生成时按行落 Td）
        text: String,
        /// 基线起点 x
        x: f64,
        /// 基线 y
        y: f64,
        /// 字号（pt）
        #[serde(default = "default_size")]
        size: f64,
        /// 字体名：标准 14（Helvetica/Times-Roman/Courier…）或系统字体名
        /// （如 "system:PingFang SC"，repack 时查找并嵌入）
        #[serde(default = "default_font")]
        font: String,
        /// 内嵌字体资源 id（fonts/*.json 的 id）；存在时优先使用原字体
        #[serde(default, skip_serializing_if = "Option::is_none")]
        font_id: Option<String>,
        /// 文字颜色 "#RRGGBB"
        #[serde(default = "default_color")]
        color: String,
        /// 逆时针旋转角度（度）
        #[serde(default, skip_serializing_if = "is_zero_f")]
        rotation: f64,
        /// 文本基镜像（文本基线性部分行列式 < 0，如水平翻转、转置）。
        /// 与 rotation 联合可精确重构全部纯镜像基 R(θ)·diag(1,-1)
        #[serde(default, skip_serializing_if = "is_false")]
        mirror: bool,
        /// 原始码位（十六进制；仅复杂文种整形文本记录）。
        /// 连写/连字由 shaper 决定，按 unicode 回写会退化为孤立形，
        /// 位置随之漂移（fpdf2 bidi_arabic_lorem_ipsum）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        codes: Option<String>,
        /// 每个码的字节数（1 或 2）
        #[serde(default = "default_one_u8", skip_serializing_if = "is_one_u8")]
        code_bytes: u8,
        /// 文本渲染模式（Tr：1 描边 / 2 填充+描边；0 缺省填充）
        #[serde(default, skip_serializing_if = "is_zero_u8")]
        render_mode: u8,
        /// 文本描边色（Tr=1/2 时有效）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_color: Option<String>,
        /// 文本描边线宽
        #[serde(default = "default_line_width", skip_serializing_if = "is_one")]
        stroke_width: f64,
        /// 词间距（Tw，pt）
        #[serde(default, skip_serializing_if = "is_zero_f")]
        word_spacing: f64,
        /// 字间距（Tc，pt）
        #[serde(default, skip_serializing_if = "is_zero_f")]
        char_spacing: f64,
        /// 水平缩放（Tz，1.0 = 100%）
        #[serde(default = "default_alpha", skip_serializing_if = "is_one_f")]
        h_scale: f64,
        /// 文本基线性部分（设备空间，按行推进向量归一到单位长）。
        /// `size` 只含推进方向的缩放，正交基以外的情形（剪切、
        /// 两轴缩放不等，如 Tm `1.4 -0.7 0.7 0.7`）靠它精确还原、
        /// 否则字形会被拉高（pdf.js/bug946506）。缺省 = 纯旋转/镜像基
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tm: Option<[f64; 4]>,
        /// 不透明度 0-1
        #[serde(default = "default_alpha", skip_serializing_if = "is_one_f")]
        alpha: f64,
        /// 混合模式（/BM，如 Multiply/Screen/Overlay）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blend: Option<String>,
        /// ExtGState /SMask 软掩膜
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smask: Option<Box<SoftMask>>,
        #[serde(default, skip_serializing_if = "is_false")]
        bold: bool,
        #[serde(default, skip_serializing_if = "is_false")]
        italic: bool,
    },
    /// 矩形
    Rect {
        /// 左下角 x
        x: f64,
        /// 左下角 y
        y: f64,
        w: f64,
        h: f64,
        /// 填充色；null 或缺省 = 不填充
        #[serde(skip_serializing_if = "Option::is_none")]
        fill: Option<String>,
        /// 描边色；null = 不描边
        #[serde(skip_serializing_if = "Option::is_none")]
        stroke: Option<String>,
        #[serde(default = "default_line_width", skip_serializing_if = "is_one")]
        line_width: f64,
        /// 逆时针旋转角度（度），绕左下角
        #[serde(default, skip_serializing_if = "is_zero_f")]
        rotation: f64,
        /// 填充不透明度 0-1
        #[serde(default = "default_alpha", skip_serializing_if = "is_one_f")]
        alpha: f64,
        /// 描边不透明度（与填充不同时记录；缺省 = alpha）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_alpha: Option<f64>,
        /// 混合模式（/BM）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blend: Option<String>,
        /// ExtGState /SMask 软掩膜
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smask: Option<Box<SoftMask>>,
    },
    /// 平铺图案填充的矩形
    PatternRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        pattern: PatternDef,
        #[serde(default = "default_alpha", skip_serializing_if = "is_one_f")]
        alpha: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blend: Option<String>,
    },
    /// 路径（折线/曲线，保留贝塞尔控制点）
    Path {
        /// 子路径段：{"op":"m"|"l"|"c"|"h", "points":[[x,y],…]}
        segments: Vec<PathSegment>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fill: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        stroke: Option<String>,
        #[serde(default = "default_line_width", skip_serializing_if = "is_one")]
        line_width: f64,
        /// 奇偶填充规则
        #[serde(default, skip_serializing_if = "is_false")]
        even_odd: bool,
        /// 虚线模式（PDF `d` 操作数；空 = 实线）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dash: Option<Vec<f64>>,
        /// 虚线段相位
        #[serde(default, skip_serializing_if = "is_zero_f")]
        dash_phase: f64,
        /// 线端样式（J：0 平帽/1 圆帽/2 方帽）；虚线外观由它决定
        #[serde(default, skip_serializing_if = "is_zero_f")]
        line_cap: f64,
        /// 线连接样式（j：0 尖角/1 圆角/2 斜切）
        #[serde(default, skip_serializing_if = "is_zero_f")]
        line_join: f64,
        /// 裁剪路径（设备空间）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        clip: Option<Vec<PathSegment>>,
        #[serde(default = "default_alpha", skip_serializing_if = "is_one_f")]
        alpha: f64,
        /// 描边不透明度（与填充不同时记录；缺省 = alpha）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_alpha: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blend: Option<String>,
        /// ExtGState /SMask 软掩膜
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smask: Option<Box<SoftMask>>,
    },
    /// 折线（两点即线段）；兼容旧格式，unpack 现在输出 path
    Polyline {
        /// [[x1,y1],[x2,y2],…]
        points: Vec<[f64; 2]>,
        #[serde(default = "default_color")]
        stroke: String,
        #[serde(default = "default_line_width", skip_serializing_if = "is_one")]
        line_width: f64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        close: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dash: Option<Vec<f64>>,
        #[serde(default, skip_serializing_if = "is_zero_f")]
        dash_phase: f64,
    },
    /// 渐变着色（轴向/径向）
    Shading(ShadingDef),
    /// 图片
    Image {
        /// 图片文件：产物根的相对路径（如 "media/img-001.png"）或绝对路径
        src: String,
        /// 预览图（产物根相对路径）。仅对无法被浏览器直接解码的原图
        /// （JPX/JBIG2/CCITT）生成，render/预览优先使用；repack 始终
        /// 用原字节（passthrough），此字段不参与重建。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preview: Option<String>,
        /// 左下角 x
        x: f64,
        /// 左下角 y
        y: f64,
        w: f64,
        h: f64,
        /// 逆时针旋转角度（度），绕左下角
        #[serde(default, skip_serializing_if = "is_zero_f")]
        rotation: f64,
        /// 不透明度 0-1
        #[serde(default = "default_alpha", skip_serializing_if = "is_one_f")]
        alpha: f64,
        /// 混合模式（/BM）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blend: Option<String>,
        /// 裁剪路径（设备空间）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        clip: Option<Vec<PathSegment>>,
        /// 软掩膜图片（media/ 相对路径；PNG/JPEG 路径也会带）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smask: Option<String>,
        /// 软掩膜的直通参数
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smask_params: Option<Box<ImagePassthrough>>,
        /// 直通参数（无法解码时原样嵌入）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        passthrough: Option<ImagePassthrough>,
    },
    /// 透明组/Form XObject：子元素在组空间，重建为 Form XObject
    Group {
        matrix: [f64; 6],
        bbox: [f64; 4],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        group: Option<GroupInfo>,
        children: Vec<Element>,
        #[serde(default = "default_alpha", skip_serializing_if = "is_one_f")]
        alpha: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blend: Option<String>,
        /// ExtGState /SMask 软掩膜（组作为整体合成时的掩膜）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        smask: Option<Box<SoftMask>>,
    },
}

/// 路径段
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PathSegment {
    /// "m" 起点 | "l" 直线 | "c" 三次贝塞尔 | "h" 闭合
    pub op: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<[f64; 2]>,
}

fn default_size() -> f64 {
    12.0
}
fn default_font() -> String {
    "Helvetica".into()
}
fn default_color() -> String {
    "#000000".into()
}
fn default_line_width() -> f64 {
    1.0
}
fn default_alpha() -> f64 {
    1.0
}
fn is_one(v: &f64) -> bool {
    (*v - 1.0).abs() < 1e-9
}
fn is_zero_f(v: &f64) -> bool {
    v.abs() < 1e-9
}

/// 标准十四字体（PDF 规范内置，无需嵌入）。
pub const STANDARD_FONTS: &[&str] = &[
    "Courier",
    "Courier-Bold",
    "Courier-Oblique",
    "Courier-BoldOblique",
    "Helvetica",
    "Helvetica-Bold",
    "Helvetica-Oblique",
    "Helvetica-BoldOblique",
    "Times-Roman",
    "Times-Bold",
    "Times-Italic",
    "Times-BoldItalic",
    "Symbol",
    "ZapfDingbats",
];

pub fn is_standard_font(name: &str) -> bool {
    STANDARD_FONTS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_serde_tag() {
        let json = r#"{"type":"text","text":"hi","x":10,"y":20}"#;
        let e: Element = serde_json::from_str(json).unwrap();
        match &e {
            Element::Text {
                text,
                size,
                font,
                color,
                rotation,
                alpha,
                ..
            } => {
                assert_eq!(text, "hi");
                assert_eq!(*size, 12.0);
                assert_eq!(font, "Helvetica");
                assert_eq!(color, "#000000");
                assert_eq!(*rotation, 0.0);
                assert_eq!(*alpha, 1.0);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn page_defaults() {
        let p: PageModel = serde_json::from_str("{}").unwrap();
        assert_eq!(p.rotation, 0);
        assert!(p.elements.is_empty());
    }

    #[test]
    fn document_roundtrip() {
        let mut doc = DocumentModel::default();
        doc.pages.push("pages/page-001.json".into());
        let s = serde_json::to_string(&doc).unwrap();
        let back: DocumentModel = serde_json::from_str(&s).unwrap();
        assert_eq!(back.pages.len(), 1);
        assert_eq!(back.version, 2);
    }

    #[test]
    fn path_and_shading_serde() {
        let e: Element = serde_json::from_str(
            r##"{"type":"path","segments":[{"op":"m","points":[[0,0]]},{"op":"c","points":[[1,1],[2,2],[3,3]]}],"stroke":"#ff0000"}"##,
        )
        .unwrap();
        match e {
            Element::Path { segments, .. } => assert_eq!(segments.len(), 2),
            _ => panic!("wrong variant"),
        }
        let s: Element = serde_json::from_str(
            r##"{"type":"shading","kind":"axial","coords":[0,0,10,0],"stops":[{"offset":0,"color":"#ffffff"},{"offset":1,"color":"#000000"}]}"##,
        )
        .unwrap();
        match s {
            Element::Shading(d) => assert_eq!(d.kind, "axial"),
            _ => panic!("wrong variant"),
        }
    }
}
