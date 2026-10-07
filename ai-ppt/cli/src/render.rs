//! 渲染层：将幻灯片渲染为 PNG（内部先生成 SVG，再用 resvg 栅格化）
//!
//! 输入：json2pptx 模型中的 Slide（+ 演示尺寸英寸 + 媒体解析器），输出 PNG。
//! 这是网页端渲染能力的 Rust 原生近似实现：按元素位置/样式生成矢量图形。

use base64::Engine;
use json2pptx::model::elements::{
    BarChart, ChartElement, Element, Fill, KpiCard, Line, LineChart, PieChart, ProcessFlow,
    ProgressBar, ProgressRing, RatingStars, RingChart, TextContent, Timeline,
};
use json2pptx::model::Slide;
use std::path::Path;
use std::sync::Arc;

/// 每英寸像素（缩放由调用方传入 scale）
pub type MediaResolver = Arc<dyn Fn(&str) -> String>;

const DEFAULT_FONT_SIZE: f64 = 18.0;
const DEFAULT_TEXT_COLOR: &str = "#000000";
const DEFAULT_SHAPE_FILL: &str = "#4472C4";

/// 渲染幻灯片为 PNG
pub fn render_slide_png(
    slide: &Slide,
    width_in: f64,
    height_in: f64,
    scale: f64,
    resolve_media: &MediaResolver,
    out_path: &Path,
) -> Result<(), String> {
    let svg = render_slide_svg(slide, width_in, height_in, scale, resolve_media)?;
    svg_to_png(&svg, out_path)
}

// === 数值 / 颜色 / 转义辅助 ===

fn n(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    let mut s = format!("{r}");
    if s.ends_with(".0") {
        s.truncate(s.len() - 2);
    }
    s
}

