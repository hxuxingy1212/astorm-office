//! 视图层：text / layout
//!
//! - text：文本 + 媒体信息（去样式/图形），树形保留层级，媒体为绝对路径
//! - layout：组件布局（类型/名称/文字/位置），树形结构

use json2pptx::model::elements::Element;
use json2pptx::model::Slide;
use serde_json::json;
use std::path::Path;

/// 递归提取文本节点（text 元素 / 形状内文字 / 表格单元格）
fn collect_texts(elements: &[Element]) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for el in elements {
        let name = el.name().unwrap_or_default();
        match el {
            Element::Group(g) => {
                let children = collect_texts(&g.children);
                if !children.is_empty() {
                    out.push(json!({ "name": name, "children": children }));
                }
            }
            Element::Table(t) => {
                let cells: Vec<String> = t
                    .rows
                    .iter()
                    .flatten()
                    .filter(|c| !c.text.is_empty())
                    .map(|c| c.text.clone())
                    .collect();
                if !cells.is_empty() {
                    out.push(json!({ "name": name, "text": cells.join(" | ") }));
                }
            }
            _ => {
                if let Some(text) = el.text_content() {
                    if !text.is_empty() {
                        out.push(json!({ "name": name, "text": text }));
                    }
                }
            }
        }
    }
    out
}

/// 递归提取媒体节点（图片元素 + 背景图）
fn collect_media(elements: &[Element], product_root: &Path, out: &mut Vec<serde_json::Value>) {
    for el in elements {
        match el {
            Element::Image(img) => {
                out.push(json!({
                    "name": img.name.as_deref().unwrap_or(""),
                    "src": crate::product::media_abs_path(product_root, &img.src),
                }));
            }
            Element::Group(g) => collect_media(&g.children, product_root, out),
            _ => {}
        }
    }
}

/// view text：文本 + 媒体（JSON）
pub fn view_text(slide: &Slide, slide_no: usize, product_root: &Path) -> serde_json::Value {
    let mut media = Vec::new();
    collect_media(&slide.elements, product_root, &mut media);

    // 背景图
    if let Some(serde_json::Value::Object(bg)) = &slide.background {
        if bg.get("type").and_then(|v| v.as_str()) == Some("image") {
            if let Some(src) = bg.get("src").and_then(|v| v.as_str()) {
                media.push(json!({
                    "name": "background",
                    "src": crate::product::media_abs_path(product_root, src),
                }));
            }
        }
    }

    json!({
        "slide": slide_no,
        "texts": collect_texts(&slide.elements),
        "media": media,
        "notes": slide.notes,
    })
}

/// 递归生成布局节点（类型/名称/文字/位置，group 含 children）
fn layout_node(el: &Element) -> serde_json::Value {
    let mut node = json!({
        "type": el.type_name(),
        "name": el.name().unwrap_or_default(),
    });
    if let Some(text) = el.text_content() {
        node["text"] = json!(text);
    }
    let pos = el.position();
    node["position"] = json!({
        "x": pos.x, "y": pos.y, "w": pos.w, "h": pos.h,
    });
    // 组件核心字段
    match el {
        Element::ProgressBar(c) => node["value"] = json!(c.value),
        Element::ProgressRing(c) => node["value"] = json!(c.value),
        Element::BarChart(c) => node["data"] = json!(c.data),
        Element::LineChart(c) => {
            node["data"] = json!(c.data);
            node["labels"] = json!(c.labels);
        }
        Element::PieChart(c) => {
            node["data"] = json!(c.data);
            node["labels"] = json!(c.labels);
        }
        Element::RingChart(c) => {
            node["data"] = json!(c.data);
            node["labels"] = json!(c.labels);
        }
        Element::KpiCard(c) => node["value"] = json!(c.value),
        Element::RatingStars(c) => node["rating"] = json!(c.rating),
        Element::ProcessFlow(c) => node["steps"] = json!(c.steps),
        Element::Timeline(c) => node["items"] = json!(c.items.len()),
        _ => {}
    }
    if let Element::Group(g) = el {
        node["children"] = json!(g.children.iter().map(layout_node).collect::<Vec<_>>());
    }
    node
}

/// view layout：组件布局（树形 JSON）
pub fn view_layout(slide: &Slide, slide_no: usize) -> serde_json::Value {
    json!({
        "slide": slide_no,
        "layout": slide.elements.iter().map(layout_node).collect::<Vec<_>>(),
    })
}
