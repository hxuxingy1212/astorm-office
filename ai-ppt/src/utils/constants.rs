//! 常量定义和单位换算工具
//!
//! OOXML 使用 EMU (English Metric Unit) 作为长度单位。
//! 1 英寸 = 914400 EMU，1 磅 = 12700 EMU。

/// 每英寸的 EMU 值（OOXML 标准单位）
pub const EMU_PER_INCH: i64 = 914400;

/// 英寸转 EMU
pub fn inch_to_emu(inches: f64) -> i64 {
    (inches * EMU_PER_INCH as f64).round() as i64
}
/// EMU 转英寸
pub fn emu_to_inch(emu: i64) -> f64 {
    (emu as f64 / EMU_PER_INCH as f64 * 1000.0).round() / 1000.0
}
/// 磅转百分之一磅（OOXML 字体大小单位）
pub fn pt_to_hundredths(pt: f64) -> i64 {
    (pt * 100.0).round() as i64
}

/// OOXML 命名空间常量
pub mod ns {
    pub const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    pub const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    pub const P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
    pub const REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
    pub const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
}

pub mod rel_types {
    pub const OFFICE_DOCUMENT: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
    pub const SLIDE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide";
    pub const SLIDE_MASTER: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster";
    pub const SLIDE_LAYOUT: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout";
    pub const THEME: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
    pub const IMAGE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
    pub const HYPERLINK: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
    pub const CORE_PROPS: &str =
        "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
    pub const EXT_PROPS: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
    pub const PRES_PROPS: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/presProps";
    pub const VIEW_PROPS: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/viewProps";
    pub const TABLE_STYLES: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/tableStyles";
    pub const NOTES_SLIDE: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide";
    pub const NOTES_MASTER: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesMaster";
    pub const CHART: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
    pub const COMMENT: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
    pub const COMMENT_AUTHORS: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors";
    pub const VIDEO: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/video";
    pub const AUDIO: &str =
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/audio";
}

pub mod content_types {
    pub const PRESENTATION: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml";
    pub const SLIDE: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.slide+xml";
    pub const SLIDE_MASTER: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml";
    pub const SLIDE_LAYOUT: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml";
    pub const THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
    pub const PRES_PROPS: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.presProps+xml";
    pub const VIEW_PROPS: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml";
    pub const TABLE_STYLES: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml";
    pub const CORE_PROPS: &str = "application/vnd.openxmlformats-package.core-properties+xml";
    pub const EXT_PROPS: &str =
        "application/vnd.openxmlformats-officedocument.extended-properties+xml";
    pub const NOTES_SLIDE: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml";
    pub const NOTES_MASTER: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml";
    pub const CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
    pub const COMMENT: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.comments+xml";
    pub const COMMENT_AUTHORS: &str =
        "application/vnd.openxmlformats-officedocument.presentationml.commentAuthors+xml";
}

