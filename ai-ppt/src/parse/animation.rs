//! 动画时序 XML 解析模块
//!
//! 从幻灯片的 p:timing 块中解析出动画数据。
//! 支持解析进入、退出、强调和路径动画。

use crate::model::elements::Animation;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

pub fn parse_timing(xml: &str, _sp_id_map: &HashMap<u32, u32>) -> Vec<(u32, Vec<Animation>)> {
    if xml.is_empty() {
        return Vec::new();
    }

    let mut result: Vec<(u32, Vec<Animation>)> = Vec::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut anim_type = String::new();
    let mut duration: Option<u64> = None;
    let mut delay: Option<u64> = None;
    let mut repeat: Option<String> = None;
    let mut restart: Option<String> = None;
    let mut auto_reverse: Option<bool> = None;
    let mut trigger: Option<String> = None;
    let mut direction: Option<String> = None;
    let mut order: Option<u32> = None;
    let mut scale: Option<u32> = None;
    let mut degrees: Option<f64> = None;
    let mut color: Option<String> = None;
    let mut opacity: Option<f64> = None;
    let mut distance: Option<f64> = None;
    let mut points: Option<Vec<HashMap<String, f64>>> = None;
    let mut target_sp_id: Option<u32> = None;
    let mut in_sp_tgt = false;
    let mut child_tn_depth = 0u32;
    let mut depth = 0u32;
    let _anim_count = 0u32;
    let mut par_depth = 0u32;
    let mut seq_depth = 0u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = super::tag(e).to_string();
                match name.as_str() {
                    "p:seq" | "p:par" => {
                        if name == "p:par" {
                            par_depth += 1;
                        }
                        if name == "p:seq" {
                            seq_depth += 1;
                        }
                    }
                    "p:cTn" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "dur" {
                                if let Ok(d) = v.parse::<u64>() {
                                    if d > 0 && d < 10000000 {
                                        duration = Some(d);
                                    }
                                }
                            }
                            if k == "repeatCount" {
                                repeat = Some(v.clone());
                            }
                            if k == "restart" {
                                restart = Some(v.clone());
                            }
                            if k == "autoRev" {
                                auto_reverse = Some(v == "1" || v == "true");
                            }
                            if k == "presetID" {
                                let pid = v.parse::<u32>().unwrap_or(0);
                                if pid > 0 {
                                    anim_type = preset_id_to_type(pid, "");
                                }
                            }
                            if k == "presetClass" {
                                if anim_type.is_empty() {
                                    anim_type = preset_class_to_default_type(&v);
                                } else if v == "exit" && anim_type.ends_with("In") {
                                    anim_type =
                                        anim_type.trim_end_matches("In").to_string() + "Out";
                                } else if v == "exit"
                                    && !anim_type.ends_with("Out")
                                    && !anim_type.ends_with("exit")
                                {
                                }
                            }
                            if k == "presetSubtype" {
                                direction = Some(subtype_to_direction(&v));
                            }
                            if k == "nodeType" {
                                trigger = Some(match v.as_str() {
                                    "afterEffect" => "afterPrevious".to_string(),
                                    "withEffect" => "withPrevious".to_string(),
                                    _ => "onClick".to_string(),
                                });
                            }
                        }
                    }
                    "p:stCondLst" => {}
                    "p:cond" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "delay" {
                                if let Ok(d) = v.parse::<u64>() {
                                    delay = Some(d);
                                }
                            }
                        }
                    }
                    "p:childTnLst" => {
                        child_tn_depth += 1;
                        depth = 0;
                    }
                    "p:cBhvr" => {}
                    "p:tgtEl" => {}
                    "p:spTgt" => {
                        in_sp_tgt = true;
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "spid" {
                                target_sp_id = v.parse::<u32>().ok();
                            }
                        }
                    }
                    "p:animEffect" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "filter" && anim_type.is_empty() {
                                let filter = &v;
                                if filter.starts_with("wipe(") {
                                    let dir =
                                        filter.trim_start_matches("wipe(").trim_end_matches(')');
                                    anim_type = "wipeIn".to_string();
                                    direction = Some(dir.to_string());
                                } else if *filter == "fade" {
                                    if let Some(trans) = e.attributes().flatten().find(|a| {
                                        String::from_utf8_lossy(a.key.as_ref()) == "transition"
                                    }) {
                                        let trans_val =
                                            String::from_utf8_lossy(&trans.value).to_string();
                                        anim_type = if trans_val == "in" {
                                            "fadeIn".to_string()
                                        } else {
                                            "fadeOut".to_string()
                                        };
                                    } else {
                                        anim_type = "fadeIn".to_string();
                                    }
                                } else if *filter == "dissolve" {
                                    if let Some(trans) = e.attributes().flatten().find(|a| {
                                        String::from_utf8_lossy(a.key.as_ref()) == "transition"
                                    }) {
                                        let trans_val =
                                            String::from_utf8_lossy(&trans.value).to_string();
                                        anim_type = if trans_val == "in" {
                                            "dissolveIn".to_string()
                                        } else {
                                            "dissolveOut".to_string()
                                        };
                                    } else {
                                        anim_type = "dissolveIn".to_string();
                                    }
                                } else {
                                    anim_type = "appear".to_string();
                                }
                            }
                        }
                    }
                    "p:animMotion" => {
                        if anim_type.is_empty() {
                            anim_type = "motionPath".to_string();
                        }
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "path" {
                                if let Some(pt) = v.split_whitespace().last() {
                                    if let Some(coords) = pt.strip_prefix('L') {
                                        let parts: Vec<&str> = coords.split(',').collect();
                                        if parts.len() == 2 {
                                            if let (Ok(dx), Ok(dy)) =
                                                (parts[0].parse::<i64>(), parts[1].parse::<i64>())
                                            {
                                                distance = Some(
                                                    ((dx as f64).hypot(dy as f64) / 914400.0
                                                        * 1000.0)
                                                        .round()
                                                        / 1000.0,
                                                );
                                                let angle =
                                                    (dy as f64).atan2(dx as f64).to_degrees();
                                                degrees = Some(angle);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "p:animRot" if anim_type.is_empty() => {
                        anim_type = "spin".to_string();
                    }
                    "p:animScale" if anim_type.is_empty() => {
                        anim_type = "growShrink".to_string();
                    }
                    "p:animClr" if anim_type.is_empty() => {
                        anim_type = "colorChange".to_string();
                    }
                    "p:set" if anim_type.is_empty() => {
                        anim_type = "appear".to_string();
                    }
                    _ => {}
                }
                if child_tn_depth > 0 {
                    let n = name.as_str();
                    match n {
                        "p:par" | "p:seq" => depth += 1,
                        "p:animEffect" | "p:animMotion" | "p:animRot" | "p:animScale"
                        | "p:animClr" | "p:set" => depth += 1,
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = super::tag(e).to_string();
                match name.as_str() {
                    "p:animEffect" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "filter" && anim_type.is_empty() {
                                let filter = &v;
                                if filter.starts_with("wipe(") {
                                    let dir =
                                        filter.trim_start_matches("wipe(").trim_end_matches(')');
                                    anim_type = "wipeIn".to_string();
                                    direction = Some(dir.to_string());
                                } else if *filter == "fade" {
                                    if let Some(trans) = e.attributes().flatten().find(|a| {
                                        String::from_utf8_lossy(a.key.as_ref()) == "transition"
                                    }) {
                                        let trans_val =
                                            String::from_utf8_lossy(&trans.value).to_string();
                                        anim_type = if trans_val == "in" {
                                            "fadeIn".to_string()
                                        } else {
                                            "fadeOut".to_string()
                                        };
                                    } else {
                                        anim_type = "fadeIn".to_string();
                                    }
                                } else {
                                    anim_type = "appear".to_string();
                                }
                            }
                            if k == "transition" && anim_type.is_empty() && v == "out" {
                                anim_type = "fadeOut".to_string();
                            }
                        }
                    }
                    "p:animMotion" if anim_type.is_empty() => {
                        anim_type = "motionPath".to_string();
                    }
                    "p:animRot" if anim_type.is_empty() => {
                        anim_type = "spin".to_string();
                    }
                    "p:animScale" if anim_type.is_empty() => {
                        anim_type = "growShrink".to_string();
                    }
                    "p:animClr" if anim_type.is_empty() => {
                        anim_type = "colorChange".to_string();
                    }
                    "p:set" if anim_type.is_empty() => {
                        anim_type = "appear".to_string();
                    }
                    "p:cTn" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "dur" {
                                if let Ok(d) = v.parse::<u64>() {
                                    if d > 0 && d < 10000000 {
                                        duration = Some(d);
                                    }
                                }
                            }
                            if k == "repeatCount" {
                                repeat = Some(v.clone());
                            }
                            if k == "restart" {
                                restart = Some(v.clone());
                            }
                            if k == "autoRev" {
                                auto_reverse = Some(v == "1" || v == "true");
                            }
                        }
                    }
                    "p:spTgt" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "spid" {
                                target_sp_id = v.parse::<u32>().ok();
                            }
                        }
                        in_sp_tgt = false;
                    }
                    "p:to" => {}
                    "a:srgbClr" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "val" {
                                color = Some(v);
                            }
                        }
                    }
                    "p:fltVal" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "val" && (anim_type == "spin" || anim_type == "teeter") {
                                if let Ok(d) = v.parse::<f64>() {
                                    degrees = Some(d);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref t)) => {
                let text = String::from_utf8_lossy(t.as_ref()).to_string();
                if in_sp_tgt {
                    if let Ok(spid) = text.trim().parse::<u32>() {
                        target_sp_id = Some(spid);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = super::tag(e).to_string();
                match name.as_str() {
                    "p:cTn" => {}
                    "p:cBhvr" => {}
                    "p:tgtEl" => {}
                    "p:spTgt" => {
                        in_sp_tgt = false;
                    }
                    "p:childTnLst" => {
                        child_tn_depth = child_tn_depth.saturating_sub(1);
                        depth = 0;
                    }
                    "p:par" => {
                        par_depth = par_depth.saturating_sub(1);
                        if child_tn_depth > 0 {
                            if let Some(spid) = target_sp_id {
                                let anim = Animation {
                                    r#type: std::mem::take(&mut anim_type),
                                    duration,
                                    delay,
                                    trigger: trigger.clone(),
                                    direction: direction.clone(),
                                    order,
                                    scale,
                                    degrees,
                                    color: color.clone(),
                                    opacity,
                                    points: points.clone(),
                                    distance,
                                    repeat: repeat.clone(),
                                    restart: restart.clone(),
                                    auto_reverse,
                                };
                                if !anim.r#type.is_empty() {
                                    let idx = result.iter().position(|(id, _)| *id == spid);
                                    if let Some(i) = idx {
                                        result[i].1.push(anim);
                                    } else {
                                        result.push((spid, vec![anim]));
                                    }
                                }
                                anim_type.clear();
                                duration = None;
                                delay = None;
                                direction = None;
                                order = None;
                                scale = None;
                                degrees = None;
                                color = None;
                                opacity = None;
                                points = None;
                                distance = None;
                                repeat = None;
                                restart = None;
                                auto_reverse = None;
                                target_sp_id = None;
                            }
                        }
                    }
                    "p:seq" => {
                        seq_depth = seq_depth.saturating_sub(1);
                    }
                    "p:timing" => break,
                    _ => {}
                }
                if child_tn_depth > 0 {
                    let n = name.as_str();
                    match n {
                        "p:par" | "p:seq" => depth = depth.saturating_sub(1),
                        "p:animEffect" | "p:animMotion" | "p:animRot" | "p:animScale"
                        | "p:animClr" | "p:set" => depth = depth.saturating_sub(1),
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    result
}

fn preset_id_to_type(pid: u32, _class: &str) -> String {
    crate::utils::constants::animation_type_from_preset_id(pid)
        .unwrap_or("appear")
        .to_string()
}

fn preset_class_to_default_type(class: &str) -> String {
    match class {
        "entr" => "appear",
        "exit" => "fadeOut",
        "emph" => "pulse",
        _ => "appear",
    }
    .to_string()
}

fn subtype_to_direction(subtype: &str) -> String {
    subtype
        .parse::<u32>()
        .ok()
        .and_then(crate::utils::constants::fly_direction_from_value)
        .unwrap_or("bottom")
        .to_string()
}
