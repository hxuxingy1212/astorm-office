//! 生成式调色板 — 忠实移植 design_engine.py 的
//! generate_color_palette / generate_cascade_palette / audit_palette。
//!
//! 铁律：LLM 只选语义参数（intent/mode/harmony），颜色由 HSL 几何推算，
//! 并强制 WCAG 对比度（text:bg ≥ 4.5:1，accent:bg ≥ 3:1）。

use crate::color::{contrast_ratio, hsl_hex, rgb_to_hls, sanitize_accent_hue};
use crate::util::{round_f, Rng};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const INTENT_HUES: &[(&str, i64)] = &[
    ("calm", 210),
    ("tension", 0),
    ("energy", 30),
    ("authority", 280),
    ("warmth", 20),
    ("nature", 150),
    ("cold", 200),
    ("neutral", 45),
    // legacy aliases
    ("serenity", 210),
    ("elegance", 280),
    ("minimalism", 0),
];

pub const INTENT_HARMONY_MAP: &[(&str, &str)] = &[
    ("calm", "analogous"),
    ("tension", "complementary"),
    ("energy", "triadic"),
    ("authority", "split_complementary"),
    ("warmth", "analogous"),
    ("nature", "analogous"),
    ("cold", "split_complementary"),
    ("neutral", "split_complementary"),
    ("serenity", "analogous"),
    ("elegance", "split_complementary"),
    ("minimalism", "monochrome"),
];

pub const HARMONIES: &[&str] = &[
    "complementary",
    "split_complementary",
    "triadic",
    "analogous",
    "monochrome",
];

pub const MODES: &[&str] = &["minimal", "dark", "pastel", "jewel", "light"];

/// 面积 ∝ 1/饱和度 的层级上限（生成与审计都强制）。
pub const CASCADE_TIER_CAPS: &[(&str, f64)] = &[
    ("xl", 0.08),
    ("l", 0.15),
    ("m", 0.30),
    ("s", 0.50),
    ("xs", 0.75),
];

pub fn intent_hue(intent: &str) -> Option<i64> {
    INTENT_HUES
        .iter()
        .find(|(k, _)| *k == intent)
        .map(|(_, v)| *v)
}

pub fn recommend_harmony(intent: &str) -> &str {
    INTENT_HARMONY_MAP
        .iter()
        .find(|(k, _)| *k == intent)
        .map(|(_, v)| *v)
        .unwrap_or("split_complementary")
}

/// 几何 accent 色相推导。
fn derive_accent_hue(base_hue: f64, harmony: &str, rng: &mut Rng) -> f64 {
    let hue = match harmony {
        "complementary" => base_hue + 180.0,
        "split_complementary" => base_hue + *rng.choose(&[150.0, 210.0]),
        "triadic" => base_hue + *rng.choose(&[120.0, 240.0]),
        "analogous" => base_hue + *rng.choose(&[30.0, -30.0, 45.0, -45.0]),
        _ => base_hue, // monochrome
    };
    sanitize_accent_hue(hue)
}

