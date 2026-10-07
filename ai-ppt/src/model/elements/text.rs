//! 文本元素模型

use crate::model::elements::{Fill, Line, Position, Shadow};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 一个文本运行片段（相同格式的一段文字）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextRun {
    /// 文本内容
    pub text: String,
    /// 字体大小（磅）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// 加粗
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// 斜体
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// 下划线
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    /// 文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 字体名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// 超链接地址（网页 URL）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<String>,
    /// 高亮背景色（十六进制）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub highlight: Option<String>,
    /// 下划线颜色（十六进制）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline_color: Option<String>,
    /// 字符间距（磅，可为负）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spacing: Option<f64>,
    /// 语言标签（如 zh-CN）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// 是否从右到左
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
}

/// 段落，包含多个 TextRun 或纯文本
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Paragraph {
    /// 文本运行片段列表（精细格式控制）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runs: Option<Vec<TextRun>>,
    /// 纯文本内容（无格式分段时使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// 段落级字体大小
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// 段落级加粗
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// 段落级斜体
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// 段落级文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 对齐方式：left, center, right, justify
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 行距倍数（如 1.0 单倍 / 1.2 / 1.5）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<f64>,
    /// 是否显示项目符号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bullet: Option<bool>,
}

/// 文本框元素
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct TextElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 文本内容（简单字符串或多段落）
    pub text: TextContent,
    /// 位置和尺寸
    pub position: Position,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 占位符引用 `type|idx`（来自 p:ph，用于保留占位符语义）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// 字体大小（磅）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// 加粗
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// 斜体
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    /// 下划线
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    /// 文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 字体名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// 水平对齐：left, center, right, justify
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 垂直对齐：top, middle, bottom
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert_align: Option<String>,
    /// 文字方向：horz（横向，默认）/ vert（纵向）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert: Option<String>,
    /// 行距倍数（如 1.0 单倍 / 1.2 / 1.5），段落级优先
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<f64>,
    /// 是否自动换行（默认 true）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap: Option<bool>,
    /// 文本背景填充
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    /// 文本背景透明度
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_alpha: Option<f64>,
    /// 文本边框线
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<Line>,
    /// 文本边框透明度
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_alpha: Option<f64>,
    /// 阴影效果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow: Option<Shadow>,
    /// 其他效果：发光/倒影/柔化边缘/模糊/内阴影/填充覆盖
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effects: Option<super::effects::Effects>,
    /// 文本自适应（normAutofit / spAutoFit / noAutofit）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autofit: Option<Autofit>,
    /// 元素的动画列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<super::animation::Animation>>,
}

/// 文本自适应设置（bodyPr 的 normAutofit / spAutoFit / noAutofit）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Autofit {
    /// none | shape | normal
    pub kind: String,
    /// 字号缩放比例（0~1，normal 时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_scale: Option<f64>,
    /// 行距缩减比例（0~1，normal 时）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ln_spc_reduction: Option<f64>,
}

/// 文本内容的两种表示方式
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum TextContent {
    /// 简单纯文本（不包含段落格式）
    Simple(String),
    /// 多段落富文本（支持精细格式控制）
    Paragraphs(Vec<Paragraph>),
}

impl Default for TextContent {
    fn default() -> Self {
        TextContent::Simple(String::new())
    }
}
