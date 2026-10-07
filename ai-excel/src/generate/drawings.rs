//! Drawing and chart OOXML generation (images + basic charts).

use crate::error::{Error, Result};
use crate::model::sheet::{Chart, Sheet, Size};
use crate::utils::a1::{index_to_col, parse_cell_ref, parse_range};
use crate::utils::image::{detect_extension, load_image_bytes};
use crate::utils::xml::esc_xml;

pub const EMU_PER_INCH: f64 = 914400.0;

const XDR_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const C_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

pub struct MediaFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

pub struct DrawingParts {
    pub xml: String,
    /// (rId, target) for the drawing's own rels file.
    pub rels: Vec<(String, String)>,
    pub media: Vec<MediaFile>,
    /// (file name, chart xml)
    pub charts: Vec<(String, String)>,
    /// 每个 chart 是否为扩展图表（chartEx），与 `charts` 对应。
    pub chart_ex: Vec<bool>,
}

/// Build the drawing part for one sheet, collecting referenced media and
/// chart parts. Returns `None` when the sheet has neither.
pub fn build_sheet_drawing(
    sheet: &Sheet,
    media_counter: &mut usize,
    chart_counter: &mut usize,
    image_cache: &mut std::collections::HashMap<String, String>,
) -> Result<Option<DrawingParts>> {
    if sheet.images.is_empty()
        && sheet.charts.is_empty()
        && sheet.drawing_shapes.is_empty()
        && sheet.shapes.is_empty()
    {
        return Ok(None);
    }

    let mut body = String::new();
    let mut rels: Vec<(String, String)> = Vec::new();
    let mut media: Vec<MediaFile> = Vec::new();
    let mut charts: Vec<(String, String)> = Vec::new();
    let mut chart_ex: Vec<bool> = Vec::new();
    let mut cid: u32 = 1000;

    // 同一张图片（按源路径）在整本工作簿内复用一个 media 部件与 rId。
    let mut name_rid: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for image in &sheet.images {
        let name = if let Some(n) = image_cache.get(&image.src) {
            n.clone()
        } else {
            let bytes = load_image_bytes(&image.src)?;
            // 优先沿用源文件扩展名（如 .jpeg），保持部件名与类型声明稳定。
            let src_ext = std::path::Path::new(&image.src)
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            let ext = match src_ext.as_deref() {
                Some(e @ ("png" | "jpg" | "jpeg" | "gif" | "bmp")) => e.to_string(),
                _ => detect_extension(&bytes, &image.src),
            };
            *media_counter += 1;
            let name = format!("image{}.{ext}", media_counter);
            media.push(MediaFile {
                name: name.clone(),
                bytes,
            });
            image_cache.insert(image.src.clone(), name.clone());
            name
        };
        let rid = if let Some(r) = name_rid.get(&name) {
            r.clone()
        } else {
            let rid = format!("rId{}", rels.len() + 1);
            rels.push((rid.clone(), format!("../media/{name}")));
            name_rid.insert(name.clone(), rid.clone());
            rid
        };

        let size = image.size.clone().unwrap_or(Size { w: 2.0, h: 1.5 });
        let anchor = image.anchor.clone().unwrap_or_else(|| "E2".into());
        body.push_str(&image_anchor(
            cid,
            &anchor,
            &size,
            &rid,
            sheet,
            image.offset,
        ));
        cid += 1;
    }

    for chart in &sheet.charts {
        *chart_counter += 1;
        let chart_no = *chart_counter;
        let chart_name = format!("chart{chart_no}.xml");
        let rid = format!("rId{}", rels.len() + 1);
        rels.push((rid.clone(), format!("../charts/{chart_name}")));
        let (cxml, is_ex) = chart_xml(&sheet.name, chart)?;
        charts.push((chart_name, cxml));
        chart_ex.push(is_ex);

        let size = chart.size.clone().unwrap_or(Size { w: 6.0, h: 4.0 });
        let anchor = chart.anchor.clone().unwrap_or_else(|| "E2".into());
        body.push_str(&chart_anchor(
            cid,
            &anchor,
            &size,
            &rid,
            sheet,
            chart.offset,
            is_ex,
        ));
        cid += 1;
    }

    // 结构化形状（创建用）。
    for s in &sheet.drawing_shapes {
        body.push_str(&shape_anchor(cid, s, sheet));
        cid += 1;
    }

    // 原样回写未建模的形状/连接线。
    for s in &sheet.shapes {
        body.push_str(s);
    }

    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <xdr:wsDr xmlns:xdr=\"{XDR_NS}\" xmlns:a=\"{A_NS}\" xmlns:r=\"{R_NS}\" xmlns:c=\"{C_NS}\" \
         xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" \
         xmlns:a16=\"http://schemas.microsoft.com/office/drawing/2014/main\" \
         xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\">{body}</xdr:wsDr>"
    );

    Ok(Some(DrawingParts {
        xml,
        rels,
        media,
        charts,
        chart_ex,
    }))
}

