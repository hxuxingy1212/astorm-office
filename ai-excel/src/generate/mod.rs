pub mod comments;
pub mod drawings;
pub mod pivot;
pub mod shared_strings;
pub mod sheet;
pub mod slicer;
pub mod styles;
pub mod tables;

use std::io::Write;
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::{Error, Result};
use crate::model::Workbook;
use crate::utils::constants::*;
use crate::utils::xml::esc_xml;

use drawings::DrawingParts;
use shared_strings::SharedStrings;
use styles::StyleTable;

#[derive(Debug, Clone)]
pub struct GenerateResult {
    pub path: String,
    pub sheets: usize,
}

/// Generate an `.xlsx` file from a [`Workbook`].
pub fn generate(wb: &Workbook, output_path: &Path) -> Result<GenerateResult> {
    if wb.sheets.is_empty() {
        return Err(Error::InvalidInput("工作簿至少需要一个工作表".into()));
    }
    let mut wb = wb.normalized();
    // 为无缓存值的公式计算并写入结果（供非重算阅读器使用）。
    crate::eval::evaluate(&mut wb);
    let styles = StyleTable::build(&wb);

    // Build drawings (images + charts) first; this interns media/chart parts.
    let mut media_counter = 0usize;
    let mut chart_counter = 0usize;
    let mut image_cache: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    let mut drawing_parts: Vec<(usize, usize, DrawingParts)> = Vec::new();
    let n = wb.sheets.len();
    let mut drawing_no: Vec<Option<usize>> = vec![None; n];
    for (i, sheet) in wb.sheets.iter().enumerate() {
        let mut d = drawings::build_sheet_drawing(
            sheet,
            &mut media_counter,
            &mut chart_counter,
            &mut image_cache,
        )?;
        if !sheet.slicers.is_empty() {
            let mut parts = d.take().unwrap_or_else(drawings::empty_drawing);
            for (k, s) in sheet.slicers.iter().enumerate() {
                let name = s
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("Slicer_{}", s.field));
                let anchor = s.anchor.clone().unwrap_or_else(|| "F2".into());
                let a = slicer::drawing_anchor(&name, &anchor, 3000 + k as u32);
                parts.xml = drawings::insert_anchor(&parts.xml, &a);
            }
            d = Some(parts);
        }
        if let Some(part) = d {
            let no = drawing_parts.len() + 1;
            drawing_no[i] = Some(no);
            drawing_parts.push((i, no, part));
        }
    }

    // Pivot tables (cache + definition parts).
    let mut pivot_builds: Vec<(usize, pivot::PivotBuilt)> = Vec::new();
    let mut pivot_cache_meta: Vec<(u32, String)> = Vec::new(); // (cacheId, cache_name)
                                                               // 每张表第一个透视表的信息（供切片器绑定）。
    let mut pivot_info: Vec<Option<(String, u32, String)>> = vec![None; n]; // (name, pivotCacheId, source)
    {
        let mut pivot_no = 0usize;
        for (i, sheet) in wb.sheets.iter().enumerate() {
            for p in &sheet.pivot_tables {
                pivot_no += 1;
                let pci = 1_000_000 + pivot_no as u32;
                let built = pivot::build_pivot(&wb, sheet, p, pivot_no, pivot_no as u32, pci)?;
                if pivot_info[i].is_none() {
                    let nm = built
                        .table_name
                        .trim_start_matches("pivotTable")
                        .trim_end_matches(".xml")
                        .to_string();
                    let name = p.name.clone().unwrap_or_else(|| format!("Pivot{nm}"));
                    pivot_info[i] = Some((name, pci, p.source.clone()));
                }
                pivot_cache_meta.push((built.cache_id, built.cache_name.clone()));
                pivot_builds.push((i, built));
            }
        }
    }

    // Slicers (pivot-backed).
    let mut slicer_builds: Vec<(usize, slicer::SlicerBuilt)> = Vec::new();
    {
        let mut slicer_no = 0usize;
        for (i, sheet) in wb.sheets.iter().enumerate() {
            for s in &sheet.slicers {
                let Some((pname, pci, source)) = pivot_info[i].clone() else {
                    continue;
                };
                slicer_no += 1;
                let items = slicer::distinct_items(&wb, sheet, &source, &s.field);
                let built = slicer::build_slicer(
                    &sheet.name,
                    (i + 1) as u32,
                    &pname,
                    pci,
                    &s.field,
                    &items,
                    s,
                    slicer_no,
                );
                slicer_builds.push((i, built));
            }
        }
    }

    // Sheet relationships + hyperlink plans.
    // entry: (rId, rel type, target)
    let mut sheet_rel_entries: Vec<Vec<(String, String, String)>> = vec![Vec::new(); n];
    let mut hyperlink_plans: Vec<Vec<sheet::Hyperlink>> = (0..n).map(|_| Vec::new()).collect();
    let mut table_counter = 0usize;
    let mut table_parts: Vec<(String, String)> = Vec::new();
    let mut table_rids: Vec<Vec<String>> = (0..n).map(|_| Vec::new()).collect();
    let mut comments_parts: Vec<comments::CommentsPart> = Vec::new();
    let mut vml_parts: Vec<(String, String)> = Vec::new();
    let mut legacy_rids: Vec<Option<String>> = (0..n).map(|_| None).collect();
    let mut pivot_rids: Vec<Vec<String>> = (0..n).map(|_| Vec::new()).collect();
    let mut slicer_rids: Vec<Vec<String>> = (0..n).map(|_| Vec::new()).collect();
    let mut bg_rids: Vec<Option<String>> = (0..n).map(|_| None).collect();
    let mut bg_media: Vec<(String, String)> = Vec::new(); // (zip name, src path)
    let mut bg_no = 0usize;
    // OLE 嵌入：(sheet_i, oleObject rId, shapeId, progId) + 部件
    let mut ole_sheet: Vec<Vec<(String, u32, String)>> = (0..n).map(|_| Vec::new()).collect();
    let mut ole_vml_rids: Vec<Option<String>> = (0..n).map(|_| None).collect();
    let mut ole_parts: Vec<(String, String, String, String)> = Vec::new(); // (bin, vml, src, vml_xml)
    let mut ole_no = 0usize;
    for (i, sheet) in wb.sheets.iter().enumerate() {
        let mut next = 1usize;
        if let Some(no) = drawing_no[i] {
            sheet_rel_entries[i].push((
                format!("rId{next}"),
                REL_DRAWING.to_string(),
                format!("../drawings/drawing{no}.xml"),
            ));
            next += 1;
        }
        for row in &sheet.rows {
            for cell in &row.cells {
                let Some(link) = &cell.link else { continue };
                let Some(reference) = cell.reference.clone() else {
                    continue;
                };
                if let Some(loc) = link.strip_prefix('#') {
                    hyperlink_plans[i].push(sheet::Hyperlink {
                        reference,
                        rid: None,
                        location: Some(loc.to_string()),
                        tooltip: cell.link_tooltip.clone(),
                    });
                } else {
                    let rid = format!("rId{next}");
                    next += 1;
                    sheet_rel_entries[i].push((
                        rid.clone(),
                        REL_HYPERLINK.to_string(),
                        link.clone(),
                    ));
                    hyperlink_plans[i].push(sheet::Hyperlink {
                        reference,
                        rid: Some(rid),
                        location: None,
                        tooltip: cell.link_tooltip.clone(),
                    });
                }
            }
        }
        for table in &sheet.tables {
            table_counter += 1;
            let (fname, txml) = tables::table_xml(sheet, table, table_counter, table_counter)?;
            table_parts.push((fname.clone(), txml));
            let rid = format!("rId{next}");
            next += 1;
            sheet_rel_entries[i].push((
                rid.clone(),
                REL_TABLE.to_string(),
                format!("../tables/{fname}"),
            ));
            table_rids[i].push(rid);
        }
        if let Some(cp) = comments::build_sheet_comments(sheet, comments_parts.len() + 1)? {
            let c_rid = format!("rId{next}");
            next += 1;
            sheet_rel_entries[i].push((
                c_rid,
                REL_COMMENTS.to_string(),
                format!("../{}", cp.comments_name),
            ));
            let v_rid = format!("rId{next}");
            next += 1;
            sheet_rel_entries[i].push((
                v_rid.clone(),
                REL_VMLDRAWING.to_string(),
                format!("../drawings/{}", cp.vml_name),
            ));
            legacy_rids[i] = Some(v_rid);
            comments_parts.push(cp);
        }
        // 保留的表单控件 VML（按钮等）。
        for (k, v) in sheet.vml_drawings.iter().enumerate() {
            let name = format!("vmlDrawing{}_{}.vml", i + 1, k + 1);
            let xml = v.0.clone().unwrap_or_default();
            if xml.is_empty() {
                continue;
            }
            let rid = format!("rId{next}");
            next += 1;
            sheet_rel_entries[i].push((
                rid.clone(),
                REL_VMLDRAWING.to_string(),
                format!("../drawings/{name}"),
            ));
            if legacy_rids[i].is_none() {
                legacy_rids[i] = Some(rid);
            }
            vml_parts.push((name, xml));
        }
        // 工作表背景图关系。
        if let Some(bg) = &sheet.background {
            let base = std::path::Path::new(bg)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "bg.png".into());
            bg_no += 1;
            let name = format!("bg{bg_no}_{base}");
            let rid = format!("rId{next}");
            next += 1;
            sheet_rel_entries[i].push((
                rid.clone(),
                REL_IMAGE.to_string(),
                format!("../media/{name}"),
            ));
            bg_rids[i] = Some(rid);
            bg_media.push((name, bg.clone()));
        }
        // OLE 嵌入对象。
        for ole in &sheet.ole_objects {
            ole_no += 1;
            let bin = format!("oleObject{ole_no}.bin");
            let vml = format!("vmlDrawingOle{ole_no}.vml");
            let shape_id = 1026 + ole_no as u32;
            let rid_ole = format!("rId{next}");
            next += 1;
            sheet_rel_entries[i].push((
                rid_ole.clone(),
                REL_OLE_OBJECT.to_string(),
                format!("../embeddings/{bin}"),
            ));
            let rid_vml = format!("rId{next}");
            next += 1;
            sheet_rel_entries[i].push((
                rid_vml.clone(),
                REL_VMLDRAWING.to_string(),
                format!("../drawings/{vml}"),
            ));
            ole_vml_rids[i] = Some(rid_vml);
            let (fc, fr) = ole_anchor(sheet, ole.anchor.as_deref().unwrap_or("B2"));
            let (w, h) = ole_px(ole.size.as_ref());
            let vml_xml = ole_vml(shape_id, &ole.prog_id, fc, fr, w, h, ole.icon.as_deref());
            ole_parts.push((bin, vml, ole.src.clone(), vml_xml));
            ole_sheet[i].push((rid_ole, shape_id, ole.prog_id.clone()));
        }
        // 透视表：工作表关系 + <pivotTable> 引用。
        for (bi, built) in pivot_builds.iter().enumerate() {
            if built.0 != i {
                continue;
            }
            let rid = format!("rId{next}");
            next += 1;
            sheet_rel_entries[i].push((
                rid.clone(),
                REL_PIVOT_TABLE.to_string(),
                format!("../pivotTables/{}", built.1.table_name),
            ));
            pivot_rids[i].push(rid);
            let _ = bi;
        }
        // 切片器：工作表 rel（slicers 部件）。
        for (bi, built) in slicer_builds.iter().enumerate() {
            if built.0 != i {
                continue;
            }
            let rid = format!("rId{next}");
            next += 1;
            sheet_rel_entries[i].push((
                rid.clone(),
                REL_SLICERS.to_string(),
                format!("../slicers/{}", built.1.slicers_name),
            ));
            slicer_rids[i].push(rid);
            let _ = bi;
        }
    }

    let mut sst = SharedStrings::default();
    let mut sheet_xmls = Vec::with_capacity(n);
    for (i, sheet) in wb.sheets.iter().enumerate() {
        if sheet.sheet_type.as_deref() == Some("chart") {
            sheet_xmls.push(String::new());
            continue;
        }
        sheet_xmls.push(sheet::worksheet_xml(
            &wb,
            sheet,
            &styles,
            &mut sst,
            drawing_no[i].is_some(),
            &hyperlink_plans[i],
            &table_rids[i],
            legacy_rids[i].as_deref(),
            &pivot_rids[i],
            &slicer_rids[i],
            bg_rids[i].as_deref(),
            &ole_sheet[i],
            ole_vml_rids[i].as_deref(),
        )?);
    }

    let mut media_exts: Vec<String> = Vec::new();
    for (name, _) in &bg_media {
        if let Some(ext) = name.rsplit('.').next() {
            if !media_exts.iter().any(|e| e == ext) {
                media_exts.push(ext.to_string());
            }
        }
    }
    for (_, _, d) in &drawing_parts {
        for m in &d.media {
            if let Some(ext) = m.name.rsplit('.').next() {
                if !media_exts.iter().any(|e| e == ext) {
                    media_exts.push(ext.to_string());
                }
            }
        }
    }

    let file = std::fs::File::create(output_path)?;
    let mut zip = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let write_part = |zip: &mut ZipWriter<std::fs::File>, name: &str, data: &str| -> Result<()> {
        zip.start_file(name, opts)?;
        zip.write_all(data.as_bytes())?;
        Ok(())
    };

    write_part(
        &mut zip,
        "[Content_Types].xml",
        &content_types(
            &wb.sheets,
            &media_exts,
            drawing_parts.len(),
            chart_counter,
            &drawing_parts
                .iter()
                .flat_map(|(_, _, d)| d.chart_ex.iter().copied())
                .collect::<Vec<_>>(),
            table_parts.len(),
            comments_parts.len(),
            wb.theme.is_some(),
            !vml_parts.is_empty() || !ole_parts.is_empty(),
            pivot_cache_meta.len(),
            &slicer_builds
                .iter()
                .map(|(_, b)| (b.cache_name.clone(), b.slicers_name.clone()))
                .collect::<Vec<_>>(),
            !ole_parts.is_empty(),
        ),
    )?;
    write_part(&mut zip, "_rels/.rels", &root_rels())?;
    write_part(&mut zip, "docProps/core.xml", &core_props(&wb))?;
    write_part(&mut zip, "docProps/app.xml", &app_props())?;
    // Pivot caches (workbook rels + <pivotCaches>).
    let mut pivot_caches_xml = String::new();
    let mut cache_rels: Vec<(String, String)> = Vec::new();
    if !pivot_cache_meta.is_empty() {
        let base = n + 3 + if wb.theme.is_some() { 1 } else { 0 };
        pivot_caches_xml.push_str("<pivotCaches>");
        for (k, (cache_id, name)) in pivot_cache_meta.iter().enumerate() {
            let rid = format!("rId{}", base + k);
            pivot_caches_xml.push_str(&format!(
                "<pivotCache cacheId=\"{cache_id}\" r:id=\"{rid}\"/>"
            ));
            cache_rels.push((rid, format!("pivotCache/{name}")));
        }
        pivot_caches_xml.push_str("</pivotCaches>");
    }

    // Slicers (workbook extLst + rels + defined names).
    let mut slicer_cache_rels: Vec<(String, String)> = Vec::new();
    let mut slicer_ext = String::new();
    let mut slicer_names: Vec<String> = Vec::new();
    if !slicer_builds.is_empty() {
        let base = n + 3 + if wb.theme.is_some() { 1 } else { 0 } + pivot_cache_meta.len();
        slicer_ext.push_str(
            "<extLst><ext uri=\"{BBE1A952-AA13-448e-AADC-164F8A28A991}\" \
             xmlns:x14=\"http://schemas.microsoft.com/office/spreadsheetml/2009/9/main\">\
             <x14:slicerCaches>",
        );
        for (k, (_, b)) in slicer_builds.iter().enumerate() {
            let rid = format!("rId{}", base + k);
            slicer_ext.push_str(&format!("<x14:slicerCache r:id=\"{rid}\"/>"));
            slicer_cache_rels.push((rid, format!("slicerCaches/{}", b.cache_name)));
            slicer_names.push(b.slicer_name.clone());
        }
        slicer_ext.push_str("</x14:slicerCaches></ext></extLst>");
    }

    write_part(
        &mut zip,
        "xl/workbook.xml",
        &workbook_xml(&wb, &pivot_caches_xml, &slicer_ext, &slicer_names),
    )?;
    write_part(
        &mut zip,
        "xl/_rels/workbook.xml.rels",
        &workbook_rels(&wb, wb.theme.is_some(), &cache_rels, &slicer_cache_rels),
    )?;
    if let Some(theme) = &wb.theme {
        write_part(&mut zip, "xl/theme/theme1.xml", theme)?;
    }
    write_part(&mut zip, "xl/styles.xml", &styles.to_xml())?;
    write_part(&mut zip, "xl/sharedStrings.xml", &sst.to_xml())?;
    for (i, xml) in sheet_xmls.iter().enumerate() {
        if wb.sheets[i].sheet_type.as_deref() == Some("chart") {
            let raw = wb.sheets[i].raw_sheet.0.clone().unwrap_or_default();
            if !raw.is_empty() {
                write_part(
                    &mut zip,
                    &format!("xl/chartsheets/sheet{}.xml", i + 1),
                    &raw,
                )?;
            }
            if let Some(rels) = wb.sheets[i].raw_sheet_rels.0.as_deref() {
                if !rels.is_empty() {
                    write_part(
                        &mut zip,
                        &format!("xl/chartsheets/_rels/sheet{}.xml.rels", i + 1),
                        rels,
                    )?;
                }
            }
        } else {
            write_part(&mut zip, &format!("xl/worksheets/sheet{}.xml", i + 1), xml)?;
        }
    }

    let rel_doc = |rels: &str| {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Relationships xmlns=\"{NS_PKG_REL}\">{rels}</Relationships>"
        )
    };
    for (_si, b) in &pivot_builds {
        write_part(
            &mut zip,
            &format!("xl/pivotCache/{}", b.cache_name),
            &b.cache_xml,
        )?;
        write_part(
            &mut zip,
            &format!("xl/pivotCache/{}", b.records_name),
            &b.records_xml,
        )?;
        write_part(
            &mut zip,
            &format!("xl/pivotCache/_rels/{}.rels", b.cache_name),
            &rel_doc(&format!(
                "<Relationship Id=\"rId1\" Type=\"{REL_PIVOT_CACHE_REC}\" Target=\"{}\"/>",
                b.records_name
            )),
        )?;
        write_part(
            &mut zip,
            &format!("xl/pivotTables/{}", b.table_name),
            &b.table_xml,
        )?;
        write_part(
            &mut zip,
            &format!("xl/pivotTables/_rels/{}.rels", b.table_name),
            &rel_doc(&format!(
                "<Relationship Id=\"rId1\" Type=\"{REL_PIVOT_CACHE_DEF}\" Target=\"../pivotCache/{}\"/>",
                b.cache_name
            )),
        )?;
    }

    for (_si, b) in &slicer_builds {
        write_part(
            &mut zip,
            &format!("xl/slicerCaches/{}", b.cache_name),
            &b.cache_xml,
        )?;
        write_part(
            &mut zip,
            &format!("xl/slicers/{}", b.slicers_name),
            &b.slicers_xml,
        )?;
    }

    for (name, xml) in &vml_parts {
        write_part(&mut zip, &format!("xl/drawings/{name}"), xml)?;
    }

    let mut written_media: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (_sheet_idx, drawing_no, d) in &drawing_parts {
        write_part(
            &mut zip,
            &format!("xl/drawings/drawing{drawing_no}.xml"),
            &d.xml,
        )?;
        if !d.rels.is_empty() {
            write_part(
                &mut zip,
                &format!("xl/drawings/_rels/drawing{drawing_no}.xml.rels"),
                &drawing_rels_xml(&d.rels),
            )?;
        }
        for (name, xml) in &d.charts {
            write_part(&mut zip, &format!("xl/charts/{name}"), xml)?;
        }
        for m in &d.media {
            if !written_media.contains(&m.name) {
                written_media.insert(m.name.clone());
                zip.start_file(format!("xl/media/{}", m.name), opts)?;
                zip.write_all(&m.bytes)?;
            }
        }
    }

    for (name, xml) in &table_parts {
        write_part(&mut zip, &format!("xl/tables/{name}"), xml)?;
    }

    // 背景图媒体
    for (name, src) in &bg_media {
        if let Ok(bytes) = std::fs::read(src) {
            zip.start_file(format!("xl/media/{name}"), opts)?;
            zip.write_all(&bytes)?;
        }
    }

    // OLE 嵌入对象
    for (bin, vml, src, vml_xml) in &ole_parts {
        if let Ok(bytes) = std::fs::read(src) {
            zip.start_file(format!("xl/embeddings/{bin}"), opts)?;
            zip.write_all(&bytes)?;
        }
        write_part(&mut zip, &format!("xl/drawings/{vml}"), vml_xml)?;
        write_part(
            &mut zip,
            &format!("xl/drawings/_rels/{vml}.rels"),
            &rel_doc(&format!(
                "<Relationship Id=\"rId1\" Type=\"{REL_OLE_OBJECT}\" Target=\"../embeddings/{bin}\"/>"
            )),
        )?;
    }

    for cp in &comments_parts {
        write_part(
            &mut zip,
            &format!("xl/{}", cp.comments_name),
            &cp.comments_xml,
        )?;
        write_part(
            &mut zip,
            &format!("xl/drawings/{}", cp.vml_name),
            &cp.vml_xml,
        )?;
    }

    for (i, entries) in sheet_rel_entries.iter().enumerate() {
        if !entries.is_empty() {
            write_part(
                &mut zip,
                &format!("xl/worksheets/_rels/sheet{}.xml.rels", i + 1),
                &sheet_rels_xml(entries),
            )?;
        }
    }

    zip.finish()?;

    Ok(GenerateResult {
        path: output_path.display().to_string(),
        sheets: wb.sheets.len(),
    })
}

