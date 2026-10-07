//! 单位换算：Word 使用 pt / twip / half-point / EMU / eighth-point。
//! 所有换算集中于此，全部为精确整数比。

/// 1 pt = 20 twip
pub const TWIP_PER_PT: f64 = 20.0;
/// 1 pt = 12700 EMU
pub const EMU_PER_PT: f64 = 12700.0;
/// 1 pt = 2 half-point（字号 w:sz）
pub const HALF_POINT_PER_PT: f64 = 2.0;
/// 1 pt = 8 eighth-point（边框 w:sz）
pub const EIGHTH_POINT_PER_PT: f64 = 8.0;
/// 1 英寸 = 72 pt
pub const PT_PER_INCH: f64 = 72.0;
/// 1 英寸 = 1440 twip
pub const TWIP_PER_INCH: f64 = 1440.0;
/// 行距单倍 = w:line 240
pub const LINE_SINGLE: f64 = 240.0;

/// pt → twip（取整）
pub fn pt_to_twip(pt: f64) -> i64 {
    (pt * TWIP_PER_PT).round() as i64
}
/// twip → pt
pub fn twip_to_pt(twip: i64) -> f64 {
    twip as f64 / TWIP_PER_PT
}
/// pt → half-point（字号，取整）
pub fn pt_to_half(pt: f64) -> i64 {
    (pt * HALF_POINT_PER_PT).round() as i64
}
/// half-point → pt
pub fn half_to_pt(half: i64) -> f64 {
    half as f64 / HALF_POINT_PER_PT
}
/// pt → EMU（取整）
pub fn pt_to_emu(pt: f64) -> i64 {
    (pt * EMU_PER_PT).round() as i64
}
/// EMU → pt
pub fn emu_to_pt(emu: i64) -> f64 {
    emu as f64 / EMU_PER_PT
}
/// pt → eighth-point（边框，取整）
pub fn pt_to_eighth(pt: f64) -> i64 {
    (pt * EIGHTH_POINT_PER_PT).round() as i64
}
/// 行距倍数 → w:line
pub fn line_to_xml(mult: f64) -> i64 {
    (mult * LINE_SINGLE).round() as i64
}
/// 解析单位限定长度串（12pt / 0.5cm / 1.5x / 150% / 2.54cm / 72pt / 96px / 1in）
///
/// 返回 pt。裸数字视为 pt。
pub fn parse_length(s: &str) -> Option<f64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    // 处理百分比与倍数：非长度语义由调用方另行处理
    let (num, unit) = split_number_unit(&lower)?;
    let v: f64 = num.parse().ok()?;
    Some(match unit.as_str() {
        "" | "pt" => v,
        "twip" | "twips" => v / TWIP_PER_PT,
        "cm" => v * 72.0 / 2.54,
        "mm" => v * 72.0 / 25.4,
        "in" | "inch" => v * 72.0,
        "px" => v * 72.0 / 96.0,
        "emu" => v / EMU_PER_PT,
        _ => return None,
    })
}

/// 解析行距：接受 "1.5x" / "150%" / 数字（倍数）或 "18pt"（精确值返回负值标记由调用方处理）
/// 这里只处理倍数形式，精确 pt 由 parse_length 处理。
pub fn parse_line_spacing(s: &str) -> Option<f64> {
    let s = s.trim().to_ascii_lowercase();
    if let Some(stripped) = s.strip_suffix('x') {
        return stripped.trim().parse().ok();
    }
    if let Some(stripped) = s.strip_suffix('%') {
        let v: f64 = stripped.trim().parse().ok()?;
        return Some(v / 100.0);
    }
    s.parse().ok()
}

fn split_number_unit(s: &str) -> Option<(String, String)> {
    let mut split = s.len();
    for (i, c) in s.char_indices() {
        if !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+') {
            split = i;
            break;
        }
    }
    let num = s[..split].to_string();
    let unit = s[split..].trim().to_string();
    if num.is_empty() {
        return None;
    }
    Some((num, unit))
}
