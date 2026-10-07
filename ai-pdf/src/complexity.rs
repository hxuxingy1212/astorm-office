//! 页面复杂度判定与文本可信性（借鉴 liteparse 的 selective-OCR 决策与
//! 文本后处理思路，参数按本工具的行级粒度重新校准）。
//!
//! 消费方：
//! 1. `extract text`：同位重复项去重（CAD/收据页的图层堆叠）；
//! 2. OCR 判定：什么样的页需要送 OCR（扫描件/无文本层/乱码）；
//! 3. `--json` 诊断：complexity 字段输出（`scanned`/`garbled` 等结构化理由）。

use crate::text::{Line, TextItem};
use serde_json::{json, Value};

/// 页面复杂度判定（各理由与 liteparse ComplexityReason 思路对齐，
/// 阈值按本工具行级粒度校准）。
#[derive(Debug, Clone, PartialEq)]
pub enum ComplexityReason {
    /// 无文本层或 <20 字符（送 OCR 的主判据）
    NoText,
    /// 少文本 + 大图占页（典型扫描页）
    Scanned,
    /// 文本少且覆盖稀疏
    SparseText,
    /// 嵌入大图存在
    EmbeddedImages,
    /// 乱码字体（ToUnicode 缺失，元音比判据）
    Garbled,
}

/// 单页判定结果
#[derive(Debug, Clone)]
pub struct PageComplexity {
    pub reasons: Vec<ComplexityReason>,
    pub text_length: usize,
    /// 可信文本包围盒面积 / 页面面积（0-1），无文本为 0
    pub text_coverage: f64,
}

pub const MIN_NATIVE_TEXT_LEN: usize = 20;
pub const SPARSE_TEXT_LEN: usize = 400;
pub const SPARSE_COVER: f64 = 0.05;
/// 乱码判定的最小字母数
pub const GARBLE_MIN_LETTERS: usize = 30;
/// 元音比阈值（正常英文 ≈37%）
pub const GARBLE_MIN_VOWEL_RATIO: f64 = 0.20;

