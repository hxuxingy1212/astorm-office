//! batch：回放 dump 风格的编辑指令（默认遇错即停，--force 跳过继续）
//!
//! 输出报告（--json 时 stdout）：
//! ```json
//! { "ok": true, "applied": 3, "failed": 0,
//!   "steps": [ { "index": 1, "op": "set", "path": "/slide[1]", "ok": true, "result": {...} } ] }
//! ```

use json2pptx::model::Slide;
use office_core::batch::{props_as_kv, Report, Step};
use std::path::Path;

use crate::{edit, path as ppath, view};

/// 顺序执行指令；`stop_on_error` = true 时遇错即停（默认），false 对应 --force
pub fn run_steps(slides: &mut [Slide], steps: &[Step], stop_on_error: bool) -> serde_json::Value {
    let mut report = Report::default();
    for step in steps {
        match apply(slides, step) {
            Ok(result) => report.push_ok(step, result),
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

fn apply(slides: &mut [Slide], step: &Step) -> Result<serde_json::Value, office_core::CliError> {
    let op = step.op.as_str();
    let path = step
        .path
        .as_deref()
        .ok_or_else(|| office_core::CliError::new(format!("第 {} 条指令缺少 path", step.index)))?;
    let kv = props_as_kv(&step.props);
    let props = edit::parse_props(&kv)?;
    let ep = ppath::parse(path)?;
    let resolved = ppath::resolve(slides, &ep)?;
    match op {
        "set" => edit::set(slides, &resolved, &props),
        "add" => {
            let etype = step
                .etype
                .as_deref()
                .or_else(|| step.props.get("type").and_then(|t| t.as_str()))
                .unwrap_or("text");
            // dump 回放优先：props 即序列化后的元素（含 type/id/position），整体反序列化保真
            if let Ok(el) = serde_json::from_value::<json2pptx::model::elements::Element>(
                serde_json::Value::Object(step.props.clone()),
            ) {
                return edit::add_built(slides, &resolved, el, etype);
            }
            edit::add(slides, &resolved, etype, &props)
        }
        "remove" => edit::remove(slides, &resolved),
        "get" => edit::get(slides, &resolved),
        "view" => {
            let mode = step
                .props
                .get("mode")
                .and_then(|m| m.as_str())
                .unwrap_or("text");
            Ok(match mode {
                "text" => view::view_text(
                    &slides[resolved.slide_idx],
                    resolved.slide_idx + 1,
                    Path::new(""),
                ),
                _ => view::view_layout(&slides[resolved.slide_idx], resolved.slide_idx + 1),
            })
        }
        other => Err(office_core::CliError::new(format!("未知 op: {other}"))
            .suggest("可用: set / add / remove / get / view")),
    }
}
