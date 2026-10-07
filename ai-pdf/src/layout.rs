//! 版式重建（Grid Projection 的子集实现，借鉴 liteparse projection.rs）：
//! 把去重后的行重建为"字符网格"文本——列识别用 XY-cut 递归切分，
//! 水平距离用空格数模拟（锚点对齐），垂直距离用空行数模拟。
//!
//! 与 liteparse 全量实现的取舍：不做旋转阅读序重排（本工具行序即阅读序）、
//! 不做图形障碍印章、不做 floating 吸附 fixup——覆盖多栏/表格/缩进场景。

use crate::text::{Line, PageText};

/// 版式重建输出：每页一段"版式保留文本"。
/// Region 树驱动阅读序：列（竖切）先左后右，块（横切）自上而下；
/// 叶内按行序输出，垂直间隙 → 空行，列间隙 → 空格。
pub fn project_page(pt: &PageText) -> String {
    let lines = &pt.lines;
    let page_w = page_width_hint(pt);
    let regions = xy_cut(lines, page_w);
    let median_char = median_char_width(lines).max(1.0);
    let med_h = median_line_height(lines);
    render_region(&regions, lines, median_char, med_h)
}

/// Region 树 → 网格文本（`first` = 该叶/块是否为其父的首个输出，控制行首空行）
fn render_region(r: &Region, lines: &[Line], median_char: f64, med_h: f64) -> String {
    match r {
        Region::Leaf(idxs) => {
            let mut ls: Vec<usize> = idxs.clone();
            ls.sort_by(|a, b| {
                lines[*b]
                    .y
                    .partial_cmp(&lines[*a].y)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let mut out = String::new();
            let mut last_y: Option<f64> = None;
            for &i in &ls {
                let line = &lines[i];
                if let Some(ly) = last_y {
                    let blank = (((ly - line.y) / med_h) - 1.0).round();
                    if blank >= 1.0 {
                        for _ in 0..blank.clamp(1.0, 6.0) as usize {
                            out.push('\n');
                        }
                    }
                }
                out.push_str(&render_line_anchored(line, median_char));
                out.push('\n');
                last_y = Some(line.y);
            }
            out
        }
        Region::Split { children, .. } => {
            let mut out = String::new();
            for c in children.iter() {
                let piece = render_region(c, lines, median_char, med_h);
                if piece.is_empty() {
                    continue;
                }
                // 列/块之间用一个空行分隔（阅读序换段）
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(piece.trim_end_matches('\n'));
            }
            if out.is_empty() {
                out
            } else {
                out.push('\n');
                out
            }
        }
    }
}

fn page_width_hint(pt: &PageText) -> f64 {
    pt.bbox.map(|b| (b.2 - b.0).max(200.0)).unwrap_or(612.0)
}

fn median_char_width(lines: &[Line]) -> f64 {
    let mut ws: Vec<f64> = Vec::new();
    for line in lines {
        for it in &line.items {
            let n = it.text.chars().count().max(1);
            ws.push((it.end_x - it.x) / n as f64);
        }
    }
    if ws.is_empty() {
        return 6.0;
    }
    ws.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ws[ws.len() / 2]
}

fn median_line_height(lines: &[Line]) -> f64 {
    let mut ys: Vec<f64> = lines
        .iter()
        .map(|l| l.items.iter().map(|i| i.size).fold(0.0, f64::max))
        .collect();
    if ys.is_empty() {
        return 14.0;
    }
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    (ys[ys.len() / 2] * 1.25).max(8.0)
}

// ── XY-cut 列识别 ─────────────────────────────────────────────────────────

/// 切分树：Leaf(行索引列表) / Split(轴 + 子树)
#[derive(Debug, Clone)]
pub enum Region {
    Leaf(Vec<usize>),
    Split {
        vertical: bool,
        children: Vec<Region>,
    },
}

/// 递归 XY-cut：先横切（按 y 空隙分块）再竖切（按 x 空隙分列）。
/// 密度投影 bucket = 2pt；谷阈值 = 局部中位密度 × 0.1；最窄谷宽 8pt。
pub fn xy_cut(lines: &[Line], page_w: f64) -> Region {
    let idx: Vec<usize> = (0..lines.len()).collect();
    cut(idx, lines, true, page_w, 0)
}

fn cut(
    idxs: Vec<usize>,
    lines: &[Line],
    vertical_first: bool,
    page_w: f64,
    depth: usize,
) -> Region {
    if idxs.len() < 4 || depth > 8 {
        return Region::Leaf(idxs);
    }
    // 收集成员 bbox
    let bbox = |i: usize| {
        let l = &lines[i];
        let x0 = l.items.iter().map(|it| it.x).fold(f64::INFINITY, f64::min);
        let x1 = l
            .items
            .iter()
            .map(|it| it.end_x)
            .fold(f64::NEG_INFINITY, f64::max);
        let y1 = l.y;
        let size = l.items.iter().map(|it| it.size).fold(0.0, f64::max);
        (x0, y1 - size, x1, y1)
    };

    if vertical_first {
        // 竖切：x 密度投影（页宽 × 2pt bucket）
        let idxs = &idxs;
        let buckets = (page_w / 2.0).ceil() as usize;
        let mut density = vec![0u32; buckets];
        for &i in idxs {
            let (x0, _, x1, _) = bbox(i);
            let b0 = ((x0 / 2.0) as usize).min(buckets - 1);
            let b1 = (((x1 / 2.0) as usize) + 1).min(buckets);
            for b in density.iter_mut().take(b1).skip(b0) {
                *b += 1;
            }
        }
        let mut occupied: Vec<u32> = density.iter().copied().filter(|&d| d > 0).collect();
        if occupied.len() < 2 {
            return Region::Leaf(idxs.clone());
        }
        occupied.sort_unstable();
        let median = occupied[occupied.len() / 2].max(1);
        let threshold = (median as f64 * 0.10).max(0.5);
        // 找最宽的空谷
        let mut best: Option<(usize, usize)> = None; // (start, end) 空区
        let mut cur_start: Option<usize> = None;
        for (b, &d) in density.iter().enumerate() {
            if (d as f64) < threshold {
                if cur_start.is_none() {
                    cur_start = Some(b);
                }
            } else if let Some(s) = cur_start.take() {
                if best.map(|(bs, be)| b - s > be - bs).unwrap_or(true) {
                    best = Some((s, b));
                }
            }
        }
        // 注意：页面边缘（开头/结尾）的空白区不是"谷"——它的一侧没有内容，
        // 永远切不出两半；只有被内容包夹的空区才算列间谷。
        if let Some((s, e)) = best {
            if (e - s) * 2 >= 8 {
                // 谷分左右两半
                let (mut left, mut right) = (Vec::new(), Vec::new());
                let gap_x = s as f64 * 2.0;
                for &i in idxs {
                    let (_, _, x1, _) = bbox(i);
                    if x1 <= gap_x {
                        left.push(i);
                    } else {
                        right.push(i);
                    }
                }
                if !left.is_empty() && !right.is_empty() {
                    let mut children = vec![cut(left, lines, false, page_w, depth + 1)];
                    children.push(cut(right, lines, false, page_w, depth + 1));
                    return Region::Split {
                        vertical: true,
                        children,
                    };
                }
            }
        }
        return Region::Leaf(idxs.clone());
    }

    // 横切：y 空隙 ≥ 1.4×中位行高即分块（块内保序，按 y 降序）
    let mut sorted = idxs.clone();
    sorted.sort_by(|a, b| {
        let (ya, yb) = (lines[*a].y, lines[*b].y);
        yb.partial_cmp(&ya).unwrap_or(std::cmp::Ordering::Equal)
    });
    let med_h = median_line_height(lines);
    let mut blocks: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = vec![sorted[0]];
    for &i in &sorted[1..] {
        let prev_y = lines[*cur.last().unwrap()].y;
        let gap = prev_y - lines[i].y;
        if gap > med_h * 1.4 {
            blocks.push(std::mem::take(&mut cur));
        }
        cur.push(i);
    }
    blocks.push(cur);
    if blocks.len() == 1 {
        return Region::Leaf(idxs);
    }
    Region::Split {
        vertical: false,
        children: blocks
            .into_iter()
            .map(|b| cut(b, lines, true, page_w, depth + 1))
            .collect(),
    }
}

// ── 锚点对齐渲染 ──────────────────────────────────────────────────────────

/// 单行 → 网格文本：x 量化到字符列，列间隙用空格填充
fn render_line_anchored(line: &Line, median_char: f64) -> String {
    let mut items = line.items.clone();
    items.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));

    // 列间距判定：间隙 > 页宽 10% 或 > 4×字符宽 → 强制列分隔（至少 2 空格）
    let mut out = String::new();
    let mut last_end: Option<f64> = None;
    for it in &items {
        let target_col = ((it.x / median_char).round() as i64).max(0);
        match last_end {
            None => {
                for _ in 0..target_col {
                    out.push(' ');
                }
            }
            Some(le) => {
                let gap_pt = it.x - le;
                let gap_cols =
                    (it.x / median_char).round() as i64 - (le / median_char).round() as i64;
                if gap_pt > median_char * 4.0 {
                    // 列间隙：至少 2 空格 + 对齐到目标列
                    let spaces = (target_col - (le / median_char).round() as i64).max(2);
                    for _ in 0..spaces {
                        out.push(' ');
                    }
                } else if gap_pt > 0.2 * it.size.max(1.0) {
                    for _ in 0..gap_cols.max(1) {
                        out.push(' ');
                    }
                }
            }
        }
        out.push_str(&it.text);
        last_end = Some(it.end_x);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::TextItem;

    fn line(y: f64, items: Vec<(f64, &str)>) -> Line {
        Line {
            y,
            items: items
                .into_iter()
                .map(|(x, t)| {
                    let w = t.chars().count() as f64 * 6.0;
                    TextItem {
                        x,
                        end_x: x + w,
                        size: 12.0,
                        text: t.to_string(),
                    }
                })
                .collect(),
        }
    }

    #[test]
    fn two_columns_separated() {
        // 左栏 12 行 + 右栏 12 行（x 320 起）→ 竖切后各自成段
        let mut lines = Vec::new();
        for i in 0..12 {
            let y = 720.0 - i as f64 * 14.0;
            lines.push(line(y, vec![(72.0, "left col text here")]));
            lines.push(line(y, vec![(340.0, "right col text")]));
        }
        let pt = PageText {
            page: 1,
            lines,
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
        let out = project_page(&pt);
        // 左栏内容全部出现在右栏之前（分列读取）
        let left_pos = out.find("left col text here").unwrap();
        let right_pos = out.find("right col text").unwrap();
        let last_left = out.rfind("left col text here").unwrap();
        assert!(last_left < right_pos, "左栏应整体在右栏之前:\n{out}");
        let _ = (left_pos, right_pos);
    }

    #[test]
    fn vertical_gap_becomes_blank_lines() {
        let pt = PageText {
            page: 1,
            lines: vec![
                line(720.0, vec![(72.0, "title line")]),
                line(600.0, vec![(72.0, "body after gap")]),
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
        let out = project_page(&pt);
        assert!(out.contains("\n\n"), "大间隙应产生空行: {out:?}");
    }
}
