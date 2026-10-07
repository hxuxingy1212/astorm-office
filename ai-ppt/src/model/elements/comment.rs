//! 批注元素模型
//!
//! 对应 OOXML `ppt/comments/commentN.xml` 中的传统批注（Legacy Comment）。
//! 为了不破坏现有 Slide 结构，批注作为 `Element::Comment` 出现在元素列表中。

use super::Position;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 批注元素
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct CommentElement {
    /// 批注锚点位置（英寸）
    pub position: Position,
    /// 作者
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// 批注内容
    pub text: String,
}
