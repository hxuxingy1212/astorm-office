use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::error::Result;
use crate::generate::styles::builtin_code;
use crate::model::style::{Alignment, Border, Font, StyleDef};
use crate::parse::theme::ThemeColors;
use crate::utils::xml::normalize_color;

fn resolve_color(m: &HashMap<String, String>, theme: Option<&ThemeColors>) -> Option<String> {
    let base = if let Some(rgb) = get(m, "rgb") {
        Some(normalize_color(rgb))
    } else if let Some(t) = get(m, "theme").and_then(|v| v.parse::<usize>().ok()) {
        theme.and_then(|th| th.get(t)).map(|c| normalize_color(&c))
    } else if let Some(i) = get(m, "indexed").and_then(|v| v.parse::<usize>().ok()) {
        indexed_color(i).map(|s| s.to_string())
    } else {
        None
    };
    // Excel 会在基础色上叠加 tint（明暗）；-1..1，负值变暗、正值变亮。
    match get(m, "tint").and_then(|v| v.parse::<f64>().ok()) {
        Some(t) if t != 0.0 => base.map(|c| apply_tint(&c, t)),
        _ => base,
    }
}

/// 标准 `indexed` 调色板（前 64 位见 ECMA-376；64/65 为系统前景/背景，视为自动）。
fn indexed_color(i: usize) -> Option<&'static str> {
    const INDEXED: [&str; 64] = [
        "000000", "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF", "000000",
        "FFFFFF", "FF0000", "00FF00", "0000FF", "FFFF00", "FF00FF", "00FFFF", "800000", "008000",
        "000080", "808000", "800080", "008080", "C0C0C0", "808080", "9999FF", "993366", "FFFFCC",
        "CCFFFF", "660066", "FF8080", "0066CC", "CCCCFF", "000080", "FF00FF", "FFFF00", "00FFFF",
        "800080", "800000", "008080", "0000FF", "00CCFF", "CCFFFF", "CCFFCC", "FFFF99", "99CCFF",
        "FF99CC", "CC99FF", "FFCC99", "3366FF", "33CCCC", "99CC00", "FFCC00", "FF9900", "FF6600",
        "666699", "969696", "003366", "339966", "003300", "333300", "993300", "993366", "333399",
        "333333",
    ];
    INDEXED.get(i).copied()
}

/// 对 6 位十六进制色应用 tint（Excel 公式：暗色 `c*(1+tint)`，亮色 `c+(1-c)*tint`）。
fn apply_tint(hex: &str, tint: f64) -> String {
    let h = normalize_color(hex);
    if h.len() != 6 {
        return h;
    }
    let mut out = String::with_capacity(6);
    for i in [0usize, 2, 4] {
        let c = u8::from_str_radix(&h[i..i + 2], 16).unwrap_or(0) as f64 / 255.0;
        let v = if tint < 0.0 {
            c * (1.0 + tint)
        } else {
            c + (1.0 - c) * tint
        };
        out.push_str(&format!(
            "{:02X}",
            (v * 255.0).round().clamp(0.0, 255.0) as u8
        ));
    }
    out
}

#[derive(Default, Clone)]
struct RawXf {
    num_fmt_id: u32,
    font_id: u32,
    fill_id: u32,
    border_id: u32,
    alignment: Option<Alignment>,
    locked: Option<bool>,
}

pub struct ParsedStyles {
    /// Index 0 is `None` (the default xf).
    pub xfs: Vec<Option<StyleDef>>,
    /// Parallel to `xfs`: the number-format code carried by each xf.
    pub num_fmts: Vec<Option<String>>,
    /// Differential formats (`<dxfs>`) used by conditional formatting.
    pub dxfs: Vec<Option<StyleDef>>,
    /// The workbook default font (`fonts[0]`).
    pub default_font: Option<Font>,
}

