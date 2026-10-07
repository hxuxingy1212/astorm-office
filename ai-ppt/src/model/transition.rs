//! 幻灯片过渡动画模型

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 幻灯片切换时的过渡动画
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Transition {
    /// 过渡类型：fade, push, wipe, split, cover, cut, dissolve, random
    pub r#type: String,
    /// 过渡速度：slow, med, fast
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    /// 是否点击鼠标时切换
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advance_on_click: Option<bool>,
    /// 自动切换延时（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advance_after: Option<u64>,
}