/// 空绘图部件（仅根节点），用于只有切片器等锚点的情况。
pub fn empty_drawing() -> DrawingParts {
    DrawingParts {
        xml: format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <xdr:wsDr xmlns:xdr=\"{XDR_NS}\" xmlns:a=\"{A_NS}\" xmlns:r=\"{R_NS}\" xmlns:c=\"{C_NS}\" \
             xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" \
             xmlns:a16=\"http://schemas.microsoft.com/office/drawing/2014/main\" \
             xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\"></xdr:wsDr>"
        ),
        rels: Vec::new(),
        media: Vec::new(),
        charts: Vec::new(),
        chart_ex: Vec::new(),
    }
}

/// 在绘图部件末尾插入一个锚点。
pub fn insert_anchor(xml: &str, anchor: &str) -> String {
    xml.replace("</xdr:wsDr>", &format!("{anchor}</xdr:wsDr>"))
}

fn anchor_from(anchor: &str) -> Result<(u32, u32)> {
    let (col, row) = parse_cell_ref(anchor)?;
    Ok((col - 1, row - 1))
}

fn ext_attrs(size: &Size) -> (i64, i64) {
    (
        (size.w * EMU_PER_INCH).round() as i64,
        (size.h * EMU_PER_INCH).round() as i64,
    )
}

/// 锚点单元格的绝对左上角位置（EMU），用于图片/图表的 `<a:off>`。WPS/Office
/// 会据此定位，若为 (0,0) 会把图片画错位置/尺寸。
fn anchor_offset_emu(sheet: &Sheet, col0: u32, row0: u32) -> (i64, i64) {
    let mut x_px = 0.0;
    for c in 0..col0 {
        x_px += col_px(sheet, c);
    }
    let mut y_px = 0.0;
    for r in 0..row0 {
        y_px += row_px(sheet, r);
    }
    (
        (x_px * EMU_PER_PX).round() as i64,
        (y_px * EMU_PER_PX).round() as i64,
    )
}

fn col_px(sheet: &Sheet, c: u32) -> f64 {
    let w = sheet
        .columns
        .get(c as usize)
        .and_then(|col| col.width)
        .filter(|w| *w > 0.0)
        .or(sheet.default_col_width.filter(|w| *w > 0.0))
        .unwrap_or(DEFAULT_COL_CHARS);
    w * 7.0 + 5.0
}

fn row_px(sheet: &Sheet, r: u32) -> f64 {
    let h = sheet
        .rows
        .iter()
        .find(|rr| rr.index == Some(r + 1))
        .and_then(|rr| rr.height)
        .filter(|h| *h > 0.0)
        .or(sheet.default_row_height.filter(|h| *h > 0.0))
        .unwrap_or(DEFAULT_ROW_PT);
    h * 96.0 / 72.0
}

fn locate_x(sheet: &Sheet, abs_emu: i64) -> (u32, i64) {
    let target = abs_emu as f64;
    let (mut acc, mut c) = (0.0, 0u32);
    loop {
        let w = col_px(sheet, c) * EMU_PER_PX;
        if target < acc + w || c > 50_000 {
            return (c, (target - acc).round() as i64);
        }
        acc += w;
        c += 1;
    }
}

fn locate_y(sheet: &Sheet, abs_emu: i64) -> (u32, i64) {
    let target = abs_emu as f64;
    let (mut acc, mut r) = (0.0, 0u32);
    loop {
        let h = row_px(sheet, r) * EMU_PER_PX;
        if target < acc + h || r > 1_000_000 {
            return (r, (target - acc).round() as i64);
        }
        acc += h;
        r += 1;
    }
}

const EMU_PER_PX: f64 = 9525.0;
const DEFAULT_COL_CHARS: f64 = 8.43;
const DEFAULT_ROW_PT: f64 = 15.0;

fn image_anchor(
    cid: u32,
    anchor: &str,
    size: &Size,
    embed: &str,
    sheet: &Sheet,
    offset: Option<[i64; 2]>,
) -> String {
    let (col, row) = anchor_from(anchor).unwrap_or((4, 1));
    let (cx, cy) = ext_attrs(size);
    let (ax, ay) = anchor_offset_emu(sheet, col, row);
    let [dx, dy] = offset.unwrap_or([0, 0]);
    let (ox, oy) = (ax + dx, ay + dy);
    format!(
        "<xdr:oneCellAnchor>\
           <xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>{dx}</xdr:colOff><xdr:row>{row}</xdr:row><xdr:rowOff>{dy}</xdr:rowOff></xdr:from>\
           <xdr:ext cx=\"{cx}\" cy=\"{cy}\"/>\
           <xdr:pic>\
             <xdr:nvPicPr><xdr:cNvPr id=\"{cid}\" name=\"Picture {cid}\"/><xdr:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></xdr:cNvPicPr></xdr:nvPicPr>\
             <xdr:blipFill><a:blip xmlns:r=\"{R_NS}\" r:embed=\"{embed}\"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill>\
             <xdr:spPr><a:xfrm><a:off x=\"{ox}\" y=\"{oy}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></xdr:spPr>\
           </xdr:pic>\
           <xdr:clientData/>\
         </xdr:oneCellAnchor>"
    )
}