/// 元音比判据：拉丁字母中元音占比 <20% 即乱码（bad cmap 的产物是随机
/// 字形名，如 "Ò¼ü±"）。非拉丁（CJK）不适用；字母 <30 不判。
pub fn is_garbled(text: &str) -> bool {
    let letters: Vec<char> = text.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    if letters.len() < GARBLE_MIN_LETTERS {
        return false;
    }
    let vowels = letters
        .iter()
        .filter(|c| matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u'))
        .count();
    (vowels as f64 / letters.len() as f64) < GARBLE_MIN_VOWEL_RATIO
}

/// 判定一页文本的复杂度（是否需要 OCR）。
/// `full_page_image`：单张大图铺满页；`image_area`：大图总面积（pt²）。
pub fn calculate_page_complexity(
    lines: &[Line],
    page_area: f64,
    full_page_image: bool,
    image_area: f64,
) -> PageComplexity {
    let text_length: usize = lines
        .iter()
        .flat_map(|l| &l.items)
        .map(|i| i.text.chars().count())
        .sum();
    let page_area = page_area.max(1.0);

    // 文本覆盖率：行级 y-band 合并 + x 区间并集（近似，不做像素积分）
    let mut bands: Vec<(f64, f64, f64, f64)> = Vec::new(); // (y0,y1,xmin,xmax)
    for line in lines {
        let Some(first) = line.items.first() else {
            continue;
        };
        let (y0, y1) = (line.y - first.size, line.y);
        let (xmin, xmax) = (
            line.items.iter().map(|i| i.x).fold(f64::INFINITY, f64::min),
            line.items
                .iter()
                .map(|i| i.end_x)
                .fold(f64::NEG_INFINITY, f64::max),
        );
        if let Some(b) = bands
            .iter_mut()
            .find(|b| y0 < b.1 + first.size && y1 + first.size > b.0)
        {
            b.0 = b.0.min(y0);
            b.1 = b.1.max(y1);
            b.2 = b.2.min(xmin);
            b.3 = b.3.max(xmax);
        } else {
            bands.push((y0, y1, xmin, xmax));
        }
    }
    let covered: f64 = bands
        .iter()
        .map(|(y0, y1, x0, x1)| (y1 - y0).max(0.0) * (x1 - x0).max(0.0))
        .sum::<f64>()
        / page_area;
    let text_coverage = covered.clamp(0.0, 1.0);

    let mut reasons = Vec::new();
    if text_length < MIN_NATIVE_TEXT_LEN {
        reasons.push(ComplexityReason::NoText);
    } else if full_page_image && text_length < 60 {
        reasons.push(ComplexityReason::Scanned);
    } else if text_length < SPARSE_TEXT_LEN && text_coverage < SPARSE_COVER {
        reasons.push(ComplexityReason::SparseText);
    }
    if image_area > 0.25 * page_area || full_page_image {
        reasons.push(ComplexityReason::EmbeddedImages);
    }

    // 乱码：整页拼串判一次元音比（识别"整页都是坏字体"场景）
    let all_text: String = lines
        .iter()
        .flat_map(|l| &l.items)
        .map(|i| i.text.as_str())
        .collect();
    if is_garbled(&all_text) {
        reasons.push(ComplexityReason::Garbled);
    }
    PageComplexity {
        reasons,
        text_length,
        text_coverage,
    }
}

/// 判定是否需要 OCR（NoText / Scanned / Garbled 即送）
pub fn needs_ocr(reasons: &[ComplexityReason]) -> bool {
    reasons.iter().any(|r| {
        matches!(
            r,
            ComplexityReason::NoText | ComplexityReason::Scanned | ComplexityReason::Garbled
        )
    })
}

/// `--json` 的 complexity 字段
pub fn complexity_json(c: &PageComplexity) -> Value {
    json!({
        "text_length": c.text_length,
        "text_coverage": (c.text_coverage * 1000.0).round() / 1000.0,
        "needs_ocr": needs_ocr(&c.reasons),
        "reasons": c
            .reasons
            .iter()
            .map(|r| match r {
                ComplexityReason::NoText => "no_text",
                ComplexityReason::Scanned => "scanned",
                ComplexityReason::SparseText => "sparse_text",
                ComplexityReason::EmbeddedImages => "embedded_images",
                ComplexityReason::Garbled => "garbled",
            })
            .collect::<Vec<_>>(),
    })
}

// ── 同位重复项去重（空间网格；借鉴 liteparse dedup_overlapping_items）──────
// CAD / 收据 / 图层堆叠的 PDF：同一像素位置的文本对象被重复放置多次。
// 空间网格划分（cell 尺寸随平均项尺寸自适应），仅比较同格候选；
// 规则：同文本 >50% 重叠 → 删后序；异文本小项被大项 >80% 覆盖 → 删被盖者。
// 结果与比较顺序无关（成对判定，删除只取决于对）。

fn span_area(s: &TextItem) -> f64 {
    ((s.end_x - s.x) * s.size).max(0.01)
}

/// 去重同位重复项（跨行比较；行坐标来自 Line.y）。
/// 空间网格（cell = 平均项宽 clamp 8..256pt）划分，仅比较同格候选：
/// 同文本 >50% 重叠 → 删后序；异文本小项被大项 >80% 覆盖 → 删被盖者。
pub fn dedup_overlapping_items(mut lines: Vec<Line>) -> Vec<Line> {
    let total: usize = lines.iter().map(|l| l.items.len()).sum();
    if total < 8 {
        return lines;
    }
    let avg_w = lines
        .iter()
        .flat_map(|l| &l.items)
        .map(|i| i.end_x - i.x)
        .sum::<f64>()
        / total as f64;
    let cell = avg_w.clamp(8.0, 256.0);

    // 扁平化：flat idx → (line, item)；网格桶 (x/cell, y/cell) → flat idx 列表
    let mut flat_line = Vec::with_capacity(total);
    let mut flat_item = Vec::with_capacity(total);
    for (li, line) in lines.iter().enumerate() {
        for ii in 0..line.items.len() {
            flat_line.push(li);
            flat_item.push(ii);
        }
    }
    let mut buckets: std::collections::BTreeMap<(i64, i64), Vec<usize>> = Default::default();
    for f in 0..total {
        let (li, ii) = (flat_line[f], flat_item[f]);
        let it = &lines[li].items[ii];
        buckets
            .entry(((it.x / cell) as i64, (lines[li].y / cell) as i64))
            .or_default()
            .push(f);
    }

    let mut removed = vec![false; total];
    for idx_list in buckets.values() {
        if idx_list.len() < 2 {
            continue;
        }
        for i in 0..idx_list.len() {
            let fa = idx_list[i];
            if removed[fa] {
                continue;
            }
            let (la, ia) = (flat_line[fa], flat_item[fa]);
            for &fb in &idx_list[i + 1..] {
                if removed[fb] {
                    continue;
                }
                let (lb, ib) = (flat_line[fb], flat_item[fb]);
                let a = &lines[la].items[ia];
                let b = &lines[lb].items[ib];
                let ox = a.end_x.min(b.end_x) - a.x.max(b.x);
                let oy =
                    (lines[la].y + a.size).min(lines[lb].y + b.size) - lines[la].y.max(lines[lb].y);
                if ox <= 0.0 || oy <= 0.0 {
                    continue;
                }
                let inter = ox * oy;
                let (area_a, area_b) = (span_area(a), span_area(b));
                if a.text.trim() == b.text.trim() && inter / area_a.min(area_b) > 0.5 {
                    removed[fb] = true; // 同文本重画：删后序
                } else if inter / area_a.min(area_b) > 0.8 && (area_a - area_b).abs() > f64::EPSILON
                {
                    // 异文本、小项几乎被大项完全覆盖：视为被盖旧层
                    let (small, large) = if area_a < area_b { (fa, fb) } else { (fb, fa) };
                    removed[small] = true;
                    let _ = large;
                }
            }
        }
    }

    let mut f = 0usize;
    for line in lines.iter_mut() {
        let keep_from = f;
        f += line.items.len();
        if !removed[keep_from..f].iter().any(|r| *r) {
            continue;
        }
        line.items = line
            .items
            .iter()
            .enumerate()
            .filter(|(ii, _)| !removed[keep_from + ii])
            .map(|(_, it)| it.clone())
            .collect();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextItem;

    fn item(x: f64, text: &str) -> TextItem {
        TextItem {
            x,
            end_x: x + text.chars().count() as f64 * 6.0,
            size: 12.0,
            text: text.to_string(),
        }
    }

    fn line(y: f64, items: Vec<TextItem>) -> Line {
        Line { y, items }
    }

    #[test]
    fn garbled_detection() {
        // bad cmap 的典型产物里极少元音
        // 坏 cmap 产物的典型形态：随机拉丁字形名（罕见元音）
        assert!(is_garbled(
            "Wkh txlfn eurzq ira mxpsv ryhu wkh odcb grj dqg uxqv dzdb dqg idvw"
        ));
        assert!(!is_garbled(
            "The quick brown fox jumps over the lazy dog and runs away"
        ));
        assert!(!is_garbled("bcd xyz")); // 短样本不判
    }

    #[test]
    fn empty_page_needs_ocr() {
        let c = calculate_page_complexity(&[], 595.28 * 841.89, false, 0.0);
        assert!(c.reasons.contains(&ComplexityReason::NoText));
        assert!(needs_ocr(&c.reasons));
    }

    #[test]
    fn normal_text_does_not_need_ocr() {
        let lines = vec![
            line(
                720.0,
                vec![item(
                    72.0,
                    "Hello PDF, this is a normal text page with plenty of",
                )],
            ),
            line(
                700.0,
                vec![item(
                    72.0,
                    "lines of text so the complexity classifier is quite happy here.",
                )],
            ),
        ];
        let c = calculate_page_complexity(&lines, 595.0 * 841.89, false, 0.0);
        assert!(!needs_ocr(&c.reasons));
    }
}
