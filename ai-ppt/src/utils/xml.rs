//! XML 辅助工具模块
//!
//! 提供 OOXML 常用的 XML 片段生成函数，包括颜色、填充、变换、文字属性等。

use crate::utils::constants::{inch_to_emu, pt_to_hundredths};

/// 取元素标签的局部名（忽略命名空间前缀）
///
/// OOXML 中同一元素可能使用不同的前缀（`a:` / `p:` 等由文件声明决定），
/// 解析时应只比较局部名，避免对前缀做硬编码匹配。
pub fn tag_local(e: &quick_xml::events::BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).to_string()
}

/// 从原始限定名（含前缀）中取局部名，开始/结束标签通用
pub fn raw_local_name(raw: &[u8]) -> String {
    let local = match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    };
    String::from_utf8_lossy(local).to_string()
}

/// XML 转义：将特殊字符替换为实体引用
pub fn esc_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// 生成颜色 XML（不含外层 solidFill）
pub fn color_xml(color: &str) -> String {
    color_xml_alpha(color, None)
}

/// 拆分颜色字符串中的修饰符：`base|k=v;k=v`
fn split_color_mods(color: &str) -> (&str, Vec<(&str, &str)>) {
    match color.split_once('|') {
        None => (color, Vec::new()),
        Some((base, mods)) => {
            let list = mods
                .split(';')
                .filter_map(|kv| kv.split_once('='))
                .collect();
            (base, list)
        }
    }
}

/// 该颜色基值是否为主题色（schemeClr）
fn is_scheme_color(base: &str) -> bool {
    base.starts_with("accent")
        || base.starts_with("dk")
        || base.starts_with("lt")
        || base.starts_with("hlink")
        || base.starts_with("folHlink")
        || base.starts_with("bg")
        || base.starts_with("tx")
}

/// 生成带透明度的颜色 XML
/// 识别 schemeClr（主题色）和 srgbClr（直接色）；支持 `base|lumMod=..;lumOff=..` 编码的修饰符
fn color_xml_alpha(color: &str, alpha: Option<f64>) -> String {
    if color.is_empty() {
        return String::new();
    }
    let (base, mods) = split_color_mods(color);
    let mut inner = String::new();
    if let Some(a) = alpha.filter(|a| (*a - 1.0).abs() > f64::EPSILON) {
        inner.push_str(&format!(
            "<a:alpha val=\"{}\"/>",
            (a * 100000.0).round() as u64
        ));
    }
    for (k, v) in mods {
        inner.push_str(&format!("<a:{} val=\"{}\"/>", k, v));
    }
    if is_scheme_color(base) {
        if inner.is_empty() {
            format!("<a:schemeClr val=\"{}\"/>", base)
        } else {
            format!("<a:schemeClr val=\"{}\">{}</a:schemeClr>", base, inner)
        }
    } else {
        let hex = base.trim_start_matches('#');
        if inner.is_empty() {
            format!("<a:srgbClr val=\"{}\"/>", hex)
        } else {
            format!("<a:srgbClr val=\"{}\">{}</a:srgbClr>", hex, inner)
        }
    }
}

/// 生成纯色填充 XML
pub fn solid_fill_xml(color: &str) -> String {
    solid_fill_xml_alpha(color, None)
}

fn solid_fill_xml_alpha(color: &str, alpha: Option<f64>) -> String {
    if color.is_empty() {
        String::new()
    } else {
        format!(
            "<a:solidFill>{}</a:solidFill>",
            color_xml_alpha(color, alpha)
        )
    }
}

/// 生成变换 XML（位置、尺寸、旋转）
pub fn xfrm_xml(x: f64, y: f64, w: f64, h: f64, rotation: Option<f64>) -> String {
    let rot = rotation
        .map(|r| format!(" rot=\"{}\"", (r * 60000.0).round() as i64))
        .unwrap_or_default();
    format!(
        "<a:xfrm{}><a:off x=\"{}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{}\"/></a:xfrm>",
        rot,
        inch_to_emu(x),
        inch_to_emu(y),
        inch_to_emu(w),
        inch_to_emu(h)
    )
}

