//! 工作表 / 工作簿 → Markdown（结构感知的纯文本视图）
//!
//! 供 `view <文件> [路径] text` 使用：已用区域渲染成 GFM 表格，
//! 默认有公式时显示 `=公式`；可选缓存值、公式与缓存值、数字格式显示值模式。
//! 整本工作簿时每张表一个 `## 表名` 小节。输出是**纯 Markdown**，不带注释标记。
//!
//! 设计取向：只表达结构（行列网格），不表达样式（字体/颜色/列宽/合并区属于
//! layout 视图）。超大区域（> 20000 格）自动退回逐行稀疏列表，避免生成
//! 百万行的表格。

use crate::model::sheet::Sheet;
use crate::model::workbook::Workbook;
use crate::utils::a1::{index_to_col, parse_cell_ref, parse_range};
use office_core::attachment::file_name_of;
use office_core::PayloadSinks;

/// Cell representation used by Markdown text views. No mode recalculates formulas.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ValueMode {
    #[default]
    Formula,
    CachedValue,
    Both,
    Formatted,
}

/// Workbook context is required to resolve named styles and the 1904 date system.
#[derive(Debug, Clone, Copy, Default)]
pub struct TextOptions<'a> {
    pub value_mode: ValueMode,
    pub date1904: bool,
    pub styles: Option<&'a std::collections::BTreeMap<String, crate::model::style::StyleDef>>,
}

impl<'a> TextOptions<'a> {
    pub fn for_workbook(wb: &'a Workbook, value_mode: ValueMode) -> Self {
        Self {
            value_mode,
            date1904: wb.date1904,
            styles: Some(&wb.styles),
        }
    }
}

/// 超过这个格子数就不再画表格（稀疏列表更省）
const MAX_TABLE_CELLS: usize = 20_000;

/// 单张工作表 → Markdown（已用区域；空表返回空串）
///
/// 全表是主路径：**直接从 rows 拼**（不建 `(行,列) → String` 的 HashMap，
/// 也不给每个格子分配字符串），与 `window_from_grid` 的输出逐字节一致；
/// 只有超大区域（> 20000 格）才回落到稀疏形态那条老路。
pub fn sheet_markdown(sheet: &Sheet) -> String {
    sheet_markdown_with(sheet, PayloadSinks::default())
}

/// 单张工作表 → Markdown（带附件导出：OLE 对象落到指定目录，引用指向导出后的路径）
pub fn sheet_markdown_with(sheet: &Sheet, sinks: PayloadSinks<'_>) -> String {
    sheet_markdown_with_options(sheet, sinks, TextOptions::default())
}

/// Render a sheet with explicit formula/value semantics and workbook context.
pub fn sheet_markdown_with_options(
    sheet: &Sheet,
    sinks: PayloadSinks<'_>,
    options: TextOptions<'_>,
) -> String {
    let mut out = match used_range_direct(sheet) {
        Some((c1, r1, c2, r2)) => render_window(sheet, c1, r1, c2, r2, options),
        None => String::new(),
    };
    append_semantics(&mut out, sheet, sinks, None, options);
    // 内嵌对象（OLE）：只有带 --attachments（sink）时才渲染——默认输出保持不变
    if let Some(sink) = sinks.attachments {
        for (index, ole) in sheet.ole_objects.iter().enumerate() {
            let _source = sink.at_source(format!("ole[{}]", index + 1));
            let _anchor = ole
                .anchor
                .as_deref()
                .map(|cell| sink.at_source(format!("cell[{cell}]")));
            let name = file_name_of(&ole.src).to_string();
            let link = sink.link_for(&name, &ole.src);
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("[{name}]({link})（附件 · {}）", ole.prog_id));
        }
    }
    out
}

fn append_section(out: &mut String, text: &str) {
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(text);
}

fn md_label(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('\n', " ")
}

fn md_target(text: &str) -> String {
    text.replace(' ', "%20")
        .replace('(', "%28")
        .replace(')', "%29")
}

