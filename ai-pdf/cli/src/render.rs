//! 渲染核对：产物目录 → 自包含 HTML → Chrome 截图 PNG / 导出 PDF。
//!
//! HTML 逐元素绝对定位（近似还原，用于 repack 前后目视核对）：
//! - 坐标 pt → px（96dpi，`scale` 可放大）；y 轴翻转（PDF y 向上 → CSS y 向下）；
//! - text/rect/image 用定位 div，path/polyline 用整页 SVG（y 翻转矩阵），
//!   shading 近似为 CSS 渐变（bbox/clip 不还原），pattern_rect 画占位底色；
//! - 图片转 base64 data URL，整文件自包含；
//! - group 子元素按「原始 y 向下」渲染，外层 wrapper 用
//!   `matrix(a,-b,c,-d, e·ppp, Hpx - f·ppp)` 精确映射组空间。

use json2pdf::model::{DocumentModel, Element, PageModel};
use office_core::{CliError, TempGuard};
use serde_json::json;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, CliError>;

/// 渲染为 HTML 文件（page_filter None = 全部页）
pub fn render_html(
    doc: &DocumentModel,
    pages: &[PageModel],
    root: &Path,
    page_filter: Option<usize>,
    out: &Path,
    scale: f64,
) -> Result<()> {
    let html = document_html(doc, pages, root, page_filter, scale);
    std::fs::write(out, html)
        .map_err(|e| CliError::with_code("io", format!("写 {} 失败: {e}", out.display())))?;
    Ok(())
}

/// 渲染为 PNG（Chrome 截图；仅单页）
pub fn render_png(
    doc: &DocumentModel,
    pages: &[PageModel],
    root: &Path,
    page_no: usize,
    out: &Path,
    scale: f64,
) -> Result<()> {
    let page = pages
        .get(page_no - 1)
        .ok_or_else(|| CliError::new(format!("页面索引越界: {page_no}")))?;
    let (w_px, h_px, _w, _h) = page_size_px(doc, page, scale);
    let html = document_html(doc, pages, root, Some(page_no), scale);
    let window = format!("{w_px}x{h_px}");
    chrome_capture(&html, out, &window, "--screenshot")
}

/// 渲染为 PDF（Chrome print-to-pdf；全部页）
pub fn render_pdf(
    doc: &DocumentModel,
    pages: &[PageModel],
    root: &Path,
    out: &Path,
    scale: f64,
) -> Result<()> {
    // @page 是文档级的：各页尺寸一致时单次打印；
    // 混合尺寸时逐页打印（每页一个 @page）再用 pages merge 合并
    let mut sizes = Vec::with_capacity(pages.len());
    for p in pages {
        let (_, _, w, h) = page_size_px(doc, p, scale);
        sizes.push((w, h));
    }
    let uniform = sizes
        .iter()
        .all(|s| (s.0 - sizes[0].0).abs() < 0.5 && (s.1 - sizes[0].1).abs() < 0.5);
    if uniform || pages.len() == 1 {
        let html = document_html(doc, pages, root, None, scale);
        return chrome_capture(&html, out, "0x0", "--print-to-pdf");
    }
    let mut parts: Vec<PathBuf> = Vec::new();
    for i in 1..=pages.len() {
        let part =
            std::env::temp_dir().join(format!("json2pdf-page-{}-{}.pdf", std::process::id(), i));
        let html = document_html(doc, pages, root, Some(i), scale);
        chrome_capture(&html, &part, "0x0", "--print-to-pdf")?;
        parts.push(part);
    }
    let r = json2pdf::page_ops::merge(&parts, out);
    for p in &parts {
        let _ = std::fs::remove_file(p);
    }
    r.map(|_| ())
        .map_err(|e| CliError::new(format!("合并逐页渲染结果失败: {e}")))
}

/// 页面可见区（打印态语义）：crop 优先，其次 mediabox，缺省整页。
/// 返回 (vx0, vy0, vw, vh)——元素坐标需平移 -(vx0, vy0)。
fn page_viewport(doc: &DocumentModel, page: &PageModel) -> (f64, f64, f64, f64) {
    let (pw, ph) = crate::product::page_size(doc, page);
    if let Some(c) = page.crop {
        (c[0], c[1], c[2] - c[0], c[3] - c[1])
    } else if let Some(m) = page.mediabox {
        (m[0], m[1], m[2] - m[0], m[3] - m[1])
    } else {
        (0.0, 0.0, pw, ph)
    }
}

