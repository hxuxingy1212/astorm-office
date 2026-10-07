//! 公式元素模型

use crate::model::elements::TextContent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 公式元素（LaTeX 文本，打包时降级为文本框）
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct FormulaElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 位置和尺寸
    pub position: crate::model::elements::Position,
    /// 公式 LaTeX 文本
    pub latex: String,
    /// 公式文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 旋转角度（度）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// 文本内容（降级时用，可与 latex 同源）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<TextContent>,
    /// 字体大小
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// 动画列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<super::animation::Animation>>,
}
