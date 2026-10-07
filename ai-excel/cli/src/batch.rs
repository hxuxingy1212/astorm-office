//! batch 回放：把 `dump` 输出的指令（或同构手写指令）重放到工作簿。
//!
//! 语义：默认遇到第一个错误即停止（保证原子性直觉）；`--force` 跳过错误继续。
//! 只要有任何一条失败，整体退出码为 1；结果 JSON 含每条指令的执行详情。

use json2xlsx::Workbook;
use office_core::batch::{props_as_kv, Report, Step};
use office_core::CliError;
use serde_json::Value;

use crate::ops;
use crate::path as expath;

type Result<T> = std::result::Result<T, CliError>;

/// 依次执行指令，返回结果 JSON（`{"ok",..,"applied","failed","steps"}`）
pub fn run_steps(wb: &mut Workbook, steps: &[Step], stop_on_error: bool) -> Value {
    let mut report = Report::default();
    for step in steps {
        match apply(wb, step) {
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

fn apply(wb: &mut Workbook, step: &Step) -> Result<Value> {
    let path_str = step
        .path
        .clone()
        .ok_or_else(|| CliError::new(format!("第 {} 条指令缺少 path", step.index)))?;
    let p = expath::parse(&path_str)?;
    let kv = props_as_kv(&step.props);
    match step.op.as_str() {
        "get" => ops::get(wb, &p),
        "set" => {
            let props = ops::parse_props(&kv);
            ops::set(wb, &p, &props)
        }
        "add" => {
            let etype = step.etype.clone().unwrap_or_else(|| "cell".to_string());
            let props = ops::parse_props(&kv);
            ops::add(wb, &p, &etype, &props)
        }
        "remove" => ops::remove(wb, &p),
        "view" => {
            let mode = step
                .raw
                .get("mode")
                .and_then(|m| m.as_str())
                .unwrap_or("text");
            match mode {
                "text" | "values" => ops::view_values(wb, &p),
                "layout" | "structure" => ops::view_structure(wb, &p),
                other => Err(CliError::new(format!(
                    "第 {} 条指令 view 模式未知: {other}",
                    step.index
                ))
                .suggest("可用: text / layout")),
            }
        }
        other => Err(CliError::new(format!("不支持的操作: {other}"))
            .suggest("可用: set / add / remove / get / view")),
    }
}
