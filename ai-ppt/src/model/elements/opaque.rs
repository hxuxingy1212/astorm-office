//! 不透明富对象（OLE / SmartArt / 3D / zoom 等）
//!
//! 这些对象引用包内部件（嵌入对象、图数据、.glb 等），无法用固定模型完整表达。
//! 这里以「原始 XML + 关系 + 部件字节」整体保存，生成时原样回写，
//! 从而保证解析↔生成回环不丢数据。

use super::Position;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 元素引用的关系（生成时按新 rId 重写，Target 保持不变）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OpaqueRel {
    /// 原始关系 Id（如 rId5）
    pub old_id: String,
    /// 关系类型（完整 URI）
    pub rel_type: String,
    /// 目标（相对路径或外部 URL）
    pub target: String,
    /// 是否外部关系
    #[serde(default)]
    pub external: bool,
}

/// 关系指向的内部部件（含传递闭包）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OpaquePart {
    /// 包内路径（如 ppt/diagrams/data1.xml）
    pub path: String,
    /// 内容类型
    pub content_type: String,
    /// 字节内容（base64）
    pub data: String,
    /// 该部件自身的关系文件内容（若有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rels_xml: Option<String>,
}

/// 不透明富对象元素
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct OpaqueElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 位置和尺寸
    pub position: Position,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 原始元素 XML（rId 为原始值）
    pub xml: String,
    /// 引用的关系
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rels: Vec<OpaqueRel>,
    /// 关系指向的内部部件
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<OpaquePart>,
    /// 提取的纯文本（用于查询/预览）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}
