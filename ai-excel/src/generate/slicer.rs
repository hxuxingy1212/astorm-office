//! Slicer creation (pivot-backed): slicerCache + slicers parts + drawing anchor.
//!
//! 结构与 OfficeCLI/Excel 参考一致：
//! - xl/slicerCaches/slicerCacheN.xml（slicerCacheDefinition）
//! - xl/slicers/slicerN.xml（slicers）
//! - workbook extLst 注册 slicerCache（{BBE1A952-...}）
//! - worksheet extLst 注册 slicerList（{A8765BA9-...}）
//! - 绘图 twoCellAnchor + mc:AlternateContent(sle:slicer)

use crate::utils::a1::{parse_cell_ref, parse_range};
use crate::utils::xml::esc_xml;

pub struct SlicerBuilt {
    pub cache_name: String,
    pub slicers_name: String,
    pub cache_xml: String,
    pub slicers_xml: String,
    pub slicer_name: String,
    pub cache_ref_name: String, // slicerCacheDefinition name (== cacheName)
    pub tab_id: u32,
}

#[allow(clippy::too_many_arguments)]
pub fn build_slicer(
    sheet_name: &str,
    sheet_id: u32,
    pivot_name: &str,
    pivot_cache_id: u32,
    field: &str,
    items: &[String],
    slicer: &crate::model::sheet::Slicer,
    no: usize,
) -> SlicerBuilt {
    let slicer_name = slicer
        .name
        .clone()
        .unwrap_or_else(|| format!("Slicer_{field}"));
    let cache_name = slicer_name.clone();
    let caption = slicer.caption.clone().unwrap_or_else(|| field.to_string());
    let row_height = slicer.row_height.unwrap_or(225425);
    let _ = sheet_name;

    let mut cache = String::new();
    cache.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    cache.push_str(&format!(
        "<x14:slicerCacheDefinition xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
         xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" \
         xmlns:x14=\"http://schemas.microsoft.com/office/spreadsheetml/2009/9/main\" \
         xmlns:x=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" mc:Ignorable=\"x\" \
         name=\"{}\" sourceName=\"{}\">",
        esc_xml(&cache_name),
        esc_xml(field)
    ));
    cache.push_str(&format!(
        "<x14:pivotTables><x14:pivotTable tabId=\"{sheet_id}\" name=\"{}\"/></x14:pivotTables>",
        esc_xml(pivot_name)
    ));
    cache.push_str(&format!(
        "<x14:slicerCacheData><x14:tabularSlicerCache pivotCacheId=\"{pivot_cache_id}\" sortOrder=\"ascending\">"
    ));
    cache.push_str(&format!("<x14:items count=\"{}\">", items.len()));
    for (i, _) in items.iter().enumerate() {
        cache.push_str(&format!("<x14:i x=\"{i}\" s=\"1\"/>"));
    }
    cache.push_str(
        "</x14:items></x14:tabularSlicerCache></x14:slicerCacheData></x14:slicerCacheDefinition>",
    );

    let mut sl = String::new();
    sl.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    sl.push_str(
        "<x14:slicers xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" \
         xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" \
         xmlns:x14=\"http://schemas.microsoft.com/office/spreadsheetml/2009/9/main\" \
         xmlns:x=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" mc:Ignorable=\"x\">",
    );
    sl.push_str(&format!(
        "<x14:slicer name=\"{}\" cache=\"{}\" caption=\"{}\" rowHeight=\"{row_height}\"",
        esc_xml(&slicer_name),
        esc_xml(&cache_name),
        esc_xml(&caption)
    ));
    if let Some(cc) = slicer.column_count {
        sl.push_str(&format!(" columnCount=\"{cc}\""));
    }
    if let Some(st) = &slicer.style {
        sl.push_str(&format!(" style=\"{}\"", esc_xml(st)));
    }
    sl.push_str("/></x14:slicers>");

    SlicerBuilt {
        cache_name: format!("slicerCache{no}.xml"),
        slicers_name: format!("slicer{no}.xml"),
        cache_xml: cache,
        slicers_xml: sl,
        slicer_name,
        cache_ref_name: cache_name,
        tab_id: sheet_id,
    }
}

