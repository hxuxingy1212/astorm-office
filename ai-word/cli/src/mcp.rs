//! 极简 MCP（Model Context Protocol）服务：stdio + JSON-RPC 2.0
//! 暴露工具：unpack / repack / view / validate / merge / edit / render / schema / templates

use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use crate::{edit, path, product, render, view};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

pub fn run() -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        // 通知（无 id）不回复
        if id.is_null() {
            continue;
        }
        let resp = match method {
            "initialize" => ok(
                id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "json2docx", "version": env!("CARGO_PKG_VERSION") }
                }),
            ),
            "ping" => ok(id, json!({})),
            "tools/list" => ok(id, json!({ "tools": tools() })),
            "tools/call" => {
                let name = msg
                    .pointer("/params/name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args = msg
                    .pointer("/params/arguments")
                    .cloned()
                    .unwrap_or(json!({}));
                match call(name, &args) {
                    Ok(text) => ok(
                        id,
                        json!({
                            "content": [ { "type": "text", "text": text } ],
                            "isError": false
                        }),
                    ),
                    Err(e) => ok(
                        id,
                        json!({
                            "content": [ { "type": "text", "text": match &e.suggestion {
                                Some(s) => format!("{}；{s}", e.message),
                                None => e.message.clone(),
                            } } ],
                            "isError": true
                        }),
                    ),
                }
            }
            _ => err(id, -32601, &format!("未知方法: {method}")),
        };
        writeln!(out, "{}", serde_json::to_string(&resp).unwrap())?;
        out.flush()?;
    }
    Ok(())
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}
fn err(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn tools() -> Value {
    let tool = |name: &str, desc: &str, props: Value, required: Value| {
        json!({
            "name": name,
            "description": desc,
            "inputSchema": { "type": "object", "properties": props, "required": required }
        })
    };
    let s = || json!({ "type": "string" });
    json!([
        tool(
            "unpack",
            "将 DOCX 解包为产物目录（document.json + word/parts + media）",
            json!({ "input": s(), "output": s() }),
            json!(["input", "output"])
        ),
        tool(
            "repack",
            "从产物目录重建 DOCX",
            json!({ "input": s(), "output": s() }),
            json!(["input", "output"])
        ),
        tool(
            "view",
            "查看文档结构：text/layout；输入 .docx 或产物目录",
            json!({ "input": s(), "path": s(), "mode": { "type": "string", "enum": ["text", "layout"] } }),
            json!(["input", "path"])
        ),
        tool(
            "validate",
            "结构校验与内容告警",
            json!({ "input": s() }),
            json!(["input"])
        ),
        tool(
            "merge",
            "用 JSON 数据替换 {{key}} 占位符",
            json!({ "input": s(), "output": s(), "data": {} }),
            json!(["input", "output", "data"])
        ),
        tool(
            "edit",
            "按路径 get/set/add/remove",
            json!({ "input": s(), "output": s(), "path": s(), "action": { "type": "string", "enum": ["get","set","add","remove"] }, "prop": s(), "type": s() }),
            json!(["input", "path", "action"])
        ),
        tool(
            "render",
            "渲染为 HTML（整篇）",
            json!({ "input": s(), "output": s() }),
            json!(["input", "output"])
        ),
        tool("schema", "打印 JSON Schema 文档指引", json!({}), json!([])),
    ])
}

fn g<'a>(args: &'a Value, k: &str) -> Result<&'a str> {
    args.get(k)
        .and_then(|v| v.as_str())
        .ok_or_else(|| CliError::new(format!("缺少参数: {k}")))
}

fn call(name: &str, args: &Value) -> Result<String> {
    match name {
        "unpack" => {
            let r = json2docx::unpack(g(args, "input")?, g(args, "output")?)?;
            Ok(format!("解包完成: {}（{} 个分片）", r.path, r.parts))
        }
        "repack" => {
            let r = json2docx::repack(g(args, "input")?, g(args, "output")?)?;
            let issues = json2docx::validate_docx(std::path::Path::new(g(args, "output")?));
            Ok(format!(
                "已生成: {}（{} 个分片）· 校验 {} 个问题",
                r.path,
                r.parts,
                issues.len()
            ))
        }
        "view" => {
            let input = PathBuf::from(g(args, "input")?);
            let (root, _t) = product::resolve_input(&input)?;
            let doc = product::load_document(&root)?;
            let segs = path::parse(g(args, "path")?)?;
            let loc = path::resolve(&doc, &segs)?;
            let mode = args
                .get("mode")
                .and_then(|v| v.as_str())
                .unwrap_or("layout");
            let mut v = if mode == "text" {
                view::view_text(&doc, loc.part)
            } else {
                view::view_layout(&doc, loc.part)
            };
            if let Value::Object(m) = &mut v {
                m.insert("issues".into(), json!(json2docx::validate_product(&root)));
                m.insert("warnings".into(), json!(json2docx::content_warnings(&doc)));
            }
            Ok(serde_json::to_string_pretty(&v).unwrap())
        }
        "validate" => {
            let p = g(args, "input")?;
            let issues = if p.ends_with(".docx") {
                json2docx::validate_docx(std::path::Path::new(p))
            } else {
                json2docx::validate_product(std::path::Path::new(p))
            };
            Ok(serde_json::to_string_pretty(&json!({ "issues": issues })).unwrap())
        }
        "merge" => {
            let input = PathBuf::from(g(args, "input")?);
            let output = g(args, "output")?;
            let (root, _t) = product::resolve_input(&input)?;
            let mut doc = product::load_document(&root)?;
            let data = args.get("data").cloned().unwrap_or(json!({}));
            let n = json2docx::merge_document(&mut doc, &data);
            product::save_document(&root, &doc)?;
            json2docx::repack(&product::path_str(&root), output)?;
            Ok(format!("已生成: {output}（替换 {n} 处）"))
        }
        "edit" => {
            let input = PathBuf::from(g(args, "input")?);
            let (root, _t) = product::resolve_input(&input)?;
            let mut doc = product::load_document(&root)?;
            let segs = path::parse(g(args, "path")?)?;
            let loc = path::resolve(&doc, &segs)?;
            let action = g(args, "action")?;
            let prop_list: Vec<String> = match args.get("prop") {
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
            let r = match action {
                "get" => edit::get(&doc, &loc)?,
                "set" => edit::set(&mut doc, &loc, &props)?,
                "add" => edit::add(
                    &mut doc,
                    &loc,
                    args.get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("paragraph"),
                    &props,
                )?,
                "remove" => edit::remove(&mut doc, &loc)?,
                other => return Err(CliError::new(format!("未知 action: {other}"))),
            };
            if action != "get" {
                product::save_document(&root, &doc)?;
            }
            if let Some(out) = args.get("output").and_then(|v| v.as_str()) {
                json2docx::repack(&product::path_str(&root), out)?;
            }
            Ok(serde_json::to_string_pretty(&r).unwrap())
        }
        "render" => {
            let input = PathBuf::from(g(args, "input")?);
            let output = PathBuf::from(g(args, "output")?);
            let (root, _t) = product::resolve_input(&input)?;
            let doc = product::load_document(&root)?;
            render::render_all(&doc, Some(&root), &output)?;
            Ok(format!("已渲染: {}", output.display()))
        }
        "schema" => Ok(crate::schema_guide().to_string()),
        _ => Err(CliError::new(format!("未知工具: {name}"))),
    }
}
