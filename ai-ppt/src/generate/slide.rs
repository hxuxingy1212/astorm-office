//! 幻灯片 XML 生成模块
//!
//! 将 Slide 模型转换为 OOXML 格式的幻灯片 XML 字符串。
//! 处理所有元素类型的 XML 生成，包括背景、过渡动画。

use super::components;
use crate::model::elements::*;
use crate::model::Slide;
use crate::utils::constants::*;
use crate::utils::xml::*;
use std::collections::HashMap;

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// 生成单张幻灯片的完整 XML
pub fn generate(
    slide: &Slide,
    r_id_map: &HashMap<String, String>,
    hyperlink_map: &HashMap<String, String>,
    chart_rids: &[String],
    media_map: &HashMap<String, String>,
    opaque_maps: &[Vec<(String, String)>],
    slide_size: (f64, f64),
) -> String {
    let mut shape_tree = String::new();
    // 每张幻灯片需要一个 p:spTree 根元素
    shape_tree.push_str(
        "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
         <p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/>\
         <a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>",
    );

    let mut sp_id_counter = 1u32;
    let mut anim_elements: Vec<(u32, &Vec<Animation>)> = Vec::new();
    let mut chart_counter = 0usize;
    let mut opaque_counter = 0usize;

    // 遍历所有元素生成 XML
    for el in &slide.elements {
        sp_id_counter += 1;
        // 元素带 id（来自 unpack 的原始文档）时原样回写，保证 @id 稳定；
        // 计数器随之推进，避免后续自动分配的 id 与已有 id 冲突
        let used_id = el
            .id()
            .filter(|i| *i > 1)
            .unwrap_or(sp_id_counter)
            .max(sp_id_counter);
        sp_id_counter = used_id;
        shape_tree.push_str(&element_to_xml(
            el,
            used_id,
            r_id_map,
            hyperlink_map,
            &mut sp_id_counter,
            &mut anim_elements,
            chart_rids,
            &mut chart_counter,
            media_map,
            opaque_maps,
            &mut opaque_counter,
        ));
    }

    let timing_xml = super::animation::generate_timing(&anim_elements, slide_size);
    let bg_xml = generate_background(slide.background.as_ref(), r_id_map);
    let transition_xml = generate_transition(slide.transition.as_ref());

    format!(
        "{}<p:sld xmlns:a=\"{}\" xmlns:r=\"{}\" xmlns:p=\"{}\" \
         xmlns:p14=\"http://schemas.microsoft.com/office/powerpoint/2010/main\" \
         xmlns:a14=\"http://schemas.microsoft.com/office/drawing/2010/main\" \
         xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\">\n\
         <p:cSld>{}{}</p:cSld>\n\
         {}\
         {}\
         </p:sld>",
        XML_DECL,
        "http://schemas.openxmlformats.org/drawingml/2006/main",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        "http://schemas.openxmlformats.org/presentationml/2006/main",
        bg_xml,
        format_args!("<p:spTree>{}</p:spTree>", shape_tree),
        transition_xml,
        timing_xml,
    )
}

#[allow(clippy::too_many_arguments)]
fn element_to_xml<'a>(
    el: &'a Element,
    sp_id: u32,
    r_id_map: &HashMap<String, String>,
    hyperlink_map: &HashMap<String, String>,
    next_sp_id: &mut u32,
    anim_elements: &mut Vec<(u32, &'a Vec<Animation>)>,
    chart_rids: &[String],
    chart_counter: &mut usize,
    media_map: &HashMap<String, String>,
    opaque_maps: &[Vec<(String, String)>],
    opaque_counter: &mut usize,
) -> String {
    match el {
        Element::Text(t) => {
            if let Some(anims) = &t.animations {
                anim_elements.push((sp_id, anims));
            }
            generate_text_shape(t, sp_id, r_id_map, hyperlink_map)
        }
        Element::Shape(s) => {
            if let Some(anims) = &s.animations {
                anim_elements.push((sp_id, anims));
            }
            generate_auto_shape(s, sp_id, r_id_map, hyperlink_map)
        }
        Element::Line(l) => {
            if let Some(anims) = &l.animations {
                anim_elements.push((sp_id, anims));
            }
            generate_line(l, sp_id)
        }
        Element::Image(img) => {
            if let Some(anims) = &img.animations {
                anim_elements.push((sp_id, anims));
            }
            generate_picture(img, sp_id, r_id_map, hyperlink_map, media_map)
        }
        Element::Icon(ico) => {
            if let Some(anims) = &ico.animations {
                anim_elements.push((sp_id, anims));
            }
            generate_icon(ico, sp_id)
        }
        Element::Formula(form) => {
            if let Some(anims) = &form.animations {
                anim_elements.push((sp_id, anims));
            }
            generate_formula(form, sp_id)
        }
        Element::Table(tbl) => {
            if let Some(anims) = &tbl.animations {
                anim_elements.push((sp_id, anims));
            }
            generate_table(tbl, sp_id, r_id_map)
        }
        Element::Chart(c) => {
            if let Some(anims) = &c.animations {
                anim_elements.push((sp_id, anims));
            }
            let rid = chart_rids
                .get(*chart_counter)
                .map(|s| s.as_str())
                .unwrap_or("");
            *chart_counter += 1;
            generate_chart_frame(c, sp_id, rid)
        }
        // 批注不由 spTree 渲染（作为独立部件写入）
        Element::Comment(_) => String::new(),
        Element::Equation(eq) => generate_equation(eq, sp_id),
        Element::Opaque(o) => {
            let map = opaque_maps
                .get(*opaque_counter)
                .cloned()
                .unwrap_or_default();
            *opaque_counter += 1;
            let mut xml = o.xml.clone();
            for (old, new) in map {
                xml = xml.replace(&format!("\"{}\"", old), &format!("\"{}\"", new));
            }
            xml
        }
        Element::Group(grp) => {
            if let Some(anims) = &grp.animations {
                anim_elements.push((sp_id, anims));
            }
            generate_group(
                grp,
                r_id_map,
                hyperlink_map,
                next_sp_id,
                anim_elements,
                chart_rids,
                chart_counter,
                media_map,
                opaque_maps,
                opaque_counter,
            )
        }
        Element::ProgressBar(comp) => {
            components::expand_component(&Element::ProgressBar(comp.clone()), next_sp_id)
        }
        Element::ProgressRing(comp) => {
            components::expand_component(&Element::ProgressRing(comp.clone()), next_sp_id)
        }
        Element::BarChart(comp) => {
            components::expand_component(&Element::BarChart(comp.clone()), next_sp_id)
        }
        Element::LineChart(comp) => {
            components::expand_component(&Element::LineChart(comp.clone()), next_sp_id)
        }
        Element::PieChart(comp) => {
            components::expand_component(&Element::PieChart(comp.clone()), next_sp_id)
        }
        Element::RingChart(comp) => {
            components::expand_component(&Element::RingChart(comp.clone()), next_sp_id)
        }
        Element::KpiCard(comp) => {
            components::expand_component(&Element::KpiCard(comp.clone()), next_sp_id)
        }
        Element::RatingStars(comp) => {
            components::expand_component(&Element::RatingStars(comp.clone()), next_sp_id)
        }
        Element::Timeline(comp) => {
            components::expand_component(&Element::Timeline(comp.clone()), next_sp_id)
        }
        Element::ProcessFlow(comp) => {
            components::expand_component(&Element::ProcessFlow(comp.clone()), next_sp_id)
        }
    }
}

