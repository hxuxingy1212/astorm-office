//! 基础元素模型
//!
//! 定义了 PPTX 中的基本元素类型及其公共数据结构。
//! 所有元素通过 Element 枚举统一管理，使用 serde tag 进行 JSON 反序列化。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 元素在幻灯片上的位置和尺寸（英寸单位）
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Position {
    /// X 坐标（左上角）
    pub x: f64,
    /// Y 坐标（左上角）
    pub y: f64,
    /// 宽度
    pub w: f64,
    /// 高度
    pub h: f64,
}

/// 填充类型：纯色、渐变或图案
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Fill {
    /// 纯色填充，直接传颜色值字符串
    Solid(String),
    /// 渐变填充
    Gradient {
        /// 渐变停止点列表
        stops: Vec<GradientStop>,
        /// 渐变角度（度）
        angle: Option<f64>,
    },
    /// 图案填充
    Pattern {
        /// 图案预设名（如 pct20, diagCross, smallGrid 等 OOXML prst 值）
        pattern: String,
        /// 前景色
        fg: String,
        /// 背景色
        bg: String,
    },
    /// 图片填充
    Image {
        /// 图片路径/URL
        src: String,
    },
}

/// 渐变停止点
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GradientStop {
    /// 停止点颜色
    pub color: String,
    /// 停止点位置（0~1）
    pub position: f64,
}

/// 线条样式
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Line {
    /// 线条颜色
    pub color: String,
    /// 线条宽度（磅），默认 1.0
    #[serde(default = "default_line_width")]
    pub width: f64,
}
fn default_line_width() -> f64 {
    1.0
}

/// 阴影效果
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Shadow {
    /// 模糊半径（磅），默认 4.0
    #[serde(default = "default_blur")]
    pub blur: f64,
    /// 阴影距离（磅），默认 3.0
    #[serde(default = "default_dist")]
    pub distance: f64,
    /// 阴影角度（度），默认 45.0
    #[serde(default = "default_angle")]
    pub angle: f64,
    /// 不透明度（0~1），默认 0.4
    #[serde(default = "default_opacity")]
    pub opacity: f64,
    /// 阴影颜色
    #[serde(default = "default_shadow_color")]
    pub color: String,
}
fn default_blur() -> f64 {
    4.0
}
fn default_dist() -> f64 {
    3.0
}
fn default_angle() -> f64 {
    45.0
}
fn default_opacity() -> f64 {
    0.4
}
fn default_shadow_color() -> String {
    "000000".to_string()
}

pub mod animation;
pub mod chart;
pub mod comment;
pub mod effects;
pub mod equation;
pub mod formula;
pub mod group;
pub mod icon;
pub mod image;
pub mod line;
pub mod opaque;
pub mod shape;
pub mod table;
pub mod text;

pub use animation::*;
pub use chart::*;
pub use comment::*;
pub use effects::*;
pub use equation::*;
pub use formula::*;
pub use group::*;
pub use icon::*;
pub use image::*;
pub use line::*;
pub use opaque::*;
pub use shape::*;
pub use table::*;
pub use text::*;

/// 幻灯片元素枚举
///
/// 通过 JSON 中的 "type" 字段自动反序列化为对应的元素类型。
/// 支持：text, shape, line, image, table, group 以及高级组件。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum Element {
    #[serde(rename = "text")]
    Text(TextElement),
    #[serde(rename = "shape")]
    Shape(ShapeElement),
    #[serde(rename = "line")]
    Line(LineElement),
    #[serde(rename = "image")]
    Image(ImageElement),
    #[serde(rename = "icon")]
    Icon(IconElement),
    #[serde(rename = "formula")]
    Formula(FormulaElement),
    #[serde(rename = "table")]
    Table(TableElement),
    #[serde(rename = "chart")]
    Chart(ChartElement),
    #[serde(rename = "comment")]
    Comment(CommentElement),
    #[serde(rename = "equation")]
    Equation(EquationElement),
    #[serde(rename = "opaque")]
    Opaque(OpaqueElement),
    #[serde(rename = "group")]
    Group(GroupElement),
    // 高级组件：JSON type 值统一 snake_case；camelCase 旧值经 alias 仍可读入（兼容），
    // 但序列化/输出一律为 snake_case。
    #[serde(rename = "progress_bar", alias = "progressBar")]
    ProgressBar(ProgressBar),
    #[serde(rename = "progress_ring", alias = "progressRing")]
    ProgressRing(ProgressRing),
    #[serde(rename = "bar_chart", alias = "barChart")]
    BarChart(BarChart),
    #[serde(rename = "line_chart", alias = "lineChart")]
    LineChart(LineChart),
    #[serde(rename = "pie_chart", alias = "pieChart")]
    PieChart(PieChart),
    #[serde(rename = "ring_chart", alias = "ringChart")]
    RingChart(RingChart),
    #[serde(rename = "kpi_card", alias = "kpiCard")]
    KpiCard(KpiCard),
    #[serde(rename = "rating_stars", alias = "ratingStars")]
    RatingStars(RatingStars),
    #[serde(rename = "timeline")]
    Timeline(Timeline),
    #[serde(rename = "process_flow", alias = "processFlow")]
    ProcessFlow(ProcessFlow),
}

