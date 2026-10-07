//! 结构校验
//!
//! 对解析出的演示文稿模型做静态检查：越界、空文本、缺失图片、
//! 表格列数不一致、名称重复等。输出问题列表（JSON）。

use json2pptx::model::elements::Element;
use json2pptx::model::Slide;
use serde_json::json;

/// 执行校验，返回问题列表
pub fn run(slides: &[Slide], width: f64, height: f64) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for (i, slide) in slides.iter().enumerate() {
        let slide_no = i + 1;
        check_elements(
            &slide.elements,
            &format!("/slide[{}]", slide_no),
            width,
            height,
            &mut out,
        );
    }
    out
}

fn check_elements(
    elements: &[Element],
    prefix: &str,
    width: f64,
    height: f64,
    out: &mut Vec<serde_json::Value>,
) {
    let mut name_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut type_counters: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();

    for el in elements {
        let c = type_counters.entry(el.type_name()).or_insert(0);
        *c += 1;
        let path = format!("{}/{}[{}]", prefix, el.type_name(), c);

        // 位置越界
        let p = el.position();
        let tol = 0.05;
        if !matches!(el, Element::Comment(_)) {
            if p.x < -tol || p.y < -tol || p.x + p.w > width + tol || p.y + p.h > height + tol {
                push(
                    out,
                    &path,
                    "warning",
                    format!(
                        "元素超出幻灯片边界（{:.2},{:.2} {:.2}x{:.2} > {:.2}x{:.2}）",
                        p.x, p.y, p.w, p.h, width, height
                    ),
                );
            }
            if p.w <= 0.0 || p.h <= 0.0 {
                push(out, &path, "warning", "元素宽或高为 0".to_string());
            }
        }

        // 名称重复
        if let Some(name) = el.name() {
            if !name.is_empty() {
                *name_counts.entry(name.to_string()).or_insert(0) += 1;
            }
        }

        // 类型相关
        let text = el.text_content();
        match el {
            Element::Text(_) if text.as_deref().unwrap_or("").trim().is_empty() => {
                push(out, &path, "info", "文本元素内容为空".to_string());
            }
            Element::Shape(s) if s.shape_type.is_empty() => {
                push(out, &path, "warning", "形状缺少 shape_type".to_string());
            }
            Element::Image(img) if img.src.trim().is_empty() => {
                push(out, &path, "error", "图片元素缺少 src".to_string());
            }
            Element::Table(t) => {
                let widths: Vec<usize> = t
                    .rows
                    .iter()
                    .map(|r| {
                        r.iter()
                            .map(|c| c.colspan.unwrap_or(1).max(1) as usize)
                            .sum()
                    })
                    .collect();
                if widths.len() > 1 && widths.iter().any(|w| *w != widths[0]) {
                    push(out, &path, "warning", "表格各行物理列数不一致".to_string());
                }
            }
            Element::Chart(c) if c.series.is_empty() => {
                push(out, &path, "warning", "图表没有数据系列".to_string());
            }
            Element::Group(g) => {
                check_elements(&g.children, &path, width, height, out);
            }
            _ => {}
        }
    }

    for (name, count) in name_counts {
        if count > 1 {
            push(
                out,
                prefix,
                "info",
                format!("名称 \"{name}\" 出现 {count} 次（选择窗格中可能冲突）"),
            );
        }
    }
}

fn push(out: &mut Vec<serde_json::Value>, path: &str, severity: &str, message: String) {
    out.push(json!({ "path": path, "severity": severity, "message": message }));
}