/// 单一调色板（bg 60% / mid 30% / accent 10% + text/muted/surface）。
pub fn generate_color_palette(
    intent: &str,
    mode: &str,
    harmony: Option<&str>,
    seed: Option<u64>,
) -> Value {
    let mut rng = Rng::new(seed);
    let harmony = match harmony {
        Some(h) if h != "auto" => h.to_string(),
        _ => recommend_harmony(intent).to_string(),
    };
    let base_hue = intent_hue(intent).unwrap_or(rng.randint(0, 359)) as f64;
    let accent_hue = derive_accent_hue(base_hue, &harmony, &mut rng);

    let (bg, mid, mut accent, mut text, muted, surface) = match mode {
        "minimal" => {
            // 纸感底 + 高纯度 accent；bg 有可见色偏，不是死白
            let is_warm = rng.bool();
            let paper_hue = if is_warm {
                rng.randint(35, 45)
            } else {
                rng.randint(200, 215)
            } as f64;
            (
                hsl_hex(paper_hue, rng.uniform(0.08, 0.15), rng.uniform(0.92, 0.96)),
                hsl_hex(paper_hue, rng.uniform(0.10, 0.18), rng.uniform(0.85, 0.90)),
                hsl_hex(accent_hue, rng.uniform(0.55, 0.72), rng.uniform(0.42, 0.52)),
                hsl_hex(paper_hue, 0.05, rng.uniform(0.10, 0.15)),
                hsl_hex(paper_hue, 0.05, rng.uniform(0.45, 0.55)),
                format!("rgba(0,0,0,{:.2})", rng.uniform(0.01, 0.03)),
            )
        }
        "dark" => (
            hsl_hex(base_hue, rng.uniform(0.04, 0.10), rng.uniform(0.04, 0.08)),
            hsl_hex(base_hue, rng.uniform(0.08, 0.15), rng.uniform(0.12, 0.20)),
            hsl_hex(accent_hue, rng.uniform(0.45, 0.60), rng.uniform(0.52, 0.62)),
            hsl_hex(base_hue, 0.05, rng.uniform(0.88, 0.95)),
            hsl_hex(base_hue, 0.05, rng.uniform(0.45, 0.55)),
            format!("rgba(255,255,255,{:.2})", rng.uniform(0.03, 0.06)),
        ),
        "pastel" => (
            hsl_hex(base_hue, rng.uniform(0.15, 0.35), rng.uniform(0.85, 0.92)),
            hsl_hex(base_hue, rng.uniform(0.20, 0.40), rng.uniform(0.75, 0.82)),
            hsl_hex(accent_hue, rng.uniform(0.38, 0.50), rng.uniform(0.38, 0.48)),
            hsl_hex(base_hue, 0.15, rng.uniform(0.15, 0.25)),
            hsl_hex(base_hue, 0.20, rng.uniform(0.45, 0.55)),
            format!("rgba(255,255,255,{:.2})", rng.uniform(0.30, 0.50)),
        ),
        "jewel" => (
            hsl_hex(base_hue, rng.uniform(0.40, 0.60), rng.uniform(0.15, 0.25)),
            hsl_hex(base_hue, rng.uniform(0.40, 0.60), rng.uniform(0.25, 0.35)),
            hsl_hex(accent_hue, rng.uniform(0.30, 0.50), rng.uniform(0.65, 0.80)),
            hsl_hex(base_hue, 0.10, rng.uniform(0.90, 0.96)),
            hsl_hex(base_hue, 0.20, rng.uniform(0.60, 0.70)),
            format!("rgba(0,0,0,{:.2})", rng.uniform(0.20, 0.40)),
        ),
        _ => {
            // light
            (
                hsl_hex(base_hue, rng.uniform(0.10, 0.20), rng.uniform(0.91, 0.95)),
                hsl_hex(base_hue, rng.uniform(0.15, 0.25), rng.uniform(0.84, 0.90)),
                hsl_hex(accent_hue, rng.uniform(0.35, 0.45), rng.uniform(0.35, 0.45)),
                hsl_hex(base_hue, 0.08, rng.uniform(0.08, 0.15)),
                hsl_hex(base_hue, 0.05, rng.uniform(0.45, 0.55)),
                format!("rgba(0,0,0,{:.2})", rng.uniform(0.02, 0.05)),
            )
        }
    };

    // WCAG 兜底：text:bg < 4.5 → 推近黑/白
    if contrast_ratio(&text, &bg) < 4.5 {
        let (_, bg_l, _) = {
            let h = bg.trim_start_matches('#');
            let c =
                |r: usize, e: usize| u64::from_str_radix(&h[r..e], 16).unwrap_or(0) as f64 / 255.0;
            rgb_to_hls(c(0, 2), c(2, 4), c(4, 6))
        };
        text = if bg_l > 0.5 {
            hsl_hex(base_hue, 0.08, 0.08)
        } else {
            hsl_hex(base_hue, 0.05, 0.95)
        };
    }
    // accent:bg < 3.0 → 拉远离底色亮度
    if contrast_ratio(&accent, &bg) < 3.0 {
        let (_, bg_l, _) = {
            let h = bg.trim_start_matches('#');
            let c =
                |r: usize, e: usize| u64::from_str_radix(&h[r..e], 16).unwrap_or(0) as f64 / 255.0;
            rgb_to_hls(c(0, 2), c(2, 4), c(4, 6))
        };
        let target_l = if bg_l > 0.5 { 0.35 } else { 0.70 };
        let (_, _, accent_s) = {
            let h = accent.trim_start_matches('#');
            let c =
                |r: usize, e: usize| u64::from_str_radix(&h[r..e], 16).unwrap_or(0) as f64 / 255.0;
            rgb_to_hls(c(0, 2), c(2, 4), c(4, 6))
        };
        accent = hsl_hex(accent_hue, accent_s, target_l);
    }

    json!({
        "bg": bg.clone(), "bg_rgb": crate::color::hex_rgb_str(&bg),
        "mid": mid.clone(), "mid_rgb": crate::color::hex_rgb_str(&mid),
        "accent": accent.clone(), "accent_rgb": crate::color::hex_rgb_str(&accent),
        "text": text, "muted": muted, "surface": surface,
        "meta": {
            "intent": intent, "mode": mode, "harmony": harmony,
            "base_hue": base_hue as i64, "accent_hue": accent_hue as i64,
            "contrast": {
                "text_on_bg": round_f(contrast_ratio(&text, &bg), 2),
                "accent_on_bg": round_f(contrast_ratio(&accent, &bg), 2),
            }
        }
    })
}

