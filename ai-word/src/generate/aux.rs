//! 生成其余 OOXML 部件：styles / numbering / settings / theme / fontTable /
//! webSettings / core / app / header / footer

use crate::generate::GenCtx;
use crate::model::style::Style;
use crate::utils::units::*;
use crate::utils::xml::{declaration, empty, esc, esc_attr};

const STYLES_NS: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"";
const THEME_NS: &str = "xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"";

fn theme_fonts(ctx: &GenCtx) -> (String, String, String, String) {
    let minor = ctx
        .doc
        .theme
        .minor_font
        .clone()
        .unwrap_or_else(|| "Calibri".to_string());
    let major = ctx
        .doc
        .theme
        .major_font
        .clone()
        .unwrap_or_else(|| "Calibri Light".to_string());
    let ea = ctx
        .doc
        .theme
        .east_asia_font
        .clone()
        .unwrap_or_else(|| "宋体".to_string());
    let heading = ctx
        .doc
        .theme
        .heading_font
        .clone()
        .unwrap_or_else(|| major.clone());
    (minor, major, ea, heading)
}

fn body_size(ctx: &GenCtx) -> f64 {
    ctx.doc
        .styles
        .get("Normal")
        .and_then(|s| s.font_size)
        .unwrap_or(11.0)
}

