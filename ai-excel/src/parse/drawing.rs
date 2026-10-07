//! Parse drawing parts (images + charts) back into the model.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::model::sheet::{Chart, Column, Image, RawXml, Row, Size};
use crate::utils::a1::{make_cell_ref, parse_range};
use crate::utils::xml::normalize_color;

const EMU_PER_INCH: f64 = 914400.0;

/// Resolve a relationship target against a base part directory, collapsing
/// `.` / `..` segments. Absolute targets (leading `/`) are returned as-is.
pub fn resolve_part(base: &str, target: &str) -> String {
    if let Some(stripped) = target.strip_prefix('/') {
        return stripped.to_string();
    }
    let mut parts: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

pub fn basename(part: &str) -> String {
    part.rsplit('/').next().unwrap_or(part).to_string()
}

fn attrs_of(e: &BytesStart) -> Vec<(String, String)> {
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

fn get<'a>(a: &'a [(String, String)], k: &str) -> Option<&'a str> {
    a.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str())
}

#[derive(Default, Clone)]
pub struct AnchorItem {
    pub col: u32,
    pub row: u32,
    pub col_off: i64,
    pub row_off: i64,
    pub cx: i64,
    pub cy: i64,
    pub to_col: i64,
    pub to_row: i64,
    pub to_col_off: i64,
    pub to_row_off: i64,
    pub two_cell: bool,
    pub embed: Option<String>,
    pub chart: Option<String>,
}