fn chart_anchor(
    cid: u32,
    anchor: &str,
    size: &Size,
    rid: &str,
    sheet: &Sheet,
    offset: Option<[i64; 2]>,
    is_ex: bool,
) -> String {
    let uri = if is_ex {
        "http://schemas.microsoft.com/office/drawing/2014/chartex"
    } else {
        C_NS
    };
    let (col, row) = anchor_from(anchor).unwrap_or((4, 1));
    let (cx, cy) = ext_attrs(size);
    let (ax, ay) = anchor_offset_emu(sheet, col, row);
    let [dx, dy] = offset.unwrap_or([0, 0]);
    let (ox, oy) = (ax + dx, ay + dy);
    let (tc, tco) = locate_x(sheet, ox + cx);
    let (tr, tro) = locate_y(sheet, oy + cy);
    // 图表用 twoCellAnchor（与 Excel/LO 原件一致，渲染尺寸最吻合）。
    let inner = if is_ex {
        format!(
            "<cx:chart xmlns:cx=\"http://schemas.microsoft.com/office/drawing/2014/chartex\" xmlns:r=\"{R_NS}\" r:id=\"{rid}\"/>"
        )
    } else {
        format!("<c:chart xmlns:c=\"{C_NS}\" xmlns:r=\"{R_NS}\" r:id=\"{rid}\"/>")
    };
    format!(
        "<xdr:twoCellAnchor>\
           <xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>{dx}</xdr:colOff><xdr:row>{row}</xdr:row><xdr:rowOff>{dy}</xdr:rowOff></xdr:from>\
           <xdr:to><xdr:col>{tc}</xdr:col><xdr:colOff>{tco}</xdr:colOff><xdr:row>{tr}</xdr:row><xdr:rowOff>{tro}</xdr:rowOff></xdr:to>\
           <xdr:graphicFrame>\
             <xdr:nvGraphicFramePr><xdr:cNvPr id=\"{cid}\" name=\"Chart {cid}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr>\
             <xdr:xfrm><a:off x=\"{ox}\" y=\"{oy}\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></xdr:xfrm>\
             <a:graphic><a:graphicData uri=\"{uri}\">{inner}</a:graphicData></a:graphic>\
           </xdr:graphicFrame>\
           <xdr:clientData/>\
         </xdr:twoCellAnchor>"
    )
}

