//! 生成 document.xml：正文块 → WordprocessingML

use crate::generate::GenCtx;
use crate::model::blocks::*;
use crate::model::style::PageSetup;
use crate::utils::units::*;
use crate::utils::xml::{empty, esc, esc_attr};

/// 修订/批注日期（固定值以保证可重现）
pub(crate) const REVISION_DATE: &str = "2026-01-01T00:00:00Z";

/// document.xml 根命名空间声明
const DOC_NS: &str = concat!(
    "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" ",
    "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" ",
    "xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" ",
    "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" ",
    "xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" ",
    "xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" ",
    "xmlns:v=\"urn:schemas-microsoft-com:vml\" ",
    "xmlns:o=\"urn:schemas-microsoft-com:office:office\" ",
    "xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\" ",
    "xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" ",
    "xmlns:dgm=\"http://schemas.openxmlformats.org/drawingml/2006/diagram\" ",
    "xmlns:dsp=\"http://schemas.microsoft.com/office/drawing/2008/diagram\" ",
    "xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\" ",
    "xmlns:wp14=\"http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing\" ",
    "xmlns:wpg=\"http://schemas.microsoft.com/office/word/2010/wordprocessingGroup\" ",
    "xmlns:wpi=\"http://schemas.microsoft.com/office/word/2010/wordprocessingInk\" ",
    "xmlns:wne=\"http://schemas.microsoft.com/office/word/2006/wordml\""
);

/// 生成完整 document.xml
pub(crate) fn document_xml(ctx: &GenCtx) -> String {
    let mut body = String::new();
    let n = ctx.doc.parts.len();
    for (i, part) in ctx.doc.parts.iter().enumerate() {
        for block in &part.blocks {
            body.push_str(&block_xml(ctx, block));
        }
        // 显式声明了 section 的分片在其末尾插入节属性（分节符），
        // 最后一段的节属性统一放在 body 末尾。
        if i + 1 < n && part.section.is_some() {
            let page = effective_page(ctx, part);
            let pns = part.section.as_ref().and_then(|s| s.page_number_start);
            body.push_str(&format!(
                "<w:p><w:pPr>{}</w:pPr></w:p>",
                sect_pr(ctx, &page, i, pns)
            ));
        }
    }
    let last = ctx.doc.parts.last();
    let last_page = last
        .map(|p| effective_page(ctx, p))
        .unwrap_or_else(|| ctx.doc.page.clone());
    let last_pns = last
        .and_then(|p| p.section.as_ref())
        .and_then(|s| s.page_number_start);
    body.push_str(&sect_pr(ctx, &last_page, n.saturating_sub(1), last_pns));

    let mut out = String::new();
    out.push_str(crate::utils::xml::declaration());
    out.push_str("<w:document ");
    out.push_str(DOC_NS);
    out.push('>');
    if let Some(bg) = &ctx.doc.background {
        out.push_str(&format!(
            "<w:background w:color=\"{}\"/>",
            crate::utils::xml::color_hex(bg)
        ));
    }
    out.push_str("<w:body>");
    out.push_str(&body);
    out.push_str("</w:body></w:document>");
    out
}

// ---------- 块 ----------

fn block_xml(ctx: &GenCtx, b: &Block) -> String {
    match b {
        Block::Heading {
            level,
            text,
            align,
            runs,
            ..
        } => heading_xml_runs(*level, text, align.as_deref(), runs.as_deref()),
        Block::Paragraph(p) => paragraph_xml(ctx, p),
        Block::List(l) => list_xml(ctx, l),
        Block::Table(t) => table_xml(ctx, t),
        Block::Image(i) => image_xml(ctx, i),
        Block::Formula(f) => formula_xml(f),
        Block::Code(c) => code_xml(c),
        Block::Quote(q) => quote_xml(q),
        Block::Toc(t) => toc_xml(ctx, t),
        Block::Bibliography(bib) => bibliography_xml(ctx, bib),
        Block::Caption(c) => caption_xml(&c.text),
        Block::Chart(c) => chart_xml(ctx, c),
        Block::TextBox(t) => textbox_xml(ctx, t),
        Block::Attachment(a) => attachment_xml(ctx, a),
        Block::Shape(sh) => shape_xml(ctx, sh),
        Block::Raw(r) => raw_xml(ctx, r),
        Block::Sdt(c) => sdt_xml(ctx, c),
        Block::PageBreak => page_break_xml(),
    }
}

fn heading_xml(level: u8, text: &str, align: Option<&str>) -> String {
    heading_xml_runs(level, text, align, None)
}

fn heading_xml_runs(level: u8, text: &str, align: Option<&str>, runs: Option<&[Run]>) -> String {
    let lvl = level.clamp(1, 9);
    let mut ppr = format!(
        "<w:pStyle w:val=\"Heading{lvl}\"/><w:outlineLvl w:val=\"{}\"/>",
        lvl - 1
    );
    if let Some(a) = align {
        ppr.push_str(&empty("w:jc", &[("w:val", jc_val(a))]));
    }
    let content = match runs {
        Some(rs) if !rs.is_empty() => {
            let mut c = String::new();
            for r in rs {
                c.push_str(&run_xml_no_ctx(r));
            }
            c
        }
        _ => run_plain(text),
    };
    paragraph_with_ppr(&ppr, &content)
}

fn paragraph_xml(ctx: &GenCtx, p: &Paragraph) -> String {
    let mut ppr = String::new();
    if let Some(style) = &p.style {
        ppr.push_str(&empty("w:pStyle", &[("w:val", style.clone())]));
    }
    ppr.push_str(&paragraph_ppr(p));
    let content = paragraph_content(ctx, p);
    // 稳定段落 ID（@paraId 寻址）：原样回写 w14:paraId
    let pid = p
        .para_id
        .as_ref()
        .map(|id| format!(" w14:paraId=\"{id}\""))
        .unwrap_or_default();
    let xml = paragraph_with_ppr(&ppr, &content);
    if pid.is_empty() {
        xml
    } else {
        xml.replacen("<w:p>", &format!("<w:p{pid}>"), 1)
    }
}

/// 段落 pPr 子元素（不含 pStyle/numPr）
fn paragraph_ppr(p: &Paragraph) -> String {
    let mut ppr = String::new();
    for (tag, v) in [
        ("w:autoSpaceDE", p.auto_space_de),
        ("w:autoSpaceDN", p.auto_space_dn),
        ("w:adjustRightInd", p.adjust_right_ind),
        ("w:snapToGrid", p.snap_to_grid),
    ] {
        if let Some(b) = v {
            ppr.push_str(&empty(
                tag,
                &[("w:val", if b { "1" } else { "0" }.to_string())],
            ));
        }
    }
    if p.rtl == Some(true) {
        ppr.push_str("<w:bidi/>");
    }
    if p.keep_next == Some(true) {
        ppr.push_str("<w:keepNext/>");
    }
    if let Some(lines) = p.drop_cap {
        ppr.push_str(&format!(
            "<w:framePr w:dropCap=\"drop\" w:lines=\"{}\" w:wrap=\"around\"/>",
            lines.clamp(1, 10)
        ));
    }
    if p.keep_lines == Some(true) {
        ppr.push_str("<w:keepLines/>");
    }
    if p.page_break_before == Some(true) {
        ppr.push_str("<w:pageBreakBefore/>");
    }
    if let Some(color) = &p.border_box {
        let c = crate::utils::xml::color_hex(color);
        ppr.push_str(&format!(
            "<w:pBdr><w:top w:val=\"single\" w:sz=\"4\" w:space=\"1\" w:color=\"{c}\"/>\
             <w:left w:val=\"single\" w:sz=\"4\" w:space=\"4\" w:color=\"{c}\"/>\
             <w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"1\" w:color=\"{c}\"/>\
             <w:right w:val=\"single\" w:sz=\"4\" w:space=\"4\" w:color=\"{c}\"/></w:pBdr>"
        ));
    } else if let Some(color) = &p.border {
        let c = crate::utils::xml::color_hex(color);
        ppr.push_str(&format!(
            "<w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"{c}\"/></w:pBdr>"
        ));
    }
    if let Some(fill) = &p.shading {
        ppr.push_str(&format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
            crate::utils::xml::color_hex(fill)
        ));
    }
    if let Some(tabs) = &p.tabs {
        ppr.push_str("<w:tabs>");
        for t in tabs {
            let mut a = vec![
                (
                    "w:val",
                    t.align.clone().unwrap_or_else(|| "left".to_string()),
                ),
                ("w:pos", pt_to_twip(t.pos).to_string()),
            ];
            if let Some(l) = &t.leader {
                a.push(("w:leader", l.clone()));
            }
            ppr.push_str(&empty("w:tab", &a));
        }
        ppr.push_str("</w:tabs>");
    }
    let mut spacing = Vec::new();
    if let Some(v) = p.space_before {
        spacing.push(("w:before", pt_to_twip(v).to_string()));
    }
    if let Some(v) = p.space_after {
        spacing.push(("w:after", pt_to_twip(v).to_string()));
    }
    if let Some(v) = p.line_pt {
        spacing.push(("w:line", pt_to_twip(v).to_string()));
        spacing.push((
            "w:lineRule",
            p.line_rule.clone().unwrap_or_else(|| "atLeast".to_string()),
        ));
    } else if let Some(v) = p.line_spacing {
        spacing.push(("w:line", line_to_xml(v).to_string()));
        spacing.push(("w:lineRule", "auto".to_string()));
    }
    if !spacing.is_empty()
        || p.before_autospacing == Some(true)
        || p.after_autospacing == Some(true)
    {
        let mut inner = String::new();
        if p.before_autospacing == Some(true) {
            inner.push_str("<w:beforeAutospacing/>");
        }
        if p.after_autospacing == Some(true) {
            inner.push_str("<w:afterAutospacing/>");
        }
        if inner.is_empty() {
            ppr.push_str(&empty("w:spacing", &spacing));
        } else {
            let attrs: String = spacing
                .iter()
                .map(|(k, v)| format!(" {k}=\"{v}\""))
                .collect();
            ppr.push_str(&format!("<w:spacing{attrs}>{inner}</w:spacing>"));
        }
    }
    let mut ind = Vec::new();
    if let Some(v) = p.first_line_indent {
        ind.push(("w:firstLine", pt_to_twip(v).to_string()));
    }
    if let Some(v) = p.indent {
        ind.push(("w:left", pt_to_twip(v).to_string()));
    }
    if let Some(v) = p.hanging_indent {
        ind.push(("w:hanging", pt_to_twip(v).to_string()));
    }
    if !ind.is_empty() {
        ppr.push_str(&empty("w:ind", &ind));
    }
    if let Some(align) = &p.align {
        ppr.push_str(&empty("w:jc", &[("w:val", jc_val(align))]));
    }
    ppr
}

