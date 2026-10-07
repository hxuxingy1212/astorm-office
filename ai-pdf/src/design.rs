//! 设计引擎 — 移植 design_engine.py 的 SVG 背景生成、空间布局计算、
//! 标题关键词 → intent 推导。SVG 生成器按原版参数面重实现。

use crate::util::{round_f, Rng};
use serde_json::{json, Value};

/// THEME_KEYWORDS 完整表（中英关键词 → intent）。
pub const THEME_KEYWORDS: &[(&str, &str)] = &[
    // Technology / Data / Analytics
    ("tech", "cold"),
    ("数据", "cold"),
    ("data", "cold"),
    ("AI", "cold"),
    ("科技", "cold"),
    ("digital", "cold"),
    ("analytics", "cold"),
    ("分析", "cold"),
    // Nature / Environment / Sustainability
    ("green", "nature"),
    ("绿色", "nature"),
    ("环保", "nature"),
    ("eco", "nature"),
    ("sustainability", "nature"),
    ("生态", "nature"),
    ("forest", "nature"),
    // Business / Finance / Corporate
    ("report", "neutral"),
    ("报告", "neutral"),
    ("finance", "neutral"),
    ("财务", "neutral"),
    ("annual", "neutral"),
    ("年度", "neutral"),
    ("corporate", "neutral"),
    // Creative / Marketing / Social
    ("marketing", "energy"),
    ("运营", "energy"),
    ("social", "energy"),
    ("品牌", "energy"),
    ("campaign", "energy"),
    ("活动", "energy"),
    ("launch", "energy"),
    // Authority / Formal / Premium / Luxury
    ("luxury", "authority"),
    ("奢华", "authority"),
    ("fashion", "authority"),
    ("时尚", "authority"),
    ("premium", "authority"),
    ("高端", "authority"),
    ("gala", "authority"),
    ("formal", "authority"),
    ("正式", "authority"),
    ("professional", "authority"),
    ("专业", "authority"),
    ("government", "authority"),
    ("政府", "authority"),
    ("bidding", "authority"),
    ("投标", "authority"),
    ("政府报告", "authority"),
    ("政府文书", "authority"),
    ("公文", "authority"),
    ("thesis", "authority"),
    ("毕业论文", "authority"),
    ("dissertation", "authority"),
    ("开题", "authority"),
    ("开题报告", "authority"),
    ("proposal", "authority"),
    ("学位", "authority"),
    // Calm / Meditation / Healthcare / Minimalist
    ("health", "calm"),
    ("健康", "calm"),
    ("meditation", "calm"),
    ("wellness", "calm"),
    ("calm", "calm"),
    ("医疗", "calm"),
    ("minimalist", "calm"),
    ("极简", "calm"),
    ("simple", "calm"),
    ("简约", "calm"),
    // Urgent / Warning / Emergency
    ("urgent", "tension"),
    ("warning", "tension"),
    ("紧急", "tension"),
    ("alert", "tension"),
    ("crisis", "tension"),
    // Warm / Food / Lifestyle
    ("food", "warmth"),
    ("美食", "warmth"),
    ("lifestyle", "warmth"),
    ("生活", "warmth"),
    ("home", "warmth"),
    ("家居", "warmth"),
];

/// 标题/描述 → intent；平分时偏好具体 intent 而非 neutral（与原版 derive_intent 一致）。
pub fn derive_intent(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut scores: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for (kw, intent) in THEME_KEYWORDS {
        if lower.contains(&kw.to_lowercase()) {
            *scores.entry(intent).or_insert(0) += 1;
        }
    }
    if scores.is_empty() {
        return "neutral".into();
    }
    let max_score = *scores.values().max().unwrap();
    let mut top: Vec<&&str> = scores
        .iter()
        .filter(|(_, v)| **v == max_score)
        .map(|(k, _)| k)
        .collect();
    if top.len() > 1 {
        top.retain(|k| **k != "neutral");
    }
    (*top[0]).to_string()
}

fn svg_open(w: i64, h: i64) -> String {
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" fill=\"none\">\n"
    )
}

