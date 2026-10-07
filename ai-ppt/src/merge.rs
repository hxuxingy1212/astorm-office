//! 模板数据填充：把幻灯片内的 `{{key}}` 占位符替换为 JSON 数据
//!
//! 与 json2docx 的 merge 同构：只替换文本语义字段（元素文字、备注、表格单元格），
//! 不触碰样式与布局。

use crate::model::elements::{Element, TextContent};
use crate::model::Slide;
use office_core::merge::fill;

/// 按数据替换全部幻灯片中的 `{{key}}`，返回替换次数
pub fn merge_slides(slides: &mut [Slide], data: &serde_json::Value) -> usize {
    let mut n = 0;
    for slide in slides.iter_mut() {
        if let Some(notes) = &mut slide.notes {
            *notes = fill(notes, data, &mut n);
        }
        for el in slide.elements.iter_mut() {
            merge_element(el, data, &mut n);
        }
    }
    n
}

fn merge_element(el: &mut Element, data: &serde_json::Value, n: &mut usize) {
    match el {
        Element::Text(t) => t.text = fill_text_content(t.text.clone(), data, n),
        Element::Shape(s) => {
            if let Some(t) = &s.text {
                s.text = Some(fill_text_content(t.clone(), data, n));
            }
        }
        Element::Table(t) => {
            for row in t.rows.iter_mut() {
                for cell in row.iter_mut() {
                    cell.text = fill(&cell.text, data, n);
                }
            }
        }
        Element::Group(g) => {
            for child in g.children.iter_mut() {
                merge_element(child, data, n);
            }
        }
        Element::Equation(e) => {
            if let Some(t) = &mut e.text {
                *t = fill(t, data, n);
            }
        }
        Element::Opaque(o) => {
            if let Some(t) = &mut o.text {
                *t = fill(t, data, n);
            }
        }
        _ => {}
    }
}

fn fill_text_content(tc: TextContent, data: &serde_json::Value, n: &mut usize) -> TextContent {
    match tc {
        TextContent::Simple(s) => TextContent::Simple(fill(&s, data, n)),
        TextContent::Paragraphs(mut ps) => {
            for p in ps.iter_mut() {
                if let Some(t) = &mut p.text {
                    *t = fill(t, data, n);
                }
                if let Some(runs) = &mut p.runs {
                    for r in runs.iter_mut() {
                        r.text = fill(&r.text, data, n);
                    }
                }
            }
            TextContent::Paragraphs(ps)
        }
    }
}
