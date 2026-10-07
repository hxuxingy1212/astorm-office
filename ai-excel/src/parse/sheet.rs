use std::collections::{BTreeMap, HashMap};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use serde_json::{Number, Value};

use crate::error::Result;
use crate::model::cell::{Cell, CellStyleRef, Run};
use crate::model::sheet::{
    Column, ConditionalFormat, DataValidation, FilterColumn, PrintSettings, Row, SortCondition,
    SortState, Sparkline,
};
use crate::model::style::StyleDef;
use crate::parse::styles::ParsedStyles;
use crate::utils::a1::{make_cell_ref, parse_cell_ref, parse_range, shift_formula};
use crate::utils::date::{has_time_format, is_date_format, serial_to_date};
use crate::utils::xml::normalize_color;

pub struct ParsedSheet {
    pub tab_color: Option<String>,
    pub freeze: Option<String>,
    pub direction: Option<String>,
    pub gridlines: Option<bool>,
    pub headings: Option<bool>,
    pub zoom: Option<u32>,
    pub default_col_width: Option<f64>,
    pub default_row_height: Option<f64>,
    pub base_col_width: Option<f64>,
    pub show_formulas: bool,
    pub tab_selected: bool,
    pub auto_filter: Option<String>,
    pub filter_columns: Vec<FilterColumn>,
    pub columns: Vec<Column>,
    pub merges: Vec<String>,
    pub protect: bool,
    pub validations: Vec<DataValidation>,
    pub conditional_formats: Vec<ConditionalFormat>,
    pub sparklines: Vec<Sparkline>,
    pub sort: Option<SortState>,
    pub print: Option<PrintSettings>,
    pub background: Option<String>,
    pub rows: Vec<Row>,
}

struct Pending {
    reference: Option<String>,
    style_idx: u32,
    t: Option<String>,
    value_text: String,
    formula_text: String,
    has_formula: bool,
    f_shared_si: Option<u32>,
    f_shared_ref: Option<String>,
    has_is: bool,
    runs: Vec<Run>,
}

type HyperlinkEntry = (String, Option<String>, Option<String>, Option<String>);

struct Acc {
    date1904: bool,
    shared: Vec<String>,
    styles: Vec<Option<StyleDef>>,
    dxfs: Vec<Option<StyleDef>>,
    num_fmts: Vec<Option<String>>,
    rows: Vec<Row>,
    cur_row: Option<Row>,
    cur_cell: Option<Pending>,
    cur_col: u32,
    cur_run: Option<Run>,
    implicit_run: bool,
    in_rpr: bool,
    in_value: bool,
    in_formula: bool,
    in_inline_t: bool,
    break_kind: u8,
    background: Option<String>,
    hf_tag: u8,
    hf_buf: String,
    in_rph: bool,
    columns: Vec<(u32, u32, Column, bool)>,
    dim_max_col: u32,
    used_max_col: u32,
    merges: Vec<String>,
    auto_filter: Option<String>,
    sort: Option<SortState>,
    filter_columns: Vec<FilterColumn>,
    cur_filter: Option<FilterColumn>,
    freeze: Option<String>,
    direction: Option<String>,
    gridlines: Option<bool>,
    headings: Option<bool>,
    zoom: Option<u32>,
    default_col_width: Option<f64>,
    default_row_height: Option<f64>,
    base_col_width: Option<f64>,
    show_formulas: bool,
    tab_selected: bool,
    tab_color: Option<String>,
    protect: bool,
    print: PrintSettings,
    validations: Vec<DataValidation>,
    cur_dv: Option<DataValidation>,
    dv_f1: String,
    dv_f2: String,
    in_f1: bool,
    in_f2: bool,
    cf_list: Vec<ConditionalFormat>,
    sparklines: Vec<Sparkline>,
    spark_group: Option<Sparkline>,
    cur_spark: Option<Sparkline>,
    spark_text: u8, // 1 = <f>, 2 = <sqref>
    spark_buf: String,
    cur_cf: Option<ConditionalFormat>,
    cur_cf_dxf: Option<u32>,
    cur_range: String,
    in_cf_formula: bool,
    cf_formula: String,
    cf_ctx: Option<u8>, // 1 = colorScale, 2 = dataBar
    hyperlinks: Vec<HyperlinkEntry>,
    sheet_rels: HashMap<String, String>,
    /// shared formula master text + anchor, keyed by `si`.
    shared_formulas: HashMap<u32, (String, (u32, u32))>,
}