fn col(s: &str) -> String {
    if s.starts_with('#') {
        s.to_string()
    } else {
        format!("#{s}")
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn alpha_attr(alpha: Option<f64>) -> String {
    match alpha {
        Some(a) if (0.0..1.0).contains(&a) => format!(" fill-opacity=\"{}\"", n(a)),
        _ => String::new(),
    }
}

fn line_attr(line: &Option<Line>, alpha: Option<f64>) -> String {
    match line {
        Some(l) => {
            let mut a = format!(
                " stroke=\"{}\" stroke-width=\"{}\"",
                col(&l.color),
                n(l.width * 1.33333)
            );
            if let Some(al) = alpha {
                a.push_str(&format!(" stroke-opacity=\"{}\"", n(al)));
            }
            a
        }
        None => String::new(),
    }
}

fn font_attrs(
    size_pt: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
) -> String {
    let mut parts = Vec::new();
    let size = size_pt.unwrap_or(DEFAULT_FONT_SIZE) * 1.33333;
    parts.push(format!("font-size=\"{}\"", n(size)));
    if bold.unwrap_or(false) {
        parts.push("font-weight=\"bold\"".to_string());
    }
    if italic.unwrap_or(false) {
        parts.push("font-style=\"italic\"".to_string());
    }
    let mut deco = Vec::new();
    if underline.unwrap_or(false) {
        deco.push("underline");
    }
    if !deco.is_empty() {
        parts.push(format!("text-decoration=\"{}\"", deco.join(" ")));
    }
    parts
        .into_iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

// === 媒体嵌入（base64 data URI）===

fn embed_image(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        return path.to_string();
    }
    let Ok(bytes) = std::fs::read(path) else {
        return String::new();
    };
    let ext = Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "image/png",
    };
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

// === 底部背景 ===

fn bg_svg(slide: &Slide, w: f64, h: f64) -> String {
    match &slide.background {
        Some(serde_json::Value::String(c)) => {
            format!(
                "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                n(w),
                n(h),
                col(c)
            )
        }
        Some(serde_json::Value::Object(bg)) => {
            let ty = bg.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if ty == "image" {
                let _src = bg.get("src").and_then(|v| v.as_str()).unwrap_or("");
                return String::new(); // 背景图由调用方按需处理，这里先留空
            }
            if ty == "gradient" {
                if let Some(stops) = bg.get("stops").and_then(|v| v.as_array()) {
                    let mut defs = String::new();
                    for (i, s) in stops.iter().enumerate() {
                        let c = s.get("color").and_then(|v| v.as_str()).unwrap_or("FFFFFF");
                        let pos = s.get("position").and_then(|v| v.as_f64()).unwrap_or(0.0) * 100.0;
                        defs.push_str(&format!(
                            "<stop offset=\"{}\" stop-color=\"{}\"/>",
                            n(pos),
                            col(c)
                        ));
                        let _ = i;
                    }
                    let mut out = format!(
                        "<defs><linearGradient id=\"bg\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">{}</linearGradient></defs>",
                        defs
                    );
                    out.push_str(&format!(
                        "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" fill=\"url(#bg)\"/>",
                        n(w),
                        n(h)
                    ));
                    return out;
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

// === 文本 ===

fn para_lines(tc: &TextContent) -> Vec<String> {
    match tc {
        TextContent::Simple(s) => s.split('\n').map(|s| s.to_string()).collect(),
        TextContent::Paragraphs(paras) => paras
            .iter()
            .map(|p| {
                p.text.clone().unwrap_or_else(|| {
                    p.runs
                        .as_ref()
                        .map(|rs| rs.iter().map(|r| r.text.clone()).collect())
                        .unwrap_or_default()
                })
            })
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)] // SVG 绘制参数天然较多，保持扁平签名
fn text_svg(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    text: &TextContent,
    size_pt: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    color: Option<&str>,
    align: Option<&str>,
    vert_align: Option<&str>,
    _vert: Option<&str>,
) -> String {
    let lines = para_lines(text);
    if lines.is_empty() {
        return String::new();
    }
    let size_px = size_pt.unwrap_or(DEFAULT_FONT_SIZE) * 1.33333;
    let line_h = size_px * 1.25;
    let total = line_h * lines.len() as f64;
    let start_y = match vert_align {
        Some("middle") => y + (h - total) / 2.0 + size_px * 0.8,
        Some("bottom") => y + (h - total) + size_px * 0.8,
        _ => y + size_px * 0.8,
    };
    let tx = match align.unwrap_or("left") {
        "center" => x + w / 2.0,
        "right" => x + w,
        _ => x,
    };
    let text_anchor = match align.unwrap_or("left") {
        "center" => "middle",
        "right" => "end",
        _ => "start",
    };
    let attrs = font_attrs(size_pt, bold, italic, underline);
    let fill = col(color.unwrap_or(DEFAULT_TEXT_COLOR));
    let mut out = String::new();
    for (i, ln) in lines.iter().enumerate() {
        if ln.is_empty() {
            continue;
        }
        let ty = start_y + line_h * i as f64;
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"{}\" fill=\"{}\" {}>{}</text>",
            n(tx),
            n(ty),
            text_anchor,
            fill,
            attrs,
            esc(ln),
        ));
    }
    out
}

// === 形状 ===

fn shape_svg(shape_type: &str, x: f64, y: f64, w: f64, h: f64) -> String {
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let rx = w / 2.0;
    let ry = h / 2.0;
    match shape_type {
        "ellipse" | "oval" => format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"/>", n(cx), n(cy), n(rx), n(ry)),
        "roundRect" | "roundedRectangle" | "flowChartProcess" => {
            let r = (h.min(w) * 0.15).max(1.0);
            format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\"/>", n(x), n(y), n(w), n(h), n(r), n(r))
        }
        "flowChartTerminator" => {
            let r = h / 2.0;
            format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\"/>", n(x), n(y), n(w), n(h), n(r), n(r))
        }
        "triangle" => {
            let pts = format!("{},{} {},{} {},{}", n(x), n(y + h), n(cx), n(y), n(x + w), n(y + h));
            format!("<polygon points=\"{pts}\"/>")
        }
        "diamond" => format!("<polygon points=\"{cx},{y} {xw},{cy} {cx},{ywh} {x},{cy}\"/>", xw = x + w, ywh = y + h),
        "hexagon" => {
            let i = 0.25 * w;
            format!("<polygon points=\"{x2},{y} {xw2},{y} {xw},{cy} {xw2},{ywh} {x2},{ywh} {x},{cy}\"/>", x2 = x + i, xw2 = x + w - i, xw = x + w, ywh = y + h)
        }
        "pentagon" => {
            let i = 0.15 * w;
            format!("<polygon points=\"{cx},{y} {xw},{yh} {xw2},{ywh} {x2},{ywh} {x},{yh}\"/>", xw = x + w, yh = y + 0.35 * h, xw2 = x + w - i, x2 = x + i, ywh = y + h)
        }
        "star5" => {
            let (ox, oy, orx, ory) = (cx, cy, w / 2.0, h / 2.0);
            let mut pts = Vec::new();
            for k in 0..10 {
                let ang = -PI / 2.0 + k as f64 * PI / 5.0;
                let (mr, mr2) = if k % 2 == 0 { (1.0, 0.42) } else { (0.42, 1.0) };
                let (rrx, rry) = (orx * mr * mr2, ory * mr * mr2);
                pts.push(format!("{},{}", n(ox + rrx * ang.cos()), n(oy + rry * ang.sin())));
            }
            format!("<polygon points=\"{}\"/>", pts.join(" "))
        }
        "chevron" => {
            let i = 0.5 * h.min(w) * 0.5;
            format!("<polygon points=\"{x},{y} {xw2},{y} {xw},{cy} {xw2},{ywh} {x},{ywh} {xi},{cy}\"/>", xw2 = x + w - i, xw = x + w, ywh = y + h, xi = x + i)
        }
        "rightArrow" => format!("<polygon points=\"{x},{y} {xw2},{y} {xw},{cy} {xw2},{ywh} {x},{ywh} {xp},{cy}\"/>", xw2 = x + w * 0.6, xw = x + w, ywh = y + h, xp = x + w * 0.6),
        "leftArrow" => format!("<polygon points=\"{xw},{yh0} {xp},{yh0} {xp},{y} {x},{cy} {xp},{ywh} {xp},{yh1} {xw},{yh1}\"/>", xw = x + w, yh0 = y + 0.2 * h, xp = x + w * 0.4, ywh = y + h, yh1 = y + 0.8 * h),
        "flowChartDecision" => format!("<polygon points=\"{cx},{y} {xw},{cy} {cx},{ywh} {x},{cy}\"/>", xw = x + w, ywh = y + h),
        "donut" | "blockArc" | "pie" => {
            format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\"/>", n(cx), n(cy), n((w.min(h) / 2.0) * 0.9))
        }
        _ => {
            let r = (h.min(w) * 0.12).max(1.0);
            format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\"/>", n(x), n(y), n(w), n(h), n(r), n(r))
        }
    }
}