fn drawing_rels_xml(rels: &[(String, String)]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!("<Relationships xmlns=\"{NS_PKG_REL}\">"));
    for (id, target) in rels {
        let rel_type = if target.contains("/media/") {
            REL_IMAGE
        } else {
            REL_CHART
        };
        out.push_str(&format!(
            "<Relationship Id=\"{id}\" Type=\"{rel_type}\" Target=\"{target}\"/>"
        ));
    }
    out.push_str("</Relationships>");
    out
}

fn sheet_rels_xml(entries: &[(String, String, String)]) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!("<Relationships xmlns=\"{NS_PKG_REL}\">"));
    for (rid, rel_type, target) in entries {
        let mode = if rel_type == REL_HYPERLINK {
            " TargetMode=\"External\""
        } else {
            ""
        };
        out.push_str(&format!(
            "<Relationship Id=\"{rid}\" Type=\"{rel_type}\" Target=\"{}\"{mode}/>",
            esc_xml(target)
        ));
    }
    out.push_str("</Relationships>");
    out
}

fn ole_anchor(sheet: &crate::model::Sheet, anchor: &str) -> (f64, f64) {
    let (c, r) = crate::utils::a1::parse_cell_ref(anchor).unwrap_or((2, 2));
    let mut x = 0.0;
    for i in 0..(c - 1) {
        let w = sheet
            .columns
            .get(i as usize)
            .and_then(|col| col.width)
            .unwrap_or(8.43);
        x += w * 7.0 + 5.0;
    }
    let dflt = sheet.default_row_height.unwrap_or(15.0);
    let mut y = 0.0;
    for row in 1..r {
        let h = sheet
            .rows
            .iter()
            .find(|rr| rr.index == Some(row))
            .and_then(|rr| rr.height)
            .unwrap_or(dflt);
        y += h * 96.0 / 72.0;
    }
    (x * 0.75, y * 0.75)
}

