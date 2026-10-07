pub mod comment;
pub mod drawing;
pub mod shared_strings;
pub mod sheet;
pub mod styles;
pub mod table;
pub mod theme;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::Reader;
use zip::ZipArchive;

use crate::error::{Error, Result};
use crate::model::cell::Cell;
use crate::model::sheet::{Chart, Column, Image, Row, Sheet, Table};
use crate::model::workbook::{Meta, NamedRange, Workbook};

/// Parse an `.xlsx` file into a [`Workbook`].
pub fn parse(input: &Path) -> Result<Workbook> {
    // 旧版 .xls（OLE2/CFB）自动识别：转换为 Workbook 后走统一流水线
    if crate::legacy::is_cfb(input) {
        if let Some(res) = crate::legacy::maybe_legacy_workbook(input) {
            return res;
        }
    }
    let mut probe = File::open(input)?;
    let mut magic = [0u8; 8];
    let n = probe.read(&mut magic)?;
    drop(probe);
    let file = File::open(input)?;
    let mut zip = match ZipArchive::new(file) {
        Ok(z) => z,
        Err(e) => {
            // OLE/CFB 复合文档（含加密 xlsx）不是 zip 包，给出明确提示。
            if n == 8 && magic == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1] {
                return Err(Error::InvalidInput(
                    "文件已加密或为受保护的 OLE/CFB 格式，暂不支持".into(),
                ));
            }
            return Err(e.into());
        }
    };

    // 通过根关系定位 workbook，兼容根级/非标准（无 xl/ 前缀）包。
    let root_rels = read_part(&mut zip, "_rels/.rels")?
        .map(|x| parse_rels(&x))
        .unwrap_or_default();
    let wb_path = root_rels
        .values()
        .find(|t| t.to_ascii_lowercase().contains("workbook.xml"))
        .map(|t| drawing::resolve_part("", t))
        .unwrap_or_else(|| "xl/workbook.xml".into());
    let wb_dir = wb_path
        .rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default();
    let wb_rels_path = join_part(
        &wb_dir,
        &format!("_rels/{}.rels", drawing::basename(&wb_path)),
    );
    let wb_rels = read_part(&mut zip, &wb_rels_path)?.unwrap_or_default();
    let rel_map = parse_rels(&wb_rels);
    let rel_types = parse_rel_types(&wb_rels);
    let rel_of = |needle: &str| -> Option<String> {
        rel_map
            .values()
            .find(|t| t.to_ascii_lowercase().contains(needle))
            .map(|t| drawing::resolve_part(&wb_dir, t))
    };

    let shared_path =
        rel_of("sharedstrings").unwrap_or_else(|| join_part(&wb_dir, "sharedStrings.xml"));
    let shared = match read_part(&mut zip, &shared_path)? {
        Some(x) => shared_strings::parse_shared_strings(&x)?,
        None => Vec::new(),
    };
    let styles_path = rel_of("styles").unwrap_or_else(|| join_part(&wb_dir, "styles.xml"));
    let theme_path = rel_of("theme").unwrap_or_else(|| join_part(&wb_dir, "theme/theme1.xml"));
    let theme_xml = read_part(&mut zip, &theme_path)?;
    let theme_colors = theme_xml.as_deref().map(theme::parse_theme);
    let parsed_styles = match read_part(&mut zip, &styles_path)? {
        Some(x) => styles::parse_styles(&x, theme_colors.as_ref())?,
        None => styles::ParsedStyles {
            xfs: vec![None],
            num_fmts: vec![None],
            dxfs: Vec::new(),
            default_font: None,
        },
    };

    let wb_xml = read_part(&mut zip, &wb_path)?
        .ok_or_else(|| Error::InvalidInput("缺少 xl/workbook.xml".into()))?;
    let info = parse_workbook_xml(&wb_xml)?;

    let mut sheets = Vec::new();
    for (i, entry) in info.sheets.iter().enumerate() {
        // 图表工作表（chartsheet）：原样保留，不按工作表解析。
        let is_chartsheet = rel_types
            .get(&entry.rid)
            .map(|t| t.ends_with("/chartsheet"))
            .unwrap_or(false);
        if is_chartsheet {
            let part = rel_map
                .get(&entry.rid)
                .map(|t| drawing::resolve_part(&wb_dir, t))
                .unwrap_or_default();
            let xml = read_part(&mut zip, &part)?.unwrap_or_default();
            let rels_path = if let Some((dir, base)) = part.rsplit_once('/') {
                format!("{dir}/_rels/{base}.rels")
            } else {
                format!("_rels/{part}.rels")
            };
            let rels_xml = read_part(&mut zip, &rels_path)?.unwrap_or_default();
            sheets.push(Sheet {
                name: entry.name.clone(),
                sheet_type: Some("chart".into()),
                raw_sheet: crate::model::sheet::RawXml::from(&xml),
                raw_sheet_rels: crate::model::sheet::RawXml::from(&rels_xml),
                hidden: entry.hidden,
                ..Default::default()
            });
            continue;
        }
        let part = match rel_map.get(&entry.rid) {
            Some(target) => drawing::resolve_part(&wb_dir, target),
            None => join_part(&wb_dir, &format!("worksheets/sheet{}.xml", entry.sheet_id)),
        };
        let xml = read_part(&mut zip, &part)?
            .ok_or_else(|| Error::InvalidInput(format!("缺少工作表部件: {part}")))?;
        let sheet_rels_xml = read_part(
            &mut zip,
            &format!("xl/worksheets/_rels/sheet{}.xml.rels", i + 1),
        )?;
        let rels = sheet_rels_xml
            .as_deref()
            .map(parse_rels)
            .unwrap_or_default();
        let sheet_rel_types = sheet_rels_xml
            .as_deref()
            .map(parse_rel_types)
            .unwrap_or_default();
        let ps = sheet::parse_sheet(&xml, &shared, &parsed_styles, info.date1904, &rels)?;
        let mut rows = ps.rows;
        // 表单控件 VML（按钮等）——注释的 VML 由生成端重建，这里只保留非注释的。
        let mut vml_drawings = Vec::new();
        if !rels.values().any(|t| t.contains("comments")) {
            for (id, ty) in &sheet_rel_types {
                if ty.ends_with("/vmlDrawing") {
                    if let Some(target) = rels.get(id) {
                        let vp = drawing::resolve_part("xl/worksheets", target);
                        if let Some(vx) = read_part(&mut zip, &vp)? {
                            vml_drawings.push(crate::model::sheet::RawXml::from(&vx));
                        }
                    }
                }
            }
        }
        let comments = read_comments(&mut zip, &rels)?;
        if !comments.is_empty() {
            apply_comments(&mut rows, &comments);
        }
        let (images, charts, shapes) = read_drawing(
            &mut zip,
            &rels,
            &ps.columns,
            &rows,
            ps.default_col_width,
            ps.default_row_height,
        )?;
        let tables = read_tables(&mut zip, &rels)?;
        sheets.push(Sheet {
            name: entry.name.clone(),
            sheet_type: None,
            raw_sheet: Default::default(),
            raw_sheet_rels: Default::default(),
            tab_color: ps.tab_color,
            hidden: entry.hidden,
            freeze: ps.freeze,
            direction: ps.direction,
            gridlines: ps.gridlines,
            headings: ps.headings,
            zoom: ps.zoom,
            default_col_width: ps.default_col_width,
            default_row_height: ps.default_row_height,
            base_col_width: ps.base_col_width,
            show_formulas: ps.show_formulas,
            tab_selected: ps.tab_selected,
            auto_filter: ps.auto_filter,
            filter_columns: ps.filter_columns,
            columns: ps.columns,
            merges: ps.merges,
            protect: ps.protect,
            validations: ps.validations,
            conditional_formats: ps.conditional_formats,
            sparklines: ps.sparklines,
            sort: ps.sort,
            background: ps.background,
            pivot_tables: Vec::new(),
            slicers: Vec::new(),
            drawing_shapes: Vec::new(),
            ole_objects: Vec::new(),
            print: ps.print,
            print_area: None,
            print_title_rows: None,
            print_title_cols: None,
            tables,
            rows,
            charts,
            images,
            shapes,
            vml_drawings,
        });
    }

    apply_reserved_names(&mut sheets, &info.reserved);

    let meta = match read_part(&mut zip, "docProps/core.xml")? {
        Some(x) => parse_core(&x),
        None => None,
    };

    Ok(Workbook {
        meta,
        date1904: info.date1904,
        styles: Default::default(),
        default_font: parsed_styles
            .default_font
            .clone()
            .filter(|f| !crate::model::workbook::is_calibri_default(f)),
        active_tab: info.active_tab,
        named_ranges: info.named_ranges,
        sheets,
        theme: theme_xml,
    })
}