/// 绘图锚点（twoCellAnchor + mc:AlternateContent）。`next_id` 为图形帧 id。
pub fn drawing_anchor(slicer_name: &str, anchor: &str, next_id: u32) -> String {
    let (fc, fr, tc, tr) = parse_anchor(anchor);
    let fallback_id = next_id + 1;
    format!(
        "<xdr:twoCellAnchor editAs=\"oneCell\">\
         <xdr:from><xdr:col>{fc}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{fr}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>\
         <xdr:to><xdr:col>{tc}</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>{tr}</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>\
         <mc:AlternateContent>\
           <mc:Choice Requires=\"a14\" xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\">\
             <xdr:graphicFrame macro=\"\">\
               <xdr:nvGraphicFramePr><xdr:cNvPr id=\"{next_id}\" name=\"{n}\"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr>\
               <xdr:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/></xdr:xfrm>\
               <a:graphic><a:graphicData uri=\"http://schemas.microsoft.com/office/drawing/2010/slicer\">\
                 <sle:slicer xmlns:sle=\"http://schemas.microsoft.com/office/drawing/2010/slicer\" name=\"{n}\"/>\
               </a:graphicData></a:graphic>\
             </xdr:graphicFrame>\
           </mc:Choice>\
           <mc:Fallback>\
             <xdr:sp macro=\"\" textlink=\"\">\
               <xdr:nvSpPr><xdr:cNvPr id=\"{fallback_id}\" name=\"{n}\"/><xdr:cNvSpPr><a:spLocks noTextEdit=\"1\"/></xdr:cNvSpPr></xdr:nvSpPr>\
               <xdr:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"1828800\" cy=\"2381250\"/></a:xfrm>\
                 <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>\
                 <a:solidFill><a:prstClr val=\"white\"/></a:solidFill>\
                 <a:ln w=\"1\"><a:solidFill><a:prstClr val=\"gray\"/></a:solidFill></a:ln>\
               </xdr:spPr>\
               <xdr:txBody><a:bodyPr vertOverflow=\"clip\" horzOverflow=\"clip\"/><a:lstStyle/>\
                 <a:p><a:r><a:rPr lang=\"en-US\" sz=\"1100\"/><a:t>Slicer (requires Excel 2010 or later)</a:t></a:r></a:p></xdr:txBody>\
             </xdr:sp>\
           </mc:Fallback>\
         </mc:AlternateContent>\
         <xdr:clientData/>\
         </xdr:twoCellAnchor>",
        n = esc_xml(slicer_name)
    )
}

fn parse_anchor(anchor: &str) -> (u32, u32, u32, u32) {
    if let Some((a, b)) = anchor.split_once(':') {
        if let (Ok((fc, fr)), Ok((tc, tr))) = (parse_cell_ref(a), parse_cell_ref(b)) {
            return (fc - 1, fr - 1, tc - 1, tr - 1);
        }
    }
    if let Ok((c, r)) = parse_cell_ref(anchor) {
        return (c - 1, r - 1, c + 2, r + 9);
    }
    (5, 1, 8, 11)
}

/// 从源区域某列收集去重值（用于切片器项）。
pub fn distinct_items(
    wb: &crate::model::workbook::Workbook,
    sheet: &crate::model::sheet::Sheet,
    source: &str,
    field: &str,
) -> Vec<String> {
    let (sname, range) = match source.rsplit_once('!') {
        Some((s, r)) => (
            s.trim_matches(|c| c == '\'').to_string(),
            r.replace('$', ""),
        ),
        None => (sheet.name.clone(), source.replace('$', "")),
    };
    let src = wb
        .sheets
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(&sname));
    let ((c1, r1), (c2, _r2)) = match parse_range(&range) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    // 找出 field 对应列
    let mut field_col = None;
    if let Some(s) = src {
        if let Some(hr) = s.rows.iter().find(|r| r.index == Some(r1)) {
            for cell in &hr.cells {
                if let Some((cc, _)) = cell
                    .reference
                    .as_deref()
                    .and_then(|x| parse_cell_ref(x).ok())
                {
                    let txt = match &cell.value {
                        Some(serde_json::Value::String(v)) => v.clone(),
                        Some(v) => v.to_string(),
                        None => String::new(),
                    };
                    if txt.eq_ignore_ascii_case(field) {
                        field_col = Some(cc);
                        break;
                    }
                }
            }
        }
    }
    let Some(fc) = field_col else {
        return Vec::new();
    };
    let _ = c1;
    let _ = c2;
    let mut out: Vec<String> = Vec::new();
    if let Some(s) = src {
        for row in &s.rows {
            if row.index.unwrap_or(0) <= r1 {
                continue;
            }
            for cell in &row.cells {
                if let Some((cc, _)) = cell
                    .reference
                    .as_deref()
                    .and_then(|x| parse_cell_ref(x).ok())
                {
                    if cc == fc {
                        let txt = match &cell.value {
                            Some(serde_json::Value::String(v)) => v.clone(),
                            Some(v) => v.to_string(),
                            None => continue,
                        };
                        if !txt.is_empty() && !out.contains(&txt) {
                            out.push(txt);
                        }
                    }
                }
            }
        }
    }
    out
}
