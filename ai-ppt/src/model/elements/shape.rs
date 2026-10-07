//! 形状元素模型

use crate::model::elements::{animation::Animation, Fill, Line, Position, Shadow, TextContent};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 自动形状元素（矩形、椭圆、箭头等）
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ShapeElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 形状类型：rect, roundRect, ellipse, triangle, rightArrow 等
    pub shape_type: String,
    /// 位置和尺寸
    pub position: Position,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 占位符引用 `type|idx`（来自 p:ph，用于保留占位符语义）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// 旋转角度（度）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// 填充色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<Fill>,
    /// 填充透明度（0~1）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_alpha: Option<f64>,
    /// 是否不填充
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_fill: Option<bool>,
    /// 边框线
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<Line>,
    /// 边框透明度（0~1）
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
    pub autofit: Option<super::Autofit>,
    /// 形状调整参数（如圆角矩形的圆角大小）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjust: Option<HashMap<String, f64>>,
    /// 形状内的文字
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextContent>,
    /// 文字字体大小
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// 文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 文字对齐方式（水平：left, center, right, justify）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 文字垂直对齐：top, middle, bottom
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert_align: Option<String>,
    /// 行距倍数（如 1.0 单倍 / 1.2 / 1.5），段落级优先
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<f64>,
    /// 动画列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
}
