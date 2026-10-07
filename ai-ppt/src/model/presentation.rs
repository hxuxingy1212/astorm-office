//! 演示文稿顶层结构

use crate::model::slide::Slide;
use crate::model::theme::Theme;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 演示文稿元信息（标题、作者）
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    /// 演示文稿标题
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// 作者名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// 公司
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    /// 生成程序（如 Microsoft Office PowerPoint）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application: Option<String>,
    /// 最后修改者
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_by: Option<String>,
}

/// 演示文稿顶层结构
///
/// 包含整份演示文稿的所有配置和数据。
/// 可作为 JSON 输入进行 PPTX 生成，也可作为 PPTX 解析的输出。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Presentation {
    /// 幻灯片宽度（英寸），默认 13.333（宽屏 16:9）
    #[serde(default = "default_width")]
    pub width: f64,
    /// 幻灯片高度（英寸），默认 7.5（宽屏 16:9）
    #[serde(default = "default_height")]
    pub height: f64,
    /// 元信息（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    /// 模板名称（可选），对应 TemplatePreset 中的预设名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// 主题配置（可选），包含颜色方案和字体
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    /// 幻灯片列表
    pub slides: Vec<Slide>,
    /// 原始 ppt/tableStyles.xml（保留内置表格样式/banding；缺省则生成默认）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_styles: Option<String>,
}

fn default_width() -> f64 {
    13.333
}
fn default_height() -> f64 {
    7.5
}

/// 额外的自定义属性的类型别名
pub type ExtraMap = HashMap<String, serde_json::Value>;
