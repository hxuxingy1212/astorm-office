//! Shared Excel number-format rendering for previews and text views.

/// 按 `;` 切分格式段，忽略引号与反斜杠转义内的分隔符
fn split_sections(nf: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = nf.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                cur.push(c);
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            '"' => {
                cur.push(c);
                for n in chars.by_ref() {
                    cur.push(n);
                    if n == '"' {
                        break;
                    }
                }
            }
            ';' => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// 去掉 `[Red]`/`[Color 3]`/`[$€-x]`/条件等格式修饰符，返回 (清理后的模板, 颜色)
fn strip_modifiers(sec: &str) -> (String, Option<String>) {
    let mut out = String::new();
    let mut color = None;
    let mut chars = sec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            out.push(c);
            if let Some(next) = chars.next() {
                out.push(next);
            }
            continue;
        }
        if c == '"' {
            out.push(c);
            for next in chars.by_ref() {
                out.push(next);
                if next == '"' {
                    break;
                }
            }
            continue;
        }
        if c == '[' {
            let mut inner = String::new();
            for n in chars.by_ref() {
                if n == ']' {
                    break;
                }
                inner.push(n);
            }
            let low = inner.to_ascii_lowercase();
            for name in ["red", "blue", "green", "black", "white", "yellow"] {
                if low == name {
                    color = Some(name.to_string());
                }
            }
            if let Some(n) = low.strip_prefix("color") {
                let idx = n.trim().parse::<usize>().unwrap_or(0);
                // 兼容旧 indexed 调色板
                let pal = [
                    "000000", "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF",
                ];
                color = Some(pal.get(idx).copied().unwrap_or("000000").to_string());
            }
            // `[$€-x]`：保留货币符号
            if let Some(sym) = inner.strip_prefix('$') {
                let cur = sym.split('-').next().unwrap_or("");
                out.push_str(cur);
            }
            continue;
        }
        out.push(c);
    }
    (out, color)
}

/// Excel 数字格式渲染（分段 / 转义 / 会计括号 / 颜色 / 百分号 / 千分位）。
/// 返回 (文本, 颜色)。
pub fn format_number_ex(v: f64, nf: &str) -> (String, Option<String>) {
    let sections = split_sections(nf);
    let idx = if v > 0.0 {
        0
    } else if v < 0.0 {
        if sections.len() > 1 {
            1
        } else {
            0
        }
    } else if sections.len() > 2 {
        2
    } else {
        0
    };
    let raw = sections.get(idx).cloned().unwrap_or_default();
    let (tpl, color) = strip_modifiers(&raw);
    // 只有一段且值为负：数值前补负号（Excel 行为）
    let add_sign = v < 0.0 && sections.len() == 1 && !tpl.contains('-');
    let text = render_section(v.abs(), &tpl, add_sign);
    (text, color)
}

fn percent_count(template: &str) -> usize {
    let mut chars = template.chars();
    let mut count = 0;
    while let Some(c) = chars.next() {
        match c {
            '\\' | '_' | '*' => {
                chars.next();
            }
            '"' => {
                for next in chars.by_ref() {
                    if next == '"' {
                        break;
                    }
                }
            }
            '%' => count += 1,
            _ => {}
        }
    }
    count
}

/// 渲染单个格式段（v 已取绝对值）
fn render_section(v: f64, tpl: &str, add_sign: bool) -> String {
    let v = v * 100f64.powi(percent_count(tpl) as i32);
    // 提取数字模式（含 #0?., 与 %），其余为字面量
    let mut pattern = String::new();
    let mut out = String::new();
    let mut chars = tpl.chars().peekable();
    let mut first = true;
    let flush = |out: &mut String, pattern: &mut String, first: &mut bool, v: f64| {
        if pattern.is_empty() {
            return;
        }
        if *first {
            *out = format!("{}{}", out, number_from_pattern(v, pattern));
            *first = false;
        }
        pattern.clear();
    };
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                // 转义字符：字面输出
                flush(&mut out, &mut pattern, &mut first, v);
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            }
            '"' => {
                flush(&mut out, &mut pattern, &mut first, v);
                for n in chars.by_ref() {
                    if n == '"' {
                        break;
                    }
                    out.push(n);
                }
            }
            '_' => {
                // 占位空白（宽度取下一个字符）：用空格近似（需先冲刷已累计的数字模式）
                flush(&mut out, &mut pattern, &mut first, v);
                chars.next();
                out.push(' ');
            }
            '*' => {
                // 重复填充：忽略填充字符（需先冲刷已累计的数字模式）
                flush(&mut out, &mut pattern, &mut first, v);
                chars.next();
            }
            '#' | '0' | '?' | '.' | ',' => pattern.push(c),
            '%' => {
                flush(&mut out, &mut pattern, &mut first, v);
                out.push('%');
            }
            _ => {
                flush(&mut out, &mut pattern, &mut first, v);
                out.push(c);
            }
        }
    }
    flush(&mut out, &mut pattern, &mut first, v);
    if add_sign {
        format!("-{out}")
    } else {
        out
    }
}

