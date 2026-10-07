//! 图片提取 — 滤镜链解码 + Predictor 反预测 + PNG 重建 / 原码流直通。

use anyhow::Result;
use lopdf::Document;
use serde_json::{json, Value};
use std::path::Path;

/// 颜色空间规格（已解析，供像素重建）。
#[derive(Debug, Clone)]
pub enum CsSpec {
    Gray,
    Rgb,
    Cmyk,
    /// 索引色：调色板 + 基色分量数（1=gray / 3=rgb）
    Indexed(Vec<u8>, usize),
    /// CalRGB：线性 RGB + 可选 Gamma 与 Matrix（PDF 规范颜色空间）。
    /// 分量是线性值，显示前需经 XYZ → sRGB（否则明显偏暗）
    CalRgb {
        gamma: [f64; 3],
        matrix: [f64; 9],
        white: [f64; 3],
    },
    /// CalGray：线性灰度 + WhitePoint/Gamma，同样要经 XYZ → sRGB
    CalGray {
        gamma: f64,
        white: [f64; 3],
    },
    /// DeviceN/Separation：分量数 + tint transform（函数对象 + 码流）+ 备选空间。
    /// tint 由 unpack 侧求值（FunctionType 4 等），把 N 个分量映射到备选空间
    Tinted {
        n: usize,
        func: Box<lopdf::Object>,
        content: Vec<u8>,
        alt: Box<CsSpec>,
    },
}

/// DeviceN/Separation 的逐像素 tint transform（FunctionType 4）。
pub struct TintSpec {
    /// 输入分量数
    pub n: usize,
    /// PostScript 计算器码流
    pub code: Vec<u8>,
    /// 备选颜色空间
    pub alt: CsSpec,
}

/// 解码结果。
pub enum Decoded {
    /// 重建出的 PNG 字节
    Png(Vec<u8>),
    /// 无法解码 → 原码流直通（保留剩余滤镜链）
    Passthrough {
        ext: &'static str,
        bytes: Vec<u8>,
        /// 尚未应用的滤镜（从终止滤镜开始）
        filter: Vec<String>,
        /// 与 filter 对应的 DecodeParms
        decode_parms: Vec<Value>,
    },
}

/// 滤镜链是否包含终止型（无法进一步解码）滤镜。
fn is_terminal(f: &str) -> bool {
    matches!(
        f,
        "dctdecode" | "jpxdecode" | "jbig2decode" | "ccittfaxdecode"
    )
}

fn terminal_ext(f: &str) -> &'static str {
    match f {
        "dctdecode" => "jpg",
        "jpxdecode" => "jp2",
        _ => "bin",
    }
}

