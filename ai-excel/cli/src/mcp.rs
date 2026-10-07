//! 极简 MCP（Model Context Protocol）服务：stdio + JSON-RPC 2.0
//!
//! 暴露工具：unpack / repack / view / validate / dump / query / edit / merge /
//! render / extract / batch / schema / templates

use json2xlsx::Workbook;
use office_core::batch::parse_steps;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use crate::{batch as batch_mod, ops, path as expath, query, validate};

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
                    "serverInfo": { "name": "json2xlsx", "version": env!("CARGO_PKG_VERSION") }
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
        tool(
            "unpack",
            "xlsx → 产物目录",
            json!({"input": s(), "output": s()}),
            json!(["input", "output"])
        ),
        tool(
            "repack",
            "产物目录 → xlsx",
            json!({"input": s(), "output": s()}),
            json!(["input", "output"])
        ),
        tool(
            "view",
            "只读视图 text/layout",
            json!({"input": s(), "path": s(), "mode": {"type":"string","enum":["text","layout"]}}),
            json!(["input", "path"])
        ),
        tool(
            "validate",
            "结构校验（issues 列表）",
            json!({"input": s()}),
            json!(["input"])
        ),
        tool(
            "dump",
            "导出可回放指令",
            json!({"input": s()}),
            json!(["input"])
        ),
        tool(
            "query",
            "选择器查询 sheets/sheet[N]/cell:contains(...)",
            json!({"input": s(), "selector": s()}),
            json!(["input", "selector"])
        ),
        tool(
            "edit",
            "按路径 get/set/add/remove",
            json!({"input": s(), "output": s(), "path": s(), "action": {"type":"string","enum":["get","set","add","remove"]}, "prop": s(), "type": s()}),
            json!(["input", "path", "action"])
        ),
        tool(
            "merge",
            "用 JSON 数据替换 {{key}} 占位符",
            json!({"input": s(), "output": s(), "data": {}}),
            json!(["input", "output", "data"])
        ),
        tool(
            "render",
            "生成 HTML 预览",
            json!({"input": s(), "output": s()}),
            json!(["input", "output"])
        ),
        tool(
            "extract",
            "提取样式模板",
            json!({"input": s(), "output": s(), "name": s()}),
            json!(["input", "output"])
        ),
        tool(
            "batch",
            "回放指令（commands 数组）",
            json!({"input": s(), "output": s(), "commands": {"type":"array","items":{"type":"object"}}}),
            json!(["input", "commands"])
        ),
        tool("schema", "JSON Schema 文档指引", json!({}), json!([])),
        tool(
            "help",
            "元素能力速查（topic: sheet/cell/range/row/col，缺省输出概览）",
            json!({"topic": s()}),
            json!([])
        ),
        tool("templates", "内置风格模板库列表", json!({}), json!([])),
    ])
}

fn g<'a>(args: &'a Value, k: &str) -> Result<&'a str, String> {
    args.get(k)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("缺少参数: {k}"))
}

fn load(input: &str) -> Result<Workbook, String> {
    let p = PathBuf::from(input);
    if json2xlsx::is_product_dir(&p) {
        json2xlsx::load_product(&p).map_err(|e| e.to_string())
    } else {
        json2xlsx::parse(&p).map_err(|e| e.to_string())
    }
}