/// 依据模式串渲染数字：
/// - `0` 固定位数、`#` 可选位（无则省略）、`?` 可选位（无则补空格，会计格式用）
/// - 支持 `%`、千分位、固定/可选小数
fn number_from_pattern(v: f64, pattern: &str) -> String {
    let pct = pattern.contains('%');
    let unscaled = pattern.trim_end_matches('%');
    let scale_commas = unscaled.len() - unscaled.trim_end_matches(',').len();
    let pattern = if scale_commas > 0 {
        unscaled.trim_end_matches(',')
    } else {
        pattern
    };
    let x = if pct { v * 100.0 } else { v } / 1000f64.powi(scale_commas as i32);
    let (int_pat, frac_pat) = match pattern.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (pattern, None),
    };
    let int_pat: String = int_pat.chars().filter(|c| *c != '%').collect();
    let group = int_pat.contains(',');
    let int_places = int_pat
        .chars()
        .filter(|c| matches!(c, '#' | '0' | '?'))
        .count();
    let int_optional = int_pat
        .chars()
        .filter(|c| matches!(c, '#' | '0' | '?'))
        .all(|c| c == '?');

    let frac: String = frac_pat
        .map(|f| {
            f.chars()
                .take_while(|c| matches!(c, '0' | '#' | '?' | '%'))
                .filter(|c| *c != '%')
                .collect()
        })
        .unwrap_or_default();
    let frac_places = frac
        .chars()
        .filter(|c| matches!(c, '0' | '#' | '?'))
        .count();
    let frac_fixed = frac.chars().any(|c| c == '0') && frac.chars().all(|c| c == '0');

    // Excel rounds decimal ties away from zero; Rust's formatter uses ties-to-even.
    let scale = 10f64.powi(frac_places.min(308) as i32);
    let rounded = if (x * scale).is_finite() {
        (x * scale).round() / scale
    } else {
        x
    };
    let mut s = format!("{rounded:.frac_places$}");
    // 可选小数位：去掉尾部无意义的 0（至少保留 0 的数量）
    if frac_places > 0 && !frac_fixed {
        let keep = frac.chars().take_while(|c| *c == '0').count();
        if let Some((i, f)) = s.split_once('.') {
            let mut f = f.to_string();
            while f.len() > keep && f.ends_with('0') {
                f.pop();
            }
            s = if f.is_empty() {
                i.to_string()
            } else {
                format!("{i}.{f}")
            };
        }
    }
    let min_digits = int_pat.chars().filter(|c| *c == '0').count();
    let digits = s.split('.').next().unwrap_or("").len();
    if digits < min_digits {
        s = format!("{}{s}", "0".repeat(min_digits - digits));
    }
    if group {
        s = group_int(&s);
    }
    // `?` 整数占位：不足宽度补空格（全 `?` 且值为 0 → 整段留空，会计格式的“-”）
    if int_optional && int_places > 0 {
        let (sign, body) = match s.strip_prefix('-') {
            Some(b) => ("-", b.to_string()),
            None => ("", s),
        };
        let (int, frac) = match body.split_once('.') {
            Some((i, f)) => (i.to_string(), Some(f.to_string())),
            None => (body, None),
        };
        let padded = if x.abs() < f64::EPSILON {
            " ".repeat(int_places)
        } else {
            format!("{int:>int_places$}")
        };
        s = match frac {
            Some(f) => format!("{sign}{padded}.{f}"),
            None => format!("{sign}{padded}"),
        };
    }
    if pct {
        s.push('%');
    }
    s
}

/// 千分位分组（保留符号与小数部分）
fn group_int(s: &str) -> String {
    let (sign, body) = match s.strip_prefix('-') {
        Some(b) => ("-", b),
        None => ("", s),
    };
    let (int, frac) = match body.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (body, None),
    };
    let mut out = String::new();
    let bytes = int.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    match frac {
        Some(f) => format!("{sign}{out}.{f}"),
        None => format!("{sign}{out}"),
    }
}

#[derive(Debug)]
enum DateToken {
    Literal(String),
    Field(char, usize),
    AmPm,
    Elapsed(char, usize),
}