fn ole_px(size: Option<&crate::model::Size>) -> (f64, f64) {
    let (w, h) = size.map(|s| (s.w, s.h)).unwrap_or((1.5, 0.5));
    (w * 72.0, h * 72.0)
}

fn ole_vml(
    shape_id: u32,
    prog: &str,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    _icon: Option<&str>,
) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <xml xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" xmlns:x=\"urn:schemas-microsoft-com:office:excel\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
         <o:shapelayout v:ext=\"edit\"><o:idmap v:ext=\"edit\" data=\"{shape_id}\"/></o:shapelayout>\
         <v:shapetype id=\"_x0000_t75\" coordsize=\"21600,21600\" o:spt=\"75\" o:preferrelative=\"t\" path=\"m@4@5l@4@11@9@11@9@5xe\" filled=\"f\" stroked=\"f\">\
         <v:stroke joinstyle=\"miter\"/><v:formulas><v:f eqn=\"if lineDrawn pixelLineWidth 0\"/><v:f eqn=\"sum @0 1 0\"/><v:f eqn=\"sum 0 0 @1\"/><v:f eqn=\"prod @2 1 2\"/><v:f eqn=\"prod @3 21600 pixelWidth\"/><v:f eqn=\"prod @3 21600 pixelHeight\"/><v:f eqn=\"sum @0 0 1\"/><v:f eqn=\"prod @6 1 2\"/><v:f eqn=\"prod @7 21600 pixelWidth\"/><v:f eqn=\"sum @8 21600 0\"/><v:f eqn=\"prod @7 21600 pixelHeight\"/><v:f eqn=\"sum @10 21600 0\"/></v:formulas>\
         <v:path o:extrusionok=\"f\" gradientshapeok=\"t\" o:connecttype=\"rect\"/><o:lock v:ext=\"edit\" aspectratio=\"t\"/></v:shapetype>\
         <v:shape id=\"_x0000_s{shape_id}\" type=\"#_x0000_t75\" style=\"position:absolute;margin-left:{x:.0}pt;margin-top:{y:.0}pt;width:{w:.0}pt;height:{h:.0}pt;z-index:1\" o:ole=\"\">\
         <o:OLEObject Type=\"Embed\" ProgID=\"{prog}\" ShapeID=\"_x0000_s{shape_id}\" DrawAspect=\"Icon\" ObjectID=\"_{shape_id}\" r:id=\"rId1\"/>\
         </v:shape></xml>"
    )
}

