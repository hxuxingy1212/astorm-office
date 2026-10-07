//! render：文档预览（HTML）。无外部依赖；如需 PDF 可后续接入 soffice。

use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use json2docx::model::blocks::*;
use json2docx::model::Document;
use std::fmt::Write as _;
use std::path::Path;

/// 将指定分片渲染为 HTML 预览
///
/// `root` 为产物根目录，用于把相对 `src` 解析为真实图片并内嵌为 data URI。
pub fn render_html(doc: &Document, part_idx: usize, root: Option<&Path>, out: &Path) -> Result<()> {
    let part = &doc.parts[part_idx];
    let header = part.section.as_ref().and_then(|s| s.header.clone());
    let mut body = String::new();
    for b in &part.blocks {
        write_block(&mut body, b, root);
    }
    write_html(doc, &body, header.as_deref(), out)
}

/// 渲染整篇文档（所有分片）为 HTML 预览
pub fn render_all(doc: &Document, root: Option<&Path>, out: &Path) -> Result<()> {
    let header = doc
        .parts
        .iter()
        .find_map(|p| p.section.as_ref().and_then(|s| s.header.clone()));
    let mut body = String::new();
    for part in &doc.parts {
        for b in &part.blocks {
            write_block(&mut body, b, root);
        }
    }
    write_html(doc, &body, header.as_deref(), out)
}

fn write_html(doc: &Document, body: &str, header: Option<&str>, out: &Path) -> Result<()> {
    let watermark = doc
        .watermark
        .as_ref()
        .map(|w| format!("<div class=\"watermark\">{}</div>", html_escape(w)))
        .unwrap_or_default();
    let header_html = header
        .filter(|h| !h.is_empty())
        .map(|h| format!("<div class=\"docheader\">{}</div>", html_escape(h)))
        .unwrap_or_default();
    let html = format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>{css}</style></head><body>{watermark}{header}{body}</body></html>",
        title = html_escape(doc.meta.title.as_deref().unwrap_or("document")),
        css = CSS,
        watermark = watermark,
        header = header_html,
        body = body
    );
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(out, html)
        .map_err(|e| CliError::new(format!("写入 {} 失败: {e}", out.display())))?;
    Ok(())
}

/// 段落内联渲染（含 run 级格式）
fn inline_runs(p: &json2docx::model::Paragraph) -> String {
    match &p.runs {
        Some(runs) if !runs.is_empty() => {
            let mut s = String::new();
            for r in runs {
                let mut t = if r.symbol.is_some() {
                    String::new()
                } else if let Some(img) = &r.image {
                    format!(
                        "<img alt=\"{}\" style=\"height:1em\"/>",
                        html_escape(img.alt.as_deref().unwrap_or(""))
                    )
                } else {
                    html_escape(&r.text)
                };
                if t.is_empty() && r.image.is_none() {
                    continue;
                }
                let mut st = String::new();
                if r.bold == Some(true) {
                    st.push_str("font-weight:bold;");
                }
                if r.italic == Some(true) {
                    st.push_str("font-style:italic;");
                }
                if r.underline == Some(true) {
                    st.push_str("text-decoration:underline;");
                }
                if r.strike == Some(true) {
                    st.push_str("text-decoration:line-through;");
                }
                if let Some(c) = &r.color {
                    st.push_str(&format!("color:#{c};"));
                }
                if let Some(sz) = r.font_size {
                    st.push_str(&format!("font-size:{sz}pt;"));
                }
                if let Some(f) = &r.font_family {
                    st.push_str(&format!("font-family:'{f}';"));
                }
                if r.superscript == Some(true) {
                    st.push_str("vertical-align:super;font-size:smaller;");
                }
                if r.subscript == Some(true) {
                    st.push_str("vertical-align:sub;font-size:smaller;");
                }
                if r.rtl == Some(true) {
                    st.push_str("direction:rtl;");
                }
                let styled = if st.is_empty() {
                    t
                } else {
                    format!("<span style=\"{st}\">{t}</span>")
                };
                t = styled;
                if let Some(url) = &r.hyperlink {
                    t = format!("<a href=\"{}\">{t}</a>", html_escape(url));
                }
                s.push_str(&t);
            }
            s
        }
        _ => html_escape(&paragraph_text(p)),
    }
}