// 常量（Rust 无内置 PI 直接用于浮点）；使用 std::f64::consts
use std::f64::consts::PI;

// === 线条 ===

fn line_svg(
    pos: (f64, f64, f64, f64),
    points: &[(f64, f64)],
    color: &str,
    width_pt: f64,
    dash: &str,
) -> String {
    if points.is_empty() {
        return String::new();
    }
    let (ox, oy, _w, _h) = pos;
    let pts: Vec<String> = points
        .iter()
        .map(|(px, py)| format!("{},{}", n(ox + px), n(oy + py)))
        .collect();
    let dash_attr = match dash {
        "dashed" => " stroke-dasharray=\"8 5\"".to_string(),
        "dotted" => " stroke-dasharray=\"2 4\"".to_string(),
        _ => String::new(),
    };
    format!(
        "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">{}</polyline>",
        pts.join(" "),
        col(color),
        n(width_pt * 1.33333),
        dash_attr
    )
}

// === 图标 ===

fn icon_svg(pos: (f64, f64, f64, f64)) -> String {
    let (x, y, w, h) = pos;
    let r = (w.min(h) / 2.0) * 0.8;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    format!(
        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke-width=\"1.5\"/>",
        n(cx),
        n(cy),
        n(r)
    )
}

// === 表格 ===

