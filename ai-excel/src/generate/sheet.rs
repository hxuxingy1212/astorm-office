use serde_json::Value;

use crate::error::Result;
use crate::generate::shared_strings::SharedStrings;
use crate::generate::styles::StyleTable;
use crate::model::cell::Cell;
use crate::model::sheet::{Column, DataValidation, Row, Sheet};
use crate::model::workbook::Workbook;
use crate::utils::a1::{make_cell_ref, parse_cell_ref};
use crate::utils::constants::NS_MAIN;
use crate::utils::date::{date_to_serial, parse_iso_date};
use crate::utils::xml::{esc_xml, to_argb};

/// A hyperlink to emit in the worksheet. External links carry an `rid`
/// (resolved through the sheet's rels); internal anchors carry a `location`.
pub struct Hyperlink {
    pub reference: String,
    pub rid: Option<String>,
    pub location: Option<String>,
    pub tooltip: Option<String>,
}

/// Build the `<worksheet>` part for a sheet, interning strings as needed.
#[allow(clippy::too_many_arguments)]
pub fn worksheet_xml(
    wb: &Workbook,
    sheet: &Sheet,
    styles: &StyleTable,
    sst: &mut SharedStrings,
    has_drawing: bool,
    hyperlinks: &[Hyperlink],
    table_rids: &[String],
    legacy_rid: Option<&str>,
    pivot_rids: &[String],
    slicer_rids: &[String],
    background_rid: Option<&str>,
    ole: &[(String, u32, String)],
    ole_vml_rid: Option<&str>,
) -> Result<String> {
    let mut rows: Vec<&Row> = sheet.rows.iter().collect();
    rows.sort_by_key(|r| r.index.unwrap_or(0));

    // used range
    let mut min_col = u32::MAX;
    let mut min_row = u32::MAX;
    let mut max_col = 0u32;
    let mut max_row = 0u32;
    for row in &rows {
        let r = row.index.unwrap_or(0);
        for (ci, cell) in row.cells.iter().enumerate() {
            let (col, crow) = cell_position(cell, ci, r)?;
            min_col = min_col.min(col);
            min_row = min_row.min(crow);
            max_col = max_col.max(col);
            max_row = max_row.max(crow);
        }
    }
    // Include merge extents so columns in a merge are preserved.
    for m in &sheet.merges {
        if let Ok((_, (c2, r2))) = crate::utils::a1::parse_range(m) {
            if c2 > max_col {
                max_col = c2;
            }
            if r2 > max_row {
                max_row = r2;
            }
        }
    }
    // Include empty-but-present rows/columns and image/chart anchors so the
    // declared used range matches the source (affects pagination/zoom).
    for row in &sheet.rows {
        if let Some(r) = row.index {
            if r > max_row {
                max_row = r;
            }
        }
    }
    if (sheet.columns.len() as u32) > max_col {
        max_col = sheet.columns.len() as u32;
    }
    let dimension = if max_row == 0 {
        "A1".to_string()
    } else {
        format!(
            "{}:{}",
            make_cell_ref(min_col, min_row),
            make_cell_ref(max_col, max_row)
        )
    };

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!(
        "<worksheet xmlns=\"{NS_MAIN}\" xmlns:r=\"{}\">",
        crate::utils::constants::NS_REL
    ));

    let fit_to_page = sheet.print.as_ref().map(|p| p.fit_to_page).unwrap_or(false);
    if sheet.tab_color.is_some() || fit_to_page {
        out.push_str("<sheetPr>");
        if let Some(color) = &sheet.tab_color {
            out.push_str(&format!(
                "<tabColor rgb=\"{}\"/>",
                crate::utils::xml::to_argb(color)
            ));
        }
        if fit_to_page {
            out.push_str("<pageSetUpPr fitToPage=\"1\"/>");
        }
        out.push_str("</sheetPr>");
    }

    out.push_str(&format!("<dimension ref=\"{dimension}\"/>"));

    // sheetViews
    out.push_str("<sheetViews><sheetView workbookViewId=\"0\"");
    if sheet.direction.as_deref() == Some("rtl") {
        out.push_str(" rightToLeft=\"1\"");
    }
    if sheet.gridlines == Some(false) {
        out.push_str(" showGridLines=\"0\"");
    }
    if sheet.headings == Some(false) {
        out.push_str(" showRowColHeaders=\"0\"");
    }
    if let Some(z) = sheet.zoom {
        out.push_str(&format!(" zoomScale=\"{z}\""));
    }
    if sheet.show_formulas {
        out.push_str(" showFormulas=\"1\"");
    }
    if sheet.tab_selected {
        out.push_str(" tabSelected=\"1\"");
    }
    out.push('>');
    if let Some(freeze) = &sheet.freeze {
        if let Ok((col, row)) = parse_cell_ref(freeze) {
            let mut pane = String::new();
            if col > 1 {
                pane.push_str(&format!(" xSplit=\"{}\"", col - 1));
            }
            if row > 1 {
                pane.push_str(&format!(" ySplit=\"{}\"", row - 1));
            }
            if !pane.is_empty() {
                out.push_str(&format!(
                    "<pane{pane} topLeftCell=\"{}\" activePane=\"bottomRight\" state=\"frozen\"/>",
                    make_cell_ref(col, row)
                ));
            }
        }
    }
    out.push_str("</sheetView></sheetViews>");

    // 仅在源文件带有非默认值时输出 sheetFormatPr；否则省略（缺省行高由应用
    // 决定，显式写入 15 会让 LibreOffice 等分页/行高与原文件不一致）。
    if sheet.base_col_width.is_some()
        || sheet.default_col_width.is_some()
        || sheet.default_row_height.is_some()
    {
        let mut fmt_attrs = String::new();
        if let Some(b) = sheet.base_col_width {
            fmt_attrs.push_str(&format!(" baseColWidth=\"{}\"", trim_num(b)));
        }
        if let Some(d) = sheet.default_col_width {
            fmt_attrs.push_str(&format!(" defaultColWidth=\"{}\"", trim_num(d)));
        }
        let row_h = sheet.default_row_height.unwrap_or(15.0);
        fmt_attrs.push_str(&format!(" defaultRowHeight=\"{}\"", trim_num(row_h)));
        out.push_str(&format!("<sheetFormatPr{fmt_attrs}/>"));
    }

    // cols（合并连续相同列；跳过全默认列，避免无谓地声明大量默认列）
    if !sheet.columns.is_empty() {
        let mut cols_body = String::new();
        let mut i = 0usize;
        while i < sheet.columns.len() {
            let start = i;
            let key = col_key(&sheet.columns[i]);
            while i < sheet.columns.len() && col_key(&sheet.columns[i]) == key {
                i += 1;
            }
            let c = &sheet.columns[start];
            let is_default = c.width.is_none()
                && !c.hidden
                && c.outline_level.is_none()
                && !c.collapsed
                && c.style.is_none();
            if is_default {
                continue;
            }
            let mut attrs = format!(" min=\"{}\" max=\"{}\"", start + 1, i);
            if let Some(w) = c.width {
                attrs.push_str(&format!(" width=\"{}\" customWidth=\"1\"", trim_num(w)));
            }
            if c.hidden {
                attrs.push_str(" hidden=\"1\"");
            }
            if let Some(l) = c.outline_level {
                attrs.push_str(&format!(" outlineLevel=\"{l}\""));
            }
            if c.collapsed {
                attrs.push_str(" collapsed=\"1\"");
            }
            if let Some(st) = &c.style {
                if let Some(id) = styles.style_id(st) {
                    attrs.push_str(&format!(" style=\"{id}\""));
                }
            }
            cols_body.push_str(&format!("<col{attrs}/>"));
        }
        if !cols_body.is_empty() {
            out.push_str(&format!("<cols>{cols_body}</cols>"));
        }
    }

    // sheetData
    out.push_str("<sheetData>");
    for row in &rows {
        let r = row.index.unwrap_or(0);
        let mut cells: Vec<(u32, &Cell)> = Vec::new();
        for (ci, cell) in row.cells.iter().enumerate() {
            let (col, _) = cell_position(cell, ci, r)?;
            cells.push((col, cell));
        }
        cells.sort_by_key(|(c, _)| *c);
        let mut attrs = format!(" r=\"{r}\"");
        if let Some(h) = row.height {
            attrs.push_str(&format!(" ht=\"{}\" customHeight=\"1\"", trim_num(h)));
        }
        if row.hidden {
            attrs.push_str(" hidden=\"1\"");
        }
        if let Some(l) = row.outline_level {
            attrs.push_str(&format!(" outlineLevel=\"{l}\""));
        }
        if row.collapsed {
            attrs.push_str(" collapsed=\"1\"");
        }
        if let Some(st) = &row.style {
            if let Some(id) = styles.style_id(st) {
                attrs.push_str(&format!(" s=\"{id}\" customFormat=\"1\""));
            }
        }
        out.push_str(&format!("<row{attrs}>"));
        for (col, cell) in cells {
            out.push_str(&cell_xml(wb, cell, col, r, styles, sst)?);
        }
        out.push_str("</row>");
    }
    out.push_str("</sheetData>");

    if sheet.protect {
        out.push_str("<sheetProtection sheet=\"1\" objects=\"1\" scenarios=\"1\"/>");
    }

    if let Some(af) = &sheet.auto_filter {
        if sheet.filter_columns.is_empty() {
            out.push_str(&format!("<autoFilter ref=\"{}\"/>", esc_xml(af)));
        } else {
            out.push_str(&format!("<autoFilter ref=\"{}\">", esc_xml(af)));
            for fc in &sheet.filter_columns {
                out.push_str(&format!("<filterColumn colId=\"{}\">", fc.col_id));
                if !fc.values.is_empty() {
                    out.push_str("<filters>");
                    for v in &fc.values {
                        out.push_str(&format!("<filter val=\"{}\"/>", esc_xml(v)));
                    }
                    out.push_str("</filters>");
                } else if let Some(c) = &fc.criteria {
                    let op = fc.operator.clone().unwrap_or_else(|| "equal".into());
                    out.push_str("<customFilters>");
                    out.push_str(&format!(
                        "<customFilter operator=\"{}\" val=\"{}\"/>",
                        esc_xml(&op),
                        esc_xml(c)
                    ));
                    if let Some(c2) = &fc.criteria2 {
                        out.push_str(&format!(
                            "<customFilter operator=\"{}\" val=\"{}\"/>",
                            esc_xml(&op),
                            esc_xml(c2)
                        ));
                    }
                    out.push_str("</customFilters>");
                }
                out.push_str("</filterColumn>");
            }
            out.push_str("</autoFilter>");
        }
    }

    if let Some(sort) = &sheet.sort {
        out.push_str(&format!("<sortState ref=\"{}\">", esc_xml(&sort.range)));
        for c in &sort.conditions {
            out.push_str(&format!("<sortCondition ref=\"{}\"", esc_xml(&c.column)));
            if let Some(sb) = &c.sort_on {
                out.push_str(&format!(" sortBy=\"{}\"", esc_xml(sb)));
            }
            if c.descending {
                out.push_str(" descending=\"1\"");
            }
            out.push_str("/>");
        }
        out.push_str("</sortState>");
    }

    if !sheet.merges.is_empty() {
        out.push_str(&format!("<mergeCells count=\"{}\">", sheet.merges.len()));
        for m in &sheet.merges {
            out.push_str(&format!("<mergeCell ref=\"{}\"/>", esc_xml(m)));
        }
        out.push_str("</mergeCells>");
    }

    if !sheet.conditional_formats.is_empty() {
        out.push_str(&conditional_formats_xml(sheet, styles));
    }

    if !sheet.validations.is_empty() {
        out.push_str(&validations_xml(&sheet.validations));
    }

    if !hyperlinks.is_empty() {
        out.push_str("<hyperlinks>");
        for h in hyperlinks {
            let tooltip = h
                .tooltip
                .as_ref()
                .map(|t| format!(" tooltip=\"{}\"", esc_xml(t)))
                .unwrap_or_default();
            if let Some(rid) = &h.rid {
                out.push_str(&format!(
                    "<hyperlink ref=\"{}\" r:id=\"{}\"{tooltip}/>",
                    esc_xml(&h.reference),
                    esc_xml(rid)
                ));
            } else if let Some(loc) = &h.location {
                out.push_str(&format!(
                    "<hyperlink ref=\"{}\" location=\"{}\"{tooltip}/>",
                    esc_xml(&h.reference),
                    esc_xml(loc)
                ));
            }
        }
        out.push_str("</hyperlinks>");
    }

    if let Some(p) = &sheet.print {
        if p.h_center || p.v_center {
            let mut o = String::new();
            if p.h_center {
                o.push_str(" horizontalCentered=\"1\"");
            }
            if p.v_center {
                o.push_str(" verticalCentered=\"1\"");
            }
            out.push_str(&format!("<printOptions{o}/>"));
        }
        let mut margins = String::new();
        let mut m = |name: &str, v: Option<f64>| {
            if let Some(x) = v {
                margins.push_str(&format!(" {name}=\"{}\"", trim_num(x)));
            }
        };
        m("left", p.margin_left);
        m("right", p.margin_right);
        m("top", p.margin_top);
        m("bottom", p.margin_bottom);
        m("header", p.margin_header);
        m("footer", p.margin_footer);
        if !margins.is_empty() {
            out.push_str(&format!("<pageMargins{margins}/>"));
        }
        let mut attrs = String::new();
        if let Some(o) = &p.orientation {
            attrs.push_str(&format!(" orientation=\"{}\"", esc_xml(o)));
        }
        if let Some(ps) = p.paper_size {
            attrs.push_str(&format!(" paperSize=\"{ps}\""));
        }
        if let Some(sc) = p.scale {
            attrs.push_str(&format!(" scale=\"{sc}\""));
        }
        if let Some(w) = p.fit_to_width {
            attrs.push_str(&format!(" fitToWidth=\"{w}\""));
        }
        if let Some(h) = p.fit_to_height {
            attrs.push_str(&format!(" fitToHeight=\"{h}\""));
        }
        if !attrs.is_empty() {
            out.push_str(&format!("<pageSetup{attrs}/>"));
        }
        // headerFooter（页眉/页脚）
        if p.odd_header.is_some()
            || p.odd_footer.is_some()
            || p.even_header.is_some()
            || p.even_footer.is_some()
            || p.first_header.is_some()
            || p.first_footer.is_some()
            || p.different_odd_even
            || p.different_first
        {
            let mut ha = String::new();
            if p.different_odd_even {
                ha.push_str(" differentOddEven=\"1\"");
            }
            if p.different_first {
                ha.push_str(" differentFirst=\"1\"");
            }
            out.push_str(&format!("<headerFooter{ha}>"));
            let mut el = |tag: &str, v: &Option<String>| {
                if let Some(t) = v {
                    out.push_str(&format!("<{tag}>{}</{tag}>", esc_xml(t)));
                }
            };
            el("oddHeader", &p.odd_header);
            el("oddFooter", &p.odd_footer);
            el("evenHeader", &p.even_header);
            el("evenFooter", &p.even_footer);
            el("firstHeader", &p.first_header);
            el("firstFooter", &p.first_footer);
            out.push_str("</headerFooter>");
        }
        if !p.row_breaks.is_empty() {
            out.push_str(&format!(
                "<rowBreaks count=\"{}\" manualBreakCount=\"{}\">",
                p.row_breaks.len(),
                p.row_breaks.len()
            ));
            for id in &p.row_breaks {
                out.push_str(&format!("<brk id=\"{id}\" max=\"16383\" man=\"1\"/>"));
            }
            out.push_str("</rowBreaks>");
        }
        if !p.col_breaks.is_empty() {
            out.push_str(&format!(
                "<colBreaks count=\"{}\" manualBreakCount=\"{}\">",
                p.col_breaks.len(),
                p.col_breaks.len()
            ));
            for id in &p.col_breaks {
                out.push_str(&format!("<brk id=\"{id}\" max=\"1048575\" man=\"1\"/>"));
            }
            out.push_str("</colBreaks>");
        }
    }

    if has_drawing {
        out.push_str("<drawing r:id=\"rId1\"/>");
    }

    if let Some(rid) = legacy_rid.or(ole_vml_rid) {
        out.push_str(&format!("<legacyDrawing r:id=\"{}\"/>", esc_xml(rid)));
    }

    if let Some(rid) = background_rid {
        out.push_str(&format!("<picture r:id=\"{}\"/>", esc_xml(rid)));
    }

    if !ole.is_empty() {
        out.push_str("<oleObjects>");
        for (rid, shape_id, prog) in ole {
            out.push_str(&format!(
                "<oleObject progId=\"{}\" shapeId=\"{}\" r:id=\"{}\"/>",
                esc_xml(prog),
                shape_id,
                esc_xml(rid)
            ));
        }
        out.push_str("</oleObjects>");
    }

    if !table_rids.is_empty() {
        out.push_str(&format!("<tableParts count=\"{}\">", table_rids.len()));
        for rid in table_rids {
            out.push_str(&format!("<tablePart r:id=\"{}\"/>", esc_xml(rid)));
        }
        out.push_str("</tableParts>");
    }

    for rid in pivot_rids {
        out.push_str(&format!(
            "<pivotTable r:id=\"{}\" name=\"\" cacheId=\"0\"/>",
            esc_xml(rid)
        ));
    }

    let mut ext_children = String::new();

    if !sheet.sparklines.is_empty() {
        ext_children.push_str(
            "<ext uri=\"{05C60535-1F16-4fd2-B633-F4F36F0B64E0}\" \
             xmlns:x14=\"http://schemas.microsoft.com/office/spreadsheetml/2009/9/main\">\
             <x14:sparklineGroups xmlns:xm=\"http://schemas.microsoft.com/office/excel/2006/main\">",
        );
        for s in &sheet.sparklines {
            let ty = match s.kind.as_deref() {
                Some("column") => "column",
                Some("stacked") | Some("winloss") | Some("winLoss") => "stacked",
                _ => "line",
            };
            let flag = |b: bool, name: &str| {
                if b {
                    format!(" {name}=\"1\"")
                } else {
                    String::new()
                }
            };
            ext_children.push_str(&format!("<x14:sparklineGroup type=\"{ty}\""));
            ext_children.push_str(&flag(s.show_high, "showHigh"));
            ext_children.push_str(&flag(s.show_low, "showLow"));
            ext_children.push_str(&flag(s.show_first, "showFirst"));
            ext_children.push_str(&flag(s.show_last, "showLast"));
            ext_children.push_str(&flag(s.show_negative, "showNegative"));
            ext_children.push('>');
            if let Some(color) = &s.color {
                ext_children.push_str(&format!(
                    "<x14:colorSeries rgb=\"{}\"/>",
                    crate::utils::xml::to_argb(color)
                ));
            }
            let range = s.data_range.clone();
            ext_children.push_str("<x14:sparklines><x14:sparkline>");
            ext_children.push_str(&format!("<xm:f>{}</xm:f>", esc_xml(&range)));
            ext_children.push_str(&format!("<xm:sqref>{}</xm:sqref>", esc_xml(&s.location)));
            ext_children.push_str("</x14:sparkline></x14:sparklines></x14:sparklineGroup>");
        }
        ext_children.push_str("</x14:sparklineGroups></ext>");
    }

    if !slicer_rids.is_empty() {
        ext_children.push_str(
            "<ext uri=\"{A8765BA9-456A-4dab-B4F3-ACF838C121DE}\" \
             xmlns:x14=\"http://schemas.microsoft.com/office/spreadsheetml/2009/9/main\">\
             <x14:slicerList>",
        );
        for rid in slicer_rids {
            ext_children.push_str(&format!("<x14:slicer r:id=\"{}\"/>", esc_xml(rid)));
        }
        ext_children.push_str("</x14:slicerList></ext>");
    }

    if !ext_children.is_empty() {
        out.push_str(&format!("<extLst>{ext_children}</extLst>"));
    }

    out.push_str("</worksheet>");
    Ok(out)
}

