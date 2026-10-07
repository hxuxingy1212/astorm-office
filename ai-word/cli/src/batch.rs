//! batch：回放 dump 风格的编辑指令（默认遇错即停，--force 跳过继续）
//!
//! 指令假定文档已存在对应分片（dump 说明见 dump.rs）；
//! 结果 JSON：`{"ok",..,"applied","failed","steps"}`。

use json2docx::model::Document;
use office_core::batch::{props_as_kv, Report, Step};
use office_core::CliError;

use crate::{edit, path as wpath, view};

/// 依次执行指令
pub fn run_steps(doc: &mut Document, steps: &[Step], stop_on_error: bool) -> serde_json::Value {
    let mut report = Report::default();
    for step in steps {
        match apply(doc, step) {
            Ok(v) => report.push_ok(step, v),
            Err(e) => {
                report.push_err(step, &e.message, e.suggestion.as_deref());
                if stop_on_error {
                    break;
                }
            }
        }
    }
    report.to_value()
}

fn apply(doc: &mut Document, step: &Step) -> Result<serde_json::Value, CliError> {
    let path = step
        .path
        .as_deref()
        .ok_or_else(|| CliError::new(format!("第 {} 条指令缺少 path", step.index)))?;
    let kv = props_as_kv(&step.props);
    let props = edit::parse_props(&kv)?;

    // 文档级（path = "/"）：styles/page/theme 等（dump 的 `set /` 指令）
    if path == "/" {
        return match step.op.as_str() {
            "set" => edit::apply_doc_props(doc, &props),
            other => Err(CliError::new(format!("文档级仅支持 set，实际: {other}"))),
        };
    }

    let segs = wpath::parse(path)?;
    let loc = wpath::resolve(doc, &segs)?;
    match step.op.as_str() {
        "get" => edit::get(doc, &loc),
        "set" => edit::set(doc, &loc, &props),
        "add" => {
            let btype = step
                .etype
                .as_deref()
                .or_else(|| step.props.get("type").and_then(|t| t.as_str()))
                .unwrap_or("paragraph");
            edit::add(doc, &loc, btype, &props)
        }
        "remove" => edit::remove(doc, &loc),
        "view" => {
            let mode = step
                .raw
                .get("mode")
                .and_then(|m| m.as_str())
                .unwrap_or("text");
            Ok(match mode {
                "text" => view::view_text(doc, loc.part),
                _ => view::view_layout(doc, loc.part),
            })
        }
        other => Err(CliError::new(format!("未知 op: {other}"))
            .suggest("可用: set / add / remove / get / view")),
    }
}
