//! 常驻编辑服务：加载一次产物目录，stdin 逐行 JSON 操作，内存保持。
//! 每行：`{"op":"edit|view|batch|issues|save|quit", ...}`
//! 回复（每行）：`{"ok":true,"result":...}` 或 `{"ok":false,"error":"...","suggestion":"..."}`

use json2pdf::model::{DocumentModel, PageModel};
use office_core::{batch::parse_steps, CliError};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use crate::batch as batch_mod;
use crate::{edit, path as ppath, product, validate, view};

pub struct Session {
    pub root: PathBuf,
    pub doc: DocumentModel,
    pub pages: Vec<PageModel>,
}

pub fn run(input: &Path) -> Result<(), CliError> {
    if !product::is_product_dir(input) {
        return Err(
            CliError::new("serve 只支持 unpack 产物目录（PDF 请先 unpack）")
                .suggest("先执行 json2pdf unpack <文件.pdf> -o <目录>"),
        );
    }
    let mut session = Session {
        root: input.to_path_buf(),
        doc: product::load_document(input)?,
        pages: product::load_pages(input)?,
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
                    json!({ "ok": false, "error": format!("JSON 解析失败: {e}") })
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
    let r: Result<Value, CliError> = (|| match op {
        "edit" => {
            let p = v.get("path").and_then(|x| x.as_str()).ok_or("缺少 path")?;
            let ep = ppath::parse(p)?;
            let resolved = ppath::resolve(&s.pages, &ep)?;
            let action = v.get("action").and_then(|x| x.as_str()).unwrap_or("get");
            let raw_props = parse_prop_strings(v.get("prop"));
            let props = edit::parse_props(&raw_props)?;
            let result = match action {
                "get" => edit::get(&s.doc, &s.pages, &resolved)?,
                "set" => edit::set(&mut s.pages, &resolved, &props)?,
                "add" => {
                    let etype = v.get("type").and_then(|x| x.as_str()).unwrap_or("text");
                    edit::add(
                        &mut s.doc,
                        &s.root,
                        &mut s.pages,
                        &resolved,
                        &ep,
                        etype,
                        &props,
                    )?
                }
                "remove" => edit::remove(&mut s.doc, &s.root, &mut s.pages, &resolved)?,
                other => {
                    return Err(CliError::new(format!("未知 action: {other}"))
                        .suggest("可用: get / set / add / remove"))
                }
            };
            if action != "get" {
                persist(s, resolved.page_idx)?;
            }
            Ok(result)
        }
        "view" => {
            let p = v.get("path").and_then(|x| x.as_str()).unwrap_or("/page[1]");
            let ep = ppath::parse(p)?;
            let resolved = ppath::resolve(&s.pages, &ep)?;
            let mode = v.get("mode").and_then(|x| x.as_str()).unwrap_or("text");
            match mode {
                "text" => Ok(view::view_text(
                    &s.doc,
                    &s.pages[resolved.page_idx],
                    resolved.page_idx + 1,
                    &s.root,
                )),
                "layout" => Ok(view::view_layout(
                    &s.doc,
                    &s.pages[resolved.page_idx],
                    resolved.page_idx + 1,
                )),
                other => Err(CliError::new(format!("未知 view 模式: {other}"))
                    .suggest("可用: text / layout")),
            }
        }
        "batch" => {
            let steps = parse_steps(v.get("commands").unwrap_or(v)).map_err(|e| {
                CliError::new(e).suggest(r#"格式: {"commands":[{"op":...,"path":...}]}"#)
            })?;
            let stop = !v.get("force").and_then(|f| f.as_bool()).unwrap_or(false);
            let report = batch_mod::run_steps(&mut s.doc, &s.root, &mut s.pages, &steps, stop);
            if report["applied"].as_u64().unwrap_or(0) > 0 {
                product::save_all(&s.root, &s.doc, &s.pages)?;
            }
            Ok(report)
        }
        "issues" => {
            let opts: Vec<Option<PageModel>> = s.pages.iter().map(|p| Some(p.clone())).collect();
            Ok(json!({ "issues": validate::run(&s.doc, &opts, &s.root) }))
        }
        "save" => {
            let output = v.get("output").and_then(|x| x.as_str()).map(PathBuf::from);
            product::save_all(&s.root, &s.doc, &s.pages)?;
            if let Some(o) = output {
                json2pdf::repack::repack(&s.root, &o).map_err(|e| CliError::new(e.to_string()))?;
                return Ok(json!({
                    "saved": s.root.display().to_string(),
                    "output": o.display().to_string(),
                }));
            }
            Ok(json!({ "saved": s.root.display().to_string() }))
        }
        "quit" => Ok(json!({ "bye": true })),
        other => Err(CliError::new(format!("未知 op: {other}"))
            .suggest("可用: edit / view / batch / issues / save / quit")),
    })();
    match r {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(e) => json!({ "ok": false, "error": e.message, "suggestion": e.suggestion }),
    }
}

fn persist(s: &Session, page_idx: usize) -> Result<(), CliError> {
    product::save_page(&s.root, page_idx, &s.pages[page_idx])?;
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
