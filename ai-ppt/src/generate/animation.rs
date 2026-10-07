//! 动画 XML 生成模块
//!
//! 生成 OOXML 标准的动画时序 XML（p:timing）。
//! 支持进入(entr)、退出(exit)、强调(emph)和路径(path)四大类动画。

use crate::model::elements::Animation;
use crate::utils::constants::*;

/// 动画预设信息，用于映射动画类型到 OOXML 标准预设
struct AnimationPreset {
    /// 预设 ID（OOXML 标准定义）
    preset_id: u32,
    /// 预设分类：entr(进入), exit(退出), emph(强调), path(路径)
    preset_class: &'static str,
    /// 滤镜名称（仅某些动画需要）
    filter: Option<&'static str>,
}

/// 根据动画类型查找对应的预设信息（查共享预设表）
fn find_preset(anim_type: &str) -> Option<AnimationPreset> {
    animation_preset(anim_type).map(|(preset_id, preset_class, filter)| AnimationPreset {
        preset_id,
        preset_class,
        filter,
    })
}

fn get_subtype(anim: &Animation) -> String {
    match anim.r#type.as_str() {
        "flyIn" | "flyOut" | "floatIn" | "floatOut" | "wipeIn" | "wipeOut" => {
            let dir = anim.direction.as_deref().unwrap_or("bottom");
            fly_direction_value(dir).to_string()
        }
        "splitIn" => match anim.direction.as_deref().unwrap_or("horizontal") {
            "vertical" => "1".to_string(),
            _ => "0".to_string(),
        },
        _ => "0".to_string(),
    }
}

fn get_node_type(trigger: &str) -> &'static str {
    match trigger {
        "afterPrevious" => "afterEffect",
        "withPrevious" => "withEffect",
        _ => "clickEffect",
    }
}

/// 生成动画时序 XML（p:timing）的顶层结构
pub fn generate_timing(elements: &[(u32, &Vec<Animation>)], slide_size: (f64, f64)) -> String {
    if elements.is_empty() {
        return String::new();
    }

    let mut all_anims: Vec<(u32, &Animation)> = Vec::new();
    for (sp_id, anims) in elements {
        for a in *anims {
            all_anims.push((*sp_id, a));
        }
    }
    if all_anims.is_empty() {
        return String::new();
    }

    all_anims.sort_by_key(|a| a.1.order.unwrap_or(0));

    let mut node_id: u32 = 1;
    let mut get_id = || {
        node_id += 1;
        node_id
    };

    // 按触发方式分组：onClick 组之间相互独立
    let mut click_groups: Vec<Vec<(u32, &Animation)>> = Vec::new();
    let mut current_group: Vec<(u32, &Animation)> = Vec::new();

    for anim in &all_anims {
        let trigger = anim.1.trigger.as_deref().unwrap_or("onClick");
        if trigger == "onClick" || current_group.is_empty() {
            if !current_group.is_empty() {
                click_groups.push(std::mem::take(&mut current_group));
            }
            current_group.push(*anim);
        } else {
            current_group.push(*anim);
        }
    }
    if !current_group.is_empty() {
        click_groups.push(current_group);
    }

    // 生成每个点击组的 XML
    let mut click_groups_xml = String::new();
    for group in &click_groups {
        let mut effects_xml = String::new();
        for (sp_id, anim) in group {
            effects_xml.push_str(&generate_effect_node(*sp_id, anim, &mut get_id, slide_size));
        }
        let gid = get_id();
        let iid = get_id();
        click_groups_xml.push_str(&format!(
            "<p:par>\n<p:cTn id=\"{}\" fill=\"hold\">\n<p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\n\
             <p:childTnLst>\n<p:par>\n<p:cTn id=\"{}\" fill=\"hold\">\n<p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\n\
             <p:childTnLst>{}</p:childTnLst>\n</p:cTn>\n</p:par>\n</p:childTnLst>\n</p:cTn>\n</p:par>\n",
            gid, iid, effects_xml
        ));
    }

    let seq_id = get_id();
    format!(
        "<p:timing>\n<p:tnLst>\n<p:par>\n<p:cTn id=\"1\" dur=\"indefinite\" restart=\"never\" nodeType=\"tmRoot\">\n\
         <p:childTnLst>\n<p:seq concurrent=\"1\" nextAc=\"seek\">\n\
         <p:cTn id=\"{}\" dur=\"indefinite\" nodeType=\"mainSeq\">\n<p:childTnLst>{}</p:childTnLst>\n</p:cTn>\n\
         <p:prevCondLst><p:cond evt=\"onPrev\" delay=\"0\"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>\n\
         <p:nextCondLst><p:cond evt=\"onNext\" delay=\"0\"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst>\n\
         </p:seq>\n</p:childTnLst>\n</p:cTn>\n</p:par>\n</p:tnLst>\n</p:timing>",
        seq_id, click_groups_xml
    )
}