/// styles.xml
pub(crate) fn styles_xml(ctx: &GenCtx) -> String {
    let (minor, _major, ea, heading) = theme_fonts(ctx);
    let size = body_size(ctx);

    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!(
        "<w:styles {STYLES_NS} \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
         xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\" \
         xmlns:w15=\"http://schemas.microsoft.com/office/word/2012/wordml\" \
         xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
         xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\">"
    ));

    // docDefaults（优先保留原文档默认样式，否则用主题字体 + 紧凑间距）
    let def = ctx.doc.defaults.clone().unwrap_or_default();
    let dm = def.font_family.clone().unwrap_or_else(|| minor.clone());
    let de = def.east_asia_font.clone().unwrap_or_else(|| ea.clone());
    let dsz = def
        .font_size
        .map(pt_to_half)
        .unwrap_or_else(|| pt_to_half(size));
    let mut drpr = format!(
        "<w:rFonts w:ascii=\"{m}\" w:hAnsi=\"{m}\" w:eastAsia=\"{e}\" w:cs=\"{m}\"/>\
         <w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/>",
        m = esc_attr(&dm),
        e = esc_attr(&de),
        sz = dsz
    );
    if def.bold == Some(true) {
        drpr.push_str("<w:b/><w:bCs/>");
    }
    if def.italic == Some(true) {
        drpr.push_str("<w:i/><w:iCs/>");
    }
    if let Some(c) = &def.color {
        drpr.push_str(&empty(
            "w:color",
            &[("w:val", crate::utils::xml::color_hex(c))],
        ));
    }
    let mut dppr = String::new();
    let mut dsp = Vec::new();
    if let Some(v) = def.space_before {
        dsp.push(("w:before", pt_to_twip(v).to_string()));
    }
    if let Some(v) = def.space_after {
        dsp.push(("w:after", pt_to_twip(v).to_string()));
    }
    if let Some(v) = def.line_spacing {
        dsp.push(("w:line", line_to_xml(v).to_string()));
        dsp.push(("w:lineRule", "auto".to_string()));
    }
    if dsp.is_empty() {
        // 无原始默认：紧凑（无段后距、单倍行距）
        dsp.push(("w:after", "0".to_string()));
        dsp.push(("w:line", "240".to_string()));
        dsp.push(("w:lineRule", "auto".to_string()));
    }
    if !dsp.is_empty() {
        dppr.push_str(&empty("w:spacing", &dsp));
    }
    out.push_str(&format!(
        "<w:docDefaults><w:rPrDefault><w:rPr>{drpr}</w:rPr></w:rPrDefault>\
         <w:pPrDefault><w:pPr>{dppr}</w:pPr></w:pPrDefault></w:docDefaults>"
    ));

    // Normal
    out.push_str(&format!(
        "<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\">\
         <w:name w:val=\"Normal\"/><w:qFormat/>{}</w:style>",
        style_body(
            ctx.doc.styles.get("Normal").unwrap_or(&Style::default()),
            ""
        )
    ));

    // 内建标题/副标题/目录标题样式
    // 内建段落样式（缺失时给默认；若模板已定义则由下方“其他命名样式”输出）
    for id in [
        "Title",
        "Subtitle",
        "TOCHeading",
        "Caption",
        "Quote",
        "Code",
        "Reference",
    ] {
        if ctx.doc.styles.contains_key(id) {
            continue;
        }
        let st = match id {
            "Title" => Style {
                font_size: Some(22.0),
                bold: Some(true),
                align: Some("center".to_string()),
                ..Default::default()
            },
            "Subtitle" => Style {
                font_size: Some(14.0),
                align: Some("center".to_string()),
                ..Default::default()
            },
            "TOCHeading" => Style {
                font_size: Some(16.0),
                bold: Some(true),
                align: Some("left".to_string()),
                ..Default::default()
            },
            "Caption" => Style {
                font_size: Some(10.0),
                italic: Some(true),
                align: Some("center".to_string()),
                ..Default::default()
            },
            "Quote" => Style {
                italic: Some(true),
                align: Some("left".to_string()),
                ..Default::default()
            },
            "Code" => Style {
                font_family: Some("Consolas".to_string()),
                font_size: Some(10.0),
                ..Default::default()
            },
            _ => Style::default(),
        };
        let name = if id == "TOCHeading" {
            "TOC Heading"
        } else {
            id
        };
        let base = st.based_on.clone().unwrap_or_else(|| "Normal".to_string());
        out.push_str(&format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"{id}\"><w:name w:val=\"{name}\"/>\
             <w:basedOn w:val=\"{base}\"/><w:next w:val=\"Normal\"/><w:qFormat/>{}</w:style>",
            style_body(&st, "")
        ));
    }

    // 目录各级样式
    for (id, name, lvl) in [
        ("TOC1", "toc 1", 1u32),
        ("TOC2", "toc 2", 2),
        ("TOC3", "toc 3", 3),
    ] {
        let indent = pt_to_twip(((lvl - 1) as f64) * 12.0);
        out.push_str(&format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"{id}\"><w:name w:val=\"{name}\"/>\
             <w:basedOn w:val=\"Normal\"/><w:qFormat/>\
             <w:pPr><w:ind w:left=\"{indent}\"/><w:tabs><w:tab w:val=\"right\" w:leader=\"dot\" w:pos=\"8000\"/></w:tabs></w:pPr></w:style>"
        ));
    }

    // Heading 1..9
    for lvl in 1..=9u8 {
        let id = format!("Heading{lvl}");
        let fallback = Style {
            font_size: Some((18.0 - (lvl as f64 - 1.0) * 1.5).max(11.0)),
            bold: Some(true),
            color: Some("1F3864".to_string()),
            font_family: Some(heading.clone()),
            east_asia_font: Some(heading.clone()),
            ..Default::default()
        };
        let st = ctx.doc.styles.get(&id).cloned().unwrap_or(fallback);
        // 仅追加 keepNext/outlineLvl；spacing 由 style_body 统一输出，避免重复 <w:spacing>
        let extra = format!("<w:keepNext/><w:outlineLvl w:val=\"{}\"/>", lvl - 1);
        let base = st.based_on.clone().unwrap_or_else(|| "Normal".to_string());
        out.push_str(&format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"{id}\"><w:name w:val=\"heading {lvl}\"/>\
             <w:basedOn w:val=\"{base}\"/><w:next w:val=\"Normal\"/><w:qFormat/>{}</w:style>",
            style_body(&st, &extra)
        ));
    }

    // 其他命名样式
    for (id, st) in &ctx.doc.styles {
        if id == "Normal" || id.starts_with("Heading") {
            continue;
        }
        let (kind, name) = if id == "Hyperlink" || id == "CodeChar" {
            ("character", id.as_str())
        } else {
            ("paragraph", id.as_str())
        };
        let base = st.based_on.clone().unwrap_or_else(|| "Normal".to_string());
        out.push_str(&format!(
            "<w:style w:type=\"{kind}\" w:styleId=\"{id}\"><w:name w:val=\"{name}\"/>\
             <w:basedOn w:val=\"{base}\"/><w:qFormat/>{}</w:style>",
            style_body(st, "")
        ));
    }

    // 内建字符/表格样式
    out.push_str(
        "<w:style w:type=\"character\" w:styleId=\"Hyperlink\"><w:name w:val=\"Hyperlink\"/>\
         <w:rPr><w:color w:val=\"0563C1\"/><w:u w:val=\"single\"/></w:rPr></w:style>",
    );
    out.push_str("<w:style w:type=\"character\" w:styleId=\"CodeChar\"><w:name w:val=\"Code Char\"/>\
         <w:rPr><w:rFonts w:ascii=\"Consolas\" w:hAnsi=\"Consolas\" w:cs=\"Consolas\"/></w:rPr></w:style>");
    out.push_str(
        "<w:style w:type=\"character\" w:styleId=\"FootnoteReference\"><w:name w:val=\"footnote reference\"/>\
         <w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr></w:style>",
    );
    out.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"FootnoteText\"><w:name w:val=\"footnote text\"/>\
         <w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
         <w:rPr><w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/></w:rPr></w:style>",
    );
    out.push_str(
        "<w:style w:type=\"character\" w:styleId=\"EndnoteReference\"><w:name w:val=\"endnote reference\"/>\
         <w:rPr><w:vertAlign w:val=\"superscript\"/></w:rPr></w:style>",
    );
    out.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"EndnoteText\"><w:name w:val=\"endnote text\"/>\
         <w:basedOn w:val=\"Normal\"/><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
         <w:rPr><w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/></w:rPr></w:style>",
    );
    out.push_str(
        "<w:style w:type=\"character\" w:styleId=\"CommentReference\"><w:name w:val=\"comment reference\"/>\
         <w:rPr><w:sz w:val=\"16\"/></w:rPr></w:style>",
    );
    out.push_str(
        "<w:style w:type=\"paragraph\" w:styleId=\"CommentText\"><w:name w:val=\"comment text\"/>\
         <w:basedOn w:val=\"Normal\"/><w:rPr><w:sz w:val=\"18\"/><w:szCs w:val=\"18\"/></w:rPr></w:style>",
    );
    out.push_str(
        "<w:style w:type=\"character\" w:styleId=\"InsertedText\"><w:name w:val=\"Inserted Text\"/>\
         <w:rPr><w:color w:val=\"C00000\"/></w:rPr></w:style>",
    );
    out.push_str(
        "<w:style w:type=\"character\" w:styleId=\"DeletedText\"><w:name w:val=\"Deleted Text\"/>\
         <w:rPr><w:strike/><w:color w:val=\"C00000\"/></w:rPr></w:style>",
    );
    out.push_str(
        "<w:style w:type=\"table\" w:styleId=\"TableGrid\"><w:name w:val=\"Table Grid\"/>\
         <w:tblPr><w:tblBorders>\
         <w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"808080\"/>\
         <w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"808080\"/>\
         <w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"808080\"/>\
         <w:right w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"808080\"/>\
         <w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"808080\"/>\
         <w:insideV w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"808080\"/>\
         </w:tblBorders></w:tblPr></w:style>",
    );

    // 未建模样式（表格/字符/编号等，原样回放）
    for raw in &ctx.doc.raw_styles {
        out.push_str(raw);
    }

    out.push_str("</w:styles>");
    out
}

