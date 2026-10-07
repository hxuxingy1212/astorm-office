//! Render a workbook into a standalone HTML preview (cells + images + charts).

use std::collections::{HashMap, HashSet};
use std::path::Path;

use serde_json::Value;

use json2xlsx::model::style::StyleDef;
use json2xlsx::model::workbook::{cell_display, resolve_style};
use json2xlsx::model::{Cell, Chart, Sheet, Workbook};
use json2xlsx::utils::a1::{make_cell_ref, parse_cell_ref, parse_range};

/// 单元格缺省对齐：数字右、布尔居中、文本左（Excel 语义；显式对齐优先）
fn default_align(cell: &Cell) -> &'static str {
    match &cell.value {
        Some(Value::Number(_)) => "right",
        Some(Value::Bool(_)) => "center",
        _ => "left",
    }
}

/// 把模型里的媒体包内路径（如 `xl/media/image1.png`）解析为可被 HTML 直接加载的路径。
/// - http(s)/data/file URL、绝对路径：原样保留；
/// - 相对包内路径：基于产物根（media_base）解析为绝对路径；解析不到时返回 None。
fn resolve_media(media_base: Option<&Path>, src: &str) -> Option<String> {
    let s = src.trim();
    if s.is_empty() {
        return None;
    }
    for scheme in ["http://", "https://", "data:", "file://"] {
        if s.starts_with(scheme) {
            return Some(s.to_string());
        }
    }
    let p = Path::new(s);
    if p.is_absolute() {
        return if p.exists() {
            Some(s.to_string())
        } else {
            None
        };
    }
    let base = media_base?;
    let joined = base.join(s);
    if joined.exists() {
        Some(joined.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// 媒体 → HTML 属性值（file:// URL，非 ASCII 做百分号编码）
fn media_url(path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") || path.starts_with("data:") {
        return path.to_string();
    }
    let p = path.strip_prefix("file://").unwrap_or(path);
    let encoded = percent_encode_path(p);
    format!("file://{}", encoded)
}

fn percent_encode_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for b in p.as_bytes() {
        match *b {
            b' ' => out.push_str("%20"),
            b'%' => out.push_str("%25"),
            b'#' => out.push_str("%23"),
            b'?' => out.push_str("%3F"),
            b'"' => out.push_str("%22"),
            0x21..=0x7E => out.push(*b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// OOXML paperSize → (宽, 高) 英寸（仅列常用规格，未知按 A4）
fn paper_inches(code: Option<u32>) -> (f64, f64) {
    match code {
        Some(1) => (8.5, 11.0),    // Letter
        Some(5) => (8.5, 14.0),    // Legal
        Some(8) => (11.69, 16.54), // A3
        Some(9) => (8.27, 11.69),  // A4（默认）
        Some(11) => (5.83, 8.27),  // A5
        Some(3) => (11.0, 17.0),   // Tabloid
        _ => (8.27, 11.69),
    }
}

/// 一页可打印内容宽度（CSS px @96dpi）：纸张宽度 − 左右边距，
/// 并按打印缩放（scale%）折算；用于模拟分页截断。
fn page_content_width_px(print: Option<&json2xlsx::model::PrintSettings>) -> u32 {
    let (mut w, mut h) = paper_inches(print.and_then(|p| p.paper_size));
    if print
        .and_then(|p| p.orientation.as_deref())
        .map(|o| o.eq_ignore_ascii_case("landscape"))
        .unwrap_or(false)
    {
        std::mem::swap(&mut w, &mut h);
    }
    let ml = print.and_then(|p| p.margin_left).unwrap_or(0.75);
    let mr = print.and_then(|p| p.margin_right).unwrap_or(0.75);
    let scale = print
        .and_then(|p| p.scale)
        .filter(|s| *s > 0)
        .unwrap_or(100) as f64
        / 100.0;
    (((w - ml - mr).max(1.0) * 96.0) * scale).round() as u32
}

/// PNG 画布宽度：按首个工作表的页面内容宽 + 页面左右留白（模拟"一页"）
pub fn page_width_for(wb: &Workbook) -> u32 {
    let print = wb.sheets.first().and_then(|s| s.print.as_ref());
    let w = page_content_width_px(print);
    (w + 48).max(320)
}

/// 渲染报告：被分页截断的列（工作表名, 截断列数）
#[derive(Default, Debug)]
pub struct RenderReport {
    pub clipped: Vec<(String, usize)>,
}

impl RenderReport {
    pub fn clipped_total(&self) -> usize {
        self.clipped.iter().map(|(_, n)| *n).sum()
    }
}

/// 列宽（字符）→ 像素（Excel 近似：7px/字符 + 5px 内边距）
fn col_width_px(chars: f64) -> u32 {
    (chars * 7.0).round() as u32 + 5
}

/// 行高（磅）→ 像素
fn row_height_px(pt: f64) -> u32 {
    (pt * 96.0 / 72.0).round() as u32
}

pub fn workbook_html(wb: &Workbook, media_base: Option<&Path>) -> (String, RenderReport) {
    let mut report = RenderReport::default();
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html lang=\"zh\"><head><meta charset=\"utf-8\">\n");
    out.push_str("<title>json2xlsx preview</title>\n<style>\n");
    out.push_str(
        "body{font-family:Calibri,Arial,'Microsoft YaHei',sans-serif;margin:24px;color:#222}\
         h2{font-size:16px;border-bottom:2px solid #4472C4;padding-bottom:4px;margin-top:32px}\
         table{border-collapse:collapse;margin:8px 0;table-layout:fixed}\
         td{border:1px solid #D0D0D0;padding:1px 4px;font-size:13px;height:18px;overflow:hidden;white-space:pre}\
         .drawings{margin:12px 0}\
         .drawings img{max-height:96px;border:1px solid #ccc;vertical-align:top}\
         .chart{display:inline-block;margin:6px 12px 6px 0;border:1px solid #e0e0e0;padding:4px}\
         .meta{color:#777;font-size:12px}\n",
    );
    out.push_str("</style></head><body>\n");
    out.push_str("<h1 style=\"font-size:20px\">json2xlsx preview</h1>\n");

    for sheet in &wb.sheets {
        out.push_str(&format!("<h2>{}</h2>\n", esc(&sheet.name)));
        let cf = cf_styles(sheet);
        // 工作表背景图：平铺在表格区域（解析媒体路径，避免相对路径失效）
        let bg_style = sheet
            .background
            .as_deref()
            .and_then(|b| resolve_media(media_base, b))
            .map(|p| {
                format!(
                    " style=\"background-image:url('{}');background-repeat:repeat\"",
                    esc(&media_url(&p))
                )
            })
            .unwrap_or_default();
        out.push_str(&format!("<table{bg_style}>\n"));

        // cell lookup by (row, col)
        let mut by_rc: HashMap<(u32, u32), &Cell> = HashMap::new();
        let mut max_col = 0u32;
        for row in &sheet.rows {
            for cell in &row.cells {
                if let Some((c, rr)) = cell
                    .reference
                    .as_deref()
                    .and_then(|x| parse_cell_ref(x).ok())
                {
                    by_rc.insert((rr, c), cell);
                    max_col = max_col.max(c);
                }
            }
        }
        // merges -> anchors (colspan,rowspan) + covered cells
        let mut anchors: HashMap<(u32, u32), (u32, u32)> = HashMap::new();
        let mut covered: HashSet<(u32, u32)> = HashSet::new();
        for m in &sheet.merges {
            if let Ok(((c1, r1), (c2, r2))) = parse_range(m) {
                if c2 > c1 || r2 > r1 {
                    anchors.insert((r1, c1), (c2 - c1 + 1, r2 - r1 + 1));
                    for rr in r1..=r2 {
                        for cc in c1..=c2 {
                            if (rr, cc) != (r1, c1) {
                                covered.insert((rr, cc));
                            }
                        }
                    }
                    // 合并区若跨越截断线，则保留整列（打印时合并区不会被切开）
                    max_col = max_col.max(c2);
                }
            }
        }

        let emit = |out: &mut String, cell: Option<&Cell>, span: &str| {
            let resolved_style = cell.and_then(|c| resolve_style(wb, c));
            let mut style = resolved_style.as_ref().map(css).unwrap_or_default();
            // 缺省对齐（Excel：数字右/布尔中/文本左）；显式 halign 时不覆盖
            if let Some(c) = cell {
                let explicit = resolved_style
                    .as_ref()
                    .and_then(|s| s.alignment.as_ref())
                    .and_then(|a| a.horizontal.as_deref());
                if explicit.is_none() {
                    if !style.is_empty() {
                        style.push(';');
                    }
                    style.push_str(&format!("text-align:{}", default_align(c)));
                }
            }
            if let Some(extra) = cell
                .and_then(|c| c.reference.as_deref())
                .and_then(|r| cf.get(r))
                .map(cf_css)
                .filter(|s| !s.is_empty())
            {
                if !style.is_empty() {
                    style.push(';');
                }
                style.push_str(&extra);
            }
            let mut attrs = span.to_string();
            if !style.is_empty() {
                attrs.push_str(&format!(" style=\"{style}\""));
            }
            if let Some(cm) = cell.and_then(|c| c.comment.clone()) {
                attrs.push_str(&format!(" title=\"{}\"", esc(&cm)));
            }
            let icon = cell
                .and_then(|c| c.reference.as_deref())
                .and_then(|r| cf.get(r))
                .and_then(|c| c.icon)
                .map(|i| format!("{i} "))
                .unwrap_or_default();
            let (text, num_color) = cell.map(display_ex).unwrap_or_default();
            let shown = match &num_color {
                Some(color) if !color.is_empty() => {
                    format!(
                        "<span style=\"color:#{}\">{}</span>",
                        normalize(color),
                        esc(&text)
                    )
                }
                _ => esc(&text),
            };
            let content = match cell.and_then(|c| c.link.clone()) {
                Some(link) => format!("<a href=\"{}\">{}</a>", esc(&link), shown),
                None => shown,
            };
            let marker = if cell.and_then(|c| c.comment.as_ref()).is_some() {
                "&#128172;"
            } else {
                ""
            };
            out.push_str(&format!("<td{attrs}>{icon}{content}{marker}</td>"));
        };

        // 列宽：sheet.columns 显式宽度优先，缺省用工作表默认列宽（字符→像素）
        let default_col = sheet
            .default_col_width
            .or(sheet.base_col_width)
            .unwrap_or(8.43);
        let col_px = |c: u32| -> u32 {
            let col = sheet.columns.get((c - 1) as usize);
            if col.map(|x| x.hidden).unwrap_or(false) {
                0
            } else {
                col_width_px(col.and_then(|x| x.width).unwrap_or(default_col))
            }
        };
        // 分页截断：一页只容纳得下的列（整列语义，与打印一致）
        let page_w = page_content_width_px(sheet.print.as_ref());
        let mut used = 0u32;
        let mut last_col = 0u32;
        for c in 1..=max_col {
            let w = col_px(c);
            if used + w > page_w {
                break;
            }
            used += w;
            last_col = c;
        }
        if last_col == 0 && max_col > 0 {
            last_col = 1; // 页宽异常小时至少保留首列
        }
        let clipped = max_col.saturating_sub(last_col) as usize;
        if clipped > 0 {
            report.clipped.push((sheet.name.clone(), clipped));
        }
        out.push_str("<colgroup>");
        for c in 1..=last_col {
            out.push_str(&format!("<col style=\"width:{}px\">", col_px(c)));
        }
        out.push_str("</colgroup>\n");

        let row_height: HashMap<u32, f64> = sheet
            .rows
            .iter()
            .filter_map(|r| r.index.zip(r.height))
            .collect();
        let default_row_pt = sheet.default_row_height.unwrap_or(15.0);

        let mut row_ids: Vec<u32> = sheet.rows.iter().map(|r| r.index.unwrap_or(0)).collect();
        row_ids.sort_unstable();
        row_ids.dedup();
        for r in row_ids {
            if max_col > 0 && (1..=max_col).all(|c| covered.contains(&(r, c))) {
                continue; // wholly swallowed by a vertical merge
            }
            let h = row_height_px(*row_height.get(&r).unwrap_or(&default_row_pt));
            out.push_str(&format!("<tr style=\"height:{h}px\">"));
            let mut c = 1u32;
            while c <= last_col {
                if covered.contains(&(r, c)) {
                    c += 1;
                    continue;
                }
                let (cs, rs) = anchors.get(&(r, c)).copied().unwrap_or((1, 1));
                let mut span = String::new();
                if cs > 1 {
                    span.push_str(&format!(" colspan=\"{cs}\""));
                }
                if rs > 1 {
                    span.push_str(&format!(" rowspan=\"{rs}\""));
                }
                emit(&mut out, by_rc.get(&(r, c)).copied(), &span);
                c += cs;
            }
            out.push_str("</tr>\n");
        }
        out.push_str("</table>\n");
        if clipped > 0 {
            out.push_str(&format!(
                "<div class=\"meta\">（已按一页宽度截断 {clipped} 列；完整内容见 xlsx 或放大页面宽度）</div>\n"
            ));
        }

        if !sheet.merges.is_empty() {
            out.push_str(&format!(
                "<p class=\"meta\">merged: {}</p>\n",
                esc(&sheet.merges.join(", "))
            ));
        }

        if !sheet.images.is_empty() || !sheet.charts.is_empty() {
            out.push_str("<div class=\"drawings\">\n");
            for img in &sheet.images {
                let anchor = img.anchor.clone().unwrap_or_default();
                let src = resolve_media(media_base, &img.src);
                match src {
                    Some(p) => out.push_str(&format!(
                        "<img src=\"{}\" title=\"anchor {}\" alt=\"{}\" style=\"max-width:420px\">\n",
                        esc(&media_url(&p)),
                        esc(&anchor),
                        esc(&img.src)
                    )),
                    None => out.push_str(&format!(
                        "<span class=\"meta\">[缺失图片: {}]</span>\n",
                        esc(&img.src)
                    )),
                }
            }
            for (i, chart) in sheet.charts.iter().enumerate() {
                out.push_str(&chart_svg(sheet, chart, i));
            }
            out.push_str("</div>\n");
        }
    }

    out.push_str("</body></html>\n");
    (out, report)
}

// ------------------------------------------------------- number display ---

/// 显示文本 + 数字格式自带颜色（如 [Red]-0.00）
fn display_ex(cell: &Cell) -> (String, Option<String>) {
    if let Some(Value::Number(n)) = &cell.value {
        if let Some(nf) = &cell.number_format {
            if let Some(v) = n.as_f64() {
                if nf != "General" && nf != "@" {
                    return format_number_ex(v, nf);
                }
            }
        }
    }
    (cell_display(cell), None)
}

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
fn format_number_ex(v: f64, nf: &str) -> (String, Option<String>) {
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

/// 渲染单个格式段（v 已取绝对值）
fn render_section(v: f64, tpl: &str, add_sign: bool) -> String {
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
                pattern.push('%');
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
    let x = if pct { v * 100.0 } else { v };
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

    let mut s = format!("{x:.frac_places$}");
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

/// 便捷入口（单测与内部使用）
#[cfg_attr(not(test), allow(dead_code))]
fn format_number(v: f64, nf: &str) -> String {
    format_number_ex(v, nf).0
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

// ---------------------------------------------------- conditional format --

#[derive(Default, Clone)]
struct CfStyle {
    bg: Option<String>,
    color: Option<String>,
    bold: bool,
    icon: Option<char>,
}

fn cf_styles(sheet: &Sheet) -> HashMap<String, CfStyle> {
    let mut map: HashMap<String, CfStyle> = HashMap::new();
    for cf in &sheet.conditional_formats {
        let Ok(((c1, r1), (c2, r2))) = parse_range(&cf.range) else {
            continue;
        };
        match cf.rule_type.as_str() {
            "cellIs" => {
                let target: f64 = cf
                    .formulas
                    .first()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let op = cf.operator.clone().unwrap_or_else(|| "greaterThan".into());
                for r in r1..=r2 {
                    for c in c1..=c2 {
                        if let Some(v) = cell_number(sheet, c, r) {
                            if compare(v, &op, target) {
                                let e = map.entry(make_cell_ref(c, r)).or_default();
                                if let Some(f) = &cf.font {
                                    if let Some(col) = &f.color {
                                        e.color = Some(normalize(col));
                                    }
                                    if f.bold {
                                        e.bold = true;
                                    }
                                }
                                if let Some(fill) = &cf.fill {
                                    e.bg = Some(normalize(fill));
                                }
                            }
                        }
                    }
                }
            }
            "colorScale" => {
                let vals = collect_numbers(sheet, c1, r1, c2, r2);
                let (min, max) = min_max(&vals);
                for (ref_, v) in vals {
                    let t = norm(v, min, max);
                    let col = scale_color(&cf.colors, t);
                    map.entry(ref_).or_default().bg = Some(col);
                }
            }
            "dataBar" => {
                let base = cf.color.clone().unwrap_or_else(|| "638EC6".into());
                let vals = collect_numbers(sheet, c1, r1, c2, r2);
                let (min, max) = min_max(&vals);
                for (ref_, v) in vals {
                    let t = norm(v, min, max);
                    let (r, g, b) = rgb(&base);
                    let a = 0.15 + 0.6 * t;
                    map.entry(ref_).or_default().bg = Some(format!("rgba({r},{g},{b},{a:.2})"));
                }
            }
            "iconSet" => {
                let vals = collect_numbers(sheet, c1, r1, c2, r2);
                let (min, max) = min_max(&vals);
                for (ref_, v) in vals {
                    let t = norm(v, min, max);
                    let icon = if t < 0.34 {
                        '\u{1F534}'
                    } else if t < 0.67 {
                        '\u{1F7E1}'
                    } else {
                        '\u{1F7E2}'
                    };
                    map.entry(ref_).or_default().icon = Some(icon);
                }
            }
            _ => {}
        }
    }
    map
}

fn cf_css(c: &CfStyle) -> String {
    let mut out = Vec::new();
    if let Some(bg) = &c.bg {
        if bg.starts_with("rgba") {
            out.push(format!("background:{bg}"));
        } else {
            out.push(format!("background:#{bg}"));
        }
    }
    if let Some(col) = &c.color {
        out.push(format!("color:#{col}"));
    }
    if c.bold {
        out.push("font-weight:bold".to_string());
    }
    out.join(";")
}

fn collect_numbers(sheet: &Sheet, c1: u32, r1: u32, c2: u32, r2: u32) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    for r in r1..=r2 {
        for c in c1..=c2 {
            if let Some(v) = cell_number(sheet, c, r) {
                out.push((make_cell_ref(c, r), v));
            }
        }
    }
    out
}

fn min_max(vals: &[(String, f64)]) -> (f64, f64) {
    let mut min = f64::MAX;
    let mut max = f64::MIN;
    for (_, v) in vals {
        min = min.min(*v);
        max = max.max(*v);
    }
    if min == f64::MAX {
        (0.0, 1.0)
    } else {
        (min, max)
    }
}

fn norm(v: f64, min: f64, max: f64) -> f64 {
    if max > min {
        (v - min) / (max - min)
    } else {
        0.5
    }
}

fn compare(v: f64, op: &str, t: f64) -> bool {
    match op {
        "greaterThan" => v > t,
        "lessThan" => v < t,
        "greaterThanOrEqual" => v >= t,
        "lessThanOrEqual" => v <= t,
        "equal" => (v - t).abs() < 1e-9,
        "notEqual" => (v - t).abs() >= 1e-9,
        _ => false,
    }
}

fn scale_color(colors: &[String], t: f64) -> String {
    if colors.is_empty() {
        return "FFFFFF".into();
    }
    if colors.len() == 1 {
        return normalize(&colors[0]);
    }
    let (a, b, tt) = if colors.len() >= 3 && t > 0.5 {
        (&colors[1], &colors[2], (t - 0.5) * 2.0)
    } else {
        (
            &colors[0],
            &colors[colors.len() - 1],
            if colors.len() >= 3 { t * 2.0 } else { t },
        )
    };
    let (r1, g1, b1) = rgb(a);
    let (r2, g2, b2) = rgb(b);
    let mix = |x: u8, y: u8| -> u8 { (x as f64 + (y as f64 - x as f64) * tt).round() as u8 };
    format!("{:02X}{:02X}{:02X}", mix(r1, r2), mix(g1, g2), mix(b1, b2))
}

fn rgb(hex: &str) -> (u8, u8, u8) {
    let s = normalize(hex);
    let parse = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap_or(0);
    if s.len() == 6 {
        (parse(0), parse(2), parse(4))
    } else {
        (0, 0, 0)
    }
}

// ------------------------------------------------------------- charts -----

const PALETTE: [&str; 6] = ["4472C4", "ED7D31", "70AD47", "FFC000", "5B9BD5", "A5A5A5"];

fn chart_svg(sheet: &Sheet, chart: &Chart, index: usize) -> String {
    let series = chart_series(sheet, chart);
    let labels = chart_labels(sheet, chart);
    if series.is_empty() {
        return String::new();
    }
    let title = chart.title.clone().unwrap_or_default();
    let color = |si: usize| -> String {
        chart
            .colors
            .get(si)
            .map(|c| normalize(c))
            .unwrap_or_else(|| PALETTE[si % PALETTE.len()].to_string())
    };
    let sv = chart.show_values;
    let body = match chart.chart_type.as_str() {
        "pie" | "ring" | "doughnut" => pie_svg(&series[0], &labels, sv),
        "line" => line_svg(&series, &labels, &color, sv),
        "area" => area_svg(&series, &labels, &color),
        "scatter" => scatter_svg(&series, &labels, &color),
        _ => column_svg(&series, &labels, &color, sv),
    };
    format!(
        "<div class=\"chart\"><div class=\"meta\">#{index} {} [{}]</div><svg width=\"420\" height=\"240\" xmlns=\"http://www.w3.org/2000/svg\">{}</svg></div>\n",
        esc(&title),
        esc(&chart.chart_type),
        body
    )
}

fn chart_series(sheet: &Sheet, chart: &Chart) -> Vec<Vec<f64>> {
    let Some(range) = chart.data_range.as_deref() else {
        return Vec::new();
    };
    let Ok(((c1, r1), (c2, r2))) = parse_range(range) else {
        return Vec::new();
    };
    let mut series = Vec::new();
    for col in c1..=c2 {
        let mut vals = Vec::new();
        for row in r1..=r2 {
            vals.push(cell_number(sheet, col, row).unwrap_or(0.0));
        }
        series.push(vals);
    }
    series
}

fn chart_labels(sheet: &Sheet, chart: &Chart) -> Vec<String> {
    let Some(range) = chart.categories.as_deref() else {
        return Vec::new();
    };
    let Ok(((c1, r1), (_, r2))) = parse_range(range) else {
        return Vec::new();
    };
    (r1..=r2)
        .map(|row| {
            let s = cell_text(sheet, c1, row).unwrap_or_else(|| row.to_string());
            truncate_label(&s, 9)
        })
        .collect()
}

fn truncate_label(s: &str, n: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > n {
        format!("{}…", chars[..n].iter().collect::<String>())
    } else {
        s.to_string()
    }
}

fn cell_number(sheet: &Sheet, col: u32, row: u32) -> Option<f64> {
    let want = make_cell_ref(col, row);
    for r in &sheet.rows {
        for c in &r.cells {
            if c.reference.as_deref() == Some(want.as_str()) {
                return match &c.value {
                    Some(Value::Number(n)) => n.as_f64(),
                    Some(Value::String(s)) => s.parse().ok(),
                    _ => None,
                };
            }
        }
    }
    None
}

fn cell_text(sheet: &Sheet, col: u32, row: u32) -> Option<String> {
    let want = make_cell_ref(col, row);
    for r in &sheet.rows {
        for c in &r.cells {
            if c.reference.as_deref() == Some(want.as_str()) {
                return Some(cell_display(c));
            }
        }
    }
    None
}

fn max_of(series: &[Vec<f64>]) -> f64 {
    series
        .iter()
        .flat_map(|s| s.iter())
        .cloned()
        .fold(0.0_f64, f64::max)
        .max(1.0)
}

const W: f64 = 420.0;
const H: f64 = 240.0;
const PAD: f64 = 34.0;

fn axes() -> String {
    format!(
        "<line x1=\"{PAD}\" y1=\"{}\" x2=\"{PAD}\" y2=\"{PAD}\" stroke=\"#999\"/>\
         <line x1=\"{PAD}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#999\"/>",
        H - PAD,
        H - PAD,
        W - 8.0,
        H - PAD
    )
}

fn column_svg(
    series: &[Vec<f64>],
    labels: &[String],
    color: &dyn Fn(usize) -> String,
    show_values: bool,
) -> String {
    let max = max_of(series);
    let ncat = series[0].len().max(1);
    let ns = series.len();
    let group_w = (W - PAD - 12.0) / ncat as f64;
    let bar_w = (group_w * 0.7) / ns as f64;
    let mut s = axes();
    for (si, vals) in series.iter().enumerate() {
        for (ci, v) in vals.iter().enumerate() {
            let h = (v / max) * (H - 2.0 * PAD);
            let x = PAD + ci as f64 * group_w + group_w * 0.15 + si as f64 * bar_w;
            let y = H - PAD - h;
            s.push_str(&format!(
                "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{bar_w:.1}\" height=\"{h:.1}\" fill=\"#{}\"/>",
                color(si)
            ));
            if show_values {
                s.push_str(&format!(
                    "<text x=\"{:.0}\" y=\"{:.0}\" font-size=\"10\" text-anchor=\"middle\" fill=\"#333\">{}</text>",
                    x + bar_w / 2.0,
                    y - 3.0,
                    trim(*v)
                ));
            }
        }
    }
    label_axis(&mut s, labels, group_w);
    s
}

fn area_svg(series: &[Vec<f64>], labels: &[String], color: &dyn Fn(usize) -> String) -> String {
    let max = max_of(series);
    let n = series[0].len().max(1);
    let step = (W - PAD - 12.0) / n as f64;
    let mut s = axes();
    for (si, vals) in series.iter().enumerate() {
        let mut pts = format!("{PAD},{}", H - PAD);
        for (i, v) in vals.iter().enumerate() {
            let x = PAD + (i as f64 + 0.5) * step;
            let y = H - PAD - (v / max) * (H - 2.0 * PAD);
            pts.push_str(&format!(" {x:.1},{y:.1}"));
        }
        pts.push_str(&format!(" {},{}", W - 12.0, H - PAD));
        s.push_str(&format!(
            "<polygon points=\"{pts}\" fill=\"#{}\" fill-opacity=\"0.35\" stroke=\"#{}\"/>",
            color(si),
            color(si)
        ));
    }
    label_axis(&mut s, labels, step);
    s
}

fn line_svg(
    series: &[Vec<f64>],
    labels: &[String],
    color: &dyn Fn(usize) -> String,
    show_values: bool,
) -> String {
    let max = max_of(series);
    let n = series[0].len().max(1);
    let step = (W - PAD - 12.0) / n as f64;
    let mut s = axes();
    for (si, vals) in series.iter().enumerate() {
        let mut pts = String::new();
        for (i, v) in vals.iter().enumerate() {
            let x = PAD + (i as f64 + 0.5) * step;
            let y = H - PAD - (v / max) * (H - 2.0 * PAD);
            pts.push_str(&format!("{x:.1},{y:.1} "));
        }
        s.push_str(&format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#{}\" stroke-width=\"2\"/>",
            pts.trim(),
            color(si)
        ));
        for (i, v) in vals.iter().enumerate() {
            let x = PAD + (i as f64 + 0.5) * step;
            let y = H - PAD - (v / max) * (H - 2.0 * PAD);
            s.push_str(&format!(
                "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"3\" fill=\"#{}\"/>",
                color(si)
            ));
            if show_values {
                s.push_str(&format!(
                    "<text x=\"{x:.0}\" y=\"{:.0}\" font-size=\"10\" text-anchor=\"middle\" fill=\"#333\">{}</text>",
                    y - 5.0,
                    trim(*v)
                ));
            }
        }
    }
    label_axis(&mut s, labels, step);
    s
}

fn scatter_svg(series: &[Vec<f64>], labels: &[String], color: &dyn Fn(usize) -> String) -> String {
    let max = max_of(series);
    let n = series[0].len().max(1);
    let step = (W - PAD - 12.0) / n as f64;
    let mut s = axes();
    for (si, vals) in series.iter().enumerate() {
        for (i, v) in vals.iter().enumerate() {
            let x = PAD + (i as f64 + 0.5) * step;
            let y = H - PAD - (v / max) * (H - 2.0 * PAD);
            s.push_str(&format!(
                "<circle cx=\"{x:.1}\" cy=\"{y:.1}\" r=\"4\" fill=\"#{}\"/>",
                color(si)
            ));
        }
    }
    label_axis(&mut s, labels, step);
    s
}

fn pie_svg(vals: &[f64], labels: &[String], show_values: bool) -> String {
    let total: f64 = vals.iter().sum::<f64>().max(1e-9);
    let cx = 130.0;
    let cy = 120.0;
    let r = 90.0;
    let mut angle = -std::f64::consts::FRAC_PI_2;
    let mut s = String::new();
    for (i, v) in vals.iter().enumerate() {
        let frac = v / total;
        let a2 = angle + frac * std::f64::consts::TAU;
        let x1 = cx + r * angle.cos();
        let y1 = cy + r * angle.sin();
        let x2 = cx + r * a2.cos();
        let y2 = cy + r * a2.sin();
        let large = if frac > 0.5 { 1 } else { 0 };
        let col = PALETTE[i % PALETTE.len()];
        s.push_str(&format!(
            "<path d=\"M {cx} {cy} L {x1:.1} {y1:.1} A {r} {r} 0 {large} 1 {x2:.1} {y2:.1} Z\" fill=\"#{col}\" stroke=\"#fff\"/>"
        ));
        let mid = (angle + a2) / 2.0;
        let lx = cx + (r + 22.0) * mid.cos();
        let ly = cy + (r + 22.0) * mid.sin();
        let lab = labels.get(i).cloned().unwrap_or_default();
        let extra = if show_values {
            format!(" {}", trim(*v))
        } else {
            String::new()
        };
        s.push_str(&format!(
            "<text x=\"{lx:.0}\" y=\"{ly:.0}\" font-size=\"11\" fill=\"#333\">{}{}</text>",
            esc(&lab),
            esc(&extra)
        ));
        angle = a2;
    }
    s
}

fn label_axis(s: &mut String, labels: &[String], step: f64) {
    for (i, l) in labels.iter().enumerate() {
        let x = PAD + i as f64 * step + step / 2.0;
        s.push_str(&format!(
            "<text x=\"{x:.0}\" y=\"{}\" font-size=\"10\" text-anchor=\"middle\" fill=\"#333\">{}</text>",
            H - PAD + 14.0,
            esc(l)
        ));
    }
}

// ------------------------------------------------------------- helpers ----

fn css(s: &StyleDef) -> String {
    let mut out = Vec::new();
    if let Some(f) = &s.font {
        if f.bold {
            out.push("font-weight:bold".to_string());
        }
        if f.italic {
            out.push("font-style:italic".to_string());
        }
        if let Some(c) = &f.color {
            out.push(format!("color:#{}", normalize(c)));
        }
        if let Some(sz) = f.size {
            out.push(format!("font-size:{}pt", trim(sz)));
        }
    }
    if let Some(fill) = &s.fill {
        if fill == "GRAY125" {
            out.push("background:#bfbfbf".to_string());
        } else {
            out.push(format!("background:#{}", normalize(fill)));
        }
    }
    if let Some(a) = &s.alignment {
        if let Some(h) = &a.horizontal {
            out.push(format!("text-align:{h}"));
        }
        if a.wrap_text {
            out.push("white-space:normal".to_string());
        }
    }
    if s.border.is_some() {
        out.push("border:1px solid #999".to_string());
    }
    out.join(";")
}

fn normalize(c: &str) -> String {
    let s = c.trim().trim_start_matches('#').to_ascii_uppercase();
    if s.len() == 8 {
        s[2..].to_string()
    } else {
        s
    }
}

fn trim(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_formats() {
        assert_eq!(format_number(0.131, "0.0%"), "13.1%");
        assert_eq!(format_number(0.1099, "0.0%"), "11.0%");
        assert_eq!(format_number(27269358.0, "#,##0"), "27,269,358");
        assert_eq!(format_number(1234.5, "#,##0.00"), "1,234.50");
        assert_eq!(format_number(42.0, "0"), "42");
    }

    #[test]
    fn number_formats_sections_and_escapes() {
        // 分段：负数走第二段（括号会计样式）
        assert_eq!(format_number(-50.0, "0.00;(0.00)"), "(50.00)");
        assert_eq!(format_number(50.0, "0.00;(0.00)"), "50.00");
        // Excel 会计格式（_ 占位、\( \) 转义、零值段）
        let acct = r#"_(* #,##0.00_);_(* \(#,##0.00\);_(* "-"??_);_(@_)"#;
        assert_eq!(format_number(123.0, acct), " 123.00 ");
        // 负值段结尾没有 `_)`，故只有前导空格（与 Excel 一致）
        assert_eq!(format_number(-50.0, acct), " (50.00)");
        // 零值段：`_(` + `"-"` + `??`(两位占位) + `_)`
        assert_eq!(format_number(0.0, acct), " -   ");
        // 引号字面量 + 货币符号
        assert_eq!(format_number(12.5, "\"$\"#,##0.00"), "$12.50");
        // 颜色修饰符
        let (t, c) = format_number_ex(-5.0, "[Red]-0.00");
        assert_eq!(t, "-5.00");
        assert_eq!(c.as_deref(), Some("red"));
        // 单段格式的负数：自动补负号
        assert_eq!(format_number(-7.5, "#,##0.00"), "-7.50");
        // `#` 可选位 / `?` 占位 / 可选小数
        assert_eq!(format_number(1.0, "0.##"), "1");
        assert_eq!(format_number(1.5, "0.##"), "1.5");
        assert_eq!(format_number(12.0, "??"), "12");
        assert_eq!(format_number(0.0, "??"), "  ");
        assert_eq!(format_number(1234.0, "#,##0"), "1,234");
    }

    #[test]
    fn media_paths_resolve() {
        assert_eq!(
            resolve_media(None, "https://example.com/a.png").as_deref(),
            Some("https://example.com/a.png")
        );
        assert!(resolve_media(None, "xl/media/missing.png").is_none());
        assert!(media_url("/tmp/a b.png").starts_with("file:///tmp/a%20b.png"));
    }

    #[test]
    fn label_truncation() {
        assert_eq!(truncate_label("United States of America", 9), "United St…");
        assert_eq!(truncate_label("Canada", 9), "Canada");
    }
}
