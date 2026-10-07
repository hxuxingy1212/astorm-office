//! 极简 MCP（Model Context Protocol）服务：stdio + JSON-RPC 2.0
//! 暴露工具：unpack / repack / view / validate / dump / edit / batch / qa / extract_text / schema / help

use office_core::batch::parse_steps;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use crate::batch as batch_mod;
use crate::{edit, path as ppath, product, validate, view};

pub fn run() -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");
        if id.is_null() {
            continue;
        }
        let resp = match method {
            "initialize" => ok(
                id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "json2pdf", "version": env!("CARGO_PKG_VERSION") }
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
                        json!({ "content": [ { "type": "text", "text": text } ], "isError": false }),
                    ),
                    Err(e) => ok(
                        id,
                        json!({ "content": [ { "type": "text", "text": e } ], "isError": true }),
                    ),
                }
            }
            _ => err(id, -32601, &format!("未知方法: {method}")),
        };
        writeln!(out, "{}", serde_json::to_string(&resp).unwrap()).map_err(|e| e.to_string())?;
        out.flush().map_err(|e| e.to_string())?;
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
            "name": name, "description": desc,
            "inputSchema": { "type": "object", "properties": props, "required": required }
        })
    };
    let s = || json!({ "type": "string" });
    json!([
        tool("unpack", "PDF → 产物目录", json!({"input": s(), "output": s()}), json!(["input","output"])),
        tool("repack", "产物目录 → PDF", json!({"input": s(), "output": s()}), json!(["input","output"])),
        tool("view", "只读视图 text/layout", json!({"input": s(), "path": s(), "mode": {"type":"string","enum":["text","layout"]}}), json!(["input","path"])),
        tool("validate", "结构校验（颜色/引用/越界/旋转）", json!({"input": s()}), json!(["input"])),
        tool("dump", "导出可回放指令（页面级，id 幂等）", json!({"input": s()}), json!(["input"])),
        tool("edit", "按路径 get/set/add/remove（/page[N]/text[K] 树寻址）", json!({"input": s(), "output": s(), "path": s(), "action": {"type":"string","enum":["get","set","add","remove"]}, "prop": s(), "type": s()}), json!(["input","path","action"])),
        tool("batch", "回放指令（commands 数组）", json!({"input": s(), "commands": {"type":"array","items":{"type":"object"}}}), json!(["input","commands"])),
        tool("extract_text", "按页提取文本", json!({"input": s(), "pages": s()}), json!(["input"])),
        tool("qa", "成品体检（空白页/越界/边距/CJK 禁则）", json!({"input": s(), "skip_cover": {"type":"boolean"}}), json!(["input"])),
        tool("schema", "产物目录 JSON Schema 指引", json!({}), json!([])),
        tool("help", "元素能力速查（topic: page/text/rect/image/polyline/path/shading/pattern_rect/group，缺省输出概览）", json!({"topic": s()}), json!([])),
    ])
}

fn g<'a>(args: &'a Value, k: &str) -> Result<&'a str, String> {
    args.get(k)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("缺少参数: {k}"))
}