/// 解压/解码一段数据（不含终止型滤镜）。
fn apply_simple_filter(name: &str, data: &[u8]) -> Option<Vec<u8>> {
    match name {
        "flatedecode" => {
            use std::io::Read;
            let mut out = Vec::new();
            let mut d = flate2::read::ZlibDecoder::new(data);
            // 某些流是裸 deflate（无 zlib 头），失败后重试 raw deflate
            if d.read_to_end(&mut out).is_ok() {
                return Some(out);
            }
            let mut out = Vec::new();
            let mut d = flate2::read::DeflateDecoder::new(data);
            d.read_to_end(&mut out).ok()?;
            Some(out)
        }
        "lzwdecode" => lzw_decode(data),
        "asciihexdecode" => {
            let mut out = Vec::new();
            let mut hi: Option<u8> = None;
            for &b in data {
                let c = b as char;
                if c.is_ascii_hexdigit() {
                    let v = c.to_digit(16)? as u8;
                    match hi {
                        None => hi = Some(v),
                        Some(h) => {
                            out.push((h << 4) | v);
                            hi = None;
                        }
                    }
                } else if c == '>' {
                    break;
                }
            }
            if let Some(h) = hi {
                out.push(h << 4);
            }
            Some(out)
        }
        "ascii85decode" => ascii85_decode(data),
        "runlengthdecode" => {
            let mut out = Vec::new();
            let mut i = 0usize;
            while i < data.len() {
                let l = data[i];
                i += 1;
                if l == 128 {
                    break;
                } else if l < 128 {
                    let n = l as usize + 1;
                    if i + n > data.len() {
                        return None;
                    }
                    out.extend_from_slice(&data[i..i + n]);
                    i += n;
                } else {
                    if i >= data.len() {
                        return None;
                    }
                    let n = 257 - l as usize;
                    out.extend(std::iter::repeat_n(data[i], n));
                    i += 1;
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn ascii85_decode(data: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut group = [0u8; 5];
    let mut n = 0usize;
    let mut i = 0usize;
    // 跳过可能的 <~ 前缀
    if data.starts_with(b"<~") {
        i = 2;
    }
    while i < data.len() {
        let b = data[i];
        i += 1;
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'~' {
            break;
        }
        if b == b'z' && n == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if !(b'!'..=b'u').contains(&b) {
            return None;
        }
        group[n] = b - b'!';
        n += 1;
        if n == 5 {
            let mut v: u32 = 0;
            for &g in &group {
                v = v.wrapping_mul(85).wrapping_add(g as u32);
            }
            out.extend_from_slice(&v.to_be_bytes());
            n = 0;
        }
    }
    if n > 0 {
        for g in group.iter_mut().skip(n) {
            *g = 84;
        }
        let mut v: u32 = 0;
        for &g in &group {
            v = v.wrapping_mul(85).wrapping_add(g as u32);
        }
        let bytes = v.to_be_bytes();
        out.extend_from_slice(&bytes[..n - 1]);
    }
    Some(out)
}

/// LZW（PDF 变体：MSB-first，EarlyChange=1）。
fn lzw_decode(data: &[u8]) -> Option<Vec<u8>> {
    let mut dict: Vec<Vec<u8>> = (0..256).map(|i| vec![i as u8]).collect();
    dict.push(Vec::new()); // 256 = clear
    dict.push(Vec::new()); // 257 = EOD
    let mut out: Vec<u8> = Vec::new();
    let mut code_len = 9usize;
    let mut prev: Option<Vec<u8>> = None;
    let mut bitbuf: u32 = 0;
    let mut bits = 0usize;
    let mut i = 0usize;
    loop {
        while bits < code_len {
            if i >= data.len() {
                return Some(out);
            }
            bitbuf = (bitbuf << 8) | data[i] as u32;
            i += 1;
            bits += 8;
        }
        let code = ((bitbuf >> (bits - code_len)) & ((1 << code_len) - 1)) as usize;
        bits -= code_len;
        match code {
            256 => {
                dict.truncate(258);
                code_len = 9;
                prev = None;
                continue;
            }
            257 => return Some(out),
            _ => {}
        }
        let entry: Vec<u8> = if code < dict.len() {
            dict[code].clone()
        } else {
            // KwKwK 情形
            let mut e = prev.clone()?;
            e.push(prev.as_ref()?.first().copied()?);
            e
        };
        out.extend_from_slice(&entry);
        if let Some(p) = &prev {
            let mut ne = p.clone();
            ne.push(entry[0]);
            dict.push(ne);
        }
        prev = Some(entry);
        // EarlyChange = 1
        let n = dict.len();
        if n + 1 >= (1 << code_len) && code_len < 12 {
            code_len += 1;
        }
    }
}

/// PNG / TIFF Predictor 反预测。
pub fn unpredict(
    data: &[u8],
    predictor: i64,
    colors: usize,
    bpc: usize,
    columns: usize,
) -> Option<Vec<u8>> {
    if predictor <= 1 {
        return Some(data.to_vec());
    }
    let bpp = (colors * bpc).div_ceil(8).max(1);
    let row_len = (columns * colors * bpc).div_ceil(8);
    if predictor == 2 {
        // TIFF predictor（仅 8/16 bit）
        if bpc != 8 && bpc != 16 {
            return None;
        }
        let mut out = data.to_vec();
        let bytes_pp = (bpc / 8).max(1);
        let row_stride = row_len;
        let rows = out.chunks_mut(row_stride);
        for row in rows {
            for i in (colors * bytes_pp)..row.len() {
                row[i] = row[i].wrapping_add(row[i - colors * bytes_pp]);
            }
        }
        return Some(out);
    }
    // PNG predictor（10-15）：每行 1 字节滤波类型
    let stride = row_len + 1;
    if data.len() < stride {
        return None;
    }
    let rows = data.len() / stride;
    let mut out: Vec<u8> = Vec::with_capacity(rows * row_len);
    let mut prev_row = vec![0u8; row_len];
    for r in 0..rows {
        let off = r * stride;
        let ft = data[off];
        let row = &data[off + 1..off + 1 + row_len];
        let mut cur = vec![0u8; row_len];
        for i in 0..row_len {
            let raw = row[i];
            let left = if i >= bpp { cur[i - bpp] } else { 0 };
            let up = prev_row[i];
            let upleft = if i >= bpp { prev_row[i - bpp] } else { 0 };
            cur[i] = match ft {
                0 => raw,
                1 => raw.wrapping_add(left),
                2 => raw.wrapping_add(up),
                3 => raw.wrapping_add(((left as u16 + up as u16) / 2) as u8),
                4 => {
                    let p = left as i32 + up as i32 - upleft as i32;
                    let pa = (p - left as i32).abs();
                    let pb = (p - up as i32).abs();
                    let pc = (p - upleft as i32).abs();
                    let pred = if pa <= pb && pa <= pc {
                        left
                    } else if pb <= pc {
                        up
                    } else {
                        upleft
                    };
                    raw.wrapping_add(pred)
                }
                _ => return None,
            };
        }
        out.extend_from_slice(&cur);
        prev_row = cur;
    }
    Some(out)
}

/// 解析 DecodeParms（单个 dict 或数组）为 (Predictor, Colors, BPC, Columns)。
fn predictor_params(parms: &Value) -> (i64, usize, usize, usize) {
    let get = |k: &str, d: i64| -> i64 {
        parms
            .get(k)
            .and_then(|v| v.as_i64())
            .or_else(|| parms.get(k).and_then(|v| v.as_f64()).map(|f| f as i64))
            .unwrap_or(d)
    };
    (
        get("Predictor", 1),
        get("Colors", 1).max(1) as usize,
        get("BitsPerComponent", 8).max(1) as usize,
        get("Columns", 1).max(1) as usize,
    )
}

/// 采样位深 → 8bit 灰度/颜色数据（1/2/4/8/16）。
/// FunctionType 4（PostScript 计算器）：栈式求值（tint transform 等）。
/// 过程块 `{...}` 作为值压栈，由 if/ifelse 取用（PostScript 语义）。
pub fn eval_ps_calculator(code: &[u8], inputs: &[f64]) -> Option<Vec<f64>> {
    #[derive(Clone)]
    enum Tok {
        Num(f64),
        Op(String),
        Block(Vec<Tok>),
    }
    #[derive(Clone)]
    enum Val {
        Num(f64),
        Proc(Vec<Tok>),
    }
    fn parse(src: &[u8]) -> Vec<Tok> {
        let text = String::from_utf8_lossy(src);
        let mut stack: Vec<Vec<Tok>> = vec![Vec::new()];
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' => stack.push(Vec::new()),
                '}' => {
                    if stack.len() > 1 {
                        let blk = stack.pop().unwrap_or_default();
                        if let Some(top) = stack.last_mut() {
                            top.push(Tok::Block(blk));
                        }
                    }
                }
                c if c.is_whitespace() => {}
                _ => {
                    let mut t = c.to_string();
                    while let Some(&n) = chars.peek() {
                        if n.is_whitespace() || n == '{' || n == '}' {
                            break;
                        }
                        t.push(n);
                        chars.next();
                    }
                    if let Ok(v) = t.parse::<f64>() {
                        if let Some(top) = stack.last_mut() {
                            top.push(Tok::Num(v));
                        }
                    } else if let Some(top) = stack.last_mut() {
                        top.push(Tok::Op(t));
                    }
                }
            }
        }
        stack.pop().unwrap_or_default()
    }
    fn pop_num(st: &mut Vec<Val>) -> f64 {
        match st.pop() {
            Some(Val::Num(v)) => v,
            _ => 0.0,
        }
    }
    fn pop_proc(st: &mut Vec<Val>) -> Vec<Tok> {
        match st.pop() {
            Some(Val::Proc(b)) => b,
            _ => Vec::new(),
        }
    }
    fn exec(toks: &[Tok], st: &mut Vec<Val>, depth: usize) -> Option<()> {
        if depth > 16 {
            return None;
        }
        let mut i = 0usize;
        while i < toks.len() {
            match &toks[i] {
                Tok::Num(v) => st.push(Val::Num(*v)),
                Tok::Block(b) => st.push(Val::Proc(b.clone())),
                Tok::Op(op) => match op.as_str() {
                    "true" => st.push(Val::Num(1.0)),
                    "false" => st.push(Val::Num(0.0)),
                    "abs" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.abs()));
                    }
                    "neg" => {
                        let a = pop_num(st);
                        st.push(Val::Num(-a));
                    }
                    "ceiling" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.ceil()));
                    }
                    "floor" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.floor()));
                    }
                    "round" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.round()));
                    }
                    "truncate" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.trunc()));
                    }
                    "sqrt" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.max(0.0).sqrt()));
                    }
                    "sin" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.to_radians().sin()));
                    }
                    "cos" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.to_radians().cos()));
                    }
                    "atan" => {
                        // PostScript atan：返回 [0,360) 的角度（atan2 的 (-180,180] 会错）
                        let a = pop_num(st);
                        let b = pop_num(st);
                        let mut deg = b.atan2(a).to_degrees();
                        if deg < 0.0 {
                            deg += 360.0;
                        }
                        st.push(Val::Num(deg));
                    }
                    "exp" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(b.powf(a)));
                    }
                    "ln" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.max(1e-12).ln()));
                    }
                    "log" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.max(1e-12).log10()));
                    }
                    "add" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(b + a));
                    }
                    "sub" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(b - a));
                    }
                    "mul" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(b * a));
                    }
                    "div" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(if a == 0.0 { 0.0 } else { b / a }));
                    }
                    "idiv" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(if a == 0.0 { 0.0 } else { (b / a).trunc() }));
                    }
                    "mod" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(if a == 0.0 { 0.0 } else { b % a }));
                    }
                    "dup" => {
                        if let Some(v) = st.last().cloned() {
                            st.push(v);
                        }
                    }
                    "exch" => {
                        let a = st.pop().unwrap_or(Val::Num(0.0));
                        let b = st.pop().unwrap_or(Val::Num(0.0));
                        st.push(a);
                        st.push(b);
                    }
                    "pop" => {
                        st.pop();
                    }
                    "index" => {
                        let n = pop_num(st).max(0.0) as usize;
                        let len = st.len();
                        if len > 0 {
                            let v = st[len.saturating_sub(1 + n).min(len - 1)].clone();
                            st.push(v);
                        }
                    }
                    "copy" => {
                        let n = pop_num(st).max(0.0) as usize;
                        let len = st.len();
                        let start = len.saturating_sub(n);
                        let items: Vec<Val> = st[start..].to_vec();
                        st.extend(items);
                    }
                    "roll" => {
                        // PostScript roll 的 j 可为负（`5 -1 roll` 常见于
                        // DeviceN tint：把栈顶元素下移）；取 max(0) 会退化成不动
                        // （pdf.js/issue13520 的 DeviceN 渐变整片偏色）
                        let j = pop_num(st);
                        let n = pop_num(st).max(0.0) as usize;
                        let len = st.len();
                        if n > 0 && n <= len {
                            let m = ((j as i64 % n as i64) + n as i64) as usize % n;
                            if m != 0 {
                                st[len - n..].rotate_right(m);
                            }
                        }
                    }
                    "eq" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(if (b - a).abs() < 1e-12 { 1.0 } else { 0.0 }));
                    }
                    "ne" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(if (b - a).abs() >= 1e-12 { 1.0 } else { 0.0 }));
                    }
                    "gt" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(if b > a { 1.0 } else { 0.0 }));
                    }
                    "ge" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(if b >= a { 1.0 } else { 0.0 }));
                    }
                    "lt" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(if b < a { 1.0 } else { 0.0 }));
                    }
                    "le" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(if b <= a { 1.0 } else { 0.0 }));
                    }
                    "and" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(((b as i64) & (a as i64)) as f64));
                    }
                    "or" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(((b as i64) | (a as i64)) as f64));
                    }
                    "xor" => {
                        let a = pop_num(st);
                        let b = pop_num(st);
                        st.push(Val::Num(((b as i64) ^ (a as i64)) as f64));
                    }
                    "not" => {
                        let a = pop_num(st);
                        st.push(Val::Num(if a == 0.0 { 1.0 } else { 0.0 }));
                    }
                    "bitshift" => {
                        let a = pop_num(st) as i32;
                        let v = pop_num(st) as i64;
                        st.push(Val::Num(if a >= 0 {
                            (v << a.min(62)) as f64
                        } else {
                            (v >> (-a).min(62)) as f64
                        }));
                    }
                    "if" => {
                        // 栈序：bool proc if
                        let proc = pop_proc(st);
                        let cond = pop_num(st);
                        if cond != 0.0 {
                            exec(&proc, st, depth + 1)?;
                        }
                    }
                    "ifelse" => {
                        // 栈序：bool proc1 proc2 ifelse
                        let p2 = pop_proc(st);
                        let p1 = pop_proc(st);
                        let cond = pop_num(st);
                        exec(if cond != 0.0 { &p1 } else { &p2 }, st, depth + 1)?;
                    }
                    "cvi" => {
                        let a = pop_num(st);
                        st.push(Val::Num(a.trunc()));
                    }
                    "cvr" => {}
                    _ => return None,
                },
            }
            i += 1;
        }
        Some(())
    }
    let mut toks = parse(code);
    // 常见的写法是把整个程序包在一对 {} 里：此时顶层过程体就是要执行的代码
    if toks.len() == 1 {
        if let Tok::Block(inner) = &toks[0] {
            toks = inner.clone();
        }
    }
    let mut st: Vec<Val> = inputs.iter().map(|v| Val::Num(*v)).collect();
    exec(&toks, &mut st, 0)?;
    Some(
        st.into_iter()
            .filter_map(|v| match v {
                Val::Num(n) => Some(n),
                Val::Proc(_) => None,
            })
            .collect(),
    )
}

