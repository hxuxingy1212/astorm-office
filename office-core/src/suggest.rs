//! 名称纠错与取值建议（agent 自愈辅助）。
//!
//! 供三个 CLI 在"未知属性 / 未知类型 / 未知取值"时给出 `suggestion`，
//! 让 agent 无需人工介入即可自我修正（对齐 OfficeCLI 的 suggestion 设计）。

/// Levenshtein 编辑距离（大小写不敏感调用，滚动数组实现）。
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// 在候选集中找与 `input` 最接近的项（大小写不敏感）。
/// 距离阈值 = max(2, len/3)，避免给无关建议。
pub fn nearest<'a, I: IntoIterator<Item = &'a str>>(input: &str, candidates: I) -> Option<String> {
    let input_l = input.to_lowercase();
    let threshold = (input.chars().count() / 3).max(2);
    let mut best: Option<(usize, String)> = None;
    for c in candidates {
        let d = levenshtein(&input_l, &c.to_lowercase());
        if best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
            best = Some((d, c.to_string()));
        }
    }
    match best {
        Some((d, c)) if d <= threshold => Some(c),
        _ => None,
    }
}

/// 生成"未知 X"类建议语句：`是否想用 "bold"？可选值: bold/italic/...`
pub fn did_you_mean(input: &str, candidates: &[&str], limit: usize) -> String {
    let mut s = String::new();
    if let Some(n) = nearest(input, candidates.iter().copied()) {
        s.push_str(&format!("是否想用 \"{n}\"？"));
    }
    if !candidates.is_empty() && s.is_empty() {
        s.push_str("可用值: ");
    } else if !candidates.is_empty() {
        s.push_str("可选值: ");
    }
    let shown = candidates
        .iter()
        .take(limit)
        .copied()
        .collect::<Vec<_>>()
        .join("/");
    s.push_str(&shown);
    if candidates.len() > limit {
        s.push_str("/…");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_finds_typo() {
        let cands = ["font_size", "bold", "italic", "color"];
        assert_eq!(nearest("fontsize", cands), Some("font_size".to_string()));
        assert_eq!(nearest("bol", cands), Some("bold".to_string()));
        assert_eq!(nearest("completely_different", cands), None);
    }

    #[test]
    fn did_you_mean_formats() {
        let s = did_you_mean("fn", &["font_size", "bold"], 8);
        assert!(s.contains("font_size"), "{s}");
    }
}