fn read_part(zip: &mut ZipArchive<File>, name: &str) -> Result<Option<String>> {
    // 1) 精确匹配（快路径）
    match zip.by_name(name) {
        Ok(mut f) => {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            return Ok(Some(decode_xml(&buf)?));
        }
        Err(zip::result::ZipError::FileNotFound) => {}
        Err(e) => return Err(e.into()),
    }
    // 2) 归一化匹配：忽略大小写与 `\` 分隔符（部分工具生成的 zip 不合规）
    let want = normalize_name(name);
    let mut idx = None;
    for i in 0..zip.len() {
        if let Ok(f) = zip.by_index(i) {
            if normalize_name(f.name()) == want {
                idx = Some(i);
                break;
            }
        }
    }
    let Some(i) = idx else { return Ok(None) };
    let mut f = zip.by_index(i)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    Ok(Some(decode_xml(&buf)?))
}

fn normalize_name(s: &str) -> String {
    s.replace('\\', "/").to_ascii_lowercase()
}

fn join_part(dir: &str, rest: &str) -> String {
    if dir.is_empty() {
        rest.to_string()
    } else {
        format!("{dir}/{rest}")
    }
}

/// 将 XML 部件字节解码为字符串，支持 UTF-8（含 BOM）与 UTF-16 LE/BE。
fn decode_xml(buf: &[u8]) -> Result<String> {
    if let Some(rest) = buf.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return Ok(String::from_utf8_lossy(rest).into_owned());
    }
    if let Some(rest) = buf.strip_prefix(&[0xFF, 0xFE]) {
        return Ok(decode_utf16(rest, true));
    }
    if let Some(rest) = buf.strip_prefix(&[0xFE, 0xFF]) {
        return Ok(decode_utf16(rest, false));
    }
    // 无 BOM 的 UTF-16：XML 以 `<?xml` 或 `<` 开头，据此判定字节序。
    if buf.starts_with(&[0x00, 0x3C]) {
        return Ok(decode_utf16(buf, false));
    }
    if buf.starts_with(&[0x3C, 0x00]) {
        return Ok(decode_utf16(buf, true));
    }
    match std::str::from_utf8(buf) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => Ok(String::from_utf8_lossy(buf).into_owned()),
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> String {
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| {
            if little_endian {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        })
        .collect();
    String::from_utf16_lossy(&units)
}

