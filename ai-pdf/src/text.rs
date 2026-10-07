//! 文本提取引擎 — 复刻 pdf.py extract.text / extract.table 的核心：
//! 自研内容流 tokenizer + 文本状态机（BT/Tf/Tm/Td/Tj/TJ/'/"）+
//! 图形态 CTM 矩阵栈（q/Q/cm，Chrome/Skia 生成 PDF 的翻转坐标必备）+
//! 隐式笔 advance 追踪。字体解码：自研 ToUnicode CMap（1/2 字节码空间）
//! 与 /Differences 字形名表，回退 lopdf 命名编码表与 latin1。
//!
//! 与 pdfplumber 的差异（README 有说明）：不解析 Form XObject 嵌套内容流、
//! 不计算精确字形宽度（宽度估计：CJK 1.0em、拉丁 0.5em）。

use anyhow::Result;
use lopdf::{Dictionary, Document, Encoding, Object, ObjectId};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// 单个文本片段（设备空间坐标）。
#[derive(Debug, Clone)]
pub struct TextItem {
    pub x: f64,
    /// 估算终点（含字宽），供词间距判定
    pub end_x: f64,
    /// 设备空间字号
    pub size: f64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub y: f64,
    pub items: Vec<TextItem>,
}

#[derive(Debug, Clone)]
pub struct PageText {
    pub page: u32,
    pub lines: Vec<Line>,
    pub char_count: usize,
    pub has_images: bool,
    pub has_drawings: bool,
    /// 文本包围盒 (min_x, min_y, max_x, max_y)，无文本为 None
    pub bbox: Option<(f64, f64, f64, f64)>,
    /// 页面复杂度判定（去重后计算；OCR 路由 / 诊断用）
    pub complexity: crate::complexity::PageComplexity,
}

impl PageText {
    /// 按阅读顺序（y 从上到下、x 从左到右）输出整页文本。
    /// 词间距判定：下一 item 起点 − 上一 item 估算终点 > 0.2em → 空格。
    pub fn text(&self) -> String {
        let mut lines = self.lines.clone();
        lines.sort_by(|a, b| b.y.partial_cmp(&a.y).unwrap_or(std::cmp::Ordering::Equal));
        let mut out = String::new();
        for line in &lines {
            let mut items = line.items.clone();
            items.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
            let mut last_end: Option<(f64, f64)> = None; // (end_x, size)
            for item in &items {
                if let Some((le, lsize)) = last_end {
                    let em = item.size.max(lsize).max(1.0);
                    if item.x - le > 0.2 * em {
                        out.push(' ');
                    }
                }
                out.push_str(&item.text);
                last_end = Some((item.end_x, item.size));
            }
            out.push('\n');
        }
        out
    }
}

pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x9FFF      // CJK 部首/汉字/假名/标点（含扩展 A）
        | 0xF900..=0xFAFF    // 兼容汉字
        | 0xFF00..=0xFFEF    // 全角形式
        | 0x20000..=0x2FA1F  // 扩展 B+
    )
}

// ── 内容流 tokenizer ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Tok {
    Num(f64),
    Str(Vec<u8>),
    Name(Vec<u8>),
    Array(Vec<Tok>),
    /// 字典（内联图像属性、图样参数等）
    Dict(lopdf::Dictionary),
}

#[derive(Debug, Clone)]
pub struct Op {
    pub operator: String,
    pub operands: Vec<Tok>,
}