/// 生成文字运行属性 XML（a:rPr）
pub fn run_props_xml(
    font_size: Option<f64>,
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<&str>,
    font_family: Option<&str>,
) -> String {
    run_props_xml_with_hyperlink(font_size, bold, italic, underline, color, font_family, None)
}

/// 文字运行的扩展属性（高亮/下划线色/字距/语言/RTL）
#[derive(Default)]
pub struct RunExtras<'a> {
    pub highlight: Option<&'a str>,
    pub underline_color: Option<&'a str>,
    pub spacing: Option<f64>,
    pub lang: Option<&'a str>,
    pub rtl: Option<bool>,
}

/// 生成带超链接的文字运行属性 XML（a:rPr 内嵌 a:hlinkClick）
pub fn run_props_xml_with_hyperlink(
    font_size: Option<f64>,
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<&str>,
    font_family: Option<&str>,
    hyperlink_rid: Option<&str>,
) -> String {
    run_props_xml_ex(
        font_size,
        bold,
        italic,
        underline,
        color,
        font_family,
        hyperlink_rid,
        RunExtras::default(),
    )
}

/// 生成带扩展属性的文字运行 XML
#[allow(clippy::too_many_arguments)]
pub fn run_props_xml_ex(
    font_size: Option<f64>,
    bold: bool,
    italic: bool,
    underline: bool,
    color: Option<&str>,
    font_family: Option<&str>,
    hyperlink_rid: Option<&str>,
    extras: RunExtras<'_>,
) -> String {
    let lang = extras.lang.unwrap_or("en-US");
    let mut attrs = vec![
        format!("lang=\"{}\"", esc_xml(lang)),
        "dirty=\"0\"".to_string(),
    ];
    if let Some(sz) = font_size {
        attrs.push(format!("sz=\"{}\"", pt_to_hundredths(sz)));
    }
    if bold {
        attrs.push("b=\"1\"".to_string());
    }
    if italic {
        attrs.push("i=\"1\"".to_string());
    }
    if underline {
        attrs.push("u=\"sng\"".to_string());
    }
    if let Some(spc) = extras.spacing {
        attrs.push(format!("spc=\"{}\"", (spc * 100.0).round() as i64));
    }
    if extras.rtl == Some(true) {
        attrs.push("rtl=\"1\"".to_string());
    }
    let mut inner = String::new();
    if let Some(c) = color {
        inner.push_str(&solid_fill_xml(c));
    }
    if let Some(f) = font_family {
        inner.push_str(&format!(
            "<a:latin typeface=\"{}\"/><a:ea typeface=\"{}\"/><a:cs typeface=\"{}\"/>",
            esc_xml(f),
            esc_xml(f),
            esc_xml(f)
        ));
    }
    if let Some(h) = extras.highlight {
        inner.push_str(&format!(
            "<a:highlight>{}</a:highlight>",
            color_xml(h.trim_start_matches('#'))
        ));
    }
    if underline {
        if let Some(uc) = extras.underline_color {
            inner.push_str(&format!("<a:uLn>{}</a:uLn>", solid_fill_xml(uc)));
        }
    }
    if let Some(rid) = hyperlink_rid {
        inner.push_str(&format!("<a:hlinkClick r:id=\"{}\"/>", rid));
    }
    format!("<a:rPr {}>{}</a:rPr>", attrs.join(" "), inner)
}

/// 生成段落属性 XML（a:pPr），支持行距倍数（line_spacing，如 1.2）
pub fn para_props_xml(align: Option<&str>, bullet: bool) -> String {
    para_props_xml_line_spacing(align, bullet, None)
}