fn call(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "unpack" => {
            let r = json2xlsx::unpack(Path::new(g(args, "input")?), Path::new(g(args, "output")?))
                .map_err(|e| e.to_string())?;
            Ok(format!("解包完成: {}（{} 个工作表）", r.dir, r.sheets))
        }
        "repack" => {
            let r = json2xlsx::repack(Path::new(g(args, "input")?), Path::new(g(args, "output")?))
                .map_err(|e| e.to_string())?;
            Ok(format!("已生成: {}（{} 个工作表）", r.path, r.sheets))
        }
        "view" => {
            let wb = load(g(args, "input")?)?;
            let p = expath::parse(g(args, "path")?).map_err(|e| e.message)?;
            let mode = args.get("mode").and_then(|v| v.as_str()).unwrap_or("text");
            let v = match mode {
                "layout" | "structure" => ops::view_structure(&wb, &p),
                _ => ops::view_values(&wb, &p),
            }
            .map_err(|e| e.message)?;
            Ok(serde_json::to_string_pretty(&v).unwrap())
        }
        "validate" => {
            let wb = load(g(args, "input")?)?;
            let issues = validate::run(&wb);
            Ok(
                serde_json::to_string_pretty(&json!({"issues": issues, "count": issues.len()}))
                    .unwrap(),
            )
        }
        "dump" => {
            let wb = load(g(args, "input")?)?;
            Ok(serde_json::to_string_pretty(&crate::dump::run(&wb)).unwrap())
        }
        "query" => {
            let wb = load(g(args, "input")?)?;
            let v = query::run(&wb, g(args, "selector")?).map_err(|e| e.message)?;
            Ok(serde_json::to_string_pretty(&v).unwrap())
        }
        "edit" => {
            let input = PathBuf::from(g(args, "input")?);
            let mut wb = load(input.to_str().unwrap_or(""))?;
            let p = expath::parse(g(args, "path")?).map_err(|e| e.message)?;
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
            let props = ops::parse_props(&prop_list);
            let r = match action {
                "get" => ops::get(&wb, &p),
                "set" => ops::set(&mut wb, &p, &props),
                "add" => ops::add(
                    &mut wb,
                    &p,
                    args.get("type").and_then(|v| v.as_str()).unwrap_or("cell"),
                    &props,
                ),
                "remove" => ops::remove(&mut wb, &p),
                other => return Err(format!("未知 action: {other}（可用: get/set/add/remove）")),
            }
            .map_err(|e| e.message)?;
            if action != "get" {
                match args.get("output").and_then(|v| v.as_str()) {
                    Some(o) => {
                        json2xlsx::generate(&wb, Path::new(o)).map_err(|e| e.to_string())?;
                    }
                    None => {
                        if json2xlsx::is_product_dir(&input) {
                            json2xlsx::save_product(&wb, &input).map_err(|e| e.to_string())?;
                        } else {
                            return Err("对 .xlsx 的写操作必须提供 output".to_string());
                        }
                    }
                }
            }
            Ok(serde_json::to_string_pretty(&r).unwrap())
        }
        "merge" => {
            let input = PathBuf::from(g(args, "input")?);
            let mut wb = load(input.to_str().unwrap_or(""))?;
            let data = args.get("data").cloned().unwrap_or(json!({}));
            let n = json2xlsx::merge_workbook(&mut wb, &data);
            json2xlsx::generate(&wb, Path::new(g(args, "output")?)).map_err(|e| e.to_string())?;
            Ok(format!("已生成: {}（替换 {n} 处）", g(args, "output")?))
        }
        "render" => {
            let (root, _guard) =
                crate::resolve_render_root(Path::new(g(args, "input")?)).map_err(|e| e.message)?;
            let wb = json2xlsx::load_product(&root).map_err(|e| e.to_string())?;
            let (html, report) = crate::render::workbook_html(&wb, Some(&root));
            std::fs::write(g(args, "output")?, html).map_err(|e| e.to_string())?;
            let _ = report;
            Ok(format!("已生成预览: {}", g(args, "output")?))
        }
        "extract" => {
            let wb = load(g(args, "input")?)?;
            let dir = PathBuf::from(g(args, "output")?);
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("template");
            let t =
                json2xlsx::template::write_template_dir(&wb, &dir, name, Some(g(args, "input")?))
                    .map_err(|e| e.to_string())?;
            Ok(format!(
                "已提取模板: {}（配色 {} / 字体 {} / 数字格式 {}）",
                dir.display(),
                t.palette.len(),
                t.fonts.len(),
                t.number_formats.len()
            ))
        }
        "batch" => {
            let input = PathBuf::from(g(args, "input")?);
            let mut wb = load(input.to_str().unwrap_or(""))?;
            let commands = args.get("commands").cloned().unwrap_or(json!([]));
            let steps = parse_steps(&commands)?;
            let report = batch_mod::run_steps(&mut wb, &steps, true);
            if report["failed"].as_u64().unwrap_or(0) == 0 {
                if let Some(o) = args.get("output").and_then(|v| v.as_str()) {
                    json2xlsx::generate(&wb, Path::new(o)).map_err(|e| e.to_string())?;
                } else if json2xlsx::is_product_dir(&input) {
                    json2xlsx::save_product(&wb, &input).map_err(|e| e.to_string())?;
                }
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
                    let el = crate::help::resolve(t).map_err(|e| match e.suggestion {
                        Some(s) => format!("{}；{s}", e.message),
                        None => e.message,
                    })?;
                    crate::help::element_value(el)
                }
                None => crate::help::overview_value(),
            };
            Ok(serde_json::to_string_pretty(&v).unwrap())
        }
        "templates" => {
            let lib: Value =
                serde_json::from_str(include_str!("../../skill/templates/library.json"))
                    .unwrap_or(Value::Null);
            Ok(serde_json::to_string_pretty(&lib).unwrap())
        }
        _ => Err(format!("未知工具: {name}")),
    }
}