fn page_size_px(doc: &DocumentModel, page: &PageModel, scale: f64) -> (i64, i64, f64, f64) {
    let (_, _, w, h) = page_viewport(doc, page);
    let ppp = px_per_pt(scale);
    ((w * ppp).round() as i64, (h * ppp).round() as i64, w, h)
}

fn px_per_pt(scale: f64) -> f64 {
    scale * 96.0 / 72.0
}

/// Chrome 无头执行；HTML 写入临时文件，结束后清理。
/// Chrome 截图/打印完成后进程常不退出：轮询产物，尺寸连续 3 次稳定即主动结束。
fn chrome_capture(html: &str, out: &Path, window: &str, mode: &str) -> Result<()> {
    let chrome = json2pdf::convert::find_chrome().ok_or_else(|| {
        CliError::new("未找到 Chrome/Chromium，无法渲染 PNG/PDF")
            .suggest("安装 Chrome 后重试，或用 -o preview.html 输出 HTML 直接查看")
    })?;
    let tmp = std::env::temp_dir().join(format!(
        "json2pdf-render-{}-{}.html",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _guard = TempGuard { path: tmp.clone() };
    std::fs::write(&tmp, html)
        .map_err(|e| CliError::with_code("io", format!("写临时 HTML 失败: {e}")))?;
    let url = format!(
        "file://{}",
        tmp.canonicalize().unwrap_or(tmp.clone()).display()
    );
    let out_str = out.display().to_string();
    let args = [
        "--headless".to_string(),
        "--disable-gpu".to_string(),
        "--no-sandbox".to_string(),
        format!("--window-size={window}"),
        if mode == "--screenshot" {
            "--hide-scrollbars".to_string()
        } else {
            "--no-pdf-header-footer".to_string()
        },
        format!("{mode}={out_str}"),
        url,
    ];

    // 先删旧产物：避免把上一次的结果误当本次输出
    let _ = std::fs::remove_file(out);
    let mut child = std::process::Command::new(&chrome)
        .args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| CliError::new(format!("执行 Chrome 失败: {e}")))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let poll = std::time::Duration::from_millis(150);
    let mut stable = 0u32;
    let mut last_size = 0u64;
    loop {
        if let Ok(meta) = std::fs::metadata(out) {
            let size = meta.len();
            if size > 0 {
                if size == last_size {
                    stable += 1;
                    if stable >= 3 {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Ok(());
                    }
                } else {
                    stable = 0;
                    last_size = size;
                }
            }
        }
        if let Ok(Some(status)) = child.try_wait() {
            // 进程自行退出：按退出码与产物判定
            return if status.success() && out.exists() {
                Ok(())
            } else {
                Err(CliError::new(format!(
                    "Chrome 渲染失败（退出码 {}）: {} 未生成",
                    status.code().unwrap_or(-1),
                    out.display()
                )))
            };
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return if out.exists() {
                Ok(())
            } else {
                Err(CliError::new("Chrome 渲染超时（120s）且未产出文件")
                    .suggest("改用 -o preview.html 输出 HTML 直接查看"))
            };
        }
        std::thread::sleep(poll);
    }
}

// === HTML 生成 ===

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn css_font(font: &str) -> String {
    if let Some(family) = font.strip_prefix("system:") {
        return format!("'{}', sans-serif", esc(family));
    }
    match font {
        f if f.starts_with("Times") => "'Times New Roman', Times, serif".to_string(),
        f if f.starts_with("Courier") => "'Courier New', Courier, monospace".to_string(),
        "Symbol" | "ZapfDingbats" => "serif".to_string(),
        f if f.starts_with("Helvetica") => "Helvetica, Arial, sans-serif".to_string(),
        other => format!("'{}', sans-serif", esc(other)),
    }
}

fn document_html(
    doc: &DocumentModel,
    pages: &[PageModel],
    root: &Path,
    page_filter: Option<usize>,
    scale: f64,
) -> String {
    let ppp = px_per_pt(scale);
    let mut body = String::new();
    let mut first_size = (0.0f64, 0.0f64);
    for (i, page) in pages.iter().enumerate() {
        let page_no = i + 1;
        if let Some(want) = page_filter {
            if want != page_no {
                continue;
            }
        }
        let (_, _, w, h) = page_size_px(doc, page, scale);
        let (vx0, vy0, _, _) = page_viewport(doc, page);
        if first_size.0 == 0.0 {
            first_size = (w, h);
        }
        body.push_str(&format!(
            "<div class=\"page\" style=\"width:{:.1}px;height:{:.1}px\">",
            w * ppp,
            h * ppp
        ));
        body.push_str(&elements_html(
            &page.elements,
            w,
            h,
            vx0,
            vy0,
            root,
            ppp,
            true,
        ));
        body.push_str(&format!(
            "<div class=\"tag\">page {page_no} / {}</div></div>\n",
            pages.len()
        ));
    }
    let (pw, ph) = if first_size.0 > 0.0 {
        first_size
    } else {
        (doc.page_size.width, doc.page_size.height)
    };
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>
@page {{ size:{:.1}px {:.1}px; margin:0; }}
body {{ margin:0; background:#e8e8ec; font-family:Helvetica,Arial,sans-serif; }}
.page {{ position:relative; margin:16px auto; background:#fff; box-shadow:0 1px 6px rgba(0,0,0,.25); overflow:hidden; }}
.page > * {{ position:absolute; }}
svg.vec {{ position:absolute; left:0; top:0; }}
.tag {{ position:absolute; left:6px; top:4px; font-size:10px; color:#bbb; user-select:none; }}
@media print {{ body {{ background:#fff; }} .page {{ margin:0; box-shadow:none; page-break-after:always; }} .page:last-child {{ page-break-after:auto; }} .tag {{ display:none; }} }}
</style></head><body>
{body}
</body></html>",
        pw * ppp,
        ph * ppp
    )
}

/// 元素列表 → HTML（flip = 页面坐标（y 向上翻转为屏幕坐标）；group 内部为 raw 模式）。
/// Tr≥4（文本参与裁剪）后紧跟覆盖它的实心矩形时，按「透过字形填充」近似：
/// 文字直接染矩形填充色并跳过该矩形（issue3584 全页蓝的根因）。
#[allow(clippy::too_many_arguments)]
fn elements_html(
    elements: &[Element],
    page_w: f64,
    page_h: f64,
    vx0: f64,
    vy0: f64,
    root: &Path,
    ppp: f64,
    flip: bool,
) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < elements.len() {
        let el = &elements[i];
        if let Element::Text {
            render_mode, x, y, ..
        } = el
        {
            if *render_mode >= 4 {
                let mut paired: Option<&String> = None;
                if let Some(Element::Rect {
                    fill: Some(fc),
                    x: rx,
                    y: ry,
                    w: rw,
                    h: rh,
                    ..
                }) = elements.get(i + 1)
                {
                    if *rx <= *x && *ry <= *y && rx + rw >= *x && ry + rh >= *y {
                        paired = Some(fc);
                    }
                }
                out.push_str(&text_html(
                    el,
                    page_w,
                    page_h,
                    vx0,
                    vy0,
                    ppp,
                    flip,
                    match paired {
                        Some(fc) => TextPaint::Fill(fc.clone()),
                        None => match render_mode {
                            5 => TextPaint::StrokeOnly,
                            _ => TextPaint::Hidden,
                        },
                    },
                ));
                if paired.is_some() {
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            // 普通文本（Tr 0-2 正常绘制；Tr 3 无填充无描边 = 不可见）
            let paint = if *render_mode == 3 {
                TextPaint::Hidden
            } else {
                TextPaint::Normal
            };
            out.push_str(&text_html(el, page_w, page_h, vx0, vy0, ppp, flip, paint));
            i += 1;
            continue;
        }
        out.push_str(&element_html(el, page_w, page_h, vx0, vy0, root, ppp, flip));
        i += 1;
    }
    out
}

/// 文本的呈现方式
enum TextPaint {
    /// 自身填充色（Tr 0-2 正常文本）
    Normal,
    /// 染成后随矩形的填充色（透过字形填充）
    Fill(String),
    /// 不可见（纯裁剪 Tr 7 / 无填充无描边 Tr 3）
    Hidden,
    /// 保留描边色（Tr=5）
    StrokeOnly,
}

/// y 坐标换算：flip 模式下屏幕 y = 页高 - pdf y；raw 模式直接用
fn fy(flip: bool, page_h: f64, y: f64) -> f64 {
    if flip {
        page_h - y
    } else {
        y
    }
}

#[allow(clippy::too_many_arguments)]
fn text_html(
    el: &Element,
    _page_w: f64,
    page_h: f64,
    vx0: f64,
    vy0: f64,
    ppp: f64,
    flip: bool,
    paint: TextPaint,
) -> String {
    if let Element::Text {
        text,
        x,
        y,
        size,
        font,
        color,
        stroke_color,
        rotation,
        alpha,
        ..
    } = el
    {
        let top = fy(flip, page_h, *y - vy0) - size;
        let color = match &paint {
            TextPaint::Normal => esc(color),
            TextPaint::Fill(fc) => esc(fc),
            TextPaint::Hidden => "transparent".to_string(),
            TextPaint::StrokeOnly => esc(stroke_color.as_deref().unwrap_or(color)),
        };
        return format!(
            "<div style=\"left:{:.1}px;top:{:.1}px;font-size:{:.1}px;font-family:{};color:{color};opacity:{};white-space:pre;transform-origin:0 100%;transform:rotate(-{:.1}deg)\">{}</div>",
            (x - vx0) * ppp,
            top * ppp,
            size * ppp,
            css_font(font),
            alpha,
            rotation,
            esc(text)
        );
    }
    String::new()
}

/// 渐变色标 → CSS stops（无色标的 mesh/func0 直通渐变兜底为中灰纯色，保证可见）
fn stops_css(stops: &[json2pdf::model::ShadingStop]) -> String {
    if stops.is_empty() {
        return "#7a7a7a".to_string();
    }
    stops
        .iter()
        .map(|s| format!("{} {:.0}%", esc(&s.color), s.offset * 100.0))
        .collect::<Vec<_>>()
        .join(", ")
}

#[allow(clippy::too_many_arguments)]
fn element_html(
    el: &Element,
    page_w: f64,
    page_h: f64,
    vx0: f64,
    vy0: f64,
    root: &Path,
    ppp: f64,
    flip: bool,
) -> String {
    match el {
        Element::Text { .. } => String::new(), // 由 elements_html 的配对逻辑统一走 text_html
        Element::Rect {
            x,
            y,
            w,
            h,
            fill,
            stroke,
            line_width,
            rotation,
            alpha,
            ..
        } => {
            let top = fy(flip, page_h, *y - vy0) - h;
            let mut style = format!(
                "left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;opacity:{}",
                (x - vx0) * ppp,
                top * ppp,
                w * ppp,
                h * ppp,
                alpha
            );
            if let Some(f) = fill {
                style.push_str(&format!(";background:{}", esc(f)));
            }
            if let Some(s) = stroke {
                style.push_str(&format!(
                    ";border:{:.1}px solid {}",
                    line_width * ppp,
                    esc(s)
                ));
            }
            if *rotation != 0.0 {
                style.push_str(&format!(
                    ";transform-origin:0 100%;transform:rotate(-{rotation:.1}deg)"
                ));
            }
            format!("<div style=\"{style}\"></div>")
        }
        Element::PatternRect { x, y, w, h, .. } => {
            // 平铺图案不还原：占位底色 + 虚线框
            let top = fy(flip, page_h, *y - vy0) - h;
            format!(
                "<div style=\"left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;background:#eef1f5;border:1px dashed #9aa7b4\"></div>",
                (x - vx0) * ppp,
                top * ppp,
                w * ppp,
                h * ppp
            )
        }
        Element::Image {
            src,
            preview,
            x,
            y,
            w,
            h,
            rotation,
            alpha,
            clip,
            ..
        } => {
            let top = fy(flip, page_h, *y - vy0) - h;
            let mut style = format!(
                "left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;opacity:{}",
                (x - vx0) * ppp,
                top * ppp,
                w * ppp,
                h * ppp,
                alpha
            );
            if *rotation != 0.0 {
                style.push_str(&format!(
                    ";transform-origin:0 0;transform:rotate(-{rotation:.1}deg)"
                ));
            }
            // 裁剪路径（设备空间多边形）→ CSS clip-path。
            // clip-path 坐标相对元素自身盒子：local = 设备坐标 − 元素原点
            if let Some(clip) = clip {
                let origin_y = if flip { y + h } else { *y };
                let pts: Vec<String> = clip
                    .iter()
                    .flat_map(|seg| seg.points.iter())
                    .map(|pt| {
                        let ly = if flip {
                            origin_y - (pt[1] - vy0)
                        } else {
                            pt[1] - vy0 - origin_y
                        };
                        format!("{:.1}px {:.1}px", (pt[0] - vx0 - x) * ppp, ly * ppp)
                    })
                    .collect();
                if pts.len() >= 3 {
                    style.push_str(&format!(";clip-path:polygon({})", pts.join(",")));
                }
            }
            // preview 是 unpack 对 JPX/JBIG2/CCITT 解出的预览 PNG，优先于原字节
            // （原字节浏览器解不了）；仍失败则落中性占位
            let shown = preview.as_deref().unwrap_or(src);
            format!(
                "<img src=\"{}\" style=\"{style}\" onerror=\"this.style.background='#e8eaed';this.style.border='1px dashed #9aa0a6'\">",
                data_url(root, shown)
            )
        }
        Element::Polyline {
            points,
            stroke,
            line_width,
            ..
        } => {
            let d: String = points
                .iter()
                .map(|p| format!("{:.2} {:.2} ", p[0], p[1]))
                .collect();
            let (sy, ty) = if flip {
                (-ppp, (page_h + vy0) * ppp)
            } else {
                (ppp, -vy0 * ppp)
            };
            format!(
                "<svg class=\"vec\" width=\"{:.1}\" height=\"{:.1}\" viewBox=\"0 0 {:.1} {:.1}\"><g transform=\"matrix({ppp:.4},0,0,{sy:.4},{:.1},{ty:.1})\"><polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\"/></g></svg>",
                page_w * ppp,
                page_h * ppp,
                page_w,
                page_h,
                -vx0 * ppp,
                d.trim_end(),
                esc(stroke),
                line_width * ppp
            )
        }
        Element::Path {
            segments,
            fill,
            stroke,
            line_width,
            even_odd,
            alpha,
            dash,
            ..
        } => {
            let mut d = String::new();
            for s in segments {
                match s.op.as_str() {
                    "m" if !s.points.is_empty() => {
                        d.push_str(&format!("M {:.2} {:.2} ", s.points[0][0], s.points[0][1]))
                    }
                    "l" if !s.points.is_empty() => {
                        d.push_str(&format!("L {:.2} {:.2} ", s.points[0][0], s.points[0][1]))
                    }
                    "c" if s.points.len() >= 3 => d.push_str(&format!(
                        "C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ",
                        s.points[0][0],
                        s.points[0][1],
                        s.points[1][0],
                        s.points[1][1],
                        s.points[2][0],
                        s.points[2][1]
                    )),
                    "h" => d.push_str("Z "),
                    _ => {}
                }
            }
            let fill_attr = match fill {
                Some(f) => format!("fill=\"{}\"", esc(f)),
                None => "fill=\"none\"".to_string(),
            };
            let dash_attr = dash
                .as_ref()
                .map(|v| {
                    format!(
                        "stroke-dasharray=\"{}\"",
                        v.iter()
                            .map(|x| format!("{x:.2}"))
                            .collect::<Vec<_>>()
                            .join(",")
                    )
                })
                .unwrap_or_default();
            let (sy, ty) = if flip {
                (-ppp, (page_h + vy0) * ppp)
            } else {
                (ppp, -vy0 * ppp)
            };
            format!(
                "<svg class=\"vec\" width=\"{:.1}\" height=\"{:.1}\" viewBox=\"0 0 {:.1} {:.1}\"><g transform=\"matrix({ppp:.4},0,0,{sy:.4},{:.1},{ty:.1})\"><path d=\"{}\" {fill_attr} stroke=\"{}\" stroke-width=\"{:.2}\" fill-rule=\"{}\" opacity=\"{alpha}\" {dash_attr}/></g></svg>",
                page_w * ppp,
                page_h * ppp,
                page_w,
                page_h,
                -vx0 * ppp,
                d.trim_end(),
                stroke.clone().unwrap_or_else(|| "none".into()),
                line_width * ppp,
                if *even_odd { "evenodd" } else { "nonzero" },
            )
        }
        Element::Shading(def) => {
            // 近似：CSS 渐变（bbox 区域；clip/mask/网格/采样函数不还原）。
            // mesh/func0 直通渐变无色标：有 bbox 画兜底灰，无 bbox 跳过（避免整页误刷）
            if def.stops.is_empty() && def.bbox.is_none() {
                return String::new();
            }
            let bbox = def.bbox.unwrap_or([vx0, vy0, page_w + vx0, page_h + vy0]);
            let (bx, bw, bh) = (bbox[0], bbox[2] - bbox[0], bbox[3] - bbox[1]);
            let top = fy(flip, page_h, bbox[1] - vy0) - bh;
            let grad = match def.kind.as_str() {
                "radial" => {
                    let cx = def.coords.first().copied().unwrap_or(0.0) - bbox[0];
                    let cy = def.coords.get(1).copied().unwrap_or(0.0) - bbox[1];
                    format!(
                        "background:radial-gradient(circle at {:.1}% {:.1}%, {})",
                        cx / bw.max(1e-6) * 100.0,
                        cy / bh.max(1e-6) * 100.0,
                        stops_css(&def.stops)
                    )
                }
                _ => {
                    let (x0, y0, x1, y1) = (
                        def.coords.first().copied().unwrap_or(0.0),
                        def.coords.get(1).copied().unwrap_or(0.0),
                        def.coords.get(2).copied().unwrap_or(0.0),
                        def.coords.get(3).copied().unwrap_or(0.0),
                    );
                    let dy = fy(flip, page_h, y1 - vy0) - fy(flip, page_h, y0 - vy0);
                    let ang = (-dy).atan2(x1 - x0).to_degrees() + 90.0;
                    format!(
                        "background:linear-gradient({ang:.1}deg, {})",
                        stops_css(&def.stops)
                    )
                }
            };
            format!(
                "<div style=\"left:{:.1}px;top:{:.1}px;width:{:.1}px;height:{:.1}px;opacity:{};{grad}\"></div>",
                bx * ppp,
                top * ppp,
                bw * ppp,
                bh * ppp,
                def.alpha
            )
        }
        Element::Group {
            matrix,
            children,
            alpha,
            ..
        } => {
            // 子元素以 raw 模式（组空间坐标直出），wrapper 矩阵映射到页面：
            // PDF: px = a·u + c·v + e, py = b·u + d·v + f；屏幕 sx = px·ppp，sy = Hpx − py·ppp
            // transform-origin 必须为 0 0（CSS 缺省 50% 50% 会让矩阵绕中心、整体错位）
            let [a, b, c, d, e, f] = *matrix;
            let inner = elements_html(children, page_w, page_h, 0.0, 0.0, root, ppp, false);
            format!(
                "<div style=\"left:0;top:0;width:100%;height:100%;opacity:{alpha};transform-origin:0 0;transform:matrix({:.4},{:.4},{:.4},{:.4},{:.1},{:.1})\">{}</div>",
                a,
                -b,
                c,
                -d,
                (e - vx0) * ppp,
                fy(flip, page_h, f - vy0) * ppp,
                inner
            )
        }
    }
}

/// 图片 → data URL（自包含 HTML）
fn data_url(root: &Path, src: &str) -> String {
    let path = if Path::new(src).is_absolute() {
        PathBuf::from(src)
    } else {
        root.join(src)
    };
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    };
    match std::fs::read(&path) {
        Ok(bytes) => {
            use base64::Engine as _;
            format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )
        }
        Err(_) => String::new(),
    }
}

/// 渲染结果摘要（--json）
pub fn summary(out: &Path, format: &str, page: Option<usize>) -> serde_json::Value {
    let mut v = json!({
        "output": out.display().to_string(),
        "format": format,
    });
    if let Some(p) = page {
        v["page"] = json!(p);
    }
    v
}
