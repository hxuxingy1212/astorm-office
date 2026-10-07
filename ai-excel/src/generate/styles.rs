use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::model::cell::Cell;
use crate::model::sheet::ConditionalFormat;
use crate::model::style::{Alignment, Border, Font, StyleDef};
use crate::model::workbook::{resolve_style, Workbook};
use crate::utils::constants::NS_MAIN;
use crate::utils::xml::{esc_xml, to_argb};

/// Builtin number-format ids (ECMA-376 18.8.30).
const BUILTIN: &[(u32, &str)] = &[
    (0, "General"),
    (1, "0"),
    (2, "0.00"),
    (3, "#,##0"),
    (4, "#,##0.00"),
    (9, "0%"),
    (10, "0.00%"),
    (11, "0.00E+00"),
    (12, "# ?/?"),
    (13, "# ??/??"),
    (14, "mm-dd-yy"),
    (15, "d-mmm-yy"),
    (16, "d-mmm"),
    (17, "mmm-yy"),
    (18, "h:mm AM/PM"),
    (19, "h:mm:ss AM/PM"),
    (20, "h:mm"),
    (21, "h:mm:ss"),
    (22, "m/d/yy h:mm"),
    (49, "@"),
];

pub fn builtin_id(code: &str) -> Option<u32> {
    BUILTIN.iter().find(|(_, c)| *c == code).map(|(id, _)| *id)
}

pub fn builtin_code(id: u32) -> Option<&'static str> {
    BUILTIN.iter().find(|(i, _)| *i == id).map(|(_, c)| *c)
}

/// Compute the effective style of a cell: its referenced/inline style merged
/// with the cell-level `number_format`, with `border.all` expanded.
pub fn effective_style(wb: &Workbook, cell: &Cell) -> StyleDef {
    let mut s = resolve_style(wb, cell).unwrap_or_default();
    if let Some(nf) = &cell.number_format {
        s.number_format = Some(nf.clone());
    }
    if let Some(b) = &mut s.border {
        b.expand();
    }
    s
}

