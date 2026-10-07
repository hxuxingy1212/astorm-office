//! Read (view/get) and write (set/add/remove) operations over the model.

use serde_json::{json, Number, Value};

use json2xlsx::model::cell::{Cell, CellStyleRef};
use json2xlsx::model::style::{Alignment, Border, Font, StyleDef};
use json2xlsx::model::workbook::cell_display;
use json2xlsx::model::{Chart, Image, Row, Shape, Sheet, Size, Workbook};
use json2xlsx::utils::a1::{index_to_col, make_cell_ref, parse_cell_ref, parse_range};
use office_core::{suggest, CliError};

use crate::capabilities as caps;

/// 本模块统一错误类型（携带自愈建议）
type Result<T> = std::result::Result<T, CliError>;

use crate::path::{Path as ExPath, SheetSel, Target};

pub fn resolve_sheet(wb: &Workbook, sel: &SheetSel) -> Result<usize> {
    match sel {
        SheetSel::Index(i) => {
            if *i >= 1 && *i <= wb.sheets.len() {
                Ok(i - 1)
            } else {
                Err(CliError::new(format!(
                    "工作表索引越界: {i}（共 {} 个）",
                    wb.sheets.len()
                )))
            }
        }
        SheetSel::Name(n) => wb
            .sheets
            .iter()
            .position(|s| &s.name == n)
            .ok_or_else(|| CliError::new(format!("未找到工作表: {n}"))),
        SheetSel::Append => Err(
            CliError::new("该操作需要具体工作表：/sheet[1] 或 /sheet[名字]")
                .suggest("裸 /sheet 仅用于 add sheet（追加新表）"),
        ),
    }
}

pub fn parse_props(list: &[String]) -> Vec<(String, String)> {
    list.iter()
        .filter_map(|p| {
            p.split_once('=')
                .map(|(k, v)| (k.trim().to_string(), v.to_string()))
        })
        .collect()
}

fn typed(s: &str) -> Value {
    match s {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => {
            if let Ok(i) = s.parse::<i64>() {
                Value::Number(Number::from(i))
            } else if let Ok(f) = s.parse::<f64>() {
                Number::from_f64(f)
                    .map(Value::Number)
                    .unwrap_or_else(|| Value::String(s.into()))
            } else if (s.starts_with('[') || s.starts_with('{'))
                && serde_json::from_str::<Value>(s).is_ok()
            {
                serde_json::from_str(s).unwrap()
            } else {
                Value::String(s.to_string())
            }
        }
    }
}

// ---------------------------------------------------------------- view ----

pub fn view_values(wb: &Workbook, path: &ExPath) -> Result<Value> {
    let si = resolve_sheet(wb, &path.sheet)?;
    let sheet = &wb.sheets[si];
    match &path.target {
        None => Ok(sheet_values(sheet)),
        Some(Target::Cell(reference)) => {
            let cell = find_cell(sheet, reference)?
                .ok_or_else(|| CliError::new(format!("单元格不存在: {reference}")))?;
            Ok(cell_json(cell))
        }
        Some(Target::Range(r)) => {
            let ((c1, r1), (c2, r2)) = parse_range(r)?;
            let mut out = Vec::new();
            for row in &sheet.rows {
                let ri = row.index.unwrap_or(0);
                if ri < r1 || ri > r2 {
                    continue;
                }
                for cell in &row.cells {
                    if let Some((c, _)) = cell
                        .reference
                        .as_deref()
                        .and_then(|x| parse_cell_ref(x).ok())
                    {
                        if c >= c1 && c <= c2 {
                            out.push(cell_json(cell));
                        }
                    }
                }
            }
            Ok(Value::Array(out))
        }
        Some(_) => Ok(sheet_values(sheet)),
    }
}

fn sheet_values(sheet: &Sheet) -> Value {
    let mut rows: Vec<&Row> = sheet.rows.iter().collect();
    rows.sort_by_key(|r| r.index.unwrap_or(0));
    let rows_json: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "index": row.index,
                "cells": row.cells.iter().map(cell_json).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({ "sheet": sheet.name, "rows": rows_json })
}