/// tint 输出 → RGB（按备选空间）。
fn rgb_from_values(comps: &[f64], alt: &CsSpec) -> (u8, u8, u8) {
    let to_u8 = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    match alt {
        CsSpec::Gray => {
            let v = to_u8(*comps.first().unwrap_or(&0.0));
            (v, v, v)
        }
        CsSpec::Cmyk => {
            let c = comps.first().copied().unwrap_or(0.0);
            let m = comps.get(1).copied().unwrap_or(0.0);
            let y = comps.get(2).copied().unwrap_or(0.0);
            let k = comps.get(3).copied().unwrap_or(0.0);
            (
                to_u8((1.0 - c) * (1.0 - k)),
                to_u8((1.0 - m) * (1.0 - k)),
                to_u8((1.0 - y) * (1.0 - k)),
            )
        }
        CsSpec::CalGray { gamma, white } => {
            let v = comps.first().copied().unwrap_or(0.0);
            let (r, g, b) = calgray_to_srgb(v, *gamma, white);
            (to_u8(r), to_u8(g), to_u8(b))
        }
        CsSpec::CalRgb {
            gamma,
            matrix,
            white,
        } => {
            let (r, g, b) = calrgb_to_srgb(
                comps.first().copied().unwrap_or(0.0),
                comps.get(1).copied().unwrap_or(0.0),
                comps.get(2).copied().unwrap_or(0.0),
                gamma,
                matrix,
                white,
            );
            (to_u8(r), to_u8(g), to_u8(b))
        }
        _ => (
            to_u8(comps.first().copied().unwrap_or(0.0)),
            to_u8(comps.get(1).copied().unwrap_or(0.0)),
            to_u8(comps.get(2).copied().unwrap_or(0.0)),
        ),
    }
}