#[allow(clippy::too_many_arguments)]
fn content_types(
    sheets: &[crate::model::Sheet],
    media_exts: &[String],
    drawing_count: usize,
    chart_count: usize,
    chart_ex: &[bool],
    table_count: usize,
    comments_count: usize,
    has_theme: bool,
    has_vml: bool,
    pivot_count: usize,
    slicers: &[(String, String)],
    has_ole: bool,
) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!("<Types xmlns=\"{NS_CONTENT_TYPES}\">"));
    out.push_str(&format!(
        "<Default Extension=\"rels\" ContentType=\"{CT_RELS}\"/>"
    ));
    out.push_str(&format!(
        "<Default Extension=\"xml\" ContentType=\"{CT_XML}\"/>"
    ));
    for ext in media_exts {
        let ct = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            _ => "application/octet-stream",
        };
        out.push_str(&format!(
            "<Default Extension=\"{ext}\" ContentType=\"{ct}\"/>"
        ));
    }
    out.push_str(&format!(
        "<Override PartName=\"/xl/workbook.xml\" ContentType=\"{CT_WORKBOOK}\"/>"
    ));
    out.push_str(&format!(
        "<Override PartName=\"/xl/styles.xml\" ContentType=\"{CT_STYLES}\"/>"
    ));
    out.push_str(&format!(
        "<Override PartName=\"/xl/sharedStrings.xml\" ContentType=\"{CT_SHARED_STRINGS}\"/>"
    ));
    if has_theme {
        out.push_str(&format!(
            "<Override PartName=\"/xl/theme/theme1.xml\" ContentType=\"{CT_THEME}\"/>"
        ));
    }
    for (i, sheet) in sheets.iter().enumerate() {
        if sheet.sheet_type.as_deref() == Some("chart") {
            out.push_str(&format!(
                "<Override PartName=\"/xl/chartsheets/sheet{}.xml\" ContentType=\"{CT_CHARTSHEET}\"/>",
                i + 1
            ));
        } else {
            out.push_str(&format!(
                "<Override PartName=\"/xl/worksheets/sheet{}.xml\" ContentType=\"{CT_WORKSHEET}\"/>",
                i + 1
            ));
        }
    }
    for i in 0..drawing_count {
        out.push_str(&format!(
            "<Override PartName=\"/xl/drawings/drawing{}.xml\" ContentType=\"{CT_DRAWING}\"/>",
            i + 1
        ));
    }
    for i in 0..chart_count {
        let ct = if chart_ex.get(i).copied().unwrap_or(false) {
            CT_CHART_EX
        } else {
            CT_CHART
        };
        out.push_str(&format!(
            "<Override PartName=\"/xl/charts/chart{}.xml\" ContentType=\"{ct}\"/>",
            i + 1
        ));
    }
    for i in 0..table_count {
        out.push_str(&format!(
            "<Override PartName=\"/xl/tables/table{}.xml\" ContentType=\"{CT_TABLE}\"/>",
            i + 1
        ));
    }
    for i in 0..pivot_count {
        out.push_str(&format!(
            "<Override PartName=\"/xl/pivotCache/pivotCacheDefinition{}.xml\" ContentType=\"{CT_PIVOT_CACHE_DEF}\"/>",
            i + 1
        ));
        out.push_str(&format!(
            "<Override PartName=\"/xl/pivotCache/pivotCacheRecords{}.xml\" ContentType=\"{CT_PIVOT_CACHE_REC}\"/>",
            i + 1
        ));
        out.push_str(&format!(
            "<Override PartName=\"/xl/pivotTables/pivotTable{}.xml\" ContentType=\"{CT_PIVOT_TABLE}\"/>",
            i + 1
        ));
    }
    for (cache, part) in slicers {
        out.push_str(&format!(
            "<Override PartName=\"/xl/slicerCaches/{cache}\" ContentType=\"{CT_SLICER_CACHE}\"/>"
        ));
        out.push_str(&format!(
            "<Override PartName=\"/xl/slicers/{part}\" ContentType=\"{CT_SLICERS}\"/>"
        ));
    }
    if comments_count > 0 || has_vml {
        out.push_str(&format!(
            "<Default Extension=\"vml\" ContentType=\"{CT_VML}\"/>"
        ));
    }
    if has_ole {
        out.push_str(&format!(
            "<Default Extension=\"bin\" ContentType=\"{CT_OLE}\"/>"
        ));
    }
    for i in 0..comments_count {
        out.push_str(&format!(
            "<Override PartName=\"/xl/comments{}.xml\" ContentType=\"{CT_COMMENTS}\"/>",
            i + 1
        ));
    }
    out.push_str(&format!(
        "<Override PartName=\"/docProps/core.xml\" ContentType=\"{CT_CORE_PROPS}\"/>"
    ));
    out.push_str(&format!(
        "<Override PartName=\"/docProps/app.xml\" ContentType=\"{CT_EXTENDED_PROPS}\"/>"
    ));
    out.push_str("</Types>");
    out
}