/// 生成单个动画效果节点
fn generate_effect_node(
    sp_id: u32,
    anim: &Animation,
    get_id: &mut dyn FnMut() -> u32,
    slide_size: (f64, f64),
) -> String {
    let preset = find_preset(&anim.r#type);
    if preset.is_none() {
        return String::new();
    }
    let preset = preset.unwrap();

    let dur = anim.duration.unwrap_or(500);
    let delay = anim.delay.unwrap_or(0);
    let subtype = get_subtype(anim);
    let node_type = get_node_type(anim.trigger.as_deref().unwrap_or("onClick"));
    let effect_id = get_id();

    let behavior_xml = generate_behavior(sp_id, anim, &preset, dur, get_id, slide_size);

    let sub = if subtype != "0" {
        format!(" presetSubtype=\"{}\"", subtype)
    } else {
        String::new()
    };

    let mut extra = String::new();
    if let Some(r) = &anim.repeat {
        extra.push_str(&format!(" repeatCount=\"{}\"", r));
    }
    if let Some(r) = &anim.restart {
        extra.push_str(&format!(" restart=\"{}\"", r));
    }
    if anim.auto_reverse == Some(true) {
        extra.push_str(" autoRev=\"1\"");
    }

    format!(
        "<p:par>\n<p:cTn id=\"{}\" presetID=\"{}\" presetClass=\"{}\"{} fill=\"hold\" grpId=\"0\" nodeType=\"{}\"{}>\n\
         <p:stCondLst><p:cond delay=\"{}\"/></p:stCondLst>\n\
         <p:childTnLst>{}</p:childTnLst>\n</p:cTn>\n</p:par>\n",
        effect_id, preset.preset_id, preset.preset_class, sub, node_type, extra, delay, behavior_xml
    )
}

/// 根据动画类型生成具体的动画行为 XML
fn generate_behavior(
    sp_id: u32,
    anim: &Animation,
    preset: &AnimationPreset,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
    slide_size: (f64, f64),
) -> String {
    match anim.r#type.as_str() {
        "appear" => gen_set_visibility(sp_id, dur, get_id),
        "fadeIn" | "fadeOut" => {
            let transition = if preset.preset_class == "entr" {
                "in"
            } else {
                "out"
            };
            let mut xml = gen_anim_effect(sp_id, dur, "fade", transition, get_id);
            if preset.preset_class == "entr" {
                xml.push_str(&gen_set_visibility(sp_id, 0, get_id));
            }
            xml
        }
        "flyIn" | "flyOut" => gen_fly_animation(sp_id, anim, preset, dur, get_id),
        "floatIn" | "floatOut" => gen_float_animation(sp_id, anim, preset, dur, get_id),
        "swivel" => {
            let mut xml = gen_set_visibility(sp_id, 0, get_id);
            xml.push_str(&gen_anim_effect(sp_id, dur, "fade", "in", get_id));
            xml.push_str(&gen_scale_keyframes(
                sp_id,
                dur,
                get_id,
                &[(0, 0.0, 1.0), (100000, 1.0, 1.0)],
            ));
            xml
        }
        "wipeIn" | "wipeOut" => {
            let dir = anim.direction.as_deref().unwrap_or("down");
            let filter = format!("wipe({})", dir);
            let transition = if preset.preset_class == "entr" {
                "in"
            } else {
                "out"
            };
            let mut xml = gen_anim_effect(sp_id, dur, &filter, transition, get_id);
            if preset.preset_class == "entr" {
                xml.push_str(&gen_set_visibility(sp_id, 0, get_id));
            }
            xml
        }
        "zoomIn" | "zoomOut" => gen_scale_animation(sp_id, anim, preset, dur, get_id),
        "bounceIn" => {
            let mut xml = gen_set_visibility(sp_id, 0, get_id);
            xml.push_str(&gen_scale_keyframes(
                sp_id,
                dur,
                get_id,
                &[
                    (0, 0.0, 0.0),
                    (60000, 1.1, 1.1),
                    (80000, 0.95, 0.95),
                    (100000, 1.0, 1.0),
                ],
            ));
            xml
        }
        "pulse" | "growShrink" => gen_scale_animation(sp_id, anim, preset, dur, get_id),
        "spin" => gen_rotate_animation(sp_id, anim, dur, get_id),
        "teeter" => gen_teeter_animation(sp_id, anim, dur, get_id),
        "colorChange" => gen_color_animation(sp_id, anim, dur, get_id),
        "transparency" => gen_transparency_animation(sp_id, anim, dur, get_id),
        "blink" => gen_blink_animation(sp_id, anim, dur, get_id),
        "motionPath" => gen_motion_path(sp_id, anim, dur, get_id, slide_size),
        _ => {
            if let Some(filter) = preset.filter {
                let transition = if preset.preset_class == "entr" {
                    "in"
                } else {
                    "out"
                };
                let mut xml = gen_anim_effect(sp_id, dur, filter, transition, get_id);
                if preset.preset_class == "entr" {
                    xml.push_str(&gen_set_visibility(sp_id, 0, get_id));
                }
                xml
            } else {
                gen_set_visibility(sp_id, dur, get_id)
            }
        }
    }
}