/// CalGray 线性灰度 → sRGB（0-1）：按 WhitePoint 展开到 XYZ，再走
/// CalRGB 的 XYZ→sRGB 通路（白点折算到 D65）。
pub fn calgray_to_srgb(v: f64, gamma: f64, white: &[f64; 3]) -> (f64, f64, f64) {
    let a = v.max(0.0).powf(gamma.max(1e-6));
    let (x, y, z) = (white[0] * a, white[1] * a, white[2] * a);
    let (sw, sy, sz) = (
        if white[0] > 1e-6 {
            0.9505 / white[0]
        } else {
            1.0
        },
        if white[1] > 1e-6 { 1.0 / white[1] } else { 1.0 },
        if white[2] > 1e-6 {
            1.089 / white[2]
        } else {
            1.0
        },
    );
    const M: [[f64; 3]; 3] = [
        [3.2406, -1.5372, -0.4986],
        [-0.9689, 1.8758, 0.0415],
        [0.0557, -0.2040, 1.0570],
    ];
    let (x, y, z) = (x * sw, y * sy, z * sz);
    let enc = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    (
        enc(M[0][0] * x + M[0][1] * y + M[0][2] * z),
        enc(M[1][0] * x + M[1][1] * y + M[1][2] * z),
        enc(M[2][0] * x + M[2][1] * y + M[2][2] * z),
    )
}

