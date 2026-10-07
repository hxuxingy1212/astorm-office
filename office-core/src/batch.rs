//! batch 指令模型：`dump` 输出的可回放格式。
//!
//! 指令文件（与三个 CLI 的 `dump` 输出同构）：
//! ```json
//! { "version": 1, "commands": [
//!   { "op": "add", "path": "/sheet", "type": "sheet", "props": { "name": "Data" } },
//!   { "op": "set", "path": "/sheet[1]", "props": { "freeze": "A2" } }
//! ] }
//! ```
//! 也接受裸数组形式；`op`/`command` 两种键名均可，`props` 亦可写作 `prop`。

use serde_json::{Map, Value};

/// 单条可回放指令
#[derive(Debug, Clone)]
pub struct Step {
    /// 1-based 指令序号（用于回显定位）
    pub index: usize,
    /// 操作类型（set/add/remove/get/view/...，由各 CLI 解释）
    pub op: String,
    pub path: Option<String>,
    pub etype: Option<String>,
    pub props: Map<String, Value>,
    /// 原始指令对象（回显用）
    pub raw: Value,
}

/// 解析指令文件为步骤列表
pub fn parse_steps(v: &Value) -> Result<Vec<Step>, String> {
    let arr = match v {
        Value::Object(o) => o
            .get("commands")
            .or_else(|| o.get("steps"))
            .cloned()
            .unwrap_or(Value::Null),
        Value::Array(a) => Value::Array(a.clone()),
        _ => Value::Null,
    };
    let Value::Array(items) = arr else {
        return Err("batch 指令应为数组，或 {\"commands\": [...]} 形式的对象".to_string());
    };

    let mut steps = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("第 {} 条指令不是对象", i + 1))?;
        let op = obj
            .get("op")
            .or_else(|| obj.get("command"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("第 {} 条指令缺少 op", i + 1))?
            .to_string();
        let path = obj
            .get("path")
            .or_else(|| obj.get("parent"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let etype = obj.get("type").and_then(|v| v.as_str()).map(String::from);
        let props = match obj.get("props").or_else(|| obj.get("prop")) {
            Some(Value::Object(m)) => m.clone(),
            Some(Value::Null) | None => Map::new(),
            Some(_) => return Err(format!("第 {} 条指令的 props 必须是对象（k: v）", i + 1)),
        };
        steps.push(Step {
            index: i + 1,
            op,
            path,
            etype,
            props,
            raw: item.clone(),
        });
    }
    Ok(steps)
}

/// 把 JSON props 对象转为 `--prop k=v` 字符串列表（值非字符串时用 JSON 文本）
pub fn props_as_kv(props: &Map<String, Value>) -> Vec<String> {
    props
        .iter()
        .map(|(k, v)| {
            let vs = match v {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            format!("{k}={vs}")
        })
        .collect()
}

/// 回放结果汇总
#[derive(Default)]
pub struct Report {
    pub steps: Vec<Value>,
    pub applied: usize,
    pub failed: usize,
}

impl Report {
    pub fn push_ok(&mut self, step: &Step, result: Value) {
        self.applied += 1;
        self.steps.push(serde_json::json!({
            "index": step.index, "op": step.op, "path": step.path,
            "ok": true, "result": result,
        }));
    }

    pub fn push_err(&mut self, step: &Step, error: &str, suggestion: Option<&str>) {
        self.failed += 1;
        self.steps.push(serde_json::json!({
            "index": step.index, "op": step.op, "path": step.path,
            "ok": false, "error": error, "suggestion": suggestion,
        }));
    }

    pub fn to_value(&self) -> Value {
        serde_json::json!({
            "ok": self.failed == 0,
            "applied": self.applied,
            "failed": self.failed,
            "steps": self.steps,
        })
    }
}