fn load(input: &str) -> Result<(PathBuf, Vec<json2pdf::model::PageModel>), String> {
    let p = PathBuf::from(input);
    if product::is_product_dir(&p) {
        let pages = product::load_pages(&p).map_err(|e| e.message)?;
        Ok((p, pages))
    } else {
        let tmp = std::env::temp_dir().join(format!(
            "json2pdf_mcp_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        json2pdf::unpack::unpack(Path::new(input), &tmp).map_err(|e| e.to_string())?;
        let pages = product::load_pages(&tmp).map_err(|e| e.message)?;
        Ok((tmp, pages))
    }
}

fn call(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "unpack" => {
            let r = json2pdf::unpack::unpack(Path::new(g(args, "input")?), Path::new(g(args, "output")?))
                .map_err(|e| e.to_string())?;
            Ok(format!("解包完成: {}（{} 页）", g(args, "output")?, r.pages))
        }
        "repack" => {
            let out = g(args, "output")?;
            let r = json2pdf::repack::repack(Path::new(g(args, "input")?), Path::new(out))
                .map_err(|e| e.to_string())?;
            Ok(format!("已生成: {out}（{} 页，{} 个警告）", r.pages, r.warnings.len()))
        }
        "view" => {
            let (root, pages) = load(g(args, "input")?)?;
            let doc = product::load_document(&root).map_err(|e| e.message)?;
            let ep = ppath::parse(g(args, "path")?).map_err(|e| e.message)?;
            let resolved = ppath::resolve(&pages, &ep).map_err(|e| e.message)?;
            let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("text");
            let v = match mode {
                "layout" => view::view_layout(&doc, &pages[resolved.page_idx], resolved.page_idx + 1),
                _ => view::view_text(&doc, &pages[resolved.page_idx], resolved.page_idx + 1, &root),
            };
            Ok(serde_json::to_string_pretty(&v).unwrap())
        }
        "validate" => {
            let (root, pages) = load(g(args, "input")?)?;
            let doc = product::load_document(&root).map_err(|e| e.message)?;
            let opts: Vec<Option<json2pdf::model::PageModel>> = pages.iter().map(|p| Some(p.clone())).collect();
            let issues = validate::run(&doc, &opts, &root);
            Ok(
                serde_json::to_string_pretty(&json!({"issues": issues, "count": issues.len()}))
                    .unwrap(),
            )
        }
        "dump" => {
            let (root, pages) = load(g(args, "input")?)?;
            let doc = product::load_document(&root).map_err(|e| e.message)?;
            Ok(serde_json::to_string_pretty(&crate::dump::run(&doc, &pages)).unwrap())
        }
        "edit" => {
            let (root, mut pages) = load(g(args, "input")?)?;
            let mut doc = product::load_document(&root).map_err(|e| e.message)?;
            let ep = ppath::parse(g(args, "path")?).map_err(|e| e.message)?;
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
            let props = edit::parse_props(&prop_list).map_err(|e| e.message)?;
            let r = (|| {
                if action == "add" && args.get("type").and_then(|v| v.as_str()) == Some("page") {
                    // add page 走文档容器（path 无需解析到既有页）
                    let resolved = ppath::Resolved { page_idx: 0, chain: vec![] };
                    return edit::add(
                        &mut doc,
                        &root,
                        &mut pages,
                        &resolved,
                        &ep,
                        "page",
                        &props,
                    );
                }
                let resolved = ppath::resolve(&pages, &ep).map_err(|e| e.message)?;
                match action {
                    "get" => edit::get(&doc, &pages, &resolved),
                    "set" => edit::set(&mut pages, &resolved, &props),
                    "add" => edit::add(&mut doc, &root, &mut pages, &resolved, &ep, args.get("type").and_then(|v| v.as_str()).unwrap_or("text"), &props),
                    "remove" => edit::remove(&mut doc, &root, &mut pages, &resolved),
                    other => Err(office_core::CliError::new(format!("未知 action: {other}（可用: get/set/add/remove）"))),
                }
            })()
            .map_err(|e| match &e.suggestion {
                Some(s) => format!("{}；{s}", e.message),
                None => e.message.clone(),
            })?;
            if action != "get" {
                product::save_all(&root, &doc, &pages).map_err(|e| e.message)?;
            }
            Ok(serde_json::to_string_pretty(&r).unwrap())
        }
        "batch" => {
            let (root, mut pages) = load(g(args, "input")?)?;
            let mut doc = product::load_document(&root).map_err(|e| e.message)?;
            let commands = args.get("commands").cloned().unwrap_or(json!([]));
            let steps = parse_steps(&commands)?;
            let report = batch_mod::run_steps(&mut doc, &root, &mut pages, &steps, true);
            if report["applied"].as_u64().unwrap_or(0) > 0 {
                product::save_all(&root, &doc, &pages).map_err(|e| e.message)?;
            }
            Ok(serde_json::to_string_pretty(&report).unwrap())
        }
        "extract_text" => {
            let doc_l = lopdf::Document::load(g(args, "input")?).map_err(|e| e.to_string())?;
            let target: Vec<u32> = match args.get("pages").and_then(|v| v.as_str()) {
                Some(spec) => json2pdf::util::parse_page_spec(spec, doc_l.get_pages().len() as u32)
                    .map_err(|e| e.to_string())?,
                None => (1..=doc_l.get_pages().len() as u32).collect(),
            };
            let pts = json2pdf::text::extract_pages(&doc_l, &target).map_err(|e| e.to_string())?;
            let page_reports: Vec<Value> = pts
                .iter()
                .map(|p| json!({"page": p.page, "chars": p.char_count, "text": p.text()}))
                .collect();
            Ok(serde_json::to_string_pretty(
                &json!({"pages": page_reports}),
            )
            .unwrap())
        }
        "qa" => {
            let files = vec![PathBuf::from(g(args, "input")?)];
            let (v, code) = json2pdf::qa::qa(&files, false).map_err(|e| e.to_string())?;
            let mut out = serde_json::to_string_pretty(&v).unwrap();
            if code != 0 {
                out.push_str("\n（发现问题，CLI 退出码为 3）");
            }
            Ok(out)
        }
        "schema" => Ok(
            "完整 JSON Schema 见 ai-pdf/skill/references/schema.json（由 `json2pdf schema --json` 生成）。"
                .to_string(),
        ),
        "help" => {
            let v = match args.get("topic").and_then(|t| t.as_str()) {
                Some(t) => {
                    let el = crate::help::resolve(t).map_err(|e| match &e.suggestion {
                        Some(s) => format!("{}；{s}", e.message),
                        None => e.message.clone(),
                    })?;
                    crate::help::element_value(el)
                }
                None => crate::help::overview_value(),
            };
            Ok(serde_json::to_string_pretty(&v).unwrap())
        }
        _ => Err(format!("未知工具: {name}")),
    }
}
