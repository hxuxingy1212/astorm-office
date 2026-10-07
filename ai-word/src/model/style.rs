//! 页面设置与样式/主题模型

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 页边距（单位：pt）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Margins {
    #[serde(default = "default_margin")]
    pub top: f64,
    #[serde(default = "default_margin")]
    pub bottom: f64,
    #[serde(default = "default_margin_lr")]
    pub left: f64,
    #[serde(default = "default_margin_lr")]
    pub right: f64,
    #[serde(default = "default_margin_hf")]
    pub header: f64,
    #[serde(default = "default_margin_hf")]
    pub footer: f64,
}

fn default_margin() -> f64 {
    72.0
}
fn default_margin_lr() -> f64 {
    90.0
}
fn default_margin_hf() -> f64 {
    42.0
}

impl Default for Margins {
    fn default() -> Self {
        Margins {
            top: 72.0,
            bottom: 72.0,
            left: 90.0,
            right: 90.0,
            header: 42.0,
            footer: 42.0,
        }
    }
}

/// 页面设置（尺寸单位 pt；纸张用命名）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct PageSetup {
    /// 纸张：A4 / Letter / A3，或 "custom"
    #[serde(default = "default_page_size")]
    pub size: String,
    /// 方向：portrait / landscape
    #[serde(default = "default_orientation")]
    pub orientation: String,
    #[serde(default)]
    pub margins: Margins,
    /// 自定义宽高（pt），仅 size=custom 时使用
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// 栏数（多栏排版）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub columns: Option<u8>,
    /// 栏间距（pt）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_space: Option<f64>,
    /// 页面边框（w:pgBorders）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_border: Option<crate::model::blocks::Border>,
    /// 是否显式指定纸张尺寸；Some(false) 表示原文档无 w:pgSz（生成时省略）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_specified: Option<bool>,
    /// 文档网格（w:docGrid，原样 XML）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doc_grid: Option<String>,
    /// 节从右到左（w:bidi）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// 装订线在右侧（w:rtlGutter）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl_gutter: Option<bool>,
    /// 装订线宽度（w:pgMar w:gutter，pt）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gutter: Option<f64>,
}

fn default_page_size() -> String {
    "A4".to_string()
}
fn default_orientation() -> String {
    "portrait".to_string()
}

impl Default for PageSetup {
    fn default() -> Self {
        PageSetup {
            size: default_page_size(),
            orientation: default_orientation(),
            margins: Margins::default(),
            width: None,
            height: None,
            columns: None,
            column_space: None,
            page_border: None,
            size_specified: None,
            doc_grid: None,
            rtl: None,
            rtl_gutter: None,
            gutter: None,
        }
    }
}

impl PageSetup {
    /// 返回页面尺寸（pt）：(宽, 高)
    ///
    /// 命名纸张按 orientation 交换宽高；custom 直接用 width/height（已是实际朝向）。
    pub fn size_pt(&self) -> (f64, f64) {
        let named = match self.size.as_str() {
            "A4" => Some((595.28, 841.89)),
            "A3" => Some((841.89, 1190.55)),
            "Letter" => Some((612.0, 792.0)),
            _ => None,
        };
        match named {
            Some((w, h)) => {
                if self.orientation.eq_ignore_ascii_case("landscape") {
                    (h, w)
                } else {
                    (w, h)
                }
            }
            None => (self.width.unwrap_or(595.28), self.height.unwrap_or(841.89)),
        }
    }
}

/// 主题（字体与配色）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Theme {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub major_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minor_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub east_asia_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading_font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<BTreeMap<String, String>>,
}

/// 命名样式定义
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Style {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strike: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_spacing: Option<f64>,
    /// 行距规则 exact/atLeast 时的行高（pt）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_pt: Option<f64>,
    /// 行距规则：auto / exact / atLeast
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_rule: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_line_indent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_before: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub space_after: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_autospacing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_autospacing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub east_asia_font: Option<String>,
    /// 段间不额外加距（w:contextualSpacing，常用于 ListParagraph）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_spacing: Option<bool>,
    /// 继承的父样式（w:basedOn）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub based_on: Option<String>,
    /// 左缩进（pt，w:ind left）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indent: Option<f64>,
    /// 悬挂缩进（pt，w:ind hanging）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hanging_indent: Option<f64>,
    /// 制表位
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tabs: Option<Vec<crate::model::blocks::TabStop>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_space_de: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_space_dn: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adjust_right_ind: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snap_to_grid: Option<bool>,
    /// 样式挂接的编号 numId（w:numPr）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_id: Option<String>,
    /// 从右到左（w:bidi / w:rtl）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// 复杂脚本字体（w:rFonts cs）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cs_font: Option<String>,
}