impl Acc {
    fn open(&mut self, name: &[u8], m: &BTreeMap<String, String>, empty: bool) {
        let g = |k: &str| m.get(k).map(|s| s.as_str());
        match name {
            b"row" => {
                let idx = g("r")
                    .and_then(|v| v.parse::<u32>().ok())
                    .unwrap_or_else(|| self.rows.last().and_then(|r| r.index).unwrap_or(0) + 1);
                self.cur_row = Some(Row {
                    index: Some(idx),
                    height: g("ht").and_then(|v| v.parse().ok()),
                    hidden: matches!(g("hidden"), Some("1") | Some("true")),
                    outline_level: g("outlineLevel").and_then(|v| v.parse().ok()),
                    collapsed: matches!(g("collapsed"), Some("1") | Some("true")),
                    style: g("s")
                        .and_then(|v| v.parse::<usize>().ok())
                        .and_then(|i| self.styles.get(i).and_then(|o| o.clone())),
                    cells: Vec::new(),
                });
                self.cur_col = 1;
                let _ = empty;
            }
            b"c" => {
                let p = Pending {
                    reference: g("r").map(|s| s.to_string()),
                    style_idx: g("s").and_then(|v| v.parse().ok()).unwrap_or(0),
                    t: g("t").map(|s| s.to_string()),
                    value_text: String::new(),
                    formula_text: String::new(),
                    has_formula: false,
                    f_shared_si: None,
                    f_shared_ref: None,
                    has_is: false,
                    runs: Vec::new(),
                };
                self.cur_cell = Some(p);
                if empty {
                    self.finalize_cell();
                }
            }
            b"v" => self.in_value = true,
            b"rPh" => self.in_rph = true,
            b"is" => {
                if let Some(c) = self.cur_cell.as_mut() {
                    c.has_is = true;
                }
            }
            b"f" if self.cur_spark.is_none() => {
                self.in_formula = true;
                let si = g("si").and_then(|v| v.parse::<u32>().ok());
                let sref = g("ref").map(|s| s.to_string());
                let shared = g("t") == Some("shared");
                if let Some(c) = self.cur_cell.as_mut() {
                    if shared {
                        c.f_shared_si = si;
                        c.f_shared_ref = sref;
                    }
                }
            }
            b"t" if !self.in_rph => {
                self.in_inline_t = true;
                // 顶层 <t>（不在 <r> 内）：作为隐式 run，保证与 <r> 交替时
                // 文本顺序不丢失（如 "<is><t>标题 </t><r><t>日期</t></r></is>"）。
                if self.cur_run.is_none()
                    && self.cur_cell.as_ref().map(|c| c.has_is).unwrap_or(false)
                {
                    self.cur_run = Some(Run::default());
                    self.implicit_run = true;
                }
            }
            b"r" => self.cur_run = Some(Run::default()),
            b"rPr" => self.in_rpr = true,
            b"b" if self.in_rpr => {
                if let Some(run) = self.cur_run.as_mut() {
                    run.bold = true;
                }
            }
            b"i" if self.in_rpr => {
                if let Some(run) = self.cur_run.as_mut() {
                    run.italic = true;
                }
            }
            b"color" if self.in_rpr => {
                if let (Some(run), Some(rgb)) = (self.cur_run.as_mut(), g("rgb")) {
                    run.color = Some(normalize_color(rgb));
                }
            }
            b"strike" if self.in_rpr => {
                if let Some(run) = self.cur_run.as_mut() {
                    run.strike = true;
                }
            }
            b"u" if self.in_rpr => {
                if let Some(run) = self.cur_run.as_mut() {
                    run.underline = Some(g("val").unwrap_or("single").to_string());
                }
            }
            b"vertAlign" if self.in_rpr => {
                if let (Some(run), Some(v)) = (self.cur_run.as_mut(), g("val")) {
                    run.vert_align = Some(v.to_string());
                }
            }
            b"rFont" if self.in_rpr => {
                if let (Some(run), Some(v)) = (self.cur_run.as_mut(), g("val")) {
                    run.font = Some(v.to_string());
                }
            }
            b"sz" if self.in_rpr => {
                if let (Some(run), Some(v)) = (self.cur_run.as_mut(), g("val")) {
                    run.size = v.parse().ok();
                }
            }
            b"hyperlink" => {
                self.hyperlinks.push((
                    g("ref").unwrap_or_default().to_string(),
                    g("id").map(|s| s.to_string()),
                    g("location").map(|s| s.to_string()),
                    g("tooltip").map(|s| s.to_string()),
                ));
            }
            b"tabColor" => {
                if let Some(rgb) = g("rgb") {
                    self.tab_color = Some(normalize_color(rgb));
                }
            }
            b"pane" => {
                if let Some(tl) = g("topLeftCell") {
                    self.freeze = Some(tl.to_string());
                }
            }
            b"sheetView" => {
                if matches!(g("rightToLeft"), Some("1") | Some("true")) {
                    self.direction = Some("rtl".to_string());
                }
                if let Some(v) = g("showGridLines") {
                    self.gridlines = Some(v != "0" && v != "false");
                }
                if let Some(v) = g("showRowColHeaders") {
                    self.headings = Some(v != "0" && v != "false");
                }
                self.zoom = g("zoomScale").and_then(|v| v.parse().ok());
                self.show_formulas = matches!(g("showFormulas"), Some("1") | Some("true"));
                self.tab_selected = matches!(g("tabSelected"), Some("1") | Some("true"));
            }
            b"autoFilter" => {
                self.auto_filter = g("ref").map(|s| s.to_string());
            }
            b"sortState" => {
                self.sort = Some(SortState {
                    range: g("ref").unwrap_or_default().to_string(),
                    conditions: Vec::new(),
                });
            }
            b"sortCondition" => {
                if let Some(st) = self.sort.as_mut() {
                    st.conditions.push(SortCondition {
                        column: g("ref").unwrap_or_default().to_string(),
                        descending: matches!(g("descending"), Some("1") | Some("true")),
                        sort_on: g("sortBy").map(|s| s.to_string()),
                    });
                }
            }
            b"sheetFormatPr" => {
                self.default_col_width = g("defaultColWidth").and_then(|v| v.parse().ok());
                self.default_row_height = g("defaultRowHeight").and_then(|v| v.parse().ok());
                self.base_col_width = g("baseColWidth").and_then(|v| v.parse().ok());
            }
            b"col" => {
                let min = g("min").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1);
                let max = g("max").and_then(|v| v.parse::<u32>().ok()).unwrap_or(min);
                let col = Column {
                    width: g("width").and_then(|v| v.parse().ok()),
                    hidden: matches!(g("hidden"), Some("1") | Some("true")),
                    outline_level: g("outlineLevel").and_then(|v| v.parse().ok()),
                    collapsed: matches!(g("collapsed"), Some("1") | Some("true")),
                    style: g("style")
                        .and_then(|v| v.parse::<usize>().ok())
                        .and_then(|i| self.styles.get(i).and_then(|o| o.clone())),
                };
                let custom = matches!(g("customWidth"), Some("1") | Some("true"));
                let meaningful =
                    custom || col.hidden || col.outline_level.is_some() || col.collapsed;
                self.columns.push((min, max, col, meaningful));
            }
            b"dimension" => {
                if let Some(r) = g("ref") {
                    if let Ok((_, (c2, _))) = parse_range(r) {
                        self.dim_max_col = c2;
                    }
                }
            }
            b"mergeCell" => {
                if let Some(r) = g("ref") {
                    self.merges.push(r.to_string());
                }
            }
            b"filterColumn" => {
                self.cur_filter = Some(FilterColumn {
                    col_id: g("colId").and_then(|v| v.parse().ok()).unwrap_or(0),
                    ..Default::default()
                });
                let _ = empty;
            }
            b"filter" => {
                if let (Some(fc), Some(v)) = (self.cur_filter.as_mut(), g("val")) {
                    fc.values.push(v.to_string());
                }
            }
            b"customFilter" => {
                if let Some(fc) = self.cur_filter.as_mut() {
                    if fc.criteria.is_none() {
                        fc.operator = g("operator").map(|s| s.to_string());
                        fc.criteria = g("val").map(|s| s.to_string());
                    } else {
                        fc.criteria2 = g("val").map(|s| s.to_string());
                    }
                }
            }
            b"sheetProtection" => {
                self.protect = g("sheet").map(|v| v == "1" || v == "true").unwrap_or(true);
            }
            b"pageSetUpPr" => {
                if matches!(g("fitToPage"), Some("1") | Some("true")) {
                    self.print.fit_to_page = true;
                }
            }
            b"pageSetup" => {
                self.print.orientation = g("orientation").map(|s| s.to_string());
                self.print.paper_size = g("paperSize").and_then(|v| v.parse().ok());
                self.print.scale = g("scale").and_then(|v| v.parse().ok());
                self.print.fit_to_width = g("fitToWidth").and_then(|v| v.parse().ok());
                self.print.fit_to_height = g("fitToHeight").and_then(|v| v.parse().ok());
            }
            b"pageMargins" => {
                self.print.margin_left = g("left").and_then(|v| v.parse().ok());
                self.print.margin_right = g("right").and_then(|v| v.parse().ok());
                self.print.margin_top = g("top").and_then(|v| v.parse().ok());
                self.print.margin_bottom = g("bottom").and_then(|v| v.parse().ok());
                self.print.margin_header = g("header").and_then(|v| v.parse().ok());
                self.print.margin_footer = g("footer").and_then(|v| v.parse().ok());
            }
            b"printOptions" => {
                self.print.h_center = matches!(g("horizontalCentered"), Some("1") | Some("true"));
                self.print.v_center = matches!(g("verticalCentered"), Some("1") | Some("true"));
            }
            b"rowBreaks" => self.break_kind = 1,
            b"colBreaks" => self.break_kind = 2,
            b"brk" => {
                if let Some(id) = g("id").and_then(|v| v.parse::<u32>().ok()) {
                    match self.break_kind {
                        1 => self.print.row_breaks.push(id),
                        2 => self.print.col_breaks.push(id),
                        _ => {}
                    }
                }
            }
            b"sparklineGroup" => {
                self.spark_group = Some(Sparkline {
                    kind: g("type").map(|s| s.to_string()),
                    show_high: matches!(g("showHigh"), Some("1") | Some("true")),
                    show_low: matches!(g("showLow"), Some("1") | Some("true")),
                    show_first: matches!(g("showFirst"), Some("1") | Some("true")),
                    show_last: matches!(g("showLast"), Some("1") | Some("true")),
                    show_negative: matches!(g("showNegative"), Some("1") | Some("true")),
                    ..Default::default()
                });
            }
            b"colorSeries" => {
                if let (Some(grp), Some(rgb)) = (self.spark_group.as_mut(), g("rgb")) {
                    grp.color = Some(crate::utils::xml::normalize_color(rgb));
                }
            }
            b"sparkline" => {
                self.cur_spark = Some(self.spark_group.clone().unwrap_or_default());
            }
            b"f" if self.cur_spark.is_some() => {
                self.spark_text = 1;
                self.spark_buf.clear();
            }
            b"sqref" if self.cur_spark.is_some() => {
                self.spark_text = 2;
                self.spark_buf.clear();
            }
            b"picture" => {
                if let Some(rid) = g("id") {
                    if let Some(target) = self.sheet_rels.get(rid) {
                        self.background =
                            Some(crate::parse::drawing::resolve_part("xl/worksheets", target));
                    }
                }
            }
            b"headerFooter" => {
                self.print.different_odd_even =
                    matches!(g("differentOddEven"), Some("1") | Some("true"));
                self.print.different_first =
                    matches!(g("differentFirst"), Some("1") | Some("true"));
            }
            b"oddHeader" => {
                self.hf_tag = 1;
                self.hf_buf.clear();
            }
            b"oddFooter" => {
                self.hf_tag = 2;
                self.hf_buf.clear();
            }
            b"evenHeader" => {
                self.hf_tag = 3;
                self.hf_buf.clear();
            }
            b"evenFooter" => {
                self.hf_tag = 4;
                self.hf_buf.clear();
            }
            b"firstHeader" => {
                self.hf_tag = 5;
                self.hf_buf.clear();
            }
            b"firstFooter" => {
                self.hf_tag = 6;
                self.hf_buf.clear();
            }
            b"dataValidation" => {
                self.cur_dv = Some(DataValidation {
                    range: g("sqref").unwrap_or_default().to_string(),
                    validation_type: g("type").unwrap_or("none").to_string(),
                    operator: g("operator").map(|s| s.to_string()),
                    formula1: None,
                    formula2: None,
                    values: Vec::new(),
                    allow_blank: matches!(g("allowBlank"), Some("1") | Some("true")),
                    show_error: matches!(g("showErrorMessage"), Some("1") | Some("true")),
                    error_title: g("errorTitle").map(|s| s.to_string()),
                    error_message: g("error").map(|s| s.to_string()),
                    prompt_title: g("promptTitle").map(|s| s.to_string()),
                    prompt_message: g("prompt").map(|s| s.to_string()),
                });
                let _ = empty;
            }
            b"formula1" => self.in_f1 = true,
            b"formula2" => self.in_f2 = true,
            b"conditionalFormatting" => {
                self.cur_range = g("sqref").unwrap_or_default().to_string();
            }
            b"cfRule" => {
                self.cur_cf = Some(ConditionalFormat {
                    range: self.cur_range.clone(),
                    rule_type: g("type").unwrap_or_default().to_string(),
                    operator: match g("type") {
                        Some("containsText")
                        | Some("beginsWith")
                        | Some("endsWith")
                        | Some("notContainsText") => None,
                        _ => g("operator").map(|s| s.to_string()),
                    },
                    formulas: Vec::new(),
                    colors: Vec::new(),
                    color: None,
                    text: g("text").map(|s| s.to_string()),
                    rank: g("rank").and_then(|v| v.parse().ok()),
                    percent: matches!(g("percent"), Some("1") | Some("true")),
                    bottom: matches!(g("bottom"), Some("1") | Some("true")),
                    icon_style: None,
                    reverse_icons: false,
                    show_value: None,
                    time_period: g("timePeriod").map(|s| s.to_string()),
                    above_average: g("aboveAverage").map(|v| v == "1" || v == "true"),
                    equal_average: matches!(g("equalAverage"), Some("1") | Some("true")),
                    std_dev: g("stdDev").and_then(|v| v.parse().ok()),
                    font: None,
                    fill: None,
                    stop_if_true: matches!(g("stopIfTrue"), Some("1") | Some("true")),
                });
                self.cur_cf_dxf = g("dxfId").and_then(|v| v.parse().ok());
                let _ = empty;
            }
            b"iconSet" => {
                if let Some(cf) = self.cur_cf.as_mut() {
                    cf.icon_style = g("iconSet").map(|s| s.to_string());
                    cf.reverse_icons = matches!(g("reverse"), Some("1") | Some("true"));
                    cf.show_value = g("showValue").map(|v| v == "1" || v == "true");
                }
            }
            b"colorScale" => self.cf_ctx = Some(1),
            b"dataBar" => self.cf_ctx = Some(2),
            b"color" if !self.in_rpr => {
                if let Some(rgb) = g("rgb") {
                    let c = normalize_color(rgb);
                    match self.cf_ctx {
                        Some(1) => {
                            if let Some(cf) = self.cur_cf.as_mut() {
                                cf.colors.push(c);
                            }
                        }
                        Some(2) => {
                            if let Some(cf) = self.cur_cf.as_mut() {
                                cf.color = Some(c);
                            }
                        }
                        _ => {}
                    }
                }
            }
            b"formula" => {
                self.in_cf_formula = true;
                self.cf_formula.clear();
            }
            _ => {}
        }
    }

    fn text(&mut self, s: &str) {
        if self.spark_text != 0 {
            self.spark_buf.push_str(s);
        }
        if self.hf_tag != 0 {
            self.hf_buf.push_str(s);
        }
        if self.in_formula {
            if let Some(c) = self.cur_cell.as_mut() {
                c.formula_text.push_str(s);
                c.has_formula = true;
            }
        } else if self.in_value || self.in_inline_t {
            if let Some(c) = self.cur_cell.as_mut() {
                c.value_text.push_str(s);
            }
        }
        if self.in_inline_t {
            if let Some(run) = self.cur_run.as_mut() {
                run.text.push_str(s);
            }
        }
        if self.in_f1 {
            self.dv_f1.push_str(s);
        }
        if self.in_f2 {
            self.dv_f2.push_str(s);
        }
        if self.in_cf_formula {
            self.cf_formula.push_str(s);
        }
    }

    fn close(&mut self, name: &[u8]) {
        // 迷你图内部元素优先处理，避免与公式 <f> 冲突。
        if self.spark_text == 1 && name == b"f" {
            if let Some(s) = self.cur_spark.as_mut() {
                s.data_range = self.spark_buf.clone();
            }
            self.spark_text = 0;
            return;
        }
        if self.spark_text == 2 && name == b"sqref" {
            if let Some(s) = self.cur_spark.as_mut() {
                s.location = self.spark_buf.clone();
            }
            self.spark_text = 0;
            return;
        }
        if name == b"sparkline" {
            if let Some(s) = self.cur_spark.take() {
                self.sparklines.push(s);
            }
            return;
        }
        if name == b"sparklineGroup" {
            self.spark_group = None;
            return;
        }
        if self.hf_tag != 0
            && matches!(
                name,
                b"oddHeader"
                    | b"oddFooter"
                    | b"evenHeader"
                    | b"evenFooter"
                    | b"firstHeader"
                    | b"firstFooter"
            )
        {
            let v = std::mem::take(&mut self.hf_buf);
            let v = if v.is_empty() { None } else { Some(v) };
            match self.hf_tag {
                1 => self.print.odd_header = v,
                2 => self.print.odd_footer = v,
                3 => self.print.even_header = v,
                4 => self.print.even_footer = v,
                5 => self.print.first_header = v,
                _ => self.print.first_footer = v,
            }
            self.hf_tag = 0;
            return;
        }
        match name {
            b"v" => self.in_value = false,
            b"f" => {
                self.in_formula = false;
                let si = self.cur_cell.as_ref().and_then(|p| p.f_shared_si);
                if let Some(si) = si {
                    let (text, sref, reference) = {
                        let p = self.cur_cell.as_mut().unwrap();
                        (
                            std::mem::take(&mut p.formula_text),
                            p.f_shared_ref.take(),
                            p.reference.clone(),
                        )
                    };
                    let final_text = if let Some(r) = sref {
                        if !text.is_empty() {
                            if let Ok(((c1, r1), _)) = parse_range(&r) {
                                self.shared_formulas.insert(si, (text.clone(), (c1, r1)));
                            }
                        }
                        text
                    } else if let Some((mtext, anchor)) = self.shared_formulas.get(&si).cloned() {
                        let (drow, dcol) =
                            match reference.as_deref().and_then(|x| parse_cell_ref(x).ok()) {
                                Some((c, rr)) => {
                                    (rr as i64 - anchor.1 as i64, c as i64 - anchor.0 as i64)
                                }
                                None => (0, 0),
                            };
                        shift_formula(&mtext, drow, dcol)
                    } else {
                        String::new()
                    };
                    if let Some(p) = self.cur_cell.as_mut() {
                        p.formula_text = final_text;
                        p.has_formula = true;
                    }
                }
            }
            b"t" => {
                self.in_inline_t = false;
                if self.implicit_run {
                    self.implicit_run = false;
                    if let Some(run) = self.cur_run.take() {
                        if !run.text.is_empty() {
                            if let Some(c) = self.cur_cell.as_mut() {
                                c.runs.push(run);
                            }
                        }
                    }
                }
            }
            b"rowBreaks" | b"colBreaks" => self.break_kind = 0,
            b"rPh" => self.in_rph = false,
            b"rPr" => self.in_rpr = false,
            b"r" => {
                if let Some(run) = self.cur_run.take() {
                    if !run.text.is_empty() {
                        if let Some(c) = self.cur_cell.as_mut() {
                            c.runs.push(run);
                        }
                    }
                }
            }
            b"formula1" => self.in_f1 = false,
            b"formula2" => self.in_f2 = false,
            b"formula" => {
                if let Some(cf) = self.cur_cf.as_mut() {
                    if matches!(cf.rule_type.as_str(), "cellIs" | "expression") {
                        cf.formulas.push(std::mem::take(&mut self.cf_formula));
                    }
                }
                self.in_cf_formula = false;
            }
            b"colorScale" | b"dataBar" => self.cf_ctx = None,
            b"cfRule" => {
                if let Some(mut cf) = self.cur_cf.take() {
                    if let Some(id) = self.cur_cf_dxf {
                        if let Some(Some(s)) = self.dxfs.get(id as usize) {
                            cf.font = s.font.clone();
                            cf.fill = s.fill.clone();
                        }
                    }
                    self.cf_list.push(cf);
                    self.cur_cf_dxf = None;
                }
            }
            b"dataValidation" => {
                if let Some(mut dv) = self.cur_dv.take() {
                    let f1 = std::mem::take(&mut self.dv_f1);
                    let f2 = std::mem::take(&mut self.dv_f2);
                    if !f1.is_empty() {
                        if dv.validation_type == "list" && f1.starts_with('"') && f1.ends_with('"')
                        {
                            dv.values = f1[1..f1.len() - 1]
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .collect();
                        } else {
                            dv.formula1 = Some(f1);
                        }
                    }
                    if !f2.is_empty() {
                        dv.formula2 = Some(f2);
                    }
                    self.validations.push(dv);
                }
            }
            b"c" => self.finalize_cell(),
            b"filterColumn" => {
                if let Some(fc) = self.cur_filter.take() {
                    self.filter_columns.push(fc);
                }
            }
            b"row" => {
                if let Some(row) = self.cur_row.take() {
                    self.rows.push(row);
                }
            }
            _ => {}
        }
    }

    fn finalize_cell(&mut self) {
        let Some(mut p) = self.cur_cell.take() else {
            return;
        };
        // OOXML allows `<row>`/`<c>` without an `r` attribute; the position is
        // implied by the previous cell in the row (+1). Synthesis it so cells
        // are not lost.
        let row_idx = self.cur_row.as_ref().and_then(|r| r.index).unwrap_or(0);
        match p.reference.as_deref().and_then(|x| parse_cell_ref(x).ok()) {
            Some((c, _)) => self.cur_col = c + 1,
            None => {
                let c = self.cur_col.max(1);
                p.reference = Some(make_cell_ref(c, row_idx));
                self.cur_col = c + 1;
            }
        }
        let style = self
            .styles
            .get(p.style_idx as usize)
            .and_then(|o| o.clone());
        let number_format = self
            .num_fmts
            .get(p.style_idx as usize)
            .and_then(|o| o.clone());
        let mut cell = Cell {
            reference: p.reference,
            style: style.clone().map(CellStyleRef::Inline),
            number_format,
            ..Default::default()
        };

        if !p.runs.is_empty() {
            for run in &mut p.runs {
                run.text = norm_newlines(std::mem::take(&mut run.text));
            }
            let text: String = p.runs.iter().map(|r| r.text.clone()).collect();
            cell.runs = p.runs;
            cell.value = Some(Value::String(text));
            cell.cell_type = Some("richtext".into());
        } else if p.has_formula {
            cell.formula = Some(p.formula_text);
            match p.t.as_deref() {
                Some("str") => {
                    cell.value = Some(Value::String(norm_newlines(p.value_text)));
                    cell.cell_type = Some("string".into());
                }
                Some("b") => {
                    cell.value = Some(Value::Bool(p.value_text == "1"));
                    cell.cell_type = Some("boolean".into());
                }
                Some("e") => {
                    cell.value = Some(Value::String(p.value_text));
                    cell.cell_type = Some("error".into());
                }
                _ => {
                    if !p.value_text.is_empty() {
                        cell.value = Some(make_number(&p.value_text));
                    }
                }
            }
        } else {
            match p.t.as_deref() {
                Some("s") => {
                    let idx: usize = p.value_text.parse().unwrap_or(usize::MAX);
                    let s = self.shared.get(idx).cloned().unwrap_or_default();
                    cell.value = Some(Value::String(norm_newlines(s)));
                    cell.cell_type = Some("string".into());
                }
                Some("inlineStr") => {
                    if p.has_is || !p.value_text.is_empty() {
                        cell.value = Some(Value::String(norm_newlines(p.value_text)));
                        cell.cell_type = Some("string".into());
                    }
                }
                Some("str") => {
                    cell.value = Some(Value::String(norm_newlines(p.value_text)));
                    cell.cell_type = Some("string".into());
                }
                Some("b") => {
                    cell.value = Some(Value::Bool(p.value_text == "1"));
                    cell.cell_type = Some("boolean".into());
                }
                Some("e") => {
                    cell.value = Some(Value::String(p.value_text));
                    cell.cell_type = Some("error".into());
                }
                Some("n") | None => {
                    if !p.value_text.is_empty() {
                        let date_fmt = cell
                            .number_format
                            .as_deref()
                            .map(is_date_format)
                            .unwrap_or(false);
                        let time_fmt = cell
                            .number_format
                            .as_deref()
                            .map(has_time_format)
                            .unwrap_or(false);
                        // 日期时间格式保留原始序列值（避免丢失时分秒）。
                        if date_fmt && !time_fmt {
                            if let Ok(serial) = p.value_text.parse::<f64>() {
                                match serial_to_date(serial.trunc() as i64, self.date1904) {
                                    Some(d) => {
                                        cell.value =
                                            Some(Value::String(d.format("%Y-%m-%d").to_string()));
                                        cell.cell_type = Some("date".into());
                                    }
                                    None => {
                                        cell.value = Some(make_number(&p.value_text));
                                        cell.cell_type = Some("number".into());
                                    }
                                }
                            } else {
                                cell.value = Some(make_number(&p.value_text));
                                cell.cell_type = Some("number".into());
                            }
                        } else {
                            cell.value = Some(make_number(&p.value_text));
                            cell.cell_type = Some("number".into());
                        }
                    }
                }
                _ => {
                    cell.value = Some(Value::String(p.value_text));
                    cell.cell_type = Some("string".into());
                }
            }
        }

        if cell.value.is_none()
            && cell.formula.is_none()
            && cell.style.is_none()
            && cell.number_format.is_none()
        {
            return;
        }
        if let Some(r) = cell.reference.as_deref() {
            if let Ok((c, _)) = parse_cell_ref(r) {
                self.used_max_col = self.used_max_col.max(c);
            }
        }
        if let Some(row) = self.cur_row.as_mut() {
            row.cells.push(cell);
        }
    }
}