/// 生成 p:nvPr（含占位符引用）
fn ph_xml(ph: Option<&str>) -> String {
    match ph {
        Some(s) => {
            let (t, idx) = s.split_once('|').unwrap_or((s, ""));
            let mut attrs = String::new();
            if !t.is_empty() {
                attrs.push_str(&format!(" type=\"{}\"", t));
            }
            if !idx.is_empty() && idx != "0" {
                attrs.push_str(&format!(" idx=\"{}\"", idx));
            }
            format!("<p:nvPr><p:ph{}/></p:nvPr>", attrs)
        }
        None => "<p:nvPr/>".to_string(),
    }
}

/// 生成 bodyPr 内的自适应子元素
fn autofit_xml(a: Option<&Autofit>) -> String {
    match a {
        Some(af) if af.kind == "normal" => {
            let mut s = "<a:normAutofit".to_string();
            if let Some(fs) = af.font_scale {
                s.push_str(&format!(
                    " fontScale=\"{}\"",
                    (fs * 100000.0).round() as i64
                ));
            }
            if let Some(r) = af.ln_spc_reduction {
                s.push_str(&format!(
                    " lnSpcReduction=\"{}\"",
                    (r * 100000.0).round() as i64
                ));
            }
            s.push_str("/>");
            s
        }
        Some(af) if af.kind == "shape" => "<a:spAutoFit/>".to_string(),
        Some(af) if af.kind == "none" => "<a:noAutofit/>".to_string(),
        _ => String::new(),
    }
}

/// 生成填充 XML；图片填充需要关系表提供 rId
fn fill_xml_mapped(fill: &Fill, alpha: Option<f64>, r_id_map: &HashMap<String, String>) -> String {
    if let Fill::Image { src } = fill {
        if let Some(rid) = r_id_map.get(src) {
            return format!(
                "<a:blipFill><a:blip r:embed=\"{}\"/><a:stretch><a:fillRect/></a:stretch></a:blipFill>",
                rid
            );
        }
        return String::new();
    }
    fill_xml_alpha(fill, alpha)
}

fn generate_text_shape(
    el: &TextElement,
    sp_id: u32,
    r_id_map: &HashMap<String, String>,
    hyperlink_map: &HashMap<String, String>,
) -> String {
    let pos = &el.position;
    let name = esc_xml(el.name.as_deref().unwrap_or(&format!("TextBox {}", sp_id)));

    let mut body_attrs: Vec<String> = Vec::new();
    match el.wrap {
        Some(false) => body_attrs.push("wrap=\"none\"".to_string()),
        _ => body_attrs.push("wrap=\"square\"".to_string()),
    }
    if let Some(va) = &el.vert_align {
        let v = match va.as_str() {
            "middle" => "ctr",
            "bottom" => "b",
            _ => "t",
        };
        body_attrs.push(format!("anchor=\"{}\"", v));
    }
    if let Some(vert) = &el.vert {
        let v = match vert.as_str() {
            "vert" => "vert",
            "vert270" => "vert270",
            _ => "horz",
        };
        body_attrs.push(format!("vert=\"{}\"", v));
    }

    let paragraphs_xml = generate_paragraphs(&el.text, el, hyperlink_map);
    let af = autofit_xml(el.autofit.as_ref());
    let body_xml = if af.is_empty() {
        format!("<a:bodyPr {}/>", body_attrs.join(" "))
    } else {
        format!("<a:bodyPr {}>{}</a:bodyPr>", body_attrs.join(" "), af)
    };
    let mut sp_pr = xfrm_xml(pos.x, pos.y, pos.w, pos.h, None);
    sp_pr.push_str("<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>");
    if let Some(f) = &el.fill {
        sp_pr.push_str(&fill_xml_mapped(f, el.fill_alpha, r_id_map));
    }
    if let Some(l) = &el.line {
        sp_pr.push_str(&line_xml(&l.color, l.width));
    }
    sp_pr.push_str(&effect_lst_xml(el.shadow.as_ref(), el.effects.as_ref()));

    format!(
        "<p:sp>\n\
         <p:nvSpPr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"/>\n\
         <p:cNvSpPr txBox=\"1\"/>\n\
         {}\n\
         </p:nvSpPr>\n\
         <p:spPr>{}</p:spPr>\n\
         <p:txBody>\n\
         {}\n\
         <a:lstStyle/>\n\
         {}\
         </p:txBody>\n\
         </p:sp>",
        sp_id,
        name,
        ph_xml(el.placeholder.as_deref()),
        sp_pr,
        body_xml,
        paragraphs_xml,
    )
}

