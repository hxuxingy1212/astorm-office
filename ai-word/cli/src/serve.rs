//! 常驻编辑服务：加载一次文档，stdin 逐行 JSON 操作，内存保持
//! 每行：{"op":"edit|view|merge|issues|save|quit", ...}
//! 回复（每行）：{"ok":true,"result":...} 或 {"ok":false,"error":"..."}

use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use crate::{edit, path, product, view};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

pub fn run(input: &Path) -> Result<()> {
    let (root, _guard) = product::resolve_input(input)?;
    let mut doc = product::load_document(&root)?;
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                writeln!(
                    out,
                    "{}",
                    json!({"ok": false, "error": format!("JSON 解析失败: {e}")})
                )
                .ok();
                out.flush().ok();
                continue;
            }
        };
        let resp = handle(&mut doc, &root, &v);
        writeln!(out, "{}", serde_json::to_string(&resp).unwrap())?;
        out.flush()?;
        if v.get("op").and_then(|o| o.as_str()) == Some("quit") {
            break;
        }
    }
    Ok(())
}

fn handle(doc: &mut json2docx::Document, root: &Path, v: &Value) -> Value {
    let op = v.get("op").and_then(|o| o.as_str()).unwrap_or("edit");
    let r: Result<Value> = (|| match op {
        "edit" => {
            let p = v.get("path").and_then(|x| x.as_str()).ok_or("缺少 path")?;
            let segs = path::parse(p)?;
            let loc = path::resolve(doc, &segs)?;
            let action = v.get("action").and_then(|x| x.as_str()).unwrap_or("get");
            let prop_list: Vec<String> = match v.get("prop") {
                Some(Value::Array(a)) => a
                    .iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect(),
                Some(Value::String(s)) => s
                    .split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect(),
                _ => Vec::new(),
            };
            let props = edit::parse_props(&prop_list)?;
            let res = match action {
                "get" => edit::get(doc, &loc)?,
                "set" => edit::set(doc, &loc, &props)?,
                "add" => edit::add(
                    doc,
                    &loc,
                    v.get("type")
                        .and_then(|x| x.as_str())
                        .unwrap_or("paragraph"),
                    &props,
                )?,
                "remove" => edit::remove(doc, &loc)?,
                other => return Err(CliError::new(format!("未知 action: {other}"))),
            };
            Ok(json!(res))
        }
        "view" => {
            let p = v.get("path").and_then(|x| x.as_str()).unwrap_or("/part[1]");
            let segs = path::parse(p)?;
            let loc = path::resolve(doc, &segs)?;
            let mode = v.get("mode").and_then(|x| x.as_str()).unwrap_or("layout");
            Ok(if mode == "text" {
                view::view_text(doc, loc.part)
            } else {
                view::view_layout(doc, loc.part)
            })
        }
        "merge" => {
            let data = v.get("data").cloned().unwrap_or(json!({}));
            let n = json2docx::merge_document(doc, &data);
            Ok(json!({ "replacements": n }))
        }
        "issues" => Ok(json!({ "issues": json2docx::validate_product(root) })),
        "warnings" => Ok(json!({ "warnings": json2docx::content_warnings(doc) })),
        "save" => {
            let out = v
                .get("output")
                .and_then(|x| x.as_str())
                .ok_or("缺少 output")?;
            product::save_document(root, doc)?;
            json2docx::repack(&product::path_str(root), out)?;
            Ok(json!({ "output": out }))
        }
        "quit" => Ok(json!({ "bye": true })),
        other => Err(CliError::new(format!("未知 op: {other}"))),
    })();
    match r {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(e) => json!({ "ok": false, "error": e.message, "suggestion": e.suggestion }),
    }
}