/// 生成结构化形状的锚点 XML（twoCellAnchor + xdr:sp）。
fn shape_anchor(cid: u32, s: &crate::model::sheet::Shape, sheet: &Sheet) -> String {
    let anchor = s.anchor.clone().unwrap_or_else(|| "C3".into());
    let [dx, dy] = s.offset.unwrap_or([0, 0]);
    let from_to = if let Some((a, b)) = anchor.split_once(':') {
        let (fc, fr) = parse_cell_ref(a).unwrap_or((3, 3));
        let (tc, tr) = parse_cell_ref(b).unwrap_or((5, 5));
        format!(
            "<xdr:from><xdr:col>{}</xdr:col><xdr:colOff>{dx}</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>{dy}</xdr:rowOff></xdr:from>\
             <xdr:to><xdr:col>{}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>",
            fc - 1, fr - 1, tc - 1, tr - 1
        )
    } else {
        let (col, row) = anchor_from(&anchor).unwrap_or((2, 2));
        let size = s.size.clone().unwrap_or(Size { w: 2.0, h: 1.5 });
        let (cx, cy) = ext_attrs(&size);
        let (ax, ay) = anchor_offset_emu(sheet, col, row);
        let (ox, oy) = (ax + dx, ay + dy);
        let (tc, tco) = locate_x(sheet, ox + cx);
        let (tr, tro) = locate_y(sheet, oy + cy);
        format!(
            "<xdr:from><xdr:col>{col}</xdr:col><xdr:colOff>{dx}</xdr:colOff><xdr:row>{row}</xdr:row><xdr:rowOff>{dy}</xdr:rowOff></xdr:from>\
             <xdr:to><xdr:col>{tc}</xdr:col><xdr:colOff>{tco}</xdr:colOff><xdr:row>{tr}</xdr:row><xdr:rowOff>{tro}</xdr:rowOff></xdr:to>"
        )
    };
    let geom = s.geometry.clone().unwrap_or_else(|| "rect".into());
    let mut xfrm = String::from("<a:xfrm");
    if let Some(r) = s.rotation {
        xfrm.push_str(&format!(" rot=\"{}\"", (r * 60000.0) as i64));
    }
    if s.flip_h {
        xfrm.push_str(" flipH=\"1\"");
    }
    if s.flip_v {
        xfrm.push_str(" flipV=\"1\"");
    }
    xfrm.push_str("><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></a:xfrm>");

    let fill = if let Some(g) = &s.gradient {
        gradient_fill(g)
    } else {
        let color = s.fill.clone().unwrap_or_else(|| "4472C4".into());
        format!(
            "<a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>",
            crate::utils::xml::normalize_color(&color)
        )
    };
    let line = match (&s.line_color, s.line_width) {
        (None, None) => String::new(),
        (c, w) => {
            let col = c.clone().unwrap_or_else(|| "000000".into());
            let wid = (w.unwrap_or(1.0) * 12700.0) as i64; // pt -> EMU
            format!(
                "<a:ln w=\"{wid}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:ln>",
                crate::utils::xml::normalize_color(&col)
            )
        }
    };
    let tx = match &s.text {
        Some(t) if !t.is_empty() => format!(
            "<xdr:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" dirty=\"0\"/><a:t>{}</a:t></a:r></a:p></xdr:txBody>",
            esc_xml(t)
        ),
        _ => String::new(),
    };
    format!(
        "<xdr:twoCellAnchor>{from_to}\
         <xdr:sp macro=\"\" textlink=\"\">\
           <xdr:nvSpPr><xdr:cNvPr id=\"{cid}\" name=\"Shape {cid}\"/><xdr:cNvSpPr/></xdr:nvSpPr>\
           <xdr:spPr>{xfrm}<a:prstGeom prst=\"{geom}\"><a:avLst/></a:prstGeom>{fill}{line}</xdr:spPr>\
           {tx}\
         </xdr:sp>\
         <xdr:clientData/></xdr:twoCellAnchor>"
    )
}

fn gradient_fill(spec: &str) -> String {
    let (colors, angle) = match spec.split_once(':') {
        Some((c, a)) => (c, a.parse::<i64>().unwrap_or(90)),
        None => (spec, 90),
    };
    let stops: Vec<&str> = colors.split('-').collect();
    let mut gs = String::new();
    for (i, c) in stops.iter().enumerate() {
        let pos = if stops.len() <= 1 {
            0
        } else {
            (i * 100000) / (stops.len() - 1)
        };
        gs.push_str(&format!(
            "<a:gs pos=\"{pos}\"><a:srgbClr val=\"{}\"/></a:gs>",
            crate::utils::xml::normalize_color(c)
        ));
    }
    format!(
        "<a:gradFill><a:gsLst>{gs}</a:gsLst><a:lin ang=\"{}\" scaled=\"0\"/></a:gradFill>",
        angle * 60000
    )
}

/// Absolute range formula, e.g. `Sheet1!$B$2:$B$8`.
pub fn range_formula(sheet: &str, range: &str) -> Result<String> {
    let ((c1, r1), (c2, r2)) = parse_range(range)?;
    Ok(format!(
        "{}!${}${}:${}${}",
        sheet,
        index_to_col(c1),
        r1,
        index_to_col(c2),
        r2
    ))
}

fn is_chartex(t: &str) -> bool {
    matches!(
        t,
        "waterfall"
            | "funnel"
            | "treemap"
            | "sunburst"
            | "histogram"
            | "pareto"
            | "boxwhisker"
            | "box"
            | "boxplot"
    )
}