fn generate_auto_shape(
    el: &ShapeElement,
    sp_id: u32,
    r_id_map: &HashMap<String, String>,
    hyperlink_map: &HashMap<String, String>,
) -> String {
    let pos = &el.position;
    let name = esc_xml(el.name.as_deref().unwrap_or(&format!("Shape {}", sp_id)));
    let shape_type = shape_ooxml_name(&el.shape_type);

    let mut sp_pr = xfrm_xml(pos.x, pos.y, pos.w, pos.h, el.rotation);
    sp_pr.push_str(&format!(
        "<a:prstGeom prst=\"{}\">{}</a:prstGeom>",
        shape_type,
        av_lst_xml(el.adjust.as_ref())
    ));
    if el.no_fill.unwrap_or(false) {
        sp_pr.push_str("<a:noFill/>");
    } else if let Some(f) = &el.fill {
        sp_pr.push_str(&fill_xml_mapped(f, el.fill_alpha, r_id_map));
    }
    if let Some(l) = &el.line {
        sp_pr.push_str(&line_xml_alpha(&l.color, l.width, el.line_alpha));
    }
    sp_pr.push_str(&effect_lst_xml(el.shadow.as_ref(), el.effects.as_ref()));

    let mut tx_body = String::new();
    if let Some(text) = &el.text {
        let anchor = match el.vert_align.as_deref() {
            Some("top") => "t",
            Some("bottom") => "b",
            _ => "ctr",
        };
        let af = autofit_xml(el.autofit.as_ref());
        let body = if af.is_empty() {
            format!("<a:bodyPr wrap=\"square\" anchor=\"{}\"/>", anchor)
        } else {
            format!(
                "<a:bodyPr wrap=\"square\" anchor=\"{}\">{}</a:bodyPr>",
                anchor, af
            )
        };
        tx_body = format!(
            "<p:txBody>{}<a:lstStyle/>{}</p:txBody>",
            body,
            generate_paragraphs(text, el, hyperlink_map)
        );
    }

    format!(
        "<p:sp>\n\
         <p:nvSpPr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"/>\n\
         <p:cNvSpPr/>\n\
         {}\n\
         </p:nvSpPr>\n\
         <p:spPr>{}</p:spPr>\n\
         {}\
         </p:sp>",
        sp_id,
        name,
        ph_xml(el.placeholder.as_deref()),
        sp_pr,
        tx_body,
    )
}

/// 图标降级为一个椭圆形状（保留颜色；PPT 中以矢量形状呈现）
fn generate_icon(el: &IconElement, sp_id: u32) -> String {
    let pos = &el.position;
    let name = esc_xml(el.name.as_deref().unwrap_or(&format!("Icon {}", sp_id)));
    let color = el.color.as_deref().unwrap_or("4472C4");

    let mut sp_pr = xfrm_xml(pos.x, pos.y, pos.w, pos.h, el.rotation);
    sp_pr.push_str("<a:prstGeom prst=\"ellipse\"><a:avLst/></a:prstGeom>");
    sp_pr.push_str(&solid_fill_xml(color));
    if let Some(l) = &el.line {
        sp_pr.push_str(&line_xml(&l.color, l.width));
    }

    format!(
        "<p:sp>\n\
         <p:nvSpPr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"/>\n\
         <p:cNvSpPr/>\n\
         <p:nvPr/>\n\
         </p:nvSpPr>\n\
         <p:spPr>{}</p:spPr>\n\
         </p:sp>",
        sp_id, name, sp_pr,
    )
}

/// 公式降级为文本框（保留 LaTeX 文字与颜色）
fn generate_formula(el: &FormulaElement, sp_id: u32) -> String {
    let pos = &el.position;
    let name = esc_xml(el.name.as_deref().unwrap_or(&format!("Formula {}", sp_id)));
    let color = el.color.as_deref().unwrap_or("000000");
    let font_size = el.font_size.unwrap_or(24.0);

    let mut sp_pr = xfrm_xml(pos.x, pos.y, pos.w, pos.h, el.rotation);
    sp_pr.push_str("<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>");

    let rpr = run_props_xml(Some(font_size), false, false, false, Some(color), None);
    let tx_body = format!(
        "<p:txBody><a:bodyPr wrap=\"square\" anchor=\"ctr\"/><a:lstStyle/><a:p>{}</a:p></p:txBody>",
        text_runs_xml(&el.latex, &rpr)
    );

    format!(
        "<p:sp>\n\
         <p:nvSpPr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"/>\n\
         <p:cNvSpPr txBox=\"1\"/>\n\
         <p:nvPr/>\n\
         </p:nvSpPr>\n\
         <p:spPr>{}</p:spPr>\n\
         {}\
         </p:sp>",
        sp_id, name, sp_pr, tx_body,
    )
}