fn paragraph_content(ctx: &GenCtx, p: &Paragraph) -> String {
    if let Some(runs) = &p.runs {
        let mut s = String::new();
        for r in runs {
            s.push_str(&run_xml(ctx, r));
        }
        s
    } else {
        // 段落级 run 属性
        let mut r = Run::plain(p.text.clone().unwrap_or_default());
        r.bold = p.bold;
        r.italic = p.italic;
        r.font_size = p.font_size;
        r.color = p.color.clone();
        r.font_family = p.font_family.clone();
        run_xml(ctx, &r)
    }
}

fn list_xml(ctx: &GenCtx, l: &ListBlock) -> String {
    let num_id = crate::generate::list_key(l)
        .and_then(|k| ctx.list_numid.get(&k).copied())
        .unwrap_or(if l.ordered { 2 } else { 1 });
    let mut out = String::new();
    for item in &l.items {
        for (i, b) in item.blocks.iter().enumerate() {
            match b {
                Block::Paragraph(p) => {
                    let mut ppr = String::new();
                    if let Some(style) = &p.style {
                        ppr.push_str(&empty("w:pStyle", &[("w:val", style.clone())]));
                    }
                    if i == 0 {
                        ppr.push_str(&format!(
                            "<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{}\"/></w:numPr>",
                            item.level, num_id
                        ));
                    }
                    ppr.push_str(&paragraph_ppr(p));
                    let content = paragraph_content(ctx, p);
                    out.push_str(&paragraph_with_ppr(&ppr, &content));
                }
                other => out.push_str(&block_xml(ctx, other)),
            }
        }
    }
    out
}

fn table_xml(ctx: &GenCtx, t: &TableBlock) -> String {
    let grid = build_grid(t);
    let cols = grid.width.max(1);

    // 文本区宽度（twip），用于未指定列宽时铺满页面
    let (pw, _ph) = ctx.doc.page.size_pt();
    let m = &ctx.doc.page.margins;
    let text_twip = pt_to_twip((pw - m.left - m.right).max(72.0));

    // 列宽：显式 widths，否则等分铺满文本区
    let col_widths: Vec<i64> = match &t.widths {
        Some(ws) if !ws.is_empty() => {
            let mut v: Vec<i64> = ws.iter().take(cols).map(|w| pt_to_twip(*w)).collect();
            while v.len() < cols {
                v.push(0);
            }
            v
        }
        _ => vec![text_twip / cols as i64; cols],
    };
    let table_w: i64 = t
        .width
        .map(pt_to_twip)
        .unwrap_or_else(|| col_widths.iter().sum());
    let align = t.align.clone().unwrap_or_else(|| "center".to_string());

    let mut out = String::new();
    out.push_str("<w:tbl><w:tblPr>");
    let style = t.style.clone().unwrap_or_else(|| "TableGrid".to_string());
    let tblw = match t.width_pct {
        Some(p) => format!(
            "<w:tblW w:w=\"{}\" w:type=\"pct\"/>",
            (p * 50.0).round() as i64
        ),
        None => format!("<w:tblW w:w=\"{table_w}\" w:type=\"dxa\"/>"),
    };
    out.push_str(&format!(
        "<w:tblStyle w:val=\"{}\" />{tblw}<w:jc w:val=\"{}\" />",
        esc_attr(&style),
        jc_val(&align)
    ));
    if let Some(layout) = &t.layout {
        out.push_str(&empty("w:tblLayout", &[("w:type", layout.clone())]));
    }
    if t.rtl == Some(true) {
        out.push_str("<w:bidiVisual/>");
    }
    if let Some(ind) = t.indent {
        out.push_str(&empty(
            "w:tblInd",
            &[
                ("w:w", pt_to_twip(ind).to_string()),
                ("w:type", "dxa".to_string()),
            ],
        ));
    }
    out.push_str(&table_borders_xml(t.border.as_ref()));
    out.push_str("</w:tblPr>");

    out.push_str("<w:tblGrid>");
    for w in &col_widths {
        out.push_str(&empty("w:gridCol", &[("w:w", w.to_string())]));
    }
    out.push_str("</w:tblGrid>");

    for (ri, row) in grid.rows.iter().enumerate() {
        let mut trpr = String::new();
        if ri == 0 && t.header_row {
            trpr.push_str("<w:tblHeader/>");
        }
        if let Some(h) = t.rows[ri].height {
            let rule = t.rows[ri]
                .height_rule
                .clone()
                .unwrap_or_else(|| "atLeast".to_string());
            trpr.push_str(&format!(
                "<w:trHeight w:val=\"{}\" w:hRule=\"{}\"/>",
                pt_to_twip(h),
                esc_attr(&rule)
            ));
        }
        out.push_str("<w:tr>");
        if !trpr.is_empty() {
            out.push_str(&format!("<w:trPr>{trpr}</w:trPr>"));
        }
        let mut ci = 0usize;
        while ci < row.len() {
            match &row[ci] {
                None => {
                    out.push_str("<w:tc><w:p/></w:tc>");
                    ci += 1;
                }
                Some(info) => {
                    let is_origin = info.origin == (ri, ci);
                    if is_origin || ci == info.origin.1 {
                        out.push_str(&grid_cell_xml(ctx, info, is_origin));
                    }
                    ci += info.colspan.max(1);
                }
            }
        }
        out.push_str("</w:tr>");
    }
    out.push_str("</w:tbl>");
    if let Some(cap) = &t.caption {
        out.push_str(&auto_caption(ctx, "table", cap));
    }
    out
}

/// 网格槽位：来源单元格 + 该槽在其 origin 中的跨度
struct SlotInfo<'a> {
    cell: &'a TableCell,
    origin: (usize, usize),
    colspan: usize,
    rowspan: usize,
}

struct Grid<'a> {
    width: usize,
    /// rows[r][c] = Some(slot) 表示该槽被某单元格覆盖；None 为空槽
    rows: Vec<Vec<Option<SlotInfo<'a>>>>,
}

/// 按 colspan/rowspan 计算表格网格，自动为纵向合并补 vMerge 占位
fn build_grid(t: &TableBlock) -> Grid<'_> {
    let nrows = t.rows.len();
    let mut width = 1usize;
    for r in &t.rows {
        let w: usize = r
            .cells
            .iter()
            .map(|c| c.colspan.unwrap_or(1).max(1) as usize)
            .sum();
        width = width.max(w);
    }
    let mut grid: Vec<Vec<Option<SlotInfo>>> = (0..nrows)
        .map(|_| (0..width).map(|_| None).collect())
        .collect();
    for (r, row) in t.rows.iter().enumerate() {
        let mut c = 0usize;
        for cell in &row.cells {
            while c < width && grid[r][c].is_some() {
                c += 1;
            }
            if c >= width {
                break;
            }
            let cs = cell.colspan.unwrap_or(1).max(1) as usize;
            let rs = cell.rowspan.unwrap_or(1).max(1) as usize;
            for dr in 0..rs {
                for dc in 0..cs {
                    let (rr, cc) = (r + dr, c + dc);
                    if rr < nrows && cc < width {
                        grid[rr][cc] = Some(SlotInfo {
                            cell,
                            origin: (r, c),
                            colspan: cs,
                            rowspan: rs,
                        });
                    }
                }
            }
            c += cs;
        }
    }
    Grid { width, rows: grid }
}