/// CalRGB 线性分量 → sRGB（0-1）。
/// 1) 逐分量 Gamma（默认 1 = 线性）；2) /Matrix → XYZ（D50/D65 自适应略）；
/// 3) 标准 sRGB 逆矩阵 → 线性 sRGB；4) sRGB 传输函数编码。
pub fn calrgb_to_srgb(
    r: f64,
    g: f64,
    b: f64,
    gamma: &[f64; 3],
    matrix: &[f64; 9],
    white: &[f64; 3],
) -> (f64, f64, f64) {
    let lg = |v: f64, gm: f64| v.max(0.0).powf(gm.max(1e-6));
    let (r, g, b) = (lg(r, gamma[0]), lg(g, gamma[1]), lg(b, gamma[2]));
    // CalRGB /Matrix 是 [Xa Ya Za Xb Yb Zb Xc Yc Zc]（列主序：RGB → XYZ）
    let x = matrix[0] * r + matrix[3] * g + matrix[6] * b;
    let y = matrix[1] * r + matrix[4] * g + matrix[7] * b;
    let z = matrix[2] * r + matrix[5] * g + matrix[8] * b;
    // Matrix 输出的 XYZ 相对于声明的 WhitePoint；折算到 D65 再做 XYZ→sRGB
    let (sw, sy, sz) = (
        if white[0] > 1e-6 {
            0.9505 / white[0]
        } else {
            1.0
        },
        if white[1] > 1e-6 { 1.0 / white[1] } else { 1.0 },
        if white[2] > 1e-6 {
            1.089 / white[2]
        } else {
            1.0
        },
    );
    let (x, y, z) = (x * sw, y * sy, z * sz);
    // XYZ → 线性 sRGB
    const M: [[f64; 3]; 3] = [
        [3.2406, -1.5372, -0.4986],
        [-0.9689, 1.8758, 0.0415],
        [0.0557, -0.2040, 1.0570],
    ];
    let lr = M[0][0] * x + M[0][1] * y + M[0][2] * z;
    let lg2 = M[1][0] * x + M[1][1] * y + M[1][2] * z;
    let lb = M[2][0] * x + M[2][1] * y + M[2][2] * z;
    // sRGB 传输函数（线性 → 编码）
    let enc = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    (enc(lr), enc(lg2), enc(lb))
}

/// 按行展开 bpc 位样本（PDF 图像每行按字节对齐）。
/// `comps` = 每像素分量数；`scale` = 是否缩放到 0-255（索引色需保留原始索引）。
/// 连续位流在行宽非 8 的倍数时会整体错位（1bpc/115px 的图即如此）。
fn expand_bits_rows(
    data: &[u8],
    bpc: usize,
    width: usize,
    height: usize,
    comps: usize,
    scale: bool,
) -> Vec<u8> {
    if bpc == 8 {
        return data.to_vec();
    }
    if bpc == 16 {
        return data.iter().step_by(2).copied().collect();
    }
    let mut out = Vec::with_capacity(width * height * comps);
    let max = (1u16 << bpc) - 1;
    let row_bytes = (width * comps * bpc).div_ceil(8);
    for y in 0..height {
        let base = y * row_bytes;
        for x in 0..(width * comps) {
            let bit = x * bpc;
            let byte = data.get(base + bit / 8).copied().unwrap_or(0);
            let shift = 8 - bpc - (bit % 8);
            let v = (byte >> shift) & (max as u8);
            out.push(if scale {
                ((v as u16 * 255) / max) as u8
            } else {
                v
            });
        }
    }
    out
}

#[cfg(test)]
fn expand_bits(data: &[u8], bpc: usize, n_samples: usize) -> Vec<u8> {
    match bpc {
        8 => data.to_vec(),
        16 => data.iter().step_by(2).copied().collect(),
        1 | 2 | 4 => {
            let mut out = Vec::with_capacity(n_samples);
            let max = (1u16 << bpc) - 1;
            let mut bit = 0usize;
            for _ in 0..n_samples {
                let byte = data.get(bit / 8).copied().unwrap_or(0);
                let shift = 8 - bpc - (bit % 8);
                let v = (byte >> shift) & (max as u8);
                out.push(((v as u16 * 255) / max) as u8);
                bit += bpc;
            }
            out
        }
        _ => Vec::new(),
    }
}

/// 像素 → PNG 字节（Gray/RGB/RGBA）。
fn png_from_pixels(w: i64, h: i64, comps: usize, pixels: &[u8]) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, w as u32, h as u32);
        enc.set_color(match comps {
            1 => png::ColorType::Grayscale,
            2 => png::ColorType::GrayscaleAlpha,
            4 => png::ColorType::Rgba,
            _ => png::ColorType::Rgb,
        });
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().ok()?;
        writer.write_image_data(pixels).ok()?;
    }
    Some(buf)
}

/// 完整解码一张图片流（滤镜链 + Predictor + 颜色空间）。
///
/// `filter_names` 为小写 PDF 滤镜名；`parms` 与之一一对应。
#[allow(clippy::too_many_arguments)]
pub mod preview;