fn generate_picture(
    el: &ImageElement,
    sp_id: u32,
    r_id_map: &HashMap<String, String>,
    hyperlink_map: &HashMap<String, String>,
    media_map: &HashMap<String, String>,
) -> String {
    let pos = &el.position;
    let name = esc_xml(el.name.as_deref().unwrap_or(&format!("Picture {}", sp_id)));
    // 按 src 精确查找媒体关系（URL 与本地路径均可）
    let r_id = r_id_map.get(&el.src).map(|s| s.as_str()).unwrap_or("rId2");

    // 视频/音频：在 p:nvPr 中挂接 mediaFile
    let nv_pr = match &el.media {
        Some(m) => match media_map.get(&m.src) {
            Some(mrid) if m.kind == "video" => {
                format!("<p:nvPr><a:videoFile r:link=\"{}\"/></p:nvPr>", mrid)
            }
            Some(mrid) => format!("<p:nvPr><a:audioFile r:link=\"{}\"/></p:nvPr>", mrid),
            None => "<p:nvPr/>".to_string(),
        },
        None => "<p:nvPr/>".to_string(),
    };

    let mut sp_pr = xfrm_xml(pos.x, pos.y, pos.w, pos.h, el.rotation);
    sp_pr.push_str("<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>");
    if let Some(l) = &el.line {
        sp_pr.push_str(&line_xml(&l.color, l.width));
    }

    // 图片超链接（输出到 p:cNvPr 内）
    let hlink_xml = el
        .hyperlink
        .as_ref()
        .and_then(|url| hyperlink_map.get(url))
        .map(|rid| {
            let tip = el
                .tooltip
                .as_deref()
                .map(|t| format!(" tooltip=\"{}\"", esc_xml(t)))
                .unwrap_or_default();
            format!("<a:hlinkClick r:id=\"{}\"{}/>", rid, tip)
        })
        .unwrap_or_default();

    // 亮度/对比度（OOXML lum 以百分比 *1000 表示）
    let lum_xml = if el.brightness.is_some() || el.contrast.is_some() {
        let bright = (el.brightness.unwrap_or(0.0) * 100000.0).round() as i64;
        let contrast = (el.contrast.unwrap_or(0.0) * 100000.0).round() as i64;
        format!("<a:lum bright=\"{}\" contrast=\"{}\"/>", bright, contrast)
    } else {
        String::new()
    };

    // 填充模式：tile 平铺，其余拉伸
    let fill_xml = if el.fill_mode.as_deref() == Some("tile") {
        "<a:tile/>".to_string()
    } else {
        "<a:stretch><a:fillRect/></a:stretch>".to_string()
    };

    let descr = el
        .tooltip
        .as_deref()
        .map(|t| format!(" descr=\"{}\"", esc_xml(t)))
        .unwrap_or_default();

    let crop_xml = el
        .crop
        .as_ref()
        .map(|c| {
            format!(
                " <a:srcRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/>",
                c.left, c.top, c.right, c.bottom
            )
        })
        .unwrap_or_default();

    format!(
        "<p:pic>\n\
         <p:nvPicPr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"{}/>{}\n\
         <p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr>\n\
         {}\n\
         </p:nvPicPr>\n\
         <p:blipFill>\n\
         <a:blip r:embed=\"{}\">{}</a:blip>{}\n\
         {}\n\
         </p:blipFill>\n\
         <p:spPr>{}</p:spPr>\n\
         </p:pic>",
        sp_id, name, descr, hlink_xml, nv_pr, r_id, lum_xml, crop_xml, fill_xml, sp_pr,
    )
}

