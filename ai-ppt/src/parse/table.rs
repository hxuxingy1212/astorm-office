//! 表格元素解析
//!
//! 从 p:graphicFrame 节点解析表格（行列、单元格样式、合并单元格、列宽）。

use crate::model::elements::*;
use crate::utils::constants::emu_to_inch;
use quick_xml::events::Event;
use quick_xml::Reader;

/// 物理单元格（含合并信息，OOXML 表格中的原始形态）
struct PhysicalCell {
    cell: Cell,
    grid_span: Option<u32>,
    row_span: Option<u32>,
    v_merge: bool,
    h_merge: bool,
}

/// 读取 a:tcPr 的属性（内边距 / 竖排 / 垂直对齐）——适用于 Start 与自闭合 Empty
fn read_tcpr_attrs(
    e: &quick_xml::events::BytesStart,
    anchor: &mut Option<String>,
    margin: &mut Option<[f64; 4]>,
    vert: &mut Option<String>,
) {
    let mut mar = [0.0_f64; 4];
    let mut has_mar = false;
    for a in e.attributes().flatten() {
        let v = String::from_utf8_lossy(&a.value).to_string();
        match a.key.as_ref() {
            b"marL" => {
                if let Ok(x) = v.parse::<i64>() {
                    mar[0] = emu_to_inch(x);
                    has_mar = true;
                }
            }
            b"marT" => {
                if let Ok(x) = v.parse::<i64>() {
                    mar[1] = emu_to_inch(x);
                    has_mar = true;
                }
            }
            b"marR" => {
                if let Ok(x) = v.parse::<i64>() {
                    mar[2] = emu_to_inch(x);
                    has_mar = true;
                }
            }
            b"marB" => {
                if let Ok(x) = v.parse::<i64>() {
                    mar[3] = emu_to_inch(x);
                    has_mar = true;
                }
            }
            b"vert" if v != "horz" => *vert = Some(v),
            b"anchor" => *anchor = Some(v),
            _ => {}
        }
    }
    if has_mar {
        *margin = Some(mar);
    }
}

