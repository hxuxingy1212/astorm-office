//! 风格样式模板提取：从一份 DOCX 提取页面/字体/段落样式，产出
//! 可复用的 `template.json`（page/theme/styles）与面向大模型的 `TEMPLATE.md` 说明。

use crate::error::Result;
use crate::model::style::{PageSetup, Style, Theme};
use crate::parse::document::Package;
use crate::parse::read_zip;
use crate::parse::xmltree::{parse_xml, XmlNode};
use crate::utils::units::{half_to_pt, twip_to_pt, LINE_SINGLE};
use std::collections::{BTreeMap, HashMap};

/// 提取结果
pub struct ExtractedTemplate {
    pub page: PageSetup,
    pub theme: Theme,
    /// 段落样式（Normal / HeadingN / Title / Subtitle / Caption / Quote / Code …）
    pub styles: BTreeMap<String, Style>,
    /// 字号等文字说明
    pub fonts_used: Vec<(String, usize)>,
    pub notes: Vec<String>,
}

/// 段落格式的有效值
#[derive(Clone, Default)]
struct Fmt {
    font: Option<String>,
    ea_font: Option<String>,
    size: Option<f64>,
    bold: Option<bool>,
    italic: Option<bool>,
    color: Option<String>,
    align: Option<String>,
    first_line: Option<f64>,
    line_spacing: Option<f64>,
    space_before: Option<f64>,
    space_after: Option<f64>,
}

impl Fmt {
    /// 用 `over` 覆盖自身中为 None 的字段
    fn merge(&self, over: &Fmt) -> Fmt {
        macro_rules! m {
            ($f:ident) => {
                over.$f.clone().or_else(|| self.$f.clone())
            };
        }
        Fmt {
            font: m!(font),
            ea_font: m!(ea_font),
            size: m!(size),
            bold: m!(bold),
            italic: m!(italic),
            color: m!(color),
            align: m!(align),
            first_line: m!(first_line),
            line_spacing: m!(line_spacing),
            space_before: m!(space_before),
            space_after: m!(space_after),
        }
    }

    /// 取非空字段构成命名字段对（用于聚合统计）
    fn fields(&self) -> Vec<(&'static str, String)> {
        let mut v = Vec::new();
        if let Some(x) = &self.font {
            v.push(("font", x.clone()));
        }
        if let Some(x) = &self.ea_font {
            v.push(("ea_font", x.clone()));
        }
        if let Some(x) = self.size {
            v.push(("size", format!("{x}")));
        }
        if let Some(x) = self.bold {
            v.push(("bold", format!("{x}")));
        }
        if let Some(x) = self.italic {
            v.push(("italic", format!("{x}")));
        }
        if let Some(x) = &self.color {
            v.push(("color", x.clone()));
        }
        if let Some(x) = &self.align {
            v.push(("align", x.clone()));
        }
        v
    }
}

/// 样式定义（styles.xml）
struct StyleDef {
    based_on: Option<String>,
    fmt: Fmt,
    name: Option<String>,
    outline: Option<u8>,
}

fn text_of(files: &Package, name: &str) -> Option<String> {
    files
        .get(name)
        .map(|b| String::from_utf8_lossy(b).to_string())
}

/// 解析 `w:rPr` 为 Fmt
fn parse_rpr(rpr: &XmlNode) -> Fmt {
    let mut f = Fmt::default();
    if let Some(fonts) = rpr.child("w:rFonts") {
        if let Some(a) = fonts.attr("w:ascii").or_else(|| fonts.attr("w:hAnsi")) {
            f.font = Some(a.to_string());
        }
        if let Some(e) = fonts.attr("w:eastAsia") {
            f.ea_font = Some(e.to_string());
        }
    }
    if let Some(b) = rpr.child("w:b") {
        f.bold = Some(
            b.attr("w:val")
                .map(|v| v != "0" && v != "false")
                .unwrap_or(true),
        );
    }
    if let Some(i) = rpr.child("w:i") {
        f.italic = Some(
            i.attr("w:val")
                .map(|v| v != "0" && v != "false")
                .unwrap_or(true),
        );
    }
    if let Some(sz) = rpr
        .child("w:sz")
        .and_then(|s| s.attr("w:val"))
        .and_then(|v| v.parse::<i64>().ok())
    {
        f.size = Some(half_to_pt(sz));
    }
    if let Some(c) = rpr.child("w:color").and_then(|c| c.attr("w:val")) {
        if c != "auto" {
            f.color = Some(c.to_string());
        }
    }
    f
}