fn generate_table(el: &TableElement, sp_id: u32, r_id_map: &HashMap<String, String>) -> String {
    let pos = &el.position;
    let name = esc_xml(el.name.as_deref().unwrap_or(&format!("Table {}", sp_id)));
    let rows = &el.rows;
    if rows.is_empty() || rows[0].is_empty() {
        return String::new();
    }

    // 物理列数：column_widths 指定时以其长度为准，否则取每行 colspan 之和的最大值
    let num_cols = match &el.column_widths {
        Some(widths) if !widths.is_empty() => widths.len(),
        _ => rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|c| c.colspan.unwrap_or(1).max(1) as usize)
                    .sum()
            })
            .max()
            .unwrap_or(0),
    };
    if num_cols == 0 {
        return String::new();
    }
    let row_height = inch_to_emu(pos.h) / rows.len() as i64;

    // 列宽：column_widths 指定则按英寸换算，否则均分表格总宽
    let col_widths: Vec<i64> = match &el.column_widths {
        Some(widths) if widths.len() == num_cols => {
            widths.iter().map(|w| inch_to_emu(*w)).collect()
        }
        _ => vec![inch_to_emu(pos.w) / num_cols as i64; num_cols],
    };
    let mut grid_cols = String::new();
    for w in &col_widths {
        grid_cols.push_str(&format!("<a:gridCol w=\"{}\"/>", w));
    }

    // 合并单元格：模型按"逻辑网格"书写（rowspan 主格只出现一次），
    // 生成时输出 gridSpan/rowSpan，并为被上方 rowspan 覆盖的列自动补 vMerge 占位格。
    let mut rowspan_cover: Vec<u32> = vec![0; num_cols];
    let mut rows_xml = String::new();
    for (r, row) in rows.iter().enumerate() {
        let mut cells_xml = String::new();
        let mut col = 0usize;
        for cell in row {
            // 补齐被上方 rowspan 覆盖的占位列
            while col < num_cols && rowspan_cover[col] > 0 {
                rowspan_cover[col] -= 1;
                cells_xml.push_str(
                    "<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p/></a:txBody><a:tcPr vMerge=\"1\"/></a:tc>",
                );
                col += 1;
            }
            let colspan = cell.colspan.unwrap_or(1).max(1) as usize;
            let rowspan = cell.rowspan.unwrap_or(1).max(1) as usize;
            if col + colspan > num_cols {
                break;
            }
            let mut tc_pr_extra = if let Some(src) = &cell.fill_image {
                match r_id_map.get(src) {
                    Some(rid) => format!(
                        "<a:blipFill><a:blip r:embed=\"{}\"/><a:stretch><a:fillRect/></a:stretch></a:blipFill>",
                        rid
                    ),
                    None => String::new(),
                }
            } else {
                cell.fill
                    .as_ref()
                    .map(|f| solid_fill_xml(f))
                    .unwrap_or_default()
            };
            // 单元格边框：四边同色同宽
            if let Some(bd) = &cell.border {
                let w = (bd.width * 12700.0).round() as i64;
                for side in ["lnL", "lnR", "lnT", "lnB"] {
                    tc_pr_extra.push_str(&format!(
                        "<a:{} w=\"{}\" cap=\"flat\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:{}>",
                        side,
                        w,
                        bd.color.trim_start_matches('#'),
                        side
                    ));
                }
            }
            let fs = cell.font_size.or(el.font_size).unwrap_or(14.0);
            let b = if cell.bold.unwrap_or(false) || (r == 0 && el.header_row.unwrap_or(false)) {
                "b=\"1\""
            } else {
                ""
            };
            let c = cell
                .color
                .as_deref()
                .or(el.color.as_deref())
                .unwrap_or("000000");
            let align = cell.align.as_deref().unwrap_or("l");
            let cell_rpr = format!("<a:rPr lang=\"en-US\" sz=\"{}\" {} dirty=\"0\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:rPr>",
                pt_to_hundredths(fs), b, c.trim_start_matches('#'));
            let mut tc_attrs = String::new();
            if colspan > 1 {
                tc_attrs.push_str(&format!(" gridSpan=\"{}\"", colspan));
            }
            if rowspan > 1 {
                tc_attrs.push_str(&format!(" rowSpan=\"{}\"", rowspan));
            }
            // tcPr 属性：内边距 + 竖排
            let mut tc_pr_attrs = String::new();
            if let Some(m) = &cell.margin {
                tc_pr_attrs.push_str(&format!(
                    " marL=\"{}\" marT=\"{}\" marR=\"{}\" marB=\"{}\"",
                    inch_to_emu(m[0]),
                    inch_to_emu(m[1]),
                    inch_to_emu(m[2]),
                    inch_to_emu(m[3])
                ));
            }
            if let Some(v) = &cell.vert {
                if v != "horz" {
                    tc_pr_attrs.push_str(&format!(" vert=\"{}\"", v));
                }
            }
            if let Some(a) = &cell.anchor {
                if a != "t" {
                    tc_pr_attrs.push_str(&format!(" anchor=\"{}\"", a));
                }
            }
            cells_xml.push_str(&format!(
                "<a:tc{}><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr algn=\"{}\"/>{}</a:p></a:txBody><a:tcPr{}>{}</a:tcPr></a:tc>",
                tc_attrs,
                if align == "center" { "ctr" } else if align == "right" { "r" } else { "l" },
                text_runs_xml(&cell.text, &cell_rpr),
                tc_pr_attrs,
                tc_pr_extra,
            ));
            // 登记下方行被覆盖的列
            if rowspan > 1 {
                for cell in rowspan_cover
                    .iter_mut()
                    .take((col + colspan).min(num_cols))
                    .skip(col)
                {
                    *cell = (*cell).max(rowspan as u32 - 1);
                }
            }
            col += colspan;
        }
        // 行尾补齐：确保每行输出 num_cols 个物理格
        while col < num_cols {
            if rowspan_cover[col] > 0 {
                rowspan_cover[col] -= 1;
                cells_xml.push_str(
                    "<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p/></a:txBody><a:tcPr vMerge=\"1\"/></a:tc>",
                );
            } else {
                cells_xml.push_str(
                    "<a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p/></a:txBody><a:tcPr/></a:tc>",
                );
            }
            col += 1;
        }
        let this_h = el
            .row_heights
            .as_ref()
            .and_then(|v| v.get(r))
            .filter(|h| **h > 0.0)
            .map(|h| inch_to_emu(*h))
            .unwrap_or(row_height);
        rows_xml.push_str(&format!("<a:tr h=\"{}\">{}</a:tr>", this_h, cells_xml));
    }

    let first_row = if el.header_row.unwrap_or(false) {
        "1"
    } else {
        "0"
    };
    // 仅当原表显式引用样式时才输出 a:tblStyle；否则交由默认样式（tableStyles.xml def）
    let style_xml = el
        .style_id
        .as_ref()
        .map(|s| format!("<a:tblStyle val=\"{}\"/>", s))
        .unwrap_or_default();
    let tbl_attrs = match &el.tbl_attrs {
        Some(a) if !a.trim().is_empty() => a.clone(),
        _ => {
            let rtl_attr = if el.rtl == Some(true) {
                " rtl=\"1\""
            } else {
                ""
            };
            format!("firstRow=\"{}\" bandRow=\"1\"{}", first_row, rtl_attr)
        }
    };
    let tbl_pr = format!("<a:tblPr {}>{}</a:tblPr>", tbl_attrs, style_xml);

    format!(
        "<p:graphicFrame>\n\
         <p:nvGraphicFramePr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"/>\n\
         <p:cNvGraphicFramePr><a:graphicFrameLocks noGrp=\"1\"/></p:cNvGraphicFramePr>\n\
         <p:nvPr/>\n\
         </p:nvGraphicFramePr>\n\
         <p:xfrm>\n\
         <a:off x=\"{}\" y=\"{}\"/>\n\
         <a:ext cx=\"{}\" cy=\"{}\"/>\n\
         </p:xfrm>\n\
         <a:graphic>\n\
         <a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/table\">\n\
         <a:tbl>\n\
         {}\n\
         <a:tblGrid>{}</a:tblGrid>\n\
         {}\
         </a:tbl>\n\
         </a:graphicData>\n\
         </a:graphic>\n\
         </p:graphicFrame>",
        sp_id,
        name,
        inch_to_emu(pos.x),
        inch_to_emu(pos.y),
        inch_to_emu(pos.w),
        inch_to_emu(pos.h),
        tbl_pr,
        grid_cols,
        rows_xml,
    )
}