/// flow — 大半径贝塞尔流线（3-5 条，低透明度）。
pub fn generate_flow_svg(
    w: i64,
    h: i64,
    color: &str,
    curves: i64,
    stroke_width: i64,
    opacity: f64,
    seed: Option<u64>,
) -> String {
    let mut rng = Rng::new(seed.or(Some(7)));
    let n = curves.clamp(1, 10);
    let mut svg = svg_open(w, h);
    svg.push_str(&format!(
        "  <defs><filter id=\"blur\"><feGaussianBlur stdDeviation=\"{}\"/></filter></defs>\n",
        stroke_width / 4
    ));
    for i in 0..n {
        // 每条曲线从画布外一侧扫到另一侧，控制点大半径偏移
        let y_base = (h as f64) * (i as f64 + 0.5) / (n as f64);
        let x0 = -w as f64 * 0.2;
        let x4 = w as f64 * 1.2;
        let cx1 = w as f64 * rng.uniform(0.1, 0.4);
        let cy1 = y_base + rng.uniform(-1.0, 1.0) * h as f64 * 0.4;
        let cx2 = w as f64 * rng.uniform(0.6, 0.9);
        let cy2 = y_base + rng.uniform(-1.0, 1.0) * h as f64 * 0.4;
        svg.push_str(&format!(
            "  <path d=\"M {x0:.1} {y_base:.1} C {cx1:.1} {cy1:.1}, {cx2:.1} {cy2:.1}, {x4:.1} {y_base:.1}\" stroke=\"{color}\" stroke-width=\"{stroke_width}\" opacity=\"{opacity:.3}\" filter=\"url(#blur)\"/>\n"
        ));
    }
    svg.push_str("</svg>\n");
    svg
}

/// grid — 极淡参考网格。
pub fn generate_grid_svg(
    w: i64,
    h: i64,
    color: &str,
    spacing: i64,
    line_width: f64,
    opacity: f64,
) -> String {
    let mut svg = svg_open(w, h);
    let mut x = spacing;
    while x < w {
        svg.push_str(&format!(
            "  <line x1=\"{x}\" y1=\"0\" x2=\"{x}\" y2=\"{h}\" stroke=\"{color}\" stroke-width=\"{line_width}\" opacity=\"{opacity:.3}\"/>\n"
        ));
        x += spacing;
    }
    let mut y = spacing;
    while y < h {
        svg.push_str(&format!(
            "  <line x1=\"0\" y1=\"{y}\" x2=\"{w}\" y2=\"{y}\" stroke=\"{color}\" stroke-width=\"{line_width}\" opacity=\"{opacity:.3}\"/>\n"
        ));
        y += spacing;
    }
    svg.push_str("</svg>\n");
    svg
}

/// noise — feTurbulence 纹理。
pub fn generate_noise_svg(w: i64, h: i64, frequency: f64, octaves: i64, opacity: f64) -> String {
    let mut svg = svg_open(w, h);
    svg.push_str(&format!(
        "  <filter id=\"noise\" x=\"0\" y=\"0\" width=\"100%\" height=\"100%\">\n    <feTurbulence type=\"fractalNoise\" baseFrequency=\"{frequency}\" numOctaves=\"{octaves}\" stitchTiles=\"stitch\"/>\n  </filter>\n"
    ));
    svg.push_str(&format!(
        "  <rect width=\"{w}\" height=\"{h}\" filter=\"url(#noise)\" opacity=\"{opacity:.3}\"/>\n"
    ));
    svg.push_str("</svg>\n");
    svg
}

/// supergraphic — 出画布的巨型圆/矩形/多边形。
pub fn generate_supergraphic_svg(w: i64, h: i64, color: &str, seed: u64) -> String {
    let mut rng = Rng::new(Some(seed));
    let mut svg = svg_open(w, h);
    let n = rng.randint(2, 4);
    for _ in 0..n {
        let cx = rng.uniform(-0.3, 1.3) * w as f64;
        let cy = rng.uniform(-0.3, 1.3) * h as f64;
        let r = rng.uniform(0.5, 1.2) * w.max(h) as f64;
        let opacity = rng.uniform(0.06, 0.16);
        match rng.randint(0, 2) {
            0 => svg.push_str(&format!(
                "  <circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"{r:.1}\" fill=\"{color}\" opacity=\"{opacity:.3}\"/>\n"
            )),
            1 => {
                let rw = rng.uniform(0.4, 1.4) * w as f64;
                let rh = rng.uniform(0.4, 1.4) * h as f64;
                svg.push_str(&format!(
                    "  <rect x=\"{cx:.1}\" y=\"{cy:.1}\" width=\"{rw:.1}\" height=\"{rh:.1}\" rx=\"{r:.1}\" fill=\"{color}\" opacity=\"{opacity:.3}\"/>\n"
                ));
            }
            _ => {
                let pts: Vec<String> = (0..rng.randint(3, 6))
                    .map(|_| {
                        format!("{:.1},{:.1}", rng.uniform(-0.2, 1.2) * w as f64, rng.uniform(-0.2, 1.2) * h as f64)
                    })
                    .collect();
                svg.push_str(&format!(
                    "  <polygon points=\"{}\" fill=\"{color}\" opacity=\"{opacity:.3}\"/>\n",
                    pts.join(" ")
                ));
            }
        }
    }
    svg.push_str("</svg>\n");
    svg
}

