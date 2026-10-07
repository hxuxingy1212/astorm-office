use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::cell::Cell;
use super::is_false;
use super::style::{Font, StyleDef};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

/// 原样保留的 XML 片段：正常序列化，但在相等性比较中被忽略，
/// 以免影响「生成→解析」往返断言的字段对齐。
#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct RawXml(#[serde(default, skip_serializing_if = "Option::is_none")] pub Option<String>);

impl RawXml {
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }
    pub fn from(xml: &str) -> Self {
        RawXml(Some(xml.to_string()))
    }
}

impl PartialEq for RawXml {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Column {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    /// Outline (grouping) level, 1..=7.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_level: Option<u8>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub collapsed: bool,
    /// Column default style applied to cells lacking their own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleDef>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Row {
    /// 1-based row number. Inferred from position when omitted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_level: Option<u8>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub collapsed: bool,
    /// Row default style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<StyleDef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cells: Vec<Cell>,
}

/// OLE 嵌入对象（创建用）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct OleObject {
    /// ProgID，如 "Excel.Sheet.12" / "Package"。
    pub prog_id: String,
    /// 载荷二进制文件路径（作为 embeddings/oleObjectN.bin 写入）。
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    /// 图标显示文本（DrawAspect=Icon 时）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

/// 图表系列级设置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct ChartSeries {
    /// 系列名（默认取列标题）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 系列类型（组合图用）：bar/column/line/area；缺省用图表类型。
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// 系列颜色（6 位十六进制）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 标记：none/circle/square/diamond/triangle/x/star/dot/dash/plus。（line/scatter）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker: Option<String>,
    /// 是否绘制在次坐标轴。
    #[serde(default, skip_serializing_if = "is_false")]
    pub secondary: bool,
}

/// 图表坐标轴设置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct ChartAxis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// 主网格线（数值轴默认显示）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gridlines: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Chart {
    #[serde(rename = "type")]
    pub chart_type: String,
    /// Source range, e.g. "B2:B8". Optional only when `series` are embedded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_range: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub categories: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Legend position: right/left/top/bottom (or "none" to hide).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_axis_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_axis_title: Option<String>,
    /// Show data labels.
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_values: bool,
    /// Per-series colors (6-hex).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    /// 锚点单元格内的偏移 `[dx, dy]`（EMU），保留图片/图表在单元格内的精确位置。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[i64; 2]>,
    /// 系列级设置（与数据列对齐）；用于组合图/系列标记/系列色。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<ChartSeries>,
    /// 数值轴设置。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_axis: Option<ChartAxis>,
    /// 类别轴设置。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category_axis: Option<ChartAxis>,
    /// 原始图表部件 XML；结构化字段未改动时优先回写以保证保真。
    #[serde(default, skip_serializing_if = "RawXml::is_empty")]
    pub raw: RawXml,
    /// 解析时的结构化字段快照；与当前一致时才会使用 `raw`。
    #[serde(default, skip_serializing_if = "RawXml::is_empty")]
    pub snapshot: RawXml,
}

impl Chart {
    /// 结构化字段（忽略 `raw`/`snapshot`）的 JSON 指纹，用于判断是否被编辑。
    pub fn structured_json(&self) -> String {
        let mut c = self.clone();
        c.raw = RawXml(None);
        c.snapshot = RawXml(None);
        serde_json::to_string(&c).unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Image {
    pub src: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    /// 锚点单元格内的偏移 `[dx, dy]`（EMU）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[i64; 2]>,
}

/// A structured table (ListObject) over a cell range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Table {
    pub range: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub show_header_row: bool,
    /// `insertRow="1"`：插入行时扩展表格（自动填充计算列）。
    #[serde(default, skip_serializing_if = "is_false")]
    pub insert_row: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_banded_rows: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_banded_columns: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_first_column: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_last_column: bool,
    /// Column header names. Derived from the header row when omitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    /// 汇总行（totals row）是否存在。
    #[serde(default, skip_serializing_if = "is_false")]
    pub totals_row: bool,
    /// 各列的汇总定义（与 `columns` 对齐）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub totals: Vec<TableTotal>,
    /// 各列计算列公式（与 `columns` 对齐）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub calculated_formulas: Vec<String>,
}

/// 结构化表格某列的汇总行定义。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct TableTotal {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// 汇总函数：sum/average/count/countNums/max/min/stdDev/var/custom 等。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
}

impl TableTotal {
    pub fn is_empty(&self) -> bool {
        self.label.is_none() && self.function.is_none() && self.formula.is_none()
    }
}

fn default_true() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// A single AutoFilter column criterion (value list or custom comparison).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct FilterColumn {
    /// 0-based column index within the filter range.
    pub col_id: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criteria: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub criteria2: Option<String>,
}

/// Data validation rule (dropdown lists, number ranges, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct DataValidation {
    pub range: String,
    #[serde(rename = "type")]
    pub validation_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula2: Option<String>,
    /// Inline list values (list type) — serialized as a quoted CSV formula.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_blank: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_message: Option<String>,
}

/// A conditional-formatting rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct ConditionalFormat {
    pub range: String,
    #[serde(rename = "type")]
    pub rule_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub formulas: Vec<String>,
    /// colorScale colors (2 or 3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<String>,
    /// dataBar color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// containsText: the text to search for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// top10 rank.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<u32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub percent: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bottom: bool,
    /// iconSet style, e.g. "3TrafficLights1".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_style: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub reverse_icons: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_value: Option<bool>,
    /// timePeriod rule, e.g. "today", "lastWeek", "thisMonth".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_period: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub above_average: Option<bool>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub equal_average: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub std_dev: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<Font>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub stop_if_true: bool,
}