fn table_svg(t: &json2pptx::model::elements::TableElement, resolve: &MediaResolver) -> String {
    let _ = resolve;
    let mut out = String::new();
    let rows = t.rows.len();
    if rows == 0 {
        return out;
    }
    let cols = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return out;
    }
    let (x, y, w, h) = (
        t.position.x * 96.0,
        t.position.y * 96.0,
        t.position.w * 96.0,
        t.position.h * 96.0,
    );
    let col_w = match &t.column_widths {
        Some(ws) if ws.len() == cols => ws.iter().map(|v| v * 96.0).collect::<Vec<_>>(),
        _ => vec![w / cols as f64; cols],
    };
    let row_h = h / rows as f64;
    let base_fs = t.font_size.unwrap_or(14.0);
    for (ri, row) in t.rows.iter().enumerate() {
        let mut cx = x;
        for (ci, cell) in row.iter().enumerate() {
            let cw = col_w.get(ci).copied().unwrap_or(col_w[0]);
            let fill = cell.fill.as_deref().unwrap_or("");
            if !fill.is_empty() {
                out.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\" stroke=\"#cccccc\" stroke-width=\"1\"/>",
                    n(cx),
                    n(y + ri as f64 * row_h),
                    n(cw),
                    n(row_h),
                    col(fill)
                ));
            } else {
                out.push_str(&format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#ffffff\" stroke=\"#cccccc\" stroke-width=\"1\"/>",
                    n(cx),
                    n(y + ri as f64 * row_h),
                    n(cw),
                    n(row_h)
                ));
            }
            let fill = cell
                .color
                .as_deref()
                .unwrap_or(t.color.as_deref().unwrap_or("000000"));
            let fs = cell.font_size.unwrap_or(base_fs) * 1.33333;
            let anch = match cell.align.as_deref().unwrap_or("left") {
                "center" => "middle",
                "right" => "end",
                _ => "start",
            };
            let txx = match cell.align.as_deref().unwrap_or("left") {
                "center" => cx + cw / 2.0,
                "right" => cx + cw - 6.0,
                _ => cx + 6.0,
            };
            let boldx = if cell.bold.unwrap_or(false) {
                " font-weight=\"bold\""
            } else {
                ""
            };
            out.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"{}\" font-size=\"{}\" fill=\"{}\"{}>{}</text>",
                n(txx),
                n(y + ri as f64 * row_h + row_h / 2.0 + fs * 0.35),
                anch,
                n(fs),
                col(fill),
                boldx,
                esc(&cell.text)
            ));
            cx += cw;
        }
    }
    // 宽度补齐（若列宽超过表格宽，忽略）
    let _ = (x, y, w, h);
    out
}

// === 主元素分发 ===