fn style_key(s: &StyleDef) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum FillKind {
    #[default]
    None,
    Gray125,
    Solid(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Xf {
    pub num_fmt_id: u32,
    pub font_id: u32,
    pub fill_id: u32,
    pub border_id: u32,
    pub alignment: Option<Alignment>,
    pub locked: Option<bool>,
}

pub struct StyleTable {
    pub num_fmts: Vec<(u32, String)>,
    pub fonts: Vec<Font>,
    pub fills: Vec<FillKind>,
    pub borders: Vec<Border>,
    pub xfs: Vec<Xf>,
    pub dxfs: Vec<Dxf>,
    xf_index: HashMap<String, u32>,
    dxf_index: HashMap<String, u32>,
    num_fmt_map: HashMap<String, u32>,
    next_custom_id: u32,
    font_index: HashMap<String, u32>,
    fill_index: HashMap<String, u32>,
    border_index: HashMap<String, u32>,
}

/// A differential format used by conditional formatting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Dxf {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<Font>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
}

impl StyleTable {
    pub fn build(wb: &Workbook) -> Self {
        let mut t = StyleTable {
            num_fmts: Vec::new(),
            fonts: vec![wb.default_font.clone().unwrap_or_else(|| Font {
                name: Some("Calibri".into()),
                size: Some(11.0),
                ..Default::default()
            })],
            fills: vec![FillKind::None, FillKind::Gray125],
            borders: vec![Border::default()],
            xfs: vec![Xf::default()],
            dxfs: Vec::new(),
            xf_index: HashMap::new(),
            dxf_index: HashMap::new(),
            num_fmt_map: HashMap::new(),
            next_custom_id: 164,
            font_index: HashMap::new(),
            fill_index: HashMap::new(),
            border_index: HashMap::new(),
        };
        t.xf_index.insert(style_key(&StyleDef::default()), 0);
        for sheet in &wb.sheets {
            for column in &sheet.columns {
                if let Some(s) = &column.style {
                    if !s.is_empty() {
                        t.intern_style(s);
                    }
                }
            }
            for row in &sheet.rows {
                if let Some(s) = &row.style {
                    if !s.is_empty() {
                        t.intern_style(s);
                    }
                }
                for cell in &row.cells {
                    let s = effective_style(wb, cell);
                    if !s.is_empty() {
                        t.intern_style(&s);
                    }
                }
            }
            for cf in &sheet.conditional_formats {
                if cf.font.is_some() || cf.fill.is_some() {
                    t.intern_dxf(cf);
                }
            }
        }
        t
    }

    /// Look up the cell-format index for a style, if interned.
    pub fn style_id(&self, s: &StyleDef) -> Option<u32> {
        self.xf_index.get(&style_key(s)).copied()
    }

    pub fn resolve(&self, wb: &Workbook, cell: &Cell) -> u32 {
        let s = effective_style(wb, cell);
        if s.is_empty() {
            return 0;
        }
        *self.xf_index.get(&style_key(&s)).unwrap_or(&0)
    }

    fn intern_style(&mut self, s: &StyleDef) -> u32 {
        let key = style_key(s);
        if let Some(&i) = self.xf_index.get(&key) {
            return i;
        }
        let num_fmt_id = self.intern_num_fmt(s.number_format.as_deref());
        let font_id = match &s.font {
            Some(f) if !f.is_empty() => self.intern_font(f.clone()),
            _ => 0,
        };
        let fill_id = self.intern_fill(s.fill.as_deref());
        let border_id = self.intern_border(s.border.clone().unwrap_or_default());
        let xf = Xf {
            num_fmt_id,
            font_id,
            fill_id,
            border_id,
            alignment: s.alignment.clone(),
            locked: s.locked,
        };
        let idx = self.xfs.len() as u32;
        self.xfs.push(xf);
        self.xf_index.insert(key, idx);
        idx
    }

    fn intern_num_fmt(&mut self, code: Option<&str>) -> u32 {
        let Some(code) = code else { return 0 };
        if let Some(id) = builtin_id(code) {
            return id;
        }
        if let Some(&id) = self.num_fmt_map.get(code) {
            return id;
        }
        let id = self.next_custom_id;
        self.next_custom_id += 1;
        self.num_fmt_map.insert(code.to_string(), id);
        self.num_fmts.push((id, code.to_string()));
        id
    }

    fn intern_font(&mut self, font: Font) -> u32 {
        let key = serde_json::to_string(&font).unwrap_or_default();
        if let Some(&i) = self.font_index.get(&key) {
            return i;
        }
        let i = self.fonts.len() as u32;
        self.fonts.push(font);
        self.font_index.insert(key, i);
        i
    }

    fn intern_fill(&mut self, color: Option<&str>) -> u32 {
        let Some(color) = color else { return 0 };
        if color == "GRAY125" {
            return 1; // reserved gray125 fill
        }
        let c = color.trim().trim_start_matches('#').to_ascii_uppercase();
        if let Some(&i) = self.fill_index.get(&c) {
            return i;
        }
        let i = self.fills.len() as u32;
        self.fills.push(FillKind::Solid(c.clone()));
        self.fill_index.insert(c, i);
        i
    }

    fn intern_border(&mut self, border: Border) -> u32 {
        if border.is_empty() {
            return 0;
        }
        let key = serde_json::to_string(&border).unwrap_or_default();
        if let Some(&i) = self.border_index.get(&key) {
            return i;
        }
        let i = self.borders.len() as u32;
        self.borders.push(border);
        self.border_index.insert(key, i);
        i
    }

    fn intern_dxf(&mut self, cf: &ConditionalFormat) -> u32 {
        let key = dxf_key(cf);
        if let Some(&i) = self.dxf_index.get(&key) {
            return i;
        }
        let i = self.dxfs.len() as u32;
        self.dxfs.push(Dxf {
            font: cf.font.clone(),
            fill: cf.fill.clone(),
        });
        self.dxf_index.insert(key, i);
        i
    }

    pub fn dxf_id_for(&self, cf: &ConditionalFormat) -> Option<u32> {
        if cf.font.is_none() && cf.fill.is_none() {
            return None;
        }
        self.dxf_index.get(&dxf_key(cf)).copied()
    }

    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        out.push_str(&format!("<styleSheet xmlns=\"{NS_MAIN}\">"));

        if !self.num_fmts.is_empty() {
            out.push_str(&format!("<numFmts count=\"{}\">", self.num_fmts.len()));
            for (id, code) in &self.num_fmts {
                out.push_str(&format!(
                    "<numFmt numFmtId=\"{id}\" formatCode=\"{}\"/>",
                    esc_xml(code)
                ));
            }
            out.push_str("</numFmts>");
        }

        // Fonts
        out.push_str(&format!("<fonts count=\"{}\">", self.fonts.len()));
        for f in &self.fonts {
            out.push_str(&font_xml(f));
        }
        out.push_str("</fonts>");

        // Fills
        out.push_str(&format!("<fills count=\"{}\">", self.fills.len()));
        for fill in &self.fills {
            match fill {
                FillKind::None => out.push_str("<fill><patternFill patternType=\"none\"/></fill>"),
                FillKind::Gray125 => {
                    out.push_str("<fill><patternFill patternType=\"gray125\"/></fill>")
                }
                FillKind::Solid(c) => out.push_str(&format!(
                    "<fill><patternFill patternType=\"solid\"><fgColor rgb=\"{}\"/><bgColor indexed=\"64\"/></patternFill></fill>",
                    to_argb(c)
                )),
            }
        }
        out.push_str("</fills>");

        // Borders
        out.push_str(&format!("<borders count=\"{}\">", self.borders.len()));
        for b in &self.borders {
            let diag_attrs = format!(
                "{}{}",
                if b.diagonal_up {
                    " diagonalUp=\"1\""
                } else {
                    ""
                },
                if b.diagonal_down {
                    " diagonalDown=\"1\""
                } else {
                    ""
                }
            );
            out.push_str(&format!("<border{diag_attrs}>"));
            out.push_str(&border_side("left", b.left.as_deref(), b.color.as_deref()));
            out.push_str(&border_side(
                "right",
                b.right.as_deref(),
                b.color.as_deref(),
            ));
            out.push_str(&border_side("top", b.top.as_deref(), b.color.as_deref()));
            out.push_str(&border_side(
                "bottom",
                b.bottom.as_deref(),
                b.color.as_deref(),
            ));
            out.push_str(&border_side(
                "diagonal",
                b.diagonal.as_deref(),
                b.diagonal_color.as_deref(),
            ));
            out.push_str("</border>");
        }
        out.push_str("</borders>");

        // cellXfs
        out.push_str("<cellStyleXfs count=\"1\"><xf numFmtId=\"0\" fontId=\"0\" fillId=\"0\" borderId=\"0\"/></cellStyleXfs>");
        out.push_str(&format!("<cellXfs count=\"{}\">", self.xfs.len()));
        for xf in &self.xfs {
            let mut attrs = format!(
                " numFmtId=\"{}\" fontId=\"{}\" fillId=\"{}\" borderId=\"{}\" xfId=\"0\"",
                xf.num_fmt_id, xf.font_id, xf.fill_id, xf.border_id
            );
            if xf.num_fmt_id != 0 {
                attrs.push_str(" applyNumberFormat=\"1\"");
            }
            if xf.font_id != 0 {
                attrs.push_str(" applyFont=\"1\"");
            }
            if xf.fill_id != 0 {
                attrs.push_str(" applyFill=\"1\"");
            }
            if xf.border_id != 0 {
                attrs.push_str(" applyBorder=\"1\"");
            }
            let has_align = xf
                .alignment
                .as_ref()
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            if has_align {
                attrs.push_str(" applyAlignment=\"1\"");
            }
            if xf.locked.is_some() {
                attrs.push_str(" applyProtection=\"1\"");
            }
            let has_children = has_align || xf.locked.is_some();
            if !has_children {
                out.push_str(&format!("<xf{attrs}/>"));
            } else {
                out.push_str(&format!("<xf{attrs}>"));
                if let Some(a) = &xf.alignment {
                    if !a.is_empty() {
                        out.push_str(&alignment_xml(a));
                    }
                }
                if let Some(l) = xf.locked {
                    out.push_str(&format!(
                        "<protection locked=\"{}\"/>",
                        if l { 1 } else { 0 }
                    ));
                }
                out.push_str("</xf>");
            }
        }
        out.push_str("</cellXfs>");

        out.push_str("<cellStyles count=\"1\"><cellStyle name=\"Normal\" xfId=\"0\" builtinId=\"0\"/></cellStyles>");

        if !self.dxfs.is_empty() {
            out.push_str(&format!("<dxfs count=\"{}\">", self.dxfs.len()));
            for d in &self.dxfs {
                out.push_str("<dxf>");
                if let Some(f) = &d.font {
                    out.push_str(&font_xml(f));
                }
                if let Some(c) = &d.fill {
                    // 差分格式（dxf）用 bgColor 表示填充色（Excel/LO 约定）。
                    out.push_str(&format!(
                        "<fill><patternFill><bgColor rgb=\"{}\"/></patternFill></fill>",
                        to_argb(c)
                    ));
                }
                out.push_str("</dxf>");
            }
            out.push_str("</dxfs>");
        }

        out.push_str("</styleSheet>");
        out
    }
}