fn chart_xml(sheet: &str, chart: &Chart) -> Result<(String, bool)> {
    // 结构化字段未改动时，优先原样回写原始图表部件以保留完整样式。
    if let Some(raw) = chart.raw.0.as_deref() {
        if chart.snapshot.0.as_deref() == Some(chart.structured_json().as_str()) {
            let ex = raw.contains("chartex") || raw.contains("cx:chartSpace");
            return Ok((raw.to_string(), ex));
        }
    }
    if is_chartex(&chart.chart_type) {
        return Ok((chart_ex_xml(sheet, chart)?, true));
    }
    let data_range = chart
        .data_range
        .as_deref()
        .ok_or_else(|| Error::InvalidInput("图表缺少 data_range".into()))?;
    let ((c1, r1), (c2, r2)) = parse_range(data_range)?;
    let cat = chart
        .categories
        .as_deref()
        .map(|r| range_formula(sheet, r))
        .transpose()?;

    let title = match &chart.title {
        Some(t) => format!(
            "<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" dirty=\"0\"/><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val=\"0\"/></c:title>",
            esc_xml(t)
        ),
        None => String::new(),
    };

    let cat_inner = cat
        .as_ref()
        .map(|c| format!("<c:strRef><c:f>{}</c:f></c:strRef>", esc_xml(c)));
    let cat_ref = |tag: &str| match &cat_inner {
        Some(inner) => format!("<c:{tag}>{inner}</c:{tag}>"),
        None => String::new(),
    };

    // One series per data column.
    let mut sers = String::new();
    let mut sers_scatter = String::new();
    let mut sers_bubble = String::new();
    for (i, col) in (c1..=c2).enumerate() {
        let col_range = format!(
            "{}!${}${}:${}${}",
            sheet,
            index_to_col(col),
            r1,
            index_to_col(col),
            r2
        );
        let name = if r1 > 1 {
            format!(
                "<c:tx><c:strRef><c:f>{}!${}${}</c:f></c:strRef></c:tx>",
                sheet,
                index_to_col(col),
                r1 - 1
            )
        } else {
            String::new()
        };
        let val = format!(
            "<c:val><c:numRef><c:f>{}</c:f></c:numRef></c:val>",
            esc_xml(&col_range)
        );
        let sppr = match chart.colors.get(i) {
            Some(c) => format!(
                "<c:spPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></c:spPr>",
                crate::utils::xml::normalize_color(c)
            ),
            None => String::new(),
        };
        sers.push_str(&format!(
            "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>{name}{sppr}{}{val}</c:ser>",
            cat_ref("cat")
        ));

        let x = match &cat {
            Some(c) => format!(
                "<c:xVal><c:numRef><c:f>{}</c:f></c:numRef></c:xVal>",
                esc_xml(c)
            ),
            None => String::new(),
        };
        let y = format!(
            "<c:yVal><c:numRef><c:f>{}</c:f></c:numRef></c:yVal>",
            esc_xml(&col_range)
        );
        sers_scatter.push_str(&format!(
            "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>{name}{sppr}{x}{y}</c:ser>"
        ));

        // bubbleChart：xVal=类别，yVal/气泡大小=当列。
        let bx = match &cat {
            Some(c) => format!(
                "<c:xVal><c:numRef><c:f>{}</c:f></c:numRef></c:xVal>",
                esc_xml(c)
            ),
            None => String::new(),
        };
        sers_bubble.push_str(&format!(
            "<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>{name}{sppr}{bx}{y}<c:bubbleSize><c:numRef><c:f>{}</c:f></c:numRef></c:bubbleSize></c:ser>",
            esc_xml(&col_range)
        ));
    }

    let ax1 = "111111111";
    let ax2 = "222222222";
    let cat_title = chart
        .category_axis_title
        .as_deref()
        .or_else(|| {
            chart
                .category_axis
                .as_ref()
                .and_then(|a| a.title.as_deref())
        })
        .map(axis_title_xml)
        .unwrap_or_default();
    let val_title = chart
        .value_axis_title
        .as_deref()
        .or_else(|| chart.value_axis.as_ref().and_then(|a| a.title.as_deref()))
        .map(axis_title_xml)
        .unwrap_or_default();
    let cat_ax = format!(
        "<c:catAx><c:axId val=\"{ax1}\"/>{}{}<c:delete val=\"{}\"/><c:axPos val=\"b\"/>{cat_title}<c:crossAx val=\"{ax2}\"/></c:catAx>",
        axis_scaling(chart.category_axis.as_ref()),
        axis_numfmt(chart.category_axis.as_ref()),
        axis_delete(chart.category_axis.as_ref()),
    );
    let val_ax = format!(
        "<c:valAx><c:axId val=\"{ax2}\"/>{}{}{}<c:delete val=\"{}\"/><c:axPos val=\"l\"/>{val_title}<c:crossAx val=\"{ax1}\"/></c:valAx>",
        axis_scaling(chart.value_axis.as_ref()),
        axis_numfmt(chart.value_axis.as_ref()),
        axis_gridlines(chart.value_axis.as_ref()),
        axis_delete(chart.value_axis.as_ref()),
    );
    let dlabels = if chart.show_values {
        "<c:dLbls><c:showVal val=\"1\"/></c:dLbls>"
    } else {
        ""
    };
    let legend = match chart.legend.as_deref() {
        None | Some("none") => String::new(),
        Some(p) => {
            let pos = match p {
                "left" => "l",
                "top" => "t",
                "bottom" => "b",
                _ => "r",
            };
            format!("<c:legend><c:legendPos val=\"{pos}\"/></c:legend>")
        }
    };

    let plot = if !chart.series.is_empty() {
        series_plot(sheet, chart, c1, c2, r1, &cat, &cat_ax, &val_ax, ax1, ax2)
    } else {
        match chart.chart_type.as_str() {
        "column" | "bar" | "col" => {
            let dir = if chart.chart_type == "bar" { "bar" } else { "col" };
            format!(
                "<c:barChart><c:barDir val=\"{dir}\"/><c:grouping val=\"clustered\"/>{sers}{dlabels}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:barChart>{cat_ax}{val_ax}"
            )
        }
        "column3d" | "bar3d" | "3dbar" | "3dcolumn" => {
            let dir = if chart.chart_type.contains("bar") { "bar" } else { "col" };
            format!(
                "<c:bar3DChart><c:barDir val=\"{dir}\"/><c:grouping val=\"clustered\"/><c:shape val=\"box\"/>{sers}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/><c:axId val=\"333333333\"/></c:bar3DChart>{cat_ax}{val_ax}<c:serAx><c:axId val=\"333333333\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"{ax1}\"/></c:serAx>"
            )
        }
        "line" => format!(
            "<c:lineChart><c:grouping val=\"standard\"/>{sers}{dlabels}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:lineChart>{cat_ax}{val_ax}"
        ),
        "line3d" => format!(
            "<c:line3DChart><c:grouping val=\"standard\"/>{sers}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/><c:axId val=\"333333333\"/></c:line3DChart>{cat_ax}{val_ax}<c:serAx><c:axId val=\"333333333\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"{ax1}\"/></c:serAx>"
        ),
        "area" => format!(
            "<c:areaChart><c:grouping val=\"standard\"/>{sers}{dlabels}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:areaChart>{cat_ax}{val_ax}"
        ),
        "area3d" => format!(
            "<c:area3DChart><c:grouping val=\"standard\"/>{sers}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/><c:axId val=\"333333333\"/></c:area3DChart>{cat_ax}{val_ax}<c:serAx><c:axId val=\"333333333\"/><c:scaling><c:orientation val=\"minMax\"/></c:scaling><c:delete val=\"0\"/><c:axPos val=\"b\"/><c:crossAx val=\"{ax1}\"/></c:serAx>"
        ),
        "pie" => format!(
            "<c:pieChart><c:varyColors val=\"1\"/>{sers}{dlabels}</c:pieChart>"
        ),
        "pie3d" => format!(
            "<c:pie3DChart><c:varyColors val=\"1\"/>{sers}{dlabels}</c:pie3DChart>"
        ),
        "ring" | "doughnut" => format!(
            "<c:doughnutChart><c:varyColors val=\"1\"/>{sers}{dlabels}<c:holeSize val=\"50\"/></c:doughnutChart>"
        ),
        "radar" | "spider" => format!(
            "<c:radarChart><c:radarStyle val=\"standard\"/><c:varyColors val=\"1\"/>{sers}{dlabels}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:radarChart>{cat_ax}{val_ax}"
        ),
        "scatter" | "xy" => format!(
            "<c:scatterChart><c:scatterStyle val=\"lineMarker\"/>{sers_scatter}{dlabels}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:scatterChart>{val_ax}{val_ax}"
        ),
        "bubble" => format!(
            "<c:bubbleChart><c:varyColors val=\"1\"/>{sers_bubble}{dlabels}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:bubbleChart>{val_ax}{val_ax}"
        ),
        "stock" | "ohlc" => format!(
            "<c:stockChart>{sers}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:stockChart>{cat_ax}{val_ax}"
        ),
        "combo" => format!(
            "<c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/>{sers}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:barChart>\
             <c:lineChart><c:grouping val=\"standard\"/>{sers}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:lineChart>{cat_ax}{val_ax}"
        ),
        other => {
            return Err(Error::InvalidInput(format!(
                "不支持的图表类型: {other}（可用: column/bar/line/area/pie/doughnut/ring/radar/scatter/bubble/stock/combo、3D 变体，以及扩展图表 waterfall/funnel/treemap/sunburst/histogram/pareto/boxWhisker）"
            )))
        }
        }
    };

    Ok((
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <c:chartSpace xmlns:c=\"{C_NS}\" xmlns:a=\"{A_NS}\" xmlns:r=\"{R_NS}\">\
               <c:chart>{title}<c:plotArea><c:layout/>{plot}</c:plotArea>{legend}<c:plotVisOnly val=\"1\"/></c:chart>\
             </c:chartSpace>"
        ),
        false,
    ))
}

