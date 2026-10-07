use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::Result;

/// Parse `xl/sharedStrings.xml` into an indexed list of strings.
pub fn parse_shared_strings(xml: &str) -> Result<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_si = false;
    let mut in_t = false;
    let mut in_rph = false;
    loop {
        match reader.read_event()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"si" => {
                    in_si = true;
                    cur.clear();
                }
                b"rPh" => in_rph = true,
                b"t" if in_si && !in_rph => in_t = true,
                _ => {}
            },
            Event::Empty(e) if e.local_name().as_ref() == b"si" => {
                // self-closing `<si/>` is an empty shared string (must keep the slot)
                out.push(String::new());
            }
            Event::Text(e) if in_t => {
                cur.push_str(&e.unescape()?);
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"t" => in_t = false,
                b"rPh" => in_rph = false,
                b"si" => {
                    in_si = false;
                    out.push(std::mem::take(&mut cur));
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_closing_si_keeps_slot() {
        let xml = r#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="3" uniqueCount="3"><si><t>A</t></si><si/><si><t>B</t></si></sst>"#;
        assert_eq!(parse_shared_strings(xml).unwrap(), vec!["A", "", "B"]);
    }

    #[test]
    fn rich_text_runs_concatenated() {
        let xml = r#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><r><t>Hello </t></r><r><t>World</t></r></si></sst>"#;
        assert_eq!(parse_shared_strings(xml).unwrap(), vec!["Hello World"]);
    }

    #[test]
    fn phonetic_runs_ignored() {
        let xml = r#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><si><t>漢字</t><rPh sb="0" eb="2"><t>カンジ</t></rPh><phoneticPr fontId="1"/></si></sst>"#;
        assert_eq!(parse_shared_strings(xml).unwrap(), vec!["漢字"]);
    }
}