/// 生成一个 `<w:tc>`（区分 origin / 纵向延续；横向覆盖槽由调用方跳过）
fn grid_cell_xml(ctx: &GenCtx, info: &SlotInfo, is_origin: bool) -> String {
    let mut tcpr = String::new();
    if info.colspan > 1 {
        tcpr.push_str(&empty("w:gridSpan", &[("w:val", info.colspan.to_string())]));
    }
    if info.rowspan > 1 {
        if is_origin {
            tcpr.push_str("<w:vMerge w:val=\"restart\"/>");
        } else {
            tcpr.push_str("<w:vMerge/>");
        }
    }
    if is_origin {
        if let Some(fill) = &info.cell.fill {
            tcpr.push_str(&format!(
                "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
                crate::utils::xml::color_hex(fill)
            ));
        }
    }
    if is_origin {
        if let Some(b) = &info.cell.border {
            tcpr.push_str(&cell_borders_xml(b));
        }
        if let Some(m) = info.cell.margin {
            let w = pt_to_twip(m);
            // 仅左右边距；上下保持默认 0（避免行高被撑大）
            tcpr.push_str(&format!(
                "<w:tcMar><w:top w:w=\"0\" w:type=\"dxa\"/><w:left w:w=\"{w}\" w:type=\"dxa\"/>\
                 <w:bottom w:w=\"0\" w:type=\"dxa\"/><w:right w:w=\"{w}\" w:type=\"dxa\"/></w:tcMar>"
            ));
        }
        if let Some(v) = &info.cell.valign {
            tcpr.push_str(&empty("w:vAlign", &[("w:val", v.clone())]));
        }
    }
    let tcpr_tag = if tcpr.is_empty() {
        String::new()
    } else {
        format!("<w:tcPr>{tcpr}</w:tcPr>")
    };

    // 纵向延续单元格不重复内容
    let mut body = String::new();
    if is_origin {
        let mut wrote = false;
        for b in &info.cell.blocks {
            body.push_str(&block_xml(ctx, b));
            wrote = true;
        }
        if !wrote {
            body.push_str("<w:p/>");
        }
    } else {
        body.push_str("<w:p/>");
    }
    format!("<w:tc>{tcpr_tag}{body}</w:tc>")
}

const TBL_EDGES: [&str; 6] = ["top", "left", "bottom", "right", "insideH", "insideV"];

fn edge_el(edge: &str, b: &Border) -> String {
    let val = b.style.clone().unwrap_or_else(|| "single".to_string());
    if val == "none" {
        return format!("<w:{edge} w:val=\"none\" w:sz=\"0\" w:space=\"0\" w:color=\"auto\"/>");
    }
    let sz = (b.width.unwrap_or(0.5) * 8.0).round().max(2.0) as i64;
    let color = match b.color.as_deref() {
        Some(c) if !c.is_empty() => c.trim_start_matches('#').to_uppercase(),
        _ => "auto".to_string(),
    };
    format!(
        "<w:{edge} w:val=\"{}\" w:sz=\"{sz}\" w:space=\"0\" w:color=\"{color}\"/>",
        esc_attr(&val)
    )
}

fn table_borders_xml(b: Option<&Border>) -> String {
    let mut s = String::from("<w:tblBorders>");
    for e in TBL_EDGES {
        match b {
            None => s.push_str(&format!(
                "<w:{e} w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"808080\"/>"
            )),
            Some(b) if b.style.as_deref() == Some("none") => s.push_str(&format!(
                "<w:{e} w:val=\"none\" w:sz=\"0\" w:space=\"0\" w:color=\"auto\"/>"
            )),
            Some(b) => s.push_str(&edge_el(e, b)),
        }
    }
    s.push_str("</w:tblBorders>");
    s
}

fn cell_borders_xml(b: &Border) -> String {
    let mut s = String::from("<w:tcBorders>");
    for e in TBL_EDGES {
        if b.style.as_deref() == Some("none") {
            s.push_str(&format!(
                "<w:{e} w:val=\"none\" w:sz=\"0\" w:space=\"0\" w:color=\"auto\"/>"
            ));
        } else {
            s.push_str(&edge_el(e, b));
        }
    }
    s.push_str("</w:tcBorders>");
    s
}

/// 生成内联/浮动的 `<w:drawing>`（供 run 级行内图片与块级图片复用）；失败返回空串
fn image_drawing_xml(ctx: &GenCtx, img: &ImageBlock) -> String {
    if ctx.img_failed.contains(&img.src) {
        return String::new();
    }
    let rid = ctx
        .img_rid
        .get(&img.src)
        .cloned()
        .unwrap_or_else(|| "rId0".to_string());
    let (mut w, mut h) = ctx
        .img_size
        .get(&img.src)
        .copied()
        .unwrap_or((360.0, 240.0));
    if let Some(wp) = img.width {
        let ratio = if w > 0.0 { h / w } else { 0.75 };
        w = wp;
        h = img.height.unwrap_or(wp * ratio);
    }
    if let Some(hp) = img.height {
        h = hp;
    }
    let cx = pt_to_emu(w);
    let cy = pt_to_emu(h);
    let id = ctx.next_draw_id.get();
    ctx.next_draw_id.set(id + 1);
    let alt = img.alt.clone().unwrap_or_default();
    let crop = img
        .crop
        .map(|c| {
            format!(
                "<a:srcRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/>",
                (c[0] * 1000.0).round() as i64,
                (c[1] * 1000.0).round() as i64,
                (c[2] * 1000.0).round() as i64,
                (c[3] * 1000.0).round() as i64
            )
        })
        .unwrap_or_default();
    let rot = img
        .rotation
        .map(|r| format!(" rot=\"{}\"", (r * 60000.0).round() as i64))
        .unwrap_or_default();
    let graphic = format!(
        concat!(
            "<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">",
            "<pic:pic><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"Picture {id}\"/>",
            "<pic:cNvPicPr/></pic:nvPicPr>",
            "<pic:blipFill><a:blip r:embed=\"{rid}\"/>{crop}<a:stretch><a:fillRect/></a:stretch></pic:blipFill>",
            "<pic:spPr><a:xfrm{rot}><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>",
            "<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>",
            "</pic:pic></a:graphicData></a:graphic>"
        ),
        cx = cx,
        cy = cy,
        id = id,
        rid = rid,
        crop = crop,
        rot = rot
    );
    let wrap = img.wrap.clone().unwrap_or_else(|| "inline".to_string());
    let floating = !wrap.eq_ignore_ascii_case("inline");
    if floating {
        anchored_drawing(&graphic, img, &wrap, cx, cy, id, &alt)
    } else {
        format!(
            "<w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\
             <wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>\
             <wp:docPr id=\"{id}\" name=\"Picture {id}\" descr=\"{alt}\"/>\
             {graphic}</wp:inline></w:drawing>",
            cx = cx,
            cy = cy,
            id = id,
            alt = crate::utils::xml::esc_attr(&alt),
            graphic = graphic
        )
    }
}

fn image_xml(ctx: &GenCtx, img: &ImageBlock) -> String {
    let drawing = image_drawing_xml(ctx, img);
    if drawing.is_empty() {
        return String::new();
    }
    let run = format!("<w:r>{drawing}</w:r>");
    // 未显式指定对齐时保持 Word 默认（左），不强制居中
    let ppr = img
        .align
        .as_ref()
        .map(|a| empty("w:jc", &[("w:val", jc_val(a))]))
        .unwrap_or_default();
    let mut out = paragraph_with_ppr(&ppr, &run);
    if let Some(cap) = &img.caption {
        out.push_str(&auto_caption(ctx, "figure", cap));
    }
    out
}

/// 生成浮动图片的 `<wp:anchor>`
fn anchored_drawing(
    graphic: &str,
    img: &ImageBlock,
    wrap: &str,
    cx: i64,
    cy: i64,
    id: i32,
    alt: &str,
) -> String {
    let x = pt_to_emu(img.x.unwrap_or(0.0));
    let y = pt_to_emu(img.y.unwrap_or(0.0));
    let behind = if img.behind_text == Some(true) {
        "1"
    } else {
        "0"
    };
    let wrap_xml = match wrap.to_ascii_lowercase().as_str() {
        "none" => "<wp:wrapNone/>".to_string(),
        "topandbottom" | "top_bottom" => "<wp:wrapTopAndBottom/>".to_string(),
        "tight" => "<wp:wrapTight wrapText=\"bothSides\"><wp:wrapPolygon edited=\"0\"><wp:start x=\"0\" y=\"0\"/><wp:lineTo x=\"0\" y=\"21600\"/><wp:lineTo x=\"21600\" y=\"21600\"/><wp:lineTo x=\"21600\" y=\"0\"/><wp:lineTo x=\"0\" y=\"0\"/></wp:wrapPolygon></wp:wrapTight>".to_string(),
        _ => "<wp:wrapSquare wrapText=\"bothSides\"/>".to_string(),
    };
    format!(
        concat!(
            "<w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"114300\" distR=\"114300\" ",
            "simplePos=\"0\" relativeHeight=\"251658240\" behindDoc=\"{behind}\" locked=\"0\" ",
            "layoutInCell=\"1\" allowOverlap=\"1\">",
            "<wp:simplePos x=\"0\" y=\"0\"/>",
            "<wp:positionH relativeFrom=\"page\"><wp:posOffset>{x}</wp:posOffset></wp:positionH>",
            "<wp:positionV relativeFrom=\"page\"><wp:posOffset>{y}</wp:posOffset></wp:positionV>",
            "<wp:extent cx=\"{cx}\" cy=\"{cy}\"/>",
            "<wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>",
            "{wrap}",
            "<wp:docPr id=\"{id}\" name=\"Picture {id}\" descr=\"{alt}\"/>",
            "{graphic}</wp:anchor></w:drawing>"
        ),
        behind = behind,
        x = x,
        y = y,
        cx = cx,
        cy = cy,
        wrap = wrap_xml,
        id = id,
        alt = crate::utils::xml::esc_attr(alt),
        graphic = graphic
    )
}