/// 解析 `w:pPr` 为 Fmt
fn parse_ppr(ppr: &XmlNode) -> Fmt {
    let mut f = Fmt::default();
    if let Some(jc) = ppr.child("w:jc").and_then(|j| j.attr("w:val")) {
        f.align = Some(if jc == "both" {
            "justify".into()
        } else {
            jc.to_string()
        });
    }
    if let Some(ind) = ppr.child("w:ind") {
        if let Some(v) = ind.attr("w:firstLine").and_then(|v| v.parse::<i64>().ok()) {
            if v != 0 {
                f.first_line = Some(twip_to_pt(v));
            }
        }
    }
    if let Some(sp) = ppr.child("w:spacing") {
        if let Some(v) = sp.attr("w:line").and_then(|v| v.parse::<f64>().ok()) {
            if v != 0.0 {
                f.line_spacing = Some(v / LINE_SINGLE);
            }
        }
        if let Some(v) = sp.attr("w:before").and_then(|v| v.parse::<i64>().ok()) {
            if v != 0 {
                f.space_before = Some(twip_to_pt(v));
            }
        }
        if let Some(v) = sp.attr("w:after").and_then(|v| v.parse::<i64>().ok()) {
            if v != 0 {
                f.space_after = Some(twip_to_pt(v));
            }
        }
    }
    // 段落标记运行属性（继承给 run）
    if let Some(rpr) = ppr.child("w:rPr") {
        f = f.merge(&parse_rpr(rpr));
    }
    f
}

fn style_id(p: &XmlNode) -> Option<String> {
    p.child("w:pPr")
        .and_then(|ppr| ppr.child("w:pStyle"))
        .and_then(|s| s.attr("w:val"))
        .map(|s| s.to_string())
}

/// 解析 styles.xml：docDefaults 与各段落样式定义
fn parse_style_defs(files: &Package) -> (Fmt, HashMap<String, StyleDef>) {
    let mut defaults = Fmt::default();
    let mut defs = HashMap::new();
    let Some(txt) = text_of(files, "word/styles.xml") else {
        return (defaults, defs);
    };
    let Ok(root) = parse_xml(&txt) else {
        return (defaults, defs);
    };
    if let Some(rpr) = root
        .child("w:docDefaults")
        .and_then(|d| d.child("w:rPrDefault"))
        .and_then(|d| d.child("w:rPr"))
    {
        defaults = defaults.merge(&parse_rpr(rpr));
    }
    for st in root.children_named("w:style") {
        if st.attr("w:type") != Some("paragraph") {
            continue;
        }
        let Some(id) = st.attr("w:styleId") else {
            continue;
        };
        let based_on = st
            .child("w:basedOn")
            .and_then(|b| b.attr("w:val"))
            .map(|s| s.to_string());
        let mut fmt = Fmt::default();
        // 直接子节点 rPr / pPr（stylesheet 规范：二者为兄弟）
        if let Some(rpr) = st.child("w:rPr") {
            fmt = fmt.merge(&parse_rpr(rpr));
        }
        // 兼容：某些生成器把 rPr 嵌在 pPr 内
        if let Some(ppr) = st.child("w:pPr") {
            fmt = fmt.merge(&parse_ppr(ppr));
            if let Some(rpr) = ppr.child("w:rPr") {
                fmt = fmt.merge(&parse_rpr(rpr));
            }
        }
        defs.insert(
            id.to_string(),
            StyleDef {
                based_on,
                fmt,
                name: st
                    .child("w:name")
                    .and_then(|n| n.attr("w:val"))
                    .map(|s| s.to_string()),
                outline: st
                    .child("w:pPr")
                    .and_then(|p| p.child("w:outlineLvl"))
                    .and_then(|o| o.attr("w:val"))
                    .and_then(|v| v.parse::<u8>().ok()),
            },
        );
    }
    (defaults, defs)
}

