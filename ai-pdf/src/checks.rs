//! 检查 — 复刻 pdf.py font.check / toc.check 的可移植子集：
//! font.check 做字符级扫描（U+FFFD/控制/零宽/Bidi/变体选择符/PUA）；
//! 字形级 .notdef 检测需要字体渲染，见 README 差异说明。
//! toc.check 解析目录页条目并校验页码单调性。

use crate::text;
use anyhow::Result;
use lopdf::Document;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Default)]
struct KindStat {
    pages: Vec<u32>,
    count: usize,
}

fn char_kind(c: char) -> Option<&'static str> {
    let u = c as u32;
    if c == '\u{FFFD}' {
        Some("replacement_char")
    } else if (u < 0x20 && !matches!(c, '\t' | '\n' | '\r'))
        || u == 0x7F
        || (0x80..=0x9F).contains(&u)
    {
        Some("control")
    } else if matches!(u, 0x200B..=0x200D | 0xFEFF | 0x2060) {
        Some("zero_width")
    } else if matches!(u, 0x202A..=0x202E | 0x2066..=0x2069 | 0x200E | 0x200F) {
        Some("bidi_control")
    } else if matches!(u, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF) {
        Some("variation_selector")
    } else if (0xE000..=0xF8FF).contains(&u) || (0xF0000..=0xFFFFD).contains(&u) {
        Some("private_use")
    } else {
        None
    }
}

pub fn font_check(file: &Path) -> Result<(Value, i32)> {
    let doc = Document::load(file)?;
    let pages = text::extract_all(&doc)?;
    let mut stats: std::collections::BTreeMap<&'static str, KindStat> = Default::default();
    for pt in &pages {
        let text = pt.text();
        for c in text.chars() {
            if let Some(kind) = char_kind(c) {
                let stat = stats.entry(kind).or_default();
                stat.count += 1;
                if !stat.pages.contains(&pt.page) {
                    stat.pages.push(pt.page);
                }
            }
        }
    }
    let problems: serde_json::Map<String, Value> = stats
        .iter()
        .map(|(kind, s)| {
            (
                kind.to_string(),
                json!({"count": s.count, "pages": s.pages}),
            )
        })
        .collect();
    let clean = problems.is_empty();
    let mut fix_hints = Vec::new();
    if problems.contains_key("replacement_char") {
        fix_hints.push("missing glyphs: the embedded font has no glyph for some characters — switch to a CJK-capable font or sanitize content (content sanitize)");
    }
    if problems.contains_key("bidi_control") || problems.contains_key("zero_width") {
        fix_hints.push("invisible control characters found — run `json2pdf content sanitize --apply` on the source text");
    }
    let v = json!({
        "file": file.display().to_string(),
        "pages_checked": pages.len(),
        "clean": clean,
        "problems": problems,
        "fix_hints": fix_hints,
    });
    let code = if clean { 0 } else { 1 };
    Ok((v, code))
}

pub fn toc_check(file: &Path) -> Result<(Value, i32)> {
    let doc = Document::load(file)?;
    let pages = text::extract_all(&doc)?;
    let head_pages: Vec<&text::PageText> = pages.iter().take(8).collect();

    // 1. 找目录标题行
    let mut toc_page = None;
    let mut entry_lines: Vec<String> = Vec::new();
    for pt in head_pages {
        let text = pt.text();
        let mut found_heading = toc_page.is_some();
        for line in text.lines() {
            let t = line.trim();
            if !found_heading {
                let is_heading = (t.contains("目录")
                    || t.to_lowercase().contains("contents")
                    || t.to_lowercase().contains("table of contents"))
                    && t.chars().count() < 40;
                if is_heading {
                    found_heading = true;
                    if toc_page.is_none() {
                        toc_page = Some(pt.page);
                    }
                }
                continue;
            }
            // 2. 标题后的条目行：以数字结尾
            if !t.is_empty() && t.chars().count() < 120 {
                entry_lines.push(t.to_string());
            }
        }
    }

    // 条目解析：text .... 12
    let mut entries: Vec<(String, u64)> = Vec::new();
    for line in &entry_lines {
        let trimmed = line.trim_end_matches(['.', ' ', '·', '…']);
        if let Some(digits_start) = trimmed.rfind(|c: char| c.is_ascii_digit()) {
            let (head, tail) = trimmed.split_at(digits_start);
            let head = head.trim_end_matches(['.', ' ', '·', '…']).trim();
            if let Ok(page_no) = tail.parse::<u64>() {
                if !head.is_empty() {
                    entries.push((head.to_string(), page_no));
                }
            }
        }
    }
    // 只保留像条目的行（数量截断防误吞正文）
    entries.truncate(200);

    let mut issues = Vec::new();
    let mut code = 0;
    let Some(tp) = toc_page else {
        issues.push(json!({"code": "TOC_NOT_FOUND", "message": "no TOC heading found in the first 8 pages", "fix": "add a TOC section titled 目录/Contents with dot-leader page numbers"}));
        let v = json!({"file": file.display().to_string(), "status": "fail", "issues": issues, "entries": []});
        return Ok((v, 1));
    };
    if entries.is_empty() {
        issues.push(json!({"code": "TOC_NO_ENTRIES", "message": "TOC heading found but no 'title ... N' entries parsed", "fix": "entries must end with a page number"}));
        code = 1;
    } else {
        let nums: Vec<u64> = entries.iter().map(|(_, n)| *n).collect();
        let all_same = nums.iter().all(|n| *n == nums[0]);
        if all_same {
            issues.push(json!({"code": "TOC_ALL_SAME_PAGE", "message": format!("all {} entries point to page {}", nums.len(), nums[0]), "fix": "page numbers were never computed — regenerate TOC after final pagination"}));
            code = 1;
        }
        let non_monotonic = nums.windows(2).any(|w| w[1] < w[0]);
        if non_monotonic {
            issues.push(json!({"code": "TOC_PAGES_INVALID", "message": "entry page numbers are not monotonically non-decreasing", "fix": "rebuild TOC from final rendered pages"}));
            code = 1;
        }
    }
    if tp == 1 {
        issues.push(json!({"code": "TOC_ON_FIRST_PAGE", "message": "TOC appears on page 1 (no cover page)", "fix": "consider adding a cover so the TOC does not open the document"}));
        if code == 0 {
            code = 1;
        }
    }
    let status = if code == 0 { "pass" } else { "fail" };
    let v = json!({
        "file": file.display().to_string(),
        "status": status,
        "toc_page": tp,
        "entries": entries.iter().map(|(t, n)| json!({"title": t, "page": n})).take(50).collect::<Vec<_>>(),
        "issues": issues,
    });
    Ok((v, code))
}
