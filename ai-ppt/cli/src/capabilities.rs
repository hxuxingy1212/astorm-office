//! 能力表：ppt CLI 支持的元素类型与属性面（单一来源）。
//!
//! 消费方：
//! 1. `help` 命令输出（人读 / `--json` 机器可读）；
//! 2. 未知属性/类型错误时的 `suggestion`（最近匹配 + 合法取值）；
//! 3. 契约测试 `tests/capabilities.rs`：逐条验证声明与实现一致
//!    （元素声明可用的属性必须 set 成功；声明不适用必须失败）。

/// 单个属性声明
pub struct Prop {
    /// 规范名
    pub name: &'static str,
    /// 示例值（契约测试样值）
    pub sample: &'static str,
    /// 取值说明
    pub note: &'static str,
    /// 接受该属性的元素类型；None = 任意元素（位置/旋转/稳定 ID 等通用属性）
    pub only: Option<&'static [&'static str]>,
}

const fn p(name: &'static str, sample: &'static str, note: &'static str) -> Prop {
    Prop {
        name,
        sample,
        note,
        only: None,
    }
}

const fn po(
    name: &'static str,
    sample: &'static str,
    note: &'static str,
    only: &'static [&'static str],
) -> Prop {
    Prop {
        name,
        sample,
        note,
        only: Some(only),
    }
}

/// 元素级可 set 的属性（apply_element_prop 的声明面）
pub const ELEMENT_PROPS: &[Prop] = &[
    p("name", "标题", "元素名（选择窗格 / @name= 寻址）"),
    p("id", "42", "稳定 ID；@id= 寻址用，缺省自动分配"),
    p("x", "1.5", "X（英寸）"),
    p("y", "2.0", "Y（英寸）"),
    p("w", "4.0", "宽（英寸）"),
    p("h", "2.0", "高（英寸）"),
    po(
        "rotation",
        "45",
        "旋转角度（度）",
        &["shape", "line", "image", "group"],
    ),
    po("text", "标题文字", "文字内容", &["text", "shape"]),
    po("font_size", "18", "字号（磅）", &["text", "shape"]),
    po("bold", "true", "加粗", &["text"]),
    po(
        "color",
        "1F4E79",
        "文字色（不带 #）",
        &["text", "shape", "line"],
    ),
    po(
        "align",
        "center",
        "left|center|right|justify",
        &["text", "shape"],
    ),
    po("fill", "2E74B5", "填充色（不带 #）", &["text", "shape"]),
    po("shape_type", "roundRect", "形状预设", &["shape"]),
    po("src", "img/logo.png", "图片路径", &["image"]),
    po("width", "1.5", "线宽（磅）", &["line"]),
    po("dash", "dash", "线型", &["line"]),
    po("arrow_start", "triangle", "起点箭头", &["line"]),
    po("arrow_end", "triangle", "终点箭头", &["line"]),
    po("points", "0,0 1,1", "顶点列表（x,y 空格分隔对）", &["line"]),
    po("smooth", "true", "平滑曲线", &["line", "line_chart"]),
    po(
        "data",
        "10,20,30",
        "数据（逗号分隔）",
        &["line_chart", "pie_chart", "ring_chart"],
    ),
    po(
        "labels",
        "Q1,Q2,Q3",
        "类目标签",
        &["line_chart", "pie_chart", "ring_chart"],
    ),
    po(
        "colors",
        "2E74B5,ED7D31",
        "系列色",
        &["line_chart", "pie_chart", "ring_chart"],
    ),
    po("max", "100", "数值轴上限", &["line_chart"]),
    po("thickness", "0.3", "环宽比例", &["ring_chart"]),
    po(
        "show_values",
        "true",
        "显示数值",
        &["line_chart", "pie_chart", "ring_chart"],
    ),
    po("axis", "true", "显示坐标轴", &["line_chart"]),
];

