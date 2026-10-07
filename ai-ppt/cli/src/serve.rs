//! 常驻编辑服务：加载一次产物目录，stdin 逐行 JSON 操作，内存保持。
//!
//! 每行：`{"op":"edit|view|batch|merge|issues|save|quit", ...}`
//! 回复（每行）：`{"ok":true,"result":...}` 或 `{"ok":false,"error":"...","suggestion":"..."}`

use json2pptx::model::Slide;
use office_core::{batch::parse_steps, path_str};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use crate::batch as batch_mod;
use crate::{edit, path as ppath, product, validate, view};

pub struct Session {
    pub root: PathBuf,
    pub slides: Vec<Slide>,
    /// 演示文稿尺寸（宽、高，英寸），校验用
    pub size: (f64, f64),
}

pub fn run(input: &Path) -> Result<(), office_core::CliError> {
    if !product::is_product_dir(input) {
        return Err(office_core::CliError::new(
            "serve 只支持 unpack 产物目录（.pptx 请先 unpack）",
        )
        .suggest("先执行 json2pptx unpack <文件.pptx> -o <目录>"));
    }
    let mut session = Session {
        root: input.to_path_buf(),
        slides: product::load_slides(input)?,
        size: product::presentation_size(input)?,
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
        let resp = handle(&mut session, &v);
        writeln!(out, "{}", serde_json::to_string(&resp).unwrap())?;
        out.flush()?;
        if quit {
            break;
        }
    }
    Ok(())
}

fn handle(s: &mut Session, v: &Value) -> Value {
    let op = v.get("op").and_then(|o| o.as_str()).unwrap_or("edit");
    let r: Result<Value, office_core::CliError> = (|| match op {
        "edit" => {
            let p = v.get("path").and_then(|x| x.as_str()).ok_or("缺少 path")?;
            let ep = ppath::parse(p)?;
            let resolved = ppath::resolve(&s.slides, &ep)?;
            let action = v.get("action").and_then(|x| x.as_str()).unwrap_or("get");
            let raw_props = parse_prop_strings(v.get("prop"));
            let props = edit::parse_props(&raw_props)?;
            let result = match action {
                "get" => edit::get(&s.slides, &resolved)?,
                "set" => edit::set(&mut s.slides, &resolved, &props)?,
                "add" => {
                    let etype = v.get("type").and_then(|x| x.as_str()).unwrap_or("text");
                    edit::add(&mut s.slides, &resolved, etype, &props)?
                }
                "remove" => edit::remove(&mut s.slides, &resolved)?,
                other => {
                    return Err(office_core::CliError::new(format!("未知 action: {other}"))
                        .suggest("可用: get / set / add / remove"))
                }
            };
            if action != "get" {
                persist(s, &resolved)?;
            }
            Ok(result)
        }
        "view" => {
            let p = v
                .get("path")
                .and_then(|x| x.as_str())
                .unwrap_or("/slide[1]");
            let ep = ppath::parse(p)?;
            let resolved = ppath::resolve(&s.slides, &ep)?;
            let mode = v.get("mode").and_then(|x| x.as_str()).unwrap_or("text");
            Ok(match mode {
                "text" => view::view_text(
                    &s.slides[resolved.slide_idx],
                    resolved.slide_idx + 1,
                    &s.root,
                ),
                "layout" => {
                    view::view_layout(&s.slides[resolved.slide_idx], resolved.slide_idx + 1)
                }
                other => {
                    return Err(
                        office_core::CliError::new(format!("未知 view 模式: {other}"))
                            .suggest("可用: text / layout"),
                    )
                }
            })
        }
        "batch" => {
            let steps = parse_steps(v.get("commands").unwrap_or(v)).map_err(|e| {
                office_core::CliError::new(e)
                    .suggest("格式: {\"commands\":[{\"op\":...,\"path\":...}]}")
            })?;
            let stop = !v.get("force").and_then(|f| f.as_bool()).unwrap_or(false);
            let report = batch_mod::run_steps(&mut s.slides, &steps, stop);
            // 任一步修改成功即整包写回（batch 语义：结果整体生效或整体保留旧值）
            if report["applied"].as_u64().unwrap_or(0) > 0 {
                product::save_slides(&s.root, &s.slides)?;
            }
            Ok(report)
        }
        "merge" => {
            let data = v.get("data").cloned().unwrap_or(json!({}));
            let n = json2pptx::merge_slides(&mut s.slides, &data);
            product::save_slides(&s.root, &s.slides)?;
            Ok(json!({ "replacements": n }))
        }
        "issues" => Ok(json!({ "issues": validate::run(&s.slides, s.size.0, s.size.1) })),
        "save" => {
            let output = v.get("output").and_then(|x| x.as_str()).map(PathBuf::from);
            product::save_slides(&s.root, &s.slides)?;
            if let Some(o) = output {
                json2pptx::repack(&path_str(&s.root), &path_str(&o))?;
                return Ok(
                    json!({ "saved": s.root.display().to_string(), "output": o.display().to_string() }),
                );
            }
            Ok(json!({ "saved": s.root.display().to_string() }))
        }
        "quit" => Ok(json!({ "bye": true })),
        other => Err(office_core::CliError::new(format!("未知 op: {other}"))
            .suggest("可用: edit / view / batch / merge / issues / save / quit")),
    })();
    match r {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(e) => json!({ "ok": false, "error": e.message, "suggestion": e.suggestion }),
    }
}

fn persist(s: &Session, resolved: &crate::path::Resolved) -> Result<(), office_core::CliError> {
    product::save_slide(&s.root, resolved.slide_idx, &s.slides[resolved.slide_idx])?;
    Ok(())
}

fn parse_prop_strings(v: Option<&Value>) -> Vec<String> {
    match v {
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
    }
}