fn attrs_of(e: &BytesStart) -> HashMap<String, String> {
    e.attributes()
        .filter_map(|a| a.ok())
        .map(|a| {
            (
                String::from_utf8_lossy(a.key.local_name().as_ref()).to_string(),
                a.unescape_value()
                    .map(|v| v.into_owned())
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn get<'a>(m: &'a HashMap<String, String>, k: &str) -> Option<&'a str> {
    m.get(k).map(|s| s.as_str())
}

/// 布尔型字体属性：缺省 `val` 视为 true；`val="0"/"false"/"off"` 视为 false。
fn flag_on(m: &HashMap<String, String>) -> bool {
    match get(m, "val") {
        None => true,
        Some(v) => matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "on"),
    }
}

pub fn parse_styles(xml: &str, theme: Option<&ThemeColors>) -> Result<ParsedStyles> {
    let mut reader = Reader::from_str(xml);

    let mut num_fmt_custom: HashMap<u32, String> = HashMap::new();
    let mut fonts: Vec<Font> = Vec::new();
    let mut fills: Vec<Option<String>> = Vec::new();
    let mut borders: Vec<Border> = Vec::new();
    let mut xfs: Vec<RawXf> = Vec::new();
    let mut dxfs: Vec<Option<StyleDef>> = Vec::new();

    let mut cur_font: Option<Font> = None;
    let mut cur_fill: Option<String> = None;
    let mut cur_border: Option<Border> = None;
    let mut cur_xf: Option<RawXf> = None;
    let mut cur_side: Option<String> = None;

    let handle_open = |name: &[u8],
                       m: &HashMap<String, String>,
                       empty: bool,
                       in_cell_xfs: bool,
                       cur_font: &mut Option<Font>,
                       cur_fill: &mut Option<String>,
                       cur_border: &mut Option<Border>,
                       cur_xf: &mut Option<RawXf>,
                       cur_side: &mut Option<String>,
                       num_fmt_custom: &mut HashMap<u32, String>,
                       fonts: &mut Vec<Font>,
                       fills: &mut Vec<Option<String>>,
                       borders: &mut Vec<Border>,
                       xfs: &mut Vec<RawXf>| {
        match name {
            b"numFmt" => {
                if let (Some(id), Some(code)) = (get(m, "numFmtId"), get(m, "formatCode")) {
                    if let Ok(id) = id.parse::<u32>() {
                        num_fmt_custom.insert(id, code.to_string());
                    }
                }
            }
            b"font" => {
                *cur_font = Some(Font::default());
                if empty {
                    fonts.push(cur_font.take().unwrap());
                }
            }
            b"b" => {
                if let Some(f) = cur_font.as_mut() {
                    f.bold = flag_on(m);
                }
            }
            b"i" => {
                if let Some(f) = cur_font.as_mut() {
                    f.italic = flag_on(m);
                }
            }
            b"strike" => {
                if let Some(f) = cur_font.as_mut() {
                    f.strike = flag_on(m);
                }
            }
            b"u" => {
                if let Some(f) = cur_font.as_mut() {
                    f.underline = Some(get(m, "val").unwrap_or("single").to_string());
                }
            }
            b"sz" => {
                if let Some(f) = cur_font.as_mut() {
                    if let Some(v) = get(m, "val").and_then(|v| v.parse().ok()) {
                        f.size = Some(v);
                    }
                }
            }
            b"color" => {
                if let Some(f) = cur_font.as_mut() {
                    if let Some(c) = resolve_color(m, theme) {
                        f.color = Some(c);
                    }
                } else if cur_side.is_some() {
                    if let Some(b) = cur_border.as_mut() {
                        if let Some(c) = resolve_color(m, theme) {
                            if cur_side.as_deref() == Some("diagonal") {
                                b.diagonal_color = Some(c);
                            } else {
                                b.color = Some(c);
                            }
                        }
                    }
                }
            }
            b"name" => {
                if let Some(f) = cur_font.as_mut() {
                    if let Some(v) = get(m, "val") {
                        f.name = Some(v.to_string());
                    }
                }
            }
            b"fill" => {
                *cur_fill = None;
                if empty {
                    fills.push(None);
                }
            }
            b"patternFill" if get(m, "patternType") == Some("gray125") => {
                *cur_fill = Some("GRAY125".to_string());
            }
            b"fgColor" => {
                if let Some(c) = resolve_color(m, theme) {
                    *cur_fill = Some(c);
                }
            }
            // 差分格式（dxf）的图案填充通常只用 bgColor 表示颜色。
            b"bgColor" if cur_fill.is_none() => {
                if let Some(c) = resolve_color(m, theme) {
                    *cur_fill = Some(c);
                }
            }
            b"border" => {
                let b = Border {
                    diagonal_up: matches!(get(m, "diagonalUp"), Some("1") | Some("true")),
                    diagonal_down: matches!(get(m, "diagonalDown"), Some("1") | Some("true")),
                    ..Default::default()
                };
                *cur_border = Some(b);
                if empty {
                    borders.push(cur_border.take().unwrap());
                }
            }
            b"left" | b"right" | b"top" | b"bottom" | b"diagonal" => {
                *cur_side = Some(String::from_utf8_lossy(name).to_string());
                if let Some(b) = cur_border.as_mut() {
                    if let Some(style) = get(m, "style") {
                        let side = cur_side.clone().unwrap();
                        match side.as_str() {
                            "left" => b.left = Some(style.to_string()),
                            "right" => b.right = Some(style.to_string()),
                            "top" => b.top = Some(style.to_string()),
                            "bottom" => b.bottom = Some(style.to_string()),
                            "diagonal" => b.diagonal = Some(style.to_string()),
                            _ => {}
                        }
                    }
                }
            }
            b"xf" => {
                if !in_cell_xfs {
                    return;
                }
                let mut xf = RawXf::default();
                if let Some(v) = get(m, "numFmtId").and_then(|v| v.parse().ok()) {
                    xf.num_fmt_id = v;
                }
                if let Some(v) = get(m, "fontId").and_then(|v| v.parse().ok()) {
                    xf.font_id = v;
                }
                if let Some(v) = get(m, "fillId").and_then(|v| v.parse().ok()) {
                    xf.fill_id = v;
                }
                if let Some(v) = get(m, "borderId").and_then(|v| v.parse().ok()) {
                    xf.border_id = v;
                }
                *cur_xf = Some(xf);
                if empty {
                    xfs.push(cur_xf.take().unwrap());
                }
            }
            b"alignment" => {
                if !in_cell_xfs {
                    return;
                }
                if let Some(xf) = cur_xf.as_mut() {
                    let a = Alignment {
                        horizontal: get(m, "horizontal").map(|s| s.to_string()),
                        vertical: get(m, "vertical").map(|s| s.to_string()),
                        wrap_text: matches!(get(m, "wrapText"), Some("1") | Some("true")),
                        indent: get(m, "indent").and_then(|v| v.parse().ok()),
                        text_rotation: get(m, "textRotation").and_then(|v| v.parse().ok()),
                    };
                    xf.alignment = Some(a);
                }
            }
            b"protection" => {
                if !in_cell_xfs {
                    return;
                }
                if let Some(xf) = cur_xf.as_mut() {
                    if let Some(l) = get(m, "locked") {
                        xf.locked = Some(l == "1" || l == "true");
                    }
                }
            }
            _ => {}
        }
    };

    let mut in_cell_xfs = false;
    let mut in_dxf = false;
    let mut cur_dxf: Option<StyleDef> = None;
    loop {
        match reader.read_event()? {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_vec();
                if name == b"cellXfs" {
                    in_cell_xfs = true;
                }
                if name == b"dxf" {
                    cur_dxf = Some(StyleDef::default());
                    in_dxf = true;
                }
                let m = attrs_of(&e);
                handle_open(
                    &name,
                    &m,
                    false,
                    in_cell_xfs,
                    &mut cur_font,
                    &mut cur_fill,
                    &mut cur_border,
                    &mut cur_xf,
                    &mut cur_side,
                    &mut num_fmt_custom,
                    &mut fonts,
                    &mut fills,
                    &mut borders,
                    &mut xfs,
                );
            }
            Event::Empty(e) => {
                let name = e.local_name().as_ref().to_vec();
                let m = attrs_of(&e);
                handle_open(
                    &name,
                    &m,
                    true,
                    in_cell_xfs,
                    &mut cur_font,
                    &mut cur_fill,
                    &mut cur_border,
                    &mut cur_xf,
                    &mut cur_side,
                    &mut num_fmt_custom,
                    &mut fonts,
                    &mut fills,
                    &mut borders,
                    &mut xfs,
                );
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"font" => {
                    if in_dxf {
                        if let Some(dxf) = cur_dxf.as_mut() {
                            dxf.font = cur_font.take();
                        }
                    } else if let Some(f) = cur_font.take() {
                        fonts.push(f);
                    }
                }
                b"fill" => {
                    if in_dxf {
                        if let Some(dxf) = cur_dxf.as_mut() {
                            dxf.fill = cur_fill.take();
                        }
                    } else {
                        fills.push(cur_fill.take());
                    }
                }
                b"dxf" => {
                    dxfs.push(cur_dxf.take());
                    in_dxf = false;
                }
                b"border" => {
                    if let Some(b) = cur_border.take() {
                        borders.push(b);
                    }
                }
                b"left" | b"right" | b"top" | b"bottom" | b"diagonal" => {
                    cur_side = None;
                }
                b"xf" => {
                    if let Some(xf) = cur_xf.take() {
                        xfs.push(xf);
                    }
                }
                b"cellXfs" => {
                    in_cell_xfs = false;
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }

    // Compose StyleDef per xf.
    let mut parsed: Vec<Option<StyleDef>> = Vec::with_capacity(xfs.len());
    let mut num_fmts: Vec<Option<String>> = Vec::with_capacity(xfs.len());
    for (i, xf) in xfs.iter().enumerate() {
        if i == 0 {
            parsed.push(None);
            num_fmts.push(None);
            continue;
        }
        let mut s = StyleDef::default();
        if xf.font_id != 0 {
            if let Some(f) = fonts.get(xf.font_id as usize) {
                if !f.is_empty() {
                    s.font = Some(f.clone());
                }
            }
        }
        // 注意：个别文件把 fill[0] 定义成实色（非规范），故不能只处理 fill_id>=1。
        if let Some(Some(c)) = fills.get(xf.fill_id as usize) {
            s.fill = Some(c.clone());
        }
        if xf.border_id != 0 {
            if let Some(b) = borders.get(xf.border_id as usize) {
                if !b.is_empty() {
                    s.border = Some(b.clone());
                }
            }
        }
        if let Some(a) = &xf.alignment {
            if !a.is_empty() {
                s.alignment = Some(a.clone());
            }
        }
        s.locked = xf.locked;
        parsed.push(if s.is_empty() { None } else { Some(s) });
        num_fmts.push(num_fmt_code(xf.num_fmt_id, &num_fmt_custom));
    }

    Ok(ParsedStyles {
        xfs: parsed,
        num_fmts,
        dxfs,
        default_font: fonts.first().cloned(),
    })
}

fn num_fmt_code(id: u32, custom: &HashMap<u32, String>) -> Option<String> {
    if id == 0 {
        return None;
    }
    if let Some(code) = builtin_code(id) {
        return Some(code.to_string());
    }
    custom.get(&id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_palette() {
        assert_eq!(indexed_color(0), Some("000000"));
        assert_eq!(indexed_color(2), Some("FF0000"));
        assert_eq!(indexed_color(22), Some("C0C0C0"));
        assert_eq!(indexed_color(99), None);
    }

    #[test]
    fn tint_darken_lighten() {
        assert_eq!(apply_tint("808080", -0.5), "404040");
        assert_eq!(apply_tint("000000", 0.5), "808080");
        assert_eq!(apply_tint("FFFFFF", -0.5), "808080");
        assert_eq!(apply_tint("4472C4", 0.0), "4472C4");
    }

    #[test]
    fn font_bool_val_zero_is_off() {
        let mk = |font: &str| {
            format!(
                r#"<?xml version="1.0"?><styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><fonts count="1"><font>{font}</font></fonts><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>"#
            )
        };
        let off = parse_styles(
            &mk(r#"<b val="0"/><i val="0"/><strike val="0"/><sz val="10"/><name val="Sans"/>"#),
            None,
        )
        .expect("parse styles");
        let f = off.default_font.as_ref().unwrap();
        assert!(
            !f.bold && !f.italic && !f.strike,
            "val=0 的 b/i/strike 应为 off"
        );

        let on = parse_styles(&mk("<b/><i/>"), None).expect("parse styles");
        let f = on.default_font.as_ref().unwrap();
        assert!(f.bold && f.italic, "无 val 的 <b/> <i/> 应为 on");
    }
}
