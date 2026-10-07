//! 分片（对应 ai-ppt 的 Slide）

use crate::model::blocks::Block;
use crate::model::style::PageSetup;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 节的覆盖设置（页眉/页脚/页面）
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Section {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<PageSetup>,
    /// 页眉文本
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<String>,
    /// 页眉块（保留格式/表格/多段；优先于 header 文本）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_blocks: Option<Vec<Block>>,
    /// 页脚块
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer_blocks: Option<Vec<Block>>,
    /// 首页页眉块
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_header_blocks: Option<Vec<Block>>,
    /// 偶数页页眉块
    #[serde(skip_serializing_if = "Option::is_none")]
    pub even_header_blocks: Option<Vec<Block>>,
    /// 页脚是否显示页码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer_page_number: Option<bool>,
    /// 页脚格式：page（默认，仅页码）/ page_of（第 X 页 共 Y 页）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer_format: Option<String>,
    /// 首页页眉（存在则启用“首页不同”）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_header: Option<String>,
    /// 首页页脚是否显示页码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_footer_page_number: Option<bool>,
    /// 偶数页页眉（存在则启用“奇偶页不同”）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub even_header: Option<String>,
    /// 偶数页页脚是否显示页码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub even_footer_page_number: Option<bool>,
    /// 重置页码起始值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number_start: Option<u32>,
    /// 分节符类型：continuous / nextPage / evenPage / oddPage（w:type）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_type: Option<String>,
    /// 首页不同（w:titlePg）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_page: Option<bool>,
    /// 页码格式（w:pgNumType w:fmt：decimal/upperRoman/lowerLetter…）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_number_format: Option<String>,
}

/// 文档分片：一个 section + 一组有序块
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<Section>,
    pub blocks: Vec<Block>,
}

impl Part {
    pub fn new(blocks: Vec<Block>) -> Self {
        Part {
            section: None,
            blocks,
        }
    }
}