/// Extract oneCellAnchor items from a drawing part.
pub fn parse_anchor_items(xml: &str) -> Vec<AnchorItem> {
    let mut reader = Reader::from_str(xml);
    let mut items = Vec::new();
    let mut cur: Option<AnchorItem> = None;
    let mut field: Option<Vec<u8>> = None;
    let mut section: u8 = 0; // 1 = from, 2 = to
    let mut buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.local_name().as_ref().to_vec();
                let a = attrs_of(&e);
                match name.as_slice() {
                    b"oneCellAnchor" => cur = Some(AnchorItem::default()),
                    b"twoCellAnchor" => {
                        cur = Some(AnchorItem {
                            two_cell: true,
                            ..Default::default()
                        })
                    }
                    b"from" => section = 1,
                    b"to" => section = 2,
                    b"col" | b"colOff" | b"row" | b"rowOff" => {
                        field = Some(name);
                        buf.clear();
                    }
                    b"ext" => {
                        if let Some(c) = cur.as_mut() {
                            c.cx = get(&a, "cx").and_then(|v| v.parse().ok()).unwrap_or(0);
                            c.cy = get(&a, "cy").and_then(|v| v.parse().ok()).unwrap_or(0);
                        }
                    }
                    b"blip" => {
                        if let Some(c) = cur.as_mut() {
                            c.embed = get(&a, "embed").map(|s| s.to_string());
                        }
                    }
                    b"chart" => {
                        if let Some(c) = cur.as_mut() {
                            c.chart = get(&a, "id").map(|s| s.to_string());
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) if field.is_some() => {
                if let Ok(t) = e.unescape() {
                    buf.push_str(&t);
                }
            }
            Ok(Event::End(e)) => {
                let name = e.local_name().as_ref().to_vec();
                match name.as_slice() {
                    b"col" | b"colOff" | b"row" | b"rowOff" => {
                        if let Some(c) = cur.as_mut() {
                            let v: i64 = buf.trim().parse().unwrap_or(0);
                            match section {
                                1 => match name.as_slice() {
                                    b"col" => c.col = v as u32,
                                    b"row" => c.row = v as u32,
                                    b"colOff" => c.col_off = v,
                                    b"rowOff" => c.row_off = v,
                                    _ => {}
                                },
                                2 => match name.as_slice() {
                                    b"col" => c.to_col = v,
                                    b"row" => c.to_row = v,
                                    b"colOff" => c.to_col_off = v,
                                    b"rowOff" => c.to_row_off = v,
                                    _ => {}
                                },
                                _ => {}
                            }
                        }
                        field = None;
                    }
                    b"from" | b"to" => section = 0,
                    b"oneCellAnchor" | b"twoCellAnchor" => {
                        if let Some(c) = cur.take() {
                            items.push(c);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    items
}

/// 用工作表的实际列宽/行高，把 twoCellAnchor 的 from/to 换算为尺寸（EMU）。
pub fn size_two_cell(
    item: &mut AnchorItem,
    columns: &[Column],
    rows: &[Row],
    default_col: Option<f64>,
    default_row: Option<f64>,
) {
    if !item.two_cell {
        return;
    }
    const EMU_PER_PX: f64 = 9525.0;
    let col_px = |c: u32| -> f64 {
        let w = columns
            .get(c as usize)
            .and_then(|x| x.width)
            .filter(|w| *w > 0.0)
            .or(default_col)
            .filter(|w| *w > 0.0)
            .unwrap_or(8.43);
        w * 7.0 + 5.0
    };
    let row_px = |r: u32| -> f64 {
        let h = rows
            .iter()
            .find(|rr| rr.index == Some(r + 1))
            .and_then(|rr| rr.height)
            .filter(|h| *h > 0.0)
            .or(default_row)
            .filter(|h| *h > 0.0)
            .unwrap_or(15.0);
        h * 96.0 / 72.0
    };
    let off_x = |col: u32, off: i64| -> f64 {
        let mut x = 0.0;
        for c in 0..col {
            x += col_px(c);
        }
        x * EMU_PER_PX + off as f64
    };
    let off_y = |row: u32, off: i64| -> f64 {
        let mut y = 0.0;
        for r in 0..row {
            y += row_px(r);
        }
        y * EMU_PER_PX + off as f64
    };
    let cx = off_x(item.to_col.max(0) as u32, item.to_col_off) - off_x(item.col, item.col_off);
    let cy = off_y(item.to_row.max(0) as u32, item.to_row_off) - off_y(item.row, item.row_off);
    // 若锚点已带显式尺寸（如图片 spPr 的 <a:ext> 原生尺寸），优先保留，避免拉伸变形。
    if item.cx == 0 && cx > 0.0 {
        item.cx = cx.round() as i64;
    }
    if item.cy == 0 && cy > 0.0 {
        item.cy = cy.round() as i64;
    }
}

/// 提取只含形状/连接线（`sp`/`cxnSp`）的锚点块原始 XML，回写时原样输出，
/// 以保留箭头/立方体/椭圆等本工具不建模的绘图。
pub fn extract_shape_blocks(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tag in ["twoCellAnchor", "oneCellAnchor", "absoluteAnchor"] {
        let open = format!("<xdr:{tag}");
        let close = format!("</xdr:{tag}>");
        let mut pos = 0;
        while let Some(s) = xml[pos..].find(&open) {
            let start = pos + s;
            let Some(e) = xml[start..].find(&close) else {
                break;
            };
            let end = start + e + close.len();
            let block = &xml[start..end];
            let is_shape = block.contains("<xdr:sp ")
                || block.contains("<xdr:sp>")
                || block.contains("<xdr:cxnSp");
            let is_pic = block.contains("<xdr:pic");
            let is_frame = block.contains("<xdr:graphicFrame");
            if is_shape && !is_pic && !is_frame {
                out.push(block.to_string());
            }
            pos = end;
        }
    }
    out
}

pub fn anchor_image(item: &AnchorItem, src: String) -> Image {
    Image {
        src,
        anchor: Some(make_cell_ref(item.col + 1, item.row + 1)),
        size: Some(Size {
            w: emu(item.cx),
            h: emu(item.cy),
        }),
        offset: offset_of(item),
    }
}

pub fn apply_anchor_to_chart(chart: &mut Chart, item: &AnchorItem) {
    chart.anchor = Some(make_cell_ref(item.col + 1, item.row + 1));
    chart.size = Some(Size {
        w: emu(item.cx),
        h: emu(item.cy),
    });
    chart.offset = offset_of(item);
}

/// `[dx, dy]`（EMU），为 0 时省略。
fn offset_of(item: &AnchorItem) -> Option<[i64; 2]> {
    if item.col_off == 0 && item.row_off == 0 {
        None
    } else {
        Some([item.col_off, item.row_off])
    }
}

fn emu(v: i64) -> f64 {
    (v as f64 / EMU_PER_INCH * 1000.0).round() / 1000.0
}

/// Parse a chart part into a minimal Chart (type + ranges + title).
pub fn parse_chart(xml: &str) -> Option<Chart> {
    let mut reader = Reader::from_str(xml);
    let mut chart_type: Option<String> = None;
    let mut bar_dir: Option<String> = None;
    let mut cat_f: Option<String> = None;
    let mut val_fs: Vec<String> = Vec::new();
    let mut title: Option<String> = None;
    let mut legend: Option<String> = None;
    let mut cat_title: Option<String> = None;
    let mut val_title: Option<String> = None;
    let mut show_values = false;
    let mut colors: Vec<String> = Vec::new();

    let mut kind: Option<u8> = None; // 1 = cat, 2 = val
    let mut axis: Option<u8> = None; // 1 = catAx, 2 = valAx
    let mut in_title = false;
    let mut text_tag: Option<u8> = None; // 1 = f, 2 = a:t
    let mut buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.local_name().as_ref().to_vec();
                match name.as_slice() {
                    b"barChart" => {
                        chart_type.get_or_insert_with(|| "column".into());
                    }
                    b"lineChart" => {
                        chart_type.get_or_insert_with(|| "line".into());
                    }
                    b"areaChart" => {
                        chart_type.get_or_insert_with(|| "area".into());
                    }
                    b"pieChart" => {
                        chart_type.get_or_insert_with(|| "pie".into());
                    }
                    b"ringChart" => {
                        chart_type.get_or_insert_with(|| "ring".into());
                    }
                    b"scatterChart" => {
                        chart_type.get_or_insert_with(|| "scatter".into());
                    }
                    // 其它图表类型（bubble/radar/stock/surface/doughnut…）：
                    // 记录类型名，便于借助原始 XML 回写保留。
                    other
                        if chart_type.is_none() && other.len() > 5 && other.ends_with(b"Chart") =>
                    {
                        let base =
                            String::from_utf8_lossy(&other[..other.len() - 5]).to_lowercase();
                        chart_type = Some(base);
                    }
                    b"barDir" => {
                        let a = attrs_of(&e);
                        bar_dir = get(&a, "val").map(|s| s.to_string());
                    }
                    b"cat" | b"xVal" => kind = Some(1),
                    b"val" | b"yVal" => kind = Some(2),
                    b"catAx" => axis = Some(1),
                    b"valAx" => axis = Some(2),
                    b"legendPos" => {
                        let a = attrs_of(&e);
                        legend = get(&a, "val").map(|s| match s {
                            "l" => "left".to_string(),
                            "t" => "top".to_string(),
                            "b" => "bottom".to_string(),
                            _ => "right".to_string(),
                        });
                    }
                    b"showVal" => {
                        let a = attrs_of(&e);
                        if matches!(get(&a, "val"), Some("1") | Some("true")) {
                            show_values = true;
                        }
                    }
                    b"srgbClr" => {
                        let a = attrs_of(&e);
                        if let Some(v) = get(&a, "val") {
                            colors.push(normalize_color(v));
                        }
                    }
                    b"title" => in_title = true,
                    b"f" => {
                        text_tag = Some(1);
                        buf.clear();
                    }
                    b"t" if in_title => {
                        text_tag = Some(2);
                        buf.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) if text_tag.is_some() => {
                if let Ok(t) = e.unescape() {
                    buf.push_str(&t);
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"cat" | b"xVal" => kind = None,
                b"val" | b"yVal" => kind = None,
                b"catAx" | b"valAx" => axis = None,
                b"title" => in_title = false,
                b"f" => {
                    let formula = strip_formula(&buf);
                    match kind {
                        Some(1) if cat_f.is_none() => cat_f = Some(formula),
                        Some(2) if !val_fs.contains(&formula) => val_fs.push(formula),
                        _ => {}
                    }
                    text_tag = None;
                }
                b"t" => {
                    if text_tag == Some(2) && !buf.is_empty() {
                        match axis {
                            Some(1) if cat_title.is_none() => cat_title = Some(buf.clone()),
                            Some(2) if val_title.is_none() => val_title = Some(buf.clone()),
                            _ => {
                                if title.is_none() {
                                    title = Some(buf.clone());
                                }
                            }
                        }
                    }
                    text_tag = None;
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    let mut chart_type = chart_type.unwrap_or_else(|| "unknown".into());
    if chart_type == "column" && bar_dir.as_deref() == Some("bar") {
        chart_type = "bar".into();
    }
    Some(Chart {
        chart_type,
        data_range: combine_val_ranges(&val_fs),
        categories: cat_f,
        title,
        legend,
        category_axis_title: cat_title,
        value_axis_title: val_title,
        show_values,
        colors,
        anchor: None,
        size: None,
        offset: None,
        value_axis: None,
        category_axis: None,
        series: Vec::new(),
        raw: RawXml::from(xml),
        snapshot: RawXml(None),
    })
}

/// Merge per-series value ranges (one column each) into a single range.
fn combine_val_ranges(ranges: &[String]) -> Option<String> {
    match ranges.len() {
        0 => None,
        1 => Some(ranges[0].clone()),
        _ => {
            let mut min_col = u32::MAX;
            let mut max_col = 0u32;
            let mut min_row = u32::MAX;
            let mut max_row = 0u32;
            for r in ranges {
                if let Ok(((c1, r1), (c2, r2))) = parse_range(r) {
                    min_col = min_col.min(c1);
                    max_col = max_col.max(c2);
                    min_row = min_row.min(r1);
                    max_row = max_row.max(r2);
                }
            }
            if min_col == u32::MAX {
                return Some(ranges[0].clone());
            }
            Some(format!(
                "{}:{}",
                make_cell_ref(min_col, min_row),
                make_cell_ref(max_col, max_row)
            ))
        }
    }
}

/// "Sheet1!$B$2:$B$8" -> "B2:B8".
fn strip_formula(f: &str) -> String {
    let s = f.trim();
    // 并集/多区间形如 `(r1,r2)`：剥一层括号。
    let s = if s.starts_with('(') && s.ends_with(')') {
        &s[1..s.len() - 1]
    } else {
        s
    };
    // 取最后一个不在单引号内的 `!` 之后的部分（工作表名可能含 , ( ) ' 等）。
    let r = match last_bang_outside_quotes(s) {
        Some(i) => &s[i + 1..],
        None => s,
    };
    r.replace('$', "")
}

fn last_bang_outside_quotes(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut in_q = false;
    let mut last = None;
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'\'' => {
                if in_q && i + 1 < b.len() && b[i + 1] == b'\'' {
                    i += 2;
                    continue;
                }
                in_q = !in_q;
            }
            b'!' if !in_q => last = Some(i),
            _ => {}
        }
        i += 1;
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_anchor_items_basic() {
        let xml = "<?xml version=\"1.0\"?><xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><xdr:oneCellAnchor><xdr:from><xdr:col>2</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>2</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:ext cx=\"1828800\" cy=\"1371600\"/><xdr:pic><xdr:nvPicPr><xdr:cNvPr id=\"1\" name=\"Picture 1\"/><xdr:cNvPicPr/></xdr:nvPicPr><xdr:blipFill><a:blip xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:embed=\"rId1\"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill></xdr:pic><xdr:clientData/></xdr:oneCellAnchor></xdr:wsDr>";
        let items = parse_anchor_items(xml);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].col, 2);
        assert_eq!(items[0].row, 2);
        assert_eq!(items[0].embed.as_deref(), Some("rId1"));
        assert_eq!(items[0].cx, 1828800);
    }

    #[test]
    fn parse_two_cell_anchor() {
        let xml = "<?xml version=\"1.0\"?><xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"><xdr:twoCellAnchor><xdr:from><xdr:col>4</xdr:col><xdr:colOff>476250</xdr:colOff><xdr:row>17</xdr:row><xdr:rowOff>14287</xdr:rowOff></xdr:from><xdr:to><xdr:col>11</xdr:col><xdr:colOff>247650</xdr:colOff><xdr:row>32</xdr:row><xdr:rowOff>166687</xdr:rowOff></xdr:to><xdr:graphicFrame><a:graphic><a:graphicData uri=\"chart\"><c:chart xmlns:c=\"c\" r:id=\"rId1\"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor><xdr:twoCellAnchor><xdr:from><xdr:col>1</xdr:col><xdr:row>1</xdr:row></xdr:from><xdr:to><xdr:col>3</xdr:col><xdr:row>6</xdr:row></xdr:to><xdr:pic><xdr:blipFill><a:blip r:embed=\"rId2\"/></xdr:blipFill></xdr:pic></xdr:twoCellAnchor></xdr:wsDr>";
        let items = parse_anchor_items(xml);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].col, 4);
        assert_eq!(items[0].row, 17);
        assert_eq!(items[0].chart.as_deref(), Some("rId1"));
        assert!(items[0].two_cell);
        assert_eq!(items[0].to_col, 11);
        assert_eq!(items[0].to_row, 32);
        assert_eq!(items[1].embed.as_deref(), Some("rId2"));
        assert_eq!(items[1].col, 1);

        // 用默认列宽/行高换算尺寸
        let mut it = items[0].clone();
        size_two_cell(&mut it, &[], &[], None, None);
        assert!(it.cx > 0 && it.cy > 0);
    }

    #[test]
    fn extract_shapes_only() {
        let xml = "<xdr:wsDr xmlns:xdr=\"d\" xmlns:a=\"a\">\
            <xdr:twoCellAnchor><xdr:from><xdr:col>0</xdr:col><xdr:row>0</xdr:row></xdr:from><xdr:to><xdr:col>1</xdr:col><xdr:row>1</xdr:row></xdr:to><xdr:sp><a:prstGeom prst=\"ellipse\"/></xdr:sp><xdr:clientData/></xdr:twoCellAnchor>\
            <xdr:twoCellAnchor><xdr:from><xdr:col>2</xdr:col><xdr:row>2</xdr:row></xdr:from><xdr:to><xdr:col>3</xdr:col><xdr:row>3</xdr:row></xdr:to><xdr:pic><xdr:blipFill><a:blip r:embed=\"rId1\"/></xdr:blipFill></xdr:pic></xdr:twoCellAnchor>\
            </xdr:wsDr>";
        let shapes = extract_shape_blocks(xml);
        assert_eq!(shapes.len(), 1);
        assert!(shapes[0].contains("<xdr:sp>"));
        assert!(!shapes[0].contains("<xdr:pic"));
    }

    #[test]
    fn strip_formula_variants() {
        assert_eq!(strip_formula("Sheet1!$A$1:$A$5"), "A1:A5");
        assert_eq!(
            strip_formula("(Sheet1!$A$1:$A$2,Sheet1!$A$4:$A$5)"),
            "A4:A5"
        );
        assert_eq!(strip_formula("'Sheet,6'!$A$1:$A$5"), "A1:A5");
        assert_eq!(strip_formula("'Sheet''1'!$A$1:$A$5"), "A1:A5");
    }

    #[test]
    fn chart_type_and_ranges() {
        let xml = "<?xml version=\"1.0\"?><c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><c:chart><c:plotArea><c:barChart><c:barDir val=\"col\"/><c:ser><c:cat><c:strRef><c:f>Data!$A$2:$A$4</c:f></c:strRef></c:cat><c:val><c:numRef><c:f>Data!$B$2:$B$4</c:f></c:numRef></c:val></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>";
        let c = parse_chart(xml).unwrap();
        assert_eq!(c.chart_type, "column");
        assert_eq!(c.data_range.as_deref(), Some("B2:B4"));
        assert_eq!(c.categories.as_deref(), Some("A2:A4"));
    }
}
