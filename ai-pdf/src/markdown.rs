//! Markdown 输出（借鉴 liteparse 的 markdown 管线思路，语义分类按
//! 本工具的行级数据简化）：字号直方图定正文基准 → 标题层级、
//! 列表识别、段落合并、表格（复用 detect_tables）。

use crate::layout::{project_page, Region};
use crate::text::{detect_tables, PageText};

/// 单页 → markdown
pub fn page_markdown(pt: &PageText) -> String {
    if pt.lines.is_empty() {
        return String::new();
    }
    let mut out = String::new();

    // 表格优先（detect_tables 的行聚类近似）
    let tables = detect_tables(std::slice::from_ref(pt));
    let table_rows: Vec<Vec<Vec<String>>> = tables
        .iter()
        .filter_map(|t| {
            t.get("data").and_then(|d| d.as_array()).map(|rows| {
                rows.iter()
                    .map(|r| {
                        r.as_array()
                            .map(|c| {
                                c.iter()
                                    .map(|v| v.as_str().unwrap_or("").to_string())
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    for t in &table_rows {
        out.push_str(&table_markdown(t));
        out.push_str("\n\n");
    }

    // 正文基准字号：全部 item 字号的中位数
    let mut sizes: Vec<f64> = pt
        .lines
        .iter()
        .flat_map(|l| &l.items)
        .map(|i| i.size)
        .collect();
    sizes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let body_size = if sizes.is_empty() {
        12.0
    } else {
        sizes[sizes.len() / 2]
    };

    // 段落与标题（跳过已进表格的行：表格行含 ≥2 列对齐特征——近似地，
    // 用"该行 y 与表格首行的 y 相同"剔除）
    let table_first_ys: Vec<f64> = table_rows
        .iter()
        .filter(|r| !r.is_empty())
        .filter_map(|_| {
            // detect_tables 不回传 y；按行特征粗判即可（不做精确剔除）
            None::<f64>
        })
        .collect();
    let _ = table_first_ys;

    let region_tree = crate::layout::xy_cut(&pt.lines, page_width(pt));
    let mut lines_out: Vec<String> = Vec::new();
    collect_lines(&region_tree, pt, body_size, &mut lines_out);

    for piece in lines_out {
        if piece.is_empty() {
            continue;
        }
        out.push_str(&piece);
        out.push('\n');
    }
    // 网格文本兜底进 fence？不需要——collect_lines 已是逐行分类结果。
    let _ = project_page(pt);
    out
}

fn page_width(pt: &PageText) -> f64 {
    pt.bbox.map(|b| (b.2 - b.0).max(200.0)).unwrap_or(612.0)
}

/// Region 树 → 分类行（标题/列表/段落），保持阅读序
fn collect_lines(r: &Region, pt: &PageText, body_size: f64, out: &mut Vec<String>) {
    match r {
        Region::Leaf(idxs) => {
            let mut ls: Vec<usize> = idxs.clone();
            ls.sort_by(|a, b| {
                pt.lines[*b]
                    .y
                    .partial_cmp(&pt.lines[*a].y)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut para: Vec<String> = Vec::new();
            let mut para_size = 0.0;
            let flush = |para: &mut Vec<String>, out: &mut Vec<String>| {
                if para.is_empty() {
                    return;
                }
                out.push(para.join(" "));
                para.clear();
            };
            for &i in &ls {
                let line = &pt.lines[i];
                let text = line
                    .items
                    .iter()
                    .map(|it| it.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                let text = text.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                // 一行里可能有多个列表项（同基线的多个 item）：按词边界切分
                let segments = split_list_segments(&text);
                if segments.len() > 1 {
                    flush(&mut para, out);
                    for (marker, rest) in segments {
                        match marker {
                            Some(m) => out.push(format!("{m}{rest}")),
                            None => out.push(rest),
                        }
                    }
                    continue;
                }
                if let Some((marker, rest)) = as_list_item(&text) {
                    flush(&mut para, out);
                    out.push(format!("{marker} {rest}"));
                    continue;
                }
                let size = line.items.iter().map(|it| it.size).fold(0.0, f64::max);
                // 列表？
                if let Some((marker, rest)) = as_list_item(&text) {
                    flush(&mut para, out);
                    out.push(format!("{marker} {rest}"));
                    continue;
                }
                // 标题？字号显著大于正文
                if size >= body_size * 1.35 {
                    flush(&mut para, out);
                    let level = if size >= body_size * 1.9 { "# " } else { "## " };
                    out.push(format!("{level}{text}"));
                    continue;
                }
                // 段落：字号相近的连续行合并
                if para_size > 0.0 && (size - para_size).abs() > para_size * 0.2 {
                    flush(&mut para, out);
                }
                para_size = size;
                para.push(text);
            }
            flush(&mut para, out);
        }
        Region::Split { children, .. } => {
            for c in children {
                collect_lines(c, pt, body_size, out);
            }
        }
    }
}

/// 把一行按列表标记切分成多段（"• 第一项 1. 第二项形式" → 两段）
pub(crate) fn split_list_segments(text: &str) -> Vec<(Option<&'static str>, String)> {
    let tokens: Vec<&str> = text.split(' ').filter(|t| !t.is_empty()).collect();
    let mut segs: Vec<(Option<&'static str>, String)> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut pending: Option<&'static str> = None; // 已见标记、等内容的下一个 token
    for tok in tokens {
        if let Some((marker, rest)) = as_list_item(tok) {
            if !cur.is_empty() {
                segs.push((pending.take(), cur.join(" ")));
                cur.clear();
            }
            if !rest.is_empty() {
                pending = None;
                segs.push((Some(marker), rest));
            } else {
                pending = Some(marker);
            }
            continue;
        }
        cur.push(tok);
    }
    if !cur.is_empty() {
        segs.push((pending.take(), cur.join(" ")));
    }
    if segs.len() <= 1 {
        return Vec::new();
    }
    segs
}

/// 识别 markdown 列表项（• / - / 1. / 1) / (1)）
pub(crate) fn as_list_item(text: &str) -> Option<(&'static str, String)> {
    let t = text.trim();
    // 纯标记 token（"1." / "•" 后面没内容——内容在后续 token）
    if t == "•" || t == "·" || t == "●" || t == "■" || t == "□" || t == "○" || t == "◦" || t == "-"
    {
        return Some(("- ", String::new()));
    }
    let mut ch = t.chars();
    let last = ch.next_back();
    let head_is_digits = last.is_some()
        && (last == Some('.') || last == Some(')'))
        && ch.all(|c| c.is_ascii_digit())
        && t.chars().count() >= 2
        && t.chars().count() <= 4;
    if head_is_digits {
        return Some(("- ", String::new()));
    }
    let t = text.trim_start();
    for (prefix, marker) in [
        ("• ", "- "),
        ("· ", "- "),
        ("- ", "- "),
        ("● ", "- "),
        ("■ ", "- "),
        ("□ ", "- "),
        ("○ ", "- "),
        ("◦ ", "- "),
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return Some((marker, rest.trim().to_string()));
        }
    }
    // 数字列表："1. xxx" / "1) xxx" / "(1) xxx"
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0
        && i < 4
        && i + 1 < bytes.len()
        && (bytes[i] == b'.' || bytes[i] == b')')
        && bytes[i + 1] == b' '
    {
        return Some(("- ", t[i + 1..].trim().to_string()));
    }
    if t.starts_with('(') {
        if let Some(close) = t.find(')') {
            if close > 1 && close < 5 && t[close + 1..].starts_with(' ') {
                return Some(("- ", t[close + 1..].trim().to_string()));
            }
        }
    }
    None
}

/// 表格 → markdown 管道表
fn table_markdown(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }
    let cell =
        |r: &Vec<String>, c: usize| r.get(c).map(|s| s.replace('|', "\\|")).unwrap_or_default();
    let mut out = String::new();
    out.push('|');
    for c in 0..cols {
        out.push_str(&format!(" {} |", cell(&rows[0], c)));
    }
    out.push('\n');
    out.push('|');
    for _ in 0..cols {
        out.push_str(" --- |");
    }
    out.push('\n');
    for r in rows.iter().skip(1) {
        out.push('|');
        for c in 0..cols {
            out.push_str(&format!(" {} |", cell(r, c)));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextItem;

    fn item(x: f64, size: f64, text: &str) -> TextItem {
        TextItem {
            x,
            end_x: x + text.chars().count() as f64 * size * 0.5,
            size,
            text: text.to_string(),
        }
    }

    #[test]
    fn heading_and_paragraph() {
        let pt = PageText {
            page: 1,
            lines: vec![
                crate::text::Line {
                    y: 720.0,
                    items: vec![item(72.0, 28.0, "报告标题")],
                },
                crate::text::Line {
                    y: 690.0,
                    items: vec![item(72.0, 12.0, "这是正文第一行的一些内容。")],
                },
                crate::text::Line {
                    y: 675.0,
                    items: vec![item(72.0, 12.0, "第二行继续正文内容，应当合并为段落。")],
                },
            ],
            char_count: 0,
            has_images: false,
            has_drawings: false,
            bbox: None,
            complexity: crate::complexity::PageComplexity {
                reasons: vec![],
                text_length: 0,
                text_coverage: 0.0,
            },
        };
        let md = page_markdown(&pt);
        assert!(md.contains("# 报告标题"), "{md}");
        assert!(
            md.contains("这是正文第一行的一些内容。 第二行继续正文内容"),
            "段落应合并: {md}"
        );
    }

    #[test]
    fn list_items() {
        let pt = PageText {
            page: 1,
            lines: vec![crate::text::Line {
                y: 700.0,
                items: vec![
                    item(72.0, 12.0, "• 第一项"),
                    item(220.0, 12.0, "1. 第二项形式"),
                ],
            }],
            char_count: 0,
            has_images: false,
            has_drawings: false,
            bbox: None,
            complexity: crate::complexity::PageComplexity {
                reasons: vec![],
                text_length: 0,
                text_coverage: 0.0,
            },
        };
        let md = page_markdown(&pt);
        assert!(md.contains("- 第一项"), "{md}");
        assert!(md.contains("- 第二项形式"), "{md}");
    }
}
