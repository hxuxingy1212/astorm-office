//! 文档块模型（对应 ai-ppt 的 Element 枚举）
//!
//! 文档是线性流：`parts[].blocks[]` 顺序排列。块用 `type` 标签区分。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 行内运行（run）：一段格式一致的文本
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Run {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    /// 文字底纹填充色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub superscript: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscript: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<bool>,
    /// 超链接 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<String>,
    /// 脚注文本（挂在该 run 之后）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footnote: Option<String>,
    /// 尾注文本（挂在该 run 之后）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endnote: Option<String>,
    /// 批注文本（范围覆盖该 run）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// 批注作者
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_author: Option<String>,
    /// 修订类型：ins（插入）/ del（删除）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    /// 修订作者
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision_author: Option<String>,
    /// 书签名称（在该 run 位置插入书签）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,
    /// 交叉引用目标书签名（插入 REF 域）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_target: Option<String>,
    /// 交叉引用使用 PAGEREF（否则 REF）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pageref: Option<bool>,
    /// 行内符号（w:sym），格式 "Font:Char"（如 "Wingdings:F0B7"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    /// 字间距（pt，负值为紧缩）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f64>,
    /// 全部大写（w:caps）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caps: Option<bool>,
    /// 小型大写字母（w:smallCaps）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_caps: Option<bool>,
    /// 注音/振假名（w:ruby 的 rt 文本，text 为基底文字）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby: Option<String>,
    /// 文字描边（w:outline）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outline: Option<bool>,
    /// 文字阴影（w:shadow）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<bool>,
    /// 阳文（w:emboss）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emboss: Option<bool>,
    /// 阴文（w:imprint）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imprint: Option<bool>,
    /// 行内图片（图元/图标等，随文本流）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<Box<ImageBlock>>,
    /// 行内内容控件属性（SDT）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdt: Option<SdtProps>,
    /// 通用域指令（IF/DOCPROPERTY/STYLEREF/MERGEFIELD/DATE…），text 为缓存结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// 语言标签（w:lang w:val，BCP-47）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// 东亚语言标签（w:lang w:eastAsia）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang_ea: Option<String>,
    /// 从右到左（w:rtl）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// 表单域（w:ffData）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_field: Option<FormField>,
    /// 东亚字体（w:rFonts eastAsia）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub east_asia_font: Option<String>,
    /// 复杂脚本字体（w:rFonts cs）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cs_font: Option<String>,
}

impl Run {
    pub fn plain(text: impl Into<String>) -> Self {
        Run {
            text: text.into(),
            ..Default::default()
        }
    }

    /// 是否为"无样式"纯文本 run
    pub fn is_plain(&self) -> bool {
        self.bold.is_none()
            && self.italic.is_none()
            && self.underline.is_none()
            && self.strike.is_none()
            && self.color.is_none()
            && self.highlight.is_none()
            && self.shading.is_none()
            && self.font_family.is_none()
            && self.font_size.is_none()
            && self.superscript.is_none()
            && self.subscript.is_none()
            && self.code.is_none()
            && self.hyperlink.is_none()
            && self.footnote.is_none()
            && self.endnote.is_none()
            && self.comment.is_none()
            && self.revision.is_none()
            && self.bookmark.is_none()
            && self.ref_target.is_none()
            && self.pageref.is_none()
            && self.symbol.is_none()
            && self.spacing.is_none()
            && self.caps.is_none()
            && self.small_caps.is_none()
            && self.ruby.is_none()
            && self.outline.is_none()
            && self.shadow.is_none()
            && self.emboss.is_none()
            && self.imprint.is_none()
            && self.image.is_none()
            && self.sdt.is_none()
            && self.field.is_none()
            && self.lang.is_none()
            && self.lang_ea.is_none()
            && self.rtl.is_none()
            && self.form_field.is_none()
            && self.east_asia_font.is_none()
            && self.cs_font.is_none()
    }
}

/// 制表位
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TabStop {
    /// 位置（pt）
    pub pos: f64,
    /// 对齐：left/center/right/decimal/bar
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 前导符：none/dot/hyphen/underscore/middleDot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leader: Option<String>,
}

