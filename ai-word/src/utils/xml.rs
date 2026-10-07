//! XML 文本辅助：转义、颜色归一

/// 转义 XML 文本内容
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
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

/// 转义 XML 属性值（与文本转义相同，额外保留空白）
pub fn esc_attr(s: &str) -> String {
    esc(s)
}

/// 归一化颜色为 6 位大写十六进制（去掉 `#`，剔除非法字符）
pub fn color_hex(s: &str) -> String {
    let t = s.trim().trim_start_matches('#');
    let up = t.to_ascii_uppercase();
    if up.len() == 6 && up.chars().all(|c| c.is_ascii_hexdigit()) {
        up
    } else if up.len() == 3 && up.chars().all(|c| c.is_ascii_hexdigit()) {
        // #RGB → #RRGGBB
        up.chars().flat_map(|c| [c, c]).collect()
    } else {
        // 未知颜色回退黑色
        "000000".to_string()
    }
}

/// 判断字符串是否包含 CJK 字符
pub fn has_cjk(s: &str) -> bool {
    s.chars().any(|c| {
        let u = c as u32;
        (0x4E00..=0x9FFF).contains(&u)      // CJK 统一表意
            || (0x3400..=0x4DBF).contains(&u) // 扩展 A
            || (0x3000..=0x303F).contains(&u) // CJK 标点
            || (0xFF00..=0xFFEF).contains(&u) // 全角
    })
}

/// 生成 XML 元素（body 为空则自闭合）
pub fn el(name: &str, attrs: &[(&str, String)], body: &str) -> String {
    let mut s = String::new();
    s.push('<');
    s.push_str(name);
    for (k, v) in attrs {
        s.push(' ');
        s.push_str(k);
        s.push_str("=\"");
        s.push_str(&esc_attr(v));
        s.push('"');
    }
    if body.is_empty() {
        s.push_str("/>");
    } else {
        s.push('>');
        s.push_str(body);
        s.push_str("</");
        s.push_str(name);
        s.push('>');
    }
    s
}

/// 生成自闭合 XML 元素
pub fn empty(name: &str, attrs: &[(&str, String)]) -> String {
    el(name, attrs, "")
}

/// 生成带 XML 声明头的文档
pub fn declaration() -> &'static str {
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n"
}