fn write_block(out: &mut String, b: &Block, root: Option<&Path>) {
    match b {
        Block::Heading { level, text, .. } => {
            let l = (*level).clamp(1, 6);
            let _ = writeln!(out, "<h{l}>{}</h{l}>", html_escape(text));
        }
        Block::Paragraph(p) => {
            let mut style = String::new();
            if let Some(a) = &p.align {
                style.push_str(&format!(
                    "text-align:{};",
                    match a.as_str() {
                        "justify" | "both" => "justify",
                        "center" => "center",
                        "right" => "right",
                        _ => "left",
                    }
                ));
            }
            if let Some(v) = p.first_line_indent {
                style.push_str(&format!("text-indent:{}pt;", v));
            }
            if let Some(v) = p.indent {
                style.push_str(&format!("margin-left:{}pt;", v));
            }
            if let Some(v) = p.font_size {
                style.push_str(&format!("font-size:{}pt;", v));
            }
            if let Some(c) = &p.color {
                style.push_str(&format!("color:#{};", c));
            }
            let style = if style.is_empty() {
                String::new()
            } else {
                format!(" style=\"{style}\"")
            };
            let _ = writeln!(out, "<p{style}>{}</p>", inline_runs(p));
        }
        Block::List(l) => {
            let tag = if l.ordered { "ol" } else { "ul" };
            let _ = writeln!(out, "<{tag}>");
            for it in &l.items {
                let text = it
                    .blocks
                    .iter()
                    .map(|b| html_escape(&b.plain_text()))
                    .collect::<Vec<_>>()
                    .join(" ");
                let _ = writeln!(out, "<li>{text}</li>");
            }
            let _ = writeln!(out, "</{tag}>");
        }
        Block::Table(t) => {
            let _ = writeln!(out, "<table>");
            for (ri, r) in t.rows.iter().enumerate() {
                let _ = writeln!(out, "<tr>");
                for c in &r.cells {
                    let text = c
                        .blocks
                        .iter()
                        .map(|b| html_escape(&b.plain_text()))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let tag = if ri == 0 && t.header_row { "th" } else { "td" };
                    let mut attrs = String::new();
                    if let Some(cs) = c.colspan {
                        if cs > 1 {
                            attrs.push_str(&format!(" colspan=\"{cs}\""));
                        }
                    }
                    if let Some(rs) = c.rowspan {
                        if rs > 1 {
                            attrs.push_str(&format!(" rowspan=\"{rs}\""));
                        }
                    }
                    let style = c
                        .fill
                        .as_ref()
                        .map(|f| format!(" style=\"background:#{f}\""))
                        .unwrap_or_default();
                    let _ = writeln!(out, "<{tag}{attrs}{style}>{text}</{tag}>");
                }
                let _ = writeln!(out, "</tr>");
            }
            let _ = writeln!(out, "</table>");
            if let Some(cap) = &t.caption {
                let _ = writeln!(out, "<p class=\"cap\">{}</p>", html_escape(cap));
            }
        }
        Block::Image(i) => {
            let src = resolve_image(&i.src, root);
            let _ = writeln!(
                out,
                "<figure><img src=\"{}\" alt=\"{}\" style=\"max-width:80%\"/></figure>",
                html_escape(&src),
                html_escape(i.alt.as_deref().unwrap_or(""))
            );
            if let Some(cap) = &i.caption {
                let _ = writeln!(out, "<p class=\"cap\">{}</p>", html_escape(cap));
            }
        }
        Block::Formula(f) => {
            let _ = writeln!(
                out,
                "<div class=\"math\">{}</div>",
                html_escape(&latex_preview(&f.latex))
            );
        }
        Block::Code(c) => {
            let _ = writeln!(out, "<pre>{}</pre>", html_escape(&c.text));
        }
        Block::Quote(q) => {
            let _ = writeln!(out, "<blockquote>{}</blockquote>", html_escape(&q.text));
        }
        Block::Toc(_) => {
            // 目录标题由随附的 Heading 块渲染；此处仅占位（live 域在预览中无页码）
        }
        Block::Bibliography(b) => {
            let _ = writeln!(out, "<h1>参考文献</h1>");
            for e in &b.entries {
                let mut line = String::new();
                if !e.authors.is_empty() {
                    line.push_str(&e.authors.join(", "));
                    line.push_str(". ");
                }
                line.push_str(&e.title);
                if let Some(y) = e.year {
                    line.push_str(&format!(", {y}"));
                }
                let _ = writeln!(out, "<p class=\"ref\">{}</p>", html_escape(&line));
            }
        }
        Block::Caption(c) => {
            let _ = writeln!(out, "<p class=\"cap\">{}</p>", html_escape(&c.text));
        }
        Block::Chart(ch) => {
            let _ = writeln!(out, "{}", chart_svg(ch));
        }
        Block::TextBox(t) => {
            let style = format!(
                "border:1px solid #{};padding:8px;background:#{}",
                t.line.as_deref().unwrap_or("999999"),
                t.fill.as_deref().unwrap_or("FFFFFF")
            );
            let _ = writeln!(
                out,
                "<div class=\"textbox\" style=\"{}\">{}</div>",
                style,
                html_escape(&t.text)
            );
        }
        Block::Raw(_) => {
            let _ = writeln!(out, "<p class=\"raw\">[未建模内容已原样保留]</p>");
        }
        Block::Sdt(c) => {
            for b in &c.blocks {
                write_block(out, b, root);
            }
        }
        Block::Shape(sh) => {
            let _ = writeln!(
                out,
                "<p class=\"shape\">⬛ {}{}</p>",
                html_escape(&sh.shape_type),
                sh.text
                    .as_deref()
                    .map(|t| format!(" — {}", html_escape(t)))
                    .unwrap_or_default()
            );
        }
        Block::Attachment(a) => {
            let name = a.name.clone().unwrap_or_else(|| a.src.clone());
            let _ = writeln!(
                out,
                "<p class=\"attach\">📎 {}{}</p>",
                html_escape(&name),
                a.label
                    .as_deref()
                    .map(|l| format!(" — {}", html_escape(l)))
                    .unwrap_or_default()
            );
        }
        Block::PageBreak => {
            let _ = writeln!(out, "<hr/>");
        }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const PALETTE: &[&str] = &[
    "#4472C4", "#ED7D31", "#A5A5A5", "#FFC000", "#5B9BD5", "#70AD47",
];

/// LaTeX 的 Unicode 近似（用于 HTML 预览）
fn latex_preview(s: &str) -> String {
    const MAP: &[(&str, &str)] = &[
        ("\\alpha", "α"),
        ("\\beta", "β"),
        ("\\gamma", "γ"),
        ("\\delta", "δ"),
        ("\\epsilon", "ε"),
        ("\\lambda", "λ"),
        ("\\mu", "μ"),
        ("\\pi", "π"),
        ("\\rho", "ρ"),
        ("\\sigma", "σ"),
        ("\\tau", "τ"),
        ("\\phi", "φ"),
        ("\\omega", "ω"),
        ("\\theta", "θ"),
        ("\\Delta", "Δ"),
        ("\\Sigma", "Σ"),
        ("\\Omega", "Ω"),
        ("\\sum", "∑"),
        ("\\prod", "∏"),
        ("\\int", "∫"),
        ("\\infty", "∞"),
        ("\\cdot", "⋅"),
        ("\\times", "×"),
        ("\\div", "÷"),
        ("\\pm", "±"),
        ("\\le", "≤"),
        ("\\leq", "≤"),
        ("\\ge", "≥"),
        ("\\geq", "≥"),
        ("\\neq", "≠"),
        ("\\approx", "≈"),
        ("\\to", "→"),
        ("\\rightarrow", "→"),
        ("\\partial", "∂"),
        ("\\nabla", "∇"),
        ("\\in", "∈"),
        ("\\sqrt", "√"),
        ("\\left", ""),
        ("\\right", ""),
        ("\\,", " "),
        ("\\;", " "),
        ("\\!", ""),
    ];
    let mut out = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            let start = i;
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
                i += 1;
            }
            let cmd = &s[start..i];
            let mut replaced = false;
            for (k, v) in MAP {
                if *k == cmd {
                    out.push_str(v);
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                // 未知命令：去掉反斜杠只留名字
                out.push_str(&cmd[1..]);
            }
        } else {
            let ch = s[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// 用内联 SVG 近似渲染图表，便于图像核对
fn chart_svg(ch: &ChartBlock) -> String {
    let kind = ch.chart_type.to_ascii_lowercase();
    let title = ch.title.clone().unwrap_or_else(|| ch.chart_type.clone());
    let values: Vec<f64> = ch
        .series
        .first()
        .map(|s| s.values.clone())
        .unwrap_or_default();
    let cats = &ch.categories;
    let max = values.iter().cloned().fold(0.0f64, f64::max).max(1.0);
    let (w, h) = (440.0f64, 220.0f64);
    let (x0, y0, x1, y1) = (40.0, 24.0, 420.0, 184.0);

    let svg = match kind.as_str() {
        "pie" | "doughnut" => pie_svg(&values, cats, w, h),
        "line" | "area" => {
            let n = values.len().max(1);
            let step = (x1 - x0) / (n.saturating_sub(1).max(1) as f64);
            let mut pts = String::new();
            for (i, v) in values.iter().enumerate() {
                let x = x0 + step * i as f64;
                let y = y1 - (v / max) * (y1 - y0);
                pts.push_str(&format!("{x:.1},{y:.1} "));
            }
            let line = if kind == "area" {
                format!(
                    "<polygon points=\"{x0:.1},{y1:.1} {pts}{x1:.1},{y1:.1}\" fill=\"#4472C4\" fill-opacity=\"0.25\"/>\
                     <polyline points=\"{pts}\" fill=\"none\" stroke=\"#4472C4\" stroke-width=\"2\"/>"
                )
            } else {
                format!("<polyline points=\"{pts}\" fill=\"none\" stroke=\"#4472C4\" stroke-width=\"2\"/>")
            };
            format!("{}{line}", axes(x0, y0, x1, y1))
        }
        "bar" => horizontal_bars(&values, cats, x0, y0, x1, y1, max),
        _ => column_bars(&values, cats, x0, y0, x1, y1, max),
    };

    format!(
        "<figure class=\"chart\"><figcaption>{}</figcaption>\
         <svg viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\" role=\"img\">{svg}</svg></figure>",
        html_escape(&title)
    )
}

fn axes(x0: f64, y0: f64, x1: f64, y1: f64) -> String {
    format!(
        "<line x1=\"{x0}\" y1=\"{y0}\" x2=\"{x0}\" y2=\"{y1}\" stroke=\"#999\"/>\
         <line x1=\"{x0}\" y1=\"{y1}\" x2=\"{x1}\" y2=\"{y1}\" stroke=\"#999\"/>"
    )
}

fn column_bars(v: &[f64], cats: &[String], x0: f64, y0: f64, x1: f64, y1: f64, max: f64) -> String {
    let mut s = axes(x0, y0, x1, y1);
    let n = v.len().max(1);
    let slot = (x1 - x0) / n as f64;
    for (i, val) in v.iter().enumerate() {
        let bh = (val / max) * (y1 - y0);
        let x = x0 + slot * i as f64 + slot * 0.2;
        let bw = slot * 0.6;
        let y = y1 - bh;
        s.push_str(&format!(
            "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{bw:.1}\" height=\"{bh:.1}\" fill=\"#4472C4\"/>"
        ));
        if let Some(c) = cats.get(i) {
            s.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"10\" text-anchor=\"middle\">{}</text>",
                x + bw / 2.0,
                y1 + 12.0,
                html_escape(c)
            ));
        }
    }
    s
}

fn horizontal_bars(
    v: &[f64],
    cats: &[String],
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    max: f64,
) -> String {
    let mut s = axes(x0, y0, x1, y1);
    let n = v.len().max(1);
    let slot = (y1 - y0) / n as f64;
    for (i, val) in v.iter().enumerate() {
        let bw = (val / max) * (x1 - x0);
        let y = y0 + slot * i as f64 + slot * 0.2;
        let bh = slot * 0.6;
        s.push_str(&format!(
            "<rect x=\"{x0}\" y=\"{y:.1}\" width=\"{bw:.1}\" height=\"{bh:.1}\" fill=\"#4472C4\"/>"
        ));
        if let Some(c) = cats.get(i) {
            s.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"10\" text-anchor=\"end\">{}</text>",
                x0 - 4.0,
                y + bh * 0.8,
                html_escape(c)
            ));
        }
    }
    s
}