/// 沿 basedOn 链计算样式的有效格式
fn effective(id: &str, defs: &HashMap<String, StyleDef>, defaults: &Fmt) -> Fmt {
    let mut chain = Vec::new();
    let mut cur = Some(id.to_string());
    let mut guard = 0;
    while let Some(c) = cur {
        if guard > 20 {
            break;
        }
        guard += 1;
        if let Some(d) = defs.get(&c) {
            chain.push(d.fmt.clone());
            cur = d.based_on.clone();
        } else {
            break;
        }
    }
    let mut f = defaults.clone();
    for fmt in chain.iter().rev() {
        f = f.merge(fmt);
    }
    f
}

/// 递归找段落中的第一个 run
fn first_run(p: &XmlNode) -> Option<&XmlNode> {
    fn find(n: &XmlNode) -> Option<&XmlNode> {
        if n.name == "w:r" {
            return Some(n);
        }
        for c in &n.children {
            if let Some(r) = find(c) {
                return Some(r);
            }
        }
        None
    }
    find(p)
}

/// 判定段落所属样式类（依据 styleId / 样式名 / outlineLvl）
fn classify(style: &Option<String>, defs: &HashMap<String, StyleDef>) -> String {
    let Some(sid) = style else {
        return "Normal".to_string();
    };
    let lower_id = sid.to_ascii_lowercase();
    if lower_id.contains("header") || lower_id.contains("footer") {
        return "Normal".to_string();
    }
    if let Some(d) = defs.get(sid) {
        // 1) outlineLvl → 标题级别
        if let Some(o) = d.outline {
            if o <= 8 {
                return format!("Heading{}", (o + 1).min(6));
            }
        }
        // 2) 样式名
        if let Some(name) = &d.name {
            let n = name.to_ascii_lowercase();
            if let Some(pos) = n.find("heading") {
                let rest = n[pos + 7..].trim();
                let lvl = rest
                    .split(|c: char| !c.is_ascii_digit())
                    .find(|t| !t.is_empty())
                    .and_then(|t| t.parse::<u8>().ok())
                    .unwrap_or(1);
                return format!("Heading{}", lvl.clamp(1, 6));
            }
            if name.contains("标题") {
                let lvl = name
                    .chars()
                    .find(|c| c.is_ascii_digit())
                    .and_then(|c| c.to_digit(10))
                    .unwrap_or(1) as u8;
                return format!("Heading{}", lvl.clamp(1, 6));
            }
            if n == "title" || name == "标题" {
                return "Title".to_string();
            }
            if n == "subtitle" {
                return "Subtitle".to_string();
            }
            if n.contains("caption") || n.contains("题注") {
                return "Caption".to_string();
            }
            if n.contains("quote") || n.contains("引用") {
                return "Quote".to_string();
            }
            if n.contains("code") || n.contains("代码") {
                return "Code".to_string();
            }
            if n.contains("toc") {
                return "TOC".to_string();
            }
        }
    }
    // 3) styleId 前缀兜底
    if let Some(lvl) = sid.strip_prefix("Heading") {
        if let Ok(v) = lvl.parse::<u8>() {
            return format!("Heading{}", v.clamp(1, 6));
        }
    }
    if sid.eq_ignore_ascii_case("Title") {
        return "Title".to_string();
    }
    "Normal".to_string()
}

/// 遍历所有段落（穿透表格/内容控件等），返回 (class, Fmt)
fn sample_paragraphs(
    node: &XmlNode,
    defs: &HashMap<String, StyleDef>,
    defaults: &Fmt,
    out: &mut Vec<(String, Fmt)>,
) {
    if node.name == "w:p" {
        let style = style_id(node);
        let mut f = match &style {
            Some(id) => effective(id, defs, defaults),
            None => defaults.clone(),
        };
        if let Some(ppr) = node.child("w:pPr") {
            f = f.merge(&parse_ppr(ppr));
        }
        if let Some(r) = first_run(node) {
            if let Some(rpr) = r.child("w:rPr") {
                f = f.merge(&parse_rpr(rpr));
            }
        }
        out.push((classify(&style, defs), f));
        return;
    }
    for c in &node.children {
        sample_paragraphs(c, defs, defaults, out);
    }
}