/// Preserve individual chart source formulas when the parsed model has collapsed
/// them into a bounding rectangle. Never read another sheet's coordinates locally.
fn chart_sources(chart: &crate::model::sheet::Chart) -> Vec<(&'static str, String)> {
    use quick_xml::{events::Event, Reader};
    let raw_is_current = chart
        .snapshot
        .0
        .as_ref()
        .is_none_or(|snapshot| snapshot == &chart.structured_json());
    if let Some(raw) = chart.raw.0.as_deref().filter(|_| raw_is_current) {
        let mut reader = Reader::from_str(raw);
        let mut kind = None;
        let mut sources = Vec::new();
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => match e.local_name().as_ref() {
                    b"cat" | b"xVal" => kind = Some("分类"),
                    b"val" | b"yVal" | b"bubbleSize" => kind = Some("数据"),
                    b"f" => {
                        if let Some(label) = kind {
                            if let Ok(text) = reader.read_text(e.name()) {
                                let text = quick_xml::escape::unescape(&text)
                                    .map(|value| value.into_owned())
                                    .unwrap_or_else(|_| text.into_owned());
                                sources.push((label, text));
                            }
                        }
                    }
                    _ => {}
                },
                Ok(Event::End(e))
                    if matches!(
                        e.local_name().as_ref(),
                        b"cat" | b"xVal" | b"val" | b"yVal" | b"bubbleSize"
                    ) =>
                {
                    kind = None
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
        if !sources.is_empty() {
            return sources;
        }
    }
    [("分类", &chart.categories), ("数据", &chart.data_range)]
        .into_iter()
        .filter_map(|(label, range)| range.as_ref().map(|range| (label, range.clone())))
        .collect()
}

fn local_chart_range<'a>(source: &'a str, sheet: &Sheet) -> Option<&'a str> {
    let source = source.trim();
    if let Some((name, range)) = source.rsplit_once('!') {
        let name = name.trim_matches('\'').replace("''", "'");
        if name != sheet.name {
            return None;
        }
        Some(range)
    } else {
        Some(source)
    }
}

/// Annotations are keyed by cell even when the cell itself has no value.
/// A range view only includes drawings anchored inside that range.
fn append_semantics(
    out: &mut String,
    sheet: &Sheet,
    sinks: PayloadSinks<'_>,
    window: Option<(u32, u32, u32, u32)>,
    options: TextOptions<'_>,
) {
    let inside = |col: u32, row: u32| {
        window.is_none_or(|(c1, r1, c2, r2)| col >= c1 && col <= c2 && row >= r1 && row <= r2)
    };
    for (ri, row) in sheet.rows.iter().enumerate() {
        let r = row.index.unwrap_or(ri as u32 + 1);
        for (ci, cell) in row.cells.iter().enumerate() {
            let c = col_of(cell, ci);
            if !inside(c, r) {
                continue;
            }
            let address = cell_ref(c, r);
            if let Some(link) = &cell.link {
                let tooltip = cell
                    .link_tooltip
                    .as_deref()
                    .map(|s| format!("（{}）", s.replace('\n', " ")))
                    .unwrap_or_default();
                append_section(
                    out,
                    &format!(
                        "- {address} 链接：[{}]({}){tooltip}",
                        md_label(link),
                        md_target(link)
                    ),
                );
            }
            if let Some(comment) = &cell.comment {
                append_section(
                    out,
                    &format!("- {address} 批注：{}", comment.replace('\n', "\n  ")),
                );
            }
        }
    }
    let drawing_inside = |anchor: Option<&str>| {
        window.is_none()
            || anchor
                .and_then(|a| parse_cell_ref(a).ok())
                .is_some_and(|(c, r)| inside(c, r))
    };
    for (index, image) in sheet.images.iter().enumerate() {
        if !drawing_inside(image.anchor.as_deref()) {
            continue;
        }
        let _source = sinks.at_source(format!("image[{}]", index + 1));
        let _anchor = image
            .anchor
            .as_deref()
            .map(|cell| sinks.at_source(format!("cell[{cell}]")));
        let link = sinks
            .media
            .map(|sink| {
                let name = file_name_of(&image.src);
                if office_core::attachment::is_url(&image.src) {
                    sink.link_for_url(name, &image.src)
                } else {
                    sink.link_for(name, &image.src)
                }
            })
            .unwrap_or_else(|| image.src.clone());
        append_section(
            out,
            &format!(
                "![图片 · {}]({})",
                md_label(image.anchor.as_deref().unwrap_or("位置未知")),
                md_target(&link)
            ),
        );
    }
    for chart in &sheet.charts {
        if !drawing_inside(chart.anchor.as_deref()) {
            continue;
        }
        append_section(
            out,
            &format!(
                "### {}（图表 · {}）",
                md_label(chart.title.as_deref().unwrap_or(&chart.chart_type)),
                chart.anchor.as_deref().unwrap_or("位置未知")
            ),
        );
        let sources = chart_sources(chart);
        for (label, range) in &sources {
            append_section(out, &format!("{label}来源：{range}"));
            match local_chart_range(range, sheet).and_then(|range| parse_range(range).ok()) {
                Some(((c1, r1), (c2, r2))) => {
                    let data = render_window(
                        sheet,
                        c1.min(c2),
                        r1.min(r2),
                        c1.max(c2),
                        r1.max(r2),
                        options,
                    );
                    append_section(out, &data);
                }
                None => append_section(out, "数据表不可用：来源不是当前工作表的有效单元格区域。"),
            }
        }
        if !sources.iter().any(|(label, _)| *label == "数据") {
            append_section(out, "数据表不可用：未解析到数据区域。");
        }
        for series in &chart.series {
            if let Some(name) = &series.name {
                append_section(out, &format!("- 系列：{}", name.replace('\n', " ")));
            }
        }
    }
}

