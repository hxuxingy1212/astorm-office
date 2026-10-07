//! 能力表：pdf CLI 支持的元素类型与属性面（单一来源）。
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
    /// 接受该属性的元素类型；None = 任意元素
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

/// 元素级可 set 的属性（edit apply_element_prop 的声明面）。
/// 注意：PDF 产物目录颜色带 `#`（与 Office 三格式相反）。
pub const ELEMENT_PROPS: &[Prop] = &[
    po(
        "x",
        "72",
        "X（pt，左下角/基线）",
        &["text", "rect", "pattern_rect", "image"],
    ),
    po(
        "y",
        "720",
        "Y（pt，y 向上；文本为基线）",
        &["text", "rect", "pattern_rect", "image"],
    ),
    po("w", "200", "宽（pt）", &["rect", "pattern_rect", "image"]),
    po("h", "40", "高（pt）", &["rect", "pattern_rect", "image"]),
    po(
        "rotation",
        "45",
        "逆时针旋转（度，绕左下角）",
        &["text", "rect", "image"],
    ),
    po(
        "alpha",
        "0.5",
        "不透明度 0-1",
        &["text", "rect", "pattern_rect", "path", "image", "group"],
    ),
    po("text", "新文本", "文字内容（\\n 强制换行）", &["text"]),
    po("size", "12", "字号（pt）", &["text"]),
    po("font", "Helvetica", "标准 14 或 system:<family>", &["text"]),
    po("color", "#23456e", "文字色（#RRGGBB，带 #）", &["text"]),
    po("bold", "true", "加粗（标准 14 映射变体名）", &["text"]),
    po("italic", "true", "斜体", &["text"]),
    po(
        "fill",
        "#ffffff",
        "填充色（带 #；none = 不填充）",
        &["rect", "path"],
    ),
    po(
        "stroke",
        "#000000",
        "描边色（带 #；none = 不描边）",
        &["rect", "path", "polyline"],
    ),
    po(
        "line_width",
        "1",
        "描边线宽（pt）",
        &["rect", "path", "polyline"],
    ),
    po("close", "true", "闭合折线", &["polyline"]),
    po(
        "points",
        "50,600 150,650",
        "折线顶点（x,y 空格分隔对）",
        &["polyline"],
    ),
    po(
        "src",
        "media/img-001.png",
        "图片路径（产物根相对）",
        &["image"],
    ),
];

/// 页面级可 set 的属性
pub const PAGE_PROPS: &[Prop] = &[
    p("width", "595.28", "页宽（pt），缺省用顶层 page_size"),
    p("height", "841.89", "页高（pt），缺省用顶层 page_size"),
    p("rotation", "90", "顺时针旋转：0/90/180/270"),
    p("crop", "[50,60,545,780]", "裁剪框 [x0,y0,x1,y1]（pt）"),
];

/// add 支持的元素类型
pub const ADD_TYPES: &[(&str, &str)] = &[
    (
        "page",
        "页面（path 用 /page，文档容器；--prop width= height= rotation=）",
    ),
    ("text", "文本（--prop text=你好 size=14 color=#23456e）"),
    ("rect", "矩形（--prop x=50 y=700 w=200 h=40 fill=#ffffff）"),
    (
        "polyline",
        "折线（--prop points=50,600 150,650 stroke=#000000）",
    ),
    ("image", "图片（--prop src=media/img.png x= y= w= h=）"),
    ("group", "透明组（后续 add 以 /page[N]/group[M] 为容器）"),
    ("path", "路径（仅 batch 整元素回放；交互 add 不支持）"),
    ("shading", "渐变（仅 batch 整元素回放；交互 add 不支持）"),
    (
        "pattern_rect",
        "平铺图案（仅 batch 整元素回放；交互 add 不支持）",
    ),
];

/// view 模式
pub const VIEW_MODES: &[(&str, &str)] = &[
    ("text", "文本 + 媒体（去图形，含各元素 CLI 路径）"),
    ("layout", "元素布局树（类型/几何/文字摘要/CLI 路径）"),
];

/// batch 支持的 op
pub const BATCH_OPS: &[(&str, &str)] = &[
    ("set", "修改属性（path + props；page 或元素级）"),
    (
        "add",
        "新增：type=page 追加整页（dump 回放按页面 id 幂等替换）；其余类型向容器追加元素",
    ),
    ("remove", "删除元素或页面（path）"),
    ("get", "读取节点 JSON（path）"),
    ("view", "只读视图（path；props.mode=text|layout）"),
];

/// 按元素类型给出可 set 的属性名（含 page 级），供纠错建议。
pub fn prop_names_for(type_name: &str) -> Vec<&'static str> {
    prop_names(if type_name == "page" {
        PAGE_PROPS
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
        element: "page",
        path: "/page[1]",
        note: "页级属性（width/height/rotation/crop）；add 以 /page 为容器追加页面或元素",
    },
    ElementCaps {
        element: "text",
        path: "/page[1]/text[1]",
        note: "文本（size/font/color；y 为基线；标准 14 零嵌入）",
    },
    ElementCaps {
        element: "rect",
        path: "/page[1]/rect[1]",
        note: "矩形（fill/stroke 置 none 表示不填充/不描边）",
    },
    ElementCaps {
        element: "image",
        path: "/page[1]/image[1]",
        note: "图片（src 为产物根相对路径；png/jpg 原生）",
    },
    ElementCaps {
        element: "polyline",
        path: "/page[1]/polyline[1]",
        note: "折线（points 顶点对）",
    },
    ElementCaps {
        element: "path",
        path: "/page[1]/path[1]",
        note: "贝塞尔路径（segments 原样保留；编辑建议直接改分片 JSON）",
    },
    ElementCaps {
        element: "shading",
        path: "/page[1]/shading[1]",
        note: "渐变着色（axial/radial + stops；编辑建议直接改分片 JSON）",
    },
    ElementCaps {
        element: "pattern_rect",
        path: "/page[1]/pattern_rect[1]",
        note: "平铺图案填充矩形（编辑建议直接改分片 JSON）",
    },
    ElementCaps {
        element: "group",
        path: "/page[1]/group[1]",
        note: "透明组；add 可继续下钻到 group 内",
    },
];

/// 汇总属性名（规范名）
pub fn prop_names(props: &[Prop]) -> Vec<&'static str> {
    props.iter().map(|pr| pr.name).collect()
}