/// style 的 pPr + rPr
fn style_body(st: &Style, extra_ppr: &str) -> String {
    let mut ppr = String::new();
    ppr.push_str(extra_ppr);
    if st.contextual_spacing == Some(true) {
        ppr.push_str("<w:contextualSpacing/>");
    }
    if st.rtl == Some(true) {
        ppr.push_str("<w:bidi/>");
    }
    for (tag, v) in [
        ("w:autoSpaceDE", st.auto_space_de),
        ("w:autoSpaceDN", st.auto_space_dn),
        ("w:adjustRightInd", st.adjust_right_ind),
        ("w:snapToGrid", st.snap_to_grid),
    ] {
        if let Some(b) = v {
            ppr.push_str(&empty(
                tag,
                &[("w:val", if b { "1" } else { "0" }.to_string())],
            ));
        }
    }
    let mut spacing = Vec::new();
    if let Some(v) = st.space_before {
        spacing.push(("w:before", pt_to_twip(v).to_string()));
    }
    if let Some(v) = st.space_after {
        spacing.push(("w:after", pt_to_twip(v).to_string()));
    }
    if let Some(v) = st.line_pt {
        spacing.push(("w:line", pt_to_twip(v).to_string()));
        spacing.push((
            "w:lineRule",
            st.line_rule
                .clone()
                .unwrap_or_else(|| "atLeast".to_string()),
        ));
    } else if let Some(v) = st.line_spacing {
        spacing.push(("w:line", line_to_xml(v).to_string()));
        spacing.push(("w:lineRule", "auto".to_string()));
    }
    if !spacing.is_empty()
        || st.before_autospacing == Some(true)
        || st.after_autospacing == Some(true)
    {
        let mut inner = String::new();
        if st.before_autospacing == Some(true) {
            inner.push_str("<w:beforeAutospacing/>");
        }
        if st.after_autospacing == Some(true) {
            inner.push_str("<w:afterAutospacing/>");
        }
        if inner.is_empty() {
            ppr.push_str(&empty("w:spacing", &spacing));
        } else {
            let attrs: String = spacing
                .iter()
                .map(|(k, v)| format!(" {k}=\"{v}\""))
                .collect();
            ppr.push_str(&format!("<w:spacing{attrs}>{inner}</w:spacing>"));
        }
    }
    let mut ind = Vec::new();
    if let Some(v) = st.first_line_indent {
        ind.push(("w:firstLine", pt_to_twip(v).to_string()));
    }
    if let Some(v) = st.indent {
        ind.push(("w:left", pt_to_twip(v).to_string()));
    }
    if let Some(v) = st.hanging_indent {
        ind.push(("w:hanging", pt_to_twip(v).to_string()));
    }
    if !ind.is_empty() {
        ppr.push_str(&empty("w:ind", &ind));
    }
    if let Some(tabs) = &st.tabs {
        let mut t = String::from("<w:tabs>");
        for p in tabs {
            let mut a = vec![
                (
                    "w:val",
                    p.align.clone().unwrap_or_else(|| "left".to_string()),
                ),
                ("w:pos", pt_to_twip(p.pos).to_string()),
            ];
            if let Some(l) = &p.leader {
                a.push(("w:leader", l.clone()));
            }
            t.push_str(&empty("w:tab", &a));
        }
        t.push_str("</w:tabs>");
        ppr.push_str(&t);
    }
    if let Some(a) = &st.align {
        let jc = match a.as_str() {
            "center" => "center",
            "right" => "right",
            "justify" | "both" => "both",
            _ => "left",
        };
        ppr.push_str(&empty("w:jc", &[("w:val", jc.to_string())]));
    }

    let mut rpr = String::new();
    if st.font_family.is_some() || st.east_asia_font.is_some() || st.cs_font.is_some() {
        let f = st.font_family.clone().unwrap_or_default();
        let ea = st.east_asia_font.clone().unwrap_or_else(|| f.clone());
        let cs = st.cs_font.clone().unwrap_or_else(|| f.clone());
        let fa = if f.is_empty() {
            String::new()
        } else {
            format!(" w:ascii=\"{}\" w:hAnsi=\"{}\"", esc_attr(&f), esc_attr(&f))
        };
        let ea_a = if ea.is_empty() {
            String::new()
        } else {
            format!(" w:eastAsia=\"{}\"", esc_attr(&ea))
        };
        let csa = if cs.is_empty() {
            String::new()
        } else {
            format!(" w:cs=\"{}\"", esc_attr(&cs))
        };
        rpr.push_str(&format!("<w:rFonts{fa}{ea_a}{csa}/>"));
    }
    if st.bold == Some(true) {
        rpr.push_str("<w:b/><w:bCs/>");
    }
    if st.italic == Some(true) {
        rpr.push_str("<w:i/><w:iCs/>");
    }
    if st.underline == Some(true) {
        rpr.push_str("<w:u w:val=\"single\"/>");
    }
    if st.strike == Some(true) {
        rpr.push_str("<w:strike/>");
    }
    if let Some(c) = &st.color {
        rpr.push_str(&empty(
            "w:color",
            &[("w:val", crate::utils::xml::color_hex(c))],
        ));
    }
    if let Some(sz) = st.font_size {
        rpr.push_str(&empty("w:sz", &[("w:val", pt_to_half(sz).to_string())]));
        rpr.push_str(&empty("w:szCs", &[("w:val", pt_to_half(sz).to_string())]));
    }

    // stylesheet 中 pPr 与 rPr 为兄弟节点（rPr 不可嵌套在 pPr 内）
    let mut out = String::new();
    if !ppr.is_empty() {
        out.push_str(&format!("<w:pPr>{ppr}</w:pPr>"));
    }
    if !rpr.is_empty() {
        out.push_str(&format!("<w:rPr>{rpr}</w:rPr>"));
    }
    out
}