#[allow(clippy::too_many_arguments)]
pub fn decode_image(
    raw: &[u8],
    filter_names: &[String],
    parms: &[Value],
    width: i64,
    height: i64,
    bpc: i64,
    cs: Option<&CsSpec>,
    image_mask: bool,
    mask_fill: Option<(u8, u8, u8)>,
    decode_arr: Option<&[f64]>,
    // 颜色键遮罩（/Mask 数组，min/max 逐分量；命中即透明）
    color_key: Option<&[f64]>,
    // DeviceN/Separation 的逐像素 tint（FunctionType 4）
    tint: Option<&TintSpec>,
) -> Decoded {
    let mut data = raw.to_vec();
    let mut idx = 0usize;
    while idx < filter_names.len() {
        let f = filter_names[idx].to_lowercase();
        if is_terminal(&f) {
            // 终止型滤镜：原码流直通（保留本滤镜及之后的链）
            return Decoded::Passthrough {
                ext: terminal_ext(&f),
                bytes: data,
                filter: filter_names[idx..].to_vec(),
                decode_parms: parms.get(idx..).unwrap_or(&[]).to_vec(),
            };
        }
        let Some(mut out) = apply_simple_filter(&f, &data) else {
            // 未知滤镜 → 直通
            return Decoded::Passthrough {
                ext: "bin",
                bytes: data,
                filter: filter_names[idx..].to_vec(),
                decode_parms: parms.get(idx..).unwrap_or(&[]).to_vec(),
            };
        };
        // Predictor（Flate/LZW 后生效）
        if let Some(p) = parms.get(idx) {
            let (pred, colors, pbpc, columns) = predictor_params(p);
            if pred > 1 {
                match unpredict(&out, pred, colors, pbpc, columns) {
                    Some(u) => out = u,
                    None => {
                        return Decoded::Passthrough {
                            ext: "bin",
                            bytes: data,
                            filter: filter_names[idx..].to_vec(),
                            decode_parms: parms.get(idx..).unwrap_or(&[]).to_vec(),
                        }
                    }
                }
            }
        }
        data = out;
        idx += 1;
    }

    // 全部滤镜已解码 → 按颜色空间重建 PNG
    if width <= 0 || height <= 0 {
        return Decoded::Passthrough {
            ext: "bin",
            bytes: data,
            filter: Vec::new(),
            decode_parms: Vec::new(),
        };
    }
    let n_pixels = (width * height) as usize;

    if image_mask {
        // 1bit 掩膜：0 = 绘制（填充色），1 = 透明
        let bits = expand_bits_rows(
            &data,
            bpc.max(1) as usize,
            width as usize,
            height as usize,
            1,
            true,
        );
        let (r, g, b) = mask_fill.unwrap_or((0, 0, 0));
        let invert = decode_arr
            .map(|d| d.first().copied().unwrap_or(0.0) > d.get(1).copied().unwrap_or(1.0))
            .unwrap_or(false);
        let mut rgba = Vec::with_capacity(n_pixels * 4);
        for v in bits {
            let alpha = if invert { v } else { 255 - v };
            rgba.extend_from_slice(&[r, g, b, alpha]);
        }
        if let Some(png) = png_from_pixels(width, height, 4, &rgba) {
            return Decoded::Png(png);
        }
        return Decoded::Passthrough {
            ext: "bin",
            bytes: data,
            filter: Vec::new(),
            decode_parms: Vec::new(),
        };
    }

    let Some(cs) = cs else {
        return Decoded::Passthrough {
            ext: "bin",
            bytes: data,
            filter: Vec::new(),
            decode_parms: Vec::new(),
        };
    };
    let comps = match cs {
        CsSpec::Gray => 1,
        CsSpec::Rgb => 3,
        CsSpec::Cmyk => 4,
        CsSpec::CalRgb { .. } => 3,
        CsSpec::CalGray { .. } => 1,
        // DeviceN/Separation 图像（罕见）：按分量的备选空间近似
        CsSpec::Tinted { alt, .. } => match alt.as_ref() {
            CsSpec::Gray => 1,
            CsSpec::Cmyk => 4,
            _ => 3,
        },
        CsSpec::Indexed(pal, base) => {
            let base_comps = *base;
            let idx_bits = bpc.max(1) as usize;
            // 原始索引（不缩放，按行对齐）
            let indices =
                expand_bits_rows(&data, idx_bits, width as usize, height as usize, 1, false);
            let mut rgb = Vec::with_capacity(n_pixels * 3);
            for &i in &indices {
                let pos = (i as usize) * base_comps;
                let (r, g, b) = match base_comps {
                    1 => {
                        let v = pal.get(i as usize).copied().unwrap_or(0);
                        (v, v, v)
                    }
                    3 => (
                        pal.get(pos).copied().unwrap_or(0),
                        pal.get(pos + 1).copied().unwrap_or(0),
                        pal.get(pos + 2).copied().unwrap_or(0),
                    ),
                    _ => (0, 0, 0),
                };
                rgb.extend_from_slice(&[r, g, b]);
            }
            pos_unused(pal);
            // 颜色键遮罩在 Indexed 分支同样生效（pdfium/bug_343075986 即 Indexed）
            if let Some(key) = color_key {
                if key.len() >= 6 {
                    let mut rgba = Vec::with_capacity(n_pixels * 4);
                    for px in rgb.chunks_exact(3) {
                        let hit = (0..3).all(|i| {
                            let lo = key[i * 2] * 255.0;
                            let hi = key[i * 2 + 1] * 255.0;
                            let v = px[i] as f64;
                            v >= lo - 0.5 && v <= hi + 0.5
                        });
                        rgba.extend_from_slice(&[px[0], px[1], px[2], if hit { 0 } else { 255 }]);
                    }
                    if let Some(png) = png_from_pixels(width, height, 4, &rgba) {
                        return Decoded::Png(png);
                    }
                }
            }
            return match png_from_pixels(width, height, 3, &rgb) {
                Some(png) => Decoded::Png(png),
                None => Decoded::Passthrough {
                    ext: "bin",
                    bytes: data,
                    filter: Vec::new(),
                    decode_parms: Vec::new(),
                },
            };
        }
    };
    // DeviceN/Separation：先逐像素 tint 到备选空间，再转 RGB
    if let Some(ts) = tint {
        if comps == ts.n {
            let samples = expand_bits_rows(
                &data,
                bpc.max(1) as usize,
                width as usize,
                height as usize,
                comps,
                true,
            );
            if samples.len() < n_pixels * comps {
                return Decoded::Passthrough {
                    ext: "bin",
                    bytes: data,
                    filter: Vec::new(),
                    decode_parms: Vec::new(),
                };
            }
            let mut rgb = Vec::with_capacity(n_pixels * 3);
            for px in samples[..n_pixels * comps].chunks_exact(comps) {
                let comps_in: Vec<f64> = px.iter().map(|v| *v as f64 / 255.0).collect();
                let out = eval_ps_calculator(&ts.code, &comps_in).unwrap_or_default();
                let c = rgb_from_values(&out, &ts.alt);
                rgb.extend_from_slice(&[c.0, c.1, c.2]);
            }
            return match png_from_pixels(width, height, 3, &rgb) {
                Some(png) => Decoded::Png(png),
                None => Decoded::Passthrough {
                    ext: "bin",
                    bytes: data,
                    filter: Vec::new(),
                    decode_parms: Vec::new(),
                },
            };
        }
    }
    let samples = expand_bits_rows(
        &data,
        bpc.max(1) as usize,
        width as usize,
        height as usize,
        comps,
        true,
    );
    if samples.len() < n_pixels * comps {
        return Decoded::Passthrough {
            ext: "bin",
            bytes: data,
            filter: Vec::new(),
            decode_parms: Vec::new(),
        };
    }
    // Decode 反相（默认 [0 1] 不反相）
    let samples = if let Some(d) = decode_arr {
        if d.len() >= comps * 2 && d[0] > d[1] {
            samples.iter().map(|v| 255 - *v).collect::<Vec<u8>>()
        } else {
            samples
        }
    } else {
        samples
    };
    let pixels: Vec<u8> = match cs {
        CsSpec::Gray => samples[..n_pixels].to_vec(),
        CsSpec::Rgb => samples[..n_pixels * 3].to_vec(),
        CsSpec::Cmyk => {
            // CMYK → RGB（朴素换算；Adobe 反相已由 Decode 处理）
            let mut rgb = Vec::with_capacity(n_pixels * 3);
            for px in samples[..n_pixels * 4].chunks_exact(4) {
                let (c, m, y, k) = (
                    px[0] as f32 / 255.0,
                    px[1] as f32 / 255.0,
                    px[2] as f32 / 255.0,
                    px[3] as f32 / 255.0,
                );
                rgb.push(((1.0 - c) * (1.0 - k) * 255.0) as u8);
                rgb.push(((1.0 - m) * (1.0 - k) * 255.0) as u8);
                rgb.push(((1.0 - y) * (1.0 - k) * 255.0) as u8);
            }
            rgb
        }
        CsSpec::CalRgb {
            gamma,
            matrix,
            white,
        } => {
            let mut rgb = Vec::with_capacity(n_pixels * 3);
            for px in samples[..n_pixels * 3].chunks_exact(3) {
                let (r, g, b) = calrgb_to_srgb(
                    px[0] as f64 / 255.0,
                    px[1] as f64 / 255.0,
                    px[2] as f64 / 255.0,
                    gamma,
                    matrix,
                    white,
                );
                rgb.push((r * 255.0).round().clamp(0.0, 255.0) as u8);
                rgb.push((g * 255.0).round().clamp(0.0, 255.0) as u8);
                rgb.push((b * 255.0).round().clamp(0.0, 255.0) as u8);
            }
            rgb
        }
        CsSpec::CalGray { gamma, white } => {
            let mut gray = Vec::with_capacity(n_pixels);
            for px in samples[..n_pixels].iter() {
                let (r, g, b) = calgray_to_srgb(*px as f64 / 255.0, *gamma, white);
                gray.push((0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 1.0) * 255.0);
            }
            gray.iter().map(|v| *v as u8).collect()
        }
        CsSpec::Indexed(..) | CsSpec::Tinted { .. } => unreachable!(),
    };
    let out_comps = if matches!(cs, CsSpec::Gray | CsSpec::CalGray { .. }) {
        1
    } else {
        3
    };
    // 颜色键遮罩：命中 /Mask 范围的像素置透明（pdfium/bug_343075986
    // 用 [0 0 0 0 0 0] 让黑色区域透出下层黄色）
    if let Some(key) = color_key {
        if key.len() >= out_comps * 2 {
            let mut rgba = Vec::with_capacity(n_pixels * 4);
            for px in pixels.chunks_exact(out_comps) {
                let hit = (0..out_comps).all(|i| {
                    let lo = key[i * 2] * 255.0;
                    let hi = key[i * 2 + 1] * 255.0;
                    let v = px[i] as f64;
                    v >= lo - 0.5 && v <= hi + 0.5
                });
                let (r, g, b) = if out_comps == 1 {
                    (px[0], px[0], px[0])
                } else {
                    (px[0], px[1], px[2])
                };
                rgba.extend_from_slice(&[r, g, b, if hit { 0 } else { 255 }]);
            }
            if let Some(png) = png_from_pixels(width, height, 4, &rgba) {
                return Decoded::Png(png);
            }
        }
    }
    match png_from_pixels(width, height, out_comps, &pixels) {
        Some(png) => Decoded::Png(png),
        None => Decoded::Passthrough {
            ext: "bin",
            bytes: data,
            filter: Vec::new(),
            decode_parms: Vec::new(),
        },
    }
}