/// ordered_texture — 点阵 + 坐标网格 + 刻度 + 等高线。
pub fn generate_ordered_texture_svg(w: i64, h: i64, color: &str, seed: u64) -> String {
    let mut rng = Rng::new(Some(seed));
    let mut svg = svg_open(w, h);
    // 点阵
    let step = 48;
    let mut y = step;
    while y < h {
        let mut x = step;
        while x < w {
            let jitter = rng.uniform(-2.0, 2.0);
            svg.push_str(&format!(
                "  <circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"1.2\" fill=\"{color}\" opacity=\"0.10\"/>\n",
                x as f64 + jitter,
                y as f64 + jitter
            ));
            x += step;
        }
        y += step;
    }
    // 坐标轴刻度
    svg.push_str(&format!(
        "  <line x1=\"40\" y1=\"{h}\" x2=\"{w}\" y2=\"{h}\" stroke=\"{color}\" stroke-width=\"1\" opacity=\"0.18\"/>\n"
    ));
    let mut x = 40;
    while x < w {
        svg.push_str(&format!(
            "  <line x1=\"{x}\" y1=\"{h}\" x2=\"{x}\" y2=\"{}\" stroke=\"{color}\" stroke-width=\"1\" opacity=\"0.18\"/>\n",
            h - 6
        ));
        x += 96;
    }
    // 等高线
    for _ in 0..3 {
        let y0 = rng.uniform(0.15, 0.85) * h as f64;
        let amp = rng.uniform(8.0, 28.0);
        svg.push_str(&format!(
            "  <path d=\"M 0 {y0:.1} C {w4} {y1:.1}, {w34} {y2:.1}, {w} {y3:.1}\" stroke=\"{color}\" stroke-width=\"0.8\" opacity=\"0.12\" fill=\"none\"/>\n",
            w4 = w as f64 / 4.0,
            w34 = w as f64 * 0.75,
            y1 = y0 + rng.uniform(-1.0, 1.0) * amp,
            y2 = y0 + rng.uniform(-1.0, 1.0) * amp,
            y3 = y0 + rng.uniform(-1.0, 1.0) * amp * 0.5,
        ));
    }
    svg.push_str("</svg>\n");
    svg
}

pub fn generate_generative_svg(svg_type: &str, w: i64, h: i64, color: &str) -> String {
    match svg_type {
        "flow" => generate_flow_svg(w, h, color, 4, 80, 0.05, Some(7)),
        "grid" => generate_grid_svg(w, h, color, 60, 0.5, 0.04),
        "noise" => generate_noise_svg(w, h, 0.8, 4, 0.035),
        "supergraphic" => generate_supergraphic_svg(w, h, color, 42),
        "ordered_texture" => generate_ordered_texture_svg(w, h, color, 42),
        _ => generate_flow_svg(w, h, color, 4, 80, 0.05, Some(7)),
    }
}

// ── 布局计算 ──────────────────────────────────────────────────────────────

pub const BREATHING_MARGIN: f64 = 0.12;