/// numbering.xml（numId 1 = 无序，numId 2 = 有序；numId>=3 为自定义格式）
pub(crate) fn numbering_xml(ctx: &GenCtx) -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!("<w:numbering {STYLES_NS}>"));

    // abstractNum 0：无序
    out.push_str(
        "<w:abstractNum w:abstractNumId=\"0\"><w:multiLevelType w:val=\"hybridMultilevel\"/>",
    );
    for lvl in 0..9 {
        let indent = 720 + lvl * 360;
        out.push_str(&format!(
            "<w:lvl w:ilvl=\"{lvl}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/>\
             <w:lvlText w:val=\"•\"/><w:lvlJc w:val=\"left\"/>\
             <w:pPr><w:ind w:left=\"{indent}\" w:hanging=\"360\"/></w:pPr>\
             <w:rPr><w:rFonts w:ascii=\"Symbol\" w:hAnsi=\"Symbol\" w:hint=\"default\"/></w:rPr></w:lvl>",
            lvl = lvl,
            indent = indent
        ));
    }
    out.push_str("</w:abstractNum>");

    // abstractNum 1：有序
    out.push_str(
        "<w:abstractNum w:abstractNumId=\"1\"><w:multiLevelType w:val=\"hybridMultilevel\"/>",
    );
    for lvl in 0..9 {
        let indent = 720 + lvl * 360;
        let fmt = if lvl % 3 == 0 {
            "decimal"
        } else if lvl % 3 == 1 {
            "lowerLetter"
        } else {
            "lowerRoman"
        };
        let text = format!("%{}", lvl + 1);
        out.push_str(&format!(
            "<w:lvl w:ilvl=\"{lvl}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{fmt}\"/>\
             <w:lvlText w:val=\"{text}.\"/><w:lvlJc w:val=\"left\"/>\
             <w:pPr><w:ind w:left=\"{indent}\" w:hanging=\"360\"/></w:pPr></w:lvl>",
            lvl = lvl,
            fmt = fmt,
            text = text,
            indent = indent
        ));
    }
    out.push_str("</w:abstractNum>");

    out.push_str("<w:num w:numId=\"1\"><w:abstractNumId w:val=\"0\"/></w:num>");
    out.push_str("<w:num w:numId=\"2\"><w:abstractNumId w:val=\"1\"/></w:num>");

    // 自定义编号（来自列表的 num_format/lvl_text/start/num_font）
    for (id, levels) in &ctx.num_defs {
        out.push_str(&format!(
            "<w:abstractNum w:abstractNumId=\"{id}\"><w:multiLevelType w:val=\"hybridMultilevel\"/>"
        ));
        for (lvl, (fmt, text, start, font)) in levels.iter().enumerate() {
            let indent = 720 + lvl as i64 * 360;
            out.push_str(&format!(
                "<w:lvl w:ilvl=\"{lvl}\"><w:start w:val=\"{start}\"/><w:numFmt w:val=\"{}\"/>\
                 <w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/>\
                 <w:pPr><w:ind w:left=\"{indent}\" w:hanging=\"360\"/></w:pPr>",
                esc_attr(fmt),
                esc_attr(text)
            ));
            if let Some(f) = font {
                out.push_str(&format!(
                    "<w:rPr><w:rFonts w:ascii=\"{}\" w:hAnsi=\"{}\" w:hint=\"default\"/></w:rPr>",
                    esc_attr(f),
                    esc_attr(f)
                ));
            }
            out.push_str("</w:lvl>");
        }
        out.push_str("</w:abstractNum>");
    }
    for (id, _) in &ctx.num_defs {
        out.push_str(&format!(
            "<w:num w:numId=\"{id}\"><w:abstractNumId w:val=\"{id}\"/></w:num>"
        ));
    }
    out.push_str("</w:numbering>");
    out
}

