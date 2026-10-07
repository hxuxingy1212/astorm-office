//! 模板数据填充：把 `{{key}}` 占位符替换为 JSON 数据，生成新文档

use crate::model::blocks::Block;
use crate::model::{Document, Part, Section};

/// 按 JSON 数据替换文档内 `{{key}}` 占位符，返回替换次数
pub fn merge_document(doc: &mut Document, data: &serde_json::Value) -> usize {
    let mut n = 0;
    for part in &mut doc.parts {
        merge_part(part, data, &mut n);
    }
    n
}

fn merge_part(part: &mut Part, data: &serde_json::Value, n: &mut usize) {
    merge_blocks(&mut part.blocks, data, n);
    if let Some(s) = &mut part.section {
        merge_section(s, data, n);
    }
}

fn merge_section(s: &mut Section, data: &serde_json::Value, n: &mut usize) {
    let mut rep = |t: &mut Option<String>| {
        if let Some(v) = t {
            *v = fill(v, data, n);
        }
    };
    rep(&mut s.header);
    rep(&mut s.first_header);
    rep(&mut s.even_header);
    for b in [
        &mut s.header_blocks,
        &mut s.footer_blocks,
        &mut s.first_header_blocks,
        &mut s.even_header_blocks,
    ]
    .into_iter()
    .flatten()
    {
        merge_blocks(b, data, n);
    }
}

fn merge_blocks(blocks: &mut [Block], data: &serde_json::Value, n: &mut usize) {
    for b in blocks {
        match b {
            Block::Heading { text, runs, .. } => {
                *text = fill(text, data, n);
                if let Some(rs) = runs {
                    for r in rs {
                        r.text = fill(&r.text, data, n);
                    }
                }
            }
            Block::Paragraph(p) => {
                if let Some(t) = &mut p.text {
                    *t = fill(t, data, n);
                }
                if let Some(rs) = &mut p.runs {
                    for r in rs {
                        r.text = fill(&r.text, data, n);
                    }
                }
            }
            Block::Caption(c) => c.text = fill(&c.text, data, n),
            Block::Quote(q) => q.text = fill(&q.text, data, n),
            Block::Code(c) => c.text = fill(&c.text, data, n),
            Block::TextBox(t) => t.text = fill(&t.text, data, n),
            Block::Shape(sh) => {
                if let Some(t) = &mut sh.text {
                    *t = fill(t, data, n);
                }
            }
            Block::Bibliography(bib) => {
                if let Some(t) = &mut bib.title {
                    *t = fill(t, data, n);
                }
                for e in &mut bib.entries {
                    for a in &mut e.authors {
                        *a = fill(a, data, n);
                    }
                    e.title = fill(&e.title, data, n);
                    if let Some(t) = &mut e.container {
                        *t = fill(t, data, n);
                    }
                }
            }
            Block::List(l) => {
                for it in &mut l.items {
                    merge_blocks(&mut it.blocks, data, n);
                }
            }
            Block::Table(t) => {
                for row in &mut t.rows {
                    for c in &mut row.cells {
                        merge_blocks(&mut c.blocks, data, n);
                    }
                }
            }
            Block::Sdt(c) => merge_blocks(&mut c.blocks, data, n),
            _ => {}
        }
    }
}

/// 替换单个字符串中的 `{{key}}`
fn fill(s: &str, data: &serde_json::Value, n: &mut usize) -> String {
    if !s.contains("{{") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = s[i + 2..].find("}}") {
                let key = s[i + 2..i + 2 + end].trim();
                if let Some(v) = lookup(data, key) {
                    out.push_str(&value_to_string(v));
                    *n += 1;
                    i = i + 2 + end + 2;
                    continue;
                }
            }
        }
        // 处理多字节字符
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn lookup<'a>(data: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    let mut cur = data;
    for seg in key.split('.') {
        cur = cur.get(seg)?;
    }
    Some(cur)
}

fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}