fn cell_position(cell: &Cell, ci: usize, row: u32) -> Result<(u32, u32)> {
    match &cell.reference {
        Some(r) => parse_cell_ref(r),
        None => Ok(((ci + 1) as u32, row)),
    }
}

fn cell_xml(
    wb: &Workbook,
    cell: &Cell,
    col: u32,
    row: u32,
    styles: &StyleTable,
    sst: &mut SharedStrings,
) -> Result<String> {
    let reference = cell
        .reference
        .clone()
        .unwrap_or_else(|| make_cell_ref(col, row));
    let style_idx = styles.resolve(wb, cell);
    let mut attrs = format!(" r=\"{reference}\"");
    if style_idx != 0 {
        attrs.push_str(&format!(" s=\"{style_idx}\""));
    }

    let mut content = String::new();

    if !cell.runs.is_empty() {
        attrs.push_str(" t=\"inlineStr\"");
        content.push_str("<is>");
        for run in &cell.runs {
            content.push_str("<r>");
            content.push_str(&run_props_xml(run));
            content.push_str(&format!(
                "<t xml:space=\"preserve\">{}</t>",
                esc_xml(&run.text)
            ));
            content.push_str("</r>");
        }
        content.push_str("</is>");
    } else if let Some(f) = &cell.formula {
        content.push_str(&format!("<f>{}</f>", esc_xml(f)));
        if let Some(v) = &cell.value {
            if cell.cell_type.as_deref() == Some("string") || matches!(v, Value::String(_)) {
                attrs.push_str(" t=\"str\"");
                content.push_str(&format!("<v>{}</v>", esc_xml(&value_string(v))));
            } else if cell.cell_type.as_deref() == Some("boolean") || matches!(v, Value::Bool(_)) {
                attrs.push_str(" t=\"b\"");
                content.push_str(&format!("<v>{}</v>", bool_num(v)));
            } else {
                content.push_str(&format!("<v>{}</v>", esc_xml(&value_string(v))));
            }
        }
    } else {
        match cell.cell_type.as_deref() {
            Some("string") => {
                let s = cell.value.as_ref().map(value_string).unwrap_or_default();
                let idx = sst.intern(&s);
                attrs.push_str(" t=\"s\"");
                content.push_str(&format!("<v>{idx}</v>"));
            }
            Some("boolean") => {
                attrs.push_str(" t=\"b\"");
                let v = cell
                    .value
                    .as_ref()
                    .map(bool_num)
                    .unwrap_or_else(|| "0".into());
                content.push_str(&format!("<v>{v}</v>"));
            }
            Some("number") => {
                let v = cell.value.as_ref().map(value_string).unwrap_or_default();
                content.push_str(&format!("<v>{}</v>", esc_xml(&v)));
            }
            Some("date") => {
                if let Some(serial) = date_serial(cell, wb.date1904) {
                    content.push_str(&format!("<v>{serial}</v>"));
                }
            }
            Some("error") => {
                attrs.push_str(" t=\"e\"");
                let v = cell.value.as_ref().map(value_string).unwrap_or_default();
                content.push_str(&format!("<v>{}</v>", esc_xml(&v)));
            }
            Some("richtext") | None if cell.value.is_some() => {
                // For M1 rich text is flattened to a plain shared string.
                if cell.cell_type.as_deref() == Some("number")
                    || matches!(cell.value, Some(Value::Number(_)))
                {
                    let v = cell.value.as_ref().map(value_string).unwrap_or_default();
                    content.push_str(&format!("<v>{}</v>", esc_xml(&v)));
                } else {
                    let s = cell.value.as_ref().map(value_string).unwrap_or_default();
                    let idx = sst.intern(&s);
                    attrs.push_str(" t=\"s\"");
                    content.push_str(&format!("<v>{idx}</v>"));
                }
            }
            _ => {}
        }
    }

    if content.is_empty() {
        Ok(format!("<c{attrs}/>"))
    } else {
        Ok(format!("<c{attrs}>{content}</c>"))
    }
}