#[allow(clippy::too_many_arguments)]
/// 生成公式形状（文本框内嵌原始 OMML）
fn generate_equation(el: &EquationElement, sp_id: u32) -> String {
    let pos = &el.position;
    let name = esc_xml(el.name.as_deref().unwrap_or(&format!("Equation {}", sp_id)));
    let mut sp_pr = xfrm_xml(pos.x, pos.y, pos.w, pos.h, None);
    sp_pr.push_str("<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>");
    let math = el.omml.trim();
    let tx_body = format!(
        "<p:txBody><a:bodyPr/><a:lstStyle/><a:p>{}</a:p></p:txBody>",
        math
    );
    format!(
        "<p:sp>\n\
         <p:nvSpPr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"/>\n\
         <p:cNvSpPr txBox=\"1\"/>\n\
         <p:nvPr/>\n\
         </p:nvSpPr>\n\
         <p:spPr>{}</p:spPr>\n\
         {}\
         </p:sp>",
        sp_id, name, sp_pr, tx_body,
    )
}

/// 生成真实图表 graphicFrame（引用 ppt/charts/chartN.xml）
fn generate_chart_frame(el: &ChartElement, sp_id: u32, chart_rid: &str) -> String {
    let pos = &el.position;
    let name = esc_xml(el.name.as_deref().unwrap_or(&format!("Chart {}", sp_id)));
    format!(
        "<p:graphicFrame>\n\
         <p:nvGraphicFramePr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"/>\n\
         <p:cNvGraphicFramePr/>\n\
         <p:nvPr/>\n\
         </p:nvGraphicFramePr>\n\
         <p:xfrm>\n\
         <a:off x=\"{}\" y=\"{}\"/>\n\
         <a:ext cx=\"{}\" cy=\"{}\"/>\n\
         </p:xfrm>\n\
         <a:graphic>\n\
         <a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">\n\
         <c:chart xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:id=\"{}\"/>\n\
         </a:graphicData>\n\
         </a:graphic>\n\
         </p:graphicFrame>",
        sp_id,
        name,
        inch_to_emu(pos.x),
        inch_to_emu(pos.y),
        inch_to_emu(pos.w),
        inch_to_emu(pos.h),
        chart_rid,
    )
}

#[allow(clippy::too_many_arguments)] // 生成器上下文参数较多，保持扁平签名
fn generate_group<'a>(
    el: &'a GroupElement,
    r_id_map: &HashMap<String, String>,
    hyperlink_map: &HashMap<String, String>,
    next_sp_id: &mut u32,
    anim_elements: &mut Vec<(u32, &'a Vec<Animation>)>,
    chart_rids: &[String],
    chart_counter: &mut usize,
    media_map: &HashMap<String, String>,
    opaque_maps: &[Vec<(String, String)>],
    opaque_counter: &mut usize,
) -> String {
    let pos = &el.position;
    *next_sp_id += 1;
    let sp_id = el
        .id
        .filter(|i| *i > 1)
        .unwrap_or(*next_sp_id)
        .max(*next_sp_id);
    *next_sp_id = sp_id;
    let mut children_xml = String::new();

    for child in &el.children {
        *next_sp_id += 1;
        let used_id = child
            .id()
            .filter(|i| *i > 1)
            .unwrap_or(*next_sp_id)
            .max(*next_sp_id);
        *next_sp_id = used_id;
        children_xml.push_str(&element_to_xml(
            child,
            used_id,
            r_id_map,
            hyperlink_map,
            next_sp_id,
            anim_elements,
            chart_rids,
            chart_counter,
            media_map,
            opaque_maps,
            opaque_counter,
        ));
    }

    let rot = el
        .rotation
        .map(|r| format!(" rot=\"{}\"", (r * 60000.0).round() as i64))
        .unwrap_or_default();

    format!(
        "<p:grpSp>\n\
         <p:nvGrpSpPr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"/>\n\
         <p:cNvGrpSpPr/>\n\
         <p:nvPr/>\n\
         </p:nvGrpSpPr>\n\
         <p:grpSpPr>\n\
         <a:xfrm{}>\n\
         <a:off x=\"{}\" y=\"{}\"/>\n\
         <a:ext cx=\"{}\" cy=\"{}\"/>\n\
         <a:chOff x=\"{}\" y=\"{}\"/>\n\
         <a:chExt cx=\"{}\" cy=\"{}\"/>\n\
         </a:xfrm>\n\
         </p:grpSpPr>\n\
         {}\
         </p:grpSp>",
        sp_id,
        esc_xml(el.name.as_deref().unwrap_or(&format!("Group {sp_id}"))),
        rot,
        inch_to_emu(pos.x),
        inch_to_emu(pos.y),
        inch_to_emu(pos.w),
        inch_to_emu(pos.h),
        inch_to_emu(pos.x),
        inch_to_emu(pos.y),
        inch_to_emu(pos.w),
        inch_to_emu(pos.h),
        children_xml,
    )
}

fn text_runs_xml(text: &str, r_pr: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut xml = String::new();
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            xml.push_str(&format!("<a:br>{}</a:br>", r_pr));
        }
        xml.push_str(&format!("<a:r>{}<a:t>{}</a:t></a:r>", r_pr, esc_xml(line)));
    }
    xml
}