/// 原生图表：内联 drawing 引用 chart 部件
fn chart_xml(ctx: &GenCtx, chart: &ChartBlock) -> String {
    let idx = ctx.chart_seq.get() as usize;
    ctx.chart_seq.set(ctx.chart_seq.get() + 1);
    let rid = ctx
        .chart_rids
        .get(idx)
        .cloned()
        .unwrap_or_else(|| "rId0".to_string());
    let w = chart.width.unwrap_or(360.0);
    let h = chart.height.unwrap_or(200.0);
    let cx = pt_to_emu(w);
    let cy = pt_to_emu(h);
    let id = ctx.next_draw_id.get();
    ctx.next_draw_id.set(id + 1);
    let alt = chart.title.clone().unwrap_or_else(|| "图表".to_string());
    let drawing = format!(
        concat!(
            "<w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">",
            "<wp:extent cx=\"{cx}\" cy=\"{cy}\"/>",
            "<wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>",
            "<wp:docPr id=\"{id}\" name=\"Chart {id}\" descr=\"{alt}\"/>",
            "<wp:cNvGraphicFramePr/>",
            "<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">",
            "<c:chart xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" ",
            "xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:id=\"{rid}\"/>",
            "</a:graphicData></a:graphic></wp:inline></w:drawing>"
        ),
        cx = cx,
        cy = cy,
        id = id,
        alt = esc_attr(&alt),
        rid = rid
    );
    let run = format!("<w:r>{drawing}</w:r>");
    let align = chart.align.clone().unwrap_or_else(|| "center".to_string());
    let ppr = empty("w:jc", &[("w:val", jc_val(&align))]);
    paragraph_with_ppr(&ppr, &run)
}

/// 文本框 / 形状（DrawingML wps）
fn textbox_xml(ctx: &GenCtx, tb: &TextBoxBlock) -> String {
    if tb.vml == Some(true) {
        return vml_textbox_xml(ctx, tb);
    }
    let w = tb.width.unwrap_or(200.0);
    let h = tb.height.unwrap_or(60.0);
    let cx = pt_to_emu(w);
    let cy = pt_to_emu(h);
    let id = ctx.next_draw_id.get();
    ctx.next_draw_id.set(id + 1);

    let mut sp_pr = format!(
        "<a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
         <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>"
    );
    if let Some(fill) = &tb.fill {
        sp_pr.push_str(&format!(
            "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            crate::utils::xml::color_hex(fill)
        ));
    } else {
        sp_pr.push_str("<a:noFill/>");
    }
    if let Some(line) = &tb.line {
        sp_pr.push_str(&format!(
            "<a:ln><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:ln>",
            crate::utils::xml::color_hex(line)
        ));
    }

    // 内容段落
    let mut rpr = String::new();
    if let Some(sz) = tb.font_size {
        let hp = pt_to_half(sz).to_string();
        rpr.push_str(&format!("<w:sz w:val=\"{hp}\"/><w:szCs w:val=\"{hp}\"/>"));
    }
    if let Some(c) = &tb.color {
        rpr.push_str(&format!(
            "<w:color w:val=\"{}\"/>",
            crate::utils::xml::color_hex(c)
        ));
    }
    let rpr_tag = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };
    let jc = tb
        .align
        .as_ref()
        .map(|a| empty("w:jc", &[("w:val", jc_val(a))]))
        .unwrap_or_default();
    let para = format!(
        "<w:p><w:pPr>{jc}</w:pPr><w:r>{rpr_tag}<w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        esc(&tb.text)
    );

    let wsp = format!(
        "<wps:wsp xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\">\
         <wps:cNvSpPr txBox=\"1\"/><wps:spPr>{sp_pr}</wps:spPr>\
         <wps:txbx><w:txbxContent>{para}</w:txbxContent></wps:txbx>\
         <wps:bodyPr rot=\"0\" wrap=\"square\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\" anchor=\"t\"/></wps:wsp>"
    );
    let graphic = format!(
        "<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingShape\">{wsp}</a:graphicData></a:graphic>"
    );
    let inline_pr = format!(
        "<wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>\
         <wp:docPr id=\"{id}\" name=\"TextBox {id}\"/><wp:cNvGraphicFramePr/>"
    );
    let floating = tb
        .wrap
        .as_deref()
        .map(|w| !w.eq_ignore_ascii_case("inline"))
        .unwrap_or(false);
    let drawing = if floating {
        let x = pt_to_emu(tb.x.unwrap_or(0.0));
        let y = pt_to_emu(tb.y.unwrap_or(0.0));
        format!(
            "<w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"114300\" distR=\"114300\" \
             simplePos=\"0\" relativeHeight=\"251658240\" behindDoc=\"0\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\">\
             <wp:simplePos x=\"0\" y=\"0\"/>\
             <wp:positionH relativeFrom=\"page\"><wp:posOffset>{x}</wp:posOffset></wp:positionH>\
             <wp:positionV relativeFrom=\"page\"><wp:posOffset>{y}</wp:posOffset></wp:positionV>\
             {inline_pr}<wp:wrapSquare wrapText=\"bothSides\"/>{graphic}</wp:anchor></w:drawing>"
        )
    } else {
        format!("<w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">{inline_pr}{graphic}</wp:inline></w:drawing>")
    };
    let run = format!("<w:r>{drawing}</w:r>");
    paragraph_with_ppr("", &run)
}

/// 附件：OLE 嵌入对象（图标预览 + 双击打开）
fn attachment_xml(ctx: &GenCtx, a: &AttachmentBlock) -> String {
    if ctx.attach_failed.contains(&a.src) {
        return String::new();
    }
    let Some(embed_rid) = ctx.attach_embed_rid.get(&a.src) else {
        return String::new();
    };
    let icon_rid = ctx.attach_icon_rid.get(&a.src).cloned().unwrap_or_default();
    let prog = ctx
        .attach_progid
        .get(&a.src)
        .cloned()
        .unwrap_or_else(|| "Package".to_string());
    let (w, h) = ctx.attach_size.get(&a.src).copied().unwrap_or((72.0, 72.0));
    let n = ctx.next_draw_id.get();
    ctx.next_draw_id.set(n + 1);
    let shape_id = format!("_x0000_i{n}");
    let objid = format!("_{}", 100000000 + n);
    let field = format!(
        concat!(
            "<w:r><w:object>",
            "<v:shapetype id=\"_x0000_t75\" coordsize=\"21600,21600\" o:spt=\"75\" o:preferrelative=\"t\" ",
            "path=\"m@4@5l@4@11@9@11@9@5xe\" filled=\"f\" stroked=\"f\">",
            "<v:stroke joinstyle=\"miter\"/><v:formulas>",
            "<v:f eqn=\"if lineDrawn pixelLineWidth 0\"/><v:f eqn=\"sum @0 1 0\"/><v:f eqn=\"sum 0 0 @1\"/>",
            "<v:f eqn=\"prod @2 1 2\"/><v:f eqn=\"prod @3 21600 pixelWidth\"/><v:f eqn=\"prod @3 21600 pixelHeight\"/>",
            "<v:f eqn=\"sum @0 0 1\"/><v:f eqn=\"prod @6 1 2\"/><v:f eqn=\"prod @7 21600 pixelWidth\"/>",
            "<v:f eqn=\"sum @8 21600 0\"/><v:f eqn=\"prod @7 21600 pixelHeight\"/><v:f eqn=\"sum @10 21600 0\"/>",
            "</v:formulas><v:path o:extrusionok=\"f\" gradientshapeok=\"t\" o:connecttype=\"rect\"/>",
            "<o:lock v:ext=\"edit\" aspectratio=\"t\"/></v:shapetype>",
            "<v:shape id=\"{sid}\" type=\"#_x0000_t75\" style=\"width:{w}pt;height:{h}pt\" o:ole=\"\">",
            "<v:imagedata r:id=\"{icon}\" o:title=\"\"/></v:shape>",
            "<o:OLEObject Type=\"Embed\" ProgID=\"{prog}\" ShapeID=\"{sid}\" DrawAspect=\"Icon\" ",
            "ObjectID=\"{objid}\" r:id=\"{embed}\" w:UpdateMode=\"Always\"/>",
            "</w:object></w:r>"
        ),
        sid = shape_id,
        w = w,
        h = h,
        icon = icon_rid,
        prog = esc_attr(&prog),
        objid = objid,
        embed = embed_rid
    );
    if let Some(label) = ctx.attach_label.get(&a.src) {
        if !label.is_empty() {
            return paragraph_with_ppr(
                "",
                &format!(
                    "{}<w:r><w:t xml:space=\"preserve\"> {}</w:t></w:r>",
                    field,
                    esc(label)
                ),
            );
        }
    }
    paragraph_with_ppr("", &field)
}

