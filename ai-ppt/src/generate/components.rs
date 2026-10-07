//! 高级组件展开模块
//!
//! 将高级组件（进度条、柱状图、KPI卡片等）展开为多个基础 OOXML 形状。
//! 每个组件被包装在 p:grpSp（组合形状）中。

use crate::model::elements::*;
use crate::utils::constants::*;
use crate::utils::xml::*;

/// 将高级组件展开为 OOXML 形状 XML
///
/// # 参数
/// - `comp`: 组件引用（根据枚举变体分发到对应展开函数）
/// - `sp_id`: 形状 ID 计数器（会被修改）
pub fn expand_component(comp: &Element, sp_id: &mut u32) -> String {
    let group_id = *sp_id;
    *sp_id += 1;
    let (pos, children) = match comp {
        Element::ProgressBar(c) => (&c.position, expand_progress_bar(c, sp_id)),
        Element::ProgressRing(c) => (&c.position, expand_progress_ring(c, sp_id)),
        Element::BarChart(c) => (&c.position, expand_bar_chart(c, sp_id)),
        Element::LineChart(c) => (&c.position, expand_line_chart(c, sp_id)),
        Element::PieChart(c) => (&c.position, expand_pie_chart(c, sp_id)),
        Element::RingChart(c) => (&c.position, expand_ring_chart(c, sp_id)),
        Element::KpiCard(c) => (&c.position, expand_kpi_card(c, sp_id)),
        Element::RatingStars(c) => (&c.position, expand_rating_stars(c, sp_id)),
        Element::Timeline(c) => (&c.position, expand_timeline(c, sp_id)),
        Element::ProcessFlow(c) => (&c.position, expand_process_flow(c, sp_id)),
        _ => return String::new(),
    };

    format!(
        "<p:grpSp>\n\
         <p:nvGrpSpPr>\n\
         <p:cNvPr id=\"{}\" name=\"Component {}\"/>\n\
         <p:cNvGrpSpPr/>\n<p:nvPr/>\n\
         </p:nvGrpSpPr>\n\
         <p:grpSpPr>\n\
         <a:xfrm>\n\
         <a:off x=\"{}\" y=\"{}\"/>\n\
         <a:ext cx=\"{}\" cy=\"{}\"/>\n\
         <a:chOff x=\"{}\" y=\"{}\"/>\n\
         <a:chExt cx=\"{}\" cy=\"{}\"/>\n\
         </a:xfrm>\n\
         </p:grpSpPr>\n\
         {}\
         </p:grpSp>",
        group_id,
        group_id,
        inch_to_emu(pos.x),
        inch_to_emu(pos.y),
        inch_to_emu(pos.w),
        inch_to_emu(pos.h),
        inch_to_emu(pos.x),
        inch_to_emu(pos.y),
        inch_to_emu(pos.w),
        inch_to_emu(pos.h),
        children,
    )
}

fn expand_progress_bar(comp: &ProgressBar, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let value = comp.value.unwrap_or(50.0).clamp(0.0, 100.0);
    let color = comp.color.as_deref().unwrap_or("4472C4");
    let track_color = comp.track_color.as_deref().unwrap_or("E0E0E0");
    let rounded = comp.rounded.unwrap_or(true);
    let show_label = comp.show_label.unwrap_or(true);
    let label_txt = comp.label.as_deref().unwrap_or("");
    let text_color = comp.text_color.as_deref().unwrap_or("000000");
    let font_size = comp.font_size.unwrap_or(14.0);
    let shape_type = if rounded { "roundRect" } else { "rect" };
    let fill_width = pos.w * value / 100.0;
    let label_str = if !label_txt.is_empty() {
        label_txt.to_string()
    } else {
        format!("{:.0}%", value)
    };

    let mut out = String::new();
    *sp_id += 1;
    out.push_str(&format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Track {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
         <p:spPr>{}<a:prstGeom prst=\"{}\"><a:avLst/></a:prstGeom>{}</p:spPr></p:sp>",
        *sp_id,
        *sp_id,
        xfrm_xml(pos.x, pos.y, pos.w, pos.h, None),
        shape_type,
        solid_fill_xml(track_color),
    ));
    if fill_width > 0.0 {
        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Fill {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"{}\"><a:avLst/></a:prstGeom>{}</p:spPr></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(pos.x, pos.y, fill_width, pos.h, None),
            shape_type,
            solid_fill_xml(color),
        ));
    }
    if show_label {
        *sp_id += 1;
        let rpr = run_props_xml(Some(font_size), false, false, false, Some(text_color), None);
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Label {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
             <p:txBody><a:bodyPr wrap=\"square\" anchor=\"l\"/><a:lstStyle/>\
             <a:p><a:pPr algn=\"l\"/>{}</a:p></p:txBody></p:sp>",
            *sp_id, *sp_id, xfrm_xml(pos.x + pos.w + 0.1, pos.y, 1.5, pos.h, None),
            format_args!("<a:r>{}<a:t>{}</a:t></a:r>", rpr, esc_xml(&label_str)),
        ));
    }
    out
}

