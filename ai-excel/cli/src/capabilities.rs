//! 能力表：CLI 支持的元素类型与属性面（单一来源）。
//!
//! 三处消费同一份数据：
//! 1. `help` 命令的输出（人读表格 / `--json` 机器可读）；
//! 2. 未知属性/类型错误时的 `suggestion`（最近匹配 + 合法取值）；
//! 3. 契约测试 `tests/capabilities.rs`：逐条验证声明与实现一致。

/// 单个属性声明
pub struct Prop {
    /// 规范名（文档只写这个）
    pub name: &'static str,
    /// 兼容别名（仍可用，不推荐）
    pub aliases: &'static [&'static str],
    /// 示例值（同时作为契约测试的样值）
    pub sample: &'static str,
    /// 取值说明
    pub note: &'static str,
}

const fn p(name: &'static str, sample: &'static str, note: &'static str) -> Prop {
    Prop {
        name,
        aliases: &[],
        sample,
        note,
    }
}

const fn pa(
    name: &'static str,
    aliases: &'static [&'static str],
    sample: &'static str,
    note: &'static str,
) -> Prop {
    Prop {
        name,
        aliases,
        sample,
        note,
    }
}

/// 单元格（`/sheet[N]/cell[A1]`）
pub const CELL_PROPS: &[Prop] = &[
    p("value", "新值", "字符串/数字/布尔，按内容推断类型"),
    p("formula", "SUM(B2:B9)", "公式不带 =（生成端自动补）"),
    pa(
        "type",
        &["cell_type"],
        "number",
        "string|number|boolean|date|error|richtext",
    ),
    pa(
        "number_format",
        &["numfmt", "format"],
        "#,##0.00",
        "Excel 数字格式串；none 表示清除",
    ),
    pa("link", &["url"], "https://example.com", "超链接"),
    pa("link_tooltip", &["tooltip"], "说明", "超链接提示"),
    p("comment", "备注", "单元格批注"),
    pa("bold", &["font.bold"], "true", "true|false"),
    pa("italic", &["font.italic"], "true", "true|false"),
    pa("strike", &["font.strike"], "true", "true|false"),
    pa(
        "underline",
        &["font.underline"],
        "single",
        "single|double|none",
    ),
    pa("font", &["font.name", "fontname"], "Arial", "字体名"),
    pa("size", &["font.size", "fontsize"], "11", "字号（pt）"),
    pa("color", &["font.color"], "FF0000", "字体色（不带 #）"),
    pa("fill", &["bgcolor"], "FFF2CC", "填充色（不带 #）"),
    pa(
        "halign",
        &["alignment.horizontal"],
        "center",
        "left|center|right",
    ),
    pa(
        "valign",
        &["alignment.vertical"],
        "center",
        "top|center|bottom",
    ),
    pa(
        "wrap",
        &["wrapText", "alignment.wrap_text"],
        "true",
        "自动换行 true|false",
    ),
    pa("border", &["border.all"], "thin", "全边框线型"),
    p("border.color", "999999", "边框色"),
    p("border.top", "thin", "上边框"),
    p("border.bottom", "thin", "下边框"),
    p("border.left", "thin", "左边框"),
    p("border.right", "thin", "右边框"),
    p("locked", "true", "锁定单元格（配合保护）"),
];

/// 工作表（`/sheet[N]` 或 `/sheet[名字]`）
pub const SHEET_PROPS: &[Prop] = &[
    p("name", "Data", "工作表名"),
    p("tab_color", "0070C0", "标签颜色（不带 #）"),
    p("hidden", "true", "true|false"),
    p("freeze", "A2", "冻结窗格起始单元格"),
    p("direction", "rtl", "ltr|rtl"),
    p("auto_filter", "A1:D100", "自动筛选区域"),
    p("default_col_width", "10", "默认列宽（字符）"),
    p("default_row_height", "15", "默认行高（pt）"),
    p("base_col_width", "10", "基准列宽（字符）"),
    p("gridlines", "false", "网格线显示 true|false"),
    p("headings", "false", "行号列标显示 true|false"),
    p("zoom", "120", "缩放百分比"),
];

/// 行（`/sheet[N]/row[N]`）
pub const ROW_PROPS: &[Prop] = &[p("height", "22", "行高（pt）")];

/// 列（`/sheet[N]/col[C]`）
pub const COL_PROPS: &[Prop] = &[
    p("width", "14", "列宽（字符）"),
    p("hidden", "true", "true|false"),
];

/// 区域（`/sheet[N]/range[A1:C10]`）
pub const RANGE_PROPS: &[Prop] = &[p(
    "merge",
    "true",
    "true 合并 / false 取消（与单元格样式属性同用时先合并）",
)];

/// add 支持的元素类型
pub const ADD_TYPES: &[(&str, &str)] = &[
    ("sheet", "工作表（--prop name=X）"),
    ("row", "行（--prop index=N，缺省追加）"),
    ("cell", "单元格（--prop ref=A1 / value / formula ...）"),
    ("chart", "图表（--prop chart_type=column data_range=A1:B9）"),
    ("image", "图片（--prop src=路径）"),
    ("table", "表格（--prop range=A1:D10，缺省自动识别数据区）"),
    ("shape", "形状（--prop preset=roundRect / text / anchor）"),
];

/// view 模式
pub const VIEW_MODES: &[(&str, &str)] = &[
    ("text", "值/公式视图（旧值 values 兼容）"),
    ("layout", "结构树视图（旧值 structure 兼容）"),
];

/// batch 支持的操作
pub const BATCH_OPS: &[(&str, &str)] = &[
    ("set", "修改属性（path + props）"),
    ("add", "新增元素（path + type + props）"),
    ("remove", "删除元素（path）"),
    ("get", "读取节点 JSON（path）"),
    ("view", "只读视图（path + raw 中可选 mode=text|layout）"),
];

/// 所有元素能力（help 渲染与契约测试用）
pub struct ElementCaps {
    pub element: &'static str,
    pub path: &'static str,
    pub props: &'static [Prop],
    pub note: &'static str,
}

pub const ELEMENTS: &[ElementCaps] = &[
    ElementCaps {
        element: "sheet",
        path: "/sheet[1]（或 /sheet[名字]）",
        props: SHEET_PROPS,
        note: "工作表级属性；set 直接作用于该表",
    },
    ElementCaps {
        element: "cell",
        path: "/sheet[1]/cell[B2]",
        props: CELL_PROPS,
        note: "样式属性写入 cell.style（内联样式）",
    },
    ElementCaps {
        element: "range",
        path: "/sheet[1]/range[A1:C10]",
        props: RANGE_PROPS,
        note: "区域广播：样式属性应用到区域内每个单元格",
    },
    ElementCaps {
        element: "row",
        path: "/sheet[1]/row[2]",
        props: ROW_PROPS,
        note: "行属性",
    },
    ElementCaps {
        element: "col",
        path: "/sheet[1]/col[C]",
        props: COL_PROPS,
        note: "列属性",
    },
];

/// 汇总某元素键的全部可用属性名（规范名 + 别名），供纠错建议
pub fn prop_names(props: &[Prop]) -> Vec<&'static str> {
    let mut out = Vec::new();
    for p in props {
        out.push(p.name);
        out.extend_from_slice(p.aliases);
    }
    out
}
