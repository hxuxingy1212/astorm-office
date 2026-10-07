//! Pivot table creation: `pivotCacheDefinition` + `pivotTableDefinition` parts.
//!
//! 采用 `refreshOnLoad="1"`：Excel/LibreOffice 打开时按 `worksheetSource`
//! 重新计算透视，因此无需预先写入缓存记录。

use serde_json::Value;

use crate::error::Result;
use crate::model::sheet::{PivotTable, Sheet};
use crate::model::workbook::Workbook;
use crate::utils::a1::{parse_cell_ref, parse_range};
use crate::utils::constants::NS_MAIN;
use crate::utils::xml::esc_xml;

pub struct PivotBuilt {
    pub cache_name: String,
    pub records_name: String,
    pub table_name: String,
    pub cache_xml: String,
    pub records_xml: String,
    pub table_xml: String,
    pub cache_id: u32,
}

/// 源引用：返回 (sheet 名, "A1:D100")。
fn split_source(sheet: &Sheet, source: &str) -> (String, String) {
    match source.rsplit_once('!') {
        Some((s, r)) => (
            s.trim_matches(|c| c == '\'' || c == '"').to_string(),
            r.replace('$', ""),
        ),
        None => (sheet.name.clone(), source.replace('$', "")),
    }
}

/// 源区域首行的表头名称（缺失则 ColumnN）。
pub fn header_fields(wb: &Workbook, sheet: &Sheet, source: &str) -> Vec<String> {
    let (sname, range) = split_source(sheet, source);
    let src = wb
        .sheets
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(&sname));
    let (c1, r1, c2) = match parse_range(&range) {
        Ok(((c1, r1), (c2, _))) => (c1, r1, c2),
        Err(_) => (1, 1, 1),
    };
    let mut out = Vec::new();
    for c in c1..=c2 {
        let name = src
            .and_then(|s| {
                s.rows
                    .iter()
                    .find(|row| row.index == Some(r1))
                    .and_then(|row| {
                        row.cells
                            .iter()
                            .find(|cell| {
                                cell.reference
                                    .as_deref()
                                    .and_then(|r| parse_cell_ref(r).ok())
                                    .map(|(cc, _)| cc == c)
                                    .unwrap_or(false)
                            })
                            .and_then(|cell| match &cell.value {
                                Some(Value::String(s)) => Some(s.clone()),
                                Some(v) => Some(v.to_string()),
                                None => None,
                            })
                    })
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("Column{}", c - c1 + 1));
        out.push(name);
    }
    out
}

fn field_index(fields: &[String], name: &str) -> Option<usize> {
    fields.iter().position(|f| f.eq_ignore_ascii_case(name))
}