pub fn palette_to_css(p: &Value) -> String {
    format!(
        ":root {{\n  /* 60% ground */\n  --c-bg: {};\n  --c-bg-rgb: {};\n  /* 30% structure */\n  --c-mid: {};\n  --c-mid-rgb: {};\n  /* 10% emphasis */\n  --c-accent: {};\n  --c-accent-rgb: {};\n  /* Typography */\n  --c-text: {};\n  --c-muted: {};\n  /* Surfaces */\n  --c-surface: {};\n}}",
        p["bg"].as_str().unwrap_or(""),
        p["bg_rgb"].as_str().unwrap_or(""),
        p["mid"].as_str().unwrap_or(""),
        p["mid_rgb"].as_str().unwrap_or(""),
        p["accent"].as_str().unwrap_or(""),
        p["accent_rgb"].as_str().unwrap_or(""),
        p["text"].as_str().unwrap_or(""),
        p["muted"].as_str().unwrap_or(""),
        p["surface"].as_str().unwrap_or(""),
    )
}

/// 调色板可粘贴 ReportLab 常量（原版 palette.generate --format python 的输出形态）。
pub fn palette_to_reportlab(p: &Value) -> String {
    let meta = &p["meta"];
    format!(
        "# Auto-generated by json2pdf palette — intent={}, mode={}, harmony={}\nfrom reportlab.lib.colors import HexColor\n\nBG      = HexColor(\"{}\")   # 60% ground\nMID     = HexColor(\"{}\")   # 30% structure\nACCENT  = HexColor(\"{}\")   # 10% emphasis\nTEXT    = HexColor(\"{}\")\nMUTED   = HexColor(\"{}\")\n",
        meta["intent"].as_str().unwrap_or(""),
        meta["mode"].as_str().unwrap_or(""),
        meta["harmony"].as_str().unwrap_or(""),
        p["bg"].as_str().unwrap_or(""),
        p["mid"].as_str().unwrap_or(""),
        p["accent"].as_str().unwrap_or(""),
        p["text"].as_str().unwrap_or(""),
        p["muted"].as_str().unwrap_or(""),
    )
}

// ── 级联调色板 ────────────────────────────────────────────────────────────

fn make_role(h: f64, s: f64, l: f64, tier: &str, usage_hint: &str) -> Value {
    json!({
        "hex": hsl_hex(h, s, l),
        "hsl": [round_f(h, 1), round_f(s, 3), round_f(l, 3)],
        "tier": tier,
        "usage_hint": usage_hint,
    })
}

