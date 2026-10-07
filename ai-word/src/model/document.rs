//! 顶层文档结构（对应 ai-ppt 的 Presentation）

use crate::model::part::Part;
use crate::model::style::{PageSetup, Style, Theme};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 文档元信息
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

/// 未建模的透传部件（原样保留，如 SmartArt diagrams / customXml / fonts）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct PassthroughPart {
    /// 包内相对路径，如 word/diagrams/data1.xml
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// Base64 编码的原始字节
    pub data: String,
}

/// 顶层文档：整份文档的所有配置与内容
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Document {
    #[serde(default)]
    pub meta: Meta,
    #[serde(default)]
    pub page: PageSetup,
    #[serde(default)]
    pub theme: Theme,
    /// 命名样式覆盖（None 时使用内置默认）
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, Style>,
    /// 文档默认样式（styles.xml 的 w:docDefaults）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defaults: Option<Style>,
    /// 模板名（academic-paper / report / letter / resume / memo）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    /// 水印文本（注入页眉）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watermark: Option<String>,
    /// 页面背景色（w:background，6 位 hex）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// 文档分片
    pub parts: Vec<Part>,
    /// 未建模部件（原样透传）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub passthrough: Vec<PassthroughPart>,
    /// 未建模的样式定义（表格/字符/编号等，原样 XML）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub raw_styles: Vec<String>,
    /// 默认制表位（w:defaultTabStop，twip）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_tab_stop: Option<i64>,
    /// 原样保留的 theme1.xml（配色/字体/效果方案）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_xml: Option<String>,
    /// 是否启用奇偶页不同（settings 的 w:evenAndOddHeaders）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub even_and_odd: Option<bool>,
}

impl Default for Document {
    fn default() -> Self {
        Document {
            meta: Meta::default(),
            page: PageSetup::default(),
            theme: Theme::default(),
            styles: BTreeMap::new(),
            defaults: None,
            template: None,
            watermark: None,
            background: None,
            parts: vec![Part::default()],
            passthrough: Vec::new(),
            raw_styles: Vec::new(),
            default_tab_stop: None,
            theme_xml: None,
            even_and_odd: None,
        }
    }
}

impl Document {
    /// 文档所有块的扁平迭代（跨分片）
    pub fn all_blocks(&self) -> impl Iterator<Item = &crate::model::blocks::Block> {
        self.parts.iter().flat_map(|p| p.blocks.iter())
    }
}