/// Read a sheet's drawing part and return its images and charts.
fn read_drawing(
    zip: &mut ZipArchive<File>,
    rels: &HashMap<String, String>,
    columns: &[Column],
    rows: &[Row],
    default_col: Option<f64>,
    default_row: Option<f64>,
) -> Result<(Vec<Image>, Vec<Chart>, Vec<String>)> {
    let Some(drawing_target) = rels
        .values()
        .find(|t| t.contains("drawings/") && t.to_ascii_lowercase().ends_with(".xml"))
        .cloned()
    else {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    };
    let drawing_part = drawing::resolve_part("xl/worksheets", &drawing_target);
    let Some(drawing_xml) = read_part(zip, &drawing_part)? else {
        return Ok((Vec::new(), Vec::new(), Vec::new()));
    };
    let shapes = drawing::extract_shape_blocks(&drawing_xml);
    let mut items = drawing::parse_anchor_items(&drawing_xml);
    for it in items.iter_mut() {
        drawing::size_two_cell(it, columns, rows, default_col, default_row);
    }
    if items.is_empty() {
        return Ok((Vec::new(), Vec::new(), shapes));
    }

    let draw_rels_name = format!(
        "xl/drawings/_rels/{}.rels",
        drawing::basename(&drawing_part)
    );
    let draw_rels = match read_part(zip, &draw_rels_name)? {
        Some(x) => parse_rels(&x),
        None => HashMap::new(),
    };

    let mut images = Vec::new();
    let mut charts = Vec::new();
    for item in &items {
        if let Some(rid) = &item.embed {
            if let Some(target) = draw_rels.get(rid) {
                let part = drawing::resolve_part("xl/drawings", target);
                let src = format!("xl/media/{}", drawing::basename(&part));
                images.push(drawing::anchor_image(item, src));
            }
        } else if let Some(rid) = &item.chart {
            if let Some(target) = draw_rels.get(rid) {
                let part = drawing::resolve_part("xl/drawings", target);
                if let Some(cxml) = read_part(zip, &part)? {
                    if let Some(mut c) = drawing::parse_chart(&cxml) {
                        drawing::apply_anchor_to_chart(&mut c, item);
                        c.snapshot = crate::model::sheet::RawXml::from(&c.structured_json());
                        charts.push(c);
                    }
                }
            }
        }
    }
    Ok((images, charts, shapes))
}