impl Element {
    /// 元素类型名（与 JSON 的 type 字段一致，用于路径寻址）
    pub fn type_name(&self) -> &'static str {
        match self {
            Element::Text(_) => "text",
            Element::Shape(_) => "shape",
            Element::Line(_) => "line",
            Element::Image(_) => "image",
            Element::Icon(_) => "icon",
            Element::Formula(_) => "formula",
            Element::Group(_) => "group",
            Element::Table(_) => "table",
            Element::Chart(_) => "chart",
            Element::Comment(_) => "comment",
            Element::Equation(_) => "equation",
            Element::Opaque(_) => "opaque",
            Element::ProgressBar(_) => "progress_bar",
            Element::ProgressRing(_) => "progress_ring",
            Element::BarChart(_) => "bar_chart",
            Element::LineChart(_) => "line_chart",
            Element::PieChart(_) => "pie_chart",
            Element::RingChart(_) => "ring_chart",
            Element::KpiCard(_) => "kpi_card",
            Element::RatingStars(_) => "rating_stars",
            Element::Timeline(_) => "timeline",
            Element::ProcessFlow(_) => "process_flow",
        }
    }

    /// 元素名称（PowerPoint 选择窗格中的名称）
    pub fn name(&self) -> Option<&str> {
        match self {
            Element::Text(t) => t.name.as_deref(),
            Element::Shape(s) => s.name.as_deref(),
            Element::Line(l) => l.name.as_deref(),
            Element::Image(i) => i.name.as_deref(),
            Element::Icon(i) => i.name.as_deref(),
            Element::Formula(f) => f.name.as_deref(),
            Element::Table(t) => t.name.as_deref(),
            Element::Chart(c) => c.name.as_deref(),
            Element::Comment(_) => None,
            Element::Equation(e) => e.name.as_deref(),
            Element::Opaque(o) => o.name.as_deref(),
            Element::Group(g) => g.name.as_deref(),
            _ => None,
        }
    }

    /// 稳定 ID（p:cNvPr/@id）。高级组件由多个内部形状组成，无单一 id。
    pub fn id(&self) -> Option<u32> {
        match self {
            Element::Text(t) => t.id,
            Element::Shape(s) => s.id,
            Element::Line(l) => l.id,
            Element::Image(i) => i.id,
            Element::Icon(i) => i.id,
            Element::Formula(f) => f.id,
            Element::Table(t) => t.id,
            Element::Chart(c) => c.id,
            Element::Equation(e) => e.id,
            Element::Opaque(o) => o.id,
            Element::Group(g) => g.id,
            _ => None,
        }
    }

    /// 设置稳定 ID（生成端回写 cNvPr/@id）
    pub fn set_id(&mut self, id: u32) {
        let slot = match self {
            Element::Text(t) => &mut t.id,
            Element::Shape(s) => &mut s.id,
            Element::Line(l) => &mut l.id,
            Element::Image(i) => &mut i.id,
            Element::Icon(i) => &mut i.id,
            Element::Formula(f) => &mut f.id,
            Element::Table(t) => &mut t.id,
            Element::Chart(c) => &mut c.id,
            Element::Equation(e) => &mut e.id,
            Element::Opaque(o) => &mut o.id,
            Element::Group(g) => &mut g.id,
            _ => return,
        };
        *slot = Some(id);
    }

    /// 分组元素的子元素（仅 group）
    pub fn children(&self) -> Option<&Vec<Element>> {
        match self {
            Element::Group(g) => Some(&g.children),
            _ => None,
        }
    }

    /// 提取元素文本内容（文本/形状内文字/表格单元格；去样式）
    pub fn text_content(&self) -> Option<String> {
        match self {
            Element::Text(t) => Some(extract_text_content(&t.text)),
            Element::Shape(s) => s.text.as_ref().map(extract_text_content),
            Element::Formula(f) => Some(f.latex.clone()),
            Element::Equation(e) => Some(e.text.clone().unwrap_or_default()),
            Element::Opaque(o) => o.text.clone(),
            Element::Table(t) => {
                let cells: Vec<String> = t
                    .rows
                    .iter()
                    .flatten()
                    .filter(|c| !c.text.is_empty())
                    .map(|c| c.text.clone())
                    .collect();
                if cells.is_empty() {
                    None
                } else {
                    Some(cells.join(" | "))
                }
            }
            _ => None,
        }
    }

    /// 获取元素位置的可变引用（所有元素类型通用）
    pub fn position_mut(&mut self) -> &mut Position {
        match self {
            Element::Text(t) => &mut t.position,
            Element::Shape(s) => &mut s.position,
            Element::Line(l) => &mut l.position,
            Element::Image(i) => &mut i.position,
            Element::Icon(i) => &mut i.position,
            Element::Formula(f) => &mut f.position,
            Element::Table(t) => &mut t.position,
            Element::Chart(c) => &mut c.position,
            Element::Comment(c) => &mut c.position,
            Element::Equation(e) => &mut e.position,
            Element::Opaque(o) => &mut o.position,
            Element::Group(g) => &mut g.position,
            Element::ProgressBar(c) => &mut c.position,
            Element::ProgressRing(c) => &mut c.position,
            Element::BarChart(c) => &mut c.position,
            Element::LineChart(c) => &mut c.position,
            Element::PieChart(c) => &mut c.position,
            Element::RingChart(c) => &mut c.position,
            Element::KpiCard(c) => &mut c.position,
            Element::RatingStars(c) => &mut c.position,
            Element::Timeline(c) => &mut c.position,
            Element::ProcessFlow(c) => &mut c.position,
        }
    }

    /// 获取元素位置（所有元素类型通用）
    pub fn position(&self) -> &Position {
        match self {
            Element::Text(t) => &t.position,
            Element::Shape(s) => &s.position,
            Element::Line(l) => &l.position,
            Element::Image(i) => &i.position,
            Element::Icon(i) => &i.position,
            Element::Formula(f) => &f.position,
            Element::Table(t) => &t.position,
            Element::Chart(c) => &c.position,
            Element::Comment(c) => &c.position,
            Element::Equation(e) => &e.position,
            Element::Opaque(o) => &o.position,
            Element::Group(g) => &g.position,
            Element::ProgressBar(c) => &c.position,
            Element::ProgressRing(c) => &c.position,
            Element::BarChart(c) => &c.position,
            Element::LineChart(c) => &c.position,
            Element::PieChart(c) => &c.position,
            Element::RingChart(c) => &c.position,
            Element::KpiCard(c) => &c.position,
            Element::RatingStars(c) => &c.position,
            Element::Timeline(c) => &c.position,
            Element::ProcessFlow(c) => &c.position,
        }
    }
}