/// Preserve source line endings exactly (CRLF/CR kept) so round-trip output is
/// byte-faithful; Excel/LibreOffice render both forms as line breaks.
fn norm_newlines(s: String) -> String {
    s
}

fn make_number(text: &str) -> Value {
    if let Ok(i) = text.parse::<i64>() {
        Value::Number(Number::from(i))
    } else if let Ok(f) = text.parse::<f64>() {
        Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(text.into()))
    } else {
        Value::String(text.into())
    }
}

fn attrs_of(e: &BytesStart) -> BTreeMap<String, String> {
    e.attributes()
        .filter_map(|a| a.ok())
        .map(|a| {
            (
                String::from_utf8_lossy(a.key.local_name().as_ref()).to_string(),
                a.unescape_value()
                    .map(|v| v.into_owned())
                    .unwrap_or_default(),
            )
        })
        .collect()
}

pub fn parse_sheet(
    xml: &str,
    shared: &[String],
    styles: &ParsedStyles,
    date1904: bool,
    sheet_rels: &HashMap<String, String>,
) -> Result<ParsedSheet> {
    let mut acc = Acc {
        date1904,
        shared: shared.to_vec(),
        styles: styles.xfs.clone(),
        dxfs: styles.dxfs.clone(),
        num_fmts: styles.num_fmts.clone(),
        rows: Vec::new(),
        cur_row: None,
        cur_cell: None,
        cur_col: 1,
        cur_run: None,
        implicit_run: false,
        in_rpr: false,
        in_value: false,
        in_formula: false,
        in_inline_t: false,
        break_kind: 0,
        background: None,
        hf_tag: 0,
        hf_buf: String::new(),
        in_rph: false,
        columns: Vec::new(),
        dim_max_col: 0,
        used_max_col: 0,
        merges: Vec::new(),
        auto_filter: None,
        sort: None,
        filter_columns: Vec::new(),
        cur_filter: None,
        freeze: None,
        direction: None,
        gridlines: None,
        headings: None,
        zoom: None,
        default_col_width: None,
        default_row_height: None,
        base_col_width: None,
        show_formulas: false,
        tab_selected: false,
        tab_color: None,
        protect: false,
        print: PrintSettings::default(),
        validations: Vec::new(),
        cur_dv: None,
        dv_f1: String::new(),
        dv_f2: String::new(),
        in_f1: false,
        in_f2: false,
        cf_list: Vec::new(),
        sparklines: Vec::new(),
        spark_group: None,
        cur_spark: None,
        spark_text: 0,
        spark_buf: String::new(),
        cur_cf: None,
        cur_cf_dxf: None,
        cur_range: String::new(),
        in_cf_formula: false,
        cf_formula: String::new(),
        cf_ctx: None,
        hyperlinks: Vec::new(),
        sheet_rels: sheet_rels.clone(),
        shared_formulas: HashMap::new(),
    };

    let mut reader = Reader::from_str(xml);
    loop {
        match reader.read_event()? {
            Event::Start(e) => {
                let name = e.local_name().as_ref().to_vec();
                let m = attrs_of(&e);
                acc.open(&name, &m, false);
            }
            Event::Empty(e) => {
                let name = e.local_name().as_ref().to_vec();
                let m = attrs_of(&e);
                acc.open(&name, &m, true);
                acc.close(&name);
            }
            Event::Text(e) => {
                let t = e.unescape()?;
                acc.text(&t);
            }
            Event::End(e) => {
                let name = e.local_name().as_ref().to_vec();
                acc.close(&name);
            }
            Event::Eof => break,
            _ => {}
        }
    }

    let mut ncols = acc.dim_max_col.max(acc.used_max_col);
    let soft = ncols.max(128);
    for (_, max, _, meaningful) in &acc.columns {
        if *meaningful {
            ncols = ncols.max((*max).min(soft));
        }
    }
    ncols = ncols.min(4096);
    let mut columns = vec![Column::default(); ncols as usize];
    for (min, max, col, _) in &acc.columns {
        let lo = (*min).max(1);
        let hi = (*max).min(ncols);
        let mut i = lo;
        while i <= hi {
            columns[(i - 1) as usize] = col.clone();
            i += 1;
        }
    }
    while columns
        .last()
        .map(|c| c.width.is_none() && !c.hidden && c.outline_level.is_none() && !c.collapsed)
        .unwrap_or(false)
    {
        columns.pop();
    }

    // Apply hyperlinks to their target cells.
    let hyperlinks = std::mem::take(&mut acc.hyperlinks);
    for (reference, rid, location, tooltip) in hyperlinks {
        let link = if let Some(loc) = location {
            format!("#{loc}")
        } else if let Some(rid) = rid {
            acc.sheet_rels.get(&rid).cloned().unwrap_or_default()
        } else {
            String::new()
        };
        if link.is_empty() {
            continue;
        }
        set_cell_link(&mut acc.rows, &reference, link, tooltip);
    }

    let print = if acc.print == PrintSettings::default() {
        None
    } else {
        Some(acc.print)
    };

    Ok(ParsedSheet {
        tab_color: acc.tab_color,
        freeze: acc.freeze,
        direction: acc.direction,
        gridlines: acc.gridlines,
        headings: acc.headings,
        zoom: acc.zoom,
        default_col_width: acc.default_col_width,
        default_row_height: acc.default_row_height,
        base_col_width: acc.base_col_width,
        show_formulas: acc.show_formulas,
        tab_selected: acc.tab_selected,
        auto_filter: acc.auto_filter,
        sort: acc.sort,
        filter_columns: acc.filter_columns,
        columns,
        merges: acc.merges,
        protect: acc.protect,
        validations: acc.validations,
        conditional_formats: acc.cf_list,
        sparklines: acc.sparklines,
        print,
        background: acc.background,
        rows: acc.rows,
    })
}