/// 窗口 → Markdown：格子数在上限内画表格，超限退化成逐行稀疏列表。
/// 两种形态都**直接从 rows 拼**（不建 `(行,列)` 映射、不给每格分配字符串）。
fn render_window(
    sheet: &Sheet,
    c1: u32,
    r1: u32,
    c2: u32,
    r2: u32,
    options: TextOptions<'_>,
) -> String {
    let cells = (c2 - c1 + 1) as usize * (r2 - r1 + 1) as usize;
    if cells > MAX_TABLE_CELLS {
        sparse_direct(sheet, c1, r1, c2, r2, options)
    } else {
        table_direct(sheet, c1, r1, c2, r2, options)
    }
}

/// 已用区域（无分配扫描；判空只看字段，不生成显示串）
fn used_range_direct(sheet: &Sheet) -> Option<(u32, u32, u32, u32)> {
    let mut c1 = u32::MAX;
    let mut r1 = u32::MAX;
    let mut c2 = 0u32;
    let mut r2 = 0u32;
    let mut any = false;
    for (ri, row) in sheet.rows.iter().enumerate() {
        let r = row.index.unwrap_or(ri as u32 + 1);
        for (ci, cell) in row.cells.iter().enumerate() {
            if !has_display(cell) {
                continue;
            }
            let c = col_of(cell, ci);
            any = true;
            c1 = c1.min(c);
            r1 = r1.min(r);
            c2 = c2.max(c);
            r2 = r2.max(r);
        }
    }
    any.then_some((c1, r1, c2, r2))
}

