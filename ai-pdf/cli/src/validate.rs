//! 结构校验：对产物目录做静态检查，问题非空时报告（error 级影响退出码 3）。
//!
//! 检查：页面文件存在与可解析、rotation 合法、颜色格式（#RRGGBB）、
//! 图片 src 存在、font_id 引用存在、文本为空、元素越界。

use json2pdf::model::{DocumentModel, Element, PageModel};
use office_core::CliError;
use serde_json::json;
use std::path::Path;

use crate::path;
use crate::product;

/// 校验入口：宽松加载（损坏页面报告为 error 而不是命令失败）
pub fn run_on_dir(root: &Path) -> Result<Vec<serde_json::Value>, CliError> {
    let doc = product::load_document(root)?;
    let mut page_opts: Vec<Option<PageModel>> = Vec::with_capacity(doc.pages.len());
    let mut out = Vec::new();
    for (i, rel) in doc.pages.iter().enumerate() {
        match std::fs::read_to_string(root.join(rel))
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str::<PageModel>(&t).map_err(|e| e.to_string()))
        {
            Ok(p) => page_opts.push(Some(p)),
            Err(e) => {
                let mut issues = Vec::new();
                push(
                    &mut issues,
                    &format!("/page[{}]", i + 1),
                    "error",
                    format!("页面文件读取/解析失败: {rel}: {e}"),
                );
                out.extend(issues);
                page_opts.push(None);
            }
        }
    }
    out.extend(run(&doc, &page_opts, root));
    Ok(out)
}

pub fn run(
    doc: &DocumentModel,
    page_opts: &[Option<PageModel>],
    product_root: &Path,
) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    let font_ids: Vec<String> = doc.fonts.to_vec();
    for (i, page) in page_opts.iter().enumerate() {
        let page_no = i + 1;
        let prefix = format!("/page[{page_no}]");
        let Some(page) = page else {
            continue; // 加载失败已在 run_on_dir 报告
        };
        let (w, h) = product::page_size(doc, page);
        if !matches!(page.rotation, 0 | 90 | 180 | 270) {
            push(
                &mut out,
                &prefix,
                "error",
                format!("rotation 仅支持 0/90/180/270，实际 {}", page.rotation),
            );
        }
        check_elements(
            &page.elements,
            &prefix,
            w,
            h,
            product_root,
            &font_ids,
            &mut out,
        );
    }
    out
}

fn check_elements(
    elements: &[Element],
    prefix: &str,
    w: f64,
    h: f64,
    root: &Path,
    font_ids: &[String],
    out: &mut Vec<serde_json::Value>,
) {
    let mut counters: std::collections::HashMap<&str, usize> = Default::default();
    for el in elements {
        let t = path::type_name(el);
        let c = counters.entry(t).or_insert(0);
        *c += 1;
        let p = format!("{prefix}/{t}[{}]", *c);

        match el {
            Element::Text {
                text,
                color,
                font_id,
                x,
                y,
                size,
                ..
            } => {
                if text.trim().is_empty() {
                    push(out, &p, "info", "文本元素内容为空".to_string());
                }
                check_color(out, &p, "color", Some(color.as_str()));
                if let Some(fid) = font_id {
                    if !font_ids.iter().any(|f| f == fid) {
                        push(
                            out,
                            &p,
                            "warning",
                            format!("font_id \"{fid}\" 不在 document.json 的 fonts 清单中（repack 将回退系统字体）"),
                        );
                    }
                }
                let top = y + size;
                if *x < -0.5 || top > h + 0.5 || *y < -0.5 || *x > w + 0.5 {
                    push(
                        out,
                        &p,
                        "warning",
                        format!("文本基线位置 ({x:.1},{y:.1}) 超出页面 {w:.1}x{h:.1}"),
                    );
                }
            }
            Element::Rect {
                x,
                y,
                w: rw,
                h: rh,
                fill,
                stroke,
                ..
            } => {
                check_color(out, &p, "fill", fill.as_deref());
                check_color(out, &p, "stroke", stroke.as_deref());
                check_bounds(out, &p, *x, *y, *rw, *rh, w, h);
            }
            Element::PatternRect {
                x, y, w: rw, h: rh, ..
            } => {
                check_bounds(out, &p, *x, *y, *rw, *rh, w, h);
            }
            Element::Image {
                src,
                x,
                y,
                w: iw,
                h: ih,
                ..
            } => {
                if src.trim().is_empty() {
                    push(out, &p, "error", "图片元素缺少 src".to_string());
                } else if !src.starts_with("http")
                    && !root.join(src).exists()
                    && !Path::new(src).exists()
                {
                    push(
                        out,
                        &p,
                        "warning",
                        format!("图片文件不存在: {src}（repack 将以占位图代替）"),
                    );
                }
                check_bounds(out, &p, *x, *y, *iw, *ih, w, h);
            }
            Element::Polyline { points, stroke, .. } => {
                check_color(out, &p, "stroke", Some(stroke));
                if points.len() < 2 {
                    push(out, &p, "warning", "折线少于 2 个顶点".to_string());
                }
            }
            Element::Path {
                segments,
                fill,
                stroke,
                ..
            } => {
                check_color(out, &p, "fill", fill.as_deref());
                check_color(out, &p, "stroke", stroke.as_deref());
                if segments.is_empty() {
                    push(out, &p, "warning", "路径没有线段".to_string());
                }
            }
            Element::Shading(d) => {
                if d.stops.is_empty() {
                    push(out, &p, "warning", "渐变没有色标".to_string());
                }
                for (i, s) in d.stops.iter().enumerate() {
                    check_color(out, &p, &format!("stops[{i}].color"), Some(&s.color));
                }
            }
            Element::Group { children, bbox, .. } => {
                if bbox[2] <= bbox[0] || bbox[3] <= bbox[1] {
                    push(out, &p, "warning", "透明组包围盒为空".to_string());
                }
                check_elements(children, &p, w, h, root, font_ids, out);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_bounds(
    out: &mut Vec<serde_json::Value>,
    p: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    pw: f64,
    ph: f64,
) {
    let tol = 0.5;
    if w <= 0.0 || h <= 0.0 {
        push(out, p, "warning", "元素宽或高为 0".to_string());
    }
    if x < -tol || y < -tol || x + w > pw + tol || y + h > ph + tol {
        push(
            out,
            p,
            "warning",
            format!("元素超出页面（{x:.1},{y:.1} {w:.1}x{h:.1} > {pw:.1}x{ph:.1}）"),
        );
    }
}

fn check_color(out: &mut Vec<serde_json::Value>, p: &str, field: &str, color: Option<&str>) {
    if let Some(c) = color {
        let ok =
            c.starts_with('#') && c.len() == 7 && c[1..].chars().all(|ch| ch.is_ascii_hexdigit());
        if !ok {
            push(
                out,
                p,
                "warning",
                format!("{field} 颜色格式应为 #RRGGBB（带 #），实际 \"{c}\""),
            );
        }
    }
}

fn push(out: &mut Vec<serde_json::Value>, path: &str, severity: &str, message: String) {
    out.push(json!({ "path": path, "severity": severity, "message": message }));
}