fn dxf_key(cf: &ConditionalFormat) -> String {
    let dxf = Dxf {
        font: cf.font.clone(),
        fill: cf.fill.clone(),
    };
    serde_json::to_string(&dxf).unwrap_or_default()
}

fn font_xml(f: &Font) -> String {
    let mut out = String::from("<font>");
    if f.bold {
        out.push_str("<b/>");
    }
    if f.italic {
        out.push_str("<i/>");
    }
    if f.strike {
        out.push_str("<strike/>");
    }
    if let Some(u) = &f.underline {
        if u == "single" {
            out.push_str("<u/>");
        } else if u != "none" {
            out.push_str(&format!("<u val=\"{}\"/>", esc_xml(u)));
        }
    }
    if let Some(sz) = f.size {
        out.push_str(&format!("<sz val=\"{}\"/>", trim_num(sz)));
    }
    if let Some(c) = &f.color {
        out.push_str(&format!("<color rgb=\"{}\"/>", to_argb(c)));
    }
    if let Some(name) = &f.name {
        out.push_str(&format!("<name val=\"{}\"/>", esc_xml(name)));
    }
    out.push_str("</font>");
    out
}

fn border_side(tag: &str, style: Option<&str>, color: Option<&str>) -> String {
    match style {
        None | Some("none") => format!("<{tag}/>"),
        Some(s) => {
            let c = color
                .map(|c| format!("<color rgb=\"{}\"/>", to_argb(c)))
                .unwrap_or_default();
            format!("<{tag} style=\"{}\">{c}</{tag}>", esc_xml(s))
        }
    }
}

fn alignment_xml(a: &Alignment) -> String {
    let mut attrs = String::new();
    if let Some(h) = &a.horizontal {
        attrs.push_str(&format!(" horizontal=\"{}\"", esc_xml(h)));
    }
    if let Some(v) = &a.vertical {
        attrs.push_str(&format!(" vertical=\"{}\"", esc_xml(v)));
    }
    if a.wrap_text {
        attrs.push_str(" wrapText=\"1\"");
    }
    if let Some(i) = a.indent {
        attrs.push_str(&format!(" indent=\"{i}\""));
    }
    if let Some(r) = a.text_rotation {
        attrs.push_str(&format!(" textRotation=\"{r}\""));
    }
    format!("<alignment{attrs}/>")
}

fn trim_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}