fn expand_progress_ring(comp: &ProgressRing, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let value = comp.value.unwrap_or(50.0).clamp(0.0, 100.0);
    let color = comp.color.as_deref().unwrap_or("4472C4");
    let track_color = comp.track_color.as_deref().unwrap_or("E0E0E0");
    let thickness = comp.thickness.unwrap_or(8.0);
    let show_label = comp.show_label.unwrap_or(true);
    let text_color = comp.text_color.as_deref().unwrap_or("000000");
    let font_size = comp.font_size.unwrap_or(18.0);
    let cx = pos.w.min(pos.h);
    let ring_angle = (value / 100.0 * 36000.0).round() as i64;
    let label_str = format!("{:.0}%", value);

    let mut out = String::new();
    *sp_id += 1;
    out.push_str(&format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"RingTrack {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
         <p:spPr>{}<a:prstGeom prst=\"ellipse\"><a:avLst/></a:prstGeom><a:noFill/>\
         <a:ln w=\"{}\">{}</a:ln></p:spPr></p:sp>",
        *sp_id, *sp_id,
        xfrm_xml(pos.x, pos.y, cx, cx, None),
        (thickness * 12700.0).round() as i64,
        solid_fill_xml(track_color),
    ));
    if value > 0.0 {
        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"RingFill {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"blockArc\"><a:avLst><a:gd name=\"adj\" fmla=\"val {}\"/></a:avLst></a:prstGeom>\
             <a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill>\
             <a:ln w=\"{}\"><a:solidFill><a:srgbClr val=\"{}\"/></a:solidFill></a:ln></p:spPr></p:sp>",
            *sp_id, *sp_id,
            xfrm_xml(pos.x, pos.y, cx, cx, None),
            ring_angle,
            color, (thickness * 12700.0).round() as i64, color,
        ));
    }
    if show_label {
        *sp_id += 1;
        let rpr = run_props_xml(Some(font_size), true, false, false, Some(text_color), None);
        let label_x = pos.x + cx * 0.15;
        let label_y = pos.y + cx * 0.3;
        let label_w = cx * 0.7;
        let label_h = cx * 0.4;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"RingLabel {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
             <p:txBody><a:bodyPr wrap=\"square\" anchor=\"ctr\"/><a:lstStyle/>\
             <a:p><a:pPr algn=\"ctr\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
            *sp_id, *sp_id,
            xfrm_xml(label_x, label_y, label_w, label_h, None),
            rpr, esc_xml(&label_str),
        ));
    }
    out
}

