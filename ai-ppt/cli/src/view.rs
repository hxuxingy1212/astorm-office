//! 视图层：text / layout
//!
//! - text：结构感知的 Markdown 纯文本（表格/图表数据/组件数据/分组嵌套/备注），媒体为绝对路径
//! - layout：组件布局（类型/名称/文字/位置），树形结构（机器可读，便于定位后 edit）

use json2pptx::model::elements::Element;
use json2pptx::model::Slide;
use json2pptx::PayloadSinks;
use serde_json::json;
use std::path::Path;

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

/// 收集页内媒体（图片元素 + 背景图，src 解析为绝对路径）
fn slide_media(slide: &Slide, product_root: &Path) -> Vec<serde_json::Value> {
    let mut media = Vec::new();
    collect_media(&slide.elements, product_root, &mut media);
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
    media
}

/// view text：结构感知 Markdown（保留表格/图表/组件/分组层次）+ 媒体绝对路径
///
/// `sink` 非空时把内嵌附件（embeddings 部件）导出到指定目录，
/// Markdown 里的引用指向导出后的路径。
pub fn view_text(
    slide: &Slide,
    slide_no: usize,
    product_root: &Path,
    sinks: PayloadSinks<'_>,
) -> serde_json::Value {
    let resolve = |src: &str| crate::product::media_abs_path(product_root, src);
    json!({
        "slide": slide_no,
        "markdown": json2pptx::markdown::slide_markdown_with(slide, &resolve, sinks),
        "media": slide_media(slide, product_root),
    })
}

/// view text（整本）：所有页 → 一段 Markdown（每页一个 `## 幻灯片 N` 小节）
pub fn view_text_all(
    slides: &[Slide],
    product_root: &Path,
    sinks: PayloadSinks<'_>,
) -> serde_json::Value {
    let resolve = |src: &str| crate::product::media_abs_path(product_root, src);
    let media: Vec<serde_json::Value> = slides
        .iter()
        .flat_map(|s| slide_media(s, product_root))
        .collect();
    json!({
        "slides": slides.len(),
        "markdown": json2pptx::markdown::deck_markdown_with(slides, &resolve, sinks),
        "media": media,
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