/// 段落
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Paragraph {
    /// 稳定段落 ID（w14:paraId），用于 `@paraId=` 寻址；缺省不输出
    #[serde(skip_serializing_if = "Option::is_none")]
    pub para_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runs: Option<Vec<Run>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_line_indent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hanging_indent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<f64>,
    /// 行距规则为 exact/atLeast 时的行高（pt）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_pt: Option<f64>,
    /// 行距规则：auto / exact / atLeast
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_before: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_after: Option<f64>,
    /// 段前/段后自动间距（w:beforeAutospacing / w:afterAutospacing）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_autospacing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_autospacing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// 与下段同页
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_next: Option<bool>,
    /// 段中不分页
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_lines: Option<bool>,
    /// 段前分页
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_break_before: Option<bool>,
    /// 段落底纹填充色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shading: Option<String>,
    /// 段落下边框颜色（水平分隔线）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    /// 段落四边框颜色（盒式边框，w:pBdr 四边）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_box: Option<String>,
    /// 制表位
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<TabStop>>,
    /// 首字下沉行数（w:framePr dropCap）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_cap: Option<u8>,
    /// 中英文/中西文自动间距（w:autoSpaceDE / w:autoSpaceDN）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_space_de: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_space_dn: Option<bool>,
    /// 调整右缩进（w:adjustRightInd）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjust_right_ind: Option<bool>,
    /// 对齐网格（w:snapToGrid）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snap_to_grid: Option<bool>,
    /// 段内从右到左（w:bidi）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
}

impl Paragraph {
    pub fn text(text: impl Into<String>) -> Self {
        Paragraph {
            text: Some(text.into()),
            ..Default::default()
        }
    }
}

/// 列表项
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ListItem {
    pub blocks: Vec<Block>,
    /// 该项的列表层级（0 起）
    #[serde(default, skip_serializing_if = "is_zero")]
    pub level: u8,
    /// 编号格式（decimal / lowerLetter / upperRoman / bullet ...），非默认为自定义
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_format: Option<String>,
    /// 编号/符号文本模板（如 "%1." 或 "•"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lvl_text: Option<String>,
    /// 起始值（非 1）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    /// 符号字体（Wingdings/Symbol 等，用于自定义项目符号字形）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_font: Option<String>,
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

/// 列表
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ListBlock {
    #[serde(default)]
    pub ordered: bool,
    pub items: Vec<ListItem>,
}

/// 边框（表格/单元格；pt 宽度）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Border {
    /// single / none / double / dashed / dotted / thick ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
}

/// 表格单元格
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TableCell {
    pub blocks: Vec<Block>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colspan: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rowspan: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// 纵向对齐：top / center / bottom
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valign: Option<String>,
    /// 单元格边框（覆盖表格边框）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    /// 单元格内边距（pt，四边）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
}

/// 表格行
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
    /// 行高（pt）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// 行高规则：exact / atLeast
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height_rule: Option<String>,
}

/// 表格
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TableBlock {
    #[serde(default)]
    pub header_row: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub widths: Option<Vec<f64>>,
    pub rows: Vec<TableRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// 表格相对页面的对齐：left / center / right（默认 center）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 表格样式名（w:tblStyle）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// 表格边框（默认灰色单线；style=none 表示无边框）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    /// 表格整体缩进（pt）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent: Option<f64>,
    /// 表格从右到左（w:bidiVisual）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// 表格宽度百分比（w:tblW type=pct）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_pct: Option<f64>,
    /// 表格布局：fixed / autofit（w:tblLayout）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    /// 表格显式宽度（pt，w:tblW type=dxa）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
}

/// 图片
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ImageBlock {
    pub src: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    /// 环绕方式：inline（默认）/ square / tight / topAndBottom / none / through
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
    /// 浮动位置 x（pt，相对页面）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    /// 浮动位置 y（pt，相对页面）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    /// 是否衬于文字下方
    #[serde(skip_serializing_if = "Option::is_none")]
    pub behind_text: Option<bool>,
    /// 裁剪百分比 [左, 上, 右, 下]（0–100）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<[f64; 4]>,
    /// 旋转角度（度）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
}

/// 公式（OMML；首期以文本承载，保留结构）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct FormulaBlock {
    pub latex: String,
    #[serde(default)]
    pub display: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
}

/// 代码块
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct CodeBlock {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    pub text: String,
}

/// 引用块
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct QuoteBlock {
    pub text: String,
}

/// 目录条目（缓存的结果，用于未更新域时也能正确显示）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TocEntry {
    /// 级别 1..3
    #[serde(default)]
    pub level: u8,
    pub text: String,
}