fn pie_svg(v: &[f64], cats: &[String], w: f64, h: f64) -> String {
    let total: f64 = v.iter().sum::<f64>().max(1.0);
    let (cx, cy, r) = (w / 2.0, h / 2.0, 80.0f64);
    let mut angle = -std::f64::consts::FRAC_PI_2;
    let mut s = String::new();
    for (i, val) in v.iter().enumerate() {
        let frac = val / total;
        let a2 = angle + frac * std::f64::consts::TAU;
        let large = if frac > 0.5 { 1 } else { 0 };
        let (x1, y1) = (cx + r * angle.cos(), cy + r * angle.sin());
        let (x2, y2) = (cx + r * a2.cos(), cy + r * a2.sin());
        let color = PALETTE[i % PALETTE.len()];
        s.push_str(&format!(
            "<path d=\"M {cx:.1} {cy:.1} L {x1:.1} {y1:.1} A {r:.1} {r:.1} 0 {large} 1 {x2:.1} {y2:.1} Z\" fill=\"{color}\"/>"
        ));
        if let Some(c) = cats.get(i) {
            let mid = (angle + a2) / 2.0;
            s.push_str(&format!(
                "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"9\" fill=\"#fff\" text-anchor=\"middle\">{}</text>",
                cx + r * 0.6 * mid.cos(),
                cy + r * 0.6 * mid.sin(),
                html_escape(c)
            ));
        }
        angle = a2;
    }
    s
}