/// 扩展图表（chartEx）：waterfall/funnel/treemap/sunburst/histogram/pareto/boxWhisker。
fn chart_ex_xml(sheet: &str, chart: &Chart) -> Result<String> {
    let data_range = chart
        .data_range
        .as_deref()
        .ok_or_else(|| Error::InvalidInput("图表缺少 data_range".into()))?;
    let ((_, r1), (_, r2)) = parse_range(data_range)?;
    let val_f = range_formula(sheet, data_range)?;
    let cat_f = chart
        .categories
        .as_deref()
        .map(|r| range_formula(sheet, r))
        .transpose()?;
    let n = (r2 as i64 - r1 as i64 + 1).max(0);
    let layout = match chart.chart_type.as_str() {
        "waterfall" => "waterfall",
        "funnel" => "funnel",
        "treemap" => "treemap",
        "sunburst" => "sunburst",
        "histogram" => "clusteredColumn",
        "pareto" => "paretoLine",
        "boxwhisker" | "box" | "boxplot" => "boxWhisker",
        _ => "clusteredColumn",
    };
    let title = match &chart.title {
        Some(t) => format!(
            "<cx:title><cx:tx><cx:rich><a:bodyPr/><a:p><a:r><a:t>{}</a:t></a:r></a:p></cx:rich></cx:tx></cx:title>",
            esc_xml(t)
        ),
        None => String::new(),
    };
    let cat_dim = match &cat_f {
        Some(f) => format!(
            "<cx:strDim type=\"cat\"><cx:f>{}</cx:f><cx:lvl ptCount=\"{n}\"/></cx:strDim>",
            esc_xml(f)
        ),
        None => String::new(),
    };
    // 笛卡尔类需要坐标轴。
    let cartesian = matches!(
        chart.chart_type.as_str(),
        "waterfall" | "histogram" | "pareto" | "boxwhisker" | "box" | "boxplot"
    );
    let axes = if cartesian {
        "<cx:axis id=\"0\"><cx:catScaling gapWidth=\"1\"/><cx:delete val=\"0\"/><cx:majorTickMark val=\"out\"/><cx:tickLblPos val=\"nextTo\"/><cx:crossing val=\"1\"/><cx:crossesAt val=\"1\"/></cx:axis>\
         <cx:axis id=\"1\"><cx:valScaling/><cx:delete val=\"0\"/><cx:majorTickMark val=\"out\"/><cx:tickLblPos val=\"nextTo\"/><cx:crossing val=\"0\"/><cx:crossesAt val=\"0\"/></cx:axis>"
    } else {
        ""
    };
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <cx:chartSpace xmlns:cx=\"http://schemas.microsoft.com/office/drawing/2014/chartex\" \
         xmlns:r=\"{R_NS}\" xmlns:a=\"{A_NS}\" xmlns:c=\"{C_NS}\">\
           <cx:chartData><cx:data id=\"0\">{cat_dim}\
             <cx:numDim type=\"val\"><cx:f>{}</cx:f><cx:lvl ptCount=\"{n}\"/></cx:numDim>\
           </cx:data></cx:chartData>\
           <cx:chart>{title}<cx:plotArea><cx:plotAreaRegion>\
             <cx:series layoutId=\"{layout}\"><cx:dataId val=\"0\"/></cx:series>\
           </cx:plotAreaRegion>{axes}</cx:plotArea></cx:chart>\
         </cx:chartSpace>",
        esc_xml(&val_f)
    ))
}