fn cell_json(cell: &Cell) -> Value {
    json!({
        "ref": cell.reference,
        "type": cell.cell_type,
        "value": cell.value,
        "formula": cell.formula,
        "number_format": cell.number_format,
        "display": cell_display(cell),
    })
}

pub fn view_structure(wb: &Workbook, path: &ExPath) -> Result<Value> {
    let si = resolve_sheet(wb, &path.sheet)?;
    let sheet = &wb.sheets[si];
    Ok(json!({
        "name": sheet.name,
        "tab_color": sheet.tab_color,
        "hidden": sheet.hidden,
        "freeze": sheet.freeze,
        "direction": sheet.direction,
        "auto_filter": sheet.auto_filter,
        "columns": sheet.columns.iter().enumerate().map(|(i, c)| json!({
            "col": index_to_col(i as u32 + 1),
            "width": c.width,
            "hidden": c.hidden,
        })).collect::<Vec<_>>(),
        "merges": sheet.merges,
        "row_count": sheet.rows.len(),
        "protect": sheet.protect,
        "gridlines": sheet.gridlines,
        "headings": sheet.headings,
        "zoom": sheet.zoom,
        "show_formulas": sheet.show_formulas,
        "tab_selected": sheet.tab_selected,
        "filter_columns": sheet.filter_columns.len(),
        "comments": sheet
            .rows
            .iter()
            .flat_map(|r| &r.cells)
            .filter(|c| c.comment.is_some())
            .count(),
        "validations": sheet.validations.iter().map(|v| json!({
            "range": v.range,
            "type": v.validation_type,
            "operator": v.operator,
        })).collect::<Vec<_>>(),
        "conditional_formats": sheet.conditional_formats.iter().map(|c| json!({
            "range": c.range,
            "type": c.rule_type,
            "operator": c.operator,
        })).collect::<Vec<_>>(),
        "print": sheet.print,
        "tables": sheet.tables.iter().map(|t| json!({
            "range": t.range,
            "name": t.name,
            "style": t.style,
        })).collect::<Vec<_>>(),
        "images": sheet.images.len(),
        "charts": sheet.charts.iter().map(|c| json!({
            "type": c.chart_type,
            "data_range": c.data_range,
            "categories": c.categories,
            "title": c.title,
        })).collect::<Vec<_>>(),
    }))
}

// ---------------------------------------------------------------- get -----

pub fn get(wb: &Workbook, path: &ExPath) -> Result<Value> {
    let si = resolve_sheet(wb, &path.sheet)?;
    let sheet = &wb.sheets[si];
    match &path.target {
        None => view_structure(wb, path),
        Some(Target::Cell(reference)) => {
            let cell = find_cell(sheet, reference)?
                .ok_or_else(|| CliError::new(format!("单元格不存在: {reference}")))?;
            Ok(serde_json::to_value(cell)?)
        }
        Some(Target::Row(n)) => {
            let row = sheet.rows.iter().find(|r| r.index == Some(*n));
            Ok(serde_json::to_value(
                row.ok_or_else(|| CliError::new(format!("行不存在: {n}")))?,
            )?)
        }
        Some(Target::Range(r)) => {
            let ((c1, r1), (c2, r2)) = parse_range(r)?;
            let mut cells = Vec::new();
            for row in &sheet.rows {
                let ri = row.index.unwrap_or(0);
                if ri < r1 || ri > r2 {
                    continue;
                }
                for cell in &row.cells {
                    if let Some((c, _)) = cell
                        .reference
                        .as_deref()
                        .and_then(|x| parse_cell_ref(x).ok())
                    {
                        if c >= c1 && c <= c2 {
                            cells.push(serde_json::to_value(cell)?);
                        }
                    }
                }
            }
            Ok(Value::Array(cells))
        }
        _ => view_structure(wb, path),
    }
}

// ---------------------------------------------------------------- set -----

