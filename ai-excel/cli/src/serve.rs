//! 常驻编辑服务：加载一次工作簿，stdin 逐行 JSON 操作，内存保持。
//!
//! 每行：`{"op":"edit|view|query|merge|issues|save|batch|quit", ...}`
//! 回复（每行）：`{"ok":true,"result":...}` 或 `{"ok":false,"error":"..."}`

use json2xlsx::Workbook;
use office_core::batch::parse_steps;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use crate::batch as batch_mod;
use crate::{ops, path as expath, query, validate};

pub fn run(input: &Path) -> Result<(), office_core::CliError> {
    let is_dir = json2xlsx::is_product_dir(input);
    let mut wb = if is_dir {
        json2xlsx::load_product(input)?
    } else {
        json2xlsx::parse(input)?
    };
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
        let quit = v.get("op").and_then(|o| o.as_str()) == Some("quit");
        let resp = handle(&mut wb, input, is_dir, &v);
        writeln!(out, "{}", serde_json::to_string(&resp).unwrap())?;
        out.flush()?;
        if quit {
            break;
        }
    }
    Ok(())
}

fn handle(wb: &mut Workbook, input: &Path, is_dir: bool, v: &Value) -> Value {
    let op = v.get("op").and_then(|o| o.as_str()).unwrap_or("edit");
    let r: Result<Value, office_core::CliError> = (|| match op {
        "edit" => {
            let p = v.get("path").and_then(|x| x.as_str()).ok_or("缺少 path")?;
            let ep = expath::parse(p)?;
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
            let props = ops::parse_props(&prop_list);
            match action {
                "get" => Ok(ops::get(wb, &ep)?),
                "set" => Ok(ops::set(wb, &ep, &props)?),
                "add" => {
                    let etype = v.get("type").and_then(|x| x.as_str()).unwrap_or("cell");
                    Ok(ops::add(wb, &ep, etype, &props)?)
                }
                "remove" => Ok(ops::remove(wb, &ep)?),
                other => Err(office_core::CliError::new(format!("未知 action: {other}"))
                    .suggest("可用: get / set / add / remove")),
            }
        }
        "view" => {
            let p = v
                .get("path")
                .and_then(|x| x.as_str())
                .unwrap_or("/sheet[1]");
            let ep = expath::parse(p)?;
            let mode = v.get("mode").and_then(|x| x.as_str()).unwrap_or("text");
            Ok(match mode {
                "text" | "values" => ops::view_values(wb, &ep)?,
                "layout" | "structure" => ops::view_structure(wb, &ep)?,
                other => {
                    return Err(
                        office_core::CliError::new(format!("未知 view 模式: {other}"))
                            .suggest("可用: text / layout"),
                    )
                }
            })
        }
        "query" => {
            let selector = v
                .get("selector")
                .and_then(|x| x.as_str())
                .ok_or("缺少 selector")?;
            Ok(query::run(wb, selector)?)
        }
        "batch" => {
            let steps = parse_steps(v.get("commands").unwrap_or(v)).map_err(|e| {
                office_core::CliError::new(e)
                    .suggest("格式: {\"commands\":[{\"op\":...,\"path\":...}]}")
            })?;
            let stop = !v.get("force").and_then(|f| f.as_bool()).unwrap_or(false);
            Ok(batch_mod::run_steps(wb, &steps, stop))
        }
        "merge" => {
            let data = v.get("data").cloned().unwrap_or(json!({}));
            let n = json2xlsx::merge_workbook(wb, &data);
            Ok(json!({ "replacements": n }))
        }
        "issues" => Ok(json!({ "issues": validate::run(wb) })),
        "save" => {
            let output = v.get("output").and_then(|x| x.as_str()).map(PathBuf::from);
            if is_dir {
                json2xlsx::save_product(wb, input)?;
                if let Some(o) = output {
                    json2xlsx::generate(wb, &o)?;
                    return Ok(json!({ "output": o.display().to_string() }));
                }
                Ok(json!({ "saved": input.display().to_string() }))
            } else {
                let o = output.ok_or_else(|| {
                    office_core::CliError::new("输入为 .xlsx 时 save 必须提供 output")
                })?;
                json2xlsx::generate(wb, &o)?;
                Ok(json!({ "output": o.display().to_string() }))
            }
        }
        "quit" => Ok(json!({ "bye": true })),
        other => Err(office_core::CliError::new(format!("未知 op: {other}"))
            .suggest("可用: edit / view / query / batch / merge / issues / save / quit")),
    })();
    match r {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(e) => json!({ "ok": false, "error": e.message, "suggestion": e.suggestion }),
    }
}