/// 生成段落属性 XML（a:pPr），支持行距倍数（line_spacing，如 1.2）
pub fn para_props_xml_line_spacing(
    align: Option<&str>,
    bullet: bool,
    line_spacing: Option<f64>,
) -> String {
    let mut attrs = Vec::new();
    if let Some(a) = align {
        let m = match a {
            "left" => "l",
            "center" => "ctr",
            "right" => "r",
            "justify" => "just",
            _ => a,
        };
        attrs.push(format!("algn=\"{}\"", m));
    }
    let mut inner = String::new();
    if let Some(ls) = line_spacing {
        // 正值 → spcPct 百分比（100000=100%）；负值 → spcPts 固定磅值（1/100 pt）
        if ls < 0.0 {
            let pts = (-ls * 100.0).round() as i64;
            inner.push_str(&format!("<a:lnSpc><a:spcPts val=\"{}\"/></a:lnSpc>", pts));
        } else {
            let pct = (ls * 100000.0).round() as i64;
            inner.push_str(&format!("<a:lnSpc><a:spcPct val=\"{}\"/></a:lnSpc>", pct));
        }
    }
    if bullet {
        return format!(
            "<a:pPr {}>{}{}</a:pPr>",
            attrs.join(" "),
            inner,
            "<a:buChar char=\"\u{2022}\"/>"
        );
    }
    if attrs.is_empty() && inner.is_empty() {
        String::new()
    } else if inner.is_empty() {
        format!("<a:pPr {}/>", attrs.join(" "))
    } else {
        format!("<a:pPr {}>{}</a:pPr>", attrs.join(" "), inner)
    }
}

/// 生成填充 XML（支持纯色和渐变）
pub fn fill_xml(fill: &crate::model::elements::Fill) -> String {
    fill_xml_alpha(fill, None)
}

/// 生成带透明度的填充 XML
pub fn fill_xml_alpha(fill: &crate::model::elements::Fill, alpha: Option<f64>) -> String {
    match fill {
        crate::model::elements::Fill::Solid(color) => solid_fill_xml_alpha(color, alpha),
        // 图片填充需由调用方结合关系表生成（此处无 rId）
        crate::model::elements::Fill::Image { .. } => String::new(),
        crate::model::elements::Fill::Pattern { pattern, fg, bg } => format!(
            "<a:pattFill prst=\"{}\"><a:fgClr>{}</a:fgClr><a:bgClr>{}</a:bgClr></a:pattFill>",
            pattern,
            color_xml(fg),
            color_xml(bg)
        ),
        crate::model::elements::Fill::Gradient { stops, angle } => {
            let mut stops_xml = String::new();
            for stop in stops {
                let pos = (stop.position * 1000.0).round() as i64;
                stops_xml.push_str(&format!(
                    "<a:gs pos=\"{}\">{}</a:gs>",
                    pos,
                    color_xml_alpha(&stop.color, alpha)
                ));
            }
            let ang = angle.map(|a| (a * 60000.0).round() as i64).unwrap_or(0);
            format!(
                "<a:gradFill><a:gsLst>{}</a:gsLst><a:lin ang=\"{}\" scaled=\"1\"/></a:gradFill>",
                stops_xml, ang
            )
        }
    }
}

/// 生成线条 XML（不含透明度）
pub fn line_xml(color: &str, width: f64) -> String {
    line_xml_alpha(color, width, None)
}

/// 生成带透明度的线条 XML
pub fn line_xml_alpha(color: &str, width: f64, alpha: Option<f64>) -> String {
    let w = (width * 12700.0).round() as i64;
    format!(
        "<a:ln w=\"{}\" cap=\"flat\">{}</a:ln>",
        w,
        solid_fill_xml_alpha(color, alpha)
    )
}

