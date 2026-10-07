//! 公式元素模型
//!
//! 对应 OOXML `a14:m`（Office 2010 Math，内嵌 OMML）。
//! 本模型以原始 OMML 字符串保存，保证回环不丢数据（不尝试 OMML↔LaTeX 转换）。

use super::Position;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 公式元素
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EquationElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 位置和尺寸
    pub position: Position,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 原始 OMML 数学 XML（`a14:m` 子树）
    pub omml: String,
    /// 纯文本近似（用于查询/预览）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}