pub const SHAPE_TYPES: &[(&str, &str)] = &[
    // 基础形状
    ("rect", "rect"),
    ("roundRect", "roundRect"),
    ("ellipse", "ellipse"),
    ("triangle", "triangle"),
    ("diamond", "diamond"),
    ("pentagon", "pentagon"),
    ("hexagon", "hexagon"),
    ("octagon", "octagon"),
    ("parallelogram", "parallelogram"),
    ("trapezoid", "trapezoid"),
    ("isocelesTriangle", "isocelesTriangle"),
    ("rightTriangle", "rightTriangle"),
    ("regularPentagon", "regularPentagon"),
    ("teardrop", "teardrop"),
    ("foldedCorner", "foldedCorner"),
    ("homePlate", "homePlate"),
    ("plus", "plus"),
    ("cross", "cross"),
    ("noSmoking", "noSmoking"),
    ("sun", "sun"),
    ("moon", "moon"),
    ("smileyFace", "smileyFace"),
    ("frame", "frame"),
    ("can", "can"),
    ("funnel", "funnel"),
    ("gear6", "gear6"),
    ("gear9", "gear9"),
    // 括号与弧
    ("bracketPair", "bracketPair"),
    ("leftBrace", "leftBrace"),
    ("rightBrace", "rightBrace"),
    ("leftBracket", "leftBracket"),
    ("rightBracket", "rightBracket"),
    ("chord", "chord"),
    ("pie", "pie"),
    ("circularArrow", "circularArrow"),
    // 星形
    ("star5", "star5"),
    ("star6", "star6"),
    ("star8", "star8"),
    ("star10", "star10"),
    ("star16", "star16"),
    ("star24", "star24"),
    // 箭头
    ("rightArrow", "rightArrow"),
    ("leftArrow", "leftArrow"),
    ("upArrow", "upArrow"),
    ("downArrow", "downArrow"),
    ("chevron", "chevron"),
    ("leftRightArrow", "leftRightArrow"),
    ("upDownArrow", "upDownArrow"),
    ("leftUpArrow", "leftUpArrow"),
    ("bentArrow", "bentArrow"),
    ("curvedUpArrow", "curvedUpArrow"),
    ("curvedDownArrow", "curvedDownArrow"),
    ("curvedLeftArrow", "curvedLeftArrow"),
    ("curvedRightArrow", "curvedRightArrow"),
    ("quadArrow", "quadArrow"),
    ("notchedRightArrow", "notchedRightArrow"),
    ("notchedLeftArrow", "notchedLeftArrow"),
    ("pentagonChevron", "pentagonChevron"),
    // 流程符号
    ("flowChartProcess", "flowChartProcess"),
    ("flowChartDecision", "flowChartDecision"),
    ("flowChartTerminator", "flowChartTerminator"),
    ("flowChartInputOutput", "flowChartInputOutput"),
    ("flowChartDocument", "flowChartDocument"),
    ("flowChartPredefinedProcess", "flowChartPredefinedProcess"),
    ("flowChartData", "flowChartData"),
    ("flowChartManualInput", "flowChartManualInput"),
    ("flowChartPreparation", "flowChartPreparation"),
    ("flowChartConnector", "flowChartConnector"),
    ("flowChartMerge", "flowChartMerge"),
    ("flowChartOr", "flowChartOr"),
    // 其他
    ("donut", "donut"),
    ("blockArc", "blockArc"),
    ("line", "line"),
    ("cloud", "cloud"),
    ("heart", "heart"),
    ("lightningBolt", "lightningBolt"),
    ("ellipseRibbon", "ellipseRibbon"),
    ("ribbon", "ribbon"),
    // 标注（JSON 名 → OOXML prst 名，别名映射）
    ("callout1", "wedgeRoundRectCallout"),
    ("callout2", "wedgeEllipseCallout"),
    ("cloudCallout", "cloudCallout"),
    ("roundRectCallout", "roundRectCallout"),
    ("borderCallout1", "borderCallout1"),
    ("borderCallout2", "borderCallout2"),
    ("wedgeRectCallout", "wedgeRectCallout"),
    ("wedgeOvalCallout", "wedgeOvalCallout"),
];

pub fn shape_ooxml_name(name: &str) -> &str {
    for (k, v) in SHAPE_TYPES {
        if *k == name {
            return v;
        }
    }
    name
}

/// OOXML prstGeom 名反向查询 JSON 形状名（生成/解析共用，查不到原样返回）
pub fn shape_json_name(ooxml: &str) -> &str {
    for (k, v) in SHAPE_TYPES {
        if *v == ooxml {
            return k;
        }
    }
    ooxml
}

/// 线条虚线样式映射：(JSON 名, OOXML prstDash 值)
pub const DASH_STYLES: &[(&str, &str)] =
    &[("solid", "solid"), ("dashed", "dash"), ("dotted", "dot")];

pub fn dash_ooxml_name(name: &str) -> &str {
    DASH_STYLES
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
        .unwrap_or(name)
}

/// OOXML prstDash 值反向查询 JSON 虚线名（查不到原样返回）
pub fn dash_json_name(ooxml: &str) -> &str {
    DASH_STYLES
        .iter()
        .find(|(_, v)| *v == ooxml)
        .map(|(k, _)| *k)
        .unwrap_or(ooxml)
}

/// 线条端点样式映射：(JSON 名, OOXML lineEndType 值)
pub const ARROW_ENDS: &[(&str, &str)] = &[("arrow", "arrow"), ("dot", "oval")];