/// settings.xml（updateFields 必须在 compat 之前）
pub(crate) fn settings_xml(ctx: &GenCtx) -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!("<w:settings {STYLES_NS}>"));
    out.push_str("<w:updateFields w:val=\"true\"/>");
    out.push_str(&format!(
        "<w:defaultTabStop w:val=\"{}\"/>",
        ctx.doc.default_tab_stop.unwrap_or(420)
    ));
    if ctx.even_and_odd {
        out.push_str("<w:evenAndOddHeaders/>");
    }
    if ctx.doc.background.is_some() {
        out.push_str("<w:displayBackgroundShape/>");
    }
    out.push_str(
        "<w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat>",
    );
    out.push_str("</w:settings>");
    out
}

/// theme1.xml
pub(crate) fn theme_xml(ctx: &GenCtx) -> String {
    let (minor, major, ea, _heading) = theme_fonts(ctx);
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!(
        "<a:theme {THEME_NS} name=\"Office\"><a:themeElements>"
    ));
    out.push_str(
        "<a:clrScheme name=\"Office\">\
         <a:dk1><a:sysClr val=\"windowText\" lastClr=\"000000\"/></a:dk1>\
         <a:lt1><a:sysClr val=\"window\" lastClr=\"FFFFFF\"/></a:lt1>\
         <a:dk2><a:srgbClr val=\"44546A\"/></a:dk2>\
         <a:lt2><a:srgbClr val=\"E7E6E6\"/></a:lt2>\
         <a:accent1><a:srgbClr val=\"4472C4\"/></a:accent1>\
         <a:accent2><a:srgbClr val=\"ED7D31\"/></a:accent2>\
         <a:accent3><a:srgbClr val=\"A5A5A5\"/></a:accent3>\
         <a:accent4><a:srgbClr val=\"FFC000\"/></a:accent4>\
         <a:accent5><a:srgbClr val=\"5B9BD5\"/></a:accent5>\
         <a:accent6><a:srgbClr val=\"70AD47\"/></a:accent6>\
         <a:hlink><a:srgbClr val=\"0563C1\"/></a:hlink>\
         <a:folHlink><a:srgbClr val=\"954F72\"/></a:folHlink>\
         </a:clrScheme>",
    );
    out.push_str(&format!(
        "<a:fontScheme name=\"Office\"><a:majorFont><a:latin typeface=\"{major}\"/>\
         <a:ea typeface=\"{ea}\"/><a:cs typeface=\"\"/></a:majorFont>\
         <a:minorFont><a:latin typeface=\"{minor}\"/><a:ea typeface=\"{ea}\"/><a:cs typeface=\"\"/></a:minorFont></a:fontScheme>",
        major = esc_attr(&major),
        ea = esc_attr(&ea),
        minor = esc_attr(&minor)
    ));
    out.push_str(
        "<a:fmtScheme name=\"Office\">\
         <a:fillStyleLst>\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
         </a:fillStyleLst>\
         <a:lnStyleLst>\
         <a:ln><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>\
         <a:ln><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>\
         <a:ln><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>\
         </a:lnStyleLst>\
         <a:effectStyleLst>\
         <a:effectStyle><a:effectLst/></a:effectStyle>\
         <a:effectStyle><a:effectLst/></a:effectStyle>\
         <a:effectStyle><a:effectLst/></a:effectStyle>\
         </a:effectStyleLst>\
         <a:bgFillStyleLst>\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\
         </a:bgFillStyleLst></a:fmtScheme>",
    );
    out.push_str("</a:themeElements></a:theme>");
    out
}