pub fn build_pivot(
    wb: &Workbook,
    sheet: &Sheet,
    pivot: &PivotTable,
    table_no: usize,
    cache_id: u32,
    pivot_cache_id: u32,
) -> Result<PivotBuilt> {
    let fields = header_fields(wb, sheet, &pivot.source);
    let n = fields.len().max(1);
    let name = pivot
        .name
        .clone()
        .unwrap_or_else(|| format!("Pivot{table_no}"));

    let row_idx: Vec<usize> = pivot
        .rows
        .iter()
        .filter_map(|f| field_index(&fields, f))
        .collect();
    let col_idx: Vec<usize> = pivot
        .columns
        .iter()
        .filter_map(|f| field_index(&fields, f))
        .collect();
    let val_idx: Vec<usize> = pivot
        .values
        .iter()
        .filter_map(|v| field_index(&fields, &v.field))
        .collect();

    // --- pivotCacheDefinition ---
    let (sname, range) = split_source(sheet, &pivot.source);
    let mut cache = String::new();
    cache.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    cache.push_str(&format!(
        "<pivotCacheDefinition xmlns=\"{NS_MAIN}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
         r:id=\"rId1\" refreshOnLoad=\"1\" refreshedVersion=\"8\" createdVersion=\"8\" recordCount=\"0\">"
    ));
    cache.push_str(&format!(
        "<cacheSource type=\"worksheet\"><worksheetSource ref=\"{}\" sheet=\"{}\"/></cacheSource>",
        esc_xml(&range),
        esc_xml(&sname)
    ));
    cache.push_str(&format!("<cacheFields count=\"{n}\">"));
    for f in &fields {
        cache.push_str(&format!(
            "<cacheField name=\"{}\" numFmtId=\"0\"><sharedItems/></cacheField>",
            esc_xml(f)
        ));
    }
    // 字段数不足时补齐
    for i in fields.len()..n {
        cache.push_str(&format!(
            "<cacheField name=\"Column{}\" numFmtId=\"0\"><sharedItems/></cacheField>",
            i + 1
        ));
    }
    cache.push_str("</cacheFields>");
    // x14:pivotCacheDefinition 扩展（供切片器引用其 pivotCacheId）。
    cache.push_str(&format!(
        "<extLst><ext uri=\"{{725AE2AE-9491-48be-B2B4-4EB974FC3084}}\" xmlns:x14=\"http://schemas.microsoft.com/office/spreadsheetml/2009/9/main\"><x14:pivotCacheDefinition pivotCacheId=\"{pivot_cache_id}\"/></ext></extLst>"
    ));
    cache.push_str("</pivotCacheDefinition>");

    // --- pivotCacheRecords ---
    let records = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<pivotCacheRecords xmlns=\"{NS_MAIN}\" count=\"0\"/>"
    );

    // --- pivotTableDefinition ---
    // 输出区域：锚点 + 粗略范围（refreshOnLoad 会修正）。
    let anchor = pivot.position.clone().unwrap_or_else(|| "A3".into());
    let (ac, ar) = parse_cell_ref(&anchor).unwrap_or((1, 3));
    let cols_span = (col_idx.len() + 1).max(2) as u32;
    let rows_span = (row_idx.len() + 2).max(3) as u32;
    let end = crate::utils::a1::make_cell_ref(ac + cols_span, ar + rows_span);
    let loc_ref = format!("{}:{}", crate::utils::a1::make_cell_ref(ac, ar), end);

    let mut t = String::new();
    t.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    t.push_str(&format!(
        "<pivotTableDefinition xmlns=\"{NS_MAIN}\" name=\"{}\" cacheId=\"{cache_id}\" dataCaption=\"Values\" \
         updatedVersion=\"8\" createdVersion=\"8\" indent=\"0\" compact=\"0\" compactData=\"0\" \
         useAutoFormatting=\"1\" itemPrintTitles=\"1\" rowGrandTotals=\"1\" colGrandTotals=\"1\" \
         showDrill=\"1\" enableDrill=\"1\" showDataTips=\"1\" enableWizard=\"1\" enableEdit=\"1\" autoFormatId=\"0\">",
        esc_xml(&name)
    ));
    t.push_str(&format!(
        "<location ref=\"{loc_ref}\" firstHeaderRow=\"1\" firstDataRow=\"1\" firstDataCol=\"1\"/>"
    ));
    t.push_str(&format!("<pivotFields count=\"{n}\">"));
    for i in 0..n {
        if row_idx.contains(&i) {
            t.push_str(
                "<pivotField axis=\"axisRow\" showAll=\"0\"><items count=\"0\"/></pivotField>",
            );
        } else if col_idx.contains(&i) {
            t.push_str(
                "<pivotField axis=\"axisCol\" showAll=\"0\"><items count=\"0\"/></pivotField>",
            );
        } else if val_idx.contains(&i) {
            t.push_str(
                "<pivotField dataField=\"1\" showAll=\"0\"><items count=\"0\"/></pivotField>",
            );
        } else {
            t.push_str("<pivotField showAll=\"0\"><items count=\"0\"/></pivotField>");
        }
    }
    t.push_str("</pivotFields>");
    if !row_idx.is_empty() {
        t.push_str(&format!("<rowFields count=\"{}\">", row_idx.len()));
        for i in &row_idx {
            t.push_str(&format!("<field x=\"{i}\"/>"));
        }
        t.push_str("</rowFields>");
    }
    if !col_idx.is_empty() {
        t.push_str(&format!("<colFields count=\"{}\">", col_idx.len()));
        for i in &col_idx {
            t.push_str(&format!("<field x=\"{i}\"/>"));
        }
        t.push_str("</colFields>");
    }
    t.push_str(&format!("<dataFields count=\"{}\">", val_idx.len()));
    for (k, (vi, v)) in val_idx.iter().zip(pivot.values.iter()).enumerate() {
        let func = v.function.as_deref().unwrap_or("sum").to_ascii_lowercase();
        let disp = v
            .name
            .clone()
            .unwrap_or_else(|| format!("{} of {}", title_case(&func), v.field));
        let subtotal = if func == "sum" {
            String::new()
        } else {
            format!(" subtotal=\"{func}\"")
        };
        t.push_str(&format!(
            "<dataField name=\"{}\" fld=\"{vi}\"{subtotal} baseField=\"0\" baseItem=\"0\"/>",
            esc_xml(&disp)
        ));
        let _ = k;
    }
    t.push_str("</dataFields>");
    let style = pivot
        .style
        .clone()
        .unwrap_or_else(|| "PivotStyleLight16".into());
    t.push_str(&format!(
        "<pivotTableStyleInfo name=\"{}\" showRowHeaders=\"1\" showColHeaders=\"1\" showRowStripes=\"0\" showColStripes=\"0\" showLastColumn=\"0\"/>",
        esc_xml(&style)
    ));
    t.push_str("</pivotTableDefinition>");

    Ok(PivotBuilt {
        cache_name: format!("pivotCacheDefinition{table_no}.xml"),
        records_name: format!("pivotCacheRecords{table_no}.xml"),
        table_name: format!("pivotTable{table_no}.xml"),
        cache_xml: cache,
        records_xml: records,
        table_xml: t,
        cache_id,
    })
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