fn date_tokens(code: &str) -> Option<Vec<DateToken>> {
    let mut tokens = Vec::new();
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut literal = String::new();
                loop {
                    match chars.next()? {
                        '"' => break,
                        c => literal.push(c),
                    }
                }
                tokens.push(DateToken::Literal(literal));
            }
            '\\' => tokens.push(DateToken::Literal(chars.next()?.to_string())),
            '[' => {
                let mut inner = String::new();
                loop {
                    match chars.next()? {
                        ']' => break,
                        c => inner.push(c),
                    }
                }
                let inner = inner.to_ascii_lowercase();
                if matches!(inner.as_str(), "h" | "hh" | "m" | "mm" | "s" | "ss") {
                    tokens.push(DateToken::Elapsed(inner.chars().next()?, inner.len()));
                } else if inner.starts_with('$')
                    || matches!(
                        inner.as_str(),
                        "red" | "blue" | "green" | "black" | "white" | "yellow"
                    )
                {
                    // Locale and display colors do not alter the numeric date serial.
                } else {
                    return None;
                }
            }
            'a' | 'A' if chars.peek().is_some_and(|c| c.eq_ignore_ascii_case(&'m')) => {
                for expected in "M/PM".chars() {
                    if !chars.next()?.eq_ignore_ascii_case(&expected) {
                        return None;
                    }
                }
                tokens.push(DateToken::AmPm);
            }
            c if matches!(c.to_ascii_lowercase(), 'y' | 'm' | 'd' | 'h' | 's') => {
                let c = c.to_ascii_lowercase();
                let mut count = 1;
                while chars
                    .peek()
                    .is_some_and(|next| next.to_ascii_lowercase() == c)
                {
                    chars.next();
                    count += 1;
                }
                tokens.push(DateToken::Field(c, count));
            }
            c if c.is_ascii_alphabetic() || matches!(c, '#' | '0' | '?' | '%' | '_' | '*') => {
                return None
            }
            c => tokens.push(DateToken::Literal(c.to_string())),
        }
    }
    tokens
        .iter()
        .any(|t| matches!(t, DateToken::Field(..) | DateToken::Elapsed(..)))
        .then_some(tokens)
}

/// Format common Excel date/time codes using the workbook's date system.
/// Return None for unsupported formats or out-of-range serials.
pub fn format_datetime(serial: f64, code: &str, date1904: bool) -> Option<String> {
    use chrono::Datelike;
    if !serial.is_finite() || serial < 0.0 || serial >= i64::MAX as f64 {
        return None;
    }
    let sections = split_sections(code);
    let selected = if serial == 0.0 && sections.len() > 2 {
        &sections[2]
    } else {
        &sections[0]
    };
    let tokens = date_tokens(selected)?;
    let has_time = tokens.iter().any(|token| {
        matches!(
            token,
            DateToken::Field('h' | 's', _) | DateToken::Elapsed(..) | DateToken::AmPm
        )
    });
    let seconds = if has_time {
        (serial.fract() * 86400.0).round() as u64
    } else {
        0
    };
    let day = serial.floor() as i64 + (seconds / 86400) as i64;
    let date = super::date::serial_to_date(day, date1904)?;
    let fictitious = !date1904 && day == 60;
    let (hour, minute, second) = (
        (seconds % 86400) / 3600,
        (seconds % 3600) / 60,
        seconds % 60,
    );
    let ampm = tokens.iter().any(|t| matches!(t, DateToken::AmPm));
    let mut out = String::new();
    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    for (i, token) in tokens.iter().enumerate() {
        match token {
            DateToken::Literal(s) => out.push_str(s),
            DateToken::AmPm => out.push_str(if hour < 12 { "AM" } else { "PM" }),
            DateToken::Elapsed(c, width) => {
                let total = (serial * 86400.0).round();
                let value = match c {
                    'h' => (total / 3600.0).floor(),
                    'm' => (total / 60.0).floor(),
                    _ => total,
                };
                out.push_str(&if *width == 1 {
                    format!("{value:.0}")
                } else {
                    format!("{value:02.0}")
                });
            }
            DateToken::Field(c, width) => {
                let previous = tokens[..i].iter().rev().find_map(|t| match t {
                    DateToken::Field(c, _) | DateToken::Elapsed(c, _) => Some(*c),
                    _ => None,
                });
                let next = tokens[i + 1..].iter().find_map(|t| match t {
                    DateToken::Field(c, _) | DateToken::Elapsed(c, _) => Some(*c),
                    _ => None,
                });
                let text = match (*c, *width) {
                    ('y', 1 | 2) => format!("{:02}", date.year().rem_euclid(100)),
                    ('y', 3 | 4) => format!("{:04}", date.year()),
                    ('m', 1 | 2) if previous == Some('h') || next == Some('s') => {
                        if *width == 1 {
                            minute.to_string()
                        } else {
                            format!("{minute:02}")
                        }
                    }
                    ('m', 1) => date.month().to_string(),
                    ('m', 2) => format!("{:02}", date.month()),
                    ('m', 3) => months[date.month0() as usize][..3].to_string(),
                    ('m', 4) => months[date.month0() as usize].to_string(),
                    ('m', 5) => months[date.month0() as usize][..1].to_string(),
                    ('d', 1) => {
                        if fictitious {
                            "29".into()
                        } else {
                            date.day().to_string()
                        }
                    }
                    ('d', 2) => {
                        if fictitious {
                            "29".into()
                        } else {
                            format!("{:02}", date.day())
                        }
                    }
                    ('d', 3) => date.format("%a").to_string(),
                    ('d', 4) => date.format("%A").to_string(),
                    ('h', 1 | 2) => {
                        let h = if ampm {
                            if hour % 12 == 0 {
                                12
                            } else {
                                hour % 12
                            }
                        } else {
                            hour
                        };
                        if *width == 1 {
                            h.to_string()
                        } else {
                            format!("{h:02}")
                        }
                    }
                    ('s', 1) => second.to_string(),
                    ('s', 2) => format!("{second:02}"),
                    _ => return None,
                };
                out.push_str(&text);
            }
        }
    }
    Some(out)
}

