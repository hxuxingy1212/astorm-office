//! 主题模型

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 演示文稿主题，包含颜色方案和字体设置
///
/// 颜色键值对说明：
/// - dk1/dk2: 深色，lt1/lt2: 浅色
/// - accent1~accent6: 强调色
/// - hlink/folHlink: 超链接颜色
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Theme {
    /// 颜色方案映射表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<HashMap<String, String>>,
    /// 主要字体（标题用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major_font: Option<String>,
    /// 次要字体（正文用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_font: Option<String>,
}

impl Theme {
    /// 合并另一个主题到当前主题，已有属性会被覆盖
    pub fn merge(&mut self, other: Theme) {
        if let Some(other_colors) = other.colors {
            self.colors
                .get_or_insert_with(HashMap::new)
                .extend(other_colors);
        }
        if other.major_font.is_some() {
            self.major_font = other.major_font;
        }
        if other.minor_font.is_some() {
            self.minor_font = other.minor_font;
        }
    }
}
