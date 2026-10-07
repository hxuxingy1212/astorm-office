//! 形状/文本效果模型
//!
//! 对应 OOXML `a:effectLst` 中的发光、柔化边缘、模糊、倒影、内阴影、填充覆盖。
//! （外阴影仍由各元素上的 `shadow` 字段表示，保持向后兼容。）

use super::Shadow;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 发光效果
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Glow {
    /// 发光颜色（十六进制）
    pub color: String,
    /// 半径（磅）
    #[serde(default = "default_glow_radius")]
    pub radius: f64,
    /// 不透明度（0~1）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}
fn default_glow_radius() -> f64 {
    8.0
}

/// 填充覆盖效果
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FillOverlay {
    /// 覆盖颜色
    pub color: String,
    /// 不透明度（0~1）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}

/// 效果集合
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Effects {
    /// 发光
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glow: Option<Glow>,
    /// 是否启用倒影
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection: Option<bool>,
    /// 柔化边缘半径（磅）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_edge: Option<f64>,
    /// 模糊半径（磅）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blur: Option<f64>,
    /// 内阴影
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inner_shadow: Option<Shadow>,
    /// 填充覆盖
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_overlay: Option<FillOverlay>,
}

impl Effects {
    /// 是否为空（无任何效果）
    pub fn is_empty(&self) -> bool {
        self.glow.is_none()
            && self.reflection.is_none()
            && self.soft_edge.is_none()
            && self.blur.is_none()
            && self.inner_shadow.is_none()
            && self.fill_overlay.is_none()
    }
}