/// Supported numeric subset: fixed/optional decimals, grouping, percentages,
/// currency/literals and positive/negative/zero sections. Avoid silently treating
/// dates, conditions, fractions or scientific notation as fixed-point numbers.
pub fn format_number_checked(value: f64, code: &str) -> Option<String> {
    if !value.is_finite() || code.len() > 256 {
        return None;
    }
    for section in split_sections(code) {
        let mut chars = section.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' => loop {
                    if chars.next()? == '"' {
                        break;
                    }
                },
                '\\' | '_' | '*' => {
                    chars.next()?;
                }
                '[' => {
                    let mut inner = String::new();
                    loop {
                        match chars.next()? {
                            ']' => break,
                            c => inner.push(c),
                        }
                    }
                    let lower = inner.to_ascii_lowercase();
                    let known = inner.starts_with('$')
                        || lower.starts_with("color")
                        || matches!(
                            lower.as_str(),
                            "red"
                                | "blue"
                                | "green"
                                | "black"
                                | "white"
                                | "yellow"
                                | "cyan"
                                | "magenta"
                        );
                    if !known {
                        return None;
                    }
                    if inner.contains(['<', '>', '='])
                        || matches!(
                            inner.to_ascii_lowercase().as_str(),
                            "h" | "hh" | "m" | "mm" | "s" | "ss"
                        )
                    {
                        return None;
                    }
                }
                c if c.is_ascii_alphabetic() || matches!(c, '/' | '@') => return None,
                _ => {}
            }
        }
    }
    Some(format_number_ex(value, code).0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_dates_and_times() {
        for (serial, code, epoch, expected) in [
            (46053.5, "yyyy-mm-dd hh:mm:ss", false, "2026-01-31 12:00:00"),
            (46053.5, "m/d/yy h:mm AM/PM", false, "1/31/26 12:00 PM"),
            (0.0, "yyyy-mm-dd", true, "1904-01-01"),
            (1.0, "yyyy-mm-dd", false, "1900-01-01"),
            (60.0, "yyyy-mm-dd", false, "1900-02-29"),
            (61.0, "yyyy-mm-dd", false, "1900-03-01"),
            (2.5, "[h]:mm:ss", false, "60:00:00"),
            (46053.0, "dd mmm yyyy", false, "31 Jan 2026"),
            (46053.0, "yyyy\"年\"m\"月\"d\"日\"", false, "2026年1月31日"),
        ] {
            assert_eq!(
                format_datetime(serial, code, epoch).as_deref(),
                Some(expected),
                "{code}"
            );
        }
        assert!(format_datetime(1e100, "yyyy-mm-dd", false).is_none());
        assert!(format_datetime(0.5, "hh:mm:ss.000", false).is_none());
        assert!(format_number_checked(1.0, "0.00E+00").is_none());
        assert!(format_number_checked(1.0, "[>10]0.0;0.00").is_none());
    }

    #[test]
    fn common_numbers_and_literal_percent() {
        for (value, code, expected) in [
            (0.125, "0.00%", "12.50%"),
            (0.0, "0.0%", "0.0%"),
            (1234.5, "#,##0.00", "1,234.50"),
            (-50.0, "0.00;(0.00)", "(50.00)"),
            (12.5, "\"$\"#,##0.00", "$12.50"),
            (12.5, "0.0\"%\"", "12.5%"),
            (0.125, "0%", "13%"),
            (0.125, "0.0 %", "12.5 %"),
            (0.125, "%0.0", "%12.5"),
            (7.0, "0000", "0007"),
            (1200000.0, "0.0,,", "1.2"),
            (7.0, "0\"[Red]\"", "7[Red]"),
        ] {
            assert_eq!(
                format_number_checked(value, code).as_deref(),
                Some(expected),
                "{code}"
            );
        }
    }
}
