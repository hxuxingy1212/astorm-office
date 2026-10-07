//! 能力表：world CLI 支持的块类型与属性面（单一来源）。
//!
//! 消费方：
//! 1. `help` 命令输出（人读 / `--json` 机器可读）；
//! 2. 未知属性/类型错误时的 `suggestion`（最近匹配 + 合法取值）；
//! 3. 契约测试 `tests/capabilities.rs`：逐条验证声明与实现一致。

/// 单个属性声明
pub struct Prop {
    pub name: &'static str,
    pub sample: &'static str,
    pub note: &'static str,
}

const fn p(name: &'static str, sample: &'static str, note: &'static str) -> Prop {
    Prop { name, sample, note }
}

/// 段落/标题可 set 的属性（set_block 的声明面）
pub const PARAGRAPH_PROPS: &[Prop] = &[
    p("text", "正文内容", "纯文本（覆盖 runs）"),
    p("align", "center", "left|center|right|justify"),
    p("style", "Quote", "命名样式 ID"),
    p("bold", "true", "整段加粗"),
    p("italic", "true", "整段斜体"),
    p("font_size", "12", "字号（磅）"),
    p("color", "333333", "文字色（不带 #）"),
    p("font_family", "SimSun", "字体"),
    p("first_line_indent", "24", "首行缩进（磅）"),
    p("indent", "24", "左缩进（磅）"),
    p("hanging_indent", "24", "悬挂缩进（磅）"),
    p("line_spacing", "1.5", "行距倍数（或 exact/atLeast 描述）"),
    p("space_before", "6", "段前（磅）"),
    p("space_after", "6", "段后（磅）"),
    p("keep_next", "true", "与下段同页"),
    p("keep_lines", "true", "段中不分页"),
    p("page_break_before", "true", "段前分页"),
    p("shading", "F2F2F2", "底纹色"),
    p("border", "single", "段落边框"),
    p("tabs", "right,9026", "制表位（逗号分隔 位置列表）"),
];

/// 分片（part）级属性
pub const PART_PROPS: &[Prop] = &[
    p("header", "页眉文字", "页眉（空串清除）"),
    p("footer_page_number", "true", "页脚页码"),
    p("page_size", "A4", "页面尺寸（A4/Letter…）"),
    p("orientation", "landscape", "portrait|landscape"),
    p("footer_format", "第 {page} 页", "页脚格式模板"),
];

/// add 支持的块类型
pub const ADD_TYPES: &[(&str, &str)] = &[
    ("heading", "标题（--prop text=标题 level=1 numbering=true）"),
    ("paragraph", "段落（--prop text=正文 align=center）"),
    ("list", "列表（--prop text=项 ordered=true）"),
    ("table", "表格（--prop data=[[a,b],[1,2]] header_row=true）"),
    ("image", "图片（--prop src=img.png width=200 caption=图1）"),
    ("formula", "公式（--prop latex=E=mc^2 display=true）"),
    ("code", "代码（--prop text=println! lang=rust）"),
    ("quote", "引用（--prop text=名言）"),
    ("caption", "题注（--prop text=图 1 of=image）"),
    ("toc", "目录（--prop levels=1,2,3 title=目录）"),
    ("bibliography", "参考文献（--prop data=[...]）"),
    ("page_break", "分页符"),
    ("shape", "形状（--prop shape_type=rect text=说明）"),
    ("textbox", "文本框（--prop text=内容）"),
    ("chart", "图表（--prop chart_type=column data=[...]）"),
    (
        "attachment",
        "附件/OLE（--prop src=file.xlsx prog_id=Excel.Sheet）",
    ),
];

/// view 模式
pub const VIEW_MODES: &[(&str, &str)] = &[
    ("text", "纯文本视图（含分片与块结构提示）"),
    ("layout", "块布局树（类型/级别/文字摘要）"),
];

/// batch 支持的 op
pub const BATCH_OPS: &[(&str, &str)] = &[
    ("set", "修改属性（path + props）"),
    ("add", "新增块（path + type + props）"),
    ("remove", "删除块（path）"),
    ("get", "读取节点 JSON（path）"),
    ("view", "只读视图（path；raw.mode=text|layout）"),
];

/// 元素条目（help 渲染用）
pub struct ElementCaps {
    pub element: &'static str,
    pub path: &'static str,
    pub note: &'static str,
}

pub const ELEMENTS: &[ElementCaps] = &[
    ElementCaps {
        element: "part",
        path: "/part[1]（别名 chapter[1]）",
        note: "分片级属性；add 以 /part[N] 为容器追加块",
    },
    ElementCaps {
        element: "heading",
        path: "/part[1]/heading[1]",
        note: "标题（level 1~9；@paraId 不适用）",
    },
    ElementCaps {
        element: "paragraph",
        path: "/part[1]/paragraph[1]",
        note: "段落；支持 @paraId= 稳定寻址（unpack 自原始 docx）",
    },
    ElementCaps {
        element: "list",
        path: "/part[1]/list[1]/item[1]/paragraph[1]",
        note: "列表（item 内段落可寻址）",
    },
    ElementCaps {
        element: "table",
        path: "/part[1]/table[1]/row[1]/cell[1]/paragraph[1]",
        note: "表格（行/列越界有范围提示）",
    },
    ElementCaps {
        element: "image",
        path: "/part[1]/image[1]",
        note: "图片块（行内图片在 run 上）",
    },
    ElementCaps {
        element: "formula",
        path: "/part[1]/formula[1]",
        note: "LaTeX 公式（生成端转 OMML）",
    },
    ElementCaps {
        element: "code",
        path: "/part[1]/code[1]",
        note: "代码块（lang 高亮语言）",
    },
    ElementCaps {
        element: "quote",
        path: "/part[1]/quote[1]",
        note: "引用块",
    },
    ElementCaps {
        element: "caption",
        path: "/part[1]/caption[1]",
        note: "题注（of 指向所属元素）",
    },
    ElementCaps {
        element: "toc",
        path: "/part[1]/toc[1]",
        note: "目录（live 域或静态条目）",
    },
    ElementCaps {
        element: "bibliography",
        path: "/part[1]/bibliography[1]",
        note: "参考文献（GB/T 7714 风格输出）",
    },
];

/// 块级扩展属性（image/formula/code/table/shape/chart 等专用，set 按块类型取用）
pub const EXTRA_BLOCK_PROPS: &[&str] = &[
    "level",
    "numbering",
    "ordered",
    "src",
    "width",
    "height",
    "alt",
    "caption",
    "wrap",
    "x",
    "y",
    "behind_text",
    "latex",
    "display",
    "number",
    "lang",
    "of",
    "levels",
    "static",
    "entries",
    "data",
    "header_row",
    "cols",
    "shape_type",
    "fill",
    "line",
    "line_width",
    "rotation",
    "text_color",
    "icon",
    "chart_type",
    "categories",
    "series_name",
    "label",
    "prog_id",
    "vml",
    "title",
    "name",
];

/// 未知属性纠错用：块级（或分片级）全部属性名
pub fn prop_names_for(type_name: &str) -> Vec<&'static str> {
    if type_name == "part" {
        return prop_names(PART_PROPS);
    }
    let mut out = prop_names(PARAGRAPH_PROPS);
    out.extend_from_slice(EXTRA_BLOCK_PROPS);
    out
}

/// 汇总属性名
pub fn prop_names(props: &[Prop]) -> Vec<&'static str> {
    props.iter().map(|pr| pr.name).collect()
}