fn generate_paragraphs(
    text: &TextContent,
    defaults: &impl HasTextDefaults,
    hyperlink_map: &HashMap<String, String>,
) -> String {
    match text {
        TextContent::Simple(s) => {
            let rpr = defaults.default_run_props();
            let ppr = defaults.default_para_props();
            format!("<a:p>{}{}</a:p>", ppr, text_runs_xml(s, &rpr))
        }
        TextContent::Paragraphs(paras) => {
            let mut xml = String::new();
            for para in paras {
                let runs_xml = if let Some(runs) = &para.runs {
                    let mut rx = String::new();
                    for run in runs {
                        let rr = run_props_xml_ex(
                            run.font_size.or(defaults.default_font_size()),
                            run.bold.unwrap_or(false),
                            run.italic.unwrap_or(false),
                            run.underline.unwrap_or(false),
                            run.color.as_deref().or(defaults.default_color()),
                            run.font_family.as_deref(),
                            run.hyperlink
                                .as_ref()
                                .and_then(|url| hyperlink_map.get(url))
                                .map(|s| s.as_str()),
                            RunExtras {
                                highlight: run.highlight.as_deref(),
                                underline_color: run.underline_color.as_deref(),
                                spacing: run.spacing,
                                lang: run.lang.as_deref(),
                                rtl: run.rtl,
                            },
                        );
                        rx.push_str(&text_runs_xml(&run.text, &rr));
                    }
                    rx
                } else if let Some(t) = &para.text {
                    let rr = run_props_xml(
                        para.font_size.or(defaults.default_font_size()),
                        para.bold.unwrap_or(false),
                        para.italic.unwrap_or(false),
                        false,
                        para.color.as_deref().or(defaults.default_color()),
                        None,
                    );
                    text_runs_xml(t, &rr)
                } else {
                    String::new()
                };
                let ppr = para_props_xml_line_spacing(
                    para.align.as_deref(),
                    para.bullet.unwrap_or(false),
                    para.line_spacing.or(defaults.default_line_spacing()),
                );
                xml.push_str(&format!("<a:p>{}{}</a:p>", ppr, runs_xml));
            }
            xml
        }
    }
}

/// 生成线条元素（p:cxnSp + a:custGeom 路径）
fn generate_line(el: &LineElement, sp_id: u32) -> String {
    let pos = &el.position;
    let name = esc_xml(el.name.as_deref().unwrap_or(&format!("Line {}", sp_id)));
    let color = el.color.as_deref().unwrap_or("4472C4");
    let width = el.width.unwrap_or(1.0);
    let w = (width * 12700.0).round() as i64;

    // 顶点（EMU 绝对坐标，= position 偏移 + 相对点）；缺省为从左上到右下的对角线
    let points: Vec<(i64, i64)> = if el.points.is_empty() {
        vec![
            (inch_to_emu(pos.x), inch_to_emu(pos.y + pos.h)),
            (inch_to_emu(pos.x + pos.w), inch_to_emu(pos.y)),
        ]
    } else {
        el.points
            .iter()
            .map(|(x, y)| (inch_to_emu(pos.x + *x), inch_to_emu(pos.y + *y)))
            .collect()
    };

    // custGeom 路径：moveTo + lnTo（或 quadBezTo 平滑）
    let mut path_xml = String::new();
    path_xml.push_str(&format!(
        "<a:path w=\"{}\" h=\"{}\">",
        inch_to_emu(pos.w),
        inch_to_emu(pos.h)
    ));
    let (fx, fy) = points[0];
    path_xml.push_str(&format!(
        "<a:moveTo><a:pt x=\"{}\" y=\"{}\"/></a:moveTo>",
        fx, fy
    ));
    if el.smooth.unwrap_or(false) {
        for w in points.windows(2) {
            let (x1, y1) = w[0];
            let (x2, y2) = w[1];
            let (cx, cy) = ((x1 + x2) / 2, (y1 + y2) / 2);
            path_xml.push_str(&format!(
                "<a:quadBezTo><a:pt x=\"{}\" y=\"{}\"/><a:pt x=\"{}\" y=\"{}\"/></a:quadBezTo>",
                cx, cy, x2, y2
            ));
        }
    } else {
        for &(x, y) in points.iter().skip(1) {
            path_xml.push_str(&format!("<a:lnTo><a:pt x=\"{}\" y=\"{}\"/></a:lnTo>", x, y));
        }
    }
    path_xml.push_str("</a:path>");

    // 线条样式：颜色/宽度/虚线/端点
    let mut ln_xml = format!("<a:ln w=\"{}\" cap=\"flat\">", w);
    ln_xml.push_str(&solid_fill_xml(color));
    if let Some(dash) = &el.dash {
        let d = dash_ooxml_name(dash);
        if d != "solid" {
            ln_xml.push_str(&format!("<a:prstDash val=\"{}\"/>", d));
        }
    }
    if let Some(end) = arrow_end_ooxml_name(el.arrow_start.as_deref().unwrap_or("none")) {
        ln_xml.push_str(&format!(
            "<a:headEnd type=\"{}\" w=\"med\" len=\"med\"/>",
            end
        ));
    }
    if let Some(end) = arrow_end_ooxml_name(el.arrow_end.as_deref().unwrap_or("none")) {
        ln_xml.push_str(&format!(
            "<a:tailEnd type=\"{}\" w=\"med\" len=\"med\"/>",
            end
        ));
    }
    ln_xml.push_str("</a:ln>");

    let mut sp_pr = xfrm_xml(pos.x, pos.y, pos.w, pos.h, el.rotation);
    sp_pr.push_str(&format!(
        "<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>\
         <a:rect l=\"0\" t=\"0\" r=\"{}\" b=\"{}\"/>\
         <a:pathLst>{}</a:pathLst></a:custGeom>",
        inch_to_emu(pos.w),
        inch_to_emu(pos.h),
        path_xml
    ));
    sp_pr.push_str(&ln_xml);

    format!(
        "<p:cxnSp>\n\
         <p:nvCxnSpPr>\n\
         <p:cNvPr id=\"{}\" name=\"{}\"/>\n\
         <p:cNvCxnSpPr/>\n\
         <p:nvPr/>\n\
         </p:nvCxnSpPr>\n\
         <p:spPr>{}</p:spPr>\n\
         </p:cxnSp>",
        sp_id, name, sp_pr,
    )
}

