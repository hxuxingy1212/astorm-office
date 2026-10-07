//! Parse `xl/theme/theme1.xml` color scheme so theme-indexed fills/fonts can
//! be resolved to concrete hex colors on parse.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// Theme colors indexed the way OOXML `theme="N"` references them:
/// 0=lt1, 1=dk1, 2=lt2, 3=dk2, 4..9=accent1..6, 10=hlink, 11=folHlink.
#[derive(Default, Clone)]
pub struct ThemeColors {
    pub colors: Vec<String>,
}

impl ThemeColors {
    pub fn get(&self, idx: usize) -> Option<String> {
        self.colors.get(idx).cloned()
    }
}

const SCHEME_NAMES: [&str; 12] = [
    "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6",
    "hlink", "folHlink",
];

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes().filter_map(|a| a.ok()).find_map(|a| {
        if a.key.local_name().as_ref() == key.as_bytes() {
            a.unescape_value().map(|v| v.into_owned()).ok()
        } else {
            None
        }
    })
}

pub fn parse_theme(xml: &str) -> ThemeColors {
    let mut reader = Reader::from_str(xml);
    let mut order: Vec<(String, String)> = Vec::new();
    let mut current: Option<String> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let n = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                if SCHEME_NAMES.contains(&n.as_str()) {
                    current = Some(n);
                }
            }
            Ok(Event::Empty(e)) => {
                let n = e.local_name().as_ref().to_vec();
                let v = match n.as_slice() {
                    b"srgbClr" => attr(&e, "val"),
                    b"sysClr" => attr(&e, "lastClr").or_else(|| attr(&e, "val")),
                    _ => None,
                };
                if let (Some(name), Some(v)) = (current.clone(), v) {
                    if !order.iter().any(|(k, _)| k == &name) {
                        order.push((name, v));
                    }
                }
            }
            Ok(Event::End(_)) => current = None,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let resolve = |k: &str| order.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let lookup = |k: &str| resolve(k).unwrap_or_else(|| "000000".into());
    // order for theme="N"
    let colors = vec![
        lookup("lt1"),
        lookup("dk1"),
        lookup("lt2"),
        lookup("dk2"),
        lookup("accent1"),
        lookup("accent2"),
        lookup("accent3"),
        lookup("accent4"),
        lookup("accent5"),
        lookup("accent6"),
        lookup("hlink"),
        lookup("folHlink"),
    ];
    ThemeColors { colors }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clr_scheme() {
        let xml = r#"<?xml version="1.0"?><a:theme xmlns:a="x"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2><a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1></a:clrScheme></a:themeElements></a:theme>"#;
        let t = parse_theme(xml);
        assert_eq!(t.get(0), Some("FFFFFF".to_string())); // lt1
        assert_eq!(t.get(1), Some("000000".to_string())); // dk1
        assert_eq!(t.get(3), Some("44546A".to_string())); // dk2
        assert_eq!(t.get(4), Some("4472C4".to_string())); // accent1
    }
}