fn root_rels() -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!("<Relationships xmlns=\"{NS_PKG_REL}\">"));
    out.push_str(&format!(
        "<Relationship Id=\"rId1\" Type=\"{REL_OFFICE_DOCUMENT}\" Target=\"xl/workbook.xml\"/>"
    ));
    out.push_str(&format!(
        "<Relationship Id=\"rId2\" Type=\"{REL_CORE_PROPS}\" Target=\"docProps/core.xml\"/>"
    ));
    out.push_str(&format!(
        "<Relationship Id=\"rId3\" Type=\"{REL_EXTENDED_PROPS}\" Target=\"docProps/app.xml\"/>"
    ));
    out.push_str("</Relationships>");
    out
}

fn core_props(wb: &Workbook) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!(
        "<cp:coreProperties xmlns:cp=\"{NS_CP}\" xmlns:dc=\"{NS_DC}\" xmlns:dcterms=\"{NS_DCTERMS}\" xmlns:xsi=\"{NS_XSI}\">"
    ));
    if let Some(meta) = &wb.meta {
        if let Some(t) = &meta.title {
            out.push_str(&format!("<dc:title>{}</dc:title>", esc_xml(t)));
        }
        if let Some(a) = &meta.author {
            out.push_str(&format!("<dc:creator>{}</dc:creator>", esc_xml(a)));
            out.push_str(&format!(
                "<cp:lastModifiedBy>{}</cp:lastModifiedBy>",
                esc_xml(a)
            ));
        }
    }
    out.push_str("</cp:coreProperties>");
    out
}