fn run_props_xml(run: &crate::model::cell::Run) -> String {
    let mut inner = String::new();
    if let Some(font) = &run.font {
        inner.push_str(&format!("<rFont val=\"{}\"/>", esc_xml(font)));
    }
    if run.bold {
        inner.push_str("<b/>");
    }
    if run.italic {
        inner.push_str("<i/>");
    }
    if run.strike {
        inner.push_str("<strike/>");
    }
    if let Some(u) = &run.underline {
        if u == "single" {
            inner.push_str("<u/>");
        } else if u != "none" {
            inner.push_str(&format!("<u val=\"{}\"/>", esc_xml(u)));
        }
    }
    if let Some(va) = &run.vert_align {
        inner.push_str(&format!("<vertAlign val=\"{}\"/>", esc_xml(va)));
    }
    if let Some(c) = &run.color {
        inner.push_str(&format!(
            "<color rgb=\"{}\"/>",
            crate::utils::xml::to_argb(c)
        ));
    }
    if let Some(sz) = run.size {
        inner.push_str(&format!("<sz val=\"{}\"/>", trim_num(sz)));
    }
    if inner.is_empty() {
        String::new()
    } else {
        format!("<rPr>{inner}</rPr>")
    }
}

fn date_serial(cell: &Cell, date1904: bool) -> Option<i64> {
    match &cell.value {
        Some(Value::String(s)) => parse_iso_date(s).map(|d| date_to_serial(d, date1904)),
        Some(Value::Number(n)) => n.as_i64(),
        _ => None,
    }
}

