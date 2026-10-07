//! 极简 MCP（Model Context Protocol）服务：stdio + JSON-RPC 2.0
//!
//! 暴露工具：unpack / repack / view / validate / dump / query / edit / merge /
//! render / extract / batch / schema / help / templates

use office_core::batch::parse_steps;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

use crate::batch as batch_mod;
use crate::{edit, path as ppath, product, query, validate, view};

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
                    "serverInfo": { "name": "json2pptx", "version": env!("CARGO_PKG_VERSION") }
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
        tool("unpack", "pptx → 产物目录", json!({"input": s(), "output": s()}), json!(["input","output"])),
        tool("repack", "产物目录 → pptx", json!({"input": s(), "output": s()}), json!(["input","output"])),
        tool("view", "只读视图 text/layout", json!({"input": s(), "path": s(), "mode": {"type":"string","enum":["text","layout"]}}), json!(["input","path"])),
        tool("validate", "结构校验（越界/空文本/缺失图片）", json!({"input": s()}), json!(["input"])),
        tool("dump", "导出可回放指令", json!({"input": s()}), json!(["input"])),
        tool("query", "选择器查询 slide[1] shape[name=Title] / text:contains(...)", json!({"input": s(), "selector": s()}), json!(["input","selector"])),
        tool("edit", "按路径 get/set/add/remove（@id/@name 稳定寻址）", json!({"input": s(), "output": s(), "path": s(), "action": {"type":"string","enum":["get","set","add","remove"]}, "prop": s(), "type": s()}), json!(["input","path","action"])),
        tool("merge", "用 JSON 数据替换 {{key}} 占位符", json!({"input": s(), "data": {}}), json!(["input","data"])),
        tool("render", "渲染幻灯片 PNG（engine: browser|native）", json!({"input": s(), "path": s(), "engine": {"type":"string","enum":["browser","native"]}, "output": s()}), json!(["input","path"])),
        tool("extract", "提取主题风格模板", json!({"input": s(), "output": s(), "name": s()}), json!(["input","output"])),
        tool("batch", "回放指令（commands 数组）", json!({"input": s(), "output": s(), "commands": {"type":"array","items":{"type":"object"}}}), json!(["input","commands"])),
        tool("schema", "JSON Schema 文档指引", json!({}), json!([])),
        tool("help", "元素能力速查（topic: slide/text/shape/line/image/table/chart/group/line_chart/pie_chart/ring_chart/progress_bar，缺省输出概览）", json!({"topic": s()}), json!([])),
        tool("templates", "内置设计风格列表", json!({}), json!([])),
    ])
}

fn g<'a>(args: &'a Value, k: &str) -> Result<&'a str, String> {
    args.get(k)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("缺少参数: {k}"))
}