fn set_cell_link(rows: &mut [Row], reference: &str, link: String, tooltip: Option<String>) {
    for row in rows {
        for cell in &mut row.cells {
            if cell.reference.as_deref() == Some(reference) {
                cell.link = Some(link);
                cell.link_tooltip = tooltip;
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_formula_expands() {
        let styles = ParsedStyles {
            xfs: vec![None],
            num_fmts: vec![None],
            dxfs: Vec::new(),
            default_font: None,
        };
        let rels = HashMap::new();
        let xml = r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="4"><c r="C4"><f t="shared" ref="C4:H4" si="0">C3/$I$3</f><v>32</v></c><c r="D4"><f t="shared" si="0"/><v>33</v></c></row></sheetData></worksheet>"#;
        let ps = parse_sheet(xml, &[], &styles, false, &rels).unwrap();
        let cells = &ps.rows[0].cells;
        assert_eq!(cells[0].formula.as_deref(), Some("C3/$I$3"));
        assert_eq!(cells[1].formula.as_deref(), Some("D3/$I$3"));
    }
}

#[cfg(test)]
mod col_tests {
    use super::*;

    #[test]
    fn huge_col_range_is_capped() {
        let styles = ParsedStyles {
            xfs: vec![None],
            num_fmts: vec![None],
            dxfs: Vec::new(),
            default_font: None,
        };
        let rels = HashMap::new();
        let xml = r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><dimension ref="A1:B2"/><cols><col min="1" max="1" width="24" customWidth="1"/><col min="3" max="16384" width="9"/></cols><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData></worksheet>"#;
        let ps = parse_sheet(xml, &[], &styles, false, &rels).unwrap();
        assert!(ps.columns.len() <= 2, "columns={}", ps.columns.len());
    }
}

#[cfg(test)]
mod ref_tests {
    use super::*;

    fn styles() -> ParsedStyles {
        ParsedStyles {
            xfs: vec![None],
            num_fmts: vec![None],
            dxfs: Vec::new(),
            default_font: None,
        }
    }

    #[test]
    fn omitted_row_and_cell_refs_are_synthesized() {
        let rels = HashMap::new();
        let xml = r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row><c><v>1</v></c><c><v>2</v></c></row><row><c><v>3</v></c></row></sheetData></worksheet>"#;
        let ps = parse_sheet(xml, &[], &styles(), false, &rels).unwrap();
        assert_eq!(ps.rows[0].index, Some(1));
        assert_eq!(ps.rows[0].cells[0].reference.as_deref(), Some("A1"));
        assert_eq!(ps.rows[0].cells[1].reference.as_deref(), Some("B1"));
        assert_eq!(ps.rows[1].index, Some(2));
        assert_eq!(ps.rows[1].cells[0].reference.as_deref(), Some("A2"));
    }
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    fn styles() -> ParsedStyles {
        ParsedStyles {
            xfs: vec![None],
            num_fmts: vec![None],
            dxfs: Vec::new(),
            default_font: None,
        }
    }

    #[test]
    fn empty_inlinestr_kept_only_with_is() {
        let rels = HashMap::new();
        let xml = r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t></t></is></c><c r="B1" t="inlineStr"/><c r="C1" t="str"><v></v></c></row></sheetData></worksheet>"#;
        let ps = parse_sheet(xml, &[], &styles(), false, &rels).unwrap();
        let cells = &ps.rows[0].cells;
        let find = |r: &str| cells.iter().find(|c| c.reference.as_deref() == Some(r));
        assert_eq!(
            find("A1").unwrap().value,
            Some(Value::String(String::new()))
        );
        assert!(find("B1").is_none());
        assert_eq!(
            find("C1").unwrap().value,
            Some(Value::String(String::new()))
        );
    }
}

#[cfg(test)]
mod richtext_tests {
    use super::*;
    use crate::parse::styles::ParsedStyles;
    use std::collections::HashMap;

    fn styles() -> ParsedStyles {
        ParsedStyles {
            xfs: vec![None],
            num_fmts: vec![None],
            dxfs: Vec::new(),
            default_font: None,
        }
    }

    #[test]
    fn inline_plain_then_run_kept_in_order() {
        let rels = HashMap::new();
        let xml = r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t xml:space="preserve">ARREST for </t><r><t>01/30/17</t></r></is></c></row></sheetData></worksheet>"#;
        let ps = parse_sheet(xml, &[], &styles(), false, &rels).unwrap();
        let c = &ps.rows[0].cells[0];
        assert_eq!(c.value, Some(Value::String("ARREST for 01/30/17".into())));
        let texts: Vec<&str> = c.runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts, vec!["ARREST for ", "01/30/17"]);
    }
}