fn value_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn bool_num(v: &Value) -> String {
    let b = match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        Value::String(s) => s == "true" || s == "1",
        _ => false,
    };
    if b {
        "1".into()
    } else {
        "0".into()
    }
}

fn trim_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn validations_xml(vs: &[DataValidation]) -> String {
    let mut out = format!("<dataValidations count=\"{}\">", vs.len());
    for v in vs {
        let mut attrs = String::new();
        if !v.validation_type.is_empty() && v.validation_type != "none" {
            attrs.push_str(&format!(" type=\"{}\"", esc_xml(&v.validation_type)));
        }
        if let Some(op) = &v.operator {
            attrs.push_str(&format!(" operator=\"{}\"", esc_xml(op)));
        }
        if v.allow_blank {
            attrs.push_str(" allowBlank=\"1\"");
        }
        if v.show_error {
            attrs.push_str(" showErrorMessage=\"1\"");
        }
        if let Some(t) = &v.error_title {
            attrs.push_str(&format!(" errorTitle=\"{}\"", esc_xml(t)));
        }
        if let Some(m) = &v.error_message {
            attrs.push_str(&format!(" error=\"{}\"", esc_xml(m)));
        }
        if let Some(t) = &v.prompt_title {
            attrs.push_str(&format!(" promptTitle=\"{}\"", esc_xml(t)));
        }
        if let Some(m) = &v.prompt_message {
            attrs.push_str(&format!(" prompt=\"{}\"", esc_xml(m)));
        }
        attrs.push_str(&format!(" sqref=\"{}\"", esc_xml(&v.range)));
        out.push_str(&format!("<dataValidation{attrs}>"));
        if let Some(f) = &v.formula1 {
            out.push_str(&format!("<formula1>{}</formula1>", esc_xml(f)));
        } else if !v.values.is_empty() {
            out.push_str(&format!(
                "<formula1>\"{}\"</formula1>",
                esc_xml(&v.values.join(","))
            ));
        }
        if let Some(f) = &v.formula2 {
            out.push_str(&format!("<formula2>{}</formula2>", esc_xml(f)));
        }
        out.push_str("</dataValidation>");
    }
    out.push_str("</dataValidations>");
    out
}