pub(super) fn parse_graphic_frame(
    reader: &mut Reader<&[u8]>,
    r_id_map: &std::collections::HashMap<String, String>,
    charts: &std::collections::HashMap<String, String>,
    slide_xml: &str,
    elem_start: usize,
) -> Option<Element> {
    let mut depth = 1u32;
    let mut position = Position {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };
    let mut name = None;
    let mut id = None;
    // 图表：graphicData uri 以 /chart 结尾，c:chart r:id 指向 chart 部件
    let mut is_chart = false;
    let mut chart_rid = String::new();
    let mut header_row = false;
    let mut column_widths: Option<Vec<f64>> = None;
    let mut physical: Vec<Vec<PhysicalCell>> = Vec::new();
    let mut current_cells: Vec<PhysicalCell> = Vec::new();
    let mut cell_text = String::new();
    let mut cell_font_size = None;
    let mut cell_bold = false;
    let mut cell_color = None;
    let mut cell_fill = None;
    let mut cell_align = None;
    let mut grid_span = None;
    let mut row_span = None;
    let mut v_merge = false;
    let mut h_merge = false;
    let mut in_tc = false;
    let mut in_tc_pr = false;
    let mut style_id: Option<String> = None;
    let mut tbl_attrs: Option<String> = None;
    let mut in_table_style_id = false;
    let mut rtl = false;
    let mut cell_border_color: Option<String> = None;
    let mut cell_border_width = 1.0f64;
    let mut in_cell_border = false;
    let mut cell_vert: Option<String> = None;
    let mut cell_margin: Option<[f64; 4]> = None;
    let mut cell_fill_image: Option<String> = None;
    let mut cell_anchor: Option<String> = None;
    let mut in_tc_blip = false;
    let mut row_heights: Vec<f64> = Vec::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let n = super::tag(e).to_string();
                match n.as_str() {
                    "p:cNvPr" => {
                        name = super::parse_name_attr(e);
                        id = super::parse_id_attr(e);
                    }
                    "graphicData" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"uri" {
                                let uri = String::from_utf8_lossy(&a.value).to_string();
                                if uri.ends_with("/chart") {
                                    is_chart = true;
                                }
                            }
                        }
                    }
                    "chart" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"r:id" {
                                chart_rid = String::from_utf8_lossy(&a.value).to_string();
                            }
                        }
                        is_chart = true;
                    }
                    "a:xfrm" => {}
                    "a:off" => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:ext" => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:tbl" => {}
                    "a:tblPr" => {
                        let mut attrs = String::new();
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "firstRow" && v == "1" {
                                header_row = true;
                            }
                            if k == "rtl" && (v == "1" || v == "true") {
                                rtl = true;
                            }
                            attrs.push_str(&format!(" {}=\"{}\"", k, v));
                        }
                        if !attrs.trim().is_empty() {
                            tbl_attrs = Some(attrs.trim().to_string());
                        }
                    }
                    "a:lnL" | "a:lnR" | "a:lnT" | "a:lnB" => {
                        in_cell_border = true;
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"w" {
                                if let Ok(w) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                                    cell_border_width = w as f64 / 12700.0;
                                }
                            }
                        }
                    }
                    "tableStyleId" | "a:tableStyleId" => {
                        in_table_style_id = true;
                    }
                    "a:tr" => {
                        current_cells = Vec::new();
                        let mut h = 0.0;
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"h" {
                                if let Ok(x) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                                    h = emu_to_inch(x);
                                }
                            }
                        }
                        row_heights.push(h);
                    }
                    "a:tc" => {
                        in_tc = true;
                        cell_text.clear();
                        cell_font_size = None;
                        cell_bold = false;
                        cell_color = None;
                        cell_fill = None;
                        cell_align = None;
                        grid_span = None;
                        row_span = None;
                        v_merge = false;
                        h_merge = false;
                        cell_border_color = None;
                        cell_border_width = 1.0;
                        in_cell_border = false;
                        cell_vert = None;
                        cell_margin = None;
                        cell_fill_image = None;
                        cell_anchor = None;
                        in_tc_blip = false;
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "gridSpan" {
                                if let Ok(s) = v.parse::<u32>() {
                                    grid_span = Some(s);
                                }
                            }
                            if k == "rowSpan" {
                                if let Ok(s) = v.parse::<u32>() {
                                    row_span = Some(s);
                                }
                            }
                        }
                    }
                    "a:tblGrid" => {
                        column_widths = Some(Vec::new());
                    }
                    "a:gridCol" => {
                        if let Some(ref mut widths) = column_widths {
                            for a in e.attributes().flatten() {
                                let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                                let v = String::from_utf8_lossy(&a.value).to_string();
                                if k == "w" {
                                    if let Ok(w) = v.parse::<i64>() {
                                        widths.push(emu_to_inch(w));
                                    }
                                }
                            }
                        }
                    }
                    "a:pPr" if in_tc => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "algn" {
                                cell_align = Some(match v.as_str() {
                                    "ctr" => "center".to_string(),
                                    "r" => "right".to_string(),
                                    "just" => "justify".to_string(),
                                    _ => "left".to_string(),
                                });
                            }
                        }
                    }
                    "a:tcPr" => {
                        in_tc_pr = true;
                        read_tcpr_attrs(e, &mut cell_anchor, &mut cell_margin, &mut cell_vert);
                    }
                    "a:blipFill" if in_tc_pr => {
                        in_tc_blip = true;
                    }
                    "a:blip" if in_tc_blip => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"r:embed" || a.key.as_ref() == b"r:link" {
                                let rid = String::from_utf8_lossy(&a.value).to_string();
                                cell_fill_image = r_id_map.get(&rid).cloned();
                            }
                        }
                    }
                    "a:solidFill" if in_tc_pr => {
                        let mut inner_buf = Vec::new();
                        if let Ok(Event::Empty(ref clr)) = reader.read_event_into(&mut inner_buf) {
                            let cn = super::tag(clr).to_string();
                            if cn == "a:srgbClr" || cn == "a:schemeClr" {
                                for a in clr.attributes().flatten() {
                                    if String::from_utf8_lossy(a.key.as_ref()) == "val" {
                                        let val = String::from_utf8_lossy(&a.value).to_string();
                                        if in_cell_border {
                                            cell_border_color = Some(val);
                                        } else {
                                            cell_fill = Some(val);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "a:rPr" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "sz" {
                                if let Ok(s) = v.parse::<i64>() {
                                    cell_font_size = Some(s as f64 / 100.0);
                                }
                            }
                            if k == "b" {
                                cell_bold = v == "1";
                            }
                        }
                        let mut inner_buf = Vec::new();
                        loop {
                            match reader.read_event_into(&mut inner_buf) {
                                Ok(Event::Empty(ref child)) => {
                                    let cn = super::tag(child).to_string();
                                    if cn == "a:srgbClr" || cn == "a:schemeClr" {
                                        for a in child.attributes().flatten() {
                                            if String::from_utf8_lossy(a.key.as_ref()) == "val" {
                                                cell_color = Some(
                                                    String::from_utf8_lossy(&a.value).to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                                Ok(Event::Start(ref child)) => {
                                    let cn = super::tag(child).to_string();
                                    if cn == "a:solidFill" {
                                        let mut fill_buf = Vec::new();
                                        if let Ok(Event::Empty(ref clr)) =
                                            reader.read_event_into(&mut fill_buf)
                                        {
                                            let clr_n = super::tag(clr).to_string();
                                            if clr_n == "a:srgbClr" || clr_n == "a:schemeClr" {
                                                for a in clr.attributes().flatten() {
                                                    if String::from_utf8_lossy(a.key.as_ref())
                                                        == "val"
                                                    {
                                                        cell_color = Some(
                                                            String::from_utf8_lossy(&a.value)
                                                                .to_string(),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Ok(Event::End(ref end)) if super::tag(end) == "a:rPr" => {
                                    break;
                                }
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                            inner_buf.clear();
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let n = super::tag(e).to_string();
                match n.as_str() {
                    "chart" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"r:id" {
                                chart_rid = String::from_utf8_lossy(&a.value).to_string();
                            }
                        }
                        is_chart = true;
                    }
                    "graphicData" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"uri"
                                && String::from_utf8_lossy(&a.value).ends_with("/chart")
                            {
                                is_chart = true;
                            }
                        }
                    }
                    "a:pPr" if in_tc => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "algn" {
                                cell_align = Some(
                                    match v.as_str() {
                                        "ctr" => "center",
                                        "r" => "right",
                                        "just" => "justify",
                                        _ => "left",
                                    }
                                    .to_string(),
                                );
                            }
                        }
                    }
                    "p:cNvPr" => {
                        name = super::parse_name_attr(e);
                        id = super::parse_id_attr(e);
                    }
                    "a:off" => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:ext" => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:gridCol" => {
                        if let Some(ref mut widths) = column_widths {
                            for a in e.attributes().flatten() {
                                let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                                let v = String::from_utf8_lossy(&a.value).to_string();
                                if k == "w" {
                                    if let Ok(w) = v.parse::<i64>() {
                                        widths.push(emu_to_inch(w));
                                    }
                                }
                            }
                        }
                    }
                    "a:tcPr" => {
                        // 仅含属性的自闭合 tcPr
                        read_tcpr_attrs(e, &mut cell_anchor, &mut cell_margin, &mut cell_vert);
                    }
                    "a:blip" if in_tc_blip => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"r:embed" || a.key.as_ref() == b"r:link" {
                                let rid = String::from_utf8_lossy(&a.value).to_string();
                                cell_fill_image = r_id_map.get(&rid).cloned();
                            }
                        }
                    }
                    "a:vMerge" if in_tc_pr => {
                        v_merge = true;
                    }
                    "a:hMerge" if in_tc_pr => {
                        h_merge = true;
                    }
                    "a:tblStyle" => {
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"val" {
                                style_id = Some(String::from_utf8_lossy(&a.value).to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref t)) if in_table_style_id => {
                if let Ok(txt) = t.unescape() {
                    let s = txt.trim();
                    if !s.is_empty() {
                        style_id = Some(s.to_string());
                    }
                }
            }
            Ok(Event::Text(ref t)) if in_tc && !in_tc_pr => {
                let text_content = t
                    .unescape()
                    .unwrap_or_else(|_| std::str::from_utf8(t.as_ref()).unwrap_or("").into())
                    .to_string();
                cell_text.push_str(&text_content);
            }
            Ok(Event::End(ref e)) => {
                let n = super::tag(e).to_string();
                match n.as_str() {
                    "a:tc" => {
                        in_tc = false;
                        current_cells.push(PhysicalCell {
                            cell: Cell {
                                text: cell_text.trim().to_string(),
                                font_size: cell_font_size,
                                bold: if cell_bold { Some(true) } else { None },
                                color: cell_color.clone(),
                                fill: cell_fill.clone(),
                                align: cell_align.clone(),
                                colspan: None,
                                rowspan: None,
                                border: cell_border_color.clone().map(|c| CellBorder {
                                    color: c,
                                    width: cell_border_width,
                                }),
                                vert: cell_vert.clone(),
                                margin: cell_margin,
                                fill_image: cell_fill_image.clone(),
                                anchor: cell_anchor.clone(),
                            },
                            grid_span,
                            row_span,
                            v_merge,
                            h_merge,
                        });
                    }
                    "a:tcPr" => {
                        in_tc_pr = false;
                        in_tc_blip = false;
                    }
                    "a:blipFill" => {
                        in_tc_blip = false;
                    }
                    "a:lnL" | "a:lnR" | "a:lnT" | "a:lnB" => {
                        in_cell_border = false;
                    }
                    "a:tr" if !current_cells.is_empty() => {
                        physical.push(std::mem::take(&mut current_cells));
                    }
                    "tableStyleId" | "a:tableStyleId" => {
                        in_table_style_id = false;
                    }
                    "a:tbl" => {}
                    "p:graphicFrame" => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    // 图表：解析引用的 chart 部件
    if is_chart || !chart_rid.is_empty() {
        if let Some(target) = r_id_map.get(&chart_rid) {
            let path = crate::parse::normalize_part_path("ppt/slides/", target);
            if let Some(cxml) = charts.get(&path) {
                if let Some(mut c) = super::chart::parse(cxml) {
                    c.position = position;
                    c.name = name;
                    c.id = id;
                    c.raw = Some(super::chart::strip_external_data(cxml));
                    return Some(Element::Chart(c));
                }
            }
        }
        return None;
    }

    if physical.is_empty() {
        // 未知 graphicFrame（OLE / SmartArt / 3D / zoom 等）：原样保存以便回写
        let endpos = reader.buffer_position() as usize;
        let xml = if endpos > elem_start && endpos <= slide_xml.len() {
            slide_xml[elem_start..endpos].to_string()
        } else {
            String::new()
        };
        if xml.is_empty() {
            return None;
        }
        let text = super::extract_all_text(&xml);
        return Some(Element::Opaque(OpaqueElement {
            position,
            name,
            id,
            xml,
            rels: Vec::new(),
            parts: Vec::new(),
            text,
        }));
    }

    // 重建逻辑网格：物理格 → 逻辑格
    // - 主格带 gridSpan/rowSpan 属性 → Cell.colspan/rowspan
    // - vMerge/hMerge 占位格合并回主格（rowspan/colspan 计数），占位格不出现在逻辑网格
    let num_cols = match &column_widths {
        Some(w) if !w.is_empty() => w.len(),
        _ => physical.iter().map(|r| r.len()).max().unwrap_or(0),
    };
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut active: Vec<u32> = vec![0; num_cols.max(1)];
    let mut prev_spans: Vec<(usize, usize)> = Vec::new();
    for row in &physical {
        let mut lrow: Vec<Cell> = Vec::new();
        let mut col = 0usize;
        let mut spans: Vec<(usize, usize)> = Vec::new();
        for pc in row {
            if col >= num_cols {
                break;
            }
            if active[col] > 0 {
                // 被上方 rowspan 覆盖的占位格
                active[col] -= 1;
                col += 1;
                continue;
            }
            if pc.v_merge || pc.h_merge {
                // 无 rowSpan 属性、仅 vMerge/hMerge 标记的占位格：回溯主格扩展
                if pc.h_merge {
                    if let Some(last) = lrow.last_mut() {
                        last.colspan = Some(last.colspan.unwrap_or(1) + 1);
                    }
                } else if let Some((gi, _)) = prev_spans
                    .iter()
                    .enumerate()
                    .find(|(_, (s, e))| *s <= col && col < *e)
                {
                    if let Some(prev_row) = rows.last_mut() {
                        if let Some(main) = prev_row.get_mut(gi) {
                            main.rowspan = Some(main.rowspan.unwrap_or(1) + 1);
                        }
                    }
                }
                col += 1;
                continue;
            }
            // 主格
            let mut cell = pc.cell.clone();
            let colspan = pc.grid_span.unwrap_or(1) as usize;
            let rowspan = pc.row_span.unwrap_or(1) as usize;
            if colspan > 1 {
                cell.colspan = Some(colspan as u32);
            }
            if rowspan > 1 {
                cell.rowspan = Some(rowspan as u32);
            }
            spans.push((col, col + colspan));
            lrow.push(cell);
            if rowspan > 1 {
                for cell_cover in active
                    .iter_mut()
                    .take((col + colspan).min(num_cols))
                    .skip(col)
                {
                    *cell_cover = (*cell_cover).max(rowspan as u32 - 1);
                }
            }
            col += colspan;
        }
        // 行尾未消费的覆盖列
        while col < num_cols && active[col] > 0 {
            active[col] -= 1;
            col += 1;
        }
        if !lrow.is_empty() {
            rows.push(lrow);
            prev_spans = spans;
        }
    }

    if rows.is_empty() {
        return None;
    }
    Some(Element::Table(TableElement {
        position,
        name,
        id,
        rows,
        column_widths,
        header_row: if header_row { Some(true) } else { None },
        style_id,
        rtl: if rtl { Some(true) } else { None },
        row_heights: if row_heights.iter().any(|h| *h > 0.0) {
            Some(row_heights)
        } else {
            None
        },
        tbl_attrs,
        font_size: None,
        color: None,
        animations: None,
    }))
}
