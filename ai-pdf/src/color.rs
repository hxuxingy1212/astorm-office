//! 颜色数学 — 复刻 design_engine.py 的 colorsys 调用与 WCAG 对比度。
//! 注意 Python colorsys 使用 HLS 顺序；这里统一为 (h: 0-360, s: 0-1, l: 0-1)。

/// colorsys.hls_to_rgb 等价实现。
pub fn hls_to_rgb(h: f64, l: f64, s: f64) -> (f64, f64, f64) {
    if s == 0.0 {
        return (l, l, l);
    }
    let h = (h.rem_euclid(360.0)) / 360.0;
    // colorsys：l ≤ 0.5 与 l > 0.5 的 m2 公式不同
    let m2 = if l <= 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let m1 = 2.0 * l - m2;
    let r = hue_to_rgb(m1, m2, h + 1.0 / 3.0);
    let g = hue_to_rgb(m1, m2, h);
    let b = hue_to_rgb(m1, m2, h - 1.0 / 3.0);
    (r, g, b)
}

fn hue_to_rgb(m1: f64, m2: f64, mut hue: f64) -> f64 {
    if hue < 0.0 {
        hue += 1.0;
    }
    if hue > 1.0 {
        hue -= 1.0;
    }
    if hue < 1.0 / 6.0 {
        m1 + (m2 - m1) * hue * 6.0
    } else if hue < 0.5 {
        m2
    } else if hue < 2.0 / 3.0 {
        m1 + (m2 - m1) * (2.0 / 3.0 - hue) * 6.0
    } else {
        m1
    }
}

/// colorsys.rgb_to_hls 等价实现，返回 (h: 0-360, l, s)。
pub fn rgb_to_hls(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let maxc = r.max(g).max(b);
    let minc = r.min(g).min(b);
    let l = (minc + maxc) / 2.0;
    if (minc - maxc).abs() < f64::EPSILON {
        return (0.0, l, 0.0);
    }
    let span = maxc - minc;
    let s = if l <= 0.5 {
        span / (maxc + minc)
    } else {
        span / (2.0 - maxc - minc)
    };
    let rc = (maxc - r) / span;
    let gc = (maxc - g) / span;
    let bc = (maxc - b) / span;
    let h = if (r - maxc).abs() < f64::EPSILON {
        bc - gc
    } else if (g - maxc).abs() < f64::EPSILON {
        2.0 + rc - bc
    } else {
        4.0 + gc - rc
    };
    let h = (h / 6.0).rem_euclid(1.0);
    (h * 360.0, l, s)
}

/// HSL → "#rrggbb"（Python 侧 int(x*255) 为向零取整）。
pub fn hsl_hex(h: f64, s: f64, l: f64) -> String {
    let (r, g, b) = hls_to_rgb(h, l, s);
    format!(
        "#{:02x}{:02x}{:02x}",
        (r * 255.0) as u64,
        (g * 255.0) as u64,
        (b * 255.0) as u64
    )
}

/// "#rrggbb" → "r,g,b"（供 rgba() 使用）。
pub fn hex_rgb_str(hex: &str) -> String {
    let h = hex.trim_start_matches('#');
    format!(
        "{},{},{}",
        u64::from_str_radix(&h[0..2], 16).unwrap_or(0),
        u64::from_str_radix(&h[2..4], 16).unwrap_or(0),
        u64::from_str_radix(&h[4..6], 16).unwrap_or(0)
    )
}

pub fn hex_to_rgb01(hex: &str) -> (f64, f64, f64) {
    let h = hex.trim_start_matches('#');
    let c = |r: std::ops::Range<usize>| u64::from_str_radix(&h[r], 16).unwrap_or(0) as f64 / 255.0;
    (c(0..2), c(2..4), c(4..6))
}

/// WCAG 2.1 相对亮度。
pub fn relative_luminance(hex: &str) -> f64 {
    let (r, g, b) = hex_to_rgb01(hex);
    let f = |c: f64| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b)
}

/// WCAG 对比度。
pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let lighter = l1.max(l2);
    let darker = l1.min(l2);
    (lighter + 0.05) / (darker + 0.05)
}

/// 把 accent 色相从浑浊区（黄绿/棕橙）推开，复刻 _sanitize_accent_hue。
pub fn sanitize_accent_hue(hue: f64) -> f64 {
    let hue = hue.rem_euclid(360.0);
    // (start=28, end=105, nudge_low=15, nudge_high=120)
    if (28.0..=105.0).contains(&hue) {
        let mid = (28.0 + 105.0) / 2.0;
        return if hue < mid { 15.0 } else { 120.0 };
    }
    hue
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hsl_hex_known_values() {
        assert_eq!(hsl_hex(0.0, 1.0, 0.5), "#ff0000");
        assert_eq!(hsl_hex(210.0, 0.0, 1.0), "#ffffff");
        assert_eq!(hsl_hex(210.0, 0.0, 0.0), "#000000");
    }

    #[test]
    fn rgb_hls_roundtrip() {
        let (h, l, s) = rgb_to_hls(0.4, 0.6, 0.2);
        let (r, g, b) = hls_to_rgb(h, l, s);
        assert!((r - 0.4).abs() < 1e-9 && (g - 0.6).abs() < 1e-9 && (b - 0.2).abs() < 1e-9);
    }

    #[test]
    fn contrast_black_white() {
        let cr = contrast_ratio("#000000", "#ffffff");
        assert!((cr - 21.0).abs() < 0.01);
    }

    #[test]
    fn ugly_hue_nudged() {
        assert_eq!(sanitize_accent_hue(60.0), 15.0); // 偏低半区
        assert_eq!(sanitize_accent_hue(100.0), 120.0); // 偏高半区
        assert_eq!(sanitize_accent_hue(200.0), 200.0); // 安全区不动
    }
}