/// VML 文本框（w:pict → v:shape t202）
fn vml_textbox_xml(ctx: &GenCtx, tb: &TextBoxBlock) -> String {
    let w = tb.width.unwrap_or(200.0);
    let h = tb.height.unwrap_or(60.0);
    let n = ctx.next_draw_id.get();
    ctx.next_draw_id.set(n + 1);
    let mut style = String::new();
    if let (Some(x), Some(y)) = (tb.x, tb.y) {
        style.push_str(&format!(
            "position:absolute;margin-left:{x}pt;margin-top:{y}pt;"
        ));
    }
    style.push_str(&format!("width:{w}pt;height:{h}pt"));
    let fill = tb
        .fill
        .as_ref()
        .map(|c| format!(" fillcolor=\"#{}\"", crate::utils::xml::color_hex(c)))
        .unwrap_or_default();
    let stroke = match &tb.line {
        Some(c) => format!("<v:stroke color=\"#{}\"/>", crate::utils::xml::color_hex(c)),
        None => "<v:stroke on=\"f\"/>".to_string(),
    };
    let jc = tb
        .align
        .as_ref()
        .map(|a| empty("w:jc", &[("w:val", jc_val(a))]))
        .unwrap_or_default();
    let para = format!(
        "<w:p><w:pPr>{jc}</w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        esc(&tb.text)
    );
    let run = format!(
        concat!(
            "<w:r><w:pict>",
            "<v:shapetype id=\"_x0000_t202\" coordsize=\"21600,21600\" o:spt=\"202\" ",
            "path=\"m,l,21600r21600,l21600,xe\"><v:stroke joinstyle=\"miter\"/>",
            "<v:path gradientshapeok=\"t\" o:connecttype=\"rect\"/></v:shapetype>",
            "<v:shape id=\"TextBox{n}\" type=\"#_x0000_t202\" style=\"{style}\"{fill}>",
            "{stroke}<v:textbox><w:txbxContent>{para}</w:txbxContent></v:textbox>",
            "</v:shape></w:pict></w:r>"
        ),
        n = n,
        style = style,
        fill = fill,
        stroke = stroke,
        para = para
    );
    paragraph_with_ppr("", &run)
}