pub fn set(wb: &mut Workbook, path: &ExPath, props: &[(String, String)]) -> Result<Value> {
    let si = resolve_sheet(wb, &path.sheet)?;
    match &path.target {
        None => {
            set_sheet_props(&mut wb.sheets[si], props)?;
            Ok(json!({ "sheet": wb.sheets[si].name }))
        }
        Some(Target::Cell(reference)) => {
            let (col, row) = parse_cell_ref(reference)?;
            let style_map = wb.styles.clone();
            let cell = get_or_create_cell(&mut wb.sheets[si], col, row, &style_map);
            apply_cell_props(cell, props)?;
            Ok(cell_json(cell))
        }
        Some(Target::Range(r)) => {
            let ((c1, r1), (c2, r2)) = parse_range(r)?;
            let style_map = wb.styles.clone();
            let style_props: Vec<(String, String)> = props
                .iter()
                .filter(|(k, _)| k != "merge")
                .cloned()
                .collect();
            for p in props {
                if p.0 == "merge" {
                    let range_ref =
                        format!("{}{}:{}{}", index_to_col(c1), r1, index_to_col(c2), r2);
                    let sheet = &mut wb.sheets[si];
                    match p.1.as_str() {
                        "true" if !sheet.merges.contains(&range_ref) => {
                            sheet.merges.push(range_ref.clone())
                        }
                        "false" | "none" => sheet.merges.retain(|m| m != &range_ref),
                        _ => {}
                    }
                }
            }
            for r in r1..=r2 {
                for c in c1..=c2 {
                    let cell = get_or_create_cell(&mut wb.sheets[si], c, r, &style_map);
                    apply_cell_props(cell, &style_props)?;
                }
            }
            Ok(json!({ "range": r }))
        }
        Some(Target::Row(n)) => {
            set_row_props(&mut wb.sheets[si], *n, props)?;
            Ok(json!({ "row": n }))
        }
        Some(Target::Col(c)) => {
            let idx = col_index(c)?;
            set_col_props(&mut wb.sheets[si], idx, props)?;
            Ok(json!({ "col": c }))
        }
        Some(_) => Err(CliError::new("该元素暂不支持 set")),
    }
}

fn set_sheet_props(sheet: &mut Sheet, props: &[(String, String)]) -> Result<()> {
    for (k, v) in props {
        match k.as_str() {
            "name" => sheet.name = v.clone(),
            "tab_color" => sheet.tab_color = Some(v.clone()),
            "hidden" => sheet.hidden = v == "true",
            "freeze" => sheet.freeze = Some(v.clone()),
            "direction" => sheet.direction = Some(v.clone()),
            "auto_filter" => sheet.auto_filter = Some(v.clone()),
            "default_col_width" => sheet.default_col_width = v.parse().ok(),
            "default_row_height" => sheet.default_row_height = v.parse().ok(),
            "base_col_width" => sheet.base_col_width = v.parse().ok(),
            "gridlines" => sheet.gridlines = Some(v == "true"),
            "headings" => sheet.headings = Some(v == "true"),
            "zoom" => sheet.zoom = v.parse().ok(),
            "protect" => sheet.protect = v == "true",
            "show_formulas" => sheet.show_formulas = v == "true",
            "merges" => {
                // dump 回放：JSON 数组；也兼容逗号分隔
                let t = v.trim();
                if t.starts_with('[') {
                    sheet.merges = serde_json::from_str::<Vec<String>>(t)
                        .map_err(|e| CliError::new(format!("merges JSON 解析失败: {e}")))?;
                } else if t.is_empty() || t == "none" {
                    sheet.merges.clear();
                } else {
                    sheet.merges = t
                        .split(',')
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty())
                        .collect();
                }
            }
            other => {
                return Err(
                    CliError::new(format!("工作表不支持的属性: {other}")).suggest(
                        suggest::did_you_mean(other, &caps::prop_names(caps::SHEET_PROPS), 12),
                    ),
                )
            }
        }
    }
    Ok(())
}