fn gen_set_visibility(sp_id: u32, dur: u64, get_id: &mut dyn FnMut() -> u32) -> String {
    format!(
        "<p:set>\n<p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\">\n\
         <p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\n</p:cTn>\n\
         <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
         <p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst>\n\
         </p:cBhvr>\n<p:to><p:strVal val=\"visible\"/></p:to>\n</p:set>",
        get_id(),
        dur,
        sp_id
    )
}

fn gen_anim_effect(
    sp_id: u32,
    dur: u64,
    filter: &str,
    transition: &str,
    get_id: &mut dyn FnMut() -> u32,
) -> String {
    format!(
        "<p:animEffect transition=\"{}\" filter=\"{}\">\n\
         <p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\"/>\n\
         <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n</p:cBhvr>\n</p:animEffect>",
        transition,
        filter,
        get_id(),
        dur,
        sp_id
    )
}

fn gen_fly_animation(
    sp_id: u32,
    anim: &Animation,
    preset: &AnimationPreset,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
) -> String {
    let dir = anim.direction.as_deref().unwrap_or("bottom");
    let is_entrance = preset.preset_class == "entr";
    let (from, to) = match dir {
        "left" => {
            if is_entrance {
                ("0-#ppt_w/2", "#ppt_x")
            } else {
                ("#ppt_x", "0-#ppt_w/2")
            }
        }
        "right" => {
            if is_entrance {
                ("1+#ppt_w/2", "#ppt_x")
            } else {
                ("#ppt_x", "1+#ppt_w/2")
            }
        }
        "top" => {
            if is_entrance {
                ("0-#ppt_h/2", "#ppt_y")
            } else {
                ("#ppt_y", "0-#ppt_h/2")
            }
        }
        _ => {
            if is_entrance {
                ("1+#ppt_h/2", "#ppt_y")
            } else {
                ("#ppt_y", "1+#ppt_h/2")
            }
        }
    };
    let attr = if dir == "left" || dir == "right" {
        "ppt_x"
    } else {
        "ppt_y"
    };

    let mut xml = String::new();
    if is_entrance {
        xml.push_str(&gen_set_visibility(sp_id, 0, get_id));
    }
    xml.push_str(&format!(
        "<p:animMotion origin=\"layout\" path=\"{}({})/{}({})\">\n\
         <p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\"/>\n\
         <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
         <p:attrNameLst><p:attrName>{}</p:attrName></p:attrNameLst>\n\
         </p:cBhvr>\n</p:animMotion>",
        attr,
        from,
        attr,
        to,
        get_id(),
        dur,
        sp_id,
        attr
    ));
    xml
}