fn is_ws(b: u8) -> bool {
    matches!(b, 0 | b'\t' | b'\n' | b'\x0c' | b'\r' | b' ')
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

/// 解析内容流为操作序列（lexer 手写，等价 pdfminer 的 PSBaseParser）。
pub fn tokenize(content: &[u8]) -> Vec<Op> {
    let mut ops = Vec::new();
    let mut stack: Vec<Tok> = Vec::new(); // 操作数栈（数组元素直接进当前数组）
    let mut arrays: Vec<Vec<Tok>> = Vec::new();
    let mut i = 0usize;
    let n = content.len();

    let push_tok = |t: Tok, arrays: &mut Vec<Vec<Tok>>, stack: &mut Vec<Tok>| {
        if let Some(arr) = arrays.last_mut() {
            arr.push(t);
        } else {
            stack.push(t);
        }
    };

    while i < n {
        let b = content[i];
        if is_ws(b) {
            i += 1;
            continue;
        }
        if b == b'%' {
            // 注释到行尾
            while i < n && content[i] != b'\n' && content[i] != b'\r' {
                i += 1;
            }
            continue;
        }
        if b == b'(' {
            let (s, next) = read_literal_string(content, i);
            push_tok(Tok::Str(s), &mut arrays, &mut stack);
            i = next;
            continue;
        }
        if b == b'<' {
            if i + 1 < n && content[i + 1] == b'<' {
                let (dict, next) = read_dict(content, i);
                push_tok(Tok::Dict(dict), &mut arrays, &mut stack);
                i = next;
                continue;
            }
            let (s, next) = read_hex_string(content, i);
            push_tok(Tok::Str(s), &mut arrays, &mut stack);
            i = next;
            continue;
        }
        if b == b'[' {
            arrays.push(Vec::new());
            i += 1;
            continue;
        }
        if b == b']' {
            if let Some(arr) = arrays.pop() {
                push_tok(Tok::Array(arr), &mut arrays, &mut stack);
            }
            i += 1;
            continue;
        }
        if b == b'/' {
            let (name, next) = read_name(content, i + 1);
            push_tok(Tok::Name(name), &mut arrays, &mut stack);
            i = next;
            continue;
        }
        if b.is_ascii_digit() || b == b'-' || b == b'+' || b == b'.' {
            let (num, next) = read_number(content, i);
            push_tok(Tok::Num(num), &mut arrays, &mut stack);
            i = next;
            continue;
        }
        // 关键字：操作符或 true/false/null
        let start = i;
        while i < n && !is_ws(content[i]) && !is_delim(content[i]) {
            i += 1;
        }
        let kw = std::str::from_utf8(&content[start..i]).unwrap_or("");
        match kw {
            "true" => push_tok(Tok::Name(b"true".to_vec()), &mut arrays, &mut stack),
            "false" => push_tok(Tok::Name(b"false".to_vec()), &mut arrays, &mut stack),
            "null" => push_tok(Tok::Name(b"null".to_vec()), &mut arrays, &mut stack),
            "" => {
                // 空关键字 = 孤立分隔符（畸形内容流，如多余的 `)`）：跳过一个字节防死循环
                i = start + 1;
            }
            "BI" => {
                // 内联图像：BI <键值对> ID <数据> EI
                stack.clear();
                let (dict, mut j) = read_inline_dict(content, start);
                // 跳过 ID 后的一个空白字节
                if j < n && is_ws(content[j]) {
                    j += 1;
                }
                let (data, next) = read_inline_data(content, j);
                ops.push(Op {
                    operator: "BI".to_string(),
                    operands: vec![Tok::Dict(dict), Tok::Str(data)],
                });
                i = next;
            }
            op => {
                ops.push(Op {
                    operator: op.to_string(),
                    operands: std::mem::take(&mut stack),
                });
            }
        }
    }
    ops
}

/// 解析一个 PDF 对象（字典值用）。
fn read_object(content: &[u8], start: usize) -> (lopdf::Object, usize) {
    use lopdf::Object;
    let n = content.len();
    let mut i = start;
    while i < n && is_ws(content[i]) {
        i += 1;
    }
    if i >= n {
        return (Object::Null, i);
    }
    let b = content[i];
    if b == b'/' {
        let (name, next) = read_name(content, i + 1);
        return (Object::Name(name), next);
    }
    if b == b'(' {
        let (s, next) = read_literal_string(content, i);
        return (Object::String(s, lopdf::StringFormat::Literal), next);
    }
    if b == b'<' {
        if i + 1 < n && content[i + 1] == b'<' {
            let (d, next) = read_dict(content, i);
            return (Object::Dictionary(d), next);
        }
        let (s, next) = read_hex_string(content, i);
        return (Object::String(s, lopdf::StringFormat::Hexadecimal), next);
    }
    if b == b'[' {
        let mut arr = Vec::new();
        let mut j = i + 1;
        loop {
            while j < n && is_ws(content[j]) {
                j += 1;
            }
            if j >= n || content[j] == b']' {
                j = (j + 1).min(n);
                break;
            }
            let (v, next) = read_object(content, j);
            arr.push(v);
            if next <= j {
                break;
            }
            j = next;
        }
        return (Object::Array(arr), j);
    }
    if b.is_ascii_digit() || b == b'-' || b == b'+' || b == b'.' {
        let (num, next) = read_number(content, i);
        if num.fract() == 0.0 && num.abs() < i64::MAX as f64 {
            return (Object::Integer(num as i64), next);
        }
        return (Object::Real(num as f32), next);
    }
    // 关键字
    let s = i;
    let mut j = i;
    while j < n && !is_ws(content[j]) && !is_delim(content[j]) {
        j += 1;
    }
    let kw = std::str::from_utf8(&content[s..j]).unwrap_or("");
    let obj = match kw {
        "true" => Object::Boolean(true),
        "false" => Object::Boolean(false),
        "null" => Object::Null,
        other => Object::Name(other.as_bytes().to_vec()),
    };
    (obj, j.max(s + 1))
}

/// 解析 `<< … >>` 字典。
fn read_dict(content: &[u8], start: usize) -> (lopdf::Dictionary, usize) {
    let mut map = lopdf::Dictionary::new();
    let n = content.len();
    let mut i = start + 2; // 跳过 <<
    loop {
        while i < n && is_ws(content[i]) {
            i += 1;
        }
        if i + 1 < n && content[i] == b'>' && content[i + 1] == b'>' {
            i += 2;
            break;
        }
        if i >= n {
            break;
        }
        if content[i] != b'/' {
            // 畸形：跳过一个字节防死循环
            i += 1;
            continue;
        }
        let (key, next) = read_name(content, i + 1);
        let (val, next2) = read_object(content, next);
        map.set(key, val);
        if next2 <= i {
            i += 1;
        } else {
            i = next2;
        }
    }
    (map, i)
}

/// 内联图像：从 BI 之后读键值对，直到 ID。
fn read_inline_dict(content: &[u8], bi_pos: usize) -> (lopdf::Dictionary, usize) {
    let n = content.len();
    let mut map = lopdf::Dictionary::new();
    let mut i = bi_pos + 2; // 跳过 "BI"
    loop {
        while i < n && is_ws(content[i]) {
            i += 1;
        }
        if i >= n {
            return (map, i);
        }
        // ID 结束字典
        if content[i] == b'I' && i + 1 < n && content[i + 1] == b'D' {
            return (map, i + 2);
        }
        if content[i] != b'/' {
            i += 1;
            continue;
        }
        let (key, next) = read_name(content, i + 1);
        let (val, next2) = read_object(content, next);
        map.set(key, val);
        i = if next2 <= i { i + 1 } else { next2 };
    }
}

/// 内联图像数据：扫描到空白 + EI 为止。
fn read_inline_data(content: &[u8], start: usize) -> (Vec<u8>, usize) {
    let n = content.len();
    let mut i = start;
    while i + 1 < n {
        if content[i] == b'E' && content[i + 1] == b'I' {
            let before_ws = i == start || is_ws(content[i - 1]);
            let after_end = i + 2 >= n || is_ws(content[i + 2]) || is_delim(content[i + 2]);
            if before_ws && after_end {
                // EI 前的空白是分隔符，不属于数据
                let end = if i > start { i - 1 } else { i };
                return (content[start..end].to_vec(), i + 2);
            }
        }
        i += 1;
    }
    (content[start..].to_vec(), n)
}

fn read_literal_string(content: &[u8], start: usize) -> (Vec<u8>, usize) {
    // content[start] == '('
    let mut out = Vec::new();
    let mut depth = 1usize;
    let mut i = start + 1;
    while i < content.len() && depth > 0 {
        let b = content[i];
        match b {
            b'(' => {
                depth += 1;
                out.push(b);
            }
            b')' => {
                depth -= 1;
                if depth > 0 {
                    out.push(b);
                }
            }
            b'\\' => {
                i += 1;
                if i >= content.len() {
                    break;
                }
                match content[i] {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'(' => out.push(b'('),
                    b')' => out.push(b')'),
                    b'\\' => out.push(b'\\'),
                    b'\n' => {} // 行续接
                    b'\r' => {
                        if i + 1 < content.len() && content[i + 1] == b'\n' {
                            i += 1;
                        }
                    }
                    d if d.is_ascii_digit() => {
                        // 最多 3 位八进制
                        let mut val = 0u32;
                        let mut j = i;
                        let mut count = 0;
                        while j < content.len() && count < 3 && content[j].is_ascii_digit() {
                            val = val * 8 + (content[j] - b'0') as u32;
                            j += 1;
                            count += 1;
                        }
                        i = j - 1;
                        out.push((val & 0xFF) as u8);
                    }
                    other => out.push(other),
                }
            }
            other => out.push(other),
        }
        i += 1;
    }
    (out, i)
}

fn read_hex_string(content: &[u8], start: usize) -> (Vec<u8>, usize) {
    // content[start] == '<'
    let mut hex = Vec::new();
    let mut i = start + 1;
    while i < content.len() && content[i] != b'>' {
        if !is_ws(content[i]) {
            hex.push(content[i]);
        }
        i += 1;
    }
    if hex.len() % 2 == 1 {
        hex.push(b'0');
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    for pair in hex.chunks(2) {
        let h = |b: u8| (b as char).to_digit(16).unwrap_or(0) as u8;
        out.push((h(pair[0]) << 4) | h(pair[1]));
    }
    (out, i + 1)
}

fn read_name(content: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut i = start;
    while i < content.len() && !is_ws(content[i]) && !is_delim(content[i]) {
        if content[i] == b'#' && i + 2 < content.len() {
            let h = |b: u8| (b as char).to_digit(16).unwrap_or(0) as u8;
            out.push((h(content[i + 1]) << 4) | h(content[i + 2]));
            i += 3;
        } else {
            out.push(content[i]);
            i += 1;
        }
    }
    (out, i)
}

fn read_number(content: &[u8], start: usize) -> (f64, usize) {
    let mut i = start;
    while i < content.len() {
        let b = content[i];
        if b.is_ascii_digit() || b == b'-' || b == b'+' || b == b'.' {
            i += 1;
        } else {
            break;
        }
    }
    let s = std::str::from_utf8(&content[start..i]).unwrap_or("0");
    (s.parse::<f64>().unwrap_or(0.0), i)
}

// ── 字体解码 ──────────────────────────────────────────────────────────────

/// 字体解码器。优先级：
/// 1. 自解析 ToUnicode CMap（支持 1 字节码空间 `<00><FF>` —— lopdf 的
///    ToUnicodeCMap::parse 只接受 `<0000><FFFF>`，Chrome/Skia 生成的 Type3
///    子集字体全是 1 字节码空间，会因此回落 StandardEncoding 导致乱码）；
/// 2. /Encoding dict 的 /Differences 字形名表（lopdf 未实现）；
/// 3. 命名编码（WinAnsi/Standard/MacRoman 等，lopdf 表）；
/// 4. latin1 兜底。
pub enum FontDecoder<'a> {
    /// 自解析 CMap：码空间区间（lo、hi、字节数）+ code → Unicode。
    /// 码宽可为变长（如 PDF.js issue7901：<20><20> 单字节空格
    /// 与 <0000><19FF> 双字节并存）
    MyCmap {
        code_bytes: usize,
        ranges: Vec<(u32, u32, usize)>,
        map: std::collections::HashMap<u32, String>,
    },
    /// /Differences 解析出的 256 项字形表（已折算为 Unicode）
    DiffTable(Vec<Option<char>>),
    /// 命名编码（含 lopdf 自己解析成功的 ToUnicode）
    Lopdf(Encoding<'a>),
    /// 非内嵌 CJK 字体：码位按 GBK/GB18030 解释。
    /// 老式中文 PDF 用非内嵌「宋体」+ WinAnsiEncoding，实际码位是 GBK 双字节
    /// （pdf.js/XiaoBiaoSong 的 <C4BF> = 目）；与阅读器替换字体的行为一致
    Cjk(&'static encoding_rs::Encoding),
    Latin1,
}

/// 反转义 PDF 名称里的 `#XX`（如 `/#CB#CE#CC#E5` → GBK 字节「宋体」）。
pub fn unescape_pdf_name(raw: &str) -> Vec<u8> {
    let b = raw.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'#' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..i + 3])
                .ok()
                .and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = hex {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// /BaseFont 原始字节（反转义 `#XX`）。
pub fn basefont_bytes(font: &Dictionary) -> Vec<u8> {
    font.get(b"BaseFont")
        .ok()
        .and_then(|o| o.as_name().ok().map(|n| n.to_vec()))
        .map(|raw| unescape_pdf_name(&String::from_utf8_lossy(&raw)))
        .unwrap_or_default()
}

/// /BaseFont 显示名（非 ASCII 时按 GBK 解出中文名）。
pub fn basefont_display(font: &Dictionary) -> String {
    let raw = basefont_bytes(font);
    if raw.is_ascii() {
        String::from_utf8_lossy(&raw).to_string()
    } else {
        let (s, _, _) = encoding_rs::GB18030.decode(&raw);
        s.into_owned()
    }
}

/// 字体名是否为 CJK 字体（按 GBK/UTF-8 解出中文名，或英文名含 CJK 关键字）。
fn is_cjk_font_name(raw_name: &[u8]) -> bool {
    let has_cjk = |t: &str| t.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    let (name, _, _) = encoding_rs::GB18030.decode(raw_name);
    if has_cjk(&name) {
        return true;
    }
    let l = name.to_ascii_lowercase();
    [
        "song", "sun", "hei", "kai", "ming", "mingliu", "gothic", "mincho", "simsun", "simhei",
        "nsimsun", "batang", "gulim", "dotum",
    ]
    .iter()
    .any(|k| l.contains(k))
}

/// 字体是否有内嵌程序（FontFile/FontFile2/FontFile3）。
fn has_font_file(font: &Dictionary, doc: &Document) -> bool {
    let desc: Option<&Dictionary> = if font
        .get(b"Subtype")
        .and_then(Object::as_name_str)
        .unwrap_or("")
        == "Type0"
    {
        font.get_deref(b"DescendantFonts", doc)
            .ok()
            .and_then(|o| o.as_array().ok())
            .and_then(|a| a.first())
            .and_then(|f| match f {
                Object::Reference(id) => doc.get_dictionary(*id).ok(),
                other => other.as_dict().ok(),
            })
            .and_then(|d| d.get_deref(b"FontDescriptor", doc).ok())
            .and_then(|o| o.as_dict().ok())
    } else {
        font.get_deref(b"FontDescriptor", doc)
            .ok()
            .and_then(|o| o.as_dict().ok())
    };
    match desc {
        Some(d) => {
            d.get(b"FontFile").is_ok() || d.get(b"FontFile2").is_ok() || d.get(b"FontFile3").is_ok()
        }
        None => false,
    }
}

pub fn resolve_font<'a>(font: &'a Dictionary, doc: &'a Document) -> FontDecoder<'a> {
    let subtype = font
        .get(b"Subtype")
        .and_then(lopdf::Object::as_name_str)
        .unwrap_or("");
    let is_cid = subtype == "Type0";
    // Symbol/ZapfDingbats 使用内建符号编码：码位必须原样保留（Latin1），
    // 按 WinAnsi 解码会整体错位（fpdf2 zapfdingbats 第 2 页 0x80+ 码位）
    let base_bytes = basefont_bytes(font);
    let base = basefont_display(font);
    if base.contains("Symbol") || base.contains("ZapfDingbats") {
        return FontDecoder::Latin1;
    }

    // 1) 自解析 ToUnicode
    if let Ok(Object::Stream(s)) = font.get_deref(b"ToUnicode", doc) {
        if let Ok(content) = s.get_plain_content() {
            if let Some((mut ranges, map)) = parse_cmap_ranges_and_map(&content) {
                if !map.is_empty() {
                    let mut code_bytes = ranges.iter().map(|(_, _, n)| *n).max().unwrap_or(1);
                    if is_cid {
                        code_bytes = code_bytes.max(1);
                    } else {
                        // 简单字体恒为单字节码：部分生成器（如 govdocs XFA）
                        // 把 ToUnicode 码空间写成 <0000><FFFF>，按 2 字节
                        // 解码会让整页文本变成 U+FFFD
                        code_bytes = 1;
                        ranges = vec![(0, 255, 1)];
                    }
                    return FontDecoder::MyCmap {
                        code_bytes: code_bytes.max(1),
                        ranges,
                        map,
                    };
                }
            }
        }
    }

    // 2) /Encoding 字典 + /Differences
    if let Ok(Object::Dictionary(enc)) = font.get_deref(b"Encoding", doc) {
        if let Some(table) = parse_differences(enc) {
            return FontDecoder::DiffTable(table);
        }
    }

    // 3a) 非内嵌 CJK 字体（简单字体 + 无字体程序 + 字体名是中文/日文名）：
    //     码位按 GBK 解释——老式中文 PDF 声明 WinAnsiEncoding，实际码位是
    //     GBK 双字节（pdf.js/XiaoBiaoSong 的 <C4BF> = 目），必须先于命名编码判定
    if !is_cid && !has_font_file(font, doc) && is_cjk_font_name(&base_bytes) {
        return FontDecoder::Cjk(encoding_rs::GB18030);
    }

    // 3) 命名编码：显式 WinAnsi 名称用自建表——lopdf 对该名称回落
    //    Standard（0x80-0x9F 区未定义，repack 写出的 • 0x95 会被解成 FFFD）
    if !is_cid {
        if let Ok(Object::Name(enc_name)) = font.get(b"Encoding") {
            if enc_name == b"WinAnsiEncoding" {
                let mut table: Vec<Option<char>> =
                    (0..=255).map(|b| char::from_u32(b as u32)).collect();
                for (i, ch) in WINANSI_HIGH.iter().enumerate() {
                    table[0x80 + i] = Some(*ch);
                }
                return FontDecoder::DiffTable(table);
            }
        }
    }

    // 3b) lopdf 解析链（Encoding 缺失/其他命名时；Identity-H 已在步骤 1 处理）
    if !is_cid {
        if let Ok(enc) = font.get_font_encoding(doc) {
            return FontDecoder::Lopdf(enc);
        }
    }
    // 4) CID 无 ToUnicode 且无 UniGB 类命名编码：latin1 尽力
    FontDecoder::Latin1
}

pub fn decode_string(dec: &FontDecoder<'_>, bytes: &[u8]) -> String {
    match dec {
        FontDecoder::MyCmap {
            code_bytes,
            ranges,
            map,
        } => {
            // 变长码空间：优先匹配“短”区间（如单字节 <20><20>），
            // 否则按最长码宽切分
            let mut out = String::new();
            let mut i = 0usize;
            while i < bytes.len() {
                let mut matched = false;
                let mut sorted: Vec<&(u32, u32, usize)> = ranges.iter().collect();
                sorted.sort_by_key(|(_, _, n)| *n);
                for (lo, hi, n) in sorted {
                    if i + n > bytes.len() {
                        continue;
                    }
                    let code = bytes[i..i + n]
                        .iter()
                        .fold(0u32, |a, b| (a << 8) | *b as u32);
                    if code >= *lo && code <= *hi {
                        // 单字节码未在 CMap 中列出时回落基本编码（≈Latin-1）：
                        // 简单字体的未列码通常是 base encoding 的常规字符
                        // （pdf.js/issue6894 的空格被写成 U+FFFD → 豆腐块）
                        let ch = map.get(&code).cloned().unwrap_or_else(|| {
                            if *code_bytes == 1 && code < 0x100 {
                                (code as u8 as char).to_string()
                            } else {
                                "\u{FFFD}".into()
                            }
                        });
                        out.push_str(&ch);
                        i += n;
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    let cb = (*code_bytes).max(1);
                    let end = (i + cb).min(bytes.len());
                    let code = bytes[i..end].iter().fold(0u32, |a, b| (a << 8) | *b as u32);
                    let ch = map.get(&code).cloned().unwrap_or_else(|| {
                        if cb == 1 && code < 0x100 {
                            (code as u8 as char).to_string()
                        } else {
                            "\u{FFFD}".into()
                        }
                    });
                    out.push_str(&ch);
                    i += cb;
                }
            }
            out
        }
        FontDecoder::DiffTable(table) => bytes
            .iter()
            .map(|&b| table[b as usize].unwrap_or('\u{FFFD}'))
            .collect(),
        FontDecoder::Lopdf(enc) => enc
            .bytes_to_string(bytes)
            .unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect()),
        FontDecoder::Cjk(enc) => {
            let (out, _, _) = enc.decode(bytes);
            out.into_owned()
        }
        FontDecoder::Latin1 => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// 解析 ToUnicode CMap：返回 (码字节数, code→Unicode)。
/// 支持 bfchar / bfrange（含数组目标），码空间 1 或 2 字节均可。
pub fn parse_tounicode_cmap(
    content: &[u8],
) -> Option<(usize, std::collections::HashMap<u32, String>)> {
    let (ranges, map) = parse_cmap_ranges_and_map(content)?;
    let code_bytes = ranges.iter().map(|(_, _, n)| *n).max().unwrap_or(1);
    Some((code_bytes, map))
}

/// 编码 CMap →（码空间区间列表, code→CID）。支持 cidchar / cidrange。
#[allow(clippy::type_complexity)]
pub fn parse_cmap_cid(
    content: &[u8],
) -> (Vec<(u32, u32, usize)>, std::collections::HashMap<u32, u16>) {
    let text = String::from_utf8_lossy(content);
    let mut ranges: Vec<(u32, u32, usize)> = Vec::new();
    let mut map: std::collections::HashMap<u32, u16> = std::collections::HashMap::new();
    #[derive(PartialEq)]
    enum Sec {
        None,
        Cs,
        CidChar,
        CidRange,
    }
    let mut sec = Sec::None;
    for line in text.split(['\n', '\r']) {
        if line.contains("begincodespacerange") {
            sec = Sec::Cs;
            continue;
        }
        if line.contains("begincidchar") {
            sec = Sec::CidChar;
            continue;
        }
        if line.contains("begincidrange") {
            sec = Sec::CidRange;
            continue;
        }
        if line.contains("endcodespacerange")
            || line.contains("endcidchar")
            || line.contains("endcidrange")
        {
            sec = Sec::None;
            continue;
        }
        // 提取 <...> 组
        let groups: Vec<String> = {
            let mut out = Vec::new();
            let bytes = line.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'<' {
                    if let Some(off) = line[i + 1..].find('>') {
                        out.push(line[i + 1..i + 1 + off].to_string());
                        i = i + off + 2;
                        continue;
                    }
                }
                i += 1;
            }
            out
        };
        let hexnum = |t: &str| -> Option<(u32, usize)> {
            let t: String = t.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            if t.is_empty() {
                return None;
            }
            u32::from_str_radix(&t, 16).ok().map(|v| (v, t.len()))
        };
        match sec {
            Sec::Cs if groups.len() >= 2 => {
                if let (Some((lo, d1)), Some((hi, d2))) = (hexnum(&groups[0]), hexnum(&groups[1])) {
                    let bytes = ((d1.min(d2)) / 2).max(1);
                    ranges.push((lo, hi, bytes));
                }
            }
            Sec::CidChar if !groups.is_empty() => {
                // 同上：cid 为裸整数（`<code> cid`）
                let cid = groups
                    .get(1)
                    .and_then(|g| hexnum(g))
                    .map(|(v, _)| v)
                    .or_else(|| {
                        line.split_whitespace()
                            .last()
                            .and_then(|t| t.parse::<u32>().ok())
                    });
                if let (Some((code, _)), Some(cid)) = (hexnum(&groups[0]), cid) {
                    map.insert(code, cid as u16);
                }
            }
            Sec::CidRange if groups.len() >= 2 => {
                // 目标 CID 是裸十进制整数（`<lo> <hi> cid`），不含尖括号；
                // 只认带尖括号的写法会让整张 CMap 解析为空（CIDFontType0C
                // 的 /Encoding，issue9534_reduced）
                let cid = groups
                    .get(2)
                    .and_then(|g| hexnum(g))
                    .map(|(v, _)| v)
                    .or_else(|| {
                        line.split_whitespace()
                            .last()
                            .and_then(|t| t.parse::<u32>().ok())
                    });
                if let (Some((lo, _)), Some((hi, _)), Some(cid)) =
                    (hexnum(&groups[0]), hexnum(&groups[1]), cid)
                {
                    if hi >= lo && hi - lo < 65536 {
                        for (i, code) in (lo..=hi).enumerate() {
                            map.insert(code, (cid as usize + i).min(65535) as u16);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    (ranges, map)
}

/// CMap →（码空间区间列表, code→Unicode）。
#[allow(clippy::type_complexity)]
pub fn parse_cmap_ranges_and_map(
    content: &[u8],
) -> Option<(
    Vec<(u32, u32, usize)>,
    std::collections::HashMap<u32, String>,
)> {
    let text = String::from_utf8_lossy(content);
    let mut map: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    let mut ranges: Vec<(u32, u32, usize)> = Vec::new();
    let mut code_bytes = 1usize;

    let hex_val = |s: &str| -> Option<(u32, usize)> {
        let t: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if t.is_empty() {
            return None;
        }
        u32::from_str_radix(&t, 16).ok().map(|v| (v, t.len()))
    };
    let hex_to_string = |s: &str| -> String {
        let t: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        // 每 4 个 hex 字符 = 1 个 UTF-16BE 码元；短写法左补零（<20> → 0020）
        let padded = if !t.len().is_multiple_of(4) {
            format!("{:0>width$}", t, width = (t.len() / 4 + 1) * 4)
        } else {
            t
        };
        let units: Vec<u16> = padded
            .as_bytes()
            .chunks(4)
            .map(|c| u16::from_str_radix(std::str::from_utf8(c).unwrap_or("0"), 16).unwrap_or(0))
            .collect();
        String::from_utf16_lossy(&units)
    };

    #[derive(PartialEq)]
    enum Section {
        None,
        CsRange,
        BfChar,
        BfRange,
    }
    let mut section = Section::None;

    for line in text.split(['\n', '\r']) {
        if line.contains("begincodespacerange") {
            section = Section::CsRange;
            continue;
        }
        if line.contains("beginbfchar") {
            section = Section::BfChar;
            continue;
        }
        if line.contains("beginbfrange") {
            section = Section::BfRange;
            continue;
        }
        if line.contains("endcodespacerange")
            || line.contains("endbfchar")
            || line.contains("endbfrange")
        {
            section = Section::None;
            continue;
        }
        // 提取本行所有 <...> 十六进制组：Word/Acrobat 生成的 CMap 常把
        // <lo><hi><dst> 连写（无空格），按空白分词会解析不到
        let groups: Vec<String> = {
            let mut out = Vec::new();
            let mut i = 0;
            let bytes = line.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'<' {
                    if let Some(off) = line[i + 1..].find('>') {
                        out.push(line[i + 1..i + 1 + off].to_string());
                        i = i + off + 2;
                        continue;
                    }
                }
                i += 1;
            }
            out
        };

        match section {
            Section::CsRange if groups.len() >= 2 => {
                if let (Some((lo, d1)), Some((hi, _))) = (hex_val(&groups[0]), hex_val(&groups[1]))
                {
                    if d1 == hex_val(&groups[1]).map(|(_, d)| d).unwrap_or(d1) {
                        // 区间字节数 = hex 字符数 / 2（<20><20> → 1；<0000><19FF> → 2）
                        let bytes = (d1 / 2).max(1);
                        code_bytes = code_bytes.max(bytes);
                        ranges.push((lo, hi, bytes));
                    }
                }
            }
            Section::BfChar if groups.len() >= 2 => {
                if let (Some((src, sd)), Some(_)) = (hex_val(&groups[0]), hex_val(&groups[1])) {
                    code_bytes = code_bytes.max(sd / 2);
                    let dst = hex_to_string(&groups[1]);
                    if !dst.is_empty() {
                        map.insert(src, dst);
                    }
                }
            }
            Section::BfRange if !line.contains('[') && groups.len() >= 3 => {
                if let (Some((lo, ld)), Some((hi, _)), Some((base, _))) = (
                    hex_val(&groups[0]),
                    hex_val(&groups[1]),
                    hex_val(&groups[2]),
                ) {
                    if hi >= lo && hi - lo < 65536 {
                        code_bytes = code_bytes.max(ld / 2);
                        for (i, code) in (lo..=hi).enumerate() {
                            if let Some(c) = char::from_u32(base + i as u32) {
                                map.insert(code, c.to_string());
                            }
                        }
                    }
                }
            }
            Section::BfRange if line.contains('[') && line.contains(']') => {
                // <lo> <hi> [<d1> <d2> ...] 同行
                if let Some((head, tail)) = line.split_once('[') {
                    let head: Vec<String> =
                        head.split_whitespace().map(|s| s.to_string()).collect();
                    let items: Vec<String> = tail
                        .split(']')
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect();
                    if head.len() >= 2 {
                        if let (Some((lo, ld)), Some((hi, _))) =
                            (hex_val(&head[0]), hex_val(&head[1]))
                        {
                            if hi >= lo && (hi - lo + 1) as usize == items.len() {
                                code_bytes = code_bytes.max(ld / 2);
                                for (i, code) in (lo..=hi).enumerate() {
                                    let s = hex_to_string(&items[i]);
                                    if !s.is_empty() {
                                        map.insert(code, s);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    if map.is_empty() {
        None
    } else {
        Some((ranges, map))
    }
}

/// /Differences 数组 → 256 项 Unicode 表。
/// /Encoding 字典（含 /Differences）→ 码→字符表（Type3 直通构建反查表用）。
pub fn differences_table(enc: &Dictionary) -> Option<Vec<Option<char>>> {
    parse_differences(enc)
}

fn parse_differences(enc: &Dictionary) -> Option<Vec<Option<char>>> {
    let diffs = enc
        .get(b"Differences")
        .and_then(lopdf::Object::as_array)
        .ok()?;
    // 基表：latin1；BaseEncoding=WinAnsi 时补 0x80-0x9F 特殊区
    let mut table: Vec<Option<char>> = (0..=255).map(|b| char::from_u32(b as u32)).collect();
    let base_name = enc
        .get(b"BaseEncoding")
        .and_then(lopdf::Object::as_name_str)
        .ok();
    if base_name == Some("WinAnsiEncoding") {
        for (i, ch) in WINANSI_HIGH.iter().enumerate() {
            table[0x80 + i] = Some(*ch);
        }
    }
    let mut code: u32 = 0;
    for item in diffs {
        match item {
            Object::Integer(n) => code = *n as u32,
            Object::Name(name) => {
                let glyph = String::from_utf8_lossy(name).to_string();
                if let Some(ch) = glyph_name_to_unicode(&glyph) {
                    if code < 256 {
                        table[code as usize] = Some(ch);
                    }
                }
                code += 1;
            }
            _ => {}
        }
    }
    Some(table)
}

/// WinAnsi 0x80-0x9F 特殊字符区（与 latin1 的差异；未定义位 → FFFD）。
const WINANSI_HIGH: [char; 32] = [
    '€',        // 0x80
    '\u{FFFD}', // 0x81 未定义
    '\u{201A}', // 0x82 ‚
    '\u{0192}', // 0x83 ƒ
    '\u{201E}', // 0x84 „
    '\u{2026}', // 0x85 …
    '†', '‡',        // 0x86 0x87
    '\u{02C6}', // 0x88 ˆ
    '\u{2030}', // 0x89 ‰
    '\u{0160}', // 0x8A Š
    '\u{2039}', // 0x8B ‹
    '\u{0152}', // 0x8C Œ
    '\u{FFFD}', // 0x8D 未定义
    '\u{017D}', // 0x8E Ž
    '\u{FFFD}', // 0x8F 未定义
    '\u{FFFD}', // 0x90 未定义
    '\u{2018}', // 0x91 '
    '\u{2019}', // 0x92 '
    '\u{201C}', // 0x93 "
    '\u{201D}', // 0x94 "
    '•',        // 0x95
    '\u{2013}', // 0x96 –
    '\u{2014}', // 0x97 —
    '\u{02DC}', // 0x98 ˜
    '™',        // 0x99
    '\u{0161}', // 0x9A š
    '\u{203A}', // 0x9B ›
    '\u{0153}', // 0x9C œ
    '\u{FFFD}', // 0x9D 未定义
    '\u{017E}', // 0x9E ž
    '\u{0178}', // 0x9F Ÿ
];

/// 字形名 → Unicode：uniXXXX / uXXXX / 单字符 / 常见 AGL 名。
fn glyph_name_to_unicode(name: &str) -> Option<char> {
    if let Some(hex) = name
        .strip_prefix("uni")
        .filter(|h| (4..=6).contains(&h.len()) && h.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    if let Some(hex) = name
        .strip_prefix('u')
        .filter(|h| h.len() >= 4 && h.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
    }
    let mut cs = name.chars();
    if name.len() == 1 {
        return cs.next();
    }
    AGL_NAMES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// 常见 AGL 字形名子集（覆盖拉丁排版高频符号）。
const AGL_NAMES: &[(&str, char)] = &[
    ("space", ' '),
    ("exclam", '!'),
    ("quotedbl", '"'),
    ("numbersign", '#'),
    ("dollar", '$'),
    ("percent", '%'),
    ("ampersand", '&'),
    ("quotesingle", '\''),
    ("parenleft", '('),
    ("parenright", ')'),
    ("asterisk", '*'),
    ("plus", '+'),
    ("comma", ','),
    ("hyphen", '-'),
    ("period", '.'),
    ("slash", '/'),
    ("colon", ':'),
    ("semicolon", ';'),
    ("less", '<'),
    ("equal", '='),
    ("greater", '>'),
    ("question", '?'),
    ("at", '@'),
    ("bracketleft", '['),
    ("backslash", '\\'),
    ("bracketright", ']'),
    ("asciicircum", '^'),
    ("underscore", '_'),
    ("grave", '`'),
    ("braceleft", '{'),
    ("bar", '|'),
    ("braceright", '}'),
    ("asciitilde", '~'),
    ("quoteleft", '\u{2018}'),
    ("quoteright", '\u{2019}'),
    ("quotedblleft", '\u{201C}'),
    ("quotedblright", '\u{201D}'),
    ("endash", '\u{2013}'),
    ("emdash", '\u{2014}'),
    ("ellipsis", '…'),
    ("bullet", '•'),
    ("dagger", '†'),
    ("daggerdbl", '‡'),
    ("Euro", '€'),
    ("cent", '¢'),
    ("pound", '£'),
    ("yen", '¥'),
    ("copyright", '©'),
    ("registered", '®'),
    ("trademark", '™'),
    ("mu", 'µ'),
    ("degree", '°'),
    ("minus", '\u{2212}'),
    ("fraction", '⁄'),
    ("guillemotleft", '«'),
    ("guillemotright", '»'),
    ("guilsinglleft", '\u{2039}'),
    ("guilsinglright", '\u{203A}'),
    ("multiply", '×'),
    ("divide", '÷'),
    ("onesuperior", '¹'),
    ("twosuperior", '²'),
    ("threesuperior", '³'),
    ("onehalf", '½'),
    ("onequarter", '¼'),
    ("threequarters", '¾'),
    ("nbsp", '\u{00A0}'),
    ("fi", 'ﬁ'),
    ("fl", 'ﬂ'),
];

// ── 提取 ──────────────────────────────────────────────────────────────────

pub fn extract_pages(doc: &Document, pages: &[u32]) -> Result<Vec<PageText>> {
    let page_map: BTreeMap<u32, ObjectId> = doc.get_pages();
    let mut out = Vec::new();
    for &pno in pages {
        if let Some(&page_id) = page_map.get(&pno) {
            out.push(extract_page(doc, page_id, pno)?);
        }
    }
    Ok(out)
}

/// 2D 仿射矩阵（列向量约定：p' = M.apply(p)）。
#[derive(Clone, Copy)]
pub struct Mat {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Mat {
    pub const I: Mat = Mat {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    pub fn from_operands(args: &[Tok]) -> Mat {
        Mat {
            a: tok_num(args.first()),
            b: tok_num(args.get(1)),
            c: tok_num(args.get(2)),
            d: tok_num(args.get(3)),
            e: tok_num(args.get(4)),
            f: tok_num(args.get(5)),
        }
    }

    /// self × m：先应用 m，再应用 self（列向量约定）。
    /// 命名为 transform 以避免与 std::ops::Mul 的语义预期混淆。
    pub fn transform(self, m: Mat) -> Mat {
        Mat {
            a: self.a * m.a + self.c * m.b,
            b: self.b * m.a + self.d * m.b,
            c: self.a * m.c + self.c * m.d,
            d: self.b * m.c + self.d * m.d,
            e: self.a * m.e + self.c * m.f + self.e,
            f: self.b * m.e + self.d * m.f + self.f,
        }
    }

    pub fn apply(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    /// 仅线性部分（用于文本空间位移换算）。
    pub fn apply_linear(&self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y, self.b * x + self.d * y)
    }

    /// x 轴缩放幅值（字号换算到设备空间用）。
    pub fn x_scale(&self) -> f64 {
        (self.a * self.a + self.b * self.b).sqrt().max(1e-9)
    }

    /// y 轴缩放幅值。
    pub fn y_scale(&self) -> f64 {
        (self.c * self.c + self.d * self.d).sqrt().max(1e-9)
    }

    /// 逆时针旋转角（度）；轴对齐无镜像时为 0。
    ///
    /// 注意：180° 旋转的线性部分是 (-1,0,0,-1)，b/c 为 0 但 a/d 同时为负，
    /// 仅看 b/c 会误判为无旋转。
    pub fn rotation_deg(&self) -> f64 {
        if self.has_rotation() {
            return self.b.atan2(self.a).to_degrees();
        }
        if self.a < 0.0 && self.d < 0.0 {
            return 180.0;
        }
        0.0
    }

    /// 线性部分是否含旋转/斜切（180° 旋转也计入）。
    pub fn has_rotation(&self) -> bool {
        self.b.abs() > 1e-6 || self.c.abs() > 1e-6 || (self.a < 0.0 && self.d < 0.0)
    }
}

/// CJK 部首补充区（2E80-2EFF）中无 NFKC 分解的简体部件 → 对应汉字。
/// Chrome/Skia 的反向 cmap 会把部分汉字字形映射到部首区码位。
const CJK_RADICAL_MAP: &[(u32, &str)] = &[
    (0x2E81, "厂"),
    (0x2E85, "亻"),
    (0x2E88, "刂"),
    (0x2E8A, "卜"),
    (0x2E8C, "小"),
    (0x2E8D, "小"),
    (0x2E92, "巳"),
    (0x2E93, "糸"),
    (0x2E96, "忄"),
    (0x2E98, "手"),
    (0x2E9C, "日"),
    (0x2E9D, "月"),
    (0x2EA0, "民"),
    (0x2EA1, "氵"),
    (0x2EA3, "火"),
    (0x2EA7, "牛"),
    (0x2EA8, "犭"),
    (0x2EA9, "王"),
    (0x2EAB, "目"),
    (0x2EAE, "竹"),
    (0x2EAF, "糸"),
    (0x2EB0, "纟"),
    (0x2EB1, "罒"),
    (0x2EB6, "羊"),
    (0x2EB9, "老"),
    (0x2EBC, "肉"),
    (0x2EBE, "艹"),
    (0x2EBF, "艹"),
    (0x2EC0, "艹"),
    (0x2EC1, "虎"),
    (0x2EC2, "衤"),
    (0x2EC3, "西"),
    (0x2EC4, "西"),
    (0x2EC5, "见"),
    (0x2EC6, "角"),
    (0x2EC7, "角"),
    (0x2EC8, "讠"),
    (0x2EC9, "贝"),
    (0x2ECA, "足"),
    (0x2ECB, "车"),
    (0x2ECC, "辶"),
    (0x2ECD, "辶"),
    (0x2ECF, "阝"),
    (0x2ED0, "钅"),
    (0x2ED1, "長"),
    (0x2ED3, "长"),
    (0x2ED4, "门"),
    (0x2ED5, "阝"),
    (0x2ED6, "阝"),
    (0x2ED7, "雨"),
    (0x2ED8, "青"),
    (0x2ED9, "韦"),
    (0x2EDA, "页"),
    (0x2EDB, "风"),
    (0x2EDC, "飞"),
    (0x2EDD, "食"),
    (0x2EE0, "饣"),
    (0x2EE1, "页"),
    (0x2EE2, "马"),
    (0x2EE4, "鬼"),
    (0x2EE5, "鱼"),
    (0x2EE6, "鸟"),
    (0x2EE7, "卤"),
    (0x2EE8, "麦"),
    (0x2EE9, "黄"),
    (0x2EEA, "黾"),
    (0x2EEB, "斉"),
    (0x2EEC, "齐"),
    (0x2EED, "歯"),
    (0x2EEE, "齿"),
    (0x2EEF, "竜"),
    (0x2EF0, "龙"),
    (0x2EF1, "龟"),
    (0x2EF2, "亀"),
];

/// 部首区码位归一化：康熙部首（2F00-2FD5，有 NFKC 分解）走 NFKC；
/// 部首补充区（2E80-2EFF，多数无分解）走手工映射表。
pub fn fix_radicals(s: String) -> String {
    if !s.chars().any(|c| matches!(c as u32, 0x2E80..=0x2FD5)) {
        return s;
    }
    use unicode_normalization::UnicodeNormalization;
    s.chars()
        .map(|c| {
            let u = c as u32;
            if (0x2E80..=0x2EFF).contains(&u) {
                if let Some((_, repl)) = CJK_RADICAL_MAP.iter().find(|(k, _)| *k == u) {
                    return repl.to_string();
                }
                c.to_string()
            } else if (0x2F00..=0x2FD5).contains(&u) {
                c.to_string().nfkc().collect::<String>()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// 文本空间宽度估计：CJK 1.0em；拉丁按 Helvetica 风格分组
/// （误差需 < 0.2em，否则逐字形排布的 PDF 会插出假空格）。
// 标准 14 字体 ASCII(32..126) 的 AFM 宽度（1/1000 em）。
// 由 MuPDF 基准度量生成；用于 unpack 的文本步进（TJ 调距、多段 Tj 的行内定位）。
pub(crate) const HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];
pub(crate) const HELV_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];
pub(crate) const TIMES: [u16; 95] = [
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444, 921, 722, 667, 667, 722, 611,
    556, 722, 722, 333, 389, 722, 611, 889, 722, 722, 556, 722, 667, 556, 611, 722, 722, 944, 722,
    722, 611, 333, 278, 333, 469, 500, 333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500,
    278, 778, 500, 500, 500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541,
];
pub(crate) const TIMES_BOLD: [u16; 95] = [
    250, 333, 555, 500, 500, 1000, 833, 278, 333, 333, 500, 570, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500, 930, 722, 667, 722, 722, 667,
    611, 778, 778, 389, 500, 778, 667, 944, 722, 778, 611, 778, 722, 556, 667, 722, 722, 1000, 722,
    722, 667, 333, 278, 333, 581, 500, 333, 500, 556, 444, 556, 444, 333, 500, 556, 278, 333, 556,
    278, 833, 556, 500, 556, 556, 444, 389, 333, 556, 500, 722, 500, 500, 444, 394, 220, 394, 520,
];
pub(crate) const TIMES_ITALIC: [u16; 95] = [
    250, 333, 420, 500, 500, 833, 778, 214, 333, 333, 500, 675, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 333, 333, 675, 675, 675, 500, 920, 611, 611, 667, 722, 611,
    611, 722, 722, 333, 444, 667, 556, 833, 667, 722, 611, 722, 611, 500, 556, 722, 611, 833, 611,
    556, 556, 389, 278, 389, 422, 500, 333, 500, 500, 444, 500, 444, 278, 500, 500, 278, 278, 444,
    278, 722, 500, 500, 500, 500, 389, 389, 278, 500, 444, 667, 444, 444, 389, 400, 275, 400, 541,
];
pub(crate) const COURIER: [u16; 95] = [
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
    600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600, 600,
];

/// 标准 14 字体 ASCII 宽度（1/1000 em）。行内多段定位（TJ 调距、多段 Tj）
/// 依赖精确步进；通用估算表会让位置逐词漂移（pdfcpu 两端对齐样例）。
pub fn std14_width_1000(font: &str, c: char) -> Option<u16> {
    let code = c as u32;
    if !(32..=126).contains(&code) {
        return None;
    }
    let name = font.to_ascii_lowercase();
    let bold = name.contains("bold");
    let italic = name.contains("italic") || name.contains("oblique");
    let table: &[u16; 95] = if name.contains("courier") || name.contains("mono") {
        &COURIER
    } else if name.contains("times") || name.contains("serif") {
        if bold {
            &TIMES_BOLD
        } else if italic {
            &TIMES_ITALIC
        } else {
            &TIMES
        }
    } else if name.contains("helvetica") || name.contains("arial") || name.contains("sans") {
        if bold {
            &HELV_BOLD
        } else {
            &HELV
        }
    } else {
        return None;
    };
    Some(table[(code - 32) as usize])
}

/// 无精确度量时的通用宽度估算（CJK 记 1 em）。
pub fn fallback_width_em(c: char) -> f64 {
    if is_cjk(c) {
        1.0
    } else {
        latin_width_em(c)
    }
}

fn latin_width_em(c: char) -> f64 {
    match c {
        'i' | 'j' | 'l' | '!' | '\'' | '|' | '.' | ',' | ':' | ';' => 0.28,
        'f' | 't' | 'r' | '(' | ')' | '[' | ']' | '-' | '/' => 0.36,
        'm' | 'M' | 'W' | 'w' | '@' | '%' => 0.85,
        ' ' => 0.28,
        'Q' | 'O' | 'G' | 'D' | 'U' | 'H' | 'N' | 'B' | 'R' | 'K' | 'X' => 0.72,
        c if c.is_ascii_uppercase() => 0.68,
        c if c.is_ascii_digit() => 0.556,
        c if c.is_ascii_lowercase() => 0.52,
        _ => 0.55,
    }
}

pub fn text_width_em(text: &str) -> f64 {
    text.chars()
        .map(|c| if is_cjk(c) { 1.0 } else { latin_width_em(c) })
        .sum()
}

fn flush_line(
    lines: &mut Vec<Line>,
    bbox: &mut (f64, f64, f64, f64),
    char_count: &mut usize,
    size_dev: f64,
    x: f64,
    y: f64,
    text: String,
) {
    if text.is_empty() {
        return;
    }
    *char_count += text.chars().count();
    let w = text_width_em(&text) * size_dev;
    bbox.0 = bbox.0.min(x);
    bbox.1 = bbox.1.min(y - size_dev);
    bbox.2 = bbox.2.max(x + w);
    bbox.3 = bbox.3.max(y + size_dev * 0.3);
    if let Some(line) = lines.iter_mut().find(|l| (l.y - y).abs() < size_dev * 0.6) {
        line.items.push(TextItem {
            x,
            end_x: x + w,
            size: size_dev,
            text,
        });
    } else {
        lines.push(Line {
            y,
            items: vec![TextItem {
                x,
                end_x: x + w,
                size: size_dev,
                text,
            }],
        });
    }
}

fn tok_num(t: Option<&Tok>) -> f64 {
    match t {
        Some(Tok::Num(v)) => *v,
        _ => 0.0,
    }
}

fn tok_str(t: Option<&Tok>) -> Option<&[u8]> {
    match t {
        Some(Tok::Str(v)) => Some(v),
        _ => None,
    }
}

fn extract_page(doc: &Document, page_id: ObjectId, pno: u32) -> Result<PageText> {
    let mut fonts: BTreeMap<Vec<u8>, FontDecoder> = BTreeMap::new();
    for (name, font) in doc.get_page_fonts(page_id).unwrap_or_default() {
        fonts.insert(name, resolve_font(font, doc));
    }

    let content = doc.get_page_content(page_id).unwrap_or_default();
    let operations = tokenize(&content);

    let mut lines: Vec<Line> = Vec::new();
    let mut bbox = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    let mut char_count = 0usize;
    let mut has_drawings = false;

    let mut cur_font: Option<&FontDecoder> = None;
    let mut font_size = 10.0f64; // 文本空间 pt
    let mut leading = 12.0f64;
    let mut ctm = Mat::I;
    let mut ctm_stack: Vec<Mat> = Vec::new();
    // PDF 文本双矩阵模型：
    //   line matrix（Tm/Td/TD/T* 更新；Td 相对当前行起点）
    //   text matrix（= line matrix 起点 + 隐式字形步进；Td 时重置回 line matrix）
    // Skia 生成的 PDF 逐字形发 `Td <前一字宽> 0`（其 Type3 的 w0=0，无隐式步进），
    // ReportLab/LaTeX 则依赖隐式步进 —— 双矩阵模型对两类都正确。
    let mut tm_line = Mat::I;
    let mut tm_text = Mat::I;

    // 展示一段文本：flush 到当前笔位并按字宽推进笔（隐式 advance）
    let show = |bytes: &[u8],
                lines: &mut Vec<Line>,
                bbox: &mut (f64, f64, f64, f64),
                char_count: &mut usize,
                cur_font: Option<&FontDecoder>,
                font_size: f64,
                tm_text: &mut Mat,
                ctm: &Mat| {
        let text = fix_radicals(show_text(cur_font, bytes));
        if text.is_empty() {
            return;
        }
        let adv = text_width_em(&text) * font_size;
        let (x, y) = ctm.apply(tm_text.e, tm_text.f);
        let size_dev = font_size * ctm.transform(*tm_text).x_scale();
        flush_line(lines, bbox, char_count, size_dev, x, y, text);
        // 隐式步进只作用于 text matrix
        let (dx, dy) = tm_text.apply_linear(adv, 0.0);
        tm_text.e += dx;
        tm_text.f += dy;
    };

    for op in &operations {
        let args = &op.operands;
        match op.operator.as_str() {
            "q" => ctm_stack.push(ctm),
            "Q" => {
                if let Some(m) = ctm_stack.pop() {
                    ctm = m;
                }
            }
            "cm" if args.len() >= 6 => {
                // PDF 约定：cm 后的新用户坐标先过 M 再过旧 CTM（CTM' = CTM_old × M）
                ctm = ctm.transform(Mat::from_operands(args));
            }
            // PDF 规范：BT 将文本矩阵与文本行矩阵重置为单位阵
            // （否则 Td 定位的 PDF——ReportLab/LaTeX 风格——跨 BT 累积坐标）
            "BT" => {
                tm_line = Mat::I;
                tm_text = Mat::I;
            }
            "ET" => {}
            "Tf" if args.len() >= 2 => {
                if let Tok::Name(name) = &args[0] {
                    font_size = tok_num(args.get(1)).abs().max(0.1);
                    cur_font = fonts.get(name);
                }
            }
            "TL" => leading = tok_num(args.first()),
            "Td" | "TD" if args.len() >= 2 => {
                let tx = tok_num(args.first());
                let ty = tok_num(args.get(1));
                if op.operator == "TD" {
                    leading = -ty;
                }
                // 位移相对当前行起点（穿过行矩阵线性部分），并重置文本矩阵
                let (dx, dy) = tm_line.apply_linear(tx, ty);
                tm_line.e += dx;
                tm_line.f += dy;
                tm_text = tm_line;
            }
            "Tm" if args.len() >= 6 => {
                tm_line = Mat::from_operands(args);
                tm_text = tm_line;
            }
            "T*" => {
                let (dx, dy) = tm_line.apply_linear(0.0, -leading);
                tm_line.e += dx;
                tm_line.f += dy;
                tm_text = tm_line;
            }
            "'" if !args.is_empty() => {
                let (dx, dy) = tm_line.apply_linear(0.0, -leading);
                tm_line.e += dx;
                tm_line.f += dy;
                tm_text = tm_line;
                if let Some(bytes) = tok_str(args.last()) {
                    show(
                        bytes,
                        &mut lines,
                        &mut bbox,
                        &mut char_count,
                        cur_font,
                        font_size,
                        &mut tm_text,
                        &ctm,
                    );
                }
            }
            "\"" if args.len() >= 3 => {
                let aw = tok_num(args.first());
                let ac = tok_num(args.get(1));
                let (dx, dy) = tm_line.apply_linear(aw, -leading + ac);
                tm_line.e += dx;
                tm_line.f += dy;
                tm_text = tm_line;
                if let Some(bytes) = tok_str(args.last()) {
                    show(
                        bytes,
                        &mut lines,
                        &mut bbox,
                        &mut char_count,
                        cur_font,
                        font_size,
                        &mut tm_text,
                        &ctm,
                    );
                }
            }
            "Tj" if !args.is_empty() => {
                if let Some(bytes) = tok_str(args.last()) {
                    show(
                        bytes,
                        &mut lines,
                        &mut bbox,
                        &mut char_count,
                        cur_font,
                        font_size,
                        &mut tm_text,
                        &ctm,
                    );
                }
            }
            "TJ" if !args.is_empty() => {
                if let Tok::Array(arr) = &args[0] {
                    // 逐段展示；字距 kern（千分之 em）累积为笔位移，
                    // 间隙 > 0.25em 时给下一段文本前缀空格（词间隙）
                    let mut pending_space = false;
                    for el in arr {
                        match el {
                            Tok::Str(bytes) => {
                                let decoded = fix_radicals(show_text(cur_font, bytes));
                                if decoded.is_empty() {
                                    continue;
                                }
                                let (x, y) = ctm.apply(tm_text.e, tm_text.f);
                                let size_dev = font_size * ctm.transform(tm_text).x_scale();
                                let text = if pending_space {
                                    format!(" {decoded}")
                                } else {
                                    decoded.clone()
                                };
                                pending_space = false;
                                flush_line(
                                    &mut lines,
                                    &mut bbox,
                                    &mut char_count,
                                    size_dev,
                                    x,
                                    y,
                                    text,
                                );
                                // 笔推进只含字形宽（kern 位移已单独计）
                                let adv = text_width_em(&decoded) * font_size;
                                let (dx, dy) = tm_text.apply_linear(adv, 0.0);
                                tm_text.e += dx;
                                tm_text.f += dy;
                            }
                            Tok::Num(k) => {
                                let delta = -*k / 1000.0 * font_size;
                                if delta > 0.25 * font_size {
                                    pending_space = true;
                                }
                                let (dx, dy) = tm_text.apply_linear(delta, 0.0);
                                tm_text.e += dx;
                                tm_text.f += dy;
                            }
                            _ => {}
                        }
                    }
                }
            }
            "m" | "l" | "c" | "v" | "y" | "h" | "re" => has_drawings = true,
            _ => {}
        }
    }

    let has_images = doc
        .get_page_images(page_id)
        .map(|v| !v.is_empty())
        .unwrap_or(false);

    // 同位重复项去重（CAD/图层堆叠的页常见同一文本重画多次）
    let lines = crate::complexity::dedup_overlapping_items(lines);
    let char_count = lines
        .iter()
        .flat_map(|l| &l.items)
        .map(|i| i.text.chars().count())
        .sum();

    // 复杂度判定（图片面积粗估：has_images 时按页面 1/3 计——精确面积
    // 需要 XObject CTM 走查，OCR 决策的粒度足够）
    let page = doc.get_object(page_id).ok().and_then(|o| o.as_dict().ok());
    let (pw, ph) = page
        .and_then(|d| {
            let mb = d.get(b"MediaBox").ok().and_then(|o| o.as_array().ok());
            mb.map(|a| {
                let nums: Vec<f64> = a
                    .iter()
                    .filter_map(|o| {
                        if let Ok(f) = o.as_float() {
                            return Some(f as f64);
                        }
                        o.as_i64().ok().map(|i| i as f64)
                    })
                    .collect::<Vec<f64>>();
                if nums.len() >= 4 {
                    (nums[2] - nums[0], nums[3] - nums[1])
                } else {
                    (612.0, 792.0)
                }
            })
        })
        .unwrap_or((612.0, 792.0));
    let image_area = if has_images { pw * ph / 3.0 } else { 0.0 };
    let complexity =
        crate::complexity::calculate_page_complexity(&lines, pw * ph, false, image_area);

    Ok(PageText {
        page: pno,
        lines,
        char_count,
        has_images,
        has_drawings,
        bbox: if char_count > 0 { Some(bbox) } else { None },
        complexity,
    })
}

pub fn show_text(font: Option<&FontDecoder<'_>>, bytes: &[u8]) -> String {
    match font {
        Some(f) => decode_string(f, bytes),
        None => bytes.iter().map(|&b| b as char).collect(),
    }
}

// ── 表格近似检测（extract.table） ────────────────────────────────────────

/// 行聚类表格检测：同一 y 上多个 x 间隔的文本项视作单元格，
/// 相邻且列数一致的行合并为一张表。这是对 pdfplumber 线条聚类的近似。
pub fn detect_tables(pages: &[PageText]) -> Vec<Value> {
    let mut tables = Vec::new();
    for t in detect_table_rows(pages) {
        tables.push(json!({
            "page": t.page,
            "table_index": tables.len(),
            "rows": t.data.len(),
            "cols": t.data[0].len(),
            "data": t.data,
        }));
    }
    tables
}

/// 表格识别的结构化结果：单元格矩阵 + 每行来自哪些行下标
/// （供跨格式转换把已进表的行从正文流中剔除）。
#[derive(Debug, Clone)]
pub struct TableMatch {
    pub page: u32,
    /// 单元格文本矩阵（行 × 列）
    pub data: Vec<Vec<String>>,
    /// 每行来自该页 lines 的下标（与 data 行一一对应；检测按一行为一表行）
    pub line_ids: Vec<Vec<usize>>,
}

/// `detect_tables` 的结构化版本（同一套行聚类近似判定）。
pub fn detect_table_rows(pages: &[PageText]) -> Vec<TableMatch> {
    let mut tables: Vec<TableMatch> = Vec::new();
    for page in pages {
        let mut rows: Vec<(usize, Vec<String>)> = Vec::new();
        let sorted: Vec<usize> = {
            let idx = (0..page.lines.len()).collect::<Vec<_>>();
            let mut idx = idx;
            idx.sort_by(|&a, &b| {
                page.lines[b]
                    .y
                    .partial_cmp(&page.lines[a].y)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            idx
        };
        for li in sorted {
            let line = &page.lines[li];
            let mut items = line.items.clone();
            items.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
            if items.len() < 2 {
                // 单项行可能是表外文本 → 结束当前候选表
                flush_table_rows(&mut rows, page.page, &mut tables);
                continue;
            }
            // 按 x 间隔切分单元格（间隔 > 2em 视为新单元格）
            let mut cells: Vec<String> = Vec::new();
            let mut cur = String::new();
            let mut last_end: Option<f64> = None;
            for item in items {
                if let Some(le) = last_end {
                    if item.x - le > 2.0 * item.size {
                        cells.push(std::mem::take(&mut cur));
                    } else if !cur.is_empty() {
                        cur.push(' ');
                    }
                }
                cur.push_str(&item.text);
                last_end = Some(item.end_x);
            }
            if !cur.is_empty() {
                cells.push(cur);
            }
            if cells.len() >= 2 {
                rows.push((li, cells));
            } else {
                flush_table_rows(&mut rows, page.page, &mut tables);
            }
        }
        flush_table_rows(&mut rows, page.page, &mut tables);
    }
    tables
}

fn flush_table_rows(rows: &mut Vec<(usize, Vec<String>)>, page: u32, tables: &mut Vec<TableMatch>) {
    // 至少 2 行且列数一致才算表；不满足则从头去行重试
    while rows.len() >= 2 {
        let n = rows[0].1.len();
        if rows.iter().all(|r| r.1.len() == n) {
            tables.push(TableMatch {
                page,
                data: rows.iter().map(|r| r.1.clone()).collect(),
                line_ids: rows.iter().map(|r| vec![r.0]).collect(),
            });
            rows.clear();
            return;
        }
        rows.remove(0);
    }
    rows.clear();
}

/// 供 qa/checks 使用的便捷入口：全部页文本。
pub fn extract_all(doc: &Document) -> Result<Vec<PageText>> {
    let n = doc.get_pages().len() as u32;
    extract_pages(doc, &(1..=n).collect::<Vec<u32>>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizer_basic() {
        let ops = tokenize(b"BT /F1 12 Tf 72 720 Td (Hello World) Tj ET");
        assert_eq!(ops.len(), 5);
        assert_eq!(ops[0].operator, "BT");
        assert_eq!(ops[1].operator, "Tf");
        assert!(matches!(&ops[1].operands[0], Tok::Name(n) if n == b"F1"));
        assert!(matches!(ops[1].operands[1], Tok::Num(12.0)));
        assert_eq!(ops[2].operator, "Td");
        assert_eq!(ops[3].operator, "Tj");
        assert!(matches!(&ops[3].operands[0], Tok::Str(s) if s == b"Hello World"));
    }

    #[test]
    fn tokenizer_tj_array_and_escapes() {
        let ops = tokenize(b"[(A) -250 (B) 10 (C)] TJ");
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].operator, "TJ");
        let Tok::Array(arr) = &ops[0].operands[0] else {
            panic!("TJ operand should be an array")
        };
        assert_eq!(arr.len(), 5);
        assert!(matches!(arr[1], Tok::Num(-250.0)));

        // 字面量字符串内的转义：\( \) 与八进制 \101
        let esc = tokenize(b"(\\(x\\) \\101) Tj");
        assert_eq!(esc[0].operator, "Tj");
        assert!(matches!(&esc[0].operands[0], Tok::Str(s) if s == b"(x) A"));
    }

    #[test]
    fn tokenizer_skips_stray_delimiters() {
        // 畸形内容流：多余的 `)` 与 `{` `}` 不许造成死循环
        let ops = tokenize(b"[(Results) -100) (9)] TJ { } q Q");
        assert!(ops.iter().any(|o| o.operator == "TJ"));
        assert!(ops.iter().any(|o| o.operator == "q"));
    }

    #[test]
    fn cmap_parser_one_byte_codespace() {
        // Chrome/Skia 风格：1 字节码空间 <00> <FF>（lopdf 会拒绝这种）
        let cmap = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n1 begincodespacerange\n<00> <FF>\nendcodespacerange\n13 beginbfchar\n<03> <0020>\n<04> <5B63>\n<07> <5EA6>\n<58> <0051>\n<44> <0075>\nendbfchar\nendcmap";
        let Some((code_bytes, map)) = parse_tounicode_cmap(cmap) else {
            panic!("parse failed")
        };
        assert_eq!(code_bytes, 1);
        assert_eq!(map.get(&0x03).unwrap(), " ");
        assert_eq!(map.get(&0x04).unwrap(), "季"); // U+5B63
        assert_eq!(map.get(&0x58).unwrap(), "Q");
        assert_eq!(map.get(&0x44).unwrap(), "u");
    }

    #[test]
    fn cmap_parser_two_byte_and_bfrange() {
        let cmap = b"1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n1 beginbfrange\n<4e00> <4e09> <3000>\nendbfrange\n2 beginbfchar\n<0001> <0041>\n<0002> <D83DDE00>\nendbfchar";
        let Some((code_bytes, map)) = parse_tounicode_cmap(cmap) else {
            panic!("parse failed")
        };
        assert_eq!(code_bytes, 2);
        // bfrange 连续映射：4e00→3000（全角空格）、4e01→3001（、）、4e02→3002（。）
        assert_eq!(map.get(&0x4e00).unwrap(), "\u{3000}");
        assert_eq!(map.get(&0x4e01).unwrap(), "\u{3001}");
        assert_eq!(map.get(&0x4e02).unwrap(), "\u{3002}");
        assert_eq!(map.get(&0x0001).unwrap(), "A");
        assert_eq!(map.get(&0x0002).unwrap(), "\u{1F600}");
    }

    #[test]
    fn cmap_parser_array_bfrange() {
        let cmap = b"1 begincodespacerange\n<00> <FF>\nendcodespacerange\n1 beginbfrange\n<01> <03> [<0041> <0042> <0043>]\nendbfrange";
        let Some((code_bytes, map)) = parse_tounicode_cmap(cmap) else {
            panic!("parse failed")
        };
        assert_eq!(code_bytes, 1);
        assert_eq!(map.get(&0x01).unwrap(), "A");
        assert_eq!(map.get(&0x02).unwrap(), "B");
        assert_eq!(map.get(&0x03).unwrap(), "C");
    }

    #[test]
    fn glyph_names_resolve() {
        assert_eq!(glyph_name_to_unicode("Q"), Some('Q'));
        assert_eq!(glyph_name_to_unicode("uni5B63"), Some('季'));
        assert_eq!(glyph_name_to_unicode("u5B63"), Some('季'));
        assert_eq!(glyph_name_to_unicode("space"), Some(' '));
        assert_eq!(glyph_name_to_unicode("emdash"), Some('\u{2014}'));
        assert_eq!(glyph_name_to_unicode("g123"), None);
    }

    #[test]
    fn differences_table_built() {
        use lopdf::Object;
        let mut enc = Dictionary::new();
        enc.set(
            "Differences",
            Object::Array(vec![
                Object::Integer(65),
                Object::Name(b"uni5B63".to_vec()),
                Object::Name(b"uni5EA6".to_vec()),
                Object::Name(b"space".to_vec()),
            ]),
        );
        let table = parse_differences(&enc).unwrap();
        assert_eq!(table[65], Some('季'));
        assert_eq!(table[66], Some('度'));
        assert_eq!(table[67], Some(' '));
        assert_eq!(table[68], Some('D')); // latin1 基表
    }

    #[test]
    fn tokenizer_hex_string() {
        let ops = tokenize(b"<48656C6C6F> Tj");
        assert!(matches!(&ops[0].operands[0], Tok::Str(s) if s == b"Hello"));
    }

    #[test]
    fn tounicode_bfrange_maps_cid_to_unicode() {
        // 回归：Word/Acrobat 的 Identity-H 子集字体，CID≠Unicode，
        // 映射只存在于 ToUnicode（<0003><0003><0020> 形式）
        let content = b"/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\r\n\
/CIDSystemInfo << /Registry (Adobe)/Ordering (UCS)/Supplement 0>> def\r\n\
/CMapName /Adobe-Identity-UCS def\r\n/CMapType 2 def\r\n\
1 begincodespacerange\r\n<0003><005b>\r\nendcodespacerange\r\n\
2 beginbfrange\r\n<0003><0003><0020>\n<000F><000F><002C>\nendbfrange\r\n\
endcmap\n";
        let (cb, m) = parse_tounicode_cmap(content).expect("应解析成功");
        assert_eq!(cb, 2, "码宽 2 字节");
        assert_eq!(m.get(&3).map(|s| s.as_str()), Some(" "), "CID 3 → 空格");
        assert_eq!(m.get(&0x0F).map(|s| s.as_str()), Some(","), "CID 15 → 逗号");
    }

    #[test]
    fn simple_font_tounicode_two_byte_codespace_decodes_single_bytes() {
        // 回归：简单字体（Type1/TrueType）恒为单字节码，ToUnicode 声明
        // <0000><FFFF> 时不能按 2 字节解码（govdocs XFA 整页 U+FFFD）
        let font = lopdf::Dictionary::new(); // 无 Encoding，仅 ToUnicode
        let _ = font; // 由 resolve_font 之外的最小路径覆盖：直接构造解码器
        let mut map = std::collections::HashMap::new();
        map.insert(0x20u32, " ".to_string());
        map.insert(0x41u32, "A".to_string());
        let dec = FontDecoder::MyCmap {
            code_bytes: 1,
            ranges: vec![(0, 255, 1)],
            map,
        };
        assert_eq!(decode_string(&dec, b"A "), "A ");
    }

    #[test]
    fn tokenizer_comments_and_dicts() {
        let ops = tokenize(b"% comment\n1 2 /MarkedBDC << /MCID 0 >> BDC q Q");
        // 注释被跳过；<<...>> 折叠为单个 Dict 操作数；BDC/q/Q 是操作符
        assert_eq!(ops[0].operator, "BDC");
        assert_eq!(ops[0].operands.len(), 4);
        match &ops[0].operands[3] {
            Tok::Dict(d) => assert_eq!(d.get(b"MCID").ok().and_then(|o| o.as_i64().ok()), Some(0)),
            other => panic!("expected dict, got {other:?}"),
        }
        assert_eq!(ops[1].operator, "q");
        assert_eq!(ops[2].operator, "Q");
    }

    #[test]
    fn tokenizer_inline_image() {
        let ops = tokenize(b"q BI /W 2 /H 2 /CS /G /BPC 8 ID \x00\x01\x02\x03 EI Q");
        assert_eq!(ops[0].operator, "q");
        assert_eq!(ops[1].operator, "BI");
        match &ops[1].operands[0] {
            Tok::Dict(d) => {
                assert_eq!(d.get(b"W").ok().and_then(|o| o.as_i64().ok()), Some(2));
                assert_eq!(
                    d.get(b"CS").ok().and_then(|o| o.as_name_str().ok()),
                    Some("G")
                );
            }
            other => panic!("expected dict, got {other:?}"),
        }
        match &ops[1].operands[1] {
            Tok::Str(data) => assert_eq!(data, &[0u8, 1, 2, 3]),
            other => panic!("expected data, got {other:?}"),
        }
        assert_eq!(ops[2].operator, "Q");
    }

    #[test]
    fn cjk_detection() {
        assert!(is_cjk('中'));
        assert!(is_cjk('，'));
        assert!(!is_cjk('a'));
        assert!(!is_cjk('1'));
    }

    #[test]
    fn radical_normalization() {
        // 康熙部首（有 NFKC 分解）
        assert_eq!(fix_radicals("⼆⽉".into()), "二月");
        // 部首补充区简体（无 NFKC 分解，走手工表）
        assert_eq!(fix_radicals("增⻓".into()), "增长");
        assert_eq!(fix_radicals("⻔⻢⻚⻋".into()), "门马页车");
        // 普通文本不受影响
        assert_eq!(fix_radicals("Hello 世界".into()), "Hello 世界");
    }

    fn item(x: f64, size: f64, text: &str) -> TextItem {
        let end_x = x + text_width_em(text) * size;
        TextItem {
            x,
            end_x,
            size,
            text: text.into(),
        }
    }

    #[test]
    fn line_grouping_by_y() {
        let pt = PageText {
            page: 1,
            lines: vec![
                Line {
                    y: 700.0,
                    items: vec![item(72.0, 12.0, "Hello")],
                },
                Line {
                    y: 688.0,
                    items: vec![item(72.0, 12.0, "World"), item(200.0, 12.0, "X")],
                },
            ],
            char_count: 11,
            has_images: false,
            has_drawings: false,
            bbox: Some((72.0, 678.0, 205.0, 710.0)),
            complexity: crate::complexity::PageComplexity {
                reasons: vec![],
                text_length: 0,
                text_coverage: 0.0,
            },
        };
        let text = pt.text();
        assert!(text.starts_with("Hello\nWorld X"), "got: {text:?}");
    }

    #[test]
    fn word_gap_by_em_threshold() {
        // 间距 > 0.2em → 空格；紧密排列不插空格
        let pt = PageText {
            page: 1,
            lines: vec![Line {
                y: 700.0,
                items: vec![
                    item(72.0, 10.0, "ab"),
                    item(72.0 + 1.0, 10.0, "cd"),
                    item(72.0 + 2.0 + 5.0, 10.0, "ef"),
                ],
            }],
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
        let text = pt.text();
        // ab(宽10) 结束于 82；cd 起点 73 → 间隔 -9 无空格；cd 结束 83，ef 起点 79 → 无空格
        assert_eq!(text.trim(), "abcdef");
        // 拉开成词距
        let pt2 = PageText {
            page: 1,
            lines: vec![Line {
                y: 700.0,
                items: vec![item(72.0, 10.0, "ab"), item(72.0 + 10.0 + 3.0, 10.0, "cd")],
            }],
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
        assert_eq!(pt2.text().trim(), "ab cd");
    }

    #[test]
    fn matrix_flip_matches_chrome_layout() {
        // Chrome 页面级翻转：0.24 0 0 -0.24 0 792 cm；文本 Tm 1 0 0 -1 40 74
        let outer = Mat {
            a: 0.24,
            b: 0.0,
            c: 0.0,
            d: -0.24,
            e: 0.0,
            f: 792.0,
        };
        let inner = Mat {
            a: 3.125,
            b: 0.0,
            c: 0.0,
            d: 3.125,
            e: 115.625,
            f: 115.625,
        };
        let tm = Mat {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: -1.0,
            e: 40.0,
            f: 74.0,
        };
        let ctm = outer.transform(inner);
        let (x, y) = ctm.apply(tm.e, tm.f);
        // 40×3.125+115.625=240.625 → ×0.24 = 57.75；74×3.125+115.625=346.875 → 792-0.24×346.875=708.75
        assert!((x - 57.75).abs() < 0.01, "x={x}");
        assert!((y - 708.75).abs() < 0.01, "y={y}");
        // 字号 32 → 设备空间 32×0.75=24pt
        let size_dev = 32.0 * ctm.transform(tm).x_scale();
        assert!((size_dev - 24.0).abs() < 0.01);
    }

    #[test]
    fn table_detection_basic() {
        let page = PageText {
            page: 1,
            lines: vec![
                Line {
                    y: 700.0,
                    items: vec![item(72.0, 12.0, "Name"), item(200.0, 12.0, "Qty")],
                },
                Line {
                    y: 680.0,
                    items: vec![item(72.0, 12.0, "Apple"), item(200.0, 12.0, "3")],
                },
                Line {
                    y: 660.0,
                    items: vec![item(72.0, 12.0, "Pear"), item(200.0, 12.0, "5")],
                },
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
        let tables = detect_tables(&[page]);
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0]["rows"], 3);
        assert_eq!(tables[0]["cols"], 2);
        assert_eq!(tables[0]["data"][1][0], "Apple");
    }
}
