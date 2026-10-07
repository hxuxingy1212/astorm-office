//! 线条元素模型
//!
//! 独立的线条元素（直线/折线/曲线），支持箭头端点与虚线样式。
//! 生成时输出为 OOXML 连接线 `<p:cxnSp>` + `<a:custGeom>`。

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 线条元素
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct LineElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 位置和尺寸（线条的包围盒）
    pub position: super::Position,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 顶点列表（英寸，相对 position 原点；两点为直线，多点折线）
    #[serde(default)]
    pub points: Vec<(f64, f64)>,
    /// 线条颜色（默认主题强调色）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 线条宽度（磅，默认 1.0）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// 虚线样式：solid / dashed / dotted
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dash: Option<String>,
    /// 起点端点：none / arrow / dot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow_start: Option<String>,
    /// 终点端点：none / arrow / dot
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow_end: Option<String>,
    /// 是否平滑曲线（true 时相邻顶点间用二次贝塞尔连接）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<bool>,
    /// 旋转角度（度）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// 元素的动画列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<super::animation::Animation>>,
}
