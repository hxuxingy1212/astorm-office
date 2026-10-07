//! 表单 — 复刻 pdf.py form.info / form.fill（pikepdf 路线）：
//! 遍历 AcroForm 字段树，读取字段类型/选项/状态；填写时写 /V 与 /AS 并置
//! NeedAppearances 让查看器重绘外观。

use anyhow::Result;
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

struct FieldInfo {
    id: ObjectId,
    /// 全限定名（父字段 /T 以 '.' 连接）
    fq_name: String,
    ft: String,
    value: Value,
    options: Vec<String>,
    states: Vec<String>,
    page: Option<u32>,
}

/// 页注释 → 字段根的对象号映射（widget 可能自带 /T 或挂 /Parent）。
fn field_page_map(doc: &Document) -> HashMap<ObjectId, u32> {
    let mut map = HashMap::new();
    for (pno, page_id) in doc.get_pages() {
        let Ok(page) = doc.get_dictionary(page_id) else {
            continue;
        };
        let Ok(annots) = page.get(b"Annots").and_then(Object::as_array) else {
            continue;
        };
        for annot in annots {
            let Ok(annot_ref) = annot.as_reference() else {
                continue;
            };
            let Ok(w) = doc.get_dictionary(annot_ref) else {
                continue;
            };
            // widget 自身是终端字段（合并式）或指向父字段
            if w.has(b"FT") {
                map.insert(annot_ref, pno);
            }
            if let Ok(parent) = w.get(b"Parent").and_then(Object::as_reference) {
                map.insert(parent, pno);
                // 再上一层（嵌套非终端字段）
                if let Ok(grand) = doc
                    .get_dictionary(parent)
                    .and_then(|d| d.get(b"Parent"))
                    .and_then(Object::as_reference)
                {
                    map.insert(grand, pno);
                }
            }
        }
    }
    map
}

fn field_name(dict: &Dictionary) -> String {
    dict.get(b"T")
        .and_then(Object::as_str)
        .ok()
        .map(|b| String::from_utf8_lossy(b).to_string())
        .or_else(|| {
            dict.get(b"T")
                .and_then(Object::as_string)
                .ok()
                .map(|s| s.to_string())
        })
        .unwrap_or_default()
}

fn field_type(dict: &Dictionary) -> String {
    match dict.get(b"FT").and_then(Object::as_name_str).unwrap_or("") {
        "Tx" => "text".into(),
        "Btn" => {
            // flag bit 16 (32768) = radio, bit 17 = pushbutton
            let flags = dict.get(b"Ff").and_then(Object::as_i64).unwrap_or(0);
            if flags & 0x8000 != 0 {
                "radio".into()
            } else if flags & 0x10000 != 0 {
                "pushbutton".into()
            } else {
                "checkbox".into()
            }
        }
        "Ch" => {
            let flags = dict.get(b"Ff").and_then(Object::as_i64).unwrap_or(0);
            if flags & 0x20000 != 0 {
                "listbox".into()
            } else {
                "dropdown".into()
            }
        }
        "Sig" => "signature".into(),
        _ => "unknown".into(),
    }
}

fn field_value(dict: &Dictionary) -> Value {
    match dict.get(b"V") {
        Ok(Object::String(bytes, _)) => crate::meta::json_str(bytes),
        Ok(Object::Name(n)) => Value::String(String::from_utf8_lossy(n).to_string()),
        _ => Value::Null,
    }
}

fn field_options(doc: &Document, dict: &Dictionary) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(arr) = dict.get(b"Opt").and_then(Object::as_array) {
        for el in arr {
            match el {
                Object::String(bytes, _) => out.push(crate::meta::str_lossy(bytes)),
                Object::Array(pair) => {
                    // [exportValue, label]
                    if let Some(Object::String(bytes, _)) = pair.last() {
                        out.push(crate::meta::str_lossy(bytes));
                    }
                }
                other => out.push(format!("{other:?}")),
            }
        }
    }
    let _ = doc;
    out
}

fn field_states(doc: &Document, dict: &Dictionary) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(ap) = dict.get_deref(b"AP", doc).and_then(Object::as_dict) {
        if let Ok(n) = ap.get_deref(b"N", doc).and_then(Object::as_dict) {
            for (k, _) in n.iter() {
                out.push(String::from_utf8_lossy(k).to_string());
            }
        }
    }
    out
}

fn walk_fields(
    doc: &Document,
    parent_prefix: &str,
    fields: &[Object],
    page_map: &HashMap<ObjectId, u32>,
    out: &mut Vec<FieldInfo>,
) {
    for f in fields {
        let Ok(id) = f.as_reference() else { continue };
        let Ok(dict) = doc.get_dictionary(id) else {
            continue;
        };
        let partial = field_name(dict);
        let fq = if parent_prefix.is_empty() {
            partial.clone()
        } else {
            format!("{parent_prefix}.{partial}")
        };
        if dict.has(b"FT") {
            // 终端字段
            out.push(FieldInfo {
                id,
                fq_name: fq.clone(),
                ft: field_type(dict),
                value: field_value(dict),
                options: field_options(doc, dict),
                states: field_states(doc, dict),
                page: page_map.get(&id).copied(),
            });
        }
        if let Ok(kids) = dict.get(b"Kids").and_then(Object::as_array) {
            walk_fields(doc, &fq, kids, page_map, out);
        }
    }
}

