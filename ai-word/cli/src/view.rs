//! 第一层视图：text / layout（对应 ai-ppt 的 view.rs）

use json2docx::model::blocks::*;
use json2docx::model::Document;
use serde_json::{json, Value};

/// view text：文本 + 媒体（去样式/图形）
pub fn view_text(doc: &Document, part_idx: usize) -> Value {
    let part = &doc.parts[part_idx];
    let mut blocks = Vec::new();
    let mut media = Vec::new();
    for b in &part.blocks {
        blocks.push(json!({
            "type": b.type_name(),
            "text": b.plain_text(),
        }));
        if let Block::Image(img) = b {
            media.push(json!({ "type": "image", "src": img.src }));
        }
    }
    json!({
        "part": part_idx + 1,
        "blocks": blocks,
        "media": media,
    })
}

/// view layout：块结构树（类型/文字/位置无，文档为线性）
pub fn view_layout(doc: &Document, part_idx: usize) -> Value {
    let part = &doc.parts[part_idx];
    let layout: Vec<Value> = part.blocks.iter().map(block_node).collect();
    json!({
        "part": part_idx + 1,
        "layout": layout,
    })
}

fn block_node(b: &Block) -> Value {
    match b {
        Block::Heading { level, text, .. } => {
            json!({ "type": "heading", "level": level, "text": text })
        }
        Block::Paragraph(p) => json!({ "type": "paragraph", "text": paragraph_text(p) }),
        Block::List(l) => {
            let items: Vec<Value> = l
                .items
                .iter()
                .map(|it| {
                    json!({
                        "level": it.level,
                        "children": it.blocks.iter().map(block_node).collect::<Vec<_>>(),
                    })
                })
                .collect();
            json!({ "type": "list", "ordered": l.ordered, "items": items })
        }
        Block::Table(t) => {
            let rows: Vec<Value> = t
                .rows
                .iter()
                .map(|r| {
                    let cells: Vec<Value> = r
                        .cells
                        .iter()
                        .map(|c| {
                            json!({
                                "children": c.blocks.iter().map(block_node).collect::<Vec<_>>()
                            })
                        })
                        .collect();
                    json!({ "cells": cells })
                })
                .collect();
            json!({
                "type": "table",
                "header_row": t.header_row,
                "caption": t.caption,
                "rows": rows,
            })
        }
        Block::Image(i) => json!({
            "type": "image",
            "src": i.src,
            "width": i.width,
            "height": i.height,
            "alt": i.alt,
            "caption": i.caption,
        }),
        Block::Formula(f) => json!({
            "type": "formula",
            "latex": f.latex,
            "display": f.display,
            "number": f.number,
        }),
        Block::Code(c) => json!({ "type": "code", "lang": c.lang, "text": c.text }),
        Block::Quote(q) => json!({ "type": "quote", "text": q.text }),
        Block::Toc(t) => json!({ "type": "toc", "title": t.title, "levels": t.levels }),
        Block::Bibliography(b) => json!({
            "type": "bibliography",
            "style": b.style,
            "entries": b.entries.len(),
        }),
        Block::Caption(c) => json!({ "type": "caption", "text": c.text, "of": c.of }),
        Block::Chart(ch) => json!({
            "type": "chart",
            "chart_type": ch.chart_type,
            "title": ch.title,
            "categories": ch.categories,
            "series": ch.series.iter().map(|s| json!({"name": s.name, "values": s.values})).collect::<Vec<_>>(),
        }),
        Block::PageBreak => json!({ "type": "page_break" }),
        Block::Raw(r) => json!({
            "type": "raw",
            "rels": r.rels.len(),
        }),
        Block::Sdt(c) => json!({
            "type": "sdt",
            "sdt_type": c.props.sdt_type,
            "alias": c.props.alias,
            "blocks": c.blocks.iter().map(block_node).collect::<Vec<_>>(),
        }),
        Block::Shape(sh) => json!({
            "type": "shape",
            "shape_type": sh.shape_type,
            "width": sh.width,
            "height": sh.height,
            "text": sh.text,
        }),
        Block::Attachment(a) => json!({
            "type": "attachment",
            "src": a.src,
            "name": a.name,
            "prog_id": a.prog_id,
            "label": a.label,
        }),
        Block::TextBox(t) => json!({
            "type": "textbox",
            "text": t.text,
            "width": t.width,
            "height": t.height,
            "fill": t.fill,
            "line": t.line,
            "wrap": t.wrap,
        }),
    }
}