/// 解析关系文件，返回 Id -> Type（用于区分 chartsheet 等）。
fn parse_rel_types(xml: &str) -> HashMap<String, String> {
    let mut reader = Reader::from_str(xml);
    let mut map = HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if e.local_name().as_ref() == b"Relationship" =>
            {
                let mut id = None;
                let mut ty = None;
                for a in e.attributes().filter_map(|a| a.ok()) {
                    let v = a
                        .unescape_value()
                        .map(|v| v.into_owned())
                        .unwrap_or_default();
                    match a.key.local_name().as_ref() {
                        b"Id" => id = Some(v),
                        b"Type" => ty = Some(v),
                        _ => {}
                    }
                }
                if let (Some(id), Some(ty)) = (id, ty) {
                    map.insert(id, ty);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    map
}

fn parse_rels(xml: &str) -> HashMap<String, String> {
    let mut reader = Reader::from_str(xml);
    let mut map = HashMap::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if e.local_name().as_ref() == b"Relationship" =>
            {
                let mut id = None;
                let mut target = None;
                for a in e.attributes().filter_map(|a| a.ok()) {
                    let v = a
                        .unescape_value()
                        .map(|v| v.into_owned())
                        .unwrap_or_default();
                    match a.key.local_name().as_ref() {
                        b"Id" => id = Some(v),
                        b"Target" => target = Some(v),
                        _ => {}
                    }
                }
                if let (Some(id), Some(target)) = (id, target) {
                    map.insert(id, target);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    map
}

struct SheetEntry {
    name: String,
    sheet_id: u32,
    hidden: bool,
    rid: String,
}

struct WbInfo {
    date1904: bool,
    active_tab: Option<u32>,
    sheets: Vec<SheetEntry>,
    named_ranges: Vec<NamedRange>,
    /// (name, localSheetId, formula) for reserved `_xlnm.*` names.
    reserved: Vec<(String, Option<usize>, String)>,
}

fn parse_workbook_xml(xml: &str) -> Result<WbInfo> {
    let mut reader = Reader::from_str(xml);
    let mut info = WbInfo {
        date1904: false,
        active_tab: None,
        sheets: Vec::new(),
        named_ranges: Vec::new(),
        reserved: Vec::new(),
    };
    let mut in_defined = false;
    let mut cur_defined: Option<String> = None;
    let mut cur_local: Option<usize> = None;
    let mut cur_text = String::new();
    loop {
        match reader.read_event()? {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.local_name().as_ref().to_vec();
                let mut attrs: HashMap<String, String> = HashMap::new();
                for a in e.attributes().filter_map(|a| a.ok()) {
                    attrs.insert(
                        String::from_utf8_lossy(a.key.local_name().as_ref()).to_string(),
                        a.unescape_value()
                            .map(|v| v.into_owned())
                            .unwrap_or_default(),
                    );
                }
                match name.as_slice() {
                    b"workbookPr" => {
                        info.date1904 = matches!(
                            attrs.get("date1904").map(|s| s.as_str()),
                            Some("1") | Some("true")
                        );
                    }
                    b"workbookView" => {
                        info.active_tab = attrs.get("activeTab").and_then(|v| v.parse().ok());
                    }
                    b"sheet" => {
                        info.sheets.push(SheetEntry {
                            name: attrs.get("name").cloned().unwrap_or_default(),
                            sheet_id: attrs
                                .get("sheetId")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(info.sheets.len() as u32 + 1),
                            hidden: matches!(
                                attrs.get("state").map(|s| s.as_str()),
                                Some("hidden") | Some("veryHidden")
                            ),
                            rid: attrs.get("id").cloned().unwrap_or_default(),
                        });
                    }
                    b"definedName" => {
                        in_defined = true;
                        cur_defined = attrs.get("name").cloned();
                        cur_local = attrs.get("localSheetId").and_then(|v| v.parse().ok());
                        cur_text.clear();
                    }
                    _ => {}
                }
            }
            Event::Text(e) if in_defined => {
                cur_text.push_str(&e.unescape()?);
            }
            Event::End(e) if e.local_name().as_ref() == b"definedName" => {
                if let Some(name) = cur_defined.take() {
                    if name.starts_with("_xlnm.") {
                        info.reserved
                            .push((name, cur_local.take(), cur_text.clone()));
                    } else if !info.named_ranges.iter().any(|n| n.name == name) {
                        info.named_ranges.push(NamedRange {
                            name,
                            reference: cur_text.clone(),
                        });
                    }
                }
                cur_local = None;
                in_defined = false;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(info)
}

fn parse_core(xml: &str) -> Option<Meta> {
    let mut reader = Reader::from_str(xml);
    let mut meta = Meta::default();
    let mut cur: Option<Vec<u8>> = None;
    let mut buf = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let n = e.local_name().as_ref().to_vec();
                if n == b"title" || n == b"creator" {
                    cur = Some(n);
                    buf.clear();
                }
            }
            Ok(Event::Text(e)) if cur.is_some() => {
                if let Ok(t) = e.unescape() {
                    buf.push_str(&t);
                }
            }
            Ok(Event::End(e)) => {
                if let Some(n) = cur.take() {
                    if e.local_name().as_ref() == n.as_slice() {
                        match n.as_slice() {
                            b"title" => meta.title = Some(buf.clone()),
                            b"creator" => meta.author = Some(buf.clone()),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if meta.title.is_none() && meta.author.is_none() {
        None
    } else {
        Some(meta)
    }
}

/// Read a sheet's table parts.
fn read_tables(zip: &mut ZipArchive<File>, rels: &HashMap<String, String>) -> Result<Vec<Table>> {
    let mut items: Vec<(u64, String)> = Vec::new();
    for target in rels.values() {
        if !target.contains("tables/") {
            continue;
        }
        let part = drawing::resolve_part("xl/worksheets", target);
        if let Some(x) = read_part(zip, &part)? {
            items.push((table_part_order(target), x));
        }
    }
    // 关系存放于 HashMap，顺序不定；按部件名中的序号还原文档顺序，
    // 否则表格编号/样式会错配到不同的引用区域。
    items.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let xmls: Vec<String> = items.into_iter().map(|(_, x)| x).collect();
    Ok(table::parse_tables(&xmls))
}

/// 从 `../tables/table12.xml` 提取序号（12），用于稳定排序。
fn table_part_order(target: &str) -> u64 {
    let stem = target.rsplit('/').next().unwrap_or(target);
    let stem = stem.strip_suffix(".xml").unwrap_or(stem);
    let digits: String = stem
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    digits.parse().unwrap_or(0)
}

/// Attach reserved `_xlnm.Print_Area` / `_xlnm.Print_Titles` to their sheets.
fn apply_reserved_names(sheets: &mut [Sheet], reserved: &[(String, Option<usize>, String)]) {
    for (name, local, formula) in reserved {
        let Some(id) = local else { continue };
        if *id >= sheets.len() {
            continue;
        }
        let sheet = &mut sheets[*id];
        match name.as_str() {
            "_xlnm.Print_Area" => sheet.print_area = Some(strip_ref(formula)),
            "_xlnm.Print_Titles" => {
                for part in formula.split(',') {
                    let r = strip_ref(part);
                    if r.chars()
                        .next()
                        .map(|c| c.is_ascii_digit())
                        .unwrap_or(false)
                    {
                        sheet.print_title_rows = Some(r);
                    } else if !r.is_empty() {
                        sheet.print_title_cols = Some(r);
                    }
                }
            }
            _ => {}
        }
    }
}

/// "Sheet1!$A$1:$C$20" -> "A1:C20".
fn strip_ref(f: &str) -> String {
    let r = f.split('!').next_back().unwrap_or(f);
    r.replace('$', "")
}

/// Read a sheet's legacy comments part.
fn read_comments(
    zip: &mut ZipArchive<File>,
    rels: &HashMap<String, String>,
) -> Result<Vec<(String, String)>> {
    for target in rels.values() {
        if !target.contains("comments") {
            continue;
        }
        let part = drawing::resolve_part("xl/worksheets", target);
        if let Some(x) = read_part(zip, &part)? {
            return Ok(comment::parse_comments(&x));
        }
    }
    Ok(Vec::new())
}

fn apply_comments(rows: &mut Vec<Row>, comments: &[(String, String)]) {
    use crate::utils::a1::parse_cell_ref;
    for (reference, text) in comments {
        let mut placed = false;
        for row in rows.iter_mut() {
            for cell in &mut row.cells {
                if cell.reference.as_deref() == Some(reference.as_str()) {
                    cell.comment = Some(text.clone());
                    placed = true;
                }
            }
        }
        if placed {
            continue;
        }
        let (col, rnum) = match parse_cell_ref(reference) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let ridx = match rows.iter().position(|r| r.index == Some(rnum)) {
            Some(i) => i,
            None => {
                let pos = rows
                    .iter()
                    .position(|r| r.index.is_some_and(|x| x > rnum))
                    .unwrap_or(rows.len());
                rows.insert(
                    pos,
                    Row {
                        index: Some(rnum),
                        ..Default::default()
                    },
                );
                pos
            }
        };
        let cells = &mut rows[ridx].cells;
        let cpos = cells
            .iter()
            .position(|c| {
                c.reference
                    .as_deref()
                    .and_then(|r| parse_cell_ref(r).ok())
                    .map(|(c, _)| c)
                    .unwrap_or(u32::MAX)
                    > col
            })
            .unwrap_or(cells.len());
        cells.insert(
            cpos,
            Cell {
                reference: Some(reference.clone()),
                comment: Some(text.clone()),
                ..Default::default()
            },
        );
    }
}