fn expand_bar_chart(comp: &BarChart, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let data = &comp.data;
    if data.is_empty() {
        return String::new();
    }

    let labels = comp.labels.clone().unwrap_or_default();
    let colors = comp.colors.clone().unwrap_or_default();
    let max_val = comp
        .max
        .unwrap_or_else(|| data.iter().cloned().fold(0.0_f64, f64::max));
    let show_values = comp.show_values.unwrap_or(true);
    let axis = comp.axis.unwrap_or(true);
    let label_color = comp.label_color.as_deref().unwrap_or("000000");
    let font_size = comp.font_size.unwrap_or(12.0);
    let max_val = if max_val <= 0.0 { 1.0 } else { max_val };

    let n = data.len();
    let gap = pos.w * 0.1;
    let bar_area = pos.w - gap;
    let bar_total_w = bar_area / n.max(1) as f64;
    let bar_w = bar_total_w * 0.7;
    let chart_h = pos.h * 0.85;
    let baseline_y = pos.y + chart_h;

    let mut out = String::new();

    if axis {
        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Axis {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>\
             <a:ln w=\"12700\">{}</a:ln></p:spPr></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(pos.x, baseline_y, pos.w, 0.01, None),
            solid_fill_xml("CCCCCC"),
        ));
    }

    for (i, &val) in data.iter().enumerate() {
        let bar_h = (val / max_val) * chart_h;
        let bar_x = pos.x + gap / 2.0 + i as f64 * bar_total_w;
        let bar_y = baseline_y - bar_h;
        let c = colors.get(i).map(|s| s.as_str()).unwrap_or("4472C4");

        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Bar {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>{}</p:spPr></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(bar_x, bar_y, bar_w, bar_h, None),
            solid_fill_xml(c),
        ));

        if show_values && bar_h > 0.3 {
            *sp_id += 1;
            let rpr = run_props_xml(
                Some(font_size * 0.8),
                false,
                false,
                false,
                Some(label_color),
                None,
            );
            let val_str = format!("{:.1}", val);
            let vw = val_str.len() as f64 * 0.15;
            let vx = bar_x + (bar_w - vw) / 2.0;
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Val {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr wrap=\"square\" anchor=\"b\"/><a:lstStyle/>\
                 <a:p><a:pPr algn=\"ctr\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
                *sp_id, *sp_id, xfrm_xml(vx, bar_y - 0.35, vw.max(0.5), 0.35, None),
                rpr, esc_xml(&val_str),
            ));
        }

        if let Some(lbl) = labels.get(i) {
            *sp_id += 1;
            let rpr = run_props_xml(
                Some(font_size),
                false,
                false,
                false,
                Some(label_color),
                None,
            );
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Lbl {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr wrap=\"square\" anchor=\"t\"/><a:lstStyle/>\
                 <a:p><a:pPr algn=\"ctr\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
                *sp_id, *sp_id, xfrm_xml(bar_x, baseline_y + 0.05, bar_w, 0.35, None),
                rpr, esc_xml(lbl),
            ));
        }
    }
    out
}