fn element_svg(el: &Element, resolve: &MediaResolver) -> String {
    let p = el.position();
    let (pxx, pyy) = (p.x, p.y); // 英寸
    let (x, y, w, h) = (pxx * 96.0, pyy * 96.0, p.w * 96.0, p.h * 96.0);
    match el {
        Element::Text(t) => {
            let mut g = String::new();
            if let Some(f) = &t.fill {
                g.push_str(&fill_svg(f, x, y, w, h, t.fill_alpha, (t.line).is_some()));
            }
            g.push_str(&text_svg(
                x,
                y,
                w,
                h,
                &t.text,
                t.font_size,
                t.bold,
                t.italic,
                t.underline,
                t.color.as_deref(),
                t.align.as_deref(),
                t.vert_align.as_deref(),
                t.vert.as_deref(),
            ));
            g
        }
        Element::Shape(s) => {
            let fill = match (&s.fill, s.no_fill) {
                (_, Some(true)) => "none".to_string(),
                (Some(Fill::Solid(c)), _) => col(c),
                (Some(Fill::Gradient { .. }), _) => DEFAULT_SHAPE_FILL.to_string(),
                _ => DEFAULT_SHAPE_FILL.to_string(),
            };
            let sh = shape_svg(&s.shape_type, x, y, w, h);
            let mut shape_open = format!("<g fill=\"{}\"{}", fill, alpha_attr(s.fill_alpha));
            shape_open.push_str(&line_attr(&s.line, s.line_alpha));
            shape_open.push('>');
            shape_open.push_str(&sh);
            shape_open.push_str("</g>");
            if let Some(text) = &s.text {
                shape_open.push_str(&text_svg(
                    x,
                    y,
                    w,
                    h,
                    text,
                    s.font_size,
                    None,
                    None,
                    None,
                    s.color.as_deref(),
                    s.align.as_deref(),
                    s.vert_align.as_deref(),
                    None,
                ));
            }
            shape_open
        }
        Element::Line(l) => {
            let color = l.color.as_deref().unwrap_or("4472C4");
            let wpt = l.width.unwrap_or(1.0);
            let dash = l.dash.as_deref().unwrap_or("solid");
            line_svg((x, y, w, h), &l.points, color, wpt, dash)
        }
        Element::Image(i) => {
            let abs = resolve(&i.src);
            let data = embed_image(&abs);
            if data.is_empty() {
                return String::new();
            }
            format!(
                "<image href=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"xMidYMid meet\"/>",
                esc(&data),
                n(x),
                n(y),
                n(w),
                n(h)
            )
        }
        Element::Icon(_) => icon_svg((x, y, w, h)),
        Element::Formula(f) => text_svg(
            x,
            y,
            w,
            h,
            &TextContent::Simple(f.latex.clone()),
            f.font_size,
            None,
            None,
            None,
            f.color.as_deref(),
            Some("left"),
            None,
            None,
        ),
        Element::Table(t) => table_svg(t, resolve),
        Element::Chart(c) => real_chart_svg(c, x, y, w, h),
        Element::Comment(_) => String::new(),
        Element::Equation(eq) => text_svg(
            x,
            y,
            w,
            h,
            &TextContent::Simple(eq.text.clone().unwrap_or_else(|| "[公式]".to_string())),
            None,
            None,
            None,
            None,
            None,
            Some("left"),
            None,
            None,
        ),
        Element::Opaque(o) => {
            if let Some(t) = &o.text {
                text_svg(
                    x,
                    y,
                    w,
                    h,
                    &TextContent::Simple(t.clone()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some("left"),
                    None,
                    None,
                )
            } else {
                String::new()
            }
        }
        Element::Group(g) => {
            let mut inner = String::new();
            for child in &g.children {
                inner.push_str(&element_svg(child, resolve));
            }
            let rot = g.rotation.unwrap_or(0.0);
            let tr = if rot != 0.0 {
                format!(
                    "transform=\"rotate({} {} {})\"",
                    n(rot),
                    n(x + w / 2.0),
                    n(y + h / 2.0)
                )
            } else {
                String::new()
            };
            // group children 位置相对 group 原点，需平移
            let real = format!(
                "<g transform=\"translate({} {})\" {}>{}</g>",
                n(x),
                n(y),
                tr,
                inner
            );
            real
        }
        Element::ProgressBar(c) => progress_bar_svg(c, x, y, w, h),
        Element::ProgressRing(c) => progress_ring_svg(c, x, y, w, h),
        Element::BarChart(c) => bar_chart_svg(c, x, y, w, h),
        Element::LineChart(c) => line_chart_svg(c, x, y, w, h),
        Element::PieChart(c) => pie_chart_svg(c, x, y, w, h),
        Element::RingChart(c) => ring_chart_svg(c, x, y, w, h),
        Element::KpiCard(c) => kpi_card_svg(c, x, y, w, h),
        Element::RatingStars(c) => rating_stars_svg(c, x, y, w, h),
        Element::Timeline(c) => timeline_svg(c, x, y, w, h),
        Element::ProcessFlow(c) => process_flow_svg(c, x, y, w, h),
    }
}

fn fill_svg(
    f: &Fill,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    alpha: Option<f64>,
    has_line: bool,
) -> String {
    match f {
        Fill::Solid(c) => {
            let mut s = format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"",
                n(x),
                n(y),
                n(w),
                n(h),
                col(c)
            );
            s.push_str(&alpha_attr(alpha));
            s.push_str(&line_attr(&None, None));
            s.push_str("/>");
            s
        }
        Fill::Gradient { .. } => {
            let c = if has_line { "4472C4" } else { "FFFFFF" };
            let mut s = format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"",
                n(x),
                n(y),
                n(w),
                n(h),
                col(c)
            );
            s.push_str(&alpha_attr(alpha));
            s.push_str("/>");
            s
        }
        Fill::Pattern { fg, .. } => {
            let mut s = format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"",
                n(x),
                n(y),
                n(w),
                n(h),
                col(fg)
            );
            s.push_str(&alpha_attr(alpha));
            s.push_str("/>");
            s
        }
        Fill::Image { .. } => {
            format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"CCCCCC\"",
                n(x),
                n(y),
                n(w),
                n(h)
            )
        }
    }
}

// === 真实图表的近似渲染（原生引擎回退） ===

fn real_chart_svg(c: &ChartElement, x: f64, y: f64, w: f64, h: f64) -> String {
    let palette = ["4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47"];
    let mut out = format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"FFFFFF\" stroke=\"D9D9D9\"/>",
        n(x),
        n(y),
        n(w),
        n(h)
    );
    let title_h = if c.title.is_some() { 18.0 } else { 0.0 };
    if let Some(t) = &c.title {
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"12\" text-anchor=\"middle\" fill=\"333333\">{}</text>",
            n(x + w / 2.0),
            n(y + 14.0),
            esc(t)
        ));
    }
    let plot_y = y + title_h + 6.0;
    let plot_h = (h - title_h - 18.0).max(1.0);
    let cat_n = c
        .series
        .iter()
        .map(|s| s.values.len())
        .chain(std::iter::once(c.categories.len()))
        .max()
        .unwrap_or(0);
    if cat_n == 0 {
        return out;
    }
    let max = c
        .series
        .iter()
        .flat_map(|s| s.values.iter().cloned())
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let slot = w / cat_n as f64;
    let ser_n = c.series.len().max(1);
    let bar_w = (slot * 0.8) / ser_n as f64;
    for (si, s) in c.series.iter().enumerate() {
        let color = s
            .color
            .clone()
            .unwrap_or_else(|| palette[si % palette.len()].to_string());
        for (i, v) in s.values.iter().enumerate() {
            let bh = (v / max) * plot_h;
            let bx = x + slot * i as f64 + slot * 0.1 + bar_w * si as f64;
            out.push_str(&format!(
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
                n(bx),
                n(plot_y + plot_h - bh),
                n(bar_w),
                n(bh),
                col(&color)
            ));
        }
    }
    for (i, cat) in c.categories.iter().enumerate() {
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"9\" text-anchor=\"middle\" fill=\"666666\">{}</text>",
            n(x + slot * i as f64 + slot / 2.0),
            n(plot_y + plot_h + 12.0),
            esc(cat)
        ));
    }
    out
}