fn axis_title_xml(text: &str) -> String {
    format!(
        "<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" dirty=\"0\"/><a:t>{}</a:t></a:r></a:p></c:rich></c:tx><c:overlay val=\"0\"/></c:title>",
        esc_xml(text)
    )
}

/// 按系列级设置构建绘图区（支持组合图：每系列 bar/line/area、颜色、标记）。
#[allow(clippy::too_many_arguments)]
fn series_plot(
    sheet: &str,
    chart: &Chart,
    c1: u32,
    c2: u32,
    r1: u32,
    cat: &Option<String>,
    cat_ax: &str,
    val_ax: &str,
    ax1: &str,
    ax2: &str,
) -> String {
    let dlabels = if chart.show_values {
        "<c:dLbls><c:showVal val=\"1\"/></c:dLbls>"
    } else {
        ""
    };
    let mut bars = String::new();
    let mut lines = String::new();
    let mut areas = String::new();
    for i in 0..(c2 - c1 + 1) as usize {
        let col = c1 + i as u32;
        let spec = chart.series.get(i);
        let t = spec
            .and_then(|s| s.kind.clone())
            .unwrap_or_else(|| chart.chart_type.clone())
            .to_ascii_lowercase();
        let r2 = chart_range_r2(chart);
        let range = format!(
            "{}!${}${}:${}${}",
            sheet,
            index_to_col(col),
            r1,
            index_to_col(col),
            r2
        );
        let name = match spec.and_then(|s| s.name.clone()) {
            Some(n) => format!("<c:tx><c:v>{}</c:v></c:tx>", esc_xml(&n)),
            None => {
                if r1 > 1 {
                    format!(
                        "<c:tx><c:strRef><c:f>{}!${}${}</c:f></c:strRef></c:tx>",
                        sheet,
                        index_to_col(col),
                        r1 - 1
                    )
                } else {
                    String::new()
                }
            }
        };
        let color = spec
            .and_then(|s| s.color.clone())
            .or_else(|| chart.colors.get(i).cloned());
        let sppr = match color {
            Some(c) => format!(
                "<c:spPr><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></c:spPr>",
                crate::utils::xml::normalize_color(&c)
            ),
            None => String::new(),
        };
        let marker = match spec.and_then(|s| s.marker.clone()) {
            Some(m) => format!("<c:marker><c:symbol val=\"{}\"/></c:marker>", esc_xml(&m)),
            None => String::new(),
        };
        let catx = match cat {
            Some(c) => format!(
                "<c:cat><c:strRef><c:f>{}</c:f></c:strRef></c:cat>",
                esc_xml(c)
            ),
            None => String::new(),
        };
        let val = format!(
            "<c:val><c:numRef><c:f>{}</c:f></c:numRef></c:val>",
            esc_xml(&range)
        );
        let ser =
            format!("<c:ser><c:idx val=\"{i}\"/><c:order val=\"{i}\"/>{name}{sppr}{marker}{catx}{val}</c:ser>");
        match t.as_str() {
            "line" => lines.push_str(&ser),
            "area" => areas.push_str(&ser),
            _ => bars.push_str(&ser),
        }
    }
    let bar_dir = if chart.chart_type == "bar" {
        "bar"
    } else {
        "col"
    };
    let mut plot = String::new();
    if !bars.is_empty() {
        plot.push_str(&format!(
            "<c:barChart><c:barDir val=\"{bar_dir}\"/><c:grouping val=\"clustered\"/>{bars}{dlabels}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:barChart>"
        ));
    }
    if !lines.is_empty() {
        plot.push_str(&format!(
            "<c:lineChart><c:grouping val=\"standard\"/>{lines}{dlabels}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:lineChart>"
        ));
    }
    if !areas.is_empty() {
        plot.push_str(&format!(
            "<c:areaChart><c:grouping val=\"standard\"/>{areas}{dlabels}<c:axId val=\"{ax1}\"/><c:axId val=\"{ax2}\"/></c:areaChart>"
        ));
    }
    plot.push_str(cat_ax);
    plot.push_str(val_ax);
    plot
}