/// 折线图展开：坐标轴 + 连接线段 + 数据点 + 标签
fn expand_line_chart(comp: &LineChart, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let data = &comp.data;
    if data.is_empty() {
        return String::new();
    }

    let labels = comp.labels.clone().unwrap_or_default();
    let colors = comp.colors.clone().unwrap_or_default();
    let line_color = colors.first().map(|s| s.as_str()).unwrap_or("4472C4");
    let max_val = comp
        .max
        .unwrap_or_else(|| data.iter().cloned().fold(0.0_f64, f64::max));
    let show_values = comp.show_values.unwrap_or(true);
    let axis = comp.axis.unwrap_or(true);
    let smooth = comp.smooth.unwrap_or(false);
    let label_color = comp.label_color.as_deref().unwrap_or("000000");
    let font_size = comp.font_size.unwrap_or(12.0);
    let max_val = if max_val <= 0.0 { 1.0 } else { max_val };

    let n = data.len();
    let gap = pos.w * 0.1;
    let plot_h = pos.h * 0.8;
    let baseline_y = pos.y + plot_h;
    let seg_w = (pos.w - gap) / (n.max(2) - 1) as f64;
    let cx = pos.w * 0.5;

    // 数据点坐标（绝对英寸）
    let pts: Vec<(f64, f64)> = data
        .iter()
        .enumerate()
        .map(|(i, &val)| {
            let x = if n == 1 {
                pos.x + gap / 2.0 + cx
            } else {
                pos.x + gap / 2.0 + i as f64 * seg_w
            };
            let y = baseline_y - (val / max_val) * plot_h;
            (x, y)
        })
        .collect();

    let mut out = String::new();

    if axis {
        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Axis {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>\
             <a:ln w=\"12700\">{}</a:ln></p:spPr></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(pos.x, baseline_y, pos.w, 0.01, None),
            solid_fill_xml("CCCCCC"),
        ));
    }

    // 折线段（相邻点之间一条连接线；平滑时用二次贝塞尔）
    for w in pts.windows(2) {
        let (x1, y1) = w[0];
        let (x2, y2) = w[1];
        let mut path = String::from("<a:pathLst><a:path w=\"100000\" h=\"100000\">");
        path.push_str(&format!(
            "<a:moveTo><a:pt x=\"{}\" y=\"{}\"/></a:moveTo>",
            inch_to_emu(x1),
            inch_to_emu(y1)
        ));
        if smooth {
            let (cxm, cym) = (
                (inch_to_emu(x1) + inch_to_emu(x2)) / 2,
                (inch_to_emu(y1) + inch_to_emu(y2)) / 2,
            );
            path.push_str(&format!(
                "<a:quadBezTo><a:pt x=\"{}\" y=\"{}\"/><a:pt x=\"{}\" y=\"{}\"/></a:quadBezTo>",
                cxm,
                cym,
                inch_to_emu(x2),
                inch_to_emu(y2)
            ));
        } else {
            path.push_str(&format!(
                "<a:lnTo><a:pt x=\"{}\" y=\"{}\"/></a:lnTo>",
                inch_to_emu(x2),
                inch_to_emu(y2)
            ));
        }
        path.push_str("</a:path></a:pathLst>");
        *sp_id += 1;
        out.push_str(&format!(
            "<p:cxnSp><p:nvCxnSpPr><p:cNvPr id=\"{}\" name=\"LineSeg {}\"/><p:cNvCxnSpPr/><p:nvPr/></p:nvCxnSpPr>\
             <p:spPr>{}<a:custGeom><a:avLst/><a:gdLst/><a:ahLst/><a:cxnLst/>\
             <a:rect l=\"0\" t=\"0\" r=\"100000\" b=\"100000\"/>{}</a:custGeom>\
             <a:ln w=\"12700\">{}</a:ln></p:spPr></p:cxnSp>",
            *sp_id,
            *sp_id,
            xfrm_xml(pos.x, pos.y, pos.w, pos.h, None),
            path,
            solid_fill_xml(line_color),
        ));
    }

    // 数据点与标签
    for (i, &(px, py)) in pts.iter().enumerate() {
        let c = colors.get(i).map(|s| s.as_str()).unwrap_or(line_color);
        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Point {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"ellipse\"><a:avLst/></a:prstGeom>{}</p:spPr></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(px - 0.09, py - 0.09, 0.18, 0.18, None),
            solid_fill_xml(c),
        ));

        if show_values {
            *sp_id += 1;
            let rpr = run_props_xml(
                Some(font_size * 0.8),
                false,
                false,
                false,
                Some(label_color),
                None,
            );
            let val_str = format!("{:.1}", data[i]);
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Val {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr wrap=\"square\" anchor=\"b\"/><a:lstStyle/>\
                 <a:p><a:pPr algn=\"ctr\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
                *sp_id, *sp_id, xfrm_xml(px - 0.4, py - 0.4, 0.8, 0.35, None),
                rpr, esc_xml(&val_str),
            ));
        }

        if let Some(lbl) = labels.get(i) {
            *sp_id += 1;
            let rpr = run_props_xml(
                Some(font_size),
                false,
                false,
                false,
                Some(label_color),
                None,
            );
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Lbl {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr wrap=\"square\" anchor=\"t\"/><a:lstStyle/>\
                 <a:p><a:pPr algn=\"ctr\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
                *sp_id, *sp_id, xfrm_xml(px - 0.6, baseline_y + 0.05, 1.2, 0.35, None),
                rpr, esc_xml(lbl),
            ));
        }
    }
    out
}

