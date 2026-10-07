//! 序列化为可回放的编辑指令（batch JSON），与 json2pptx / json2xlsx 的 `dump` 对齐。
//!
//! 输出约定：
//! ```json
//! { "version": 1, "commands": [
//!   { "op": "set", "path": "/part[1]", "props": { "section": { ... } } },
//!   { "op": "add", "path": "/part[1]", "type": "heading", "props": { "level": 1, "text": "标题" } }
//! ] }
//! ```
//! 注意：指令假定文档已存在对应数量的分片（`add` 不支持创建 part）；
//! 可先 `unpack` 一份骨架产物目录再回放。

use json2docx::model::Document;
use serde_json::{json, Map, Value};

/// 生成可回放指令
pub fn run(doc: &Document) -> Value {
    let mut commands = Vec::new();

    // 文档级属性（可回放到新文档）
    let mut doc_props = Map::new();
    if !doc.styles.is_empty() {
        if let Ok(v) = serde_json::to_value(&doc.styles) {
            doc_props.insert("styles".into(), v);
        }
    }
    if let Some(t) = &doc.template {
        doc_props.insert("template".into(), json!(t));
    }
    if let Ok(v) = serde_json::to_value(&doc.page) {
        doc_props.insert("page".into(), v);
    }
    if let Ok(v) = serde_json::to_value(&doc.theme) {
        doc_props.insert("theme".into(), v);
    }
    if !doc_props.is_empty() {
        commands.push(json!({ "op": "set", "path": "/", "props": doc_props }));
    }

    for (i, part) in doc.parts.iter().enumerate() {
        let pp = format!("/part[{}]", i + 1);
        if let Some(section) = &part.section {
            commands.push(json!({
                "op": "set",
                "path": pp,
                "props": { "section": serde_json::to_value(section).unwrap_or(Value::Null) },
            }));
        }
        for block in &part.blocks {
            let mut props = match serde_json::to_value(block) {
                Ok(Value::Object(m)) => m,
                _ => continue,
            };
            let btype = props
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("paragraph")
                .to_string();
            props.remove("type");
            commands.push(json!({
                "op": "add",
                "path": pp,
                "type": btype,
                "props": props,
            }));
        }
    }

    json!({ "version": 1, "commands": commands })
}