pub fn generate_cascade_palette(
    intent: &str,
    mode: &str,
    harmony: Option<&str>,
    seed: Option<u64>,
) -> Value {
    let mut rng = Rng::new(seed);
    let harmony = match harmony {
        Some(h) if h != "auto" => h.to_string(),
        _ => recommend_harmony(intent).to_string(),
    };
    let base_hue = intent_hue(intent).unwrap_or(rng.randint(0, 359)) as f64;
    let accent_hue = derive_accent_hue(base_hue, &harmony, &mut rng);
    let secondary_hue = sanitize_accent_hue(if accent_hue != base_hue {
        (base_hue + accent_hue) / 2.0
    } else {
        (base_hue + 30.0).rem_euclid(360.0)
    });

    let is_dark = mode == "dark";
    let (bg_l_range, text_l, muted_l) = if is_dark {
        (
            (0.04, 0.08),
            rng.uniform(0.88, 0.95),
            rng.uniform(0.50, 0.60),
        )
    } else if mode == "jewel" {
        (
            (0.15, 0.25),
            rng.uniform(0.90, 0.96),
            rng.uniform(0.60, 0.70),
        )
    } else {
        (
            (0.94, 0.97),
            rng.uniform(0.08, 0.15),
            rng.uniform(0.45, 0.55),
        )
    };
    let nd = !is_dark; // not dark → 浅色分支

    let mut roles: BTreeMap<String, Value> = BTreeMap::new();
    // XL
    roles.insert(
        "page_bg".into(),
        make_role(
            base_hue,
            rng.uniform(0.03, 0.08),
            rng.uniform(bg_l_range.0, bg_l_range.1),
            "xl",
            "Page background, full-bleed areas",
        ),
    );
    roles.insert(
        "section_bg".into(),
        make_role(
            base_hue,
            rng.uniform(0.04, 0.08),
            if nd {
                rng.uniform(bg_l_range.0 - 0.03, bg_l_range.1 - 0.02)
            } else {
                rng.uniform(0.08, 0.14)
            },
            "xl",
            "Section background, alternating bands",
        ),
    );
    // L
    roles.insert(
        "card_bg".into(),
        make_role(
            base_hue,
            rng.uniform(0.06, 0.14),
            if nd {
                rng.uniform(0.90, 0.94)
            } else {
                rng.uniform(0.10, 0.16)
            },
            "l",
            "Card/container background, table even rows",
        ),
    );
    roles.insert(
        "table_stripe".into(),
        make_role(
            base_hue,
            rng.uniform(0.05, 0.12),
            if nd {
                rng.uniform(0.92, 0.96)
            } else {
                rng.uniform(0.08, 0.12)
            },
            "l",
            "Table alternating row fill",
        ),
    );
    // M
    roles.insert(
        "header_fill".into(),
        make_role(
            base_hue,
            rng.uniform(0.15, 0.28),
            if nd {
                rng.uniform(0.25, 0.40)
            } else {
                rng.uniform(0.20, 0.30)
            },
            "m",
            "Table header, sidebar background, cover top bar",
        ),
    );
    roles.insert(
        "cover_block".into(),
        make_role(
            base_hue,
            rng.uniform(0.12, 0.25),
            if nd {
                rng.uniform(0.30, 0.45)
            } else {
                rng.uniform(0.15, 0.25)
            },
            "m",
            "Cover decorative block, sidebar pillar",
        ),
    );
    // S
    roles.insert(
        "border".into(),
        make_role(
            base_hue,
            rng.uniform(0.10, 0.25),
            if nd {
                rng.uniform(0.70, 0.82)
            } else {
                rng.uniform(0.25, 0.35)
            },
            "s",
            "Borders, divider lines, chart grid",
        ),
    );
    roles.insert(
        "icon".into(),
        make_role(
            base_hue,
            rng.uniform(0.25, 0.45),
            if nd {
                rng.uniform(0.35, 0.50)
            } else {
                rng.uniform(0.55, 0.70)
            },
            "s",
            "Icons, bullet points, small UI elements",
        ),
    );
    // XS
    roles.insert(
        "accent".into(),
        make_role(
            accent_hue,
            rng.uniform(0.50, 0.70),
            if nd {
                rng.uniform(0.40, 0.55)
            } else {
                rng.uniform(0.55, 0.70)
            },
            "xs",
            "Primary accent: badges, tags, data point highlights, CTA",
        ),
    );
    roles.insert(
        "accent_secondary".into(),
        make_role(
            secondary_hue,
            rng.uniform(0.40, 0.60),
            if nd {
                rng.uniform(0.45, 0.58)
            } else {
                rng.uniform(0.50, 0.65)
            },
            "xs",
            "Secondary accent: chart series 2, secondary badge",
        ),
    );
    // text
    roles.insert(
        "text_primary".into(),
        make_role(base_hue, 0.05, text_l, "text", "Primary body text"),
    );
    roles.insert(
        "text_muted".into(),
        make_role(
            base_hue,
            0.04,
            muted_l,
            "text",
            "Captions, footnotes, secondary text",
        ),
    );

    let mut semantic: BTreeMap<String, Value> = BTreeMap::new();
    semantic.insert(
        "success".into(),
        make_role(
            140.0,
            rng.uniform(0.25, 0.40),
            if nd {
                rng.uniform(0.35, 0.45)
            } else {
                rng.uniform(0.55, 0.65)
            },
            "xs",
            "Positive: growth, pass, complete",
        ),
    );
    semantic.insert(
        "warning".into(),
        make_role(
            40.0,
            rng.uniform(0.30, 0.45),
            if nd {
                rng.uniform(0.40, 0.50)
            } else {
                rng.uniform(0.55, 0.65)
            },
            "xs",
            "Caution: pending, alert",
        ),
    );
    semantic.insert(
        "error".into(),
        make_role(
            5.0,
            rng.uniform(0.30, 0.45),
            if nd {
                rng.uniform(0.40, 0.50)
            } else {
                rng.uniform(0.55, 0.65)
            },
            "xs",
            "Negative: decline, fail, error",
        ),
    );
    semantic.insert(
        "info".into(),
        make_role(
            210.0,
            rng.uniform(0.25, 0.40),
            if nd {
                rng.uniform(0.40, 0.50)
            } else {
                rng.uniform(0.55, 0.65)
            },
            "xs",
            "Informational: neutral status",
        ),
    );

    // WCAG 强制
    let page_bg_hex = roles["page_bg"]["hex"]
        .as_str()
        .unwrap_or("#ffffff")
        .to_string();
    let text_hex = roles["text_primary"]["hex"]
        .as_str()
        .unwrap_or("#000000")
        .to_string();
    if contrast_ratio(&text_hex, &page_bg_hex) < 4.5 {
        roles.insert(
            "text_primary".into(),
            if nd {
                make_role(base_hue, 0.08, 0.08, "text", "Primary body text")
            } else {
                make_role(base_hue, 0.05, 0.95, "text", "Primary body text")
            },
        );
    }
    let accent_hex = roles["accent"]["hex"]
        .as_str()
        .unwrap_or("#888888")
        .to_string();
    if contrast_ratio(&accent_hex, &page_bg_hex) < 3.0 {
        let (r, g, b) = crate::color::hex_to_rgb01(&page_bg_hex);
        let (_, bg_l, _) = rgb_to_hls(r, g, b);
        let target_l = if bg_l > 0.5 { 0.35 } else { 0.75 };
        roles.insert(
            "accent".into(),
            make_role(
                accent_hue,
                rng.uniform(0.50, 0.70),
                target_l,
                "xs",
                "Primary accent: badges, tags, data point highlights, CTA",
            ),
        );
    }

    // 层级饱和度钳制
    for map in [&mut roles, &mut semantic] {
        for (_name, role) in map.iter_mut() {
            let tier = role["tier"].as_str().unwrap_or("").to_string();
            if let Some((_, cap)) = CASCADE_TIER_CAPS.iter().find(|(t, _)| *t == tier) {
                let s = role["hsl"][1].as_f64().unwrap_or(0.0);
                if s > *cap {
                    let h = role["hsl"][0].as_f64().unwrap_or(0.0);
                    let l = role["hsl"][2].as_f64().unwrap_or(0.5);
                    let hint = role["usage_hint"].as_str().unwrap_or("").to_string();
                    *role = make_role(h, cap * rng.uniform(0.85, 1.0), l, &tier, &hint);
                }
            }
        }
    }

    let role_hex = |name: &str| roles[name]["hex"].as_str().unwrap_or("").to_string();
    let cover = json!({
        "background":  role_hex("page_bg"),
        "top_bar":     role_hex("header_fill"),
        "sidebar":     role_hex("cover_block"),
        "accent_line": role_hex("accent"),
        "title":       role_hex("text_primary"),
        "subtitle":    role_hex("text_muted"),
        "watermark":   role_hex("border"),
    });
    let body = json!({
        "page_bg":      role_hex("page_bg"),
        "section_bg":   role_hex("section_bg"),
        "card_bg":      role_hex("card_bg"),
        "table_header": role_hex("header_fill"),
        "table_stripe": role_hex("table_stripe"),
        "border":       role_hex("border"),
        "heading":      role_hex("text_primary"),
        "body_text":    role_hex("text_primary"),
        "caption":      role_hex("text_muted"),
        "highlight":    role_hex("accent"),
    });
    let charts = json!({
        "series_1": role_hex("accent"),
        "series_2": role_hex("accent_secondary"),
        "series_3": role_hex("header_fill"),
        "series_4": role_hex("icon"),
        "series_5": role_hex("cover_block"),
        "grid":     role_hex("border"),
    });

    json!({
        "roles": roles,
        "cover": cover,
        "body": body,
        "charts": charts,
        "semantic": semantic,
        "meta": {
            "intent": intent, "mode": mode, "harmony": harmony,
            "base_hue": base_hue as i64, "accent_hue": accent_hue as i64,
            "tier_caps": serde_json::to_value(CASCADE_TIER_CAPS.iter()
                .map(|(k, v)| (k.to_string(), *v)).collect::<BTreeMap<_, _>>()).unwrap(),
        },
    })
}

