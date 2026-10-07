//! 幻灯片模型

use crate::model::elements::Element;
use crate::model::transition::Transition;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 单张幻灯片
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Slide {
    /// 幻灯片中的元素列表（文本、形状、图片等）
    pub elements: Vec<Element>,
    /// 背景设置
    /// - 字符串: 纯色背景，如 "#FFFFFF"
    /// - 对象: 渐变背景或图片背景
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<serde_json::Value>,
    /// 幻灯片过渡动画
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<Transition>,
    /// 演讲者备注（纯文本，多行用换行符分隔）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}
