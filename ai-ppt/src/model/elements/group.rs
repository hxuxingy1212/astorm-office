//! 分组元素模型

use crate::model::elements::{animation::Animation, Element, Position};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 元素分组，可将多个元素组合为一个整体
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GroupElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 分组位置和尺寸
    pub position: Position,
    /// 分组名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 旋转角度（度）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// 子元素列表
    pub children: Vec<Element>,
    /// 分组的动画列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
}