/// 级联调色板 → CSS 自定义属性。
pub fn cascade_to_css(c: &Value) -> String {
    let mut out = String::from(":root {\n");
    if let Some(roles) = c["roles"].as_object() {
        for (name, role) in roles {
            out.push_str(&format!(
                "  --c-{}: {};\n",
                name.replace('_', "-"),
                role["hex"].as_str().unwrap_or("")
            ));
        }
    }
    if let Some(sem) = c["semantic"].as_object() {
        for (name, role) in sem {
            out.push_str(&format!(
                "  --c-semantic-{}: {};\n",
                name,
                role["hex"].as_str().unwrap_or("")
            ));
        }
    }
    out.push_str("}\n");
    out
}

/// 级联调色板 → ReportLab 常量（驼峰命名）。
pub fn cascade_to_reportlab(c: &Value) -> String {
    let mut out = String::from("# Auto-generated cascade palette — json2pdf\nfrom reportlab.lib.colors import HexColor\n\n");
    let push = |prefix: &str, name: &str, hex: &str, out: &mut String| {
        let const_name: String = name
            .split('_')
            .map(|w| {
                let mut cs = w.chars();
                match cs.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
                    None => String::new(),
                }
            })
            .collect::<String>();
        out.push_str(&format!("{prefix}{const_name} = HexColor(\"{hex}\")\n"));
    };
    if let Some(roles) = c["roles"].as_object() {
        for (name, role) in roles {
            push("", name, role["hex"].as_str().unwrap_or(""), &mut out);
        }
    }
    if let Some(sem) = c["semantic"].as_object() {
        for (name, role) in sem {
            push("SEM_", name, role["hex"].as_str().unwrap_or(""), &mut out);
        }
    }
    out
}