/// 目录
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TocBlock {
    #[serde(default = "default_toc_title")]
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<Vec<u8>>,
    /// 是否生成静态目录（不依赖 Word 更新域；缺点是无自动页码）
    #[serde(rename = "static", default, skip_serializing_if = "Option::is_none")]
    pub is_static: Option<bool>,
    /// 缓存目录项（原文档域结果）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<TocEntry>,
}

fn default_toc_title() -> String {
    "目录".to_string()
}

/// 参考文献条目
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct BibEntry {
    /// article / book / inproceedings / web …
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default)]
    pub authors: Vec<String>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// 参考文献（条目使用悬挂缩进；标题可选）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct BibliographyBlock {
    #[serde(default = "default_bib_style")]
    pub style: String,
    /// 可选标题；为空/缺省则不输出标题（由作者另行添加）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub entries: Vec<BibEntry>,
}

fn default_bib_style() -> String {
    "GB/T 7714".to_string()
}

/// 图表数据系列
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ChartSeries {
    #[serde(default)]
    pub name: String,
    pub values: Vec<f64>,
    /// 系列颜色（6 位 hex）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// 原生图表（bar/column/line/pie/doughnut/area）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ChartBlock {
    #[serde(rename = "chart_type", default = "default_chart_type")]
    pub chart_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    pub series: Vec<ChartSeries>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 图例位置：b/t/l/r/none
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend: Option<String>,
    /// 分类轴标题
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_title: Option<String>,
    /// 数值轴标题
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y_max: Option<f64>,
    /// 显示数据标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_labels: Option<bool>,
    /// 分组：clustered / stacked / percentStacked（柱/条/线/面积）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grouping: Option<String>,
}

fn default_chart_type() -> String {
    "column".to_string()
}

/// 文本框 / 形状
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct TextBoxBlock {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 环绕：inline（默认）/ square 等浮动
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 是否为 VML 文本框（w:pict/v:shape）；由解析得到时置 true，生成时按 VML 输出
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vml: Option<bool>,
}

/// 附件（以 OLE 嵌入对象形式插入，显示预览图标，双击可打开）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct AttachmentBlock {
    /// 被嵌入文件的本地路径 / URL
    pub src: String,
    /// 显示/存储用的文件名（含扩展名）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// OLE ProgID（如 Excel.Sheet.12 / Word.Document.12）；缺省按扩展名推断
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prog_id: Option<String>,
    /// 预览图标（PNG 路径）；缺省用生成的占位图标
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// 附件旁的说明文字
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// 形状 / 线条（DrawingML wps 预设形状；VML 自选图形解析后按此建模）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ShapeBlock {
    /// 预设形状：rect / roundRect / ellipse / triangle / diamond / pentagon /
    /// hexagon / octagon / star5 / rightArrow / leftArrow / upArrow / downArrow /
    /// chevron / line / cloud / heart ...
    pub shape_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// 边框/线条颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// 形状内文字
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 环绕：inline（默认）/ square 等（浮动）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
}

/// 透传关系（原始 rId → 目标）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct RawRel {
    /// 原始关系 Id（在 xml 中出现）
    pub id: String,
    /// 关系类型（完整 URI 或短名）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rel_type: Option<String>,
    /// 目标（相对 document.xml，如 diagrams/data1.xml）
    pub target: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub external: bool,
}

fn is_false(v: &bool) -> bool {
    !*v
}

/// 未建模内容的原样 WordprocessingML 片段（如 SmartArt drawing）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct RawBlock {
    pub xml: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rels: Vec<RawRel>,
    /// SmartArt 可编辑数据（存在时 texts 可修改回写）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagram: Option<DiagramInfo>,
}

/// SmartArt 图形数据（可编辑文本）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct DiagramInfo {
    /// 数据部件路径，如 word/diagrams/data1.xml
    pub data: String,
    /// 文本节点（按顺序），可编辑后回写数据部件
    #[serde(default)]
    pub texts: Vec<String>,
}

/// 表单域（w:ffData）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct FormField {
    /// text / checkbox / dropdown
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,
}

/// 内容控件（SDT）属性
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct SdtProps {
    /// 类型：text / richText / dropDownList / comboBox / date / checkbox / picture
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sdt_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock: Option<String>,
    /// 下拉/组合框选项（displayText）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<String>,
}

/// 块级内容控件（SDT）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct ContentControlBlock {
    #[serde(flatten)]
    pub props: SdtProps,
    pub blocks: Vec<Block>,
}