/// 饼图展开：每扇区一个 pie 形状（旋转到起始角 + adj 扇区角），从 12 点方向顺时针
fn expand_pie_chart(comp: &PieChart, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let data = &comp.data;
    let total: f64 = data.iter().sum();
    if data.is_empty() || total <= 0.0 {
        return String::new();
    }

    let labels = comp.labels.clone().unwrap_or_default();
    let colors = comp.colors.clone().unwrap_or_default();
    let show_values = comp.show_values.unwrap_or(true);
    let label_color = comp.label_color.as_deref().unwrap_or("000000");
    let font_size = comp.font_size.unwrap_or(12.0);

    let default_colors = [
        "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47", "FF0000", "7030A0",
    ];
    let size = pos.w.min(pos.h);
    let cx = pos.x + pos.w / 2.0;
    let cy = pos.y + pos.h / 2.0;
    let r = size / 2.0 * 0.82;

    let mut out = String::new();
    let mut start = 0.0f64;
    for (i, &val) in data.iter().enumerate() {
        let angle = val / total * 360.0;
        let c = colors
            .get(i)
            .map(|s| s.as_str())
            .unwrap_or(default_colors[i.min(7)]);
        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Sector {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"pie\"><a:avLst><a:gd name=\"adj\" fmla=\"val {}\"/></a:avLst></a:prstGeom>\
             {}</p:spPr></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(pos.x, pos.y, size, size, Some(start)),
            (angle * 60000.0).round() as i64,
            solid_fill_xml(c),
        ));

        // 标签（百分比，扇区外侧）
        if show_values {
            let mid = (start + angle / 2.0) * std::f64::consts::PI / 180.0;
            let lx = cx + (r + 0.2) * mid.sin();
            let ly = cy - (r + 0.2) * mid.cos();
            let pct = format!("{:.1}%", val / total * 100.0);
            *sp_id += 1;
            let rpr = run_props_xml(
                Some(font_size),
                false,
                false,
                false,
                Some(label_color),
                None,
            );
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Pct {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr wrap=\"square\" anchor=\"ctr\"/><a:lstStyle/>\
                 <a:p><a:pPr algn=\"ctr\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
                *sp_id, *sp_id, xfrm_xml(lx - 0.5, ly - 0.2, 1.0, 0.35, None),
                rpr, esc_xml(&pct),
            ));
        }

        // 图例（标签置于扇区右侧下方）
        if let Some(lbl) = labels.get(i) {
            *sp_id += 1;
            let rpr = run_props_xml(
                Some(font_size * 0.9),
                false,
                false,
                false,
                Some(label_color),
                None,
            );
            let ly = pos.y + size + 0.1 + i as f64 * 0.3;
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Legend {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr wrap=\"square\" anchor=\"l\"/><a:lstStyle/>\
                 <a:p><a:pPr algn=\"l\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
                *sp_id, *sp_id, xfrm_xml(pos.x, ly, pos.w, 0.3, None),
                rpr, esc_xml(lbl),
            ));
        }
        start += angle;
    }
    out
}