/// 形状 / 线条（DrawingML wps 预设形状）
fn shape_xml(ctx: &GenCtx, sh: &ShapeBlock) -> String {
    let w = sh.width.unwrap_or(120.0);
    let h = sh.height.unwrap_or(80.0);
    let cx = pt_to_emu(w).max(1);
    let cy = pt_to_emu(h).max(1);
    let id = ctx.next_draw_id.get();
    ctx.next_draw_id.set(id + 1);
    let prst = {
        let t = sh.shape_type.trim();
        if t.is_empty() {
            "rect"
        } else {
            t
        }
    };
    let rot = sh
        .rotation
        .map(|r| format!(" rot=\"{}\"", (r * 60000.0).round() as i64))
        .unwrap_or_default();
    let mut sp = format!(
        "<a:xfrm{rot}><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
         <a:prstGeom prst=\"{}\"><a:avLst/></a:prstGeom>",
        esc_attr(prst)
    );
    match &sh.fill {
        Some(f) => sp.push_str(&format!(
            "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            crate::utils::xml::color_hex(f)
        )),
        None => sp.push_str("<a:noFill/>"),
    }
    if let Some(l) = &sh.line {
        let lw = pt_to_emu(sh.line_width.unwrap_or(1.0)).max(1);
        sp.push_str(&format!(
            "<a:ln w=\"{lw}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:ln>",
            crate::utils::xml::color_hex(l)
        ));
    }
    let txbx = match &sh.text {
        Some(t) if !t.is_empty() => {
            let mut rpr = String::new();
            if let Some(sz) = sh.font_size {
                let hp = pt_to_half(sz).to_string();
                rpr.push_str(&format!("<w:sz w:val=\"{hp}\"/><w:szCs w:val=\"{hp}\"/>"));
            }
            if let Some(c) = &sh.text_color {
                rpr.push_str(&format!(
                    "<w:color w:val=\"{}\"/>",
                    crate::utils::xml::color_hex(c)
                ));
            }
            let rpr_tag = if rpr.is_empty() {
                String::new()
            } else {
                format!("<w:rPr>{rpr}</w:rPr>")
            };
            let jc = sh
                .align
                .as_ref()
                .map(|a| empty("w:jc", &[("w:val", jc_val(a))]))
                .unwrap_or_default();
            format!(
                "<wps:txbx><w:txbxContent><w:p><w:pPr>{jc}</w:pPr><w:r>{rpr_tag}\
                 <w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:txbxContent></wps:txbx>",
                esc(t)
            )
        }
        _ => String::new(),
    };
    let wsp = format!(
        "<wps:wsp xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\">\
         <wps:cNvSpPr/><wps:spPr>{sp}</wps:spPr>{txbx}\
         <wps:bodyPr rot=\"0\" lIns=\"91440\" tIns=\"45720\" rIns=\"91440\" bIns=\"45720\" anchor=\"ctr\"/></wps:wsp>"
    );
    let graphic = format!(
        "<a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingShape\">{wsp}</a:graphicData></a:graphic>"
    );
    let inline_pr = format!(
        "<wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>\
         <wp:docPr id=\"{id}\" name=\"Shape {id}\"/><wp:cNvGraphicFramePr/>"
    );
    let floating = sh
        .wrap
        .as_deref()
        .map(|x| !x.eq_ignore_ascii_case("inline"))
        .unwrap_or(false)
        || sh.x.is_some()
        || sh.y.is_some();
    let drawing = if floating {
        let x = pt_to_emu(sh.x.unwrap_or(0.0));
        let y = pt_to_emu(sh.y.unwrap_or(0.0));
        format!(
            "<w:drawing><wp:anchor distT=\"0\" distB=\"0\" distL=\"114300\" distR=\"114300\" \
             simplePos=\"0\" relativeHeight=\"251658240\" behindDoc=\"0\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\">\
             <wp:simplePos x=\"0\" y=\"0\"/>\
             <wp:positionH relativeFrom=\"page\"><wp:posOffset>{x}</wp:posOffset></wp:positionH>\
             <wp:positionV relativeFrom=\"page\"><wp:posOffset>{y}</wp:posOffset></wp:positionV>\
             {inline_pr}<wp:wrapSquare wrapText=\"bothSides\"/>{graphic}</wp:anchor></w:drawing>"
        )
    } else {
        format!("<w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">{inline_pr}{graphic}</wp:inline></w:drawing>")
    };
    paragraph_with_ppr("", &format!("<w:r>{drawing}</w:r>"))
}

fn formula_xml(f: &FormulaBlock) -> String {
    let inner = crate::generate::math::latex_to_omml(&f.latex);
    let omml = format!("<m:oMath>{inner}</m:oMath>");
    if f.display {
        format!("<m:oMathPara>{omml}</m:oMathPara>")
    } else {
        format!("<w:p>{omml}</w:p>")
    }
}

fn code_xml(c: &CodeBlock) -> String {
    let mut out = String::new();
    for line in c.text.split('\n') {
        let ppr = "<w:pStyle w:val=\"Code\"/>".to_string();
        out.push_str(&paragraph_with_ppr(&ppr, &run_plain(line)));
    }
    out
}

fn quote_xml(q: &QuoteBlock) -> String {
    let ppr = "<w:pStyle w:val=\"Quote\"/>".to_string();
    paragraph_with_ppr(&ppr, &run_plain(&q.text))
}

fn caption_xml(text: &str) -> String {
    let ppr = "<w:pStyle w:val=\"Caption\"/>".to_string();
    paragraph_with_ppr(&ppr, &run_plain(text))
}

/// 是否已被手动编号（以 图/表/Figure/Table 开头）
fn already_numbered(text: &str) -> bool {
    let t = text.trim_start();
    let lower = t.to_ascii_lowercase();
    t.starts_with('图')
        || t.starts_with('表')
        || lower.starts_with("figure")
        || lower.starts_with("table")
}

/// 自动编号题注：未手动编号时插入 live SEQ 域（缓存当前序号）
fn auto_caption(ctx: &GenCtx, kind: &str, text: &str) -> String {
    if already_numbered(text) {
        return caption_xml(text);
    }
    let (label, identifier) = if kind == "table" {
        ("表", "Table")
    } else {
        ("图", "Figure")
    };
    let n = if kind == "table" {
        let v = ctx.tbl_seq.get() + 1;
        ctx.tbl_seq.set(v);
        v
    } else {
        let v = ctx.fig_seq.get() + 1;
        ctx.fig_seq.set(v);
        v
    };
    numbered_caption_xml(label, identifier, n, text)
}

fn numbered_caption_xml(label: &str, identifier: &str, n: u32, text: &str) -> String {
    let field = format!(
        concat!(
            "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>",
            "<w:r><w:instrText xml:space=\"preserve\"> SEQ {id} \\* ARABIC </w:instrText></w:r>",
            "<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>",
            "<w:r><w:t>{n}</w:t></w:r>",
            "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
        ),
        id = identifier,
        n = n
    );
    let head = format!(
        "<w:r><w:t xml:space=\"preserve\">{} </w:t></w:r>",
        esc(label)
    );
    let tail = format!(
        "<w:r><w:t xml:space=\"preserve\">  {}</w:t></w:r>",
        esc(text)
    );
    let ppr = "<w:pStyle w:val=\"Caption\"/>".to_string();
    paragraph_with_ppr(&ppr, &format!("{head}{field}{tail}"))
}

fn toc_xml(ctx: &GenCtx, t: &TocBlock) -> String {
    let levels = t.levels.clone().unwrap_or_else(|| vec![1, 2, 3]);
    let mut out = String::new();

    if t.is_static == Some(true) {
        // 静态目录：遍历标题生成点前导行，跨查看器可靠（无自动页码）
        out.push_str(&heading_xml(1, &t.title, None));
        for block in ctx.doc.parts.iter().flat_map(|p| p.blocks.iter()) {
            if let Block::Heading { level, text, .. } = block {
                if levels.contains(level) {
                    let lvl = (*level).clamp(1, 3);
                    let ppr = format!("<w:pStyle w:val=\"TOC{lvl}\"/>");
                    let content = format!(
                        "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r><w:r><w:tab/></w:r>",
                        esc(text)
                    );
                    out.push_str(&paragraph_with_ppr(&ppr, &content));
                }
            }
        }
        return out;
    }

    // 标题
    out.push_str(&heading_xml(1, &t.title, None));
    // TOC 域
    let range = format!(
        "{}-{}",
        levels.first().copied().unwrap_or(1),
        levels.last().copied().unwrap_or(3)
    );
    // 缓存结果：优先使用原文档目录项，保证未更新域时也正确显示
    let cached = if !t.entries.is_empty() {
        let mut s = String::new();
        for e in &t.entries {
            let lvl = e.level.clamp(1, 3);
            s.push_str(&paragraph_with_ppr(
                &format!("<w:pStyle w:val=\"TOC{lvl}\"/>"),
                &format!(
                    "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r><w:r><w:tab/></w:r>",
                    esc(&e.text)
                ),
            ));
        }
        s
    } else {
        "<w:r><w:t>请在 Word 中按 F9 更新目录</w:t></w:r>".to_string()
    };
    // 域开始段
    out.push_str(&paragraph_with_ppr(
        "",
        &format!(
            "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>\
             <w:r><w:instrText xml:space=\"preserve\"> TOC \\o \"{range}\" \\h \\z \\u </w:instrText></w:r>\
             <w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>"
        ),
    ));
    // 缓存结果（独立段落）
    out.push_str(&cached);
    // 域结束段
    out.push_str(&paragraph_with_ppr(
        "",
        "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>",
    ));
    out
}

fn bibliography_xml(_ctx: &GenCtx, bib: &BibliographyBlock) -> String {
    let mut out = String::new();
    // 仅当显式给出非空标题时才输出标题（避免与作者自加标题重复）
    if let Some(t) = &bib.title {
        if !t.is_empty() {
            out.push_str(&heading_xml(1, t, None));
        }
    }
    for entry in &bib.entries {
        let text = format_bib_entry(entry, &bib.style);
        let ppr =
            "<w:pStyle w:val=\"Reference\"/><w:ind w:left=\"720\" w:hanging=\"720\"/>".to_string();
        out.push_str(&paragraph_with_ppr(&ppr, &run_plain(&text)));
    }
    out
}

/// 按引用格式拼装参考文献条目
pub(crate) fn format_bib_entry(e: &BibEntry, style: &str) -> String {
    let authors = e.authors.join(", ");
    if style.contains("7714") {
        // 顺序编码：作者. 题名[类型]. 刊名, 年.
        let kind = match e.kind.as_deref() {
            Some("book") => "[M]",
            Some("inproceedings") => "[C]",
            Some("web") => "[EB/OL]",
            _ => "[J]",
        };
        let mut s = String::new();
        if !authors.is_empty() {
            s.push_str(&format!("{authors}. "));
        }
        s.push_str(&format!("{}{kind}. ", e.title));
        if let Some(c) = &e.container {
            s.push_str(&format!("{c}, "));
        }
        if let Some(y) = e.year {
            s.push_str(&format!("{y}."));
        }
        s
    } else {
        // 简化著者-出版年
        let mut s = String::new();
        if !authors.is_empty() {
            s.push_str(&format!("{authors}. "));
        }
        if let Some(y) = e.year {
            s.push_str(&format!("({y}). "));
        }
        s.push_str(&format!("{}. ", e.title));
        if let Some(c) = &e.container {
            s.push_str(&format!("{c}. "));
        }
        if let Some(p) = &e.publisher {
            s.push_str(&format!("{p}. "));
        }
        if let Some(u) = &e.url {
            s.push_str(u);
        }
        s
    }
}

/// 未建模内容原样输出，并重映射其中的关系 Id
fn raw_xml(ctx: &GenCtx, r: &RawBlock) -> String {
    let mut xml = r.xml.clone();
    // 两阶段替换，避免新旧 rId 相互覆盖（如 orig rId7 -> new rId7 的连锁替换）
    for rel in &r.rels {
        xml = xml.replace(&format!("\"{}\"", rel.id), &format!("\u{1}{}\u{1}", rel.id));
    }
    for rel in &r.rels {
        if let Some(new_id) = ctx.raw_rel_map.get(&rel.id) {
            xml = xml.replace(&format!("\u{1}{}\u{1}", rel.id), &format!("\"{new_id}\""));
        }
    }
    xml
}

/// 内容控件 sdtPr
fn sdt_pr_xml(p: &SdtProps) -> String {
    let mut pr = String::new();
    if let Some(a) = &p.alias {
        pr.push_str(&empty("w:alias", &[("w:val", a.clone())]));
    }
    if let Some(t) = &p.tag {
        pr.push_str(&empty("w:tag", &[("w:val", t.clone())]));
    }
    if let Some(l) = &p.lock {
        pr.push_str(&empty("w:lock", &[("w:val", l.clone())]));
    }
    if let Some(pl) = &p.placeholder {
        pr.push_str(&format!(
            "<w:placeholder><w:docPart w:val=\"{}\"/></w:placeholder>",
            esc_attr(pl)
        ));
    }
    match p.sdt_type.as_deref() {
        Some("text") => pr.push_str("<w:text/>"),
        Some("richText") => pr.push_str("<w:richText/>"),
        Some("date") => pr.push_str("<w:date/>"),
        Some("checkbox") => pr.push_str("<w:checkbox/>"),
        Some("picture") => pr.push_str("<w:picture/>"),
        Some("dropDownList") | Some("comboBox") => {
            let tag = p.sdt_type.as_deref().unwrap();
            pr.push_str(&format!("<w:{tag}>"));
            for it in &p.items {
                pr.push_str(&format!(
                    "<w:listItem w:displayText=\"{}\" w:value=\"{}\"/>",
                    esc_attr(it),
                    esc_attr(it)
                ));
            }
            pr.push_str(&format!("</w:{tag}>"));
        }
        _ => {}
    }
    pr
}

fn sdt_xml(ctx: &GenCtx, c: &ContentControlBlock) -> String {
    let content = blocks_xml(ctx, &c.blocks);
    format!(
        "<w:sdt><w:sdtPr>{}</w:sdtPr><w:sdtContent>{content}</w:sdtContent></w:sdt>",
        sdt_pr_xml(&c.props)
    )
}

/// 通用域（五段式：begin/instrText/separate/result/end）
fn field_xml(instr: &str, cached: &str) -> String {
    let result = if cached.is_empty() {
        String::new()
    } else {
        format!(
            "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
            esc(cached)
        )
    };
    format!(
        "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>\
         <w:r><w:instrText xml:space=\"preserve\"> {instr} </w:instrText></w:r>\
         <w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>{result}\
         <w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
    )
}

/// 表单域（含 w:ffData）
fn form_field_xml(ff: &FormField, cached: &str) -> String {
    let (instr, data) = match ff.kind.as_str() {
        "checkbox" => {
            let checked = if ff.checked == Some(true) { "1" } else { "0" };
            (
                " FORMCHECKBOX ",
                format!(
                    "<w:checkBox><w:sizeAuto/><w:default w:val=\"0\"/><w:checked w:val=\"{checked}\"/></w:checkBox>"
                ),
            )
        }
        "dropdown" => {
            let mut s = String::from("<w:dropDownList>");
            for it in &ff.items {
                s.push_str(&format!("<w:listEntry w:val=\"{}\"/>", esc_attr(it)));
            }
            s.push_str("</w:dropDownList>");
            (" FORMDROPDOWN ", s)
        }
        _ => (" FORMTEXT ", "<w:textInput/>".to_string()),
    };
    let name = ff
        .name
        .as_deref()
        .map(|n| format!("<w:name w:val=\"{}\"/>", esc_attr(n)))
        .unwrap_or_default();
    let result = if cached.is_empty() {
        String::new()
    } else {
        format!(
            "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
            esc(cached)
        )
    };
    format!(
        "<w:r><w:fldChar w:fldCharType=\"begin\"><w:ffData>{name}{data}</w:ffData></w:fldChar></w:r>\
         <w:r><w:instrText xml:space=\"preserve\">{instr}</w:instrText></w:r>\
         <w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>{result}\
         <w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
    )
}

fn page_break_xml() -> String {
    let run = "<w:r><w:br w:type=\"page\"/></w:r>";
    format!("<w:p>{run}</w:p>")
}

/// 分片的有效页面设置（section.page 优先，否则文档 page）
fn effective_page(ctx: &GenCtx, part: &crate::model::Part) -> PageSetup {
    part.section
        .as_ref()
        .and_then(|s| s.page.clone())
        .unwrap_or_else(|| ctx.doc.page.clone())
}

fn sect_pr(ctx: &GenCtx, page: &PageSetup, part_idx: usize, page_num_start: Option<u32>) -> String {
    let (w, h) = page.size_pt();
    let m = &page.margins;
    let mut s = String::new();
    s.push_str("<w:sectPr>");
    // 分节符类型（continuous 等）；仅对存在 section 的分片输出
    if let Some(st) = ctx
        .doc
        .parts
        .get(part_idx)
        .and_then(|p| p.section.as_ref())
        .and_then(|sec| sec.section_type.clone())
    {
        s.push_str(&empty("w:type", &[("w:val", st)]));
    }
    let mut hdrftr = String::new();
    if let Some(refs) = ctx.part_header_refs.get(part_idx) {
        for (kind, rid) in refs {
            hdrftr.push_str(&format!(
                "<w:headerReference w:type=\"{kind}\" r:id=\"{rid}\"/>"
            ));
        }
    }
    if let Some(refs) = ctx.part_footer_refs.get(part_idx) {
        for (kind, rid) in refs {
            hdrftr.push_str(&format!(
                "<w:footerReference w:type=\"{kind}\" r:id=\"{rid}\"/>"
            ));
        }
    }
    if ctx.title_pages.get(part_idx).copied().unwrap_or(false) {
        hdrftr.push_str("<w:titlePg/>");
    }
    s.push_str(&hdrftr);
    if page.size_specified != Some(false) {
        s.push_str(&empty(
            "w:pgSz",
            &[
                ("w:w", pt_to_twip(w).to_string()),
                ("w:h", pt_to_twip(h).to_string()),
                (
                    "w:orient",
                    if page.orientation.eq_ignore_ascii_case("landscape") {
                        "landscape".to_string()
                    } else {
                        "portrait".to_string()
                    },
                ),
            ],
        ));
    }
    s.push_str(&empty(
        "w:pgMar",
        &[
            ("w:top", pt_to_twip(m.top).to_string()),
            ("w:right", pt_to_twip(m.right).to_string()),
            ("w:bottom", pt_to_twip(m.bottom).to_string()),
            ("w:left", pt_to_twip(m.left).to_string()),
            ("w:header", pt_to_twip(m.header).to_string()),
            ("w:footer", pt_to_twip(m.footer).to_string()),
            (
                "w:gutter",
                pt_to_twip(page.gutter.unwrap_or(0.0)).to_string(),
            ),
        ],
    ));
    let pnf = ctx
        .doc
        .parts
        .get(part_idx)
        .and_then(|p| p.section.as_ref())
        .and_then(|sec| sec.page_number_format.clone());
    if page_num_start.is_some() || pnf.is_some() {
        let mut a: Vec<(&str, String)> = Vec::new();
        if let Some(start) = page_num_start {
            a.push(("w:start", start.to_string()));
        }
        if let Some(f) = &pnf {
            a.push(("w:fmt", f.clone()));
        }
        s.push_str(&empty("w:pgNumType", &a));
    }
    if let Some(b) = &page.page_border {
        let val = b.style.clone().unwrap_or_else(|| "single".to_string());
        let sz = (b.width.unwrap_or(0.5) * 8.0).round().max(2.0) as i64;
        let color = match b.color.as_deref() {
            Some(c) if !c.is_empty() => c.trim_start_matches('#').to_uppercase(),
            _ => "auto".to_string(),
        };
        let mut pb = String::from("<w:pgBorders w:offsetFrom=\"page\">");
        for e in ["top", "left", "bottom", "right"] {
            pb.push_str(&format!(
                "<w:{e} w:val=\"{}\" w:sz=\"{sz}\" w:space=\"24\" w:color=\"{color}\"/>",
                esc_attr(&val)
            ));
        }
        pb.push_str("</w:pgBorders>");
        s.push_str(&pb);
    }
    if let Some(cols) = page.columns {
        if cols > 1 {
            s.push_str(&empty(
                "w:cols",
                &[
                    ("w:num", cols.to_string()),
                    (
                        "w:space",
                        pt_to_twip(page.column_space.unwrap_or(36.0)).to_string(),
                    ),
                ],
            ));
        }
    }
    if let Some(dg) = &page.doc_grid {
        s.push_str(dg);
    }
    if page.rtl == Some(true) {
        s.push_str("<w:bidi/>");
    }
    if page.rtl_gutter == Some(true) {
        s.push_str("<w:rtlGutter/>");
    }
    s.push_str("</w:sectPr>");
    s
}

// ---------- 通用 ----------

fn paragraph_with_ppr(ppr: &str, content: &str) -> String {
    if ppr.is_empty() {
        format!("<w:p>{content}</w:p>")
    } else {
        format!("<w:p><w:pPr>{ppr}</w:pPr>{content}</w:p>")
    }
}

fn run_plain(text: &str) -> String {
    run_xml_no_ctx(&Run::plain(text))
}

fn run_xml(ctx: &GenCtx, r: &Run) -> String {
    let mut out = String::new();

    // 批注范围起点
    let comment_id = r.comment.as_ref().map(|_| {
        let v = ctx.comment_seq.get() + 1;
        ctx.comment_seq.set(v);
        v
    });
    if let Some(id) = comment_id {
        out.push_str(&format!("<w:commentRangeStart w:id=\"{id}\"/>"));
    }

    // 书签起点
    let bm_id = r.bookmark.as_ref().map(|_| {
        let v = ctx.bookmark_id.get();
        ctx.bookmark_id.set(v + 1);
        v
    });
    if let (Some(name), Some(id)) = (&r.bookmark, bm_id) {
        out.push_str(&format!(
            "<w:bookmarkStart w:id=\"{id}\" w:name=\"{}\"/>",
            esc_attr(name)
        ));
    }

    // 主体：行内图片 / 注音 / 交叉引用域 / 超链接 / 普通 run（按需包裹修订）
    let payload = if let Some(ff) = &r.form_field {
        form_field_xml(ff, &r.text)
    } else if let Some(instr) = &r.field {
        field_xml(instr, &r.text)
    } else if let Some(img) = &r.image {
        let d = image_drawing_xml(ctx, img);
        if d.is_empty() {
            String::new()
        } else {
            format!("<w:r>{d}</w:r>")
        }
    } else if let Some(rt) = &r.ruby {
        format!(
            "<w:r><w:ruby><w:rubyPr><w:rubyAlign w:val=\"distributeSpace\"/>\
             <w:hps w:val=\"20\"/><w:hpsRaise w:val=\"20\"/><w:hpsBaseText w:val=\"40\"/>\
             <w:lid w:val=\"ja-JP\"/></w:rubyPr>\
             <w:rt><w:r><w:rPr><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/></w:rPr>\
             <w:t xml:space=\"preserve\">{}</w:t></w:r></w:rt>\
             <w:rubyBase><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:rubyBase>\
             </w:ruby></w:r>",
            esc(rt),
            esc(&r.text)
        )
    } else if let Some(target) = &r.ref_target {
        field_ref_xml(target, r.pageref == Some(true), &r.text)
    } else if let Some(url) = &r.hyperlink {
        let inner = if let Some(img) = &r.image {
            let d = image_drawing_xml(ctx, img);
            if d.is_empty() {
                run_xml_no_ctx(r)
            } else {
                format!("<w:r>{d}</w:r>")
            }
        } else {
            run_xml_no_ctx(r)
        };
        if let Some(anchor) = url.strip_prefix('#') {
            format!(
                "<w:hyperlink w:anchor=\"{}\" w:history=\"1\">{inner}</w:hyperlink>",
                esc_attr(anchor)
            )
        } else {
            let rid = ctx
                .hyper_rid
                .get(url)
                .cloned()
                .unwrap_or_else(|| "rId0".to_string());
            format!("<w:hyperlink r:id=\"{rid}\" w:history=\"1\">{inner}</w:hyperlink>")
        }
    } else if !r.text.is_empty() || !run_props(r).is_empty() || r.symbol.is_some() {
        run_xml_no_ctx(r)
    } else {
        String::new()
    };
    out.push_str(&wrap_revision(ctx, r, &payload));

    // 脚注引用
    if r.footnote.is_some() {
        let id = ctx.footnote_seq.get() + 1;
        ctx.footnote_seq.set(id);
        out.push_str(&format!(
            "<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/><w:vertAlign w:val=\"superscript\"/></w:rPr><w:footnoteReference w:id=\"{id}\"/></w:r>"
        ));
    }
    // 尾注引用
    if r.endnote.is_some() {
        let id = ctx.endnote_seq.get() + 1;
        ctx.endnote_seq.set(id);
        out.push_str(&format!(
            "<w:r><w:rPr><w:rStyle w:val=\"EndnoteReference\"/><w:vertAlign w:val=\"superscript\"/></w:rPr><w:endnoteReference w:id=\"{id}\"/></w:r>"
        ));
    }

    // 书签终点
    if let Some(id) = bm_id {
        out.push_str(&format!("<w:bookmarkEnd w:id=\"{id}\"/>"));
    }

    // 批注范围终点 + 引用
    if let Some(id) = comment_id {
        out.push_str(&format!("<w:commentRangeEnd w:id=\"{id}\"/>"));
        out.push_str(&format!(
            "<w:r><w:rPr><w:rStyle w:val=\"CommentReference\"/></w:rPr><w:commentReference w:id=\"{id}\"/></w:r>"
        ));
    }
    out
}

/// 按 run.revision 包裹 w:ins / w:del（删除时文本改用 w:delText）
fn wrap_revision(ctx: &GenCtx, r: &Run, payload: &str) -> String {
    if payload.is_empty() {
        return String::new();
    }
    let revision_id = |ctx: &GenCtx| {
        let v = ctx.revision_id.get();
        ctx.revision_id.set(v + 1);
        v
    };
    match r.revision.as_deref() {
        Some("ins") => {
            let id = revision_id(ctx);
            let author = r.revision_author.as_deref().unwrap_or("json2docx");
            format!(
                "<w:ins w:id=\"{id}\" w:author=\"{}\" w:date=\"{}\">{payload}</w:ins>",
                esc_attr(author),
                REVISION_DATE
            )
        }
        Some("del") => {
            let id = revision_id(ctx);
            let author = r.revision_author.as_deref().unwrap_or("json2docx");
            let del = payload
                .replace("<w:t", "<w:delText")
                .replace("</w:t>", "</w:delText>");
            format!(
                "<w:del w:id=\"{id}\" w:author=\"{}\" w:date=\"{}\">{del}</w:del>",
                esc_attr(author),
                REVISION_DATE
            )
        }
        _ => payload.to_string(),
    }
}

/// 生成 REF / PAGEREF 交叉引用域
fn field_ref_xml(target: &str, pageref: bool, cached: &str) -> String {
    let instr = if pageref {
        format!(" PAGEREF {target} \\h ")
    } else {
        format!(" REF {target} \\h ")
    };
    let cached = if cached.is_empty() { target } else { cached };
    format!(
        concat!(
            "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>",
            "<w:r><w:instrText xml:space=\"preserve\">{instr}</w:instrText></w:r>",
            "<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>",
            "<w:r><w:t>{cached}</w:t></w:r>",
            "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
        ),
        instr = esc(&instr),
        cached = esc(cached)
    )
}

fn run_xml_no_ctx(r: &Run) -> String {
    let rpr = run_props(r);
    let text = run_text_only(r);
    let base = if rpr.is_empty() {
        format!("<w:r>{text}</w:r>")
    } else {
        format!("<w:r><w:rPr>{rpr}</w:rPr>{text}</w:r>")
    };
    if let Some(sp) = &r.sdt {
        format!(
            "<w:sdt><w:sdtPr>{}</w:sdtPr><w:sdtContent>{base}</w:sdtContent></w:sdt>",
            sdt_pr_xml(sp)
        )
    } else {
        base
    }
}

fn run_text_only(r: &Run) -> String {
    if let Some(sym) = &r.symbol {
        let (font, ch) = sym.split_once(':').unwrap_or(("", sym.as_str()));
        let font = if font.is_empty() { "Symbol" } else { font };
        return format!(
            "<w:sym w:font=\"{}\" w:char=\"{}\"/>",
            esc_attr(font),
            esc_attr(ch)
        );
    }
    if r.text.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = r.text.split('\n').collect();
    let mut s = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            s.push_str("<w:br/>");
        }
        if !part.is_empty() {
            s.push_str(&format!("<w:t xml:space=\"preserve\">{}</w:t>", esc(part)));
        }
    }
    s
}