fn chart_range_r2(chart: &Chart) -> u32 {
    chart
        .data_range
        .as_deref()
        .and_then(|r| parse_range(r).ok())
        .map(|(_, (_, r2))| r2)
        .unwrap_or(1)
}

fn axis_scaling(a: Option<&crate::model::sheet::ChartAxis>) -> String {
    let mut s = String::from("<c:scaling><c:orientation val=\"minMax\"/>");
    if let Some(ax) = a {
        if let Some(min) = ax.min {
            s.push_str(&format!("<c:min val=\"{min}\"/>"));
        }
        if let Some(max) = ax.max {
            s.push_str(&format!("<c:max val=\"{max}\"/>"));
        }
    }
    s.push_str("</c:scaling>");
    s
}

fn axis_delete(a: Option<&crate::model::sheet::ChartAxis>) -> &'static str {
    if a.and_then(|x| x.visible) == Some(false) {
        "1"
    } else {
        "0"
    }
}

fn axis_gridlines(a: Option<&crate::model::sheet::ChartAxis>) -> String {
    if a.and_then(|x| x.gridlines) == Some(false) {
        String::new()
    } else {
        "<c:majorGridlines/>".to_string()
    }
}

fn axis_numfmt(a: Option<&crate::model::sheet::ChartAxis>) -> String {
    match a.and_then(|x| x.number_format.as_deref()) {
        Some(f) => format!(
            "<c:numFmt formatCode=\"{}\" sourceLinked=\"0\"/>",
            esc_xml(f)
        ),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::sheet::RawXml;

    const CHART: &str = "<?xml version=\"1.0\"?><c:chartSpace xmlns:c=\"c\" xmlns:a=\"a\"><c:chart><c:plotArea><c:barChart><c:ser><c:cat><c:strRef><c:f>Data!$A$2:$A$4</c:f></c:strRef></c:cat><c:val><c:numRef><c:f>Data!$B$2:$B$4</c:f></c:numRef></c:val></c:ser></c:barChart></c:plotArea></c:chart></c:chartSpace>";

    #[test]
    fn raw_chart_used_until_edited() {
        let mut c = crate::parse::drawing::parse_chart(CHART).unwrap();
        c.anchor = Some("E2".into());
        c.snapshot = RawXml::from(&c.structured_json());

        assert_eq!(chart_xml("Data", &c).unwrap().0, CHART);

        // 改动结构化字段：应重新生成，不再使用 raw
        c.data_range = Some("B2:B9".into());
        let regenerated = chart_xml("Data", &c).unwrap().0;
        assert_ne!(regenerated, CHART);
        assert!(regenerated.contains("$B$9"));
    }
}
