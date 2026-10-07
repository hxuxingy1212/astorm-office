//! 元数据 — 复刻 pdf.py meta.get / meta.set / meta.brand。

use crate::util::pdf_date_now;
use anyhow::Result;
use lopdf::{Dictionary, Document, Object, StringFormat};
use serde_json::{json, Value};
use std::path::Path;

fn info_id(doc: &Document) -> Option<(u32, u16)> {
    doc.trailer.get(b"Info").and_then(Object::as_reference).ok()
}

/// 读 Info 字典为 JSON（键原样、值解码为字符串）。
fn info_json(doc: &Document) -> Value {
    let Some(id) = info_id(doc) else {
        return json!({});
    };
    let Ok(dict) = doc.get_object(id).and_then(Object::as_dict) else {
        return json!({});
    };
    let mut map = serde_json::Map::new();
    for (k, v) in dict.iter() {
        let key = String::from_utf8_lossy(k).to_string();
        let val = match v {
            Object::String(bytes, _) => decode_text_string(bytes),
            other => serde_json::to_value(format!("{other:?}")).unwrap_or(Value::Null),
        };
        map.insert(key, val);
    }
    Value::Object(map)
}

/// PDF 文本字符串：UTF-16BE BOM → 解码，否则按 UTF-8/latin1 尽力。
pub fn str_lossy(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| ((c[0] as u16) << 8) | c[1] as u16)
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(bytes).to_string()
    }
}

pub fn json_str(bytes: &[u8]) -> Value {
    Value::String(str_lossy(bytes))
}

fn decode_text_string(bytes: &[u8]) -> Value {
    json_str(bytes)
}

/// Info 字典 → Map<String, Value>（qa 等模块复用）。
pub fn get_info_map(doc: &Document) -> serde_json::Map<String, Value> {
    match info_json(doc) {
        Value::Object(m) => m,
        _ => serde_json::Map::new(),
    }
}

pub fn get(file: &Path) -> Result<Value> {
    let doc = Document::load(file)?;
    let page_map = doc.get_pages();
    let page_size = page_map
        .values()
        .next()
        .and_then(|&pid| crate::page_ops::page_mediabox(&doc, pid))
        .map(|(w, h, _)| json!({"width": round2(w), "height": round2(h)}));

    let has_acroform = doc.catalog().map(|c| c.has(b"AcroForm")).unwrap_or(false);
    let has_outlines = doc.catalog().map(|c| c.has(b"Outlines")).unwrap_or(false);

    Ok(json!({
        "file": file.display().to_string(),
        "pages": page_map.len(),
        "pdf_version": doc.version,
        "encrypted": doc.is_encrypted(),
        "acroform": has_acroform,
        "outlines": has_outlines,
        "first_page_size": page_size,
        "info": info_json(&doc),
    }))
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Info 字符串编码：ASCII 直接写，非 ASCII 用 UTF-16BE(BOM)。
fn encode_text_string(text: &str) -> Vec<u8> {
    if text.is_ascii() {
        text.as_bytes().to_vec()
    } else {
        let mut out = vec![0xFE, 0xFF];
        for u in text.encode_utf16() {
            out.push((u >> 8) as u8);
            out.push((u & 0xFF) as u8);
        }
        out
    }
}

fn set_info_entries(doc: &mut Document, entries: &[(String, String)]) -> Result<()> {
    let existing = info_id(doc);
    let info_id = match existing {
        Some(id) if doc.get_object(id).is_ok() => id,
        _ => {
            let id = doc.add_object(Object::Dictionary(Dictionary::new()));
            doc.trailer.set("Info", Object::Reference(id));
            id
        }
    };
    let dict = doc.get_object_mut(info_id).and_then(Object::as_dict_mut)?;
    for (k, v) in entries {
        dict.set(
            k.as_bytes().to_vec(),
            Object::String(encode_text_string(v), StringFormat::Literal),
        );
    }
    Ok(())
}

const SETTABLE: &[&str] = &[
    "Title", "Author", "Subject", "Keywords", "Creator", "Producer",
];

pub fn set(file: &Path, out: &Path, data: &serde_json::Map<String, Value>) -> Result<Value> {
    let mut doc = Document::load(file)?;
    let mut entries: Vec<(String, String)> = Vec::new();
    for key in SETTABLE {
        if let Some(v) = data.get(*key) {
            let s = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            entries.push((key.to_string(), s));
        }
    }
    if entries.is_empty() {
        anyhow::bail!("no settable keys found (allowed: {SETTABLE:?})");
    }
    entries.push(("ModDate".into(), pdf_date_now()));
    let updated: Vec<String> = entries.iter().map(|(k, _)| k.clone()).collect();
    set_info_entries(&mut doc, &entries)?;
    doc.save(out)?;
    Ok(json!({"output": out.display().to_string(), "updated": updated}))
}

/// 品牌元数据批量写入：Author/Creator=Z.ai，Producer=http://z.ai。
/// 多文件时原地覆盖（与原版一致）；单文件可用 -o 指定输出。
pub fn brand(
    files: &[std::path::PathBuf],
    out: Option<&Path>,
    title: Option<&str>,
) -> Result<Value> {
    let multi = files.len() > 1;
    if multi && out.is_some() {
        anyhow::bail!("-o cannot be used with multiple files");
    }
    let mut results = Vec::new();
    for f in files {
        let dest: std::path::PathBuf = if !multi {
            out.map(|p| p.to_path_buf()).unwrap_or_else(|| f.clone())
        } else {
            f.clone()
        };
        let mut doc = Document::load(f)?;
        let mut entries = vec![
            ("Author".to_string(), "Z.ai".to_string()),
            ("Creator".to_string(), "Z.ai".to_string()),
            ("Producer".to_string(), "http://z.ai".to_string()),
        ];
        if let Some(t) = title {
            entries.push(("Title".to_string(), t.to_string()));
        }
        entries.push(("ModDate".to_string(), pdf_date_now()));
        set_info_entries(&mut doc, &entries)?;
        doc.save(&dest)?;
        results
            .push(json!({"file": f.display().to_string(), "output": dest.display().to_string()}));
    }
    Ok(json!({"branded": results.len(), "files": results}))
}