// === 组件近似 ===

fn progress_bar_svg(c: &ProgressBar, x: f64, y: f64, w: f64, h: f64) -> String {
    let v = c.value.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0;
    let track = c.track_color.as_deref().unwrap_or("E0E0E0");
    let color = c.color.as_deref().unwrap_or("4472C4");
    let r = if c.rounded.unwrap_or(true) {
        (h / 2.0).min(10.0)
    } else {
        1.0
    };
    let mut out = format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{}\"/>",
        n(x),
        n(y),
        n(w),
        n(h),
        n(r),
        n(r),
        col(track)
    );
    out.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{}\"/>",
        n(x),
        n(y),
        n(w * v),
        n(h),
        n(r),
        n(r),
        col(color)
    ));
    out
}

fn progress_ring_svg(c: &ProgressRing, x: f64, y: f64, w: f64, h: f64) -> String {
    let v = c.value.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0;
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let r = (w.min(h) / 2.0) * 0.8;
    let sw = c.thickness.unwrap_or(12.0);
    let track = c.track_color.as_deref().unwrap_or("E0E0E0");
    let color = c.color.as_deref().unwrap_or("4472C4");
    let circ = 2.0 * PI * r;
    let mut out =
        format!(
        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"/>",
        n(cx), n(cy), n(r), col(track), n(sw)
    );
    out.push_str(&format!(
        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-dasharray=\"{}\" stroke-dashoffset=\"{}\" transform=\"rotate(-90 {} {})\"/>",
        n(cx), n(cy), n(r), col(color), n(sw), n(circ * v), n(circ), n(cx), n(cy)
    ));
    out
}

fn bar_chart_svg(c: &BarChart, x: f64, y: f64, w: f64, h: f64) -> String {
    if c.data.is_empty() {
        return String::new();
    }
    let max = c
        .max
        .unwrap_or_else(|| c.data.iter().cloned().fold(0.0, f64::max).max(1.0));
    let n_bars = c.data.len();
    let gap = w * 0.08;
    let bw = (w - gap * (n_bars as f64 + 1.0)) / n_bars as f64;
    let mut out = String::new();
    for (i, v) in c.data.iter().enumerate() {
        let hh = (v / max).max(0.0) * (h * 0.85);
        let bx = x + gap + i as f64 * (bw + gap);
        let by = y + h - hh;
        let color = c
            .colors
            .as_ref()
            .and_then(|cs| cs.get(i))
            .map(|s| col(s))
            .unwrap_or_else(|| "#4472C4".to_string());
        out.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{}\"/>",
            n(bx),
            n(by),
            n(bw),
            n(hh),
            color
        ));
        if c.show_values.unwrap_or(false) {
            out.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"12\" fill=\"#666\">{}</text>",
                n(bx + bw / 2.0), n(by - 4.0), n(*v)
            ));
        }
        if let Some(labels) = &c.labels {
            if let Some(lb) = labels.get(i) {
                out.push_str(&format!(
                    "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"11\" fill=\"#888\">{}</text>",
                    n(bx + bw / 2.0), n(y + h - 4.0), esc(lb)
                ));
            }
        }
    }
    out
}

