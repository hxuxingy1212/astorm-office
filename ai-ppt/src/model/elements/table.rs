//! 表格元素模型

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 单元格边框（四边同色同宽）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CellBorder {
    /// 边框颜色
    pub color: String,
    /// 边框宽度（磅）
    #[serde(default = "default_border_width")]
    pub width: f64,
}
fn default_border_width() -> f64 {
    1.0
}

/// 表格单元格
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Cell {
    /// 单元格文字内容
    pub text: String,
    /// 字体大小
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// 加粗
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bold: Option<bool>,
    /// 文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 单元格背景色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// 对齐方式：left, center, right
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    /// 横向合并的列数（缺省 1）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colspan: Option<u32>,
    /// 纵向合并的行数（缺省 1）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rowspan: Option<u32>,
    /// 单元格边框
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border: Option<CellBorder>,
    /// 文字方向：horz（默认）/ vert
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vert: Option<String>,
    /// 单元格内边距 [left, top, right, bottom]（英寸）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<[f64; 4]>,
    /// 单元格图片填充（图片路径/URL）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_image: Option<String>,
    /// 单元格垂直对齐：t（默认）/ ctr / b
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
}

/// 表格元素
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TableElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 位置和尺寸
    pub position: super::Position,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 表格行数据（每行包含多个单元格）
    pub rows: Vec<Vec<Cell>>,
    /// 每列宽度（英寸，缺省均分表格总宽）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_widths: Option<Vec<f64>>,
    /// 是否将第一行作为表头
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_row: Option<bool>,
    /// 内置表格样式 ID（GUID，如 {5C22544A-...}）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_id: Option<String>,
    /// 是否从右到左
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtl: Option<bool>,
    /// 各行高度（英寸）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_heights: Option<Vec<f64>>,
    /// 原始 a:tblPr 属性（firstRow/firstCol/bandRow 等，原样保留）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tbl_attrs: Option<String>,
    /// 默认字体大小
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    /// 默认文字颜色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 动画列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<super::animation::Animation>>,
}