fn run_props(r: &Run) -> String {
    let mut s = String::new();
    if r.code == Some(true) {
        s.push_str("<w:rStyle w:val=\"CodeChar\"/>");
    }
    if r.font_family.is_some() || r.east_asia_font.is_some() || r.cs_font.is_some() {
        let f = r.font_family.clone().unwrap_or_default();
        let ea = r.east_asia_font.clone().unwrap_or_else(|| f.clone());
        let cs = r.cs_font.clone().unwrap_or_else(|| f.clone());
        let fa = if f.is_empty() {
            String::new()
        } else {
            format!(" w:ascii=\"{}\" w:hAnsi=\"{}\"", esc_attr(&f), esc_attr(&f))
        };
        let ea_a = if ea.is_empty() {
            String::new()
        } else {
            format!(" w:eastAsia=\"{}\"", esc_attr(&ea))
        };
        let cs_a = if cs.is_empty() {
            String::new()
        } else {
            format!(" w:cs=\"{}\"", esc_attr(&cs))
        };
        s.push_str(&format!("<w:rFonts{fa}{ea_a}{cs_a}/>"));
    }
    if r.bold == Some(true) {
        s.push_str("<w:b/><w:bCs/>");
    }
    if r.italic == Some(true) {
        s.push_str("<w:i/><w:iCs/>");
    }
    if r.strike == Some(true) {
        s.push_str("<w:strike/>");
    }
    if let Some(c) = &r.color {
        s.push_str(&empty(
            "w:color",
            &[("w:val", crate::utils::xml::color_hex(c))],
        ));
    } else if r.hyperlink.is_some() {
        // 超链接未显式指定颜色：用标准超链接色，避免回环变黑
        s.push_str(&empty("w:color", &[("w:val", "0563C1".to_string())]));
    }
    if let Some(sz) = r.font_size {
        let hp = pt_to_half(sz).to_string();
        s.push_str(&empty("w:sz", &[("w:val", hp.clone())]));
        s.push_str(&empty("w:szCs", &[("w:val", hp)]));
    }
    if let Some(hl) = &r.highlight {
        s.push_str(&empty("w:highlight", &[("w:val", hl.clone())]));
    }
    if let Some(u) = r.underline {
        if u {
            s.push_str("<w:u w:val=\"single\"/>");
        }
    }
    if let Some(fill) = &r.shading {
        s.push_str(&format!(
            "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
            crate::utils::xml::color_hex(fill)
        ));
    }
    if r.superscript == Some(true) {
        s.push_str("<w:vertAlign w:val=\"superscript\"/>");
    }
    if r.subscript == Some(true) {
        s.push_str("<w:vertAlign w:val=\"subscript\"/>");
    }
    if r.caps == Some(true) {
        s.push_str("<w:caps/>");
    }
    if r.small_caps == Some(true) {
        s.push_str("<w:smallCaps/>");
    }
    if r.outline == Some(true) {
        s.push_str("<w:outline/>");
    }
    if r.shadow == Some(true) {
        s.push_str("<w:shadow/>");
    }
    if r.emboss == Some(true) {
        s.push_str("<w:emboss/>");
    }
    if r.imprint == Some(true) {
        s.push_str("<w:imprint/>");
    }
    if let Some(sp) = r.spacing {
        if sp != 0.0 {
            s.push_str(&empty(
                "w:spacing",
                &[("w:val", ((sp * 20.0).round() as i64).to_string())],
            ));
        }
    }
    if r.rtl == Some(true) {
        s.push_str("<w:rtl/>");
    }
    if r.lang.is_some() || r.lang_ea.is_some() {
        let mut a: Vec<(&str, String)> = Vec::new();
        if let Some(v) = &r.lang {
            a.push(("w:val", v.clone()));
        }
        if let Some(v) = &r.lang_ea {
            a.push(("w:eastAsia", v.clone()));
        }
        s.push_str(&empty("w:lang", &a));
    }
    s
}

/// 对齐值归一（justify/both → both）
fn jc_val(align: &str) -> String {
    match align.to_ascii_lowercase().as_str() {
        "center" => "center",
        "right" => "right",
        "justify" | "both" => "both",
        "distribute" => "distribute",
        _ => "left",
    }
    .to_string()
}

/// 渲染一组块（供页眉/页脚复用）
pub(crate) fn blocks_xml(ctx: &GenCtx, blocks: &[crate::model::blocks::Block]) -> String {
    let mut out = String::new();
    for b in blocks {
        out.push_str(&block_xml(ctx, b));
    }
    out
}