fn conditional_formats_xml(sheet: &Sheet, styles: &StyleTable) -> String {
    let mut out = String::new();
    for (i, cf) in sheet.conditional_formats.iter().enumerate() {
        let priority = i + 1;
        out.push_str(&format!(
            "<conditionalFormatting sqref=\"{}\">",
            esc_xml(&cf.range)
        ));
        let dxf = styles
            .dxf_id_for(cf)
            .map(|d| format!(" dxfId=\"{d}\""))
            .unwrap_or_default();
        let stop = if cf.stop_if_true {
            " stopIfTrue=\"1\""
        } else {
            ""
        };
        match cf.rule_type.as_str() {
            "cellIs" => {
                let op = cf.operator.clone().unwrap_or_else(|| "greaterThan".into());
                out.push_str(&format!(
                    "<cfRule type=\"cellIs\"{dxf} priority=\"{priority}\" operator=\"{}\"{stop}>",
                    esc_xml(&op)
                ));
                for f in &cf.formulas {
                    out.push_str(&format!("<formula>{}</formula>", esc_xml(f)));
                }
                out.push_str("</cfRule>");
            }
            "expression" | "formula" => {
                out.push_str(&format!(
                    "<cfRule type=\"expression\"{dxf} priority=\"{priority}\"{stop}>"
                ));
                for f in &cf.formulas {
                    out.push_str(&format!("<formula>{}</formula>", esc_xml(f)));
                }
                out.push_str("</cfRule>");
            }
            "colorScale" => {
                out.push_str(&format!(
                    "<cfRule type=\"colorScale\" priority=\"{priority}\"{stop}><colorScale>"
                ));
                if cf.colors.len() >= 3 {
                    out.push_str(
                        "<cfvo type=\"min\"/><cfvo type=\"percentile\" val=\"50\"/><cfvo type=\"max\"/>",
                    );
                } else {
                    out.push_str("<cfvo type=\"min\"/><cfvo type=\"max\"/>");
                }
                for c in &cf.colors {
                    out.push_str(&format!("<color rgb=\"{}\"/>", to_argb(c)));
                }
                out.push_str("</colorScale></cfRule>");
            }
            "dataBar" => {
                let c = cf.color.clone().unwrap_or_else(|| "638EC6".into());
                out.push_str(&format!(
                    "<cfRule type=\"dataBar\" priority=\"{priority}\"{stop}><dataBar><cfvo type=\"min\"/><cfvo type=\"max\"/><color rgb=\"{}\"/></dataBar></cfRule>",
                    to_argb(&c)
                ));
            }
            "containsText" => {
                let text = cf.text.clone().unwrap_or_default();
                let first = first_cell(&cf.range);
                let formula = format!("NOT(ISERROR(SEARCH(\"{}\",{})))", text, first);
                out.push_str(&format!(
                    "<cfRule type=\"containsText\"{dxf} priority=\"{priority}\" operator=\"containsText\" text=\"{}\"{stop}>",
                    esc_xml(&text)
                ));
                out.push_str(&format!("<formula>{}</formula>", esc_xml(&formula)));
                out.push_str("</cfRule>");
            }
            "top10" => {
                let rank = cf.rank.unwrap_or(10);
                out.push_str(&format!(
                    "<cfRule type=\"top10\"{dxf} priority=\"{priority}\" rank=\"{rank}\" percent=\"{}\" bottom=\"{}\"{stop}/>",
                    bnum(cf.percent),
                    bnum(cf.bottom)
                ));
            }
            "duplicateValues" => {
                out.push_str(&format!(
                    "<cfRule type=\"duplicateValues\"{dxf} priority=\"{priority}\"{stop}/>"
                ));
            }
            "aboveAverage" => {
                let mut attrs = String::new();
                if let Some(b) = cf.above_average {
                    attrs.push_str(&format!(" aboveAverage=\"{}\"", bnum(b)));
                }
                if cf.equal_average {
                    attrs.push_str(" equalAverage=\"1\"");
                }
                if let Some(sd) = cf.std_dev {
                    attrs.push_str(&format!(" stdDev=\"{sd}\""));
                }
                out.push_str(&format!(
                    "<cfRule type=\"aboveAverage\"{dxf} priority=\"{priority}\"{attrs}{stop}/>"
                ));
            }
            "beginsWith" | "endsWith" | "notContainsText" => {
                let text = cf.text.clone().unwrap_or_default();
                let first = first_cell(&cf.range);
                let (op, formula) = match cf.rule_type.as_str() {
                    "beginsWith" => (
                        "beginsWith",
                        format!("LEFT({first},LEN(\"{text}\"))=\"{text}\""),
                    ),
                    "endsWith" => (
                        "endsWith",
                        format!("RIGHT({first},LEN(\"{text}\"))=\"{text}\""),
                    ),
                    _ => (
                        "notContains",
                        format!("ISERROR(SEARCH(\"{text}\",{first}))"),
                    ),
                };
                out.push_str(&format!(
                    "<cfRule type=\"{}\"{dxf} priority=\"{priority}\" operator=\"{op}\" text=\"{}\"{stop}><formula>{}</formula></cfRule>",
                    esc_xml(&cf.rule_type),
                    esc_xml(&text),
                    esc_xml(&formula)
                ));
            }
            "timePeriod" => {
                let tp = cf.time_period.clone().unwrap_or_else(|| "today".into());
                out.push_str(&format!(
                    "<cfRule type=\"timePeriod\"{dxf} priority=\"{priority}\" timePeriod=\"{}\"{stop}/>",
                    esc_xml(&tp)
                ));
            }
            "iconSet" => {
                let style = cf
                    .icon_style
                    .clone()
                    .unwrap_or_else(|| "3TrafficLights1".into());
                let count = icon_count(&style);
                out.push_str(&format!(
                    "<cfRule type=\"iconSet\" priority=\"{priority}\"{stop}><iconSet iconSet=\"{}\"",
                    esc_xml(&style)
                ));
                if cf.reverse_icons {
                    out.push_str(" reverse=\"1\"");
                }
                if let Some(sv) = cf.show_value {
                    out.push_str(&format!(" showValue=\"{}\"", bnum(sv)));
                }
                out.push('>');
                for i in 0..count {
                    let val = (100 * i) / count;
                    out.push_str(&format!("<cfvo type=\"percent\" val=\"{val}\"/>"));
                }
                out.push_str("</iconSet></cfRule>");
            }
            "uniqueValues" | "containsBlanks" | "notContainsBlanks" | "containsErrors"
            | "notContainsErrors" => {
                out.push_str(&format!(
                    "<cfRule type=\"{}\"{dxf} priority=\"{priority}\"{stop}/>",
                    cf.rule_type
                ));
            }
            _ => {}
        }
        out.push_str("</conditionalFormatting>");
    }
    out
}

fn first_cell(range: &str) -> String {
    match crate::utils::a1::parse_range(range) {
        Ok(((c1, r1), _)) => make_cell_ref(c1, r1),
        Err(_) => "A1".to_string(),
    }
}

fn bnum(b: bool) -> &'static str {
    if b {
        "1"
    } else {
        "0"
    }
}

fn icon_count(style: &str) -> u32 {
    match style.chars().next() {
        Some('4') => 4,
        Some('5') => 5,
        _ => 3,
    }
}

fn col_key(c: &Column) -> String {
    serde_json::to_string(&(c.width, c.hidden, c.outline_level, c.collapsed, &c.style))
        .unwrap_or_default()
}