/// 生成效果列表 XML（外阴影 + 发光/柔化边缘/模糊/倒影/内阴影/填充覆盖）
///
/// 子元素顺序遵循 CT_EffectList 的 schema 顺序：
/// blur → fillOverlay → glow → innerShdw → outerShdw → reflection → softEdge
pub fn effect_lst_xml(
    shadow: Option<&crate::model::elements::Shadow>,
    effects: Option<&crate::model::elements::Effects>,
) -> String {
    let mut inner = String::new();
    if let Some(fx) = effects {
        if let Some(b) = fx.blur {
            inner.push_str(&format!(
                "<a:blur rad=\"{}\" grow=\"0\"/>",
                (b * 12700.0).round() as i64
            ));
        }
        if let Some(fo) = &fx.fill_overlay {
            let alpha = fo
                .opacity
                .map(|o| format!("<a:alpha val=\"{}\"/>", (o * 100000.0).round() as i64))
                .unwrap_or_default();
            inner.push_str(&format!(
                "<a:fillOverlay blend=\"over\"><a:solidFill><a:srgbClr val=\"{}\">{}</a:srgbClr></a:solidFill></a:fillOverlay>",
                fo.color.trim_start_matches('#'),
                alpha
            ));
        }
        if let Some(g) = &fx.glow {
            let alpha = g
                .opacity
                .map(|o| format!("<a:alpha val=\"{}\"/>", (o * 100000.0).round() as i64))
                .unwrap_or_default();
            inner.push_str(&format!(
                "<a:glow rad=\"{}\"><a:srgbClr val=\"{}\">{}</a:srgbClr></a:glow>",
                (g.radius * 12700.0).round() as i64,
                g.color.trim_start_matches('#'),
                alpha
            ));
        }
        if let Some(s) = &fx.inner_shadow {
            inner.push_str(&format!(
                "<a:innerShdw blurRad=\"{}\" dist=\"{}\" dir=\"{}\"><a:srgbClr val=\"{}\"><a:alpha val=\"{}\"/></a:srgbClr></a:innerShdw>",
                (s.blur * 12700.0).round() as i64,
                (s.distance * 12700.0).round() as i64,
                (s.angle * 60000.0).round() as i64,
                s.color.trim_start_matches('#'),
                (s.opacity * 1000.0).round() as i64,
            ));
        }
    }
    if let Some(s) = shadow {
        inner.push_str(&shadow_xml_inner(
            s.blur, s.distance, s.angle, s.opacity, &s.color,
        ));
    }
    if let Some(fx) = effects {
        if fx.reflection == Some(true) {
            inner.push_str(
                "<a:reflection blurRad=\"6350\" stA=\"52000\" endA=\"300\" dist=\"20000\" dir=\"5400000\" sy=\"-100000\" algn=\"b\" rotWithShape=\"0\"/>",
            );
        }
        if let Some(radius) = fx.soft_edge {
            inner.push_str(&format!(
                "<a:softEdge rad=\"{}\"/>",
                (radius * 12700.0).round() as i64
            ));
        }
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<a:effectLst>{}</a:effectLst>", inner)
    }
}

/// 生成外阴影元素 XML（不含 effectLst 包裹）
pub fn shadow_xml_inner(blur: f64, distance: f64, angle: f64, opacity: f64, color: &str) -> String {
    let b = (blur * 12700.0).round() as i64;
    let d = (distance * 12700.0).round() as i64;
    let a = (angle * 60000.0).round() as i64;
    let o = (opacity * 1000.0).round() as i64;
    let c = color.trim_start_matches('#');
    format!(
        "<a:outerShdw blurRad=\"{}\" dist=\"{}\" dir=\"{}\" rotWithShape=\"0\">\
         <a:srgbClr val=\"{}\"><a:alpha val=\"{}\"/></a:srgbClr></a:outerShdw>",
        b, d, a, c, o
    )
}

/// 生成阴影效果 XML（仅外阴影，包裹在 effectLst 中）
pub fn shadow_xml(blur: f64, distance: f64, angle: f64, opacity: f64, color: &str) -> String {
    format!(
        "<a:effectLst>{}</a:effectLst>",
        shadow_xml_inner(blur, distance, angle, opacity, color)
    )
}
