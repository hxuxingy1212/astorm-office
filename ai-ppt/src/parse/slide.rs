//! 幻灯片 XML 解析模块
//!
//! 解析每张幻灯片 XML 中的元素、背景和过渡动画。
//! 支持文本、形状、图片、表格、分组等所有元素类型。

use super::background::parse_background;
use super::grp_sp::parse_grp_sp;
use super::pic::parse_pic;
use super::table::parse_graphic_frame;
use crate::model::elements::*;
use crate::model::transition::Transition;
use crate::model::Slide;
use crate::utils::constants::{arrow_end_json_name, dash_json_name, emu_to_inch, shape_json_name};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub(crate) fn parse(
    slide_xml: &str,
    rels_xml: &str,
    inherit: Option<&crate::parse::inherit::Inheritance>,
    charts: &HashMap<String, String>,
) -> Slide {
    let mut elements: Vec<Element> = Vec::new();
    let mut background = None;
    let mut has_bg = false;
    let mut transition = None;
    let mut in_sp_tree = false;
    let mut sp_depth = 0;
    let mut in_bg = false;
    let mut in_transition = false;
    let mut trans_speed: Option<String> = None;
    let mut trans_adv_click: Option<bool> = None;
    let mut trans_adv_after: Option<u64> = None;
    let mut buf = Vec::new();

    let r_id_to_src: std::collections::HashMap<String, String> = parse_rels(rels_xml);

    let mut reader = Reader::from_str(slide_xml);
    loop {
        let ev_pos = reader.buffer_position() as usize;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = super::tag(e).to_string();
                match name.as_str() {
                    "p:spTree" => {
                        in_sp_tree = true;
                        sp_depth = 0;
                    }
                    "p:bg" => {
                        in_bg = true;
                        has_bg = true;
                    }
                    "p:transition" => {
                        in_transition = true;
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "spd" {
                                trans_speed = Some(v.clone());
                            }
                            if k == "advClick" {
                                trans_adv_click = Some(v == "1");
                            }
                            if k == "advTm" {
                                if let Ok(tm) = v.parse() {
                                    trans_adv_after = Some(tm);
                                }
                            }
                        }
                    }
                    _ => {
                        if in_sp_tree {
                            sp_depth += 1;
                            if sp_depth == 1 {
                                if let Some(el) = parse_element_start(
                                    e,
                                    &r_id_to_src,
                                    &mut reader,
                                    inherit,
                                    charts,
                                    slide_xml,
                                    ev_pos,
                                ) {
                                    elements.push(el);
                                    sp_depth = 0;
                                }
                            }
                        }
                        if in_bg {
                            // bgPr 解析具体填充；bgRef 解析为 schemeClr 主题色引用
                            background = parse_background(e, &mut reader, &r_id_to_src);
                        }
                        if in_transition {
                            // Transition type is parsed from Empty event in outer match
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = super::tag(e).to_string();
                match name.as_str() {
                    "p:spTree" => in_sp_tree = false,
                    "p:bg" => in_bg = false,
                    "p:transition" => in_transition = false,
                    _ => {
                        if in_sp_tree {
                            sp_depth -= 1;
                        }
                    }
                }
            }
            Ok(Event::Empty(ref e)) if in_transition => {
                // 过渡类型即子元素局部名（命名空间无关；p14:morph → morph）
                let r#type = crate::utils::xml::tag_local(e);
                transition = Some(Transition {
                    r#type,
                    speed: trans_speed.take(),
                    advance_on_click: trans_adv_click.take(),
                    advance_after: trans_adv_after.take(),
                });
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    let sp_id_map = collect_sp_ids(slide_xml);
    if let Some(timing_xml) = extract_timing_block(slide_xml) {
        let anims = super::animation::parse_timing(&timing_xml, &HashMap::new());
        apply_animations(&mut elements, &anims, &sp_id_map);
    }

    // 幻灯片未声明背景元素时，才继承版式/母版背景
    if !has_bg && background.is_none() {
        if let Some(inh) = inherit {
            background = inh.background.clone();
        }
    }

    Slide {
        elements,
        background,
        transition,
        notes: None,
    }
}

fn collect_sp_ids(slide_xml: &str) -> HashMap<u32, usize> {
    let mut map = HashMap::new();
    let mut idx = 0usize;
    let mut reader = Reader::from_str(slide_xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e))
                // 局部名匹配，忽略命名空间前缀
                if e.local_name().as_ref() == b"cNvPr" => {
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == b"id" {
                            if let Ok(sp_id) = String::from_utf8_lossy(&a.value).parse::<u32>() {
                                if sp_id > 1 {
                                    map.insert(sp_id, idx);
                                    idx += 1;
                                }
                            }
                        }
                    }
                }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    map
}

fn extract_timing_block(slide_xml: &str) -> Option<String> {
    // 兼容任意命名空间前缀的 timing 开始标签
    let bytes = slide_xml.as_bytes();
    let mut pos = 0usize;
    let start = loop {
        let rel = slide_xml[pos..].find(":timing")? + pos;
        let after = rel + ":timing".len();
        let ok_after = matches!(
            bytes.get(after),
            Some(b'>') | Some(b' ') | Some(b'\r') | Some(b'\n') | Some(b'/')
        );
        let ok_before = slide_xml[..rel]
            .rfind('<')
            .map(|lt| !slide_xml[lt..rel].contains('>'))
            .unwrap_or(false);
        if ok_after && ok_before {
            break slide_xml[..rel].rfind('<')?;
        }
        pos = rel + 1;
    };
    // 从开始标签的 '>' 之后再找闭合标签，避免匹配到开始标签自身
    let body_start = slide_xml[start..].find('>')? + start + 1;
    let end = slide_xml[body_start..].find(":timing>")? + body_start + ":timing>".len();
    Some(slide_xml[start..end].to_string())
}

fn apply_animations(
    elements: &mut [Element],
    anims: &[(u32, Vec<Animation>)],
    sp_id_map: &HashMap<u32, usize>,
) {
    for (sp_id, anim_list) in anims {
        if anim_list.is_empty() {
            continue;
        }
        let Some(&el_idx) = sp_id_map.get(sp_id) else {
            continue;
        };
        if el_idx >= elements.len() {
            continue;
        }
        let el = &elements[el_idx];
        let replacement = match el {
            Element::Text(t) => {
                let mut t2 = t.clone();
                t2.animations = Some(anim_list.clone());
                Element::Text(t2)
            }
            Element::Shape(s) => {
                let mut s2 = s.clone();
                s2.animations = Some(anim_list.clone());
                Element::Shape(s2)
            }
            Element::Image(img) => {
                let mut i2 = img.clone();
                i2.animations = Some(anim_list.clone());
                Element::Image(i2)
            }
            Element::Table(tbl) => {
                let mut t2 = tbl.clone();
                t2.animations = Some(anim_list.clone());
                Element::Table(t2)
            }
            Element::Group(g) => {
                let mut g2 = g.clone();
                g2.animations = Some(anim_list.clone());
                Element::Group(g2)
            }
            _ => continue,
        };
        elements[el_idx] = replacement;
    }
}

fn parse_rels(rels_xml: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    if rels_xml.is_empty() {
        return map;
    }
    let mut reader = Reader::from_str(rels_xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                let name = super::tag(e).to_string();
                if name == "Relationship" {
                    let mut id = String::new();
                    let mut target = String::new();
                    for attr in e.attributes().flatten() {
                        let k = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        // 属性值需反转义实体（如 URL 中的 &amp;）
                        let v = attr
                            .unescape_value()
                            .unwrap_or_else(|_| {
                                std::borrow::Cow::Borrowed(
                                    std::str::from_utf8(&attr.value).unwrap_or(""),
                                )
                            })
                            .to_string();
                        if k == "Id" {
                            id = v.clone();
                        }
                        if k == "Target" {
                            target = v.clone();
                        }
                    }
                    if !id.is_empty() && !target.is_empty() {
                        map.insert(id, target);
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    map
}

pub(super) fn parse_element_start(
    e: &quick_xml::events::BytesStart,
    r_id_map: &std::collections::HashMap<String, String>,
    reader: &mut Reader<&[u8]>,
    inherit: Option<&crate::parse::inherit::Inheritance>,
    charts: &HashMap<String, String>,
    slide_xml: &str,
    elem_start: usize,
) -> Option<Element> {
    let name = super::tag(e).to_string();
    match name.as_str() {
        "p:sp" => parse_sp(reader, r_id_map, inherit, slide_xml),
        "p:pic" => parse_pic(reader, r_id_map),
        "p:graphicFrame" => parse_graphic_frame(reader, r_id_map, charts, slide_xml, elem_start),
        "p:grpSp" => parse_grp_sp(reader, r_id_map, inherit, charts, slide_xml),
        "p:cxnSp" => parse_sp(reader, r_id_map, inherit, slide_xml),
        _ => None,
    }
}

fn parse_sp(
    reader: &mut Reader<&[u8]>,
    r_id_map: &std::collections::HashMap<String, String>,
    inherit: Option<&crate::parse::inherit::Inheritance>,
    slide_xml: &str,
) -> Option<Element> {
    let mut depth = 1u32;
    let mut in_sp_pr = false;
    let mut in_tx_body = false;

    let mut position = Position {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };
    let mut shape_type: Option<String> = None;
    // 线条：custGeom 路径点（EMU 绝对坐标）、端点与虚线样式
    let mut path_pts: Vec<(i64, i64)> = Vec::new();
    let mut pt_kind = String::new();
    let mut quad_pt_index = 0u32;
    let mut smooth = false;
    let mut line_dash: Option<String> = None;
    let mut arrow_start: Option<String> = None;
    let mut arrow_end: Option<String> = None;
    let mut fill_json: Option<serde_json::Value> = None;
    let mut no_fill = false;
    let mut line_color = None;
    let mut line_width = 1.0;
    let mut fill_alpha: Option<f64> = None;
    let mut line_alpha: Option<f64> = None;
    let mut shadow: Option<Shadow> = None;
    let mut rotation = None;
    let mut name = None;
    let mut sp_id: Option<u32> = None;
    // 公式：捕获 a14:m 原始 OMML 子树
    let mut math_start: Option<usize> = None;
    let mut math_depth = 0u32;
    let mut omml: Option<String> = None;
    let mut math_text = String::new();
    // 占位符（p:ph）：用于从版式/母版继承位置与文字样式
    let mut ph_type: Option<String> = None;
    let mut ph_idx = String::new();
    let mut text: Option<TextContent> = None;
    let font_size = None;
    let bold = false;
    let italic = false;
    let underline = false;
    let color = None;
    let font_family = None;
    let align = None;
    let mut vert_align = None;
    let mut vert = None;
    let mut wrap = true;
    let mut is_tx_box = false;
    let mut text_depth = 0u32;
    let mut adjust: Option<std::collections::HashMap<String, f64>> = None;
    let mut in_ln = false;
    let mut in_outer_shdw = false;
    let mut shdw_blur = 4.0;
    let mut shdw_dist = 3.0;
    let mut shdw_angle = 45.0;
    let mut shdw_opacity = 0.4;
    let mut shdw_color = "000000".to_string();
    // 其他效果：发光/倒影/柔化边缘/模糊/内阴影/填充覆盖
    let mut in_glow = false;
    let mut glow_radius = 8.0;
    let mut glow_color = String::new();
    let mut glow_opacity: Option<f64> = None;
    let mut in_fill_overlay = false;
    let mut fo_color = String::new();
    let mut fo_opacity: Option<f64> = None;
    let mut in_inner_shdw = false;
    let mut inner_shadow: Option<Shadow> = None;
    let mut glow_fx: Option<Glow> = None;
    let mut fill_overlay_fx: Option<FillOverlay> = None;
    let mut soft_edge: Option<f64> = None;
    let mut blur_rad: Option<f64> = None;
    let mut reflection = false;
    // 图案填充
    let mut in_patt_fill = false;
    let mut patt_prst = String::new();
    let mut patt_fg = String::new();
    let mut patt_bg = String::new();
    let mut in_fg_clr = false;
    let mut in_bg_clr = false;
    let mut collect_text = String::new();
    let mut in_text_run = false;
    let mut current_run_font_size = None;
    let mut current_run_bold = false;
    let mut current_run_italic = false;
    let mut current_run_underline = false;
    let mut current_run_color = None;
    let mut current_run_font_family = None;
    let mut current_run_hyperlink: Option<String> = None;
    // bodyPr 自适应
    let mut autofit: Option<Autofit> = None;
    // txBody lstStyle 默认 defRPr（形状/文本框的段落级默认样式）
    let mut in_lst_style = false;
    let mut def_rpr_done = false;
    let mut in_def_rpr = false;
    let mut def_font_size: Option<f64> = None;
    let mut def_bold = false;
    let mut def_italic = false;
    let mut def_underline = false;
    let mut def_color: Option<String> = None;
    let mut def_font_family: Option<String> = None;
    let mut current_run_highlight: Option<String> = None;
    let mut current_run_underline_color: Option<String> = None;
    let mut current_run_spacing: Option<f64> = None;
    let mut current_run_lang: Option<String> = None;
    let mut current_run_rtl: Option<bool> = None;
    let mut runs: Vec<TextRun> = Vec::new();
    let mut in_p = false;
    let mut para_align: Option<String> = None;
    let mut para_bullet = false;
    let mut para_line_spacing: Option<f64> = None;
    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut in_solid_fill = false;
    let mut in_blip_fill = false;
    // p:style 引用（fillRef/lnRef/fontRef）推导出的默认样式
    let mut in_style = false;
    let mut cur_style_ref: Option<String> = None;
    let mut style_fill: Option<String> = None;
    let mut style_line: Option<String> = None;
    let mut style_font: Option<String> = None;
    // 颜色修饰符（lumMod/lumOff/tint/shade/...）缓存
    let mut in_color_el = false;
    let mut cur_color_base = String::new();
    let mut cur_color_mods: Vec<(String, String)> = Vec::new();
    let mut fill_color: Option<String> = None;
    let mut grad_stops: Vec<GradientStop> = Vec::new();
    let mut grad_angle: Option<f64> = None;
    let mut in_gs = false;
    let mut gs_pos = 0.0;
    let mut gs_color = String::new();
    let mut srgb_val = String::new();
    let mut scheme_val = String::new();

    let mut buf = Vec::new();
    loop {
        let ev_pos = reader.buffer_position() as usize;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e2)) => {
                let n = super::tag(e2).to_string();
                if math_start.is_some() {
                    math_depth += 1;
                } else {
                    let local = crate::utils::xml::tag_local(e2);
                    if local == "m" || local == "oMath" {
                        math_start = Some(ev_pos);
                        math_depth = 1;
                    }
                }
                match n.as_str() {
                    "p:nvSpPr" | "p:nvCxnSpPr" => {}
                    "p:cNvSpPr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "txBox" && v == "1" {
                                is_tx_box = true;
                            }
                        }
                    }
                    "p:cNvPr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "name" {
                                name = Some(v);
                            } else if k == "id" {
                                sp_id = v.parse::<u32>().ok();
                            }
                        }
                    }
                    "p:ph" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "type" {
                                ph_type = Some(v);
                            } else if k == "idx" {
                                ph_idx = v;
                            }
                        }
                        if ph_type.is_none() {
                            ph_type = Some(String::new());
                        }
                    }
                    "p:spPr" => {
                        in_sp_pr = true;
                    }
                    "a:xfrm" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "rot" {
                                if let Ok(r) = v.parse::<i64>() {
                                    rotation = Some(r as f64 / 60000.0);
                                }
                            }
                        }
                    }
                    "a:off" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "x" {
                                if let Ok(val) = v.parse::<i64>() {
                                    position.x = emu_to_inch(val);
                                }
                            }
                            if k == "y" {
                                if let Ok(val) = v.parse::<i64>() {
                                    position.y = emu_to_inch(val);
                                }
                            }
                        }
                    }
                    "a:ext" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "cx" {
                                if let Ok(val) = v.parse::<i64>() {
                                    position.w = emu_to_inch(val);
                                }
                            }
                            if k == "cy" {
                                if let Ok(val) = v.parse::<i64>() {
                                    position.h = emu_to_inch(val);
                                }
                            }
                        }
                    }
                    "a:prstGeom" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "prst" {
                                // OOXML prst 名反向映射为 JSON 形状名（查不到原样保留）
                                shape_type = Some(shape_json_name(&v).to_string());
                            }
                        }
                    }
                    "a:moveTo" => {
                        pt_kind = "moveTo".to_string();
                    }
                    "a:lnTo" => {
                        pt_kind = "lnTo".to_string();
                    }
                    "a:quadBezTo" => {
                        pt_kind = "quadBezTo".to_string();
                        quad_pt_index = 0;
                        smooth = true;
                    }
                    "a:pt" if !pt_kind.is_empty() => {
                        let mut px = 0i64;
                        let mut py = 0i64;
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "x" {
                                if let Ok(val) = v.parse::<i64>() {
                                    px = val;
                                }
                            }
                            if k == "y" {
                                if let Ok(val) = v.parse::<i64>() {
                                    py = val;
                                }
                            }
                        }
                        // quadBezTo 含两个点：控制点跳过，终点记录
                        if pt_kind == "quadBezTo" {
                            quad_pt_index += 1;
                            if quad_pt_index == 2 {
                                path_pts.push((px, py));
                            }
                        } else {
                            path_pts.push((px, py));
                        }
                    }
                    "a:avLst" => {
                        adjust = Some(std::collections::HashMap::new());
                    }
                    "a:gd" => {
                        if let Some(ref mut adj) = adjust {
                            let mut gd_name = String::new();
                            let mut gd_val = 0.0;
                            for a in e2.attributes().flatten() {
                                let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                                let v = String::from_utf8_lossy(&a.value).to_string();
                                if k == "name" {
                                    gd_name = v.clone();
                                }
                                if k == "fmla" {
                                    if let Some(val_str) = v.strip_prefix("val ") {
                                        gd_val = val_str.parse().unwrap_or(0.0);
                                    }
                                }
                            }
                            adj.insert(gd_name, gd_val);
                        }
                    }
                    "a:lstStyle" => {
                        in_lst_style = true;
                    }
                    "a:defRPr" if in_lst_style && !def_rpr_done => {
                        def_rpr_done = true;
                        in_def_rpr = true;
                        for a in e2.attributes().flatten() {
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            match a.key.as_ref() {
                                b"sz" => {
                                    if let Ok(s) = v.parse::<i64>() {
                                        def_font_size = Some(s as f64 / 100.0);
                                    }
                                }
                                b"b" => def_bold = v == "1" || v == "true",
                                b"i" => def_italic = v == "1" || v == "true",
                                b"u" => def_underline = v == "sng" || v == "true",
                                _ => {}
                            }
                        }
                    }
                    "a:normAutofit" => {
                        let mut fs = None;
                        let mut lsr = None;
                        for a in e2.attributes().flatten() {
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            match a.key.as_ref() {
                                b"fontScale" => {
                                    if let Ok(x) = v.parse::<i64>() {
                                        fs = Some(x as f64 / 100000.0);
                                    }
                                }
                                b"lnSpcReduction" => {
                                    if let Ok(x) = v.parse::<i64>() {
                                        lsr = Some(x as f64 / 100000.0);
                                    }
                                }
                                _ => {}
                            }
                        }
                        autofit = Some(Autofit {
                            kind: "normal".to_string(),
                            font_scale: fs,
                            ln_spc_reduction: lsr,
                        });
                    }
                    "a:spAutoFit" => {
                        autofit = Some(Autofit {
                            kind: "shape".to_string(),
                            font_scale: None,
                            ln_spc_reduction: None,
                        });
                    }
                    "a:noAutofit" => {
                        autofit = Some(Autofit {
                            kind: "none".to_string(),
                            font_scale: None,
                            ln_spc_reduction: None,
                        });
                    }
                    "a:blipFill" => {
                        in_blip_fill = true;
                    }
                    "a:blip" if in_blip_fill => {
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"r:embed" || a.key.as_ref() == b"r:link" {
                                let rid = String::from_utf8_lossy(&a.value).to_string();
                                if let Some(src) = r_id_map.get(&rid) {
                                    fill_json = Some(serde_json::json!({
                                        "type": "image",
                                        "src": src,
                                    }));
                                }
                            }
                        }
                    }
                    "a:solidFill" => {
                        in_solid_fill = true;
                        fill_color = None;
                    }
                    "a:gradFill" => {
                        grad_stops.clear();
                        grad_angle = None;
                    }
                    "a:gsLst" => {}
                    "a:gs" => {
                        in_gs = true;
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "pos" {
                                if let Ok(p) = v.parse::<i64>() {
                                    gs_pos = p as f64 / 1000.0;
                                }
                            }
                        }
                    }
                    "a:lin" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "ang" {
                                if let Ok(ang) = v.parse::<i64>() {
                                    grad_angle = Some(ang as f64 / 60000.0);
                                }
                            }
                        }
                    }
                    "a:srgbClr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "val" {
                                srgb_val = v;
                                if in_solid_fill {
                                    fill_color = Some(srgb_val.clone());
                                }
                                if in_ln {
                                    line_color = Some(srgb_val.clone());
                                }
                                if in_glow {
                                    glow_color = srgb_val.clone();
                                }
                                if in_fill_overlay {
                                    fo_color = srgb_val.clone();
                                }
                                if in_patt_fill && in_fg_clr {
                                    patt_fg = srgb_val.clone();
                                }
                                if in_patt_fill && in_bg_clr {
                                    patt_bg = srgb_val.clone();
                                }
                                if in_inner_shdw {
                                    if let Some(s) = inner_shadow.as_mut() {
                                        s.color = srgb_val.clone();
                                    }
                                }
                            }
                        }
                        if in_style {
                            route_style(
                                &mut style_fill,
                                &mut style_line,
                                &mut style_font,
                                &cur_style_ref,
                                &srgb_val,
                            );
                        }
                        if in_def_rpr {
                            def_color = Some(srgb_val.clone());
                        }
                        in_color_el = true;
                        cur_color_base = srgb_val.clone();
                        cur_color_mods.clear();
                    }
                    "a:schemeClr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "val" {
                                scheme_val = v;
                                if in_solid_fill {
                                    fill_color = Some(scheme_val.clone());
                                }
                                if in_ln {
                                    line_color = Some(scheme_val.clone());
                                }
                                if in_glow {
                                    glow_color = scheme_val.clone();
                                }
                                if in_fill_overlay {
                                    fo_color = scheme_val.clone();
                                }
                                if in_patt_fill && in_fg_clr {
                                    patt_fg = scheme_val.clone();
                                }
                                if in_patt_fill && in_bg_clr {
                                    patt_bg = scheme_val.clone();
                                }
                            }
                        }
                        let raw = String::from_utf8_lossy(e2.as_ref());
                        if let Some(idx) = raw.find("a:alpha val=\"") {
                            if let Some(val_str) = raw[idx..].split('"').nth(1) {
                                if let Ok(a) = val_str.parse::<i64>() {
                                    let alpha = a as f64 / 100000.0;
                                    if !in_ln {
                                        fill_alpha = Some(alpha);
                                    } else {
                                        line_alpha = Some(alpha);
                                    }
                                }
                            }
                        }
                        if in_style {
                            route_style(
                                &mut style_fill,
                                &mut style_line,
                                &mut style_font,
                                &cur_style_ref,
                                &scheme_val,
                            );
                        }
                        if in_def_rpr {
                            def_color = Some(scheme_val.clone());
                        }
                        in_color_el = true;
                        cur_color_base = scheme_val.clone();
                        cur_color_mods.clear();
                    }
                    "a:noFill" if !in_ln => {
                        no_fill = true;
                    }
                    "a:ln" => {
                        in_ln = true;
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "w" {
                                if let Ok(w) = v.parse::<i64>() {
                                    line_width = w as f64 / 12700.0;
                                }
                            }
                        }
                    }
                    "style" => {
                        in_style = true;
                    }
                    "fillRef" => {
                        cur_style_ref = Some("fill".to_string());
                    }
                    "lnRef" => {
                        cur_style_ref = Some("line".to_string());
                    }
                    "fontRef" => {
                        cur_style_ref = Some("font".to_string());
                    }
                    "effectRef" => {
                        cur_style_ref = Some("effect".to_string());
                    }
                    "a:lumMod" | "a:lumOff" | "a:tint" | "a:shade" | "a:alphaMod"
                    | "a:alphaOff"
                        if in_color_el =>
                    {
                        let name = n.trim_start_matches("a:").to_string();
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"val" {
                                cur_color_mods.push((
                                    name.clone(),
                                    String::from_utf8_lossy(&a.value).to_string(),
                                ));
                            }
                        }
                    }
                    "a:effectLst" => {}
                    "a:glow" => {
                        in_glow = true;
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"rad" {
                                if let Ok(r) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                                    glow_radius = r as f64 / 12700.0;
                                }
                            }
                        }
                    }
                    "a:fillOverlay" => {
                        in_fill_overlay = true;
                    }
                    "a:innerShdw" => {
                        in_inner_shdw = true;
                        let (mut b, mut d, mut ang) = (4.0_f64, 3.0_f64, 45.0_f64);
                        for a in e2.attributes().flatten() {
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            match a.key.as_ref() {
                                b"blurRad" => {
                                    if let Ok(x) = v.parse::<i64>() {
                                        b = x as f64 / 12700.0;
                                    }
                                }
                                b"dist" => {
                                    if let Ok(x) = v.parse::<i64>() {
                                        d = x as f64 / 12700.0;
                                    }
                                }
                                b"dir" => {
                                    if let Ok(x) = v.parse::<i64>() {
                                        ang = x as f64 / 60000.0;
                                    }
                                }
                                _ => {}
                            }
                        }
                        inner_shadow = Some(Shadow {
                            blur: b,
                            distance: d,
                            angle: ang,
                            opacity: 0.4,
                            color: "000000".to_string(),
                        });
                    }
                    "a:softEdge" => {
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"rad" {
                                if let Ok(r) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                                    soft_edge = Some(r as f64 / 12700.0);
                                }
                            }
                        }
                    }
                    "a:blur" => {
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"rad" {
                                if let Ok(r) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                                    blur_rad = Some(r as f64 / 12700.0);
                                }
                            }
                        }
                    }
                    "a:reflection" => {
                        reflection = true;
                    }
                    "a:pattFill" => {
                        in_patt_fill = true;
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"prst" {
                                patt_prst = String::from_utf8_lossy(&a.value).to_string();
                            }
                        }
                    }
                    "a:fgClr" => {
                        in_fg_clr = true;
                    }
                    "a:bgClr" => {
                        in_bg_clr = true;
                    }
                    "a:outerShdw" => {
                        in_outer_shdw = true;
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "blurRad" {
                                if let Ok(b) = v.parse::<i64>() {
                                    shdw_blur = b as f64 / 12700.0;
                                }
                            }
                            if k == "dist" {
                                if let Ok(d) = v.parse::<i64>() {
                                    shdw_dist = d as f64 / 12700.0;
                                }
                            }
                            if k == "dir" {
                                if let Ok(d) = v.parse::<i64>() {
                                    shdw_angle = d as f64 / 60000.0;
                                }
                            }
                            if k == "rotWithShape" {}
                        }
                    }
                    "a:alpha" => {
                        if in_outer_shdw {
                            for a in e2.attributes().flatten() {
                                let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                                let v = String::from_utf8_lossy(&a.value).to_string();
                                if k == "val" {
                                    if let Ok(o) = v.parse::<i64>() {
                                        shdw_opacity = o as f64 / 1000.0;
                                    }
                                }
                            }
                        }
                        if in_solid_fill && !in_ln {
                            for a in e2.attributes().flatten() {
                                let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                                let v = String::from_utf8_lossy(&a.value).to_string();
                                if k == "val" {
                                    if let Ok(o) = v.parse::<i64>() {
                                        fill_alpha = Some(o as f64 / 100000.0);
                                    }
                                }
                            }
                        }
                        if in_ln {
                            for a in e2.attributes().flatten() {
                                let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                                let v = String::from_utf8_lossy(&a.value).to_string();
                                if k == "val" {
                                    if let Ok(o) = v.parse::<i64>() {
                                        line_alpha = Some(o as f64 / 100000.0);
                                    }
                                }
                            }
                        }
                    }
                    "p:txBody" => {
                        in_tx_body = true;
                        text_depth = 0;
                    }
                    "a:bodyPr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "anchor" {
                                vert_align = Some(match v.as_str() {
                                    "ctr" => "middle".to_string(),
                                    "b" => "bottom".to_string(),
                                    _ => "top".to_string(),
                                });
                            }
                            if k == "wrap" && v == "none" {
                                wrap = false;
                            }
                            if k == "vert" && (v == "vert" || v == "vert270") {
                                vert = Some(v);
                            }
                        }
                    }
                    "a:p" => {
                        in_p = true;
                        collect_text.clear();
                    }
                    "a:pPr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "algn" {
                                para_align = Some(match v.as_str() {
                                    "ctr" => "center".to_string(),
                                    "r" => "right".to_string(),
                                    "just" => "justify".to_string(),
                                    _ => "left".to_string(),
                                });
                            }
                        }
                        let mut inner_buf = Vec::new();
                        loop {
                            match reader.read_event_into(&mut inner_buf) {
                                Ok(Event::Empty(ref child)) => {
                                    let bn = super::tag(child).to_string();
                                    if bn == "a:buChar" {
                                        para_bullet = true;
                                    }
                                }
                                Ok(Event::Start(ref child)) => {
                                    let bn = super::tag(child).to_string();
                                    // a:lnSpc > a:spcPct val="120000"（单位百分之一，120000=120%）
                                    if bn == "a:lnSpc" {
                                        let mut spc_buf = Vec::new();
                                        loop {
                                            match reader.read_event_into(&mut spc_buf) {
                                                Ok(Event::Empty(ref spc)) => {
                                                    let sn = super::tag(spc);
                                                    if sn == "a:spcPct" || sn == "a:spcPts" {
                                                        for a in spc.attributes().flatten() {
                                                            let k = String::from_utf8_lossy(
                                                                a.key.as_ref(),
                                                            )
                                                            .to_string();
                                                            let v =
                                                                String::from_utf8_lossy(&a.value)
                                                                    .to_string();
                                                            if k == "val" {
                                                                if let Ok(n) = v.parse::<i64>() {
                                                                    // spcPct：倍数；spcPts：固定磅值（用负值编码）
                                                                    para_line_spacing =
                                                                        Some(if sn == "a:spcPct" {
                                                                            n as f64 / 100000.0
                                                                        } else {
                                                                            -(n as f64 / 100.0)
                                                                        });
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                                Ok(Event::End(ref end))
                                                    if super::tag(end) == "a:lnSpc" =>
                                                {
                                                    break;
                                                }
                                                Ok(Event::Eof) => break,
                                                _ => {}
                                            }
                                            spc_buf.clear();
                                        }
                                    }
                                }
                                Ok(Event::End(ref end)) if super::tag(end) == "a:pPr" => {
                                    break;
                                }
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                            inner_buf.clear();
                        }
                    }
                    "a:r" => {}
                    "a:rPr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "sz" {
                                if let Ok(s) = v.parse::<i64>() {
                                    current_run_font_size = Some(s as f64 / 100.0);
                                }
                            }
                            if k == "b" {
                                current_run_bold = v == "1" || v == "true";
                            }
                            if k == "i" {
                                current_run_italic = v == "1" || v == "true";
                            }
                            if k == "u" {
                                current_run_underline = v == "sng" || v == "true";
                            }
                            if k == "spc" {
                                if let Ok(s) = v.parse::<i64>() {
                                    current_run_spacing = Some(s as f64 / 100.0);
                                }
                            }
                            if k == "lang" {
                                current_run_lang = Some(v.clone());
                            }
                            if k == "rtl" {
                                current_run_rtl = Some(v == "1" || v == "true");
                            }
                        }
                        let mut in_highlight = false;
                        let mut in_uln = false;
                        let mut inner_buf = Vec::new();
                        loop {
                            match reader.read_event_into(&mut inner_buf) {
                                Ok(Event::Empty(ref child)) => {
                                    let cn = super::tag(child).to_string();
                                    if cn == "a:srgbClr" || cn == "a:schemeClr" {
                                        for a in child.attributes().flatten() {
                                            if String::from_utf8_lossy(a.key.as_ref()) == "val" {
                                                let val =
                                                    String::from_utf8_lossy(&a.value).to_string();
                                                if in_highlight {
                                                    current_run_highlight = Some(val);
                                                } else {
                                                    current_run_color = Some(val);
                                                }
                                            }
                                        }
                                    } else if cn == "a:latin" || cn == "a:ea" {
                                        for a in child.attributes().flatten() {
                                            if String::from_utf8_lossy(a.key.as_ref()) == "typeface"
                                            {
                                                current_run_font_family = Some(
                                                    String::from_utf8_lossy(&a.value).to_string(),
                                                );
                                            }
                                        }
                                    } else if cn == "a:hlinkClick" {
                                        for a in child.attributes().flatten() {
                                            if String::from_utf8_lossy(a.key.as_ref()) == "r:id" {
                                                let rid =
                                                    String::from_utf8_lossy(&a.value).to_string();
                                                current_run_hyperlink =
                                                    r_id_map.get(&rid).cloned().or(Some(rid));
                                            }
                                        }
                                    }
                                }
                                Ok(Event::Start(ref child)) => {
                                    let cn = super::tag(child).to_string();
                                    if cn == "a:highlight" {
                                        in_highlight = true;
                                    } else if cn == "a:uLn" {
                                        in_uln = true;
                                    } else if cn == "a:solidFill" {
                                        let mut fill_buf = Vec::new();
                                        let next = reader.read_event_into(&mut fill_buf);
                                        let clr_event = match next {
                                            Ok(Event::Empty(ref clr)) => Some(clr),
                                            Ok(Event::Start(ref clr)) => Some(clr),
                                            _ => None,
                                        };
                                        if let Some(clr) = clr_event {
                                            let clr_n = super::tag(clr).to_string();
                                            if clr_n == "a:srgbClr" || clr_n == "a:schemeClr" {
                                                for a in clr.attributes().flatten() {
                                                    if String::from_utf8_lossy(a.key.as_ref())
                                                        == "val"
                                                    {
                                                        let val = String::from_utf8_lossy(&a.value)
                                                            .to_string();
                                                        if in_uln {
                                                            current_run_underline_color = Some(val);
                                                        } else if in_highlight {
                                                            current_run_highlight = Some(val);
                                                        } else {
                                                            current_run_color = Some(val);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Ok(Event::End(ref end)) if super::tag(end) == "a:rPr" => {
                                    break;
                                }
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                            inner_buf.clear();
                        }
                    }
                    "a:br" if in_p => {
                        collect_text.push('\n');
                    }
                    "a:t" if math_start.is_none() => {
                        in_text_run = true;
                    }
                    _ => {}
                }
                if in_tx_body && !in_sp_pr {
                    text_depth += 1;
                }
            }
            Ok(Event::Empty(ref e2)) => {
                let n = super::tag(e2).to_string();
                match n.as_str() {
                    "p:cNvSpPr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "txBox" && v == "1" {
                                is_tx_box = true;
                            }
                        }
                    }
                    "p:cNvPr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "name" {
                                name = Some(v);
                            } else if k == "id" {
                                sp_id = v.parse::<u32>().ok();
                            }
                        }
                    }
                    "p:ph" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "type" {
                                ph_type = Some(v);
                            } else if k == "idx" {
                                ph_idx = v;
                            }
                        }
                        if ph_type.is_none() {
                            ph_type = Some(String::new());
                        }
                    }
                    "a:off" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "x" {
                                if let Ok(val) = v.parse::<i64>() {
                                    position.x = emu_to_inch(val);
                                }
                            }
                            if k == "y" {
                                if let Ok(val) = v.parse::<i64>() {
                                    position.y = emu_to_inch(val);
                                }
                            }
                        }
                    }
                    "a:ext" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "cx" {
                                if let Ok(val) = v.parse::<i64>() {
                                    position.w = emu_to_inch(val);
                                }
                            }
                            if k == "cy" {
                                if let Ok(val) = v.parse::<i64>() {
                                    position.h = emu_to_inch(val);
                                }
                            }
                        }
                    }
                    "a:noFill" if !in_ln => {
                        no_fill = true;
                    }
                    "a:gd" => {
                        if let Some(ref mut adj) = adjust {
                            let mut gd_name = String::new();
                            let mut gd_val = 0.0;
                            for a in e2.attributes().flatten() {
                                let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                                let v = String::from_utf8_lossy(&a.value).to_string();
                                if k == "name" {
                                    gd_name = v.clone();
                                }
                                if k == "fmla" {
                                    if let Some(val_str) = v.strip_prefix("val ") {
                                        gd_val = val_str.parse().unwrap_or(0.0);
                                    }
                                }
                            }
                            adj.insert(gd_name, gd_val);
                        }
                    }
                    "a:pt" if !pt_kind.is_empty() => {
                        let mut px = 0i64;
                        let mut py = 0i64;
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "x" {
                                if let Ok(val) = v.parse::<i64>() {
                                    px = val;
                                }
                            }
                            if k == "y" {
                                if let Ok(val) = v.parse::<i64>() {
                                    py = val;
                                }
                            }
                        }
                        if pt_kind == "quadBezTo" {
                            quad_pt_index += 1;
                            if quad_pt_index == 2 {
                                path_pts.push((px, py));
                            }
                        } else {
                            path_pts.push((px, py));
                        }
                    }
                    "a:prstDash" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "val" {
                                line_dash = Some(dash_json_name(&v).to_string());
                            }
                        }
                    }
                    "a:headEnd" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "type" {
                                arrow_start = Some(arrow_end_json_name(&v).to_string());
                            }
                        }
                    }
                    "a:tailEnd" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "type" {
                                arrow_end = Some(arrow_end_json_name(&v).to_string());
                            }
                        }
                    }
                    "a:alpha" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "val" {
                                if let Ok(o) = v.parse::<i64>() {
                                    let alpha = o as f64 / 100000.0;
                                    if in_glow {
                                        glow_opacity = Some(alpha);
                                    } else if in_fill_overlay {
                                        fo_opacity = Some(alpha);
                                    } else if in_inner_shdw {
                                        if let Some(s) = inner_shadow.as_mut() {
                                            s.opacity = alpha;
                                        }
                                    } else if !in_ln {
                                        fill_alpha = Some(alpha);
                                    } else {
                                        line_alpha = Some(alpha);
                                    }
                                }
                            }
                        }
                    }
                    "a:softEdge" => {
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"rad" {
                                if let Ok(r) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                                    soft_edge = Some(r as f64 / 12700.0);
                                }
                            }
                        }
                    }
                    "a:blur" => {
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"rad" {
                                if let Ok(r) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                                    blur_rad = Some(r as f64 / 12700.0);
                                }
                            }
                        }
                    }
                    "a:reflection" => {
                        reflection = true;
                    }
                    "a:defRPr" if in_lst_style && !def_rpr_done => {
                        def_rpr_done = true;
                        for a in e2.attributes().flatten() {
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            match a.key.as_ref() {
                                b"sz" => {
                                    if let Ok(s) = v.parse::<i64>() {
                                        def_font_size = Some(s as f64 / 100.0);
                                    }
                                }
                                b"b" => def_bold = v == "1" || v == "true",
                                b"i" => def_italic = v == "1" || v == "true",
                                b"u" => def_underline = v == "sng" || v == "true",
                                _ => {}
                            }
                        }
                    }
                    "a:latin" if in_def_rpr => {
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"typeface" {
                                def_font_family =
                                    Some(String::from_utf8_lossy(&a.value).to_string());
                            }
                        }
                    }
                    "a:rPr" => {
                        // 自闭合 run 属性（无子元素时）
                        for a in e2.attributes().flatten() {
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            match a.key.as_ref() {
                                b"sz" => {
                                    if let Ok(s) = v.parse::<i64>() {
                                        current_run_font_size = Some(s as f64 / 100.0);
                                    }
                                }
                                b"b" => current_run_bold = v == "1" || v == "true",
                                b"i" => current_run_italic = v == "1" || v == "true",
                                b"u" => current_run_underline = v == "sng" || v == "true",
                                b"spc" => {
                                    if let Ok(s) = v.parse::<i64>() {
                                        current_run_spacing = Some(s as f64 / 100.0);
                                    }
                                }
                                b"lang" => current_run_lang = Some(v.clone()),
                                b"rtl" => current_run_rtl = Some(v == "1" || v == "true"),
                                _ => {}
                            }
                        }
                    }
                    "a:blip" if in_blip_fill => {
                        for a in e2.attributes().flatten() {
                            if a.key.as_ref() == b"r:embed" || a.key.as_ref() == b"r:link" {
                                let rid = String::from_utf8_lossy(&a.value).to_string();
                                if let Some(src) = r_id_map.get(&rid) {
                                    fill_json = Some(serde_json::json!({
                                        "type": "image",
                                        "src": src,
                                    }));
                                }
                            }
                        }
                    }
                    "a:bodyPr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "anchor" {
                                vert_align = Some(match v.as_str() {
                                    "ctr" => "middle".to_string(),
                                    "b" => "bottom".to_string(),
                                    _ => "top".to_string(),
                                });
                            }
                            if k == "wrap" && v == "none" {
                                wrap = false;
                            }
                        }
                    }
                    "a:pPr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "algn" {
                                para_align = Some(match v.as_str() {
                                    "ctr" => "center".to_string(),
                                    "r" => "right".to_string(),
                                    "just" => "justify".to_string(),
                                    _ => "left".to_string(),
                                });
                            }
                        }
                    }
                    "a:srgbClr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "val" {
                                srgb_val = v.clone();
                            }
                        }
                        if in_solid_fill {
                            fill_color = Some(srgb_val.clone());
                        }
                        if in_gs {
                            gs_color = srgb_val.clone();
                        }
                        if in_outer_shdw {
                            shdw_color = srgb_val.clone();
                        }
                        if in_ln {
                            line_color = Some(srgb_val.clone());
                        }
                        if in_glow {
                            glow_color = srgb_val.clone();
                        }
                        if in_fill_overlay {
                            fo_color = srgb_val.clone();
                        }
                        if in_patt_fill && in_fg_clr {
                            patt_fg = srgb_val.clone();
                        }
                        if in_patt_fill && in_bg_clr {
                            patt_bg = srgb_val.clone();
                        }
                        if in_inner_shdw {
                            if let Some(s) = inner_shadow.as_mut() {
                                s.color = srgb_val.clone();
                            }
                        }
                        if in_style {
                            route_style(
                                &mut style_fill,
                                &mut style_line,
                                &mut style_font,
                                &cur_style_ref,
                                &srgb_val,
                            );
                        }
                        if in_def_rpr {
                            def_color = Some(srgb_val.clone());
                        }
                        in_color_el = true;
                        cur_color_base = srgb_val.clone();
                        cur_color_mods.clear();
                    }
                    "a:schemeClr" => {
                        for a in e2.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "val" {
                                scheme_val = v;
                            }
                        }
                        if in_style {
                            route_style(
                                &mut style_fill,
                                &mut style_line,
                                &mut style_font,
                                &cur_style_ref,
                                &scheme_val,
                            );
                        }
                        if in_glow {
                            glow_color = scheme_val.clone();
                        }
                        if in_fill_overlay {
                            fo_color = scheme_val.clone();
                        }
                        if in_patt_fill && in_fg_clr {
                            patt_fg = scheme_val.clone();
                        }
                        if in_patt_fill && in_bg_clr {
                            patt_bg = scheme_val.clone();
                        }
                        if in_solid_fill {
                            fill_color = Some(scheme_val.clone());
                        }
                        if in_ln {
                            line_color = Some(scheme_val.clone());
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref t)) if in_text_run => {
                let text_content = t
                    .unescape()
                    .unwrap_or_else(|_| std::str::from_utf8(t.as_ref()).unwrap_or("").into())
                    .to_string();
                collect_text.push_str(&text_content);
            }
            Ok(Event::Text(ref t)) if math_start.is_some() => {
                if let Ok(txt) = t.unescape() {
                    math_text.push_str(&txt);
                }
            }
            Ok(Event::End(ref e2)) => {
                if math_start.is_some() {
                    math_depth = math_depth.saturating_sub(1);
                    if math_depth == 0 {
                        let start = math_start.take().unwrap();
                        let endpos = reader.buffer_position() as usize;
                        if endpos > start && endpos <= slide_xml.len() {
                            omml = Some(slide_xml[start..endpos].to_string());
                        }
                    }
                }
                let n = super::tag(e2).to_string();
                match n.as_str() {
                    "a:t" => {
                        in_text_run = false;
                    }
                    "a:r" => {
                        if !collect_text.is_empty() {
                            runs.push(TextRun {
                                text: std::mem::take(&mut collect_text),
                                font_size: current_run_font_size.take(),
                                bold: if current_run_bold { Some(true) } else { None },
                                italic: if current_run_italic { Some(true) } else { None },
                                underline: if current_run_underline {
                                    Some(true)
                                } else {
                                    None
                                },
                                color: current_run_color.take(),
                                font_family: current_run_font_family.take(),
                                hyperlink: current_run_hyperlink.take(),
                                highlight: current_run_highlight.take(),
                                underline_color: current_run_underline_color.take(),
                                spacing: current_run_spacing.take(),
                                lang: current_run_lang.take(),
                                rtl: current_run_rtl.take(),
                            });
                        }
                        current_run_font_size = None;
                        current_run_bold = false;
                        current_run_italic = false;
                        current_run_underline = false;
                        current_run_color = None;
                        current_run_font_family = None;
                        current_run_hyperlink = None;
                        current_run_highlight = None;
                        current_run_underline_color = None;
                        current_run_spacing = None;
                        current_run_lang = None;
                        current_run_rtl = None;
                    }
                    "a:p" => {
                        in_p = false;
                        if !runs.is_empty() || !collect_text.is_empty() {
                            let para_text = if !collect_text.is_empty() {
                                Some(std::mem::take(&mut collect_text))
                            } else {
                                None
                            };
                            paragraphs.push(Paragraph {
                                runs: if runs.is_empty() {
                                    None
                                } else {
                                    Some(std::mem::take(&mut runs))
                                },
                                text: para_text,
                                font_size: None,
                                bold: None,
                                italic: None,
                                color: None,
                                align: para_align.take(),
                                line_spacing: para_line_spacing.take(),
                                bullet: if para_bullet { Some(true) } else { None },
                            });
                        }
                        para_align = None;
                        para_bullet = false;
                        para_line_spacing = None;
                    }
                    "p:txBody" => {
                        in_tx_body = false;
                        text = if paragraphs.len() == 1
                            && paragraphs[0].runs.is_none()
                            && paragraphs[0].text.is_some()
                            && paragraphs[0].align.is_none()
                            && paragraphs[0].bullet.is_none()
                        {
                            if let Some(p) = paragraphs.first() {
                                p.text.as_ref().map(|t| TextContent::Simple(t.clone()))
                            } else {
                                None
                            }
                        } else if !paragraphs.is_empty() {
                            Some(TextContent::Paragraphs(std::mem::take(&mut paragraphs)))
                        } else {
                            None
                        };
                    }
                    "a:gs" => {
                        in_gs = false;
                        grad_stops.push(GradientStop {
                            color: gs_color.clone(),
                            position: gs_pos,
                        });
                    }
                    "a:gsLst" => {}
                    "a:lin" => {}
                    "a:gradFill" => {
                        fill_json = Some(serde_json::json!({
                            "type": "gradient",
                            "stops": grad_stops.iter().map(|s| serde_json::json!({
                                "color": s.color,
                                "position": s.position,
                            })).collect::<Vec<_>>(),
                            "angle": grad_angle,
                        }));
                    }
                    "a:blipFill" => {
                        in_blip_fill = false;
                    }
                    "a:solidFill" => {
                        in_solid_fill = false;
                        // 仅 spPr 内的填充生效；避免 txBody 内 run/endParaRPr 的 solidFill 污染形状填充
                        if !in_ln && !in_tx_body && !in_fill_overlay && !in_glow && !in_inner_shdw {
                            if let Some(c) = fill_color.take() {
                                fill_json = Some(serde_json::json!({ "Solid": c }));
                            }
                        }
                    }
                    "a:srgbClr" | "a:schemeClr" => {
                        in_color_el = false;
                        if !cur_color_mods.is_empty() {
                            let mods = cur_color_mods
                                .iter()
                                .map(|(k, v)| format!("{}={}", k, v))
                                .collect::<Vec<_>>()
                                .join(";");
                            let enc = format!("{}|{}", cur_color_base, mods);
                            if in_solid_fill {
                                fill_color = Some(enc.clone());
                            }
                            if in_ln {
                                line_color = Some(enc.clone());
                            }
                            if in_gs {
                                gs_color = enc.clone();
                            }
                            if in_glow {
                                glow_color = enc.clone();
                            }
                            if in_fill_overlay {
                                fo_color = enc.clone();
                            }
                            if in_patt_fill && in_fg_clr {
                                patt_fg = enc.clone();
                            }
                            if in_patt_fill && in_bg_clr {
                                patt_bg = enc.clone();
                            }
                            if in_outer_shdw {
                                shdw_color = enc.clone();
                            }
                            if in_inner_shdw {
                                if let Some(s) = inner_shadow.as_mut() {
                                    s.color = enc.clone();
                                }
                            }
                            if in_style {
                                route_style(
                                    &mut style_fill,
                                    &mut style_line,
                                    &mut style_font,
                                    &cur_style_ref,
                                    &enc,
                                );
                            }
                            cur_color_mods.clear();
                        }
                    }
                    "a:ln" => {
                        in_ln = false;
                    }
                    "style" => {
                        in_style = false;
                        cur_style_ref = None;
                    }
                    "fillRef" | "lnRef" | "fontRef" | "effectRef" => {
                        cur_style_ref = None;
                    }
                    "a:defRPr" => {
                        in_def_rpr = false;
                    }
                    "a:lstStyle" => {
                        in_lst_style = false;
                        in_def_rpr = false;
                    }
                    "a:outerShdw" => {
                        in_outer_shdw = false;
                        shadow = Some(Shadow {
                            blur: shdw_blur,
                            distance: shdw_dist,
                            angle: shdw_angle,
                            opacity: shdw_opacity,
                            color: shdw_color.clone(),
                        });
                    }
                    "a:glow" => {
                        in_glow = false;
                        if !glow_color.is_empty() {
                            glow_fx = Some(Glow {
                                color: glow_color.clone(),
                                radius: glow_radius,
                                opacity: glow_opacity,
                            });
                        }
                    }
                    "a:fillOverlay" => {
                        in_fill_overlay = false;
                        if !fo_color.is_empty() {
                            fill_overlay_fx = Some(FillOverlay {
                                color: fo_color.clone(),
                                opacity: fo_opacity,
                            });
                        }
                    }
                    "a:innerShdw" => {
                        in_inner_shdw = false;
                    }
                    "a:fgClr" => {
                        in_fg_clr = false;
                    }
                    "a:bgClr" => {
                        in_bg_clr = false;
                    }
                    "a:pattFill" => {
                        in_patt_fill = false;
                        if !patt_prst.is_empty() {
                            fill_json = Some(serde_json::json!({
                                "Pattern": {
                                    "pattern": patt_prst,
                                    "fg": patt_fg,
                                    "bg": patt_bg,
                                }
                            }));
                        }
                    }
                    "a:effectLst" => {}
                    "a:avLst" => {}
                    "a:prstGeom" => {}
                    "p:spPr" => {
                        in_sp_pr = false;
                    }
                    "p:nvSpPr" | "p:nvCxnSpPr" => {}
                    "p:sp" | "p:cxnSp" => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                if in_tx_body
                    && !in_sp_pr
                    && n != "p:txBody"
                    && n != "a:p"
                    && n != "a:r"
                    && n != "a:t"
                {
                    text_depth = text_depth.saturating_sub(1);
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    // 汇总效果
    let effects = {
        let mut e = Effects::default();
        e.glow = glow_fx;
        e.fill_overlay = fill_overlay_fx;
        e.inner_shadow = inner_shadow;
        e.soft_edge = soft_edge;
        e.blur = blur_rad;
        if reflection {
            e.reflection = Some(true);
        }
        if e.is_empty() {
            None
        } else {
            Some(e)
        }
    };

    // p:style 回退：无显式填充/线条时采用 fillRef/lnRef
    let mut fill_resolved = parse_fill(&fill_json);
    if fill_resolved.is_none() && !no_fill {
        if let Some(sf) = &style_fill {
            fill_resolved = Some(Fill::Solid(sf.clone()));
        }
    }
    if line_color.is_none() {
        if let Some(sl) = &style_line {
            line_color = Some(sl.clone());
        }
    }
    let text_color_resolved = color
        .or(parse_text_color(&text))
        .or_else(|| style_font.clone());
    let fill_shape = if no_fill { None } else { fill_resolved.clone() };
    let placeholder_str = ph_type.as_ref().map(|t| format!("{}|{}", t, ph_idx));

    // Build element
    // 线条：仅 custGeom 路径（p:cxnSp 连接线）判定为 Line；
    // prstGeom prst="line" 是预设直线形状，仍保留为 Shape。
    let is_line = path_pts.len() >= 2;
    let mut result = if is_line {
        // custGeom 路径点为 EMU 绝对坐标，转换为相对 position 的英寸
        let points: Vec<(f64, f64)> = path_pts
            .iter()
            .map(|(x, y)| (emu_to_inch(*x) - position.x, emu_to_inch(*y) - position.y))
            .collect();
        Element::Line(LineElement {
            position,
            name,
            id: sp_id,
            points,
            color: line_color,
            width: if (line_width - 1.0).abs() > f64::EPSILON {
                Some(line_width)
            } else {
                None
            },
            dash: line_dash,
            arrow_start,
            arrow_end,
            smooth: if smooth { Some(true) } else { None },
            rotation,
            animations: None,
        })
    } else if shape_type.is_some() && !is_tx_box {
        Element::Shape(ShapeElement {
            placeholder: placeholder_str.clone(),
            autofit: None,
            effects: effects.clone(),
            shape_type: shape_type.unwrap_or_default(),
            position,
            name,
            id: sp_id,
            rotation,
            fill: fill_shape.clone(),
            fill_alpha,
            no_fill: if no_fill { Some(true) } else { None },
            line: line_color.map(|c| Line {
                color: c,
                width: line_width,
            }),
            line_alpha,
            shadow,
            adjust,
            text: text.clone(),
            font_size: font_size.or(parse_text_font_size(&text)).or(def_font_size),
            color: text_color_resolved.clone().or_else(|| def_color.clone()),
            align: align.or(parse_text_align(&text)),
            vert_align,
            line_spacing: parse_text_line_spacing(&text),
            animations: None,
        })
    } else {
        Element::Text(TextElement {
            placeholder: placeholder_str.clone(),
            autofit: None,
            effects: effects.clone(),
            text: text.clone().unwrap_or(TextContent::Simple(String::new())),
            position,
            name,
            id: sp_id,
            font_size: font_size.or(parse_text_font_size(&text)).or(def_font_size),
            bold: if bold || def_bold { Some(true) } else { None },
            italic: if italic || def_italic {
                Some(true)
            } else {
                None
            },
            underline: if underline || def_underline {
                Some(true)
            } else {
                None
            },
            color: text_color_resolved.clone().or_else(|| def_color.clone()),
            font_family: font_family
                .or(parse_text_font_family(&text))
                .or(def_font_family.clone()),
            align: align.or(parse_text_align(&text)),
            vert_align,
            vert,
            line_spacing: parse_text_line_spacing(&text),
            wrap: Some(wrap),
            fill: fill_resolved.clone(),
            fill_alpha,
            line: line_color.map(|c| Line {
                color: c,
                width: line_width,
            }),
            line_alpha,
            shadow,
            animations: None,
        })
    };

    // 含 a14:m 的形状视为公式元素
    if let Some(omml) = omml {
        let pos = result.position().clone();
        let eq_name = result.name().map(|s| s.to_string());
        let text = if math_text.trim().is_empty() {
            None
        } else {
            Some(math_text.trim().to_string())
        };
        result = Element::Equation(EquationElement {
            position: pos,
            name: eq_name,
            id: sp_id,
            omml,
            text,
        });
    }

    // 自适应设置
    if let Some(af) = autofit {
        match &mut result {
            Element::Text(t) => t.autofit = Some(af),
            Element::Shape(s) => s.autofit = Some(af),
            _ => {}
        }
    }

    // 占位符继承：版式/母版补齐位置与文字默认样式
    if let (Some(inh), Some(t)) = (inherit, ph_type.as_deref()) {
        let style = inh.resolve(t, &ph_idx);
        let is_zero = {
            let p = result.position();
            p.x == 0.0 && p.y == 0.0 && p.w == 0.0 && p.h == 0.0
        };
        if is_zero {
            if let Some(p) = &style.pos {
                *result.position_mut() = p.clone();
            }
        }
        let inherited_bullet = style.bullet;
        match &mut result {
            Element::Shape(s) => {
                if s.font_size.is_none() {
                    s.font_size = style.font_size;
                }
                if s.color.is_none() {
                    s.color = style.color.clone();
                }
                if let Some(text) = &mut s.text {
                    apply_inherited_bullet(text, inherited_bullet);
                }
            }
            Element::Text(t2) => {
                if t2.font_size.is_none() {
                    t2.font_size = style.font_size;
                }
                if t2.color.is_none() {
                    t2.color = style.color.clone();
                }
                if t2.font_family.is_none() {
                    t2.font_family = style.font_family.clone();
                }
                if t2.bold.is_none() && style.bold == Some(true) {
                    t2.bold = style.bold;
                }
                if t2.italic.is_none() && style.italic == Some(true) {
                    t2.italic = style.italic;
                }
                apply_inherited_bullet(&mut t2.text, inherited_bullet);
            }
            _ => {}
        }
    }

    Some(result)
}

fn parse_text_font_size(text: &Option<TextContent>) -> Option<f64> {
    match text {
        Some(TextContent::Paragraphs(paras)) => {
            for p in paras {
                if let Some(runs) = &p.runs {
                    if let Some(r) = runs.first() {
                        return r.font_size;
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn parse_text_color(text: &Option<TextContent>) -> Option<String> {
    match text {
        Some(TextContent::Paragraphs(paras)) => {
            for p in paras {
                if let Some(runs) = &p.runs {
                    if let Some(r) = runs.first() {
                        return r.color.clone();
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn parse_text_font_family(text: &Option<TextContent>) -> Option<String> {
    match text {
        Some(TextContent::Paragraphs(paras)) => {
            for p in paras {
                if let Some(runs) = &p.runs {
                    if let Some(r) = runs.first() {
                        return r.font_family.clone();
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn parse_text_align(text: &Option<TextContent>) -> Option<String> {
    match text {
        Some(TextContent::Paragraphs(paras)) => paras.first().and_then(|p| p.align.clone()),
        _ => None,
    }
}

fn parse_text_line_spacing(text: &Option<TextContent>) -> Option<f64> {
    match text {
        Some(TextContent::Paragraphs(paras)) => paras.iter().find_map(|p| p.line_spacing),
        _ => None,
    }
}

/// 将继承得到的项目符号应用到未显式设置的段落
fn apply_inherited_bullet(text: &mut TextContent, bullet: Option<bool>) {
    if let Some(b) = bullet {
        if let TextContent::Paragraphs(paras) = text {
            for p in paras.iter_mut() {
                if p.bullet.is_none() {
                    p.bullet = Some(b);
                }
            }
        }
    }
}

/// 将 p:style 子引用的颜色按引用类型（fill/line/font）分派
fn route_style(
    fill: &mut Option<String>,
    line: &mut Option<String>,
    font: &mut Option<String>,
    which: &Option<String>,
    val: &str,
) {
    match which.as_deref() {
        Some("fill") => *fill = Some(val.to_string()),
        Some("line") => *line = Some(val.to_string()),
        Some("font") => *font = Some(val.to_string()),
        _ => {}
    }
}

fn parse_fill(fill_json: &Option<serde_json::Value>) -> Option<Fill> {
    match fill_json {
        Some(serde_json::Value::Object(obj)) => {
            if let Some(serde_json::Value::Array(stops)) = obj.get("stops") {
                let parsed_stops: Vec<GradientStop> = stops
                    .iter()
                    .filter_map(|s| {
                        Some(GradientStop {
                            color: s.get("color")?.as_str()?.to_string(),
                            position: s.get("position")?.as_f64()?,
                        })
                    })
                    .collect();
                if !parsed_stops.is_empty() {
                    return Some(Fill::Gradient {
                        stops: parsed_stops,
                        angle: obj.get("angle").and_then(|v| v.as_f64()),
                    });
                }
            }
            if obj.get("type").and_then(|v| v.as_str()) == Some("image") {
                if let Some(src) = obj.get("src").and_then(|v| v.as_str()) {
                    return Some(Fill::Image {
                        src: src.to_string(),
                    });
                }
            }
            if let Some(pat) = obj.get("Pattern") {
                if let (Some(p), Some(fg), Some(bg)) = (
                    pat.get("pattern").and_then(|v| v.as_str()),
                    pat.get("fg").and_then(|v| v.as_str()),
                    pat.get("bg").and_then(|v| v.as_str()),
                ) {
                    return Some(Fill::Pattern {
                        pattern: p.to_string(),
                        fg: fg.to_string(),
                        bg: bg.to_string(),
                    });
                }
            }
            if let Some(color) = obj.get("Solid").and_then(|v| v.as_str()) {
                return Some(Fill::Solid(color.to_string()));
            }
            None
        }
        Some(serde_json::Value::String(s)) => Some(Fill::Solid(s.clone())),
        _ => None,
    }
}
