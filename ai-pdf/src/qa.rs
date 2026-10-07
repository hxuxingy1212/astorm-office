//! 成品体检 — 复刻 pdf_qa.py 的可移植子集：
//! 元数据三字段、页尺寸一致性、空白页、CJK 标点禁则、内容越界（2pt 容差）、
//! 边距对称性。fill-rate / full-bleed / 表格居中依赖精确字形盒，见 README 差异说明。

use crate::text;
use anyhow::Result;
use lopdf::Document;
use serde_json::{json, Value};
use std::path::Path;

const LINE_START_FORBIDDEN: &[char] = &[
    '，', '。', '、', '；', '：', '？', '！', '”', '’', '）', '》', '】', '」', '』', '%', '．',
];
const LINE_END_FORBIDDEN: &[char] = &['“', '‘', '（', '《', '【', '「', '『'];

pub fn qa(files: &[std::path::PathBuf], skip_cover: bool) -> Result<(Value, i32)> {
    let mut reports = Vec::new();
    let mut any_fail = false;
    for file in files {
        let r = qa_one(file, skip_cover)?;
        if r["status"] == "fail" {
            any_fail = true;
        }
        reports.push(r);
    }
    let overall = if any_fail { "fail" } else { "pass" };
    Ok((
        json!({"overall": overall, "files": reports}),
        if any_fail { 1 } else { 0 },
    ))
}

fn qa_one(file: &Path, skip_cover: bool) -> Result<Value> {
    let doc = Document::load(file)?;
    let pages = text::extract_all(&doc)?;
    let page_map = doc.get_pages();
    let mut fails = Vec::new();
    let mut warns = Vec::new();

    // 1. 元数据三字段
    let info = crate::meta::get_info_map(&doc);
    for key in ["Title", "Author", "Creator"] {
        if info
            .get(key)
            .map(|v| v.as_str().unwrap_or("").is_empty())
            .unwrap_or(true)
        {
            warns.push(json!({"check": "metadata", "message": format!("info field {key} is empty"), "fix": "run `json2pdf meta set` / `meta brand`"}));
        }
    }

    // 2. 页尺寸一致性
    let mut sizes: Vec<String> = Vec::new();
    for (pno, pid) in &page_map {
        if let Some((w, h, _)) = crate::page_ops::page_mediabox(&doc, *pid) {
            sizes.push(format!("{:.1}x{:.1}", w, h));
            let _ = pno;
        }
    }
    sizes.dedup();
    if sizes.len() > 1 {
        fails.push(json!({"check": "page_size_consistency", "message": format!("mixed page sizes: {sizes:?}")}));
    }

    // 3. 空白页 / 越界 / 边距对称 / CJK 禁则
    for pt in &pages {
        let is_blank = pt.char_count == 0 && !pt.has_images && !pt.has_drawings;
        if is_blank {
            fails.push(json!({"check": "blank_page", "page": pt.page, "message": "page has no text, images or drawings"}));
        }
        if let (Some(bbox), Some(&pid)) = (pt.bbox, page_map.get(&pt.page)) {
            if let Some((w, h, [l, b, r, t])) = crate::page_ops::page_mediabox(&doc, pid) {
                let tol = 2.0;
                if bbox.0 < l - tol || bbox.1 < b - tol || bbox.2 > r + tol || bbox.3 > t + tol {
                    fails.push(json!({"check": "content_overflow", "page": pt.page, "message": format!("text bbox {bbox:?} exceeds page box [{l}, {b}, {r}, {t}] (+/-{tol}pt)")}));
                }
                let _ = (w, h);
            }
        }
        // CJK 标点禁则（首行页码 1 为封面的场景由 skip_cover 控制；禁则全页检查）
        let text = pt.text();
        for line in text.lines() {
            let line = line.trim_end();
            if line.is_empty() {
                continue;
            }
            if let Some(first) = line.chars().next() {
                if LINE_START_FORBIDDEN.contains(&first) {
                    warns.push(json!({"check": "cjk_punctuation", "page": pt.page, "message": format!("line starts with forbidden punctuation '{first}'")}));
                }
            }
            if let Some(last) = line.chars().last() {
                if LINE_END_FORBIDDEN.contains(&last) {
                    warns.push(json!({"check": "cjk_punctuation", "page": pt.page, "message": format!("line ends with forbidden punctuation '{last}'")}));
                }
            }
        }
    }

    // 4. 边距对称（跳过封面 = 第 1 页）
    for pt in &pages {
        if skip_cover && pt.page == 1 {
            continue;
        }
        if let (Some(bbox), Some(&pid)) = (pt.bbox, page_map.get(&pt.page)) {
            if let Some((w, _, [l, _, r, _])) = crate::page_ops::page_mediabox(&doc, pid) {
                let left = bbox.0 - l;
                let right = r - bbox.2;
                if (left - right).abs() > w * 0.05 && left > 5.0 && right > 5.0 {
                    warns.push(json!({"check": "margin_symmetry", "page": pt.page, "message": format!("left margin {left:.1}pt vs right {right:.1}pt (diff > 5% of {w:.0}pt)")}));
                }
            }
        }
    }

    let status = if fails.is_empty() { "pass" } else { "fail" };
    Ok(json!({
        "file": file.display().to_string(),
        "pages": page_map.len(),
        "status": status,
        "fails": fails,
        "warnings": warns,
    }))
}
