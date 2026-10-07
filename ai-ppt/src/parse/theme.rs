//! 主题 XML 解析模块
//!
//! 从 theme.xml 中解析颜色方案和字体方案。

use crate::model::Theme;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// 规范化主题颜色值：保留十六进制；系统色名映射为具体颜色
fn normalize_color(v: Option<String>) -> Option<String> {
    let v = v?;
    let hex = v.trim_start_matches('#');
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some(hex.to_uppercase());
    }
    match v.as_str() {
        "window" | "windowFrame" | "background" => Some("FFFFFF".to_string()),
        "windowText" | "captionText" | "btnText" => Some("000000".to_string()),
        "activeCaption" | "highlight" => Some("0078D4".to_string()),
        _ => None,
    }
}

/// 解析主题 XML 为 Theme 数据模型
pub fn parse(xml: &str) -> Theme {
    if xml.is_empty() {
        return Theme {
            colors: None,
            major_font: None,
            minor_font: None,
        };
    }

    let mut colors = HashMap::new();
    let mut major_font = None;
    let mut minor_font = None;
    let mut in_clr_scheme = false;
    let mut in_font_scheme = false;
    let mut in_major = false;
    let mut in_minor = false;
    let mut current_color_key = String::new();

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = super::tag(e).to_string();
                if name == "a:clrScheme" {
                    in_clr_scheme = true;
                } else if name == "a:fontScheme" {
                    in_font_scheme = true;
                } else if in_clr_scheme && name.starts_with("a:") {
                    current_color_key = name[2..].to_string();
                } else if in_font_scheme && name == "a:majorFont" {
                    in_major = true;
                } else if in_font_scheme && name == "a:minorFont" {
                    in_minor = true;
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = super::tag(e).to_string();
                if in_clr_scheme && name.starts_with("a:") && current_color_key.is_empty() {
                    current_color_key = name[2..].to_string();
                }
                if !current_color_key.is_empty() {
                    // sysClr 优先取 lastClr（具体十六进制），否则取 val
                    let mut val = None;
                    let mut last = None;
                    for a in e.attributes().flatten() {
                        let v = String::from_utf8_lossy(&a.value).to_string();
                        match a.key.as_ref() {
                            b"lastClr" => last = Some(v),
                            b"val" => val = Some(v),
                            _ => {}
                        }
                    }
                    if let Some(hex) = normalize_color(last.or(val)) {
                        colors.insert(current_color_key.clone(), hex);
                    }
                    current_color_key.clear();
                }
                if in_font_scheme && name == "a:latin" {
                    if let Some(attr) = e.attributes().flatten().next() {
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        if in_major {
                            major_font = Some(val.clone());
                        }
                        if in_minor {
                            minor_font = Some(val);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = super::tag(e).to_string();
                if name == "a:clrScheme" {
                    in_clr_scheme = false;
                    current_color_key.clear();
                } else if name == "a:fontScheme" {
                    in_font_scheme = false;
                } else if name == "a:majorFont" {
                    in_major = false;
                } else if name == "a:minorFont" {
                    in_minor = false;
                } else if in_clr_scheme {
                    current_color_key.clear();
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    Theme {
        colors: if colors.is_empty() {
            None
        } else {
            Some(colors)
        },
        major_font,
        minor_font,
    }
}
