//! 序列化为可回放的编辑指令（batch JSON，对齐 ai-ppt/cli 的 dump.rs）
//!
//! ```json
//! { "version": 1, "page_size": {"width":595.28,"height":841.89},
//!   "commands": [ {"op":"add","path":"/page","type":"page","props":{...整页含 elements...}} ] }
//! ```
//! 页面 props 带 `id`（1..N）：batch 对同一文档回放时按 id 原位替换，逐条回放语义不变。

use json2pdf::model::{DocumentModel, PageModel};
use serde_json::json;

/// 生成可回放指令
pub fn run(doc: &DocumentModel, pages: &[PageModel]) -> serde_json::Value {
    let mut commands = Vec::new();
    for (i, page) in pages.iter().enumerate() {
        let mut p = page.clone();
        if p.id.is_none() {
            p.id = Some(i as u64 + 1);
        }
        let props = serde_json::to_value(&p).unwrap_or(json!({}));
        commands.push(json!({
            "op": "add",
            "path": "/page",
            "type": "page",
            "props": props,
        }));
    }
    json!({
        "version": 1,
        "page_size": { "width": doc.page_size.width, "height": doc.page_size.height },
        "commands": commands,
    })
}