/// JSON 端点名 → OOXML lineEndType 值（"none" 或未知返回 None，省略端点标签）
pub fn arrow_end_ooxml_name(name: &str) -> Option<&str> {
    ARROW_ENDS.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}

/// OOXML lineEndType 值反向查询 JSON 端点名（oval → dot；未知返回原样）
pub fn arrow_end_json_name(ooxml: &str) -> &str {
    ARROW_ENDS
        .iter()
        .find(|(_, v)| *v == ooxml)
        .map(|(k, _)| *k)
        .unwrap_or(ooxml)
}

pub const FLY_DIRECTIONS: &[(&str, u32)] = &[
    ("top", 1),
    ("right", 2),
    ("bottom", 4),
    ("left", 8),
    ("topLeft", 9),
    ("topRight", 3),
    ("bottomLeft", 12),
    ("bottomRight", 6),
];

pub fn fly_direction_value(dir: &str) -> u32 {
    FLY_DIRECTIONS
        .iter()
        .find(|(k, _)| *k == dir)
        .map(|(_, v)| *v)
        .unwrap_or(4)
}

/// 方向数值反向查询（OOXML presetSubtype → JSON 方向名）
pub fn fly_direction_from_value(value: u32) -> Option<&'static str> {
    FLY_DIRECTIONS
        .iter()
        .find(|(_, v)| *v == value)
        .map(|(k, _)| *k)
}

/// 动画预设表：(类型, preset_id, preset_class, filter)
/// generate 与 parse 两侧共用，避免两份互为逆映射的表漂移
pub const ANIMATION_PRESETS: &[(&str, u32, &str, Option<&str>)] = &[
    ("appear", 1, "entr", None),
    ("fadeIn", 2, "entr", None),
    ("flyIn", 3, "entr", None),
    ("wipeIn", 4, "entr", None),
    ("zoomIn", 5, "entr", None),
    ("bounceIn", 6, "entr", None),
    ("floatIn", 7, "entr", None),
    ("swivel", 8, "entr", None),
    ("dissolveIn", 9, "entr", Some("dissolve")),
    ("splitIn", 10, "entr", None),
    ("fadeOut", 2, "exit", None),
    ("flyOut", 3, "exit", None),
    ("wipeOut", 4, "exit", None),
    ("zoomOut", 5, "exit", None),
    ("floatOut", 7, "exit", None),
    ("dissolveOut", 9, "exit", Some("dissolve")),
    ("pulse", 11, "emph", None),
    ("spin", 12, "emph", None),
    ("growShrink", 13, "emph", None),
    ("colorChange", 14, "emph", None),
    ("transparency", 15, "emph", None),
    ("teeter", 16, "emph", None),
    ("blink", 17, "emph", None),
    ("motionPath", 18, "path", None),
];

/// 按动画类型查预设 (preset_id, preset_class, filter)
pub fn animation_preset(anim_type: &str) -> Option<(u32, &'static str, Option<&'static str>)> {
    ANIMATION_PRESETS
        .iter()
        .find(|(t, _, _, _)| *t == anim_type)
        .map(|(_, id, class, filter)| (*id, *class, *filter))
}

/// 按 preset_id 查动画类型（取第一个匹配，与 parse 侧原行为一致）
pub fn animation_type_from_preset_id(preset_id: u32) -> Option<&'static str> {
    ANIMATION_PRESETS
        .iter()
        .find(|(_, id, _, _)| *id == preset_id)
        .map(|(t, _, _, _)| *t)
}

pub const TRANSITIONS: &[(&str, &str)] = &[
    ("fade", "<p:fade/>"),
    ("push", "<p:push dir=\"r\"/>"),
    ("wipe", "<p:wipe dir=\"d\"/>"),
    ("split", "<p:split orient=\"horz\" dir=\"out\"/>"),
    ("cover", "<p:cover dir=\"r\"/>"),
    ("cut", "<p:cut/>"),
    ("dissolve", "<p:dissolve/>"),
    ("random", "<p:random/>"),
    ("morph", "<p14:morph option=\"byObject\"/>"),
];

pub fn transition_ooxml(name: &str) -> &str {
    TRANSITIONS
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
        .unwrap_or("<p:fade/>")
}

pub const DEFAULT_SLIDE_WIDTH: f64 = 13.333;
pub const DEFAULT_SLIDE_HEIGHT: f64 = 7.5;