pub fn info(file: &Path) -> Result<Value> {
    let doc = Document::load(file)?;
    let catalog = doc.catalog()?;
    let empty = vec![];
    let field_arr = catalog
        .get(b"AcroForm")
        .and_then(Object::as_dict)
        .and_then(|a| a.get(b"Fields"))
        .and_then(Object::as_array)
        .unwrap_or(&empty);
    let page_map = field_page_map(&doc);
    let mut fields = Vec::new();
    walk_fields(&doc, "", field_arr, &page_map, &mut fields);

    let by_type: serde_json::Map<String, Value> = {
        let mut m: HashMap<String, u64> = HashMap::new();
        for f in &fields {
            *m.entry(f.ft.clone()).or_default() += 1;
        }
        m.into_iter().map(|(k, v)| (k, json!(v))).collect()
    };

    let fields_json: Vec<Value> = fields
        .iter()
        .map(|f| {
            json!({
                "name": f.fq_name,
                "type": f.ft,
                "value": if f.value.is_null() { Value::Null } else { f.value.clone() },
                "options": if f.options.is_empty() { Value::Null } else { json!(f.options) },
                "states": if f.states.is_empty() { Value::Null } else { json!(f.states) },
                "page": f.page,
            })
        })
        .collect();

    Ok(json!({
        "file": file.display().to_string(),
        "total_fields": fields.len(),
        "by_type": by_type,
        "fields": fields_json,
    }))
}

pub fn fill(file: &Path, out: &Path, data: &serde_json::Map<String, Value>) -> Result<Value> {
    let mut doc = Document::load(file)?;
    let catalog_dict = doc.catalog()?;
    let empty = vec![];
    let field_arr = catalog_dict
        .get(b"AcroForm")
        .and_then(Object::as_dict)
        .and_then(|a| a.get(b"Fields"))
        .and_then(Object::as_array)
        .cloned()
        .unwrap_or(empty);
    let page_map = field_page_map(&doc);
    let mut fields = Vec::new();
    walk_fields(&doc, "", &field_arr, &page_map, &mut fields);

    let mut filled = Vec::new();
    let mut missing = Vec::new();
    for (name, value) in data {
        let Some(field) = fields.iter().find(|f| &f.fq_name == name) else {
            missing.push(name.clone());
            continue;
        };
        let dict = doc.get_object_mut(field.id).and_then(Object::as_dict_mut)?;
        let value_str = match value {
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            other => other.to_string().trim_matches('"').to_string(),
        };
        match field.ft.as_str() {
            "checkbox" => {
                // 复选框：/V 用导出名；/AS 取与 /V 一致的外观状态（无则取非 Off 态）
                let state = if !value_str.is_empty()
                    && value_str != "false"
                    && field.states.iter().any(|s| s == &value_str)
                {
                    value_str.clone()
                } else if value_str.is_empty() || value_str == "false" {
                    "Off".to_string()
                } else {
                    field
                        .states
                        .iter()
                        .find(|s| s.as_str() != "Off")
                        .cloned()
                        .unwrap_or_else(|| "Yes".into())
                };
                dict.set("V", Object::Name(state.clone().into_bytes()));
                dict.set("AS", Object::Name(state.clone().into_bytes()));
                filled.push(json!({"name": name, "type": "checkbox", "state": state}));
            }
            "radio" => {
                let state = value_str.clone();
                if !field.states.iter().any(|s| s == &state)
                    && !field.options.iter().any(|o| o == &state)
                {
                    anyhow::bail!(
                        "radio field '{name}': '{state}' is not a valid state (available: {:?})",
                        field.states
                    );
                }
                dict.set("V", Object::Name(state.clone().into_bytes()));
                dict.set("AS", Object::Name(state.clone().into_bytes()));
                filled.push(json!({"name": name, "type": "radio", "state": state}));
            }
            _ => {
                // text / dropdown / listbox：/V 直接写字符串
                dict.set(
                    "V",
                    Object::String(value_str.clone().into_bytes(), lopdf::StringFormat::Literal),
                );
                filled.push(json!({"name": name, "type": field.ft, "value": value_str}));
            }
        }
    }

    // NeedAppearances = true：让查看器重算字段外观
    if !filled.is_empty() {
        if let Ok(Object::Dictionary(acro_dict)) = doc.catalog_mut()?.get_mut(b"AcroForm") {
            acro_dict.set("NeedAppearances", Object::Boolean(true));
        }
    }

    doc.save(out)?;
    if !missing.is_empty() {
        anyhow::bail!("fields not found: {missing:?}");
    }
    Ok(json!({"output": out.display().to_string(), "filled": filled.len(), "fields": filled}))
}