fn load(input: &str) -> Result<(PathBuf, Vec<json2pptx::model::Slide>), String> {
    let p = PathBuf::from(input);
    if product::is_product_dir(&p) {
        let slides = product::load_slides(&p).map_err(|e| e.message)?;
        Ok((p, slides))
    } else {
        // .pptx：解包到一次性临时目录
        let tmp = std::env::temp_dir().join(format!(
            "json2pptx_mcp_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        json2pptx::unpack(input, &tmp.to_string_lossy()).map_err(|e| e.to_string())?;
        let slides = product::load_slides(&tmp).map_err(|e| e.message)?;
        Ok((tmp, slides))
    }
}

fn call(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "unpack" => {
            let r = json2pptx::unpack(g(args, "input")?, g(args, "output")?)
                .map_err(|e| e.to_string())?;
            Ok(format!("解包完成: {}（{} 张幻灯片）", r.path, r.slides))
        }
        "repack" => {
            let r = json2pptx::repack(g(args, "input")?, g(args, "output")?)
                .map_err(|e| e.to_string())?;
            Ok(format!("已生成: {}（{} 张幻灯片）", r.path, r.slides))
        }
        "view" => {
            let (root, slides) = load(g(args, "input")?)?;
            let ep = ppath::parse(g(args, "path")?).map_err(|e| e.message)?;
            let resolved = ppath::resolve(&slides, &ep).map_err(|e| e.message)?;
            let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("text");
            let slide = &slides[resolved.slide_idx];
            let v = match mode {
                "layout" => view::view_layout(slide, resolved.slide_idx + 1),
                _ => view::view_text(slide, resolved.slide_idx + 1, &root),
            };
            Ok(serde_json::to_string_pretty(&v).unwrap())
        }
        "validate" => {
            let (_root, slides) = load(g(args, "input")?)?;
            let (w, h) = (13.33, 7.5);
            let issues = validate::run(&slides, w, h);
            Ok(
                serde_json::to_string_pretty(&json!({"issues": issues, "count": issues.len()}))
                    .unwrap(),
            )
        }
        "dump" => {
            let (_root, slides) = load(g(args, "input")?)?;
            let (w, h) = (13.33, 7.5);
            Ok(serde_json::to_string_pretty(&crate::dump::run(&slides, w, h)).unwrap())
        }
        "query" => {
            let (_root, slides) = load(g(args, "input")?)?;
            let sel = query::parse_selector(g(args, "selector")?).map_err(|e| e.message)?;
            let v = query::run(&slides, &sel);
            Ok(serde_json::to_string_pretty(&json!({"matches": v.len(), "items": v})).unwrap())
        }
        "edit" => {
            let (root, mut slides) = load(g(args, "input")?)?;
            let ep = ppath::parse(g(args, "path")?).map_err(|e| e.message)?;
            let resolved = ppath::resolve(&slides, &ep).map_err(|e| e.message)?;
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
            let r = match action {
                "get" => edit::get(&slides, &resolved),
                "set" => edit::set(&mut slides, &resolved, &props),
                "add" => edit::add(
                    &mut slides,
                    &resolved,
                    args.get("type").and_then(|v| v.as_str()).unwrap_or("text"),
                    &props,
                ),
                "remove" => edit::remove(&mut slides, &resolved),
                other => return Err(format!("未知 action: {other}（可用: get/set/add/remove）")),
            }
            .map_err(|e| match &e.suggestion {
                Some(s) => format!("{}；{s}", e.message),
                None => e.message.clone(),
            })?;
            if action != "get" {
                product::save_slides(&root, &slides).map_err(|e| e.message)?;
            }
            Ok(serde_json::to_string_pretty(&r).unwrap())
        }
        "merge" => {
            let (root, mut slides) = load(g(args, "input")?)?;
            let data = args.get("data").cloned().unwrap_or(json!({}));
            let n = json2pptx::merge_slides(&mut slides, &data);
            product::save_slides(&root, &slides).map_err(|e| e.message)?;
            Ok(format!("替换 {n} 处（产物目录: {}）", root.display()))
        }
        "render" => {
            let (root, _slides) = load(g(args, "input")?)?;
            let (w, h) = product::presentation_size(&root).map_err(|e| e.message)?;
            let out = args
                .get("output")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("slide.png"));
            let engine = args
                .get("engine")
                .and_then(|v| v.as_str())
                .unwrap_or("native");
            let ep = ppath::parse(g(args, "path")?).map_err(|e| e.message)?;
            let slide_no = ep.slide;
            if engine == "browser" {
                crate::browser::render_slide_browser(&root, slide_no, w, h, 2.0, &out)?;
            } else {
                let slides = product::load_slides(&root).map_err(|e| e.message)?;
                let slide = &slides[slide_no - 1];
                let root2 = root.clone();
                let resolver: crate::render::MediaResolver =
                    std::sync::Arc::new(move |src: &str| {
                        crate::product::media_abs_path(&root2, src)
                    });
                crate::render::render_slide_png(slide, w, h, 2.0, &resolver, &out)?;
            }
            Ok(format!("已渲染: {}", out.display()))
        }
        "extract" => {
            let (root, _slides) = load(g(args, "input")?)?;
            let dir = PathBuf::from(g(args, "output")?);
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("template");
            let pres = json2pptx::parse(&root.to_string_lossy()).map_err(|e| e.to_string())?;
            let (path, n) =
                crate::extract::write_template_dir(&pres, &dir, name, g(args, "input")?)
                    .map_err(|e| e.to_string())?;
            Ok(format!(
                "已提取模板: {}（配色/字体 {} 项）",
                path.display(),
                n
            ))
        }
        "batch" => {
            let (root, mut slides) = load(g(args, "input")?)?;
            let commands = args.get("commands").cloned().unwrap_or(json!([]));
            let steps = parse_steps(&commands)?;
            let report = batch_mod::run_steps(&mut slides, &steps, true);
            if report["applied"].as_u64().unwrap_or(0) > 0 {
                product::save_slides(&root, &slides).map_err(|e| e.message)?;
            }
            Ok(serde_json::to_string_pretty(&report).unwrap())
        }
        "schema" => Ok(
            "完整 JSON Schema 见 skill/references/schema.json（由 `schema --json` 生成）。"
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
        "templates" => {
            let presets: Vec<_> = json2pptx::model::TemplatePreset::all()
                .iter()
                .map(|t| json!({ "name": t.name, "description": t.desc, "display": t.display }))
                .collect();
            Ok(serde_json::to_string_pretty(&json!({ "templates": presets })).unwrap())
        }
        _ => Err(format!("未知工具: {name}")),
    }
}