fn gen_float_animation(
    sp_id: u32,
    anim: &Animation,
    preset: &AnimationPreset,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
) -> String {
    let dir = anim.direction.as_deref().unwrap_or("top");
    let is_entrance = preset.preset_class == "entr";
    let dist = 0.3;
    let (from_y, to_y) = if is_entrance {
        if dir == "top" {
            (
                format!("-#ppt_y-{}", inch_to_emu(dist)),
                "#ppt_y".to_string(),
            )
        } else {
            (
                format!("#ppt_y+{}", inch_to_emu(dist)),
                "#ppt_y".to_string(),
            )
        }
    } else {
        if dir == "top" {
            (
                "#ppt_y".to_string(),
                format!("-#ppt_y-{}", inch_to_emu(dist)),
            )
        } else {
            (
                "#ppt_y".to_string(),
                format!("#ppt_y+{}", inch_to_emu(dist)),
            )
        }
    };

    let mut xml = String::new();
    if is_entrance {
        xml.push_str(&gen_set_visibility(sp_id, 0, get_id));
    }
    xml.push_str(&format!(
        "<p:animMotion origin=\"layout\" path=\"ppt_y({})/ppt_y({})\">\n\
         <p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\"/>\n\
         <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
         <p:attrNameLst><p:attrName>ppt_y</p:attrName></p:attrNameLst>\n\
         </p:cBhvr>\n</p:animMotion>",
        from_y,
        to_y,
        get_id(),
        dur,
        sp_id
    ));
    xml
}

fn gen_scale_animation(
    sp_id: u32,
    anim: &Animation,
    preset: &AnimationPreset,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
) -> String {
    let is_entrance = preset.preset_class == "entr";
    let is_exit = preset.preset_class == "exit";
    let scale = anim.scale.map(|s| s as f64 / 100.0).unwrap_or(0.0);

    let keyframes = if is_entrance {
        if scale > 0.0 {
            vec![(0, 0.0, 0.0), (100000, scale, scale)]
        } else {
            vec![(0, 0.0, 0.0), (100000, 1.0, 1.0)]
        }
    } else if is_exit {
        if scale > 0.0 {
            vec![(0, scale, scale), (100000, 0.0, 0.0)]
        } else {
            vec![(0, 1.0, 1.0), (100000, 0.0, 0.0)]
        }
    } else {
        if scale > 0.0 {
            vec![(0, 1.0, 1.0), (50000, scale, scale), (100000, 1.0, 1.0)]
        } else {
            vec![(0, 1.0, 1.0), (50000, 1.5, 1.5), (100000, 1.0, 1.0)]
        }
    };

    let mut xml = String::new();
    if is_entrance {
        xml.push_str(&gen_set_visibility(sp_id, 0, get_id));
    }
    xml.push_str(&gen_scale_keyframes(sp_id, dur, get_id, &keyframes));
    xml
}

fn gen_scale_keyframes(
    sp_id: u32,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
    keyframes: &[(u32, f64, f64)],
) -> String {
    let mut kf_xml = String::new();
    for (_, sx, sy) in keyframes {
        kf_xml.push_str(&format!(
            "<a:spPrVal><a:xfrm><a:scale sx=\"{}\" sy=\"{}\"/></a:xfrm></a:spPrVal>",
            (sx * 100000.0).round() as i64,
            (sy * 100000.0).round() as i64,
        ));
    }
    format!(
        "<p:animScale>\n<p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\">\n\
         <p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\n</p:cTn>\n\
         <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
         <p:attrNameLst><p:attrName>transform.scaleX</p:attrName><p:attrName>transform.scaleY</p:attrName></p:attrNameLst>\n\
         </p:cBhvr>\n<p:tLst>{}</p:tLst>\n</p:animScale>",
        get_id(), dur, sp_id, kf_xml
    )
}

fn gen_rotate_animation(
    sp_id: u32,
    anim: &Animation,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
) -> String {
    let degrees = anim.degrees.unwrap_or(360.0);
    let duration_total = dur as f64 * (degrees / 360.0).max(1.0);
    format!(
        "<p:animRot>\n<p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\">\n\
         <p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\n</p:cTn>\n\
         <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
         <p:attrNameLst><p:attrName>transform.rotation</p:attrName></p:attrNameLst>\n\
         </p:cBhvr>\n<p:to><p:fltVal val=\"{}\"/></p:to>\n</p:animRot>",
        get_id(),
        duration_total as u64,
        sp_id,
        degrees
    )
}