/// fontTable.xml
pub(crate) fn font_table_xml(ctx: &GenCtx) -> String {
    let (minor, major, ea, heading) = theme_fonts(ctx);
    let mut fonts: Vec<String> = vec![minor, major, ea, heading, "Consolas".to_string()];
    fonts.sort();
    fonts.dedup();
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!("<w:fonts {STYLES_NS}>"));
    for (i, f) in fonts.iter().enumerate() {
        out.push_str(&format!(
            "<w:font w:name=\"{}\"><w:charset w:val=\"00\"/><w:family w:val=\"auto\"/><w:pitch w:val=\"variable\"/></w:font>",
            esc_attr(f)
        ));
        let _ = i;
    }
    out.push_str("</w:fonts>");
    out
}

/// webSettings.xml
pub(crate) fn web_settings_xml() -> String {
    format!(
        "{}<w:webSettings {STYLES_NS}><w:optimizeForBrowser/></w:webSettings>",
        declaration()
    )
}

/// docProps/core.xml
pub(crate) fn core_xml(ctx: &GenCtx) -> String {
    let title = esc(&ctx.doc.meta.title.clone().unwrap_or_default());
    let author = esc(&ctx.doc.meta.author.clone().unwrap_or_default());
    let subject = esc(&ctx.doc.meta.subject.clone().unwrap_or_default());
    let keywords = esc(&ctx.doc.meta.keywords.join(", "));
    let now = "2026-01-01T00:00:00Z";
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>",
            "<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" ",
            "xmlns:dc=\"http://purl.org/dc/elements/1.1/\" ",
            "xmlns:dcterms=\"http://purl.org/dc/terms/\" ",
            "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">",
            "<dc:title>{title}</dc:title><dc:creator>{author}</dc:creator>",
            "<cp:lastModifiedBy>{author}</cp:lastModifiedBy>",
            "<dc:subject>{subject}</dc:subject><cp:keywords>{keywords}</cp:keywords>",
            "<dcterms:created xsi:type=\"dcterms:W3CDTF\">{now}</dcterms:created>",
            "<dcterms:modified xsi:type=\"dcterms:W3CDTF\">{now}</dcterms:modified>",
            "</cp:coreProperties>"
        ),
        title = title,
        author = author,
        subject = subject,
        keywords = keywords,
        now = now
    )
}