/// 取一组值中的众数
fn mode_str(values: &[String]) -> Option<String> {
    if values.is_empty() {
        return None;
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for v in values {
        *counts.entry(v.as_str()).or_insert(0) += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(_, c)| *c)
        .map(|(v, _)| v.to_string())
}

fn mode_bool(values: &[bool]) -> Option<bool> {
    if values.is_empty() {
        return None;
    }
    let t = values.iter().filter(|b| **b).count();
    Some(t * 2 >= values.len())
}

fn mode_f64(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    // 保留一位小数后取众数
    let strs: Vec<String> = values.iter().map(|v| format!("{v:.1}")).collect();
    mode_str(&strs).and_then(|s| s.parse().ok())
}

fn aggregate(samples: &[Fmt]) -> Style {
    let pick = |name: &str| -> Vec<String> {
        samples
            .iter()
            .flat_map(|f| f.fields())
            .filter(|(k, _)| *k == name)
            .map(|(_, v)| v)
            .collect()
    };
    let size: Vec<f64> = samples.iter().filter_map(|f| f.size).collect();
    let line: Vec<f64> = samples.iter().filter_map(|f| f.line_spacing).collect();
    let first: Vec<f64> = samples.iter().filter_map(|f| f.first_line).collect();
    let sb: Vec<f64> = samples.iter().filter_map(|f| f.space_before).collect();
    let sa: Vec<f64> = samples.iter().filter_map(|f| f.space_after).collect();
    let bold: Vec<bool> = samples.iter().filter_map(|f| f.bold).collect();
    let italic: Vec<bool> = samples.iter().filter_map(|f| f.italic).collect();
    Style {
        font_size: mode_f64(&size),
        bold: mode_bool(&bold),
        italic: mode_bool(&italic),
        underline: None,
        strike: None,
        color: mode_str(&pick("color")),
        align: {
            let a = mode_str(&pick("align"));
            // 左对齐为默认，不落盘
            a.filter(|x| x != "left")
        },
        line_spacing: mode_f64(&line),
        line_pt: None,
        line_rule: None,
        first_line_indent: mode_f64(&first),
        space_before: mode_f64(&sb),
        space_after: mode_f64(&sa),
        before_autospacing: None,
        after_autospacing: None,
        font_family: mode_str(&pick("font")),
        east_asia_font: mode_str(&pick("ea_font")),
        contextual_spacing: None,
        based_on: None,
        indent: None,
        hanging_indent: None,
        tabs: None,
        auto_space_de: None,
        auto_space_dn: None,
        adjust_right_ind: None,
        snap_to_grid: None,
        num_id: None,
        rtl: None,
        cs_font: None,
    }
}

/// 从 DOCX 提取模板
pub fn extract_template(input: &str) -> Result<ExtractedTemplate> {
    let doc = crate::parse::parse(input)?;
    let files = read_zip(input)?;
    let (defaults, defs) = parse_style_defs(&files);

    let mut samples: Vec<(String, Fmt)> = Vec::new();
    let main = crate::parse::document::main_document_part(&files);
    if let Some(txt) = text_of(&files, &main) {
        if let Ok(root) = parse_xml(&txt) {
            sample_paragraphs(&root, &defs, &defaults, &mut samples);
        }
    }

    // 按 class 聚合
    let mut by_class: BTreeMap<String, Vec<Fmt>> = BTreeMap::new();
    let mut font_counts: HashMap<String, usize> = HashMap::new();
    for (class, f) in &samples {
        by_class.entry(class.clone()).or_default().push(f.clone());
        for (k, v) in f.fields() {
            if k == "font" {
                *font_counts.entry(v).or_insert(0) += 1;
            }
        }
    }

    let mut styles = BTreeMap::new();
    for (class, fmts) in &by_class {
        // TOC 级样式单独命名
        let id = if class == "TOC" {
            "TOC1".to_string()
        } else {
            class.clone()
        };
        let mut st = aggregate(fmts);
        // 正文样式不整体加粗/斜体（强调由 run 承担，避免误判污染整篇正文）
        if id == "Normal" {
            st.bold = None;
            st.italic = None;
            // 正文字号对齐只保留两端对齐；居中/右对齐多由标题或字段误判，视为默认左对齐
            if let Some(a) = &st.align {
                if a != "justify" {
                    st.align = None;
                }
            }
        }
        styles.insert(id, st);
    }

    // 主题字体（回退顺序：正文样式 → docDefaults → theme1.xml → 默认）
    let mut normal = styles.get("Normal").cloned().unwrap_or_default();
    if normal.font_family.is_none() {
        normal.font_family = defaults
            .font
            .clone()
            .or_else(|| doc.theme.minor_font.clone());
    }
    if normal.east_asia_font.is_none() {
        normal.east_asia_font = defaults
            .ea_font
            .clone()
            .or_else(|| doc.theme.east_asia_font.clone());
    }
    if let Some(n) = styles.get_mut("Normal") {
        n.font_family = normal.font_family.clone();
        n.east_asia_font = normal.east_asia_font.clone();
    }
    let heading1 = styles.get("Heading1").cloned().unwrap_or_default();
    let minor = normal
        .font_family
        .clone()
        .or_else(|| doc.theme.minor_font.clone())
        .unwrap_or_else(|| "Calibri".to_string());
    let ea = normal
        .east_asia_font
        .clone()
        .or_else(|| doc.theme.east_asia_font.clone())
        .unwrap_or_else(|| "宋体".to_string());
    let major = heading1
        .font_family
        .clone()
        .or_else(|| doc.theme.major_font.clone())
        .unwrap_or_else(|| minor.clone());
    let mut colors = BTreeMap::new();
    if let Some(c) = &heading1.color {
        colors.insert("heading".to_string(), c.clone());
    }
    let theme = Theme {
        major_font: Some(major.clone()),
        minor_font: Some(minor.clone()),
        east_asia_font: Some(ea.clone()),
        heading_font: Some(major.clone()),
        colors: if colors.is_empty() {
            None
        } else {
            Some(colors)
        },
    };

    let mut fonts_used: Vec<(String, usize)> = font_counts.into_iter().collect();
    fonts_used.sort_by_key(|f| std::cmp::Reverse(f.1));
    fonts_used.truncate(8);

    let mut notes = Vec::new();
    notes.push(format!("正文西文字体：{}；中文字体：{}", minor, ea));
    notes.push(format!("标题字体：{}", major));
    if let Some(sz) = normal.font_size {
        notes.push(format!("正文字号：{} pt", sz));
    }
    if let Some(h) = styles.get("Heading1").and_then(|s| s.font_size) {
        notes.push(format!("一级标题字号：{} pt", h));
    }
    if let Some(sp) = normal.line_spacing {
        notes.push(format!("正文行距：{:.2} 倍", sp));
    }
    if let Some(i) = normal.first_line_indent {
        if i > 0.0 {
            notes.push(format!("正文首行缩进：{} pt", i));
        }
    }
    if let Some(c) = styles.get("Heading1").and_then(|s| s.color.clone()) {
        notes.push(format!("标题颜色：#{}", c));
    }

    Ok(ExtractedTemplate {
        page: doc.page,
        theme,
        styles,
        fonts_used,
        notes,
    })
}

// ---------------- 说明文档 ----------------

fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

/// 生成面向大模型的模板说明（Markdown）
pub fn describe_template(ext: &ExtractedTemplate, source: &str) -> String {
    let mut md = String::new();
    md.push_str("# 文档风格模板说明\n\n");
    md.push_str(&format!("> 由 `json2docx extract` 从 `{source}` 提取。\n"));
    md.push_str("> 用途：让大模型据此生成**同风格**的其它 Word 文档。使用时把下面的 `page` / `theme` / `styles` 复制进新文档的 `document.json`，再用 `repack` 生成。\n\n");

    let (w, h) = ext.page.size_pt();
    md.push_str("## 页面\n\n");
    md.push_str(&format!(
        "- 纸张：{}（{:.0}×{:.0} pt），方向：{}\n",
        ext.page.size, w, h, ext.page.orientation
    ));
    let m = &ext.page.margins;
    md.push_str(&format!(
        "- 页边距（上/下/左/右，pt）：{} / {} / {} / {}\n\n",
        fmt_num(m.top),
        fmt_num(m.bottom),
        fmt_num(m.left),
        fmt_num(m.right)
    ));

    md.push_str("## 字体\n\n");
    md.push_str(&format!(
        "- 西文：{}，中文：{}，标题：{}\n",
        ext.theme.minor_font.clone().unwrap_or_default(),
        ext.theme.east_asia_font.clone().unwrap_or_default(),
        ext.theme.heading_font.clone().unwrap_or_default()
    ));
    if !ext.fonts_used.is_empty() {
        let list: Vec<String> = ext
            .fonts_used
            .iter()
            .map(|(f, c)| format!("{f}×{c}"))
            .collect();
        md.push_str(&format!(
            "- 文档中出现的字体（频次）：{}\n",
            list.join("、")
        ));
    }
    md.push('\n');

    md.push_str("## 段落样式\n\n");
    md.push_str("| 样式 | 字号(pt) | 粗体 | 颜色 | 对齐 | 行距 | 首行缩进(pt) | 段前/段后(pt) | 西文/中文字体 |\n");
    md.push_str("|------|---------|------|------|------|------|-------------|--------------|--------------|\n");
    for (id, s) in &ext.styles {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} / {} | {} / {} |\n",
            id,
            s.font_size.map(fmt_num).unwrap_or_else(|| "-".into()),
            match s.bold {
                Some(true) => "是",
                Some(false) => "否",
                None => "-",
            },
            s.color
                .clone()
                .map(|c| format!("#{c}"))
                .unwrap_or_else(|| "-".into()),
            s.align.clone().unwrap_or_else(|| "-".into()),
            s.line_spacing
                .map(|v| format!("{v:.2}"))
                .unwrap_or_else(|| "-".into()),
            s.first_line_indent
                .map(fmt_num)
                .unwrap_or_else(|| "-".into()),
            s.space_before.map(fmt_num).unwrap_or_else(|| "-".into()),
            s.space_after.map(fmt_num).unwrap_or_else(|| "-".into()),
            s.font_family.clone().unwrap_or_else(|| "-".into()),
            s.east_asia_font.clone().unwrap_or_else(|| "-".into()),
        ));
    }
    md.push('\n');

    md.push_str("## 观察\n\n");
    for n in &ext.notes {
        md.push_str(&format!("- {n}\n"));
    }
    md.push('\n');

    md.push_str("## 如何使用\n\n");
    md.push_str("把 `template.json` 中的 `page` / `theme` / `styles` 合并进新文档的 `document.json`（用户显式字段优先）：\n\n");
    md.push_str("```json\n");
    md.push_str("{\n");
    md.push_str("  \"page\": { ... },\n");
    md.push_str("  \"theme\": { ... },\n");
    md.push_str("  \"styles\": { \"Normal\": { ... }, \"Heading1\": { ... } },\n");
    md.push_str("  \"parts\": [ { \"blocks\": [ { \"type\": \"heading\", \"level\": 1, \"text\": \"标题\" } ] } ]\n");
    md.push_str("}\n");
    md.push_str("```\n\n");
    md.push_str("```bash\n");
    md.push_str("json2docx repack mydoc/ -o mydoc.docx\n");
    md.push_str("json2docx render mydoc.docx / -o preview.png\n");
    md.push_str("```\n");

    md
}

/// 序列化 template.json 内容（page/theme/styles）
pub fn template_json(ext: &ExtractedTemplate) -> Result<serde_json::Value> {
    Ok(serde_json::json!({
        "page": ext.page,
        "theme": ext.theme,
        "styles": ext.styles,
        "_notes": ext.notes,
    }))
}
