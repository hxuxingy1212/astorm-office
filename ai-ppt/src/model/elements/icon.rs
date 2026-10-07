//! 图标元素模型

use crate::model::elements::Line;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 图标元素（前端用内置 SVG 图标库渲染，打包时降级为形状）
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct IconElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 位置和尺寸
    pub position: crate::model::elements::Position,
    /// 图标名称（内置 SVG 库）
    pub icon: String,
    /// 图标颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 旋转角度（度）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// 图标边框
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<Line>,
    /// 动画列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<super::animation::Animation>>,
}