fn apply_cell_props(cell: &mut Cell, props: &[(String, String)]) -> Result<()> {
    for (k, v) in props {
        match k.as_str() {
            "value" => cell.value = Some(typed(v)),
            "formula" => {
                cell.formula = Some(v.trim_start_matches('=').to_string());
                if cell.cell_type.is_none() {
                    cell.cell_type = Some("number".into());
                }
            }
            "type" => cell.cell_type = Some(v.clone()),
            "number_format" | "numfmt" | "format" => {
                cell.number_format = if v == "none" { None } else { Some(v.clone()) }
            }
            "link" | "url" => cell.link = Some(v.clone()),
            "link_tooltip" | "tooltip" => cell.link_tooltip = Some(v.clone()),
            "comment" => cell.comment = Some(v.clone()),
            "style" => {
                // dump 回放：命名样式（字符串）或内联样式（JSON 对象）
                let t = v.trim();
                if t.starts_with('{') {
                    let def: StyleDef = serde_json::from_str(t)
                        .map_err(|e| CliError::new(format!("style JSON 解析失败: {e}")))?;
                    cell.style = Some(CellStyleRef::Inline(def));
                } else if t == "none" {
                    cell.style = None;
                } else {
                    cell.style = Some(CellStyleRef::Named(t.to_string()));
                }
            }
            _ => apply_style_prop(cell, k, v)?,
        }
    }
    Ok(())
}

fn style_of(cell: &mut Cell) -> StyleDef {
    match &cell.style {
        Some(CellStyleRef::Inline(s)) => s.clone(),
        Some(CellStyleRef::Named(_)) | None => StyleDef::default(),
    }
}

fn apply_style_prop(cell: &mut Cell, key: &str, value: &str) -> Result<()> {
    let mut s = style_of(cell);
    let font = s.font.get_or_insert_with(Font::default);
    match key {
        "bold" | "font.bold" => font.bold = value == "true",
        "italic" | "font.italic" => font.italic = value == "true",
        "strike" | "font.strike" => font.strike = value == "true",
        "underline" | "font.underline" => {
            font.underline = if value == "none" {
                None
            } else {
                Some(value.to_string())
            }
        }
        "font" | "font.name" | "fontname" => font.name = Some(value.to_string()),
        "size" | "font.size" | "fontsize" => {
            font.size = value.parse().ok();
        }
        "color" | "font.color" => font.color = Some(value.to_string()),
        "fill" | "bgcolor" => s.fill = Some(value.to_string()),
        "halign" | "alignment.horizontal" => {
            let a = s.alignment.get_or_insert_with(Alignment::default);
            a.horizontal = Some(value.to_string());
        }
        "valign" | "alignment.vertical" => {
            let a = s.alignment.get_or_insert_with(Alignment::default);
            a.vertical = Some(value.to_string());
        }
        "wrap" | "wrapText" | "alignment.wrap_text" => {
            let a = s.alignment.get_or_insert_with(Alignment::default);
            a.wrap_text = value == "true";
        }
        "border" | "border.all" => {
            let b = s.border.get_or_insert_with(Border::default);
            b.all = Some(value.to_string());
        }
        "border.color" => {
            let b = s.border.get_or_insert_with(Border::default);
            b.color = Some(value.to_string());
        }
        "border.top" => {
            let b = s.border.get_or_insert_with(Border::default);
            b.top = Some(value.to_string());
        }
        "border.bottom" => {
            let b = s.border.get_or_insert_with(Border::default);
            b.bottom = Some(value.to_string());
        }
        "border.left" => {
            let b = s.border.get_or_insert_with(Border::default);
            b.left = Some(value.to_string());
        }
        "border.right" => {
            let b = s.border.get_or_insert_with(Border::default);
            b.right = Some(value.to_string());
        }
        "locked" => s.locked = Some(value == "true"),
        other => {
            return Err(
                CliError::new(format!("单元格不支持的属性: {other}")).suggest(
                    suggest::did_you_mean(other, &caps::prop_names(caps::CELL_PROPS), 16),
                ),
            )
        }
    }
    if s.font.as_ref().map(|f| f.is_empty()).unwrap_or(false) {
        s.font = None;
    }
    cell.style = Some(CellStyleRef::Inline(s));
    Ok(())
}

