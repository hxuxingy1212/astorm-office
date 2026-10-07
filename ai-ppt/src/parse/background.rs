//! 幻灯片背景解析
//!
//! 支持纯色、渐变与图片三种背景的还原。

use quick_xml::events::Event;
use quick_xml::Reader;

/// 从版式/母版 XML 中抽取背景（无关系表，仅纯色/渐变）
pub(crate) fn parse_bg_only(xml: &str) -> Option<serde_json::Value> {
    let empty = std::collections::HashMap::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut in_bg = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                if super::tag(e) == "p:bg" {
                    in_bg = true;
                } else if in_bg {
                    let start = e.to_owned();
                    return parse_background(&start, &mut reader, &empty);
                }
            }
            Ok(Event::End(ref e)) if super::tag(e) == "p:bg" => {
                break;
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

pub(super) fn parse_background(
    _e: &quick_xml::events::BytesStart,
    reader: &mut Reader<&[u8]>,
    r_id_map: &std::collections::HashMap<String, String>,
) -> Option<serde_json::Value> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e2)) | Ok(Event::Empty(ref e2)) => {
                let n = super::tag(e2).to_string();
                if n == "a:blipFill" {
                    let mut src = None;
                    loop {
                        match reader.read_event_into(&mut buf) {
                            Ok(Event::Empty(ref blip)) => {
                                let bn = super::tag(blip).to_string();
                                if bn == "a:blip" {
                                    for a in blip.attributes().flatten() {
                                        let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                                        let v = String::from_utf8_lossy(&a.value).to_string();
                                        if k == "r:embed" {
                                            src = r_id_map.get(&v).cloned();
                                        }
                                    }
                                }
                            }
                            Ok(Event::Start(_)) => {}
                            Ok(Event::End(ref end)) => {
                                let en = super::tag(end).to_string();
                                if en == "a:blipFill" {
                                    break;
                                }
                            }
                            Ok(Event::Eof) => break,
                            _ => {}
                        }
                        buf.clear();
                    }
                    if let Some(s) = src {
                        return Some(serde_json::json!({ "type": "image", "src": s }));
                    }
                    return None;
                }
                if n == "a:srgbClr" || n == "a:schemeClr" {
                    let base = e2
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == b"val")
                        .map(|a| String::from_utf8_lossy(&a.value).to_string())
                        .unwrap_or_default();
                    // 读取颜色修饰符子元素（若为 Start），随后立即返回
                    let local = n.trim_start_matches("a:").to_string();
                    let mut mods: Vec<(String, String)> = Vec::new();
                    let mut mbuf = Vec::new();
                    loop {
                        mbuf.clear();
                        match reader.read_event_into(&mut mbuf) {
                            Ok(Event::Empty(ref m)) | Ok(Event::Start(ref m)) => {
                                let mn = super::tag(m).to_string();
                                if let Some(short) = mn.strip_prefix("a:") {
                                    if matches!(
                                        short,
                                        "lumMod"
                                            | "lumOff"
                                            | "tint"
                                            | "shade"
                                            | "alphaMod"
                                            | "alphaOff"
                                            | "alpha"
                                    ) {
                                        if let Some(a) = m
                                            .attributes()
                                            .flatten()
                                            .find(|a| a.key.as_ref() == b"val")
                                        {
                                            mods.push((
                                                short.to_string(),
                                                String::from_utf8_lossy(&a.value).to_string(),
                                            ));
                                        }
                                        continue;
                                    }
                                }
                                break;
                            }
                            Ok(Event::End(ref m)) => {
                                if super::tag(m) == format!("a:{}", local) {
                                    break;
                                }
                                break;
                            }
                            Ok(Event::Eof) => break,
                            _ => break,
                        }
                    }
                    let val = if mods.is_empty() {
                        base
                    } else {
                        format!(
                            "{}|{}",
                            base,
                            mods.iter()
                                .map(|(k, v)| format!("{}={}", k, v))
                                .collect::<Vec<_>>()
                                .join(";")
                        )
                    };
                    return Some(serde_json::Value::String(val));
                }
                if n == "a:gsLst" {
                    let mut stops = Vec::new();
                    loop {
                        match reader.read_event_into(&mut buf) {
                            Ok(Event::Start(ref gs)) | Ok(Event::Empty(ref gs)) => {
                                let gn = super::tag(gs).to_string();
                                if gn == "a:gs" {
                                    let mut pos = 0.0;
                                    for a in gs.attributes().flatten() {
                                        if String::from_utf8_lossy(a.key.as_ref()) == "pos" {
                                            if let Ok(p) =
                                                String::from_utf8_lossy(&a.value).parse::<i64>()
                                            {
                                                pos = p as f64 / 1000.0;
                                            }
                                        }
                                    }
                                    let mut color = String::new();
                                    if let Ok(Event::Empty(ref clr)) =
                                        reader.read_event_into(&mut buf)
                                    {
                                        let cn = super::tag(clr).to_string();
                                        if cn == "a:srgbClr" || cn == "a:schemeClr" {
                                            for a in clr.attributes().flatten() {
                                                if String::from_utf8_lossy(a.key.as_ref()) == "val"
                                                {
                                                    color = String::from_utf8_lossy(&a.value)
                                                        .to_string();
                                                }
                                            }
                                        }
                                    }
                                    stops.push(
                                        serde_json::json!({ "color": color, "position": pos }),
                                    );
                                }
                            }
                            Ok(Event::End(ref gs)) if super::tag(gs) == "a:gsLst" => {
                                break;
                            }
                            _ => {}
                        }
                        buf.clear();
                    }
                    let mut angle = 0.0;
                    loop {
                        match reader.read_event_into(&mut buf) {
                            Ok(Event::Empty(ref lin)) if super::tag(lin) == "a:lin" => {
                                for a in lin.attributes().flatten() {
                                    if String::from_utf8_lossy(a.key.as_ref()) == "ang" {
                                        if let Ok(ang) =
                                            String::from_utf8_lossy(&a.value).parse::<i64>()
                                        {
                                            angle = ang as f64 / 60000.0;
                                        }
                                    }
                                }
                            }
                            Ok(Event::End(ref e2)) => {
                                let nn = super::tag(e2).to_string();
                                if nn == "p:bgPr" || nn == "p:bg" {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        buf.clear();
                    }
                    return Some(serde_json::json!({
                        "type": "gradient",
                        "stops": stops,
                        "angle": angle,
                    }));
                }
                if n == "p:bgPr" || n == "p:bg" {
                    return None;
                }
            }
            Ok(Event::End(ref e2)) => {
                let nn = super::tag(e2).to_string();
                if nn == "p:bgPr" || nn == "p:bg" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}