/// 迷你图（sparkline）：在单个单元格内绘制一列数据的小图表。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Sparkline {
    /// 类型：line（默认）/ column / winloss（OOXML stacked）。
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// 数据源范围，如 "A1:A10"（可含跨表前缀）。
    pub data_range: String,
    /// 目标单元格，如 "B1"。
    pub location: String,
    /// 线条/柱颜色（6 位十六进制）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 是否显示高点/低点/首点/末点/负点标记。
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_high: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_low: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_first: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_last: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_negative: bool,
}

/// 透视表字段聚合。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct PivotValue {
    /// 源字段名（列名）。
    pub field: String,
    /// 聚合函数：sum（默认）/ count / average / max / min。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// 显示名（默认 "Sum of <field>"）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// 透视表定义（创建用）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct PivotTable {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 数据源区域，可含表名，如 "Sheet1!A1:D100"。
    pub source: String,
    /// 放置锚点单元格，如 "F2"。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// 行字段（列名）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<String>,
    /// 列字段（列名）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<String>,
    /// 值字段。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<PivotValue>,
}

/// 切片器（slicer）：绑定到同表透视缓存里的某个字段。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Slicer {
    /// 透视缓存字段名（列名）。
    pub field: String,
    /// 所绑定的透视表名（缺省用本表第一个）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pivot: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row_height: Option<u32>,
    /// 放置位置："B2" 或 "B2:F7"。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
}

/// 排序条件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct SortCondition {
    /// 排序列，如 "A:A" 或 "A2:A10"。
    pub column: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub descending: bool,
    /// 排序依据：value（默认）/ cellColor / fontColor / icon。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_on: Option<String>,
}

/// 排序状态（`sortState`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct SortState {
    pub range: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<SortCondition>,
}

/// 结构化形状（创建用）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Shape {
    /// 预设几何：rect/ellipse/roundRect/triangle/rightArrow/diamond 等（默认 rect）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<String>,
    /// 锚点："B2:F7"（区域）或 "B2"（单格，配合 size）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<Size>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<[i64; 2]>,
    /// 纯色填充（6 位十六进制）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    /// 渐变填充："C1-C2" 或 "C1-C2-C3[:angle]"（角度可选）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gradient: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_color: Option<String>,
    /// 线宽（磅）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line_width: Option<f64>,
    /// 旋转（度）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_h: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub flip_v: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Print / page-setup settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct PrintSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paper_size: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub fit_to_page: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_to_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_to_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_left: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_right: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_top: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_bottom: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_header: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub margin_footer: Option<f64>,
    /// `printOptions`：水平/垂直居中。
    #[serde(default, skip_serializing_if = "is_false")]
    pub h_center: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub v_center: bool,
    /// 手动分页：行/列断点（`rowBreaks`/`colBreaks` 的 id）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_breaks: Vec<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub col_breaks: Vec<u32>,
    /// 打印页眉/页脚（`headerFooter`）。
    #[serde(default, skip_serializing_if = "is_false")]
    pub different_odd_even: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub different_first: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub odd_header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub odd_footer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub even_header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub even_footer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_header: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_footer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Sheet {
    pub name: String,
    /// 工作表类型：`None`/worksheet 为普通表，`chart` 为图表工作表（chartsheet）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sheet_type: Option<String>,
    /// 图表工作表的原始部件 XML（原样回写）。
    #[serde(default, skip_serializing_if = "RawXml::is_empty")]
    pub raw_sheet: RawXml,
    /// 图表工作表的原始关系文件 XML（原样回写）。
    #[serde(default, skip_serializing_if = "RawXml::is_empty")]
    pub raw_sheet_rels: RawXml,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tab_color: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gridlines: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headings: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zoom: Option<u32>,
    /// Default column width (characters) from `<sheetFormatPr>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_col_width: Option<f64>,
    /// Default row height (points) from `<sheetFormatPr>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_row_height: Option<f64>,
    /// Base column width from `<sheetFormatPr>`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_col_width: Option<f64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_formulas: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub tab_selected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_filter: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter_columns: Vec<FilterColumn>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub columns: Vec<Column>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub merges: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub protect: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub validations: Vec<DataValidation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditional_formats: Vec<ConditionalFormat>,
    /// 迷你图（sparkline）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sparklines: Vec<Sparkline>,
    /// 透视表（创建用；读取到的透视表以不透明部件保留）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pivot_tables: Vec<PivotTable>,
    /// 切片器（基于透视缓存字段；创建用）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub slicers: Vec<Slicer>,
    /// 结构化形状（创建用；读取到的形状以 `shapes` 原样保留）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub drawing_shapes: Vec<Shape>,
    /// 排序状态（`sortState`）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<SortState>,
    /// OLE 嵌入对象（创建用；读取到的以部件原样保留）。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ole_objects: Vec<OleObject>,
    /// 工作表背景图（`<picture r:id>`），部件路径如 "xl/media/image2.jpeg"。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print: Option<PrintSettings>,
    /// Print area range, e.g. "A1:C20".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_area: Option<String>,
    /// Rows repeated at top when printing, e.g. "1:1".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_title_rows: Option<String>,
    /// Columns repeated at left when printing, e.g. "A:A".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub print_title_cols: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tables: Vec<Table>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rows: Vec<Row>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charts: Vec<Chart>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<Image>,
    /// 原样保留的绘图形状/连接线锚点（`sp`/`cxnSp` 的原始 XML），回写时按原样输出。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shapes: Vec<String>,
    /// 保留的表单控件 VML 部件（按钮等），原样回写。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vml_drawings: Vec<RawXml>,
}