fn line_chart_svg(c: &LineChart, x: f64, y: f64, w: f64, h: f64) -> String {
    if c.data.is_empty() {
        return String::new();
    }
    let max = c
        .max
        .unwrap_or_else(|| c.data.iter().cloned().fold(0.0, f64::max).max(1.0));
    let n_pt = c.data.len();
    let step = if n_pt > 1 { w / (n_pt as f64 - 1.0) } else { w };
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for (i, v) in c.data.iter().enumerate() {
        let px = x + i as f64 * step;
        let py = y + h - (v / max).max(0.0) * (h * 0.85);
        pts.push((px, py));
    }
    let color = c
        .colors
        .as_ref()
        .and_then(|cs| cs.first())
        .map(|s| col(s))
        .unwrap_or_else(|| "#4472C4".to_string());
    let pts_str: Vec<String> = pts.iter().map(|(a, b)| format!("{},{b}", n(*a))).collect();
    let mut out = format!(
        "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"2.5\" stroke-linejoin=\"round\"/>",
        pts_str.join(" "), color
    );
    for (a, b) in pts {
        out.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"3.5\" fill=\"{}\"/>",
            n(a),
            n(b),
            color
        ));
    }
    out
}

fn pie_chart_svg(c: &PieChart, x: f64, y: f64, w: f64, h: f64) -> String {
    if c.data.is_empty() {
        return String::new();
    }
    let total: f64 = c.data.iter().sum();
    if total <= 0.0 {
        return String::new();
    }
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let r = (w.min(h) / 2.0) * 0.85;
    let mut out = String::new();
    let mut angle = -PI / 2.0;
    for (i, v) in c.data.iter().enumerate() {
        let frac = v / total;
        let sweep = frac * 2.0 * PI;
        let color = c
            .colors
            .as_ref()
            .and_then(|cs| cs.get(i))
            .map(|s| col(s))
            .unwrap_or_else(|| "#4472C4".to_string())
            .to_string();
        let x1 = cx + r * angle.cos();
        let y1 = cy + r * angle.sin();
        let a2 = angle + sweep;
        let x2 = cx + r * a2.cos();
        let y2 = cy + r * a2.sin();
        let large = if sweep > PI { 1 } else { 0 };
        out.push_str(&format!(
            "<path d=\"M {cx} {cy} L {} {} A {} {} 0 {large} 1 {} {} Z\" fill=\"{}\"/>",
            n(x1),
            n(y1),
            n(r),
            n(r),
            n(x2),
            n(y2),
            color
        ));
        angle = a2;
    }
    out
}

fn ring_chart_svg(c: &RingChart, x: f64, y: f64, w: f64, h: f64) -> String {
    if c.data.is_empty() {
        return String::new();
    }
    let total: f64 = c.data.iter().sum();
    if total <= 0.0 {
        return String::new();
    }
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let r = (w.min(h) / 2.0) * 0.8;
    let sw = r * c.thickness.unwrap_or(0.35) * 0.8;
    let circ = 2.0 * PI * r;
    let mut out = String::new();
    let mut acc = 0.0;
    for (i, v) in c.data.iter().enumerate() {
        let frac = v / total;
        let color = c
            .colors
            .as_ref()
            .and_then(|cs| cs.get(i))
            .map(|s| col(s))
            .unwrap_or_else(|| "#4472C4".to_string())
            .to_string();
        let start = second_opacity(i);
        let _ = start;
        let dash = frac * circ;
        out.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" stroke-dasharray=\"{}\" stroke-dashoffset=\"{}\" transform=\"rotate(-90 {} {})\"/>",
            n(cx), n(cy), n(r), color, n(sw), n(dash), n(-acc), n(cx), n(cy)
        ));
        acc += frac * circ;
    }
    out
}

fn second_opacity(_i: usize) -> f64 {
    0.0
}

fn kpi_card_svg(c: &KpiCard, x: f64, y: f64, w: f64, h: f64) -> String {
    let bg = c.bg.as_deref().unwrap_or("FFFFFF");
    let accent = c.accent.as_deref().unwrap_or("4472C4");
    let tc = c.text_color.as_deref().unwrap_or("1F2937");
    let r = if c.rounded.unwrap_or(true) { 12.0 } else { 1.0 };
    let mut out = format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{}\" stroke=\"#E5E7EB\" stroke-width=\"1\"/>",
        n(x), n(y), n(w), n(h), n(r), n(r), col(bg)
    );
    out.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"6\" height=\"{}\" fill=\"{}\"/>",
        n(x),
        n(y),
        n(h),
        col(accent)
    ));
    out.push_str(&format!(
        "<text x=\"{}\" y=\"{}\" font-size=\"32\" font-weight=\"bold\" fill=\"{}\">{}</text>",
        n(x + 18.0),
        n(y + h * 0.5 + 10.0),
        col(tc),
        esc(&c.value)
    ));
    if let Some(label) = &c.label {
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"13\" fill=\"#6B7280\">{}</text>",
            n(x + 18.0),
            n(y + h * 0.5 + 30.0),
            esc(label.as_str())
        ));
    }
    out
}