/// 计算定位布局（offset / centered / overlap 三种风格）。
/// 安全区 = 12% 呼吸边距；黄金分割加权垂直分带（首元素 1.618）。
pub fn calculate_layout(
    elements: &[String],
    w: f64,
    h: f64,
    style: &str,
    seed: Option<u64>,
) -> Value {
    let mut rng = Rng::new(seed.or(Some(3)));
    let safe_x = w * BREATHING_MARGIN;
    let safe_y = h * BREATHING_MARGIN;
    let safe_w = w * (1.0 - 2.0 * BREATHING_MARGIN);
    let safe_h = h * (1.0 - 2.0 * BREATHING_MARGIN);

    // 黄金分割加权分带
    let weights: Vec<f64> = (0..elements.len())
        .map(|i| if i == 0 { 1.618 } else { 1.0 })
        .collect();
    let total: f64 = weights.iter().sum();
    let mut regions: Vec<(f64, f64, f64, f64)> = Vec::new();
    let mut cur_y = safe_y;
    for weight in &weights {
        let rh = safe_h * weight / total;
        regions.push((safe_x, cur_y, safe_w, rh));
        cur_y += rh;
    }

    let mut layout = serde_json::Map::new();
    for (i, name) in elements.iter().enumerate() {
        let (rx, ry, rw, rh) = regions[i];
        let entry = match style {
            "centered" => json!({
                "x": round_f(rx + rw * 0.075, 1),
                "y": round_f(ry, 1),
                "w": round_f(rw * 0.85, 1),
                "h": round_f(rh * 0.9, 1),
                "rotation": 0,
            }),
            "overlap" => {
                let overlap_y = if i > 0 { rh * 0.15 } else { 0.0 };
                json!({
                    "x": round_f(rx, 1),
                    "y": round_f(ry - overlap_y, 1),
                    "w": round_f(rw, 1),
                    "h": round_f(rh + overlap_y * 0.5, 1),
                    "rotation": 0,
                    "z_index": elements.len() - i,
                })
            }
            _ => {
                // offset — 刻意不对称
                let offset_x = rw * 0.08 * if i % 2 == 0 { -1.0 } else { 1.0 };
                json!({
                    "x": round_f(rx + offset_x, 1),
                    "y": round_f(ry, 1),
                    "w": round_f(rw * 0.85, 1),
                    "h": round_f(rh * 0.85, 1),
                    "rotation": if name != "body" { round_f(rng.uniform(-1.5, 1.5), 2) } else { 0.0 },
                })
            }
        };
        layout.insert(name.clone(), entry);
    }
    Value::Object(layout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_keywords() {
        assert_eq!(
            derive_intent("Social Media Operations Monthly Report"),
            "energy"
        );
        // 注：原版 docstring 声称 "2025 Annual Sustainability Report" → nature，
        // 但子串匹配下 "sustainability" 同时命中 "ai"(cold)，且 annual/report 各计
        // neutral 一票（2 票获胜）。算法实际返回 neutral —— 这里按算法真实行为断言。
        assert_eq!(
            derive_intent("2025 Annual Sustainability Report"),
            "neutral"
        );
        assert_eq!(derive_intent("2025 Sustainability Green Report"), "nature");
        assert_eq!(derive_intent("随便一个标题"), "neutral");
        assert_eq!(derive_intent("AI 数据平台"), "cold");
        assert_eq!(derive_intent("毕业论文"), "authority");
    }

    #[test]
    fn theme_keywords_all_valid_intents() {
        let valid = [
            "cold",
            "nature",
            "neutral",
            "energy",
            "authority",
            "calm",
            "tension",
            "warmth",
        ];
        for (_, intent) in THEME_KEYWORDS {
            assert!(valid.contains(intent), "unknown intent {intent}");
        }
    }

    #[test]
    fn layout_regions_fill_safe_area() {
        let elements: Vec<String> = ["hero", "body", "meta"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let layout = calculate_layout(&elements, 720.0, 960.0, "offset", Some(1));
        // 黄金加权：首元素 1.618/(1.618+2) ≈ 44.7%
        let hero_h = layout["hero"]["h"].as_f64().unwrap();
        let body_h = layout["body"]["h"].as_f64().unwrap();
        assert!(hero_h / (hero_h + body_h + layout["meta"]["h"].as_f64().unwrap()) > 0.40);
        // 全部元素在画布内
        for name in &elements {
            let e = &layout[name];
            assert!(e["x"].as_f64().unwrap() >= 0.0 && e["y"].as_f64().unwrap() >= 0.0);
        }
    }

    #[test]
    fn svg_outputs_are_svg() {
        for t in ["flow", "grid", "noise", "supergraphic", "ordered_texture"] {
            let svg = generate_generative_svg(t, 720, 960, "#8a8a8a");
            assert!(
                svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"),
                "type {t}"
            );
        }
    }

    #[test]
    fn layout_deterministic() {
        let elements: Vec<String> = ["hero", "body"].iter().map(|s| s.to_string()).collect();
        let a = calculate_layout(&elements, 720.0, 960.0, "offset", Some(9));
        let b = calculate_layout(&elements, 720.0, 960.0, "offset", Some(9));
        assert_eq!(a["hero"]["x"], b["hero"]["x"]);
    }
}