fn generate_background(
    bg: Option<&serde_json::Value>,
    r_id_map: &HashMap<String, String>,
) -> String {
    match bg {
        Some(serde_json::Value::String(s)) => {
            format!(
                "<p:bg><p:bgPr><a:solidFill>{}</a:solidFill><a:effectLst/></p:bgPr></p:bg>",
                color_xml(s)
            )
        }
        Some(serde_json::Value::Object(obj)) => {
            if let Some("gradient") = obj.get("type").and_then(|v| v.as_str()) {
                if let Some(stops) = obj.get("stops").and_then(|v| v.as_array()) {
                    let mut stops_xml = String::new();
                    for stop in stops {
                        let pos = stop.get("position").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let color = stop
                            .get("color")
                            .and_then(|v| v.as_str())
                            .unwrap_or("000000")
                            .trim_start_matches('#');
                        stops_xml.push_str(&format!(
                            "<a:gs pos=\"{}\"><a:srgbClr val=\"{}\"/></a:gs>",
                            (pos * 1000.0).round() as i64,
                            color
                        ));
                    }
                    let angle = obj.get("angle").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let ang = (angle * 60000.0).round() as i64;
                    return format!("<p:bg><p:bgPr><a:gradFill><a:gsLst>{}</a:gsLst><a:lin ang=\"{}\" scaled=\"1\"/></a:gradFill><a:effectLst/></p:bgPr></p:bg>", stops_xml, ang);
                }
            } else if let Some("image") = obj.get("type").and_then(|v| v.as_str()) {
                if let Some(src) = obj.get("src").and_then(|v| v.as_str()) {
                    if let Some(r_id) = r_id_map.get(src) {
                        return format!(
                            "<p:bg><p:bgPr><a:blipFill><a:blip r:embed=\"{}\"/><a:stretch><a:fillRect/></a:stretch></a:blipFill><a:effectLst/></p:bgPr></p:bg>",
                            r_id
                        );
                    }
                }
            }
            String::new()
        }
        _ => String::new(),
    }
}

fn generate_transition(transition: Option<&crate::model::Transition>) -> String {
    match transition {
        None => String::new(),
        Some(t) => {
            let speed = t.speed.as_deref().unwrap_or("med");
            let adv_click = match t.advance_on_click {
                Some(true) => " advClick=\"1\"",
                Some(false) => " advClick=\"0\"",
                None => "",
            };
            let adv_tm = t
                .advance_after
                .map(|a| format!(" advTm=\"{}\"", a))
                .unwrap_or_default();
            let type_xml = transition_ooxml(&t.r#type);
            format!(
                "<p:transition spd=\"{}\"{}{}>{}</p:transition>",
                speed, adv_click, adv_tm, type_xml
            )
        }
    }
}

fn av_lst_xml(adjust: Option<&std::collections::HashMap<String, f64>>) -> String {
    match adjust {
        None => "<a:avLst/>".to_string(),
        Some(map) if map.is_empty() => "<a:avLst/>".to_string(),
        Some(map) => {
            let mut gds = String::new();
            for (name, val) in map {
                gds.push_str(&format!(
                    "<a:gd name=\"{}\" fmla=\"val {}\"/>",
                    name,
                    val.round()
                ));
            }
            format!("<a:avLst>{}</a:avLst>", gds)
        }
    }
}

trait HasTextDefaults {
    fn default_font_size(&self) -> Option<f64>;
    fn default_color(&self) -> Option<&str>;
    fn default_line_spacing(&self) -> Option<f64>;
    fn default_run_props(&self) -> String;
    fn default_para_props(&self) -> String;
}

impl HasTextDefaults for ShapeElement {
    fn default_font_size(&self) -> Option<f64> {
        self.font_size
    }
    fn default_color(&self) -> Option<&str> {
        self.color.as_deref()
    }
    fn default_line_spacing(&self) -> Option<f64> {
        self.line_spacing
    }
    fn default_run_props(&self) -> String {
        run_props_xml(
            self.font_size,
            false,
            false,
            false,
            self.color.as_deref(),
            None,
        )
    }
    fn default_para_props(&self) -> String {
        para_props_xml_line_spacing(self.align.as_deref(), false, self.line_spacing)
    }
}

impl HasTextDefaults for TextElement {
    fn default_font_size(&self) -> Option<f64> {
        self.font_size
    }
    fn default_color(&self) -> Option<&str> {
        self.color.as_deref()
    }
    fn default_line_spacing(&self) -> Option<f64> {
        self.line_spacing
    }
    fn default_run_props(&self) -> String {
        run_props_xml(
            self.font_size,
            self.bold.unwrap_or(false),
            self.italic.unwrap_or(false),
            self.underline.unwrap_or(false),
            self.color.as_deref(),
            self.font_family.as_deref(),
        )
    }
    fn default_para_props(&self) -> String {
        para_props_xml_line_spacing(self.align.as_deref(), false, self.line_spacing)
    }
}
