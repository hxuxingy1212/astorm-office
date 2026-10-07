//! 内容清洗 — 复刻 pdf.py 的 content_sanitize / code_sanitize。
//!
//! content_sanitize：删除控制/零宽/Bidi/变体选择符/非字符，全角 ASCII → 半角，
//! 连字分解，删软连字符；保留 XML 标签结构。
//! code_sanitize：还原 HTML 实体与 \uXXXX 转义、上下标 Unicode → <super>/<sub>、
//! 符号回退表替换。

fn is_bidi_control(c: char) -> bool {
    matches!(c as u32, 0x202A..=0x202E | 0x2066..=0x2069 | 0x200E | 0x200F)
}

fn is_variation_selector(c: char) -> bool {
    matches!(c as u32, 0xFE00..=0xFE0F | 0xE0100..=0xE01EF)
}

fn is_non_character(c: char) -> bool {
    let u = c as u32;
    (0xFDD0..=0xFDEF).contains(&u) || (u & 0xFFFE) == 0xFFFE
}

fn is_zero_width(c: char) -> bool {
    matches!(c as u32, 0x200B..=0x200D | 0xFEFF | 0x2060)
}

/// 全角 ASCII → 半角（U+FF01-FF5E → 0x21-0x7E，U+3000 → 空格）。
fn fullwidth_to_ascii(c: char) -> Option<char> {
    match c as u32 {
        0x3000 => Some(' '),
        0xFF01..=0xFF5E => char::from_u32((c as u32) - 0xFEE0),
        _ => None,
    }
}

/// 常见连字 → 分解形式。
fn decompose_ligature(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{FB00}' => "ff",
        '\u{FB01}' => "fi",
        '\u{FB02}' => "fl",
        '\u{FB03}' => "ffi",
        '\u{FB04}' => "ffl",
        '\u{0132}' => "IJ",
        '\u{0133}' => "ij",
        '\u{0152}' => "OE",
        '\u{0153}' => "oe",
        '\u{00C6}' => "AE",
        '\u{00E6}' => "ae",
        _ => return None,
    })
}

/// content_sanitize — 返回 (清洗后文本, PUA 警告数)。
pub fn content_sanitize(text: &str) -> (String, usize) {
    let mut out = String::with_capacity(text.len());
    let mut pua_warn = 0usize;
    for c in text.chars() {
        let u = c as u32;
        // 控制字符：保留 \t \n \r
        if u < 0x20 && !matches!(c, '\t' | '\n' | '\r') {
            continue;
        }
        if u == 0x7F || (0x80..=0x9F).contains(&u) {
            continue;
        }
        if is_zero_width(c) || is_bidi_control(c) || is_variation_selector(c) || is_non_character(c)
        {
            continue;
        }
        if c == '\u{00AD}' {
            continue; // 软连字符
        }
        if (0xE000..=0xF8FF).contains(&u) || (0xF0000..=0xFFFFD).contains(&u) {
            pua_warn += 1;
            continue; // 私有区字符直接剔除并计数
        }
        if let Some(repl) = decompose_ligature(c) {
            out.push_str(repl);
            continue;
        }
        if let Some(half) = fullwidth_to_ascii(c) {
            out.push(half);
            continue;
        }
        out.push(c);
    }
    (out, pua_warn)
}

// ── code.sanitize ─────────────────────────────────────────────────────────

/// 常用 HTML 实体表。
const HTML_ENTITIES: &[(&str, &str)] = &[
    ("&amp;", "&"),
    ("&lt;", "<"),
    ("&gt;", ">"),
    ("&quot;", "\""),
    ("&apos;", "'"),
    ("&nbsp;", "\u{00A0}"),
    ("&mdash;", "—"),
    ("&ndash;", "–"),
    ("&hellip;", "…"),
    ("&bull;", "•"),
    ("&copy;", "©"),
    ("&reg;", "®"),
    ("&trade;", "™"),
    ("&times;", "×"),
    ("&divide;", "÷"),
    ("&plusmn;", "±"),
    ("&deg;", "°"),
    ("&micro;", "µ"),
    ("&para;", "¶"),
    ("&sect;", "§"),
    ("&middot;", "·"),
    ("&laquo;", "«"),
    ("&raquo;", "»"),
];

/// 上标/下标 Unicode → <super>/<sub> 包裹。
const SUPERSCRIPTS: &[(char, char)] = &[
    ('\u{00B9}', '1'),
    ('\u{00B2}', '2'),
    ('\u{00B3}', '3'),
    ('\u{2070}', '0'),
    ('\u{2074}', '4'),
    ('\u{2075}', '5'),
    ('\u{2076}', '6'),
    ('\u{2077}', '7'),
    ('\u{2078}', '8'),
    ('\u{2079}', '9'),
    ('\u{207A}', '+'),
    ('\u{207B}', '-'),
    ('\u{2070}', '0'),
];
const SUBSCRIPTS: &[(char, char)] = &[
    ('\u{2080}', '0'),
    ('\u{2081}', '1'),
    ('\u{2082}', '2'),
    ('\u{2083}', '3'),
    ('\u{2084}', '4'),
    ('\u{2085}', '5'),
    ('\u{2086}', '6'),
    ('\u{2087}', '7'),
    ('\u{2088}', '8'),
    ('\u{2089}', '9'),
    ('\u{208A}', '+'),
    ('\u{208B}', '-'),
];