fn app_props() -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<Properties xmlns=\"{NS_EP}\" xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\"><Application>json2xlsx</Application></Properties>"
    )
}

fn workbook_xml(
    wb: &Workbook,
    pivot_caches: &str,
    slicer_ext: &str,
    slicer_names: &[String],
) -> String {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!(
        "<workbook xmlns=\"{NS_MAIN}\" xmlns:r=\"{NS_REL}\">"
    ));
    out.push_str("<fileVersion appName=\"json2xlsx\"/>");
    if wb.date1904 {
        out.push_str("<workbookPr date1904=\"1\"/>");
    }
    out.push_str("<bookViews><workbookView");
    if let Some(t) = wb.active_tab {
        out.push_str(&format!(" activeTab=\"{t}\""));
    }
    out.push_str("/></bookViews>");
    out.push_str("<sheets>");
    for (i, sheet) in wb.sheets.iter().enumerate() {
        let state = if sheet.hidden {
            " state=\"hidden\""
        } else {
            ""
        };
        out.push_str(&format!(
            "<sheet name=\"{}\" sheetId=\"{}\"{state} r:id=\"rId{}\"/>",
            esc_xml(&sheet.name),
            i + 1,
            i + 1
        ));
    }
    out.push_str("</sheets>");
    let mut names: Vec<String> = Vec::new();
    for nr in &wb.named_ranges {
        names.push(format!(
            "<definedName name=\"{}\">{}</definedName>",
            esc_xml(&nr.name),
            esc_xml(&nr.reference)
        ));
    }
    for (i, sheet) in wb.sheets.iter().enumerate() {
        if let Some(area) = &sheet.print_area {
            if let Some(r) = abs_range(area) {
                names.push(format!(
                    "<definedName name=\"_xlnm.Print_Area\" localSheetId=\"{i}\">{}!{}</definedName>",
                    esc_xml(&sheet.name),
                    r
                ));
            }
        }
        let mut titles: Vec<String> = Vec::new();
        if let Some(rows) = &sheet.print_title_rows {
            titles.push(format!("{}!{}", esc_xml(&sheet.name), abs_rows(rows)));
        }
        if let Some(cols) = &sheet.print_title_cols {
            titles.push(format!("{}!{}", esc_xml(&sheet.name), abs_cols(cols)));
        }
        if !titles.is_empty() {
            names.push(format!(
                "<definedName name=\"_xlnm.Print_Titles\" localSheetId=\"{i}\">{}</definedName>",
                titles.join(",")
            ));
        }
    }
    for n in slicer_names {
        names.push(format!(
            "<definedName name=\"{}\">#N/A</definedName>",
            esc_xml(n)
        ));
    }
    if !names.is_empty() {
        out.push_str("<definedNames>");
        for n in names {
            out.push_str(&n);
        }
        out.push_str("</definedNames>");
    }
    out.push_str("<calcPr calcId=\"0\" fullCalcOnLoad=\"1\"/>");
    out.push_str(pivot_caches);
    out.push_str(slicer_ext);
    out.push_str("</workbook>");
    out
}

