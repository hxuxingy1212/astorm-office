//! 视图层：text / layout（对齐 ai-ppt/cli 的 view.rs）
//!
//! - text：文本 + 媒体（去图形），每项带 CLI 路径可直接 edit
//! - layout：元素布局树（类型/几何/文字摘要/CLI 路径），group 含 children

use json2pdf::model::{Element, PageModel};
use serde_json::json;
use std::path::Path;

use crate::path;
use crate::product;

/// 递归收集文本项（带路径与字号）
fn collect_texts(elements: &[Element], prefix: &str, out: &mut Vec<serde_json::Value>) {
    for el in elements {
        let t = path::type_name(el);
        if let Element::Group { children, .. } = el {
            let child_prefix = format!("{prefix}/{t}{}", type_index(elements, el));
            collect_texts(children, &child_prefix, out);
            continue;
        }
        if let Some(text) = path::text_of(el) {
            if !text.trim().is_empty() {
                out.push(json!({
                    "path": format!("{prefix}/{t}{}", type_index(elements, el)),
                    "text": text,
                }));
            }
        }
    }
}

/// 元素在其同级同类型序列中的 1-based 序号
fn type_index(elements: &[Element], target: &Element) -> String {
    let t = path::type_name(target);
    let mut n = 0;
    for el in elements {
        if path::type_name(el) == t {
            n += 1;
        }
        if std::ptr::eq(el, target) {
            return format!("[{n}]");
        }
    }
    "[1]".to_string()
}

/// 递归收集媒体项（图片）
fn collect_media(
    elements: &[Element],
    prefix: &str,
    product_root: &Path,
    out: &mut Vec<serde_json::Value>,
) {
    for el in elements {
        let t = path::type_name(el);
        let seg = format!("{prefix}/{t}{}", type_index(elements, el));
        match el {
            Element::Image { src, .. } => {
                out.push(json!({
                    "path": seg,
                    "src": product::media_abs_path(product_root, src),
                }));
            }
            Element::Group { children, .. } => collect_media(children, &seg, product_root, out),
            _ => {}
        }
    }
}

/// view text：文本 + 媒体（JSON）
pub fn view_text(
    doc: &json2pdf::model::DocumentModel,
    page: &PageModel,
    page_no: usize,
    product_root: &Path,
) -> serde_json::Value {
    let mut texts = Vec::new();
    collect_texts(&page.elements, &format!("/page[{page_no}]"), &mut texts);
    let mut media = Vec::new();
    collect_media(
        &page.elements,
        &format!("/page[{page_no}]"),
        product_root,
        &mut media,
    );
    let (w, h) = product::page_size(doc, page);
    json!({
        "page": page_no,
        "size": { "width": w, "height": h },
        "texts": texts,
        "media": media,
    })
}

/// 递归生成布局节点（类型/几何/文字摘要/路径，group 含 children）
fn layout_node(elements: &[Element], el: &Element, prefix: &str) -> serde_json::Value {
    let t = path::type_name(el);
    let p = format!("{prefix}/{t}{}", type_index(elements, el));
    let (x, y, w, h) = path::geometry(el);
    let mut node = json!({
        "path": p,
        "type": t,
        "position": { "x": x, "y": y, "w": w, "h": h },
    });
    if let Some(text) = path::text_of(el) {
        node["text"] = json!(text);
    }
    match el {
        Element::Text { size, color, .. } => {
            node["size"] = json!(size);
            node["color"] = json!(color);
        }
        Element::Rect { fill, stroke, .. } => {
            node["fill"] = json!(fill);
            node["stroke"] = json!(stroke);
        }
        Element::Polyline { points, .. } => {
            node["points_count"] = json!(points.len());
        }
        Element::Image { src, .. } => {
            node["src"] = json!(src);
        }
        Element::Shading(d) => {
            node["kind"] = json!(d.kind);
        }
        Element::Group { children, .. } => {
            node["children"] = json!(children
                .iter()
                .map(|c| layout_node(children, c, &p))
                .collect::<Vec<_>>());
        }
        _ => {}
    }
    node
}

/// view layout：元素布局树（JSON）
pub fn view_layout(
    doc: &json2pdf::model::DocumentModel,
    page: &PageModel,
    page_no: usize,
) -> serde_json::Value {
    let (w, h) = product::page_size(doc, page);
    json!({
        "page": page_no,
        "size": { "width": w, "height": h },
        "rotation": page.rotation,
        "layout": page
            .elements
            .iter()
            .map(|el| layout_node(&page.elements, el, &format!("/page[{page_no}]")))
            .collect::<Vec<_>>(),
    })
}
