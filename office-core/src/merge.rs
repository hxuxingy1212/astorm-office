//! `{{key}}` 占位符填充原语（三个格式共用）。
//!
//! 语义与 json2docx 的 `merge_document` 一致：`{{a.b}}` 支持点号嵌套取值，
//! 值非字符串时用 JSON 文本，未命中的占位符原样保留。

use serde_json::Value;

/// 替换字符串中的 `{{key}}`，命中数累加到 `n`
pub fn fill(s: &str, data: &Value, n: &mut usize) -> String {
    if !s.contains("{{") {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            if let Some(end) = s[i + 2..].find("}}") {
                let key = s[i + 2..i + 2 + end].trim();
                if let Some(v) = lookup(data, key) {
                    out.push_str(&value_to_string(v));
                    *n += 1;
                    i = i + 2 + end + 2;
                    continue;
                }
            }
        }
        // 处理多字节字符
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// `Option<String>` 版本的填充（None 保持不变）
pub fn fill_opt(t: &mut Option<String>, data: &Value, n: &mut usize) {
    if let Some(v) = t {
        *v = fill(v, data, n);
    }
}

/// 按 `a.b.c` 路径取值
pub fn lookup<'a>(data: &'a Value, key: &str) -> Option<&'a Value> {
    let mut cur = data;
    for seg in key.split('.') {
        cur = cur.get(seg)?;
    }
    Some(cur)
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}