fn abs_range(r: &str) -> Option<String> {
    let ((c1, r1), (c2, r2)) = crate::utils::a1::parse_range(r).ok()?;
    Some(format!(
        "${}${}:${}${}",
        crate::utils::a1::index_to_col(c1),
        r1,
        crate::utils::a1::index_to_col(c2),
        r2
    ))
}

fn abs_rows(r: &str) -> String {
    let r = r.replace('$', "");
    match r.split_once(':') {
        Some((a, b)) => format!("${a}:${b}"),
        None => format!("${r}"),
    }
}

fn abs_cols(c: &str) -> String {
    let c = c.replace('$', "").to_uppercase();
    match c.split_once(':') {
        Some((a, b)) => format!("${a}:${b}"),
        None => format!("${c}"),
    }
}

fn workbook_rels(
    wb: &Workbook,
    has_theme: bool,
    cache_rels: &[(String, String)],
    slicer_cache_rels: &[(String, String)],
) -> String {
    let sheet_count = wb.sheets.len();
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    out.push_str(&format!("<Relationships xmlns=\"{NS_PKG_REL}\">"));
    for (i, sheet) in wb.sheets.iter().enumerate() {
        if sheet.sheet_type.as_deref() == Some("chart") {
            out.push_str(&format!(
                "<Relationship Id=\"rId{}\" Type=\"{REL_CHARTSHEET}\" Target=\"chartsheets/sheet{}.xml\"/>",
                i + 1,
                i + 1
            ));
        } else {
            out.push_str(&format!(
                "<Relationship Id=\"rId{}\" Type=\"{REL_WORKSHEET}\" Target=\"worksheets/sheet{}.xml\"/>",
                i + 1,
                i + 1
            ));
        }
    }
    let styles_rid = sheet_count + 1;
    let sst_rid = sheet_count + 2;
    out.push_str(&format!(
        "<Relationship Id=\"rId{styles_rid}\" Type=\"{REL_STYLES}\" Target=\"styles.xml\"/>"
    ));
    out.push_str(&format!(
        "<Relationship Id=\"rId{sst_rid}\" Type=\"{REL_SHARED_STRINGS}\" Target=\"sharedStrings.xml\"/>"
    ));
    if has_theme {
        let theme_rid = sst_rid + 1;
        out.push_str(&format!(
            "<Relationship Id=\"rId{theme_rid}\" Type=\"{REL_THEME}\" Target=\"theme/theme1.xml\"/>"
        ));
    }
    for (rid, target) in cache_rels {
        out.push_str(&format!(
            "<Relationship Id=\"{rid}\" Type=\"{REL_PIVOT_CACHE_DEF}\" Target=\"{target}\"/>"
        ));
    }
    for (rid, target) in slicer_cache_rels {
        out.push_str(&format!(
            "<Relationship Id=\"{rid}\" Type=\"{REL_SLICER_CACHE}\" Target=\"{target}\"/>"
        ));
    }
    out.push_str("</Relationships>");
    out
}