/// 从 TextContent 提取纯文本（Simple 直接取，Paragraphs 拼接各段落文本）
fn extract_text_content(tc: &TextContent) -> String {
    match tc {
        TextContent::Simple(s) => s.clone(),
        TextContent::Paragraphs(paras) => paras
            .iter()
            .filter_map(|p| {
                p.text.clone().or_else(|| {
                    p.runs.as_ref().map(|runs| {
                        runs.iter()
                            .map(|r| r.text.clone())
                            .collect::<Vec<_>>()
                            .join("")
                    })
                })
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

// === 高级组件 ===
//
// 高级组件在生成时会被展开为多个基础形状（Group），
// 展开后不保留组件类型信息（回环解析为普通 Group）。

/// 水平进度条组件
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ProgressBar {
    /// 组件位置和尺寸
    pub position: Position,
    /// 进度值 0-100
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// 进度条颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 轨道（背景条）颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_color: Option<String>,
    /// 是否圆角
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rounded: Option<bool>,
    /// 是否显示标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_label: Option<bool>,
    /// 自定义标签文字（缺省显示百分比）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// 标签文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    /// 标签字号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
}

/// 环形进度指示器组件
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProgressRing {
    /// 组件位置和尺寸
    pub position: Position,
    /// 进度值 0-100
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<f64>,
    /// 进度弧颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 轨道环颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_color: Option<String>,
    /// 环的粗细（点）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness: Option<f64>,
    /// 是否显示中心百分比
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_label: Option<bool>,
    /// 中心文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    /// 中心文字字号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
}

/// 柱状图组件（用矩形形状模拟，非 Excel 图表对象）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BarChart {
    /// 组件位置和尺寸
    pub position: Position,
    /// 数据值数组，每个值一根柱（旧版 JSON 的 value 字段仍兼容）
    #[serde(alias = "value")]
    pub data: Vec<f64>,
    /// 底部标签数组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    /// 每根柱的颜色数组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<String>>,
    /// 纵轴最大值（用于计算柱高比例）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// 柱顶显示数值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_values: Option<bool>,
    /// 显示坐标轴
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<bool>,
    /// 标签颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_color: Option<String>,
    /// 标签字号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
}

/// 折线图组件（用连接线 + 数据点形状模拟，非 Excel 图表对象）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LineChart {
    /// 组件位置和尺寸
    pub position: Position,
    /// 数据值数组，每个值一个数据点（旧版 JSON 的 value 字段仍兼容）
    #[serde(alias = "value")]
    pub data: Vec<f64>,
    /// 底部标签数组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    /// 折线颜色（可传数组，缺省单色）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<String>>,
    /// 纵轴最大值（用于计算点高比例）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// 数据点上方显示数值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_values: Option<bool>,
    /// 显示坐标轴
    #[serde(skip_serializing_if = "Option::is_none")]
    pub axis: Option<bool>,
    /// 是否平滑曲线
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<bool>,
    /// 标签颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_color: Option<String>,
    /// 标签字号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
}

/// 饼图组件（用扇形形状模拟，非 Excel 图表对象）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PieChart {
    /// 组件位置和尺寸
    pub position: Position,
    /// 数据值数组，每个值一个扇区（旧版 JSON 的 value 字段仍兼容）
    #[serde(alias = "value")]
    pub data: Vec<f64>,
    /// 扇区标签数组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    /// 每个扇区的颜色数组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<String>>,
    /// 扇区外侧显示百分比
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_values: Option<bool>,
    /// 标签颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_color: Option<String>,
    /// 标签字号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
}

/// 环形图组件（用环形扇区形状模拟，非 Excel 图表对象）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RingChart {
    /// 组件位置和尺寸
    pub position: Position,
    /// 数据值数组，每个值一个环段（旧版 JSON 的 value 字段仍兼容）
    #[serde(alias = "value")]
    pub data: Vec<f64>,
    /// 环段标签数组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<Vec<String>>,
    /// 每个环段的颜色数组
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<String>>,
    /// 环的粗细（占半径比例，0~1，默认 0.35）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness: Option<f64>,
    /// 外侧显示百分比
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_values: Option<bool>,
    /// 标签颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_color: Option<String>,
    /// 标签字号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
}

