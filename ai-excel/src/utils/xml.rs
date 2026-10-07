//! XML escaping helpers.

pub fn esc_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Strip an ARGB ("FFRRGGBB") color down to "RRGGBB" uppercased. Non-8/6
/// length inputs are returned trimmed but otherwise unchanged.
pub fn normalize_color(raw: &str) -> String {
    let s = raw.trim().trim_start_matches('#').to_ascii_uppercase();
    if s.len() == 8 {
        s[2..].to_string()
    } else {
        s
    }
}

/// Convert "RRGGBB" to OOXML ARGB "FFRRGGBB".
pub fn to_argb(rgb: &str) -> String {
    let s = normalize_color(rgb);
    if s.len() == 6 {
        format!("FF{s}")
    } else {
        s
    }
}