/// 环形图展开：浅色 track 环 + 每扇区一个 blockArc（旋转到起始角 + adj 半个扇区角）
fn expand_ring_chart(comp: &RingChart, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let data = &comp.data;
    let total: f64 = data.iter().sum();
    if data.is_empty() || total <= 0.0 {
        return String::new();
    }

    let labels = comp.labels.clone().unwrap_or_default();
    let colors = comp.colors.clone().unwrap_or_default();
    let show_values = comp.show_values.unwrap_or(true);
    let label_color = comp.label_color.as_deref().unwrap_or("000000");
    let font_size = comp.font_size.unwrap_or(12.0);
    let _thickness = comp.thickness.unwrap_or(0.35); // 环宽为 blockArc 形状固有比例

    let default_colors = [
        "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47", "FF0000", "7030A0",
    ];
    let size = pos.w.min(pos.h);
    let cx = pos.x + pos.w / 2.0;
    let cy = pos.y + pos.h / 2.0;
    let r = size / 2.0 * 0.7;

    let mut out = String::new();

    // track：两个 180° blockArc 拼成整环
    for (i, rot) in [0.0f64, 180.0].iter().enumerate() {
        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"RingTrack {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"blockArc\"><a:avLst><a:gd name=\"adj\" fmla=\"val 5400000\"/></a:avLst></a:prstGeom>\
             {}</p:spPr></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(pos.x, pos.y, size, size, Some(*rot)),
            solid_fill_xml("E0E0E0"),
        ));
        let _ = i;
    }

    // 扇区：blockArc，adj = 半个扇区角；超过 180° 拆两段
    let mut start = 0.0f64;
    for (i, &val) in data.iter().enumerate() {
        let angle = val / total * 360.0;
        let c = colors
            .get(i)
            .map(|s| s.as_str())
            .unwrap_or(default_colors[i.min(7)]);
        let mut pieces: Vec<(f64, f64)> = Vec::new();
        if angle > 180.0 {
            pieces.push((start, 180.0));
            pieces.push((start + 180.0, angle - 180.0));
        } else {
            pieces.push((start, angle));
        }
        for (piece_start, piece_angle) in pieces {
            *sp_id += 1;
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"RingSector {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"blockArc\"><a:avLst><a:gd name=\"adj\" fmla=\"val {}\"/></a:avLst></a:prstGeom>\
                 {}</p:spPr></p:sp>",
                *sp_id,
                *sp_id,
                xfrm_xml(pos.x, pos.y, size, size, Some(piece_start)),
                (piece_angle / 2.0 * 60000.0).round() as i64,
                solid_fill_xml(c),
            ));
        }

        if show_values {
            let mid = (start + angle / 2.0) * std::f64::consts::PI / 180.0;
            let lx = cx + (r + 0.2) * mid.sin();
            let ly = cy - (r + 0.2) * mid.cos();
            let pct = format!("{:.1}%", val / total * 100.0);
            *sp_id += 1;
            let rpr = run_props_xml(
                Some(font_size),
                false,
                false,
                false,
                Some(label_color),
                None,
            );
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Pct {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr wrap=\"square\" anchor=\"ctr\"/><a:lstStyle/>\
                 <a:p><a:pPr algn=\"ctr\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
                *sp_id, *sp_id, xfrm_xml(lx - 0.5, ly - 0.2, 1.0, 0.35, None),
                rpr, esc_xml(&pct),
            ));
        }

        if let Some(lbl) = labels.get(i) {
            *sp_id += 1;
            let rpr = run_props_xml(
                Some(font_size * 0.9),
                false,
                false,
                false,
                Some(label_color),
                None,
            );
            let ly = pos.y + size + 0.1 + i as f64 * 0.3;
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Legend {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr wrap=\"square\" anchor=\"l\"/><a:lstStyle/>\
                 <a:p><a:pPr algn=\"l\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
                *sp_id, *sp_id, xfrm_xml(pos.x, ly, pos.w, 0.3, None),
                rpr, esc_xml(lbl),
            ));
        }
        start += angle;
    }
    out
}