/// 有内容可显示？（与 `cell_display` 的判空一致，但不分配字符串）
fn has_display(cell: &crate::model::cell::Cell) -> bool {
    if cell.formula.is_some() {
        return true;
    }
    match &cell.value {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// 单元格列号（优先取 ref，否则按行内序号）
fn col_of(cell: &crate::model::cell::Cell, ci: usize) -> u32 {
    cell.reference
        .as_deref()
        .and_then(|x| parse_cell_ref(x).ok())
        .map(|(c, _)| c)
        .unwrap_or(ci as u32 + 1)
}

/// 全表表格直出：行序扫描 + 列位填充，**逐格零分配**（显示文本直接写进输出缓冲）
fn table_direct(
    sheet: &Sheet,
    c1: u32,
    r1: u32,
    c2: u32,
    r2: u32,
    options: TextOptions<'_>,
) -> String {
    let cols = (c2 - c1 + 1) as usize;
    // 行按索引升序（与 grid 版一致：同索引后来者覆盖）
    let mut rows: Vec<(u32, &crate::model::sheet::Row)> = sheet
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| (row.index.unwrap_or(i as u32 + 1), row))
        .collect();
    rows.sort_by_key(|(r, _)| *r);

    let mut out = String::with_capacity(cols * (r2 - r1 + 1) as usize * 6 + 32);
    let mut cursor = 0usize;
    for r in r1..=r2 {
        // 跳过更早的行；同索引的连续行取最后一个（grid 的覆盖语义）
        let mut row: Option<&crate::model::sheet::Row> = None;
        while cursor < rows.len() {
            let (idx, cand) = rows[cursor];
            if idx < r {
                cursor += 1;
                continue;
            }
            if idx == r {
                row = Some(cand);
                cursor += 1;
                continue;
            }
            break;
        }
        // 该行要写的格子：窗口内且有内容，按列升序
        let mut items: Vec<(u32, &crate::model::cell::Cell)> = Vec::new();
        if let Some(row) = row {
            for (ci, cell) in row.cells.iter().enumerate() {
                if !has_display(cell) {
                    continue;
                }
                let c = col_of(cell, ci);
                if c < c1 || c > c2 {
                    continue;
                }
                items.push((c, cell));
            }
            items.sort_by_key(|(c, _)| *c);
        }
        push_row(&mut out, &items, c1, c2, options);
        if r == r1 {
            out.push('|');
            for i in 0..cols {
                if i > 0 {
                    out.push_str(" | ");
                } else {
                    out.push(' ');
                }
                out.push_str("---");
            }
            out.push_str(" |\n");
        }
    }
    out.trim_end().to_string()
}

/// A1 风格的单元格引用
fn cell_ref(col: u32, row: u32) -> String {
    format!("{}{}", index_to_col(col), row)
}

/// 稀疏形态：逐行列出有内容的单元格（`- A1: 值; B2: 值`），超限窗口用
fn sparse_direct(
    sheet: &Sheet,
    c1: u32,
    r1: u32,
    c2: u32,
    r2: u32,
    options: TextOptions<'_>,
) -> String {
    let mut rows: Vec<(u32, &crate::model::sheet::Row)> = sheet
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| (row.index.unwrap_or(i as u32 + 1), row))
        .collect();
    rows.sort_by_key(|(r, _)| *r);

    let mut out = String::new();
    let mut cursor = 0usize;
    for r in r1..=r2 {
        let mut row: Option<&crate::model::sheet::Row> = None;
        while cursor < rows.len() {
            let (idx, cand) = rows[cursor];
            if idx < r {
                cursor += 1;
                continue;
            }
            if idx == r {
                row = Some(cand);
                cursor += 1;
                continue;
            }
            break;
        }
        let Some(row) = row else { continue };
        let mut items: Vec<(u32, &crate::model::cell::Cell)> = Vec::new();
        for (ci, cell) in row.cells.iter().enumerate() {
            if !has_display(cell) {
                continue;
            }
            let c = col_of(cell, ci);
            if c < c1 || c > c2 {
                continue;
            }
            items.push((c, cell));
        }
        if items.is_empty() {
            continue;
        }
        items.sort_by_key(|(c, _)| *c);
        out.push_str("- ");
        for (i, (c, cell)) in items.iter().enumerate() {
            if i > 0 {
                out.push_str("; ");
            }
            out.push_str(&cell_ref(*c, r));
            out.push_str(": ");
            write_display(&mut out, cell, options);
        }
        out.push('\n');
    }
    out.trim_end().to_string() // 与表格形态一致：末尾不留空行
}

/// 写一行：`| a | b |`（缺列补空），与 `cells.join(" | ")` 的旧写法逐字节一致
fn push_row(
    out: &mut String,
    items: &[(u32, &crate::model::cell::Cell)],
    c1: u32,
    c2: u32,
    options: TextOptions<'_>,
) {
    out.push('|');
    let mut col = c1;
    let mut first = true;
    for (c, cell) in items {
        while col < *c {
            push_cell_sep(out, &mut first);
            col += 1;
        }
        push_cell_sep(out, &mut first);
        write_display(out, cell, options);
        col = c + 1;
    }
    while col <= c2 {
        push_cell_sep(out, &mut first);
        col += 1;
    }
    out.push_str(" |\n");
}

fn push_cell_sep(out: &mut String, first: &mut bool) {
    if *first {
        out.push(' ');
        *first = false;
    } else {
        out.push_str(" | ");
    }
}

/// 显示文本直接写进输出缓冲：与 `escape(cell_display(cell))` 逐字节一致，
/// 但正常内容（无 `|`、无换行、无需 trim）不做任何分配
fn write_display(out: &mut String, cell: &crate::model::cell::Cell, options: TextOptions<'_>) {
    let text = cell_text(cell, options);
    let text = text.trim();
    if text.contains('|') || text.contains('\n') {
        for ch in text.chars() {
            match ch {
                '|' => out.push_str("\\|"),
                '\n' => out.push(' '),
                c => out.push(c),
            }
        }
    } else {
        out.push_str(text);
    }
}

