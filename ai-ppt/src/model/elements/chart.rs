//! 图表元素模型
//!
//! 对应 OOXML 中 `ppt/charts/chartN.xml` 的真实图表对象
//! （`p:graphicFrame` → `a:graphicData` uri 以 `/chart` 结尾）。
//! 与 `BarChart`/`PieChart` 等“用形状拼出的伪图表”组件不同。

use super::{Animation, Position};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 图表数据系列
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ChartSeries {
    /// 系列名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 系列数值
    #[serde(default)]
    pub values: Vec<f64>,
    /// 系列颜色（十六进制）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

/// 真实图表元素
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ChartElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 位置和尺寸
    pub position: Position,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 图表类型：column, bar, line, pie, doughnut, area, scatter, radar
    pub chart_type: String,
    /// 图表标题
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// 分类标签
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
    /// 数据系列
    #[serde(default)]
    pub series: Vec<ChartSeries>,
    /// 是否显示图例
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend: Option<bool>,
    /// 图例位置：top, bottom, left, right, topRight
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legend_position: Option<String>,
    /// 原始图表部件 XML（保留完整样式；生成时优先原样回写）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    /// 动画列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<Animation>>,
}