/// docProps/app.xml（含字数统计）
pub(crate) fn app_xml(ctx: &GenCtx) -> String {
    let mut words = 0usize;
    let mut chars = 0usize;
    let mut paragraphs = 0usize;
    for block in ctx.doc.parts.iter().flat_map(|p| p.blocks.iter()) {
        let text = block.plain_text();
        chars += text.chars().filter(|c| !c.is_whitespace()).count();
        words += text.split_whitespace().count();
        // 中文按字符近似计词
        let cjk = text
            .chars()
            .filter(|c| {
                let u = *c as u32;
                (0x4E00..=0x9FFF).contains(&u)
            })
            .count();
        words += cjk;
        if !text.is_empty() {
            paragraphs += 1;
        }
    }
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>",
            "<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\" ",
            "xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">",
            "<Application>json2docx</Application><AppVersion>0.1</AppVersion>",
            "<Words>{words}</Words><Characters>{chars}</Characters>",
            "<Paragraphs>{paragraphs}</Paragraphs>",
            "</Properties>"
        ),
        words = words,
        chars = chars,
        paragraphs = paragraphs
    )
}

/// VML 水印段落
fn watermark_xml(text: &str) -> String {
    format!(
        concat!(
            "<w:p><w:pPr><w:spacing w:after=\"0\"/></w:pPr><w:r><w:pict>",
            "<v:shapetype id=\"_x0000_t136\" coordsize=\"21600,21600\" o:spt=\"136\" adj=\"10800\" ",
            "path=\"m@7,l@8,m@5,21600l@6,21600e\">",
            "<v:formulas><v:f eqn=\"sum #0 0 10800\"/><v:f eqn=\"prod #0 2 1\"/>",
            "<v:f eqn=\"sum 21600 0 @1\"/><v:f eqn=\"sum 0 0 @2\"/>",
            "<v:f eqn=\"sum 21600 0 @3\"/><v:f eqn=\"if @0 @3 0\"/>",
            "<v:f eqn=\"if @0 21600 @1\"/><v:f eqn=\"if @0 0 @2\"/>",
            "<v:f eqn=\"if @0 @4 21600\"/><v:f eqn=\"mid @5 @6\"/>",
            "<v:f eqn=\"mid @8 @5\"/><v:f eqn=\"mid @7 @8\"/>",
            "<v:f eqn=\"mid @6 @7\"/><v:f eqn=\"sum @6 0 @5\"/></v:formulas>",
            "<v:path textpathok=\"t\" o:connecttype=\"custom\"/>",
            "<v:textpath on=\"t\" fitshape=\"t\"/></v:shapetype>",
            "<v:shape id=\"PowerPlusWaterMarkObject\" o:spid=\"_x0000_s2049\" type=\"#_x0000_t136\" ",
            "style=\"position:absolute;margin-left:0;margin-top:0;width:468pt;height:117pt;",
            "rotation:315;z-index:-251658752;mso-position-horizontal:center;",
            "mso-position-horizontal-relative:margin;mso-position-vertical:center;",
            "mso-position-vertical-relative:margin\" o:allowincell=\"f\" fillcolor=\"#C0C0C0\" stroked=\"f\">",
            "<v:fill opacity=\".5\"/>",
            "<v:textpath style=\"font-family:&quot;宋体&quot;;font-size:1pt\" string=\"{wm}\"/>",
            "</v:shape></w:pict></w:r></w:p>"
        ),
        wm = esc(text)
    )
}

/// footnotes.xml（含分隔符与脚注内容）
pub(crate) fn footnotes_xml(ctx: &GenCtx) -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!("<w:footnotes {STYLES_NS}>"));
    out.push_str(
        "<w:footnote w:type=\"separator\" w:id=\"-1\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:separator/></w:r></w:p></w:footnote>",
    );
    out.push_str(
        "<w:footnote w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>",
    );
    for (i, text) in ctx.footnotes.iter().enumerate() {
        let id = i + 1;
        out.push_str(&format!(
            "<w:footnote w:id=\"{id}\"><w:p><w:pPr><w:pStyle w:val=\"FootnoteText\"/></w:pPr>\
             <w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteRef/></w:r>\
             <w:r><w:t xml:space=\"preserve\"> {}</w:t></w:r></w:p></w:footnote>",
            esc(text)
        ));
    }
    out.push_str("</w:footnotes>");
    out
}