/// 题注
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct CaptionBlock {
    pub text: String,
    /// figure / table
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<String>,
}

/// 文档块
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Heading {
        level: u8,
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        numbering: Option<bool>,
        /// 标题对齐：left / center / right（缺省用 Heading 样式定义）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        align: Option<String>,
        /// 标题内的 run（保留 run 级格式，如下划线/颜色）
        #[serde(default, skip_serializing_if = "Option::is_none")]
        runs: Option<Vec<Run>>,
    },
    Paragraph(Paragraph),
    List(ListBlock),
    Table(TableBlock),
    Image(ImageBlock),
    Formula(FormulaBlock),
    Code(CodeBlock),
    Quote(QuoteBlock),
    Toc(TocBlock),
    Bibliography(BibliographyBlock),
    Caption(CaptionBlock),
    Chart(ChartBlock),
    #[serde(rename = "textbox")]
    TextBox(TextBoxBlock),
    Attachment(AttachmentBlock),
    #[serde(rename = "shape")]
    Shape(ShapeBlock),
    #[serde(rename = "raw")]
    Raw(RawBlock),
    #[serde(rename = "sdt")]
    Sdt(ContentControlBlock),
    PageBreak,
}

impl Block {
    /// 路径/类型名（与 JSON `type` 字段一致）
    pub fn type_name(&self) -> &'static str {
        match self {
            Block::Heading { .. } => "heading",
            Block::Paragraph(_) => "paragraph",
            Block::List(_) => "list",
            Block::Table(_) => "table",
            Block::Image(_) => "image",
            Block::Formula(_) => "formula",
            Block::Code(_) => "code",
            Block::Quote(_) => "quote",
            Block::Toc(_) => "toc",
            Block::Bibliography(_) => "bibliography",
            Block::Caption(_) => "caption",
            Block::Chart(_) => "chart",
            Block::TextBox(_) => "textbox",
            Block::Attachment(_) => "attachment",
            Block::Shape(_) => "shape",
            Block::Raw(_) => "raw",
            Block::Sdt(_) => "sdt",
            Block::PageBreak => "page_break",
        }
    }

    /// 块内的纯文本（用于 view text）
    pub fn plain_text(&self) -> String {
        match self {
            Block::Heading { text, .. } => text.clone(),
            Block::Paragraph(p) => paragraph_text(p),
            Block::List(l) => l
                .items
                .iter()
                .map(|it| {
                    it.blocks
                        .iter()
                        .map(|b| b.plain_text())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Block::Table(t) => t
                .rows
                .iter()
                .map(|r| {
                    r.cells
                        .iter()
                        .map(|c| {
                            c.blocks
                                .iter()
                                .map(|b| b.plain_text())
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .collect::<Vec<_>>()
                        .join(" | ")
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Block::Image(i) => i.caption.clone().unwrap_or_default(),
            Block::Formula(f) => f.latex.clone(),
            Block::Code(c) => c.text.clone(),
            Block::Quote(q) => q.text.clone(),
            Block::Toc(t) => t.title.clone(),
            Block::Bibliography(b) => b
                .entries
                .iter()
                .map(|e| format!("{} {}", e.authors.join(", "), e.title))
                .collect::<Vec<_>>()
                .join("\n"),
            Block::Caption(c) => c.text.clone(),
            Block::Chart(ch) => {
                let mut s = ch.title.clone().unwrap_or_default();
                for ser in &ch.series {
                    s.push_str(&format!(" {}: {:?}", ser.name, ser.values));
                }
                s
            }
            Block::TextBox(t) => t.text.clone(),
            Block::Attachment(a) => a
                .label
                .clone()
                .unwrap_or_else(|| a.name.clone().unwrap_or_else(|| a.src.clone())),
            Block::Shape(s) => s.text.clone().unwrap_or_default(),
            Block::Raw(_) => String::new(),
            Block::Sdt(s) => s
                .blocks
                .iter()
                .map(|b| b.plain_text())
                .collect::<Vec<_>>()
                .join(""),
            Block::PageBreak => String::new(),
        }
    }
}

/// 段落纯文本（text 优先，否则拼 runs）
pub fn paragraph_text(p: &Paragraph) -> String {
    if let Some(t) = &p.text {
        return t.clone();
    }
    if let Some(runs) = &p.runs {
        return runs
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join("");
    }
    String::new()
}