fn set_row_props(sheet: &mut Sheet, n: u32, props: &[(String, String)]) -> Result<()> {
    let row = get_or_create_row(sheet, n);
    for (k, v) in props {
        match k.as_str() {
            "height" => row.height = v.parse().ok(),
            other => {
                return Err(CliError::new(format!("行不支持的属性: {other}")).suggest(
                    suggest::did_you_mean(other, &caps::prop_names(caps::ROW_PROPS), 8),
                ))
            }
        }
    }
    Ok(())
}

fn set_col_props(sheet: &mut Sheet, idx: usize, props: &[(String, String)]) -> Result<()> {
    while sheet.columns.len() < idx {
        sheet.columns.push(Default::default());
    }
    let col = &mut sheet.columns[idx - 1];
    for (k, v) in props {
        match k.as_str() {
            "width" => col.width = v.parse().ok(),
            "hidden" => col.hidden = v == "true",
            other => {
                return Err(CliError::new(format!("列不支持的属性: {other}")).suggest(
                    suggest::did_you_mean(other, &caps::prop_names(caps::COL_PROPS), 8),
                ))
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- add -----

pub fn add(
    wb: &mut Workbook,
    path: &ExPath,
    etype: &str,
    props: &[(String, String)],
) -> Result<Value> {
    let getp = |k: &str| props.iter().find(|(pk, _)| pk == k).map(|(_, v)| v.clone());
    match etype {
        "sheet" => {
            let name = getp("name").unwrap_or_else(|| format!("Sheet{}", wb.sheets.len() + 1));
            // 同名幂等：Excel 不允许重名，dump 回放时不重复建表
            if let Some(i) = wb.sheets.iter().position(|s| s.name == name) {
                return Ok(
                    json!({ "added": "sheet", "name": name, "existing": true, "index": i + 1 }),
                );
            }
            wb.sheets.push(Sheet {
                name: name.clone(),
                ..Default::default()
            });
            Ok(json!({ "added": "sheet", "name": name }))
        }
        "row" => {
            let si = resolve_sheet(wb, &path.sheet)?;
            let n = getp("index")
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| {
                    wb.sheets[si]
                        .rows
                        .iter()
                        .filter_map(|r| r.index)
                        .max()
                        .unwrap_or(0)
                        + 1
                });
            get_or_create_row(&mut wb.sheets[si], n);
            Ok(json!({ "added": "row", "index": n }))
        }
        "cell" => {
            let si = resolve_sheet(wb, &path.sheet)?;
            let reference = getp("ref")
                .or_else(|| match &path.target {
                    Some(Target::Cell(r)) => Some(r.clone()),
                    _ => None,
                })
                .ok_or_else(|| CliError::new("add cell 需要 --prop ref=A1"))?;
            let (col, row) = parse_cell_ref(&reference)?;
            let style_map = wb.styles.clone();
            let cell = get_or_create_cell(&mut wb.sheets[si], col, row, &style_map);
            // ref 只用于定位，不参与样式/值写入（dump 回放会带上 ref）
            let body: Vec<(String, String)> =
                props.iter().filter(|(k, _)| k != "ref").cloned().collect();
            apply_cell_props(cell, &body)?;
            Ok(cell_json(cell))
        }
        "chart" => {
            let si = resolve_sheet(wb, &path.sheet)?;
            // dump 回放优先：props 即 Chart 序列化结果，整体反序列化保真
            if let Ok(chart) = serde_json::from_value::<Chart>(Value::Object(props_map(props))) {
                wb.sheets[si].charts.push(chart);
                return Ok(json!({ "added": "chart", "sheet": wb.sheets[si].name }));
            }
            let size = size_from(&getp);
            let chart = Chart {
                chart_type: getp("type")
                    .or_else(|| getp("chart_type"))
                    .unwrap_or_else(|| "column".into()),
                data_range: getp("data_range").or_else(|| getp("range")),
                categories: getp("categories"),
                title: getp("title"),
                legend: getp("legend"),
                anchor: getp("anchor"),
                size,
                ..Default::default()
            };
            wb.sheets[si].charts.push(chart);
            Ok(json!({ "added": "chart", "sheet": wb.sheets[si].name }))
        }
        "image" => {
            let si = resolve_sheet(wb, &path.sheet)?;
            if let Ok(img) = serde_json::from_value::<Image>(Value::Object(props_map(props))) {
                wb.sheets[si].images.push(img);
                return Ok(json!({ "added": "image", "sheet": wb.sheets[si].name }));
            }
            let src = getp("src").ok_or_else(|| CliError::new("add image 需要 --prop src=路径"))?;
            let size = size_from(&getp);
            wb.sheets[si].images.push(Image {
                src,
                anchor: getp("anchor"),
                size,
                offset: None,
            });
            Ok(json!({ "added": "image", "sheet": wb.sheets[si].name }))
        }
        "table" => {
            use json2xlsx::model::Table;
            let si = resolve_sheet(wb, &path.sheet)?;
            if let Ok(t) = serde_json::from_value::<Table>(Value::Object(props_map(props))) {
                wb.sheets[si].tables.push(t);
                return Ok(json!({ "added": "table", "sheet": wb.sheets[si].name }));
            }
            let range = match getp("range").or_else(|| getp("ref")) {
                Some(r) => r,
                None => json2xlsx::detect::detect_range(&wb.sheets[si])
                    .ok_or_else(|| CliError::new("无法自动识别数据区（可用 --prop range=）"))?,
            };
            let columns = json2xlsx::detect::header_names(&wb.sheets[si], &range);
            wb.sheets[si].tables.push(Table {
                range,
                name: getp("name"),
                style: getp("style").or_else(|| Some("TableStyleMedium9".into())),
                show_header_row: true,
                show_banded_rows: true,
                columns,
                ..Default::default()
            });
            Ok(json!({ "added": "table", "sheet": wb.sheets[si].name }))
        }
        "shape" => {
            let si = resolve_sheet(wb, &path.sheet)?;
            if let Ok(sh) = serde_json::from_value::<Shape>(Value::Object(props_map(props))) {
                wb.sheets[si].drawing_shapes.push(sh);
                return Ok(json!({ "added": "shape", "sheet": wb.sheets[si].name }));
            }
            let size = size_from(&getp);
            wb.sheets[si].drawing_shapes.push(Shape {
                geometry: getp("geometry").or_else(|| getp("preset")),
                anchor: getp("anchor"),
                size,
                fill: getp("fill"),
                gradient: getp("gradient").or_else(|| getp("gradientFill")),
                line_color: getp("line").or_else(|| getp("line_color")),
                line_width: getp("line_width").and_then(|v| v.parse().ok()),
                rotation: getp("rotation").and_then(|v| v.parse().ok()),
                flip_h: matches!(
                    getp("flip").as_deref(),
                    Some("h") | Some("hv") | Some("vh") | Some("both")
                ),
                flip_v: matches!(
                    getp("flip").as_deref(),
                    Some("v") | Some("hv") | Some("vh") | Some("both")
                ),
                text: getp("text"),
                ..Default::default()
            });
            Ok(json!({ "added": "shape", "sheet": wb.sheets[si].name }))
        }
        other => Err(CliError::new(format!("add 不支持类型: {other}")).suggest(
            suggest::did_you_mean(
                other,
                &caps::ADD_TYPES.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
                8,
            ),
        )),
    }
}

// ------------------------------------------------------------- remove -----

pub fn remove(wb: &mut Workbook, path: &ExPath) -> Result<Value> {
    let si = resolve_sheet(wb, &path.sheet)?;
    match &path.target {
        None => {
            if wb.sheets.len() <= 1 {
                return Err(CliError::new("不能删除唯一的工作表"));
            }
            let name = wb.sheets.remove(si).name;
            Ok(json!({ "removed": "sheet", "name": name }))
        }
        Some(Target::Cell(reference)) => {
            let (col, row) = parse_cell_ref(reference)?;
            let sheet = &mut wb.sheets[si];
            if let Some(r) = sheet.rows.iter_mut().find(|r| r.index == Some(row)) {
                r.cells.retain(|c| {
                    if c.reference.as_deref() == Some(reference.as_str()) {
                        return false;
                    }
                    match c.reference.as_deref().and_then(|x| parse_cell_ref(x).ok()) {
                        Some((cc, _)) => cc != col,
                        None => true,
                    }
                });
            }
            Ok(json!({ "removed": "cell", "ref": reference }))
        }
        Some(Target::Row(n)) => {
            wb.sheets[si].rows.retain(|r| r.index != Some(*n));
            Ok(json!({ "removed": "row", "index": n }))
        }
        Some(Target::Merge(n)) => {
            let sheet = &mut wb.sheets[si];
            if *n >= 1 && *n <= sheet.merges.len() {
                let m = sheet.merges.remove(n - 1);
                Ok(json!({ "removed": "merge", "ref": m }))
            } else {
                Err(CliError::new(format!("合并区索引越界: {n}")))
            }
        }
        Some(_) => Err(CliError::new("该元素暂不支持 remove")),
    }
}

// ------------------------------------------------------------- helpers ----

fn find_cell<'a>(sheet: &'a Sheet, reference: &str) -> Result<Option<&'a Cell>> {
    let (col, row) = parse_cell_ref(reference)?;
    Ok(sheet
        .rows
        .iter()
        .filter(|r| r.index == Some(row))
        .flat_map(|r| r.cells.iter())
        .find(|c| {
            c.reference
                .as_deref()
                .and_then(|x| parse_cell_ref(x).ok())
                .map(|(cc, _)| cc == col)
                .unwrap_or(false)
        }))
}

fn get_or_create_row(sheet: &mut Sheet, n: u32) -> &mut Row {
    if let Some(pos) = sheet.rows.iter().position(|r| r.index == Some(n)) {
        return &mut sheet.rows[pos];
    }
    sheet.rows.push(Row {
        index: Some(n),
        ..Default::default()
    });
    let pos = sheet.rows.len() - 1;
    &mut sheet.rows[pos]
}

fn get_or_create_cell<'a>(
    sheet: &'a mut Sheet,
    col: u32,
    row: u32,
    _style_map: &std::collections::HashMap<String, StyleDef>,
) -> &'a mut Cell {
    let r = get_or_create_row(sheet, row);
    if let Some(pos) = r.cells.iter().position(|c| {
        c.reference
            .as_deref()
            .and_then(|x| parse_cell_ref(x).ok())
            .map(|(cc, _)| cc == col)
            .unwrap_or(false)
    }) {
        return &mut r.cells[pos];
    }
    r.cells.push(Cell {
        reference: Some(make_cell_ref(col, row)),
        ..Default::default()
    });
    let pos = r.cells.len() - 1;
    &mut r.cells[pos]
}