/// 查找可执行文件（返回路径）
pub fn which(name: &str) -> Option<String> {
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

/// 用 LibreOffice 把 docx 导出为 PDF/PNG
pub fn convert_with_soffice(docx: &Path, out: &Path) -> Result<()> {
    let so = which("soffice")
        .or_else(|| which("libreoffice"))
        .ok_or_else(|| {
            "未找到 LibreOffice（soffice/libreoffice）；请安装后重试，或 `-o *.html` 输出网页预览"
                .to_string()
        })?;
    let fmt = match out.extension().and_then(|e| e.to_str()) {
        Some("png") => "png",
        Some("pdf") => "pdf",
        _ => "pdf",
    };
    let outdir = match out.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    let status = std::process::Command::new(&so)
        .args(["--headless", "--convert-to", fmt, "--outdir"])
        .arg(&outdir)
        .arg(docx)
        .status()
        .map_err(|e| CliError::new(format!("执行 soffice 失败: {e}")))?;
    if !status.success() {
        return Err(CliError::new(format!("soffice 转换失败（{fmt}）")));
    }
    let stem = docx
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "out".to_string());
    let produced = outdir.join(format!("{stem}.{fmt}"));
    if produced != out {
        if produced.exists() {
            std::fs::rename(&produced, out)
                .or_else(|_| std::fs::copy(&produced, out).map(|_| ()))
                .map_err(|e| CliError::new(format!("移动结果失败: {e}")))?;
        } else if !out.exists() {
            return Err(CliError::new(format!(
                "未找到 soffice 输出：{}",
                produced.display()
            )));
        }
    }
    Ok(())
}