/// 符号回退表 — ReportLab 默认字体渲染不稳的符号替换为安全写法。
const SYMBOL_FALLBACK: &[(char, &str)] = &[
    ('\u{2018}', "'"),
    ('\u{2019}', "'"),
    ('\u{201C}', "\""),
    ('\u{201D}', "\""),
    ('\u{2013}', "-"),
    ('\u{2014}', "--"),
    ('\u{2026}', "..."),
    ('\u{00D7}', "x"),
    ('\u{2265}', ">="),
    ('\u{2264}', "<="),
    ('\u{2260}', "!="),
    ('\u{2248}', "~="),
    ('\u{221E}', "inf"),
    ('\u{2212}', "-"),
    ('\u{00B1}', "+/-"),
    ('\u{2227}', "AND"),
    ('\u{2228}', "OR"),
    ('\u{00B7}', "-"),
];

fn decode_html_entities(s: &str) -> String {
    let mut out = s.to_string();
    for (ent, ch) in HTML_ENTITIES {
        out = out.replace(ent, ch);
    }
    // 数字实体 &#NNN; / &#xHH;
    let mut result = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(idx) = rest.find("&#") {
        result.push_str(&rest[..idx]);
        let tail = &rest[idx + 2..];
        let (num_part, after) = match tail.find(';') {
            Some(end) => (&tail[..end], &tail[end + 1..]),
            None => {
                result.push_str(&rest[idx..]);
                rest = "";
                break;
            }
        };
        let decoded = if let Some(hex) = num_part.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else {
            num_part.parse::<u32>().ok().and_then(char::from_u32)
        };
        match decoded {
            Some(ch) => result.push(ch),
            None => result.push_str(&format!("&#{num_part};")),
        }
        rest = after;
    }
    result.push_str(rest);
    result
}

fn decode_unicode_escapes(s: &str) -> String {
    // \uXXXX（JSON/JS 风格转义）
    let mut result = String::with_capacity(s.len());
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '\\' && i + 1 < bytes.len() && bytes[i + 1] == 'u' && i + 5 < bytes.len() {
            let hex: String = bytes[i + 2..i + 6].iter().collect();
            if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                if let Some(ch) = char::from_u32(cp) {
                    result.push(ch);
                    i += 6;
                    continue;
                }
            }
        }
        result.push(bytes[i]);
        i += 1;
    }
    result
}

/// code.sanitize — 就地改写生成脚本里的文本常量。
pub fn code_sanitize(source: &str) -> String {
    let mut s = decode_html_entities(source);
    s = decode_unicode_escapes(&s);

    // 上下标 → 标签（相邻同类合并）
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if let Some((_, normal)) = SUPERSCRIPTS.iter().find(|(sc, _)| *sc == c) {
            let mut buf = normal.to_string();
            let mut j = i + 1;
            while j < chars.len() {
                if let Some((_, n)) = SUPERSCRIPTS.iter().find(|(sc, _)| *sc == chars[j]) {
                    buf.push(*n);
                    j += 1;
                } else {
                    break;
                }
            }
            out.push_str(&format!("<super>{buf}</super>"));
            i = j;
            continue;
        }
        if let Some((_, normal)) = SUBSCRIPTS.iter().find(|(sc, _)| *sc == c) {
            let mut buf = normal.to_string();
            let mut j = i + 1;
            while j < chars.len() {
                if let Some((_, n)) = SUBSCRIPTS.iter().find(|(sc, _)| *sc == chars[j]) {
                    buf.push(*n);
                    j += 1;
                } else {
                    break;
                }
            }
            out.push_str(&format!("<sub>{buf}</sub>"));
            i = j;
            continue;
        }
        if let Some((_, repl)) = SYMBOL_FALLBACK.iter().find(|(sc, _)| *sc == c) {
            out.push_str(repl);
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_removes_junk() {
        let (out, _) = content_sanitize("a\u{200B}b\u{202E}c\u{FE0F}d\u{00AD}e");
        assert_eq!(out, "abcde");
    }

    #[test]
    fn content_fullwidth_ascii() {
        let (out, _) = content_sanitize("１２３ＡＢＣ　Ｘ");
        assert_eq!(out, "123ABC X");
    }

    #[test]
    fn content_ligatures() {
        let (out, _) = content_sanitize("ﬁle ﬀ");
        assert_eq!(out, "file ff");
    }

    #[test]
    fn content_keeps_cjk_and_newlines() {
        let (out, pua) = content_sanitize("中文\t保留\n换行");
        assert_eq!(out, "中文\t保留\n换行");
        assert_eq!(pua, 0);
    }

    #[test]
    fn content_pua_counted() {
        let (_, pua) = content_sanitize("a\u{E000}b\u{F8FF}");
        assert_eq!(pua, 2);
    }

    #[test]
    fn code_entities() {
        assert_eq!(code_sanitize("&amp;&lt;&gt;"), "&<>");
        assert_eq!(code_sanitize("&#20013;&#25991;"), "中文");
        assert_eq!(code_sanitize("&#x4e2d;"), "中");
    }

    #[test]
    fn code_unicode_escapes() {
        assert_eq!(code_sanitize("\\u4e2d\\u6587"), "中文");
    }

    #[test]
    fn code_sup_sub() {
        assert_eq!(
            code_sanitize("x\u{00B2}+y\u{00B3}"),
            "x<super>2</super>+y<super>3</super>"
        );
        assert_eq!(code_sanitize("H\u{2082}O"), "H<sub>2</sub>O");
    }

    #[test]
    fn code_symbol_fallback() {
        assert_eq!(code_sanitize("a\u{2265}b"), "a>=b");
        assert_eq!(code_sanitize("\u{201C}q\u{201D}"), "\"q\"");
    }
}