/// KPI 卡片组件
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct KpiCard {
    /// 组件位置和尺寸
    pub position: Position,
    /// 主数值（大号粗体显示）
    pub value: String,
    /// 小标题
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// 变化指示（正数绿色 / 负数红色）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<String>,
    /// 卡片背景色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    /// 强调条颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accent: Option<String>,
    /// 卡片圆角
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rounded: Option<bool>,
    /// 文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
}

/// 星级评分组件
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RatingStars {
    /// 组件位置和尺寸
    pub position: Position,
    /// 评分值（旧版 JSON 的 value 字段仍兼容）
    #[serde(alias = "value")]
    pub rating: f64,
    /// 星星总数（默认 5）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<u32>,
    /// 已点亮星星颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 未点亮星星颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_color: Option<String>,
}

/// 水平时间线组件
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Timeline {
    /// 组件位置和尺寸
    pub position: Position,
    /// 时间节点数组（date/title/desc 等，旧版 JSON 的 value 字段仍兼容）
    #[serde(alias = "value")]
    pub items: Vec<serde_json::Value>,
    /// 基线颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_color: Option<String>,
    /// 圆点颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dot_color: Option<String>,
    /// 标签颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_color: Option<String>,
}

/// 流程步骤组件
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProcessFlow {
    /// 组件位置和尺寸
    pub position: Position,
    /// 步骤名称数组（旧版 JSON 的 value 字段仍兼容）
    #[serde(alias = "value")]
    pub steps: Vec<String>,
    /// 每步的圆角矩形颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<Vec<String>>,
    /// 步骤文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<String>,
    /// 步骤字号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
}
