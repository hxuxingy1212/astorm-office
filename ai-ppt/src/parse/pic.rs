//! 图片元素解析
//!
//! 从 p:pic 节点解析图片元素（位置、裁剪、超链接、边框）。

use crate::model::elements::*;
use quick_xml::events::Event;
use quick_xml::Reader;

pub(super) fn parse_pic(
    reader: &mut Reader<&[u8]>,
    r_id_map: &std::collections::HashMap<String, String>,
) -> Option<Element> {
    let mut depth = 1u32;
    let mut position = Position {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };
    let mut src = String::new();
    let mut name = None;
    let mut id = None;
    let mut rotation = None;
    let mut crop: Option<Crop> = None;
    let mut hyperlink: Option<String> = None;
    let mut line_color = None;
    let mut line_width = 1.0;
    let mut embed_r_id = String::new();
    let mut tooltip: Option<String> = None;
    let mut brightness: Option<f64> = None;
    let mut contrast: Option<f64> = None;
    let mut fill_mode: Option<String> = None;
    let mut media_kind: Option<String> = None;
    let mut media_link = String::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let n = super::tag(e).to_string();
                match n.as_str() {
                    "p:cNvPr" => {
                        name = super::parse_name_attr(e);
                        id = super::parse_id_attr(e);
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"descr" {
                                tooltip = Some(String::from_utf8_lossy(&a.value).to_string());
                            }
                        }
                    }
                    "a:hlinkClick" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "r:id" || k == "id" {
                                hyperlink = r_id_map.get(&v).cloned().or_else(|| Some(v.clone()));
                            }
                            if k == "tooltip" {
                                tooltip = Some(v.clone());
                            }
                        }
                    }
                    "a:blip" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "r:embed" {
                                embed_r_id = v;
                            }
                        }
                    }
                    "a:lum" => {
                        for a in e.attributes().flatten() {
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            match a.key.as_ref() {
                                b"bright" => {
                                    if let Ok(x) = v.parse::<i64>() {
                                        brightness = Some(x as f64 / 100000.0);
                                    }
                                }
                                b"contrast" => {
                                    if let Ok(x) = v.parse::<i64>() {
                                        contrast = Some(x as f64 / 100000.0);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "a:tile" => {
                        fill_mode = Some("tile".to_string());
                    }
                    "a:stretch" if fill_mode.is_none() => {
                        fill_mode = Some("stretch".to_string());
                    }
                    "a:videoFile" | "a:audioFile" => {
                        media_kind =
                            Some(if n == "a:videoFile" { "video" } else { "audio" }.to_string());
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"r:link" || a.key.as_ref() == b"r:embed" {
                                media_link = String::from_utf8_lossy(&a.value).to_string();
                            }
                        }
                    }
                    "a:srcRect" => {
                        let mut l = 0;
                        let mut t = 0;
                        let mut r = 0;
                        let mut b = 0;
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "l" {
                                if let Ok(val) = v.parse() {
                                    l = val;
                                }
                            }
                            if k == "t" {
                                if let Ok(val) = v.parse() {
                                    t = val;
                                }
                            }
                            if k == "r" {
                                if let Ok(val) = v.parse() {
                                    r = val;
                                }
                            }
                            if k == "b" {
                                if let Ok(val) = v.parse() {
                                    b = val;
                                }
                            }
                        }
                        crop = Some(Crop {
                            left: l,
                            top: t,
                            right: r,
                            bottom: b,
                        });
                    }
                    "a:xfrm" => {
                        for a in e.attributes().flatten() {
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
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:ext" => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:ln" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "w" {
                                if let Ok(w) = v.parse::<i64>() {
                                    line_width = w as f64 / 12700.0;
                                }
                            }
                        }
                        let mut inner_buf = Vec::new();
                        loop {
                            match reader.read_event_into(&mut inner_buf) {
                                Ok(Event::Empty(ref child)) => {
                                    let cn = super::tag(child).to_string();
                                    if cn == "a:srgbClr" || cn == "a:schemeClr" {
                                        for a in child.attributes().flatten() {
                                            if String::from_utf8_lossy(a.key.as_ref()) == "val" {
                                                line_color = Some(
                                                    String::from_utf8_lossy(&a.value).to_string(),
                                                );
                                            }
                                        }
                                    }
                                }
                                Ok(Event::Start(ref child)) => {
                                    let cn = super::tag(child).to_string();
                                    if cn == "a:solidFill" {
                                        let mut fill_buf = Vec::new();
                                        if let Ok(Event::Empty(ref clr)) =
                                            reader.read_event_into(&mut fill_buf)
                                        {
                                            let clr_n = super::tag(clr).to_string();
                                            if clr_n == "a:srgbClr" || clr_n == "a:schemeClr" {
                                                for a in clr.attributes().flatten() {
                                                    if String::from_utf8_lossy(a.key.as_ref())
                                                        == "val"
                                                    {
                                                        line_color = Some(
                                                            String::from_utf8_lossy(&a.value)
                                                                .to_string(),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Ok(Event::End(ref end)) if super::tag(end) == "a:ln" => {
                                    break;
                                }
                                Ok(Event::Eof) => break,
                                _ => {}
                            }
                            inner_buf.clear();
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let n = super::tag(e).to_string();
                match n.as_str() {
                    "p:cNvPr" => {
                        name = super::parse_name_attr(e);
                        id = super::parse_id_attr(e);
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"descr" {
                                tooltip = Some(String::from_utf8_lossy(&a.value).to_string());
                            }
                        }
                    }
                    "a:hlinkClick" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "r:id" || k == "id" {
                                hyperlink = r_id_map.get(&v).cloned().or_else(|| Some(v.clone()));
                            }
                            if k == "tooltip" {
                                tooltip = Some(v.clone());
                            }
                        }
                    }
                    "a:off" => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:ext" => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:srcRect" => {
                        let mut l = 0;
                        let mut t = 0;
                        let mut r = 0;
                        let mut b = 0;
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "l" {
                                if let Ok(val) = v.parse() {
                                    l = val;
                                }
                            }
                            if k == "t" {
                                if let Ok(val) = v.parse() {
                                    t = val;
                                }
                            }
                            if k == "r" {
                                if let Ok(val) = v.parse() {
                                    r = val;
                                }
                            }
                            if k == "b" {
                                if let Ok(val) = v.parse() {
                                    b = val;
                                }
                            }
                        }
                        crop = Some(Crop {
                            left: l,
                            top: t,
                            right: r,
                            bottom: b,
                        });
                    }
                    "a:blip" => {
                        for a in e.attributes().flatten() {
                            let k = String::from_utf8_lossy(a.key.as_ref()).to_string();
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            if k == "r:embed" {
                                embed_r_id = v;
                            }
                        }
                    }
                    "a:lum" => {
                        for a in e.attributes().flatten() {
                            let v = String::from_utf8_lossy(&a.value).to_string();
                            match a.key.as_ref() {
                                b"bright" => {
                                    if let Ok(x) = v.parse::<i64>() {
                                        brightness = Some(x as f64 / 100000.0);
                                    }
                                }
                                b"contrast" => {
                                    if let Ok(x) = v.parse::<i64>() {
                                        contrast = Some(x as f64 / 100000.0);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "a:tile" => {
                        fill_mode = Some("tile".to_string());
                    }
                    "a:stretch" if fill_mode.is_none() => {
                        fill_mode = Some("stretch".to_string());
                    }
                    "a:videoFile" | "a:audioFile" => {
                        media_kind =
                            Some(if n == "a:videoFile" { "video" } else { "audio" }.to_string());
                        for a in e.attributes().flatten() {
                            if a.key.as_ref() == b"r:link" || a.key.as_ref() == b"r:embed" {
                                media_link = String::from_utf8_lossy(&a.value).to_string();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let n = super::tag(e).to_string();
                if n == "p:pic" {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    if let Some(target) = r_id_map.get(&embed_r_id) {
        src = target.clone();
    }

    Some(Element::Image(ImageElement {
        src,
        position,
        name,
        id,
        rotation,
        crop,
        hyperlink,
        line: line_color.map(|c| Line {
            color: c,
            width: line_width,
        }),
        brightness,
        contrast,
        fill_mode,
        tooltip,
        media: media_kind.map(|kind| MediaRef {
            kind,
            src: r_id_map.get(&media_link).cloned().unwrap_or(media_link),
        }),
        animations: None,
    }))
}