/// 审计扁平调色板：模式 S/L 边界 + WCAG。返回违规列表（空 = 干净）。
pub fn audit_palette(p: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    let mode = p["meta"]["mode"].as_str().unwrap_or("minimal").to_string();
    for key in ["bg", "mid", "accent", "text"] {
        let hex_val = p[key].as_str().unwrap_or("");
        if !hex_val.starts_with('#') {
            continue;
        }
        let (r, g, b) = crate::color::hex_to_rgb01(hex_val);
        let (h, l, s) = rgb_to_hls(r, g, b);
        let _ = h;
        if key == "bg" || key == "mid" {
            let cap = match mode.as_str() {
                "minimal" => 0.20,
                "dark" => 0.16,
                "pastel" => 0.42,
                "jewel" => 0.62,
                "light" => 0.28,
                _ => 0.15,
            };
            if s > cap {
                violations.push(format!("{key}: S={s:.3} > {cap} in {mode} mode"));
            }
        }
        if key == "accent" {
            let cap = match mode.as_str() {
                "minimal" => 0.78,
                "dark" => 0.62,
                "pastel" => 0.52,
                "jewel" => 0.52,
                "light" => 0.48,
                _ => 0.60,
            };
            if s > cap {
                violations.push(format!("accent: S={s:.3} > {cap} in {mode} mode"));
            }
        }
        if key == "bg" {
            let bad = match mode.as_str() {
                "dark" => l > 0.10,
                "minimal" => l < 0.90,
                "light" => l < 0.88,
                "jewel" => l > 0.28,
                "pastel" => l < 0.83,
                _ => false,
            };
            if bad {
                violations.push(format!("bg L={l:.3} violates {mode} mode bounds"));
            }
        }
    }
    let bg_hex = p["bg"].as_str().unwrap_or("");
    let text_hex = p["text"].as_str().unwrap_or("");
    let accent_hex = p["accent"].as_str().unwrap_or("");
    if bg_hex.starts_with('#') && text_hex.starts_with('#') {
        let cr = contrast_ratio(text_hex, bg_hex);
        if cr < 4.5 {
            violations.push(format!("text:bg contrast {cr:.2} < 4.5:1 (WCAG AA fail)"));
        }
    }
    if bg_hex.starts_with('#') && accent_hex.starts_with('#') {
        let cr = contrast_ratio(accent_hex, bg_hex);
        if cr < 2.5 {
            violations.push(format!(
                "accent:bg contrast {cr:.2} < 2.5:1 (accent invisible)"
            ));
        }
    }
    violations
}