fn rating_stars_svg(c: &RatingStars, x: f64, y: f64, w: f64, h: f64) -> String {
    let max = c.max.unwrap_or(5);
    let r = c.rating.clamp(0.0, max as f64);
    let filled = c.color.as_deref().unwrap_or("F59E0B");
    let empty = c.empty_color.as_deref().unwrap_or("D1D5DB");
    let star_size = (h.min(w) / max as f64).clamp(10.0, 24.0);
    let gap = 4.0;
    let mut out = String::new();
    for i in 0..max {
        let sx = x + i as f64 * (star_size + gap);
        let sy = y + (h - star_size) / 2.0;
        let filled_now = (i as f64) < r;
        let colr = if filled_now { filled } else { empty };
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\">★</text>",
            n(sx),
            n(sy + star_size * 0.85),
            n(star_size),
            col(colr)
        ));
    }
    out
}

fn timeline_svg(c: &Timeline, x: f64, y: f64, w: f64, h: f64) -> String {
    let line_color = c.line_color.as_deref().unwrap_or("9CA3AF");
    let dot_color = c.dot_color.as_deref().unwrap_or("4472C4");
    let label_color = c.label_color.as_deref().unwrap_or("374151");
    let n_items = c.items.len().max(1);
    let step = w / n_items as f64;
    let mut out = format!(
        "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"2\"/>",
        n(x),
        n(y + h / 2.0),
        n(x + w),
        n(y + h / 2.0),
        col(line_color)
    );
    for (i, _item) in c.items.iter().enumerate() {
        let px = x + (i as f64 + 0.5) * step;
        out.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"6\" fill=\"{}\" stroke=\"#fff\" stroke-width=\"2\"/>",
            n(px),
            n(y + h / 2.0),
            col(dot_color)
        ));
    }
    let _ = label_color;
    out
}

fn process_flow_svg(c: &ProcessFlow, x: f64, y: f64, w: f64, h: f64) -> String {
    let cnt = c.steps.len().max(1);
    let gw = w / cnt as f64;
    let mut out = String::new();
    for (i, step) in c.steps.iter().enumerate() {
        let bx = x + i as f64 * gw;
        let color = c
            .colors
            .as_ref()
            .and_then(|cs| cs.get(i))
            .map(|s| col(s))
            .unwrap_or_else(|| "#4472C4".to_string());
        out.push_str(&format!(
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"8\" ry=\"8\" fill=\"{}\"/>",
            n(bx + 4.0),
            n(y + 4.0),
            n(gw - 8.0),
            n(h - 8.0),
            color
        ));
        out.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" font-size=\"13\" fill=\"#fff\">{}</text>",
            n(bx + gw / 2.0), n(y + h / 2.0 + 5.0), esc(step.as_str())
        ));
    }
    out
}

// === 组装 ===

fn render_slide_svg(
    slide: &Slide,
    width_in: f64,
    height_in: f64,
    scale: f64,
    resolve_media: &MediaResolver,
) -> Result<String, String> {
    // 坐标以 96dpi 为基准（1 英寸 = 96px），再通过 width/height 放大到 scale 倍
    let base_w = width_in * 96.0;
    let base_h = height_in * 96.0;
    let px_w = base_w * scale;
    let px_h = base_h * scale;

    let mut body = bg_svg(slide, base_w, base_h);
    body.push_str(&render_elements(slide, resolve_media));

    let svg = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">{}</svg>",
        n(px_w),
        n(px_h),
        n(base_w),
        n(base_h),
        body
    );
    Ok(svg)
}

fn render_elements(slide: &Slide, resolve_media: &MediaResolver) -> String {
    let mut out = String::new();
    for el in &slide.elements {
        out.push_str(&element_svg(el, resolve_media));
    }
    out
}

// === resvg 栅格化 ===

fn svg_to_png(svg: &str, out: &Path) -> Result<(), String> {
    use resvg::tiny_skia::Transform;
    use resvg::usvg::{fontdb, Options, Tree};

    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut opts = Options::default();
    opts.fontdb = std::sync::Arc::new(db);
    let tree = Tree::from_data(svg.as_bytes(), &opts).map_err(|e| e.to_string())?;
    let size = tree.size().to_int_size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| "无法创建位图".to_string())?;
    resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());
    pixmap.save_png(out).map_err(|e| format!("保存失败: {e}"))
}