/// endnotes.xml（含分隔符与尾注内容）
pub(crate) fn endnotes_xml(ctx: &GenCtx) -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!("<w:endnotes {STYLES_NS}>"));
    out.push_str(
        "<w:endnote w:type=\"separator\" w:id=\"-1\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:separator/></w:r></w:p></w:endnote>",
    );
    out.push_str(
        "<w:endnote w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:continuationSeparator/></w:r></w:p></w:endnote>",
    );
    for (i, text) in ctx.endnotes.iter().enumerate() {
        let id = i + 1;
        out.push_str(&format!(
            "<w:endnote w:id=\"{id}\"><w:p><w:pPr><w:pStyle w:val=\"EndnoteText\"/></w:pPr>\
             <w:r><w:rPr><w:rStyle w:val=\"EndnoteReference\"/></w:rPr><w:endnoteRef/></w:r>\
             <w:r><w:t xml:space=\"preserve\"> {}</w:t></w:r></w:p></w:endnote>",
            esc(text)
        ));
    }
    out.push_str("</w:endnotes>");
    out
}

/// comments.xml
pub(crate) fn comments_xml(ctx: &GenCtx) -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!("<w:comments {STYLES_NS}>"));
    for (i, (author, text)) in ctx.comments.iter().enumerate() {
        let id = i + 1;
        out.push_str(&format!(
            "<w:comment w:id=\"{id}\" w:author=\"{}\" w:date=\"{}\" w:initials=\"AI\">\
             <w:p><w:pPr><w:pStyle w:val=\"CommentText\"/></w:pPr><w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:comment>",
            esc_attr(author),
            crate::generate::body::REVISION_DATE,
            esc(text)
        ));
    }
    out.push_str("</w:comments>");
    out
}

/// 页眉部件内容（优先渲染块，否则纯文本中心行）
pub(crate) fn header_xml_content(
    ctx: &GenCtx,
    text: &str,
    blocks: Option<&[crate::model::blocks::Block]>,
    watermark: Option<&str>,
) -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(
        "<w:hdr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
         xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" \
         xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\" \
         xmlns:v=\"urn:schemas-microsoft-com:vml\" \
         xmlns:o=\"urn:schemas-microsoft-com:office:office\" \
         xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">",
    );
    if let Some(wm) = watermark {
        out.push_str(&watermark_xml(wm));
    }
    match blocks {
        Some(bs) if !bs.is_empty() => {
            out.push_str(&crate::generate::body::blocks_xml(ctx, bs));
        }
        _ => {
            out.push_str(&format!(
                "<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr>\
                 <w:r><w:rPr><w:sz w:val=\"18\"/></w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                esc(text)
            ));
        }
    }
    out.push_str("</w:hdr>");
    out
}

/// 页脚部件内容（块 + 可选页码）
pub(crate) fn footer_xml_content(
    ctx: &GenCtx,
    format: &str,
    blocks: Option<&[crate::model::blocks::Block]>,
) -> String {
    let page_field = concat!(
        "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>",
        "<w:r><w:instrText xml:space=\"preserve\"> PAGE </w:instrText></w:r>",
        "<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>",
        "<w:r><w:t>1</w:t></w:r>",
        "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>"
    );
    let field = if format.eq_ignore_ascii_case("page_of") {
        format!(
            concat!(
                "<w:r><w:t xml:space=\"preserve\">第 </w:t></w:r>{page}",
                "<w:r><w:t xml:space=\"preserve\"> 页 / 共 </w:t></w:r>",
                "<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>",
                "<w:r><w:instrText xml:space=\"preserve\"> NUMPAGES </w:instrText></w:r>",
                "<w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>",
                "<w:r><w:t>1</w:t></w:r>",
                "<w:r><w:fldChar w:fldCharType=\"end\"/></w:r>",
                "<w:r><w:t xml:space=\"preserve\"> 页</w:t></w:r>"
            ),
            page = page_field
        )
    } else {
        page_field.to_string()
    };
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(
        "<w:ftr xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
         xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
         xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" \
         xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">",
    );
    if let Some(bs) = blocks {
        if !bs.is_empty() {
            out.push_str(&crate::generate::body::blocks_xml(ctx, bs));
        }
    }
    out.push_str(&format!(
        "<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr>{field}</w:p>"
    ));
    out.push_str("</w:ftr>");
    out
}