/// 审计级联调色板：层级饱和度上限 + WCAG。
pub fn audit_cascade_palette(c: &Value) -> Vec<String> {
    let mut violations = Vec::new();
    let page_bg = c["roles"]["page_bg"]["hex"]
        .as_str()
        .unwrap_or("")
        .to_string();
    for (group, map) in [("roles", &c["roles"]), ("semantic", &c["semantic"])] {
        let Some(obj) = map.as_object() else { continue };
        for (name, role) in obj {
            let tier = role["tier"].as_str().unwrap_or("");
            let s = role["hsl"][1].as_f64().unwrap_or(0.0);
            if let Some((_, cap)) = CASCADE_TIER_CAPS.iter().find(|(t, _)| *t == tier) {
                if s > cap + 1e-6 {
                    violations.push(format!("{group}.{name}: tier {tier} S={s:.3} > cap {cap}"));
                }
            }
            if name == "text_primary" && page_bg.starts_with('#') {
                let cr = contrast_ratio(role["hex"].as_str().unwrap_or(""), &page_bg);
                if cr < 4.5 {
                    violations.push(format!("text_primary:page_bg contrast {cr:.2} < 4.5:1"));
                }
            }
            if name == "accent" && page_bg.starts_with('#') {
                let cr = contrast_ratio(role["hex"].as_str().unwrap_or(""), &page_bg);
                if cr < 3.0 {
                    violations.push(format!("accent:page_bg contrast {cr:.2} < 3.0:1"));
                }
            }
        }
    }
    violations
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_wcag_enforced() {
        for mode in MODES {
            for intent in ["neutral", "cold", "energy", "authority"] {
                for seed in [1u64, 7, 99] {
                    let p = generate_color_palette(intent, mode, None, Some(seed));
                    let cr = contrast_ratio(p["text"].as_str().unwrap(), p["bg"].as_str().unwrap());
                    assert!(cr >= 4.5, "text contrast {cr} for {intent}/{mode}/{seed}");
                    let ca =
                        contrast_ratio(p["accent"].as_str().unwrap(), p["bg"].as_str().unwrap());
                    assert!(ca >= 3.0, "accent contrast {ca} for {intent}/{mode}/{seed}");
                }
            }
        }
    }

    #[test]
    fn palette_deterministic_with_seed() {
        let a = generate_color_palette("cold", "minimal", None, Some(42));
        let b = generate_color_palette("cold", "minimal", None, Some(42));
        assert_eq!(a["bg"], b["bg"]);
        assert_eq!(a["accent"], b["accent"]);
    }

    #[test]
    fn palette_clean_by_construction() {
        for seed in [1u64, 2, 3, 4, 5] {
            let p = generate_color_palette("neutral", "minimal", None, Some(seed));
            assert!(
                audit_palette(&p).is_empty(),
                "audit found violations: {:?}",
                audit_palette(&p)
            );
        }
    }

    #[test]
    fn cascade_tier_caps_respected() {
        for mode in MODES {
            let c = generate_cascade_palette("neutral", mode, None, Some(11));
            assert!(
                audit_cascade_palette(&c).is_empty(),
                "cascade violations in {mode}: {:?}",
                audit_cascade_palette(&c)
            );
            let roles = c["roles"].as_object().unwrap();
            assert_eq!(roles.len(), 12, "12 roles expected");
        }
    }

    #[test]
    fn css_output_format() {
        let p = generate_color_palette("cold", "minimal", None, Some(1));
        let css = palette_to_css(&p);
        assert!(css.contains("--c-bg:") && css.contains(":root"));
    }
}