fn gen_teeter_animation(
    sp_id: u32,
    anim: &Animation,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
) -> String {
    let degrees = anim.degrees.unwrap_or(15.0);
    format!(
        "<p:animRot>\n<p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\">\n\
         <p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\n</p:cTn>\n\
         <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
         <p:attrNameLst><p:attrName>transform.rotation</p:attrName></p:attrNameLst>\n\
         </p:cBhvr>\n<p:tLst>\n\
         <p:tm val=\"0\"><p:fltVal val=\"0\"/></p:tm>\n\
         <p:tm val=\"50000\"><p:fltVal val=\"{}\"/></p:tm>\n\
         <p:tm val=\"100000\"><p:fltVal val=\"0\"/></p:tm>\n\
         </p:tLst>\n</p:animRot>",
        get_id(),
        dur,
        sp_id,
        degrees
    )
}

fn gen_color_animation(
    sp_id: u32,
    anim: &Animation,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
) -> String {
    let color = anim
        .color
        .as_deref()
        .unwrap_or("FF0000")
        .trim_start_matches('#');
    format!(
        "<p:animClr>\n<p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\">\n\
         <p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\n</p:cTn>\n\
         <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
         <p:attrNameLst><p:attrName>style.color</p:attrName></p:attrNameLst>\n\
         </p:cBhvr>\n<p:to><a:srgbClr val=\"{}\"/></p:to>\n</p:animClr>",
        get_id(),
        dur,
        sp_id,
        color
    )
}

fn gen_transparency_animation(
    sp_id: u32,
    anim: &Animation,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
) -> String {
    let _opacity = anim.opacity.unwrap_or(0.5);
    format!(
        "<p:animEffect transition=\"in\" filter=\"fade\">\n\
         <p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\">\n\
         <p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\n</p:cTn>\n\
         <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
         <p:attrNameLst><p:attrName>style.opacity</p:attrName></p:attrNameLst>\n\
         </p:cBhvr>\n</p:animEffect>",
        get_id(),
        dur,
        sp_id
    )
}

fn gen_blink_animation(
    sp_id: u32,
    anim: &Animation,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
) -> String {
    let times = anim.scale.unwrap_or(3).max(2);
    let half_dur = (dur / times as u64).max(100);
    let mut xml = String::new();
    for _ in 0..times {
        let h = get_id();
        xml.push_str(&format!(
            "<p:par><p:cTn id=\"{}\" dur=\"{}\" fill=\"freeze\"><p:stCondLst><p:cond delay=\"0\"/></p:stCondLst>\
             <p:childTnLst>{}</p:childTnLst></p:cTn></p:par>",
            h, half_dur * 2,
            gen_set_visibility(sp_id, half_dur, get_id),
        ));
    }
    xml
}

fn gen_motion_path(
    sp_id: u32,
    anim: &Animation,
    dur: u64,
    get_id: &mut dyn FnMut() -> u32,
    _slide_size: (f64, f64),
) -> String {
    let distance = anim.distance.unwrap_or(2.0);
    let angle_rad = anim.degrees.unwrap_or(180.0).to_radians();
    let dx = distance * angle_rad.cos();
    let dy = distance * angle_rad.sin();
    let points = anim.points.as_ref();

    if let Some(pts) = points {
        let mut path = String::new();
        for (i, pt) in pts.iter().enumerate() {
            let x = pt.get("x").copied().unwrap_or(0.0);
            let y = pt.get("y").copied().unwrap_or(0.0);
            let cmd = if i == 0 { "M" } else { "L" };
            path.push_str(&format!("{}{},{} ", cmd, inch_to_emu(x), inch_to_emu(y)));
        }
        format!(
            "<p:animMotion origin=\"layout\" path=\"{}\">\n\
             <p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\"/>\n\
             <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
             <p:attrNameLst><p:attrName>ppt_x</p:attrName><p:attrName>ppt_y</p:attrName></p:attrNameLst>\n\
             </p:cBhvr>\n</p:animMotion>",
            path.trim(), get_id(), dur, sp_id
        )
    } else {
        let path = format!("M0,0 L{},{}", inch_to_emu(dx), inch_to_emu(dy));
        format!(
            "<p:animMotion origin=\"layout\" path=\"{}\">\n\
             <p:cBhvr>\n<p:cTn id=\"{}\" dur=\"{}\" fill=\"hold\"/>\n\
             <p:tgtEl><p:spTgt spid=\"{}\"/></p:tgtEl>\n\
             <p:attrNameLst><p:attrName>ppt_x</p:attrName><p:attrName>ppt_y</p:attrName></p:attrNameLst>\n\
             </p:cBhvr>\n</p:animMotion>",
            path, get_id(), dur, sp_id
        )
    }
}