fn expand_kpi_card(comp: &KpiCard, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let value = comp.value.clone();
    let label = comp.label.as_deref().unwrap_or("");
    let delta = comp.delta.as_deref();
    let bg = comp.bg.as_deref().unwrap_or("2c3e50");
    let accent = comp.accent.as_deref().unwrap_or("3498db");
    let rounded = comp.rounded.unwrap_or(true);
    let text_color = comp.text_color.as_deref().unwrap_or("FFFFFF");

    let shape_type = if rounded { "roundRect" } else { "rect" };
    let mut out = String::new();

    *sp_id += 1;
    out.push_str(&format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"KpiBg {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
         <p:spPr>{}<a:prstGeom prst=\"{}\"><a:avLst/></a:prstGeom>{}</p:spPr></p:sp>",
        *sp_id,
        *sp_id,
        xfrm_xml(pos.x, pos.y, pos.w, pos.h, None),
        shape_type,
        solid_fill_xml(bg),
    ));

    *sp_id += 1;
    let accent_h = 0.06;
    out.push_str(&format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"KpiAccent {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
         <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>{}</p:spPr></p:sp>",
        *sp_id, *sp_id,
        xfrm_xml(pos.x, pos.y + pos.h - accent_h, pos.w, accent_h, None),
        solid_fill_xml(accent),
    ));

    *sp_id += 1;
    let rpr = run_props_xml(Some(36.0), true, false, false, Some(text_color), None);
    let vx = pos.x + pos.w * 0.1;
    let vy = pos.y + pos.h * 0.25;
    let vw = pos.w * 0.8;
    let vh = pos.h * 0.4;
    out.push_str(&format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"KpiVal {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
         <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
         <p:txBody><a:bodyPr wrap=\"square\" anchor=\"ctr\"/><a:lstStyle/>\
         <a:p><a:pPr algn=\"l\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
        *sp_id, *sp_id, xfrm_xml(vx, vy, vw, vh, None), rpr, esc_xml(&value),
    ));

    if !label.is_empty() {
        *sp_id += 1;
        let rpr = run_props_xml(Some(14.0), false, false, false, Some(text_color), None);
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"KpiLbl {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
             <p:txBody><a:bodyPr wrap=\"square\" anchor=\"t\"/><a:lstStyle/>\
             <a:p><a:pPr algn=\"l\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
            *sp_id, *sp_id,
            xfrm_xml(vx, pos.y + pos.h * 0.65, vw, 0.4, None),
            rpr, esc_xml(label),
        ));
    }

    if let Some(d) = delta {
        *sp_id += 1;
        let is_pos = d.starts_with('+');
        let delta_color = if is_pos { "2ecc71" } else { "e74c3c" };
        let rpr = run_props_xml(Some(16.0), false, false, false, Some(delta_color), None);
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"KpiDelta {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
             <p:txBody><a:bodyPr wrap=\"square\" anchor=\"t\"/><a:lstStyle/>\
             <a:p><a:pPr algn=\"l\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
            *sp_id, *sp_id,
            xfrm_xml(pos.x + pos.w * 0.6, pos.y + pos.h * 0.25, vw * 0.35, vh, None),
            rpr, esc_xml(d),
        ));
    }
    out
}

fn expand_rating_stars(comp: &RatingStars, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let rating = comp.rating.clamp(0.0, 5.0);
    let max_stars = comp.max.unwrap_or(5) as usize;
    let color = comp.color.as_deref().unwrap_or("F1C40F");
    let empty_color = comp.empty_color.as_deref().unwrap_or("CCCCCC");
    let star_size = pos.h.min(pos.w / max_stars.max(1) as f64) * 0.85;
    let gap = star_size * 0.15;

    let mut out = String::new();
    for i in 0..max_stars {
        let sx = pos.x + i as f64 * (star_size + gap);
        let sy = pos.y + (pos.h - star_size) / 2.0;
        let fill_color = if (i as f64) < rating {
            color
        } else {
            empty_color
        };
        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Star {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"star5\"><a:avLst/></a:prstGeom>{}</p:spPr></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(sx, sy, star_size, star_size, None),
            solid_fill_xml(fill_color),
        ));
    }
    out
}