fn pos_unused(_p: &[u8]) {}

/// 原始像素（RGB/Gray 8bit）→ PNG 字节。不支持的颜色空间/位深返回 None。
pub fn decode_to_png_bytes(
    data: &[u8],
    width: i64,
    height: i64,
    color_space: Option<&str>,
    bpc: i64,
) -> Option<Vec<u8>> {
    if width <= 0 || height <= 0 || bpc != 8 {
        return None;
    }
    let cs = color_space?;
    let components: usize = if cs.contains("RGB") {
        3
    } else if cs.contains("Gray") {
        1
    } else {
        return None;
    };
    let expected = (width * height * components as i64) as usize;
    if data.len() < expected {
        return None;
    }
    png_from_pixels(width, height, components, &data[..expected])
}

/// 供 QA 用：页内图片计数。
pub fn image_count(doc: &Document, page_id: (u32, u16)) -> usize {
    doc.get_page_images(page_id).map(|v| v.len()).unwrap_or(0)
}

/// 兼容旧接口：提取图片实体（CLI extract 用）。
pub fn extract_images(doc: &Document, pages: &[u32], out_dir: &Path) -> Result<Vec<Value>> {
    std::fs::create_dir_all(out_dir)?;
    let page_map = doc.get_pages();
    let mut results = Vec::new();
    for &pno in pages {
        let Some(&page_id) = page_map.get(&pno) else {
            continue;
        };
        let images = doc.get_page_images(page_id).unwrap_or_default();
        for (idx, img) in images.iter().enumerate() {
            let filters: Vec<String> = img
                .filters
                .clone()
                .unwrap_or_default()
                .iter()
                .map(|f| f.to_lowercase())
                .collect();
            let ext = if filters.iter().any(|f| f.contains("dct")) {
                "jpg"
            } else if filters.iter().any(|f| f.contains("jpx")) {
                "jp2"
            } else if filters
                .iter()
                .any(|f| f.contains("jbig2") || f.contains("ccitt"))
            {
                "bin"
            } else if filters.iter().any(|f| f.contains("flate") || f.is_empty()) {
                "png"
            } else {
                "bin"
            };
            let name = format!("page{pno}_img{}.{ext}", idx + 1);
            std::fs::write(out_dir.join(&name), img.content)?;
            results.push(json!({
                "page": pno, "index": idx, "file": name,
                "width": img.width, "height": img.height,
            }));
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_predictor_up() {
        // 2 行 x 2 像素灰度，滤波类型 2（Up）
        let data = vec![2, 10, 20, 2, 5, 5];
        let out = unpredict(&data, 12, 1, 8, 2).unwrap();
        assert_eq!(out, vec![10, 20, 15, 25]);
    }

    #[test]
    fn png_predictor_none_and_sub() {
        // None
        let data = vec![0, 1, 2, 0, 3, 4];
        assert_eq!(unpredict(&data, 10, 1, 8, 2).unwrap(), vec![1, 2, 3, 4]);
        // Sub（bpp=1）
        let data = vec![1, 5, 3];
        assert_eq!(unpredict(&data, 11, 1, 8, 2).unwrap(), vec![5, 8]);
    }

    #[test]
    fn expand_1bit() {
        assert_eq!(expand_bits(&[0b1010_0000], 1, 4), vec![255, 0, 255, 0]);
        assert_eq!(expand_bits(&[0b0100_0000], 2, 4), vec![85, 0, 0, 0]);
    }

    #[test]
    fn lzw_roundtrip_simple() {
        // 9bit LZW：clear(256) + 'A'(65) + 'B'(66) + EOD(257)
        // 位流：256,65,66,257 → 打包为 MSB-first 9bit
        let codes = [256u32, 65, 66, 257];
        let mut bits = Vec::new();
        for c in codes {
            for i in (0..9).rev() {
                bits.push(((c >> i) & 1) as u8);
            }
        }
        while bits.len() % 8 != 0 {
            bits.push(0);
        }
        let mut bytes = vec![0u8; bits.len() / 8];
        for (i, b) in bits.iter().enumerate() {
            bytes[i / 8] |= b << (7 - (i % 8));
        }
        assert_eq!(lzw_decode(&bytes).unwrap(), b"AB");
    }

    #[test]
    fn ascii85_basic() {
        assert_eq!(ascii85_decode(b"87cURD]j7BEbo80").unwrap(), b"Hello world!");
        assert_eq!(ascii85_decode(b"z").unwrap(), vec![0, 0, 0, 0]);
    }

    #[test]
    fn runlength_basic() {
        // 长度 2 字面量 + 重复 3 次
        let data = vec![1, b'a', b'b', 254, b'c', 128];
        assert_eq!(
            apply_simple_filter("runlengthdecode", &data).unwrap(),
            b"abccc"
        );
    }
}

#[cfg(test)]
mod ps_tests {
    use super::eval_ps_calculator;

    #[test]
    fn ps_calculator_tint_example() {
        // pdf.js/colorspace_atan 的 DeviceN tint transform（FunctionType 4）。
        // 回归要点：过程块 {…} 必须作为值压栈由 if 取用；顶层 {…} 为程序体；
        // 输出取栈顶（此前直接执行过程块并返回整个栈 → 整图变黑 50.75）
        let code = b"{ pop exch 360 mul dup sin exch cos atan 360 div sub abs .1 exch sub dup 0 lt { pop 0 } if 10 mul sqrt 1 exch sub dup dup }";
        let mid = eval_ps_calculator(code, &[0.3, 0.5, 0.7]).unwrap();
        assert!(
            (mid[mid.len() - 1] - 1.0).abs() < 1e-6,
            "中间调应为白: {mid:?}"
        );
        let hi = eval_ps_calculator(code, &[1.5, 0.5, 0.7]).unwrap();
        assert!(hi[hi.len() - 1] < 1e-6, "超范围输入应近黑: {hi:?}");
    }
}