fn col_index(c: &str) -> Result<usize> {
    if c.chars().all(|ch| ch.is_ascii_digit()) {
        let i: usize = c
            .parse()
            .map_err(|_| CliError::new(format!("非法列索引: {c}")))?;
        if i == 0 {
            return Err(CliError::new("列索引从 1 开始"));
        }
        Ok(i)
    } else {
        Ok(json2xlsx::utils::a1::col_to_index(c)? as usize)
    }
}

/// batch/dump 回放：props 里的 JSON 对象/数组/布尔恢复为带类型值，
/// 其余保持字符串（避免数字串被改写，如 value=007）。
fn prop_value(v: &str) -> Value {
    let t = v.trim();
    if t.starts_with('{') || t.starts_with('[') {
        if let Ok(j) = serde_json::from_str::<Value>(t) {
            return j;
        }
    }
    match t {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => Value::String(v.to_string()),
    }
}

/// props → JSON 对象（供整体反序列化到模型类型）
fn props_map(props: &[(String, String)]) -> serde_json::Map<String, Value> {
    props
        .iter()
        .map(|(k, v)| (k.clone(), prop_value(v)))
        .collect()
}

fn size_from(getp: &dyn Fn(&str) -> Option<String>) -> Option<Size> {
    let w = getp("w").and_then(|v| v.parse().ok());
    let h = getp("h").and_then(|v| v.parse().ok());
    match (w, h) {
        (Some(w), Some(h)) => Some(Size { w, h }),
        _ => None,
    }
}