fn expand_timeline(comp: &Timeline, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let items = &comp.items;
    if items.is_empty() {
        return String::new();
    }

    let line_color = comp.line_color.as_deref().unwrap_or("3498db");
    let dot_color = comp.dot_color.as_deref().unwrap_or("3498db");
    let label_color = comp.label_color.as_deref().unwrap_or("000000");

    let n = items.len() as f64;
    let line_y = pos.y + pos.h * 0.5;
    let dot_radius = 0.15;
    let item_width = pos.w / n;

    let mut out = String::new();

    *sp_id += 1;
    out.push_str(&format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"TimelineLine {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
         <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>\
         <a:ln w=\"12700\">{}</a:ln></p:spPr></p:sp>",
        *sp_id, *sp_id,
        xfrm_xml(pos.x, line_y - 0.02, pos.w, 0.04, None),
        solid_fill_xml(line_color),
    ));

    for (i, item) in items.iter().enumerate() {
        let cx = pos.x + (i as f64 + 0.5) * item_width;

        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Dot {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"ellipse\"><a:avLst/></a:prstGeom>{}</p:spPr></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(
                cx - dot_radius,
                line_y - dot_radius,
                dot_radius * 2.0,
                dot_radius * 2.0,
                None
            ),
            solid_fill_xml(dot_color),
        ));

        let rpr = run_props_xml(Some(11.0), false, false, false, Some(label_color), None);
        let label = item.as_str().unwrap_or("");
        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Label {}\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
             <p:txBody><a:bodyPr wrap=\"square\" anchor=\"ctr\"/><a:lstStyle/>\
             <a:p><a:pPr algn=\"ctr\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
            *sp_id, *sp_id,
            xfrm_xml(cx - item_width * 0.4, line_y - pos.h * 0.45, item_width * 0.8, pos.h * 0.4, None),
            rpr, esc_xml(label),
        ));
    }
    out
}

fn expand_process_flow(comp: &ProcessFlow, sp_id: &mut u32) -> String {
    let pos = &comp.position;
    let steps = &comp.steps;
    if steps.is_empty() {
        return String::new();
    }

    let default_colors = [
        "3498db", "2ecc71", "e74c3c", "f39c12", "9b59b6", "1abc9c", "e67e22", "34495e",
    ];
    let colors = comp.colors.clone().unwrap_or_default();
    let text_color = comp.text_color.as_deref().unwrap_or("FFFFFF");
    let font_size = comp.font_size.unwrap_or(14.0);

    let n = steps.len() as f64;
    let arrow_w = 0.3;
    let step_w = (pos.w - (n - 1.0) * arrow_w) / n;
    let step_h = pos.h * 0.6;
    let step_y = pos.y + (pos.h - step_h) / 2.0;
    let rpr = run_props_xml(Some(font_size), true, false, false, Some(text_color), None);

    let mut out = String::new();
    for (i, step) in steps.iter().enumerate() {
        let sx = pos.x + i as f64 * (step_w + arrow_w);
        let c = colors
            .get(i)
            .map(|s| s.as_str())
            .unwrap_or(default_colors[i.min(7)]);

        *sp_id += 1;
        out.push_str(&format!(
            "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Step {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
             <p:spPr>{}<a:prstGeom prst=\"roundRect\"><a:avLst/></a:prstGeom>{}</p:spPr>\
             <p:txBody><a:bodyPr wrap=\"square\" anchor=\"ctr\"/><a:lstStyle/>\
             <a:p><a:pPr algn=\"ctr\"/><a:r>{}<a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
            *sp_id,
            *sp_id,
            xfrm_xml(sx, step_y, step_w, step_h, None),
            solid_fill_xml(c),
            rpr,
            esc_xml(step),
        ));

        if i < steps.len() - 1 {
            let ax = sx + step_w;
            let ay = step_y + step_h / 2.0;
            *sp_id += 1;
            out.push_str(&format!(
                "<p:sp><p:nvSpPr><p:cNvPr id=\"{}\" name=\"Arrow {}\"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>\
                 <p:spPr>{}<a:prstGeom prst=\"rightArrow\"><a:avLst/></a:prstGeom>{}</p:spPr></p:sp>",
                *sp_id, *sp_id,
                xfrm_xml(ax, ay - 0.15, arrow_w, 0.3, None),
                solid_fill_xml("95a5a6"),
            ));
        }
    }
    out
}