/// 用无头 Chrome 把 HTML 打印为 PDF
pub fn html_to_pdf_chrome(html: &Path, out: &Path) -> Result<()> {
    let chrome = find_chrome()?;
    let html_abs = html
        .canonicalize()
        .map_err(|e| CliError::new(format!("定位 HTML 失败: {e}")))?;
    let url = format!("file://{}", html_abs.display());
    let status = std::process::Command::new(chrome)
        .args(["--headless", "--disable-gpu"])
        .arg(format!("--print-to-pdf={}", out.display()))
        .arg(&url)
        .status()
        .map_err(|e| CliError::new(format!("执行 Chrome 失败: {e}")))?;
    if !status.success() {
        return Err(CliError::new("Chrome 打印 PDF 失败"));
    }
    Ok(())
}

fn find_chrome() -> Result<String> {
    which("google-chrome")
        .or_else(|| which("chromium"))
        .or_else(|| which("chromium-browser"))
        .or_else(|| which("chrome"))
        .or_else(|| {
            let mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
            if std::path::Path::new(mac).exists() {
                Some(mac.to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| CliError::new("未找到 Chrome/Chromium；无法通过网页导出"))
}

/// 用无头 Chrome 把 HTML 截图为 PNG
pub fn html_to_png_chrome(html: &Path, out: &Path) -> Result<()> {
    let chrome = find_chrome()?;
    let html_abs = html
        .canonicalize()
        .map_err(|e| CliError::new(format!("定位 HTML 失败: {e}")))?;
    let url = format!("file://{}", html_abs.display());
    let status = std::process::Command::new(chrome)
        .args([
            "--headless",
            "--disable-gpu",
            "--hide-scrollbars",
            "--force-device-scale-factor=1.5",
            "--window-size=1200,4000",
        ])
        .arg(format!("--screenshot={}", out.display()))
        .arg(&url)
        .status()
        .map_err(|e| CliError::new(format!("执行 Chrome 失败: {e}")))?;
    if !status.success() {
        return Err(CliError::new("Chrome 截图失败"));
    }
    Ok(())
}

/// 把图片 src 解析为可内嵌的来源：URL 原样；本地文件读为 base64 data URI
fn resolve_image(src: &str, root: Option<&Path>) -> String {
    if src.starts_with("http://") || src.starts_with("https://") {
        return src.to_string();
    }
    if let Some(r) = root {
        if let Ok(bytes) = std::fs::read(r.join(src)) {
            return format!("data:{};base64,{}", mime_of(src), base64(&bytes));
        }
    }
    src.to_string()
}

fn mime_of(src: &str) -> &'static str {
    let lower = src.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else {
        "image/png"
    }
}

/// 标准 base64 编码（无外部依赖）
fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        s.push(T[((n >> 18) & 63) as usize] as char);
        s.push(T[((n >> 12) & 63) as usize] as char);
        s.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        s.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    s
}

const CSS: &str = "body{font-family:'Times New Roman',serif;max-width:720px;margin:40px auto;line-height:1.6;padding:0 16px;color:#222}\
h1{font-size:1.6em;border-bottom:1px solid #ddd;padding-bottom:4px}\
table{border-collapse:collapse;width:100%;margin:12px 0}\
td,th{border:1px solid #999;padding:6px 10px;text-align:left}\
.cap{text-align:center;font-style:italic;font-size:.9em;color:#555}\
.math{text-align:center;font-family:Cambria Math,serif;margin:12px 0}\
pre{background:#f5f5f5;padding:10px;overflow:auto}\
blockquote{border-left:3px solid #ccc;margin:12px 0;padding-left:12px;color:#555}\
.ref{padding-left:2em;text-indent:-2em}\
.watermark{position:fixed;top:45%;left:50%;transform:translate(-50%,-50%) rotate(-30deg);font-size:90px;color:rgba(192,192,192,.35);z-index:999;pointer-events:none;white-space:nowrap}\
.docheader{text-align:center;color:#333;border-bottom:1px solid #ddd;padding-bottom:4px;margin-bottom:16px;font-size:12px}\
.chart{text-align:center}\
.chart figcaption{font-size:.9em;color:#555}";