/// Render one cell. `value` is the stored cache when `formula` is present;
/// missing/null caches are explicit, while zero, false and empty strings are valid.
pub fn cell_text<'a>(
    cell: &'a crate::model::cell::Cell,
    options: TextOptions<'_>,
) -> std::borrow::Cow<'a, str> {
    use std::borrow::Cow;
    if let Some(formula) = &cell.formula {
        if options.value_mode == ValueMode::Formula {
            return Cow::Owned(format!("={formula}"));
        }
        let cached = if cell.value.as_ref().is_none_or(serde_json::Value::is_null) {
            Cow::Borrowed("[缓存值缺失]")
        } else {
            raw_value(cell)
        };
        return match options.value_mode {
            ValueMode::Both => Cow::Owned(format!(
                "={formula}（缓存值：{}）",
                if cached.is_empty() { "\"\"" } else { &cached }
            )),
            ValueMode::Formatted if cell.value.as_ref().is_some_and(|v| !v.is_null()) => {
                Cow::Owned(formatted_value(cell, options))
            }
            _ => cached,
        };
    }
    if options.value_mode == ValueMode::Formatted {
        Cow::Owned(formatted_value(cell, options))
    } else {
        raw_value(cell)
    }
}

fn raw_value(cell: &crate::model::cell::Cell) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    match &cell.value {
        Some(serde_json::Value::String(s)) => Cow::Borrowed(s),
        Some(serde_json::Value::Bool(b)) => Cow::Owned(b.to_string()),
        Some(serde_json::Value::Number(n)) => Cow::Owned(n.to_string()),
        Some(serde_json::Value::Null) | None => Cow::Borrowed(""),
        Some(other) => Cow::Owned(other.to_string()),
    }
}

fn formatted_value(cell: &crate::model::cell::Cell, options: TextOptions<'_>) -> String {
    use crate::model::cell::CellStyleRef;
    use crate::utils::{date, number_format};
    let style_format = match &cell.style {
        Some(CellStyleRef::Inline(style)) => style.number_format.as_deref(),
        Some(CellStyleRef::Named(name)) => options
            .styles
            .and_then(|styles| styles.get(name))
            .and_then(|style| style.number_format.as_deref()),
        None => None,
    };
    let raw = raw_value(cell);
    let Some(format) = cell.number_format.as_deref().or(style_format) else {
        return raw.into_owned();
    };
    if format.eq_ignore_ascii_case("General") || format == "@" {
        return raw.into_owned();
    }
    let value = match &cell.value {
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        Some(serde_json::Value::String(s)) if cell.cell_type.as_deref() == Some("date") => {
            date::parse_iso_date(s).map(|date| date::date_to_serial(date, options.date1904) as f64)
        }
        _ => return raw.into_owned(),
    };
    let rendered = value.and_then(|value| {
        number_format::format_datetime(value, format, options.date1904)
            .or_else(|| number_format::format_number_checked(value, format))
    });
    rendered.unwrap_or_else(|| format!("{raw}（格式未支持或值超限：{format}）"))
}

/// 整个工作簿 → Markdown（每张表一个 `## 表名` 小节）
pub fn workbook_markdown(wb: &Workbook) -> String {
    workbook_markdown_with(wb, PayloadSinks::default())
}

/// 整个工作簿 → Markdown（带附件导出）
pub fn workbook_markdown_with(wb: &Workbook, sinks: PayloadSinks<'_>) -> String {
    workbook_markdown_with_options(wb, sinks, TextOptions::default())
}

/// Render every sheet using the workbook's date system and named styles.
pub fn workbook_markdown_with_options(
    wb: &Workbook,
    sinks: PayloadSinks<'_>,
    options: TextOptions<'_>,
) -> String {
    let options = TextOptions::for_workbook(wb, options.value_mode);
    let mut out = String::new();
    for (i, s) in wb.sheets.iter().enumerate() {
        let _source = sinks.at_source(format!("sheet[{}]", i + 1));
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("## {}\n\n", s.name));
        out.push_str(&sheet_markdown_with_options(s, sinks, options));
    }
    out
}

/// 已用区域（最小/最大行列），空表返回 None
pub fn used_range(sheet: &Sheet) -> Option<(u32, u32, u32, u32)> {
    used_range_direct(sheet)
}