/// 幻灯片级属性
pub const SLIDE_PROPS: &[Prop] = &[
    p(
        "background",
        "F5F5F5",
        "纯色十六进制（带不带 # 均可）或渐变/图片 JSON",
    ),
    p("transition", r#"{"type":"fade"}"#, "过渡动画 JSON"),
    p("notes", "演讲备注", "演讲者备注"),
];

/// add 支持的元素类型
pub const ADD_TYPES: &[(&str, &str)] = &[
    ("text", "文本（--prop text=你好 font_size=18）"),
    (
        "shape",
        "形状（--prop shape_type=rect fill=2E74B5 text=说明）",
    ),
    ("line", "连接线（--prop points=0,0 2,1 color=999999）"),
    ("image", "图片（--prop src=img/logo.png）"),
    ("group", "分组（后续 add 以 /slide[N]/group[M] 为容器）"),
    ("line_chart", "折线图（--prop data=1,2,3 labels=a,b,c）"),
    ("pie_chart", "饼图（--prop data=1,2,3 labels=a,b,c）"),
    ("ring_chart", "环形图（--prop data=1,2,3 thickness=0.3）"),
    ("progress_bar", "进度条（--prop value=60 label=完成度）"),
];

/// view 模式
pub const VIEW_MODES: &[(&str, &str)] = &[
    ("text", "文本+媒体（去样式）"),
    ("layout", "组件布局树（类型/名称/文字/位置）"),
];

/// batch 支持的 op
pub const BATCH_OPS: &[(&str, &str)] = &[
    ("set", "修改属性（path + props）"),
    ("add", "新增元素（path + type + props）"),
    ("remove", "删除元素（path）"),
    ("get", "读取节点 JSON（path）"),
    ("view", "只读视图（path；raw.mode=text|layout）"),
];

/// 按元素类型给出可 set 的属性名（含 slide 级），供纠错建议。
pub fn prop_names_for(type_name: &str) -> Vec<&'static str> {
    prop_names(if type_name == "slide" {
        SLIDE_PROPS
    } else {
        ELEMENT_PROPS
    })
}

/// 该属性是否适用于指定元素类型
pub fn accepts(prop: &Prop, type_name: &str) -> bool {
    match prop.only {
        None => true,
        Some(list) => list.contains(&type_name),
    }
}

/// 元素条目（help 渲染用）
pub struct ElementCaps {
    pub element: &'static str,
    pub path: &'static str,
    pub note: &'static str,
}

pub const ELEMENTS: &[ElementCaps] = &[
    ElementCaps {
        element: "slide",
        path: "/slide[1]",
        note: "幻灯片级属性；add 以 /slide[N] 为容器追加元素",
    },
    ElementCaps {
        element: "text",
        path: "/slide[1]/text[1]",
        note: "文本元素",
    },
    ElementCaps {
        element: "shape",
        path: "/slide[1]/shape[1]",
        note: "自动形状（rect/ellipse/roundRect…）",
    },
    ElementCaps {
        element: "line",
        path: "/slide[1]/line[1]",
        note: "连接线/自由曲线",
    },
    ElementCaps {
        element: "image",
        path: "/slide[1]/image[1]",
        note: "图片（src 支持本地路径/URL）",
    },
    ElementCaps {
        element: "table",
        path: "/slide[1]/table[1]",
        note: "表格（文字在 rows[r][c].text）",
    },
    ElementCaps {
        element: "chart",
        path: "/slide[1]/chart[1]",
        note: "真实图表（chart XML 原样保留）",
    },
    ElementCaps {
        element: "group",
        path: "/slide[1]/group[1]",
        note: "分组；add 可继续下钻到 group 内",
    },
    ElementCaps {
        element: "line_chart",
        path: "/slide[1]/line_chart[1]",
        note: "折线图组件（数据在 --prop data）",
    },
    ElementCaps {
        element: "pie_chart",
        path: "/slide[1]/pie_chart[1]",
        note: "饼图组件",
    },
    ElementCaps {
        element: "ring_chart",
        path: "/slide[1]/ring_chart[1]",
        note: "环形图组件",
    },
    ElementCaps {
        element: "progress_bar",
        path: "/slide[1]/progress_bar[1]",
        note: "进度条组件",
    },
];

/// 汇总属性名（规范名）
pub fn prop_names(props: &[Prop]) -> Vec<&'static str> {
    props.iter().map(|pr| pr.name).collect()
}