/// 指定窗口 → Markdown
///
/// 窗口首行作为表头（电子表格的通行约定；区域视图同此）。
/// `view <xlsx> /sheet[1]/range[A1:C10] text` 走的也是这里。
/// 输出是纯 Markdown：合并区这类表格语法表达不了的信息不放注释，
/// 需要时看 `layout` 视图（它给出 merges）。
pub fn window_markdown(sheet: &Sheet, c1: u32, r1: u32, c2: u32, r2: u32) -> String {
    window_markdown_with(sheet, c1, r1, c2, r2, PayloadSinks::default())
}

/// Range view with the same annotation and media semantics as a whole sheet.
pub fn window_markdown_with(
    sheet: &Sheet,
    c1: u32,
    r1: u32,
    c2: u32,
    r2: u32,
    sinks: PayloadSinks<'_>,
) -> String {
    window_markdown_with_options(sheet, c1, r1, c2, r2, sinks, TextOptions::default())
}

/// Render a range, including annotations, with explicit value semantics.
#[allow(clippy::too_many_arguments)]
pub fn window_markdown_with_options(
    sheet: &Sheet,
    c1: u32,
    r1: u32,
    c2: u32,
    r2: u32,
    sinks: PayloadSinks<'_>,
    options: TextOptions<'_>,
) -> String {
    let (c1, c2) = (c1.min(c2), c1.max(c2));
    let (r1, r2) = (r1.min(r2), r1.max(r2));
    let mut out = if used_range_direct(sheet).is_some() {
        render_window(sheet, c1, r1, c2, r2, options)
    } else {
        String::new()
    };
    append_semantics(&mut out, sheet, sinks, Some((c1, r1, c2, r2)), options);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_modes_keep_cached_results_and_distinguish_missing_from_empty() {
        use crate::model::Cell;
        let mut cell = Cell {
            formula: Some("SUM(B2:B8)".into()),
            value: Some(serde_json::json!(123)),
            ..Default::default()
        };
        let options = |value_mode| TextOptions {
            value_mode,
            ..Default::default()
        };
        assert_eq!(cell_text(&cell, options(ValueMode::Formula)), "=SUM(B2:B8)");
        assert_eq!(cell_text(&cell, options(ValueMode::CachedValue)), "123");
        assert_eq!(
            cell_text(&cell, options(ValueMode::Both)),
            "=SUM(B2:B8)（缓存值：123）"
        );
        for value in [None, Some(serde_json::Value::Null)] {
            cell.value = value;
            for mode in [
                ValueMode::CachedValue,
                ValueMode::Both,
                ValueMode::Formatted,
            ] {
                assert!(cell_text(&cell, options(mode)).contains("[缓存值缺失]"));
            }
        }
        for (value, expected) in [
            (serde_json::json!(0), "0"),
            (serde_json::json!(false), "false"),
            (serde_json::json!(""), ""),
            (serde_json::json!("#DIV/0!"), "#DIV/0!"),
        ] {
            cell.value = Some(value);
            assert_eq!(cell_text(&cell, options(ValueMode::CachedValue)), expected);
            assert!(!cell_text(&cell, options(ValueMode::Both)).contains("缺失"));
        }
    }

    #[test]
    fn formatted_values_resolve_styles_dates_and_workbook_epoch() {
        let mut wb: Workbook = serde_json::from_value(serde_json::json!({
            "styles":{"percent":{"number_format":"0.00%"}}, "sheets":[{"name":"格式", "rows":[{"cells":[
                {"formula":"1/8", "value":0.125,"style":"percent"},
                {"value":46053,"number_format":"yyyy/mm/dd"},
                {"value":46053.5,"number_format":"yyyy-mm-dd hh:mm:ss"},
                {"value":1234.5,"style":{"number_format":"#,##0.00"}},
                {"value":-50,"number_format":"0.00;(0.00)"},
                {"value":"2026-01-31","type":"date","number_format":"dd/mm/yyyy"},
                {"value":0.125,"style":"percent","number_format":"0%"}
            ]}], "charts":[{"type":"column", "data_range":"A1:A1"}]}]
        })).unwrap();
        let options = TextOptions::for_workbook(&wb, ValueMode::Formatted);
        let md = workbook_markdown_with_options(&wb, PayloadSinks::default(), options);
        for text in [
            "12.50%",
            "2026/01/31",
            "2026-01-31 12:00:00",
            "1,234.50",
            "(50.00)",
            "31/01/2026",
            "13%",
        ] {
            assert!(md.contains(text), "{text}: {md}");
        }
        assert!(!md.contains("=1/8"));
        assert!(
            md.matches("12.50%").count() >= 2,
            "chart tables must use the same value mode: {md}"
        );
        let sparse = window_markdown_with_options(
            &wb.sheets[0],
            1,
            1,
            40,
            600,
            PayloadSinks::default(),
            options,
        );
        assert!(sparse.contains("A1: 12.50%") && sparse.contains("B1: 2026/01/31"));
        let both = workbook_markdown_with_options(
            &wb,
            PayloadSinks::default(),
            TextOptions {
                value_mode: ValueMode::Both,
                ..Default::default()
            },
        );
        assert!(both.contains("=1/8（缓存值：0.125）"));
        wb.date1904 = true;
        wb.sheets[0].rows[0].cells[1].value = Some(serde_json::json!(0));
        let md = workbook_markdown_with_options(
            &wb,
            PayloadSinks::default(),
            TextOptions {
                value_mode: ValueMode::Formatted,
                ..Default::default()
            },
        );
        assert!(md.contains("1904/01/01"), "{md}");
    }

    #[test]
    fn chart_sources_preserve_disjoint_series_and_reject_other_sheets() {
        let s = sheet(
            serde_json::json!({"name":"本表", "rows":[{"cells":[{"value":999}]}], "charts":[{
                "type":"line", "title":"跨表图", "data_range":"A1:A1", "raw":
                "<c:chart xmlns:c='chart'><c:ser><c:val><c:numRef><c:f>'别表'!$A$1:$A$1</c:f></c:numRef></c:val></c:ser><c:ser><c:val><c:numRef><c:f>'本表'!$C$1:$C$1</c:f></c:numRef></c:val></c:ser></c:chart>"
            }]}),
        );
        let sources = chart_sources(&s.charts[0]);
        assert_eq!(
            sources,
            vec![
                ("数据", "'别表'!$A$1:$A$1".into()),
                ("数据", "'本表'!$C$1:$C$1".into())
            ]
        );
        let md = sheet_markdown(&s);
        let chart = md.split("### ").nth(1).unwrap();
        assert!(chart.contains("数据表不可用"));
        assert!(
            !chart.contains("999"),
            "must not read another sheet's A1 locally: {chart}"
        );
        assert!(chart.contains("C$1"));
    }

    #[test]
    fn drawings_and_empty_cell_annotations_survive_sheet_and_range_views() {
        let s = sheet(serde_json::json!({
            "name":"汇总", "rows":[
                {"index":1,"cells":[{"ref":"A1","value":"月份"},{"ref":"B1","value":"收入"}]},
                {"index":2,"cells":[{"ref":"A2","value":"一月"},{"ref":"B2","value":123}]},
                {"index":5,"cells":[{"ref":"C5","comment":"含税口径\n按月确认","link":"https://example.com/a (b)","link_tooltip":"出处"}]}
            ],
            "images":[{"src":"xl/media/image1.png","anchor":"D4"},{"src":"other.png","anchor":"F8"}],
            "charts":[{"type":"column","title":"月度收入","anchor":"D4","categories":"A2:A2","data_range":"B2:B2"}]
        }));
        let md = sheet_markdown(&s);
        assert!(md.contains("C5 批注：含税口径\n  按月确认"));
        assert!(md.contains("https://example.com/a%20%28b%29") && md.contains("出处"));
        assert!(md.contains("![图片 · D4](xl/media/image1.png)"));
        assert!(md.contains("月度收入") && md.contains("| 123 |") && md.contains("| 一月 |"));
        let range = window_markdown(&s, 4, 5, 3, 4);
        assert!(
            range.contains("C5 批注") && range.contains("![图片 · D4]"),
            "{range}"
        );
        assert!(!range.contains("other.png"));
        assert!(!window_markdown(&s, 1, 1, 2, 2).contains("批注"));
        let only_notes =
            sheet(serde_json::json!({"name":"s","rows":[{"cells":[{"comment":"说明"}]}]}));
        assert!(sheet_markdown(&only_notes).contains("A1 批注：说明"));
        assert!(window_markdown(&only_notes, 1, 1, 1, 1).contains("A1 批注：说明"));
    }

    fn sheet(v: serde_json::Value) -> Sheet {
        serde_json::from_value(v).expect("sheet json")
    }

    #[test]
    fn grid_becomes_table_with_first_row_as_header() {
        let s = sheet(serde_json::json!({
            "name": "经营看板",
            "rows": [
                { "index": 1, "cells": [ { "ref": "A1", "value": "月份" }, { "ref": "B1", "value": "收入" } ] },
                { "index": 2, "cells": [ { "ref": "A2", "value": "1月" }, { "ref": "B2", "value": 12.5 } ] }
            ]
        }));
        let md = sheet_markdown(&s);
        assert!(md.starts_with("| 月份 | 收入 |\n| --- | --- |\n"), "{md}");
        assert!(md.contains("| 1月 | 12.5 |"), "{md}");
    }

    #[test]
    fn gaps_are_filled_with_empty_cells() {
        let s = sheet(serde_json::json!({
            "name": "s",
            "rows": [
                { "index": 1, "cells": [ { "ref": "A1", "value": "左" }, { "ref": "C1", "value": "右" } ] },
                { "index": 2, "cells": [ { "ref": "B2", "value": "中" } ] }
            ]
        }));
        let md = sheet_markdown(&s);
        assert!(md.contains("| 左 |  | 右 |"), "{md}");
        assert!(md.contains("|  | 中 |  |"), "{md}");
    }

    #[test]
    fn formula_shows_as_formula_text() {
        let s = sheet(serde_json::json!({
            "name": "s",
            "rows": [
                { "index": 1, "cells": [ { "ref": "A1", "value": "合计" }, { "ref": "B1", "formula": "SUM(B2:B9)" } ] }
            ]
        }));
        let md = sheet_markdown(&s);
        assert!(md.contains("| 合计 | =SUM(B2:B9) |"), "{md}");
    }

    #[test]
    fn output_is_plain_markdown_without_comments() {
        let s = sheet(serde_json::json!({
            "name": "s",
            "merges": ["A1:B1"],
            "rows": [ { "index": 1, "cells": [ { "ref": "A1", "value": "标题" } ] } ]
        }));
        let md = sheet_markdown(&s);
        assert_eq!(md, "| 标题 |\n| --- |");
        assert!(!md.contains("<!--"), "{md}");
    }

    #[test]
    fn empty_sheet_is_empty_string() {
        let s = sheet(serde_json::json!({ "name": "s", "rows": [] }));
        assert_eq!(sheet_markdown(&s), "");
    }

    #[test]
    fn workbook_marks_sheet_names() {
        let wb: Workbook = serde_json::from_value(serde_json::json!({
            "sheets": [
                { "name": "汇总", "rows": [ { "index": 1, "cells": [ { "ref": "A1", "value": "x" } ] } ] },
                { "name": "明细", "rows": [] }
            ]
        }))
        .unwrap();
        let md = workbook_markdown(&wb);
        assert!(md.starts_with("## 汇总\n\n| x |\n| --- |\n"), "{md}");
        // 空表只留小节标题，不加注释标记
        assert!(md.trim_end().ends_with("## 明细"), "{md}");
    }

    #[test]
    fn sparse_form_matches_grid_semantics() {
        // 稀疏形态：行内按列升序、缺行跳过、末尾不留空行、`|` 转义
        let s = sheet(serde_json::json!({
            "name": "s",
            "rows": [
                { "index": 1, "cells": [ { "ref": "C1", "value": "右" }, { "ref": "A1", "value": "左|竖" } ] },
                { "index": 3, "cells": [ { "ref": "B3", "value": "三" } ] }
            ]
        }));
        let md = window_markdown(&s, 1, 1, 40, 600);
        assert_eq!(
            md, "- A1: 左\\|竖; C1: 右\n- B3: 三",
            "行内按列升序、跳空行、无尾随空行：{md}"
        );
    }

    #[test]
    fn huge_window_falls_back_to_sparse_rows() {
        let s = sheet(serde_json::json!({
            "name": "s",
            "rows": [
                { "index": 1, "cells": [ { "ref": "A1", "value": "起点" } ] },
                { "index": 500, "cells": [ { "ref": "A500", "value": "终点" } ] }
            ]
        }));
        let md = window_markdown(&s, 1, 1, 40, 600);
        assert!(md.contains("- A1: 起点"), "{md}");
        assert!(md.contains("- A500: 终点"), "{md}");
        assert!(!md.contains("<!--"), "{md}");
    }
}
