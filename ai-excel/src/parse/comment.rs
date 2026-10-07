//! Parse legacy cell comments (notes).

use quick_xml::events::Event;
use quick_xml::Reader;

/// Extract `(cell_ref, text)` pairs from an `xl/commentsN.xml` part.
pub fn parse_comments(xml: &str) -> Vec<(String, String)> {
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    let mut cur_ref: Option<String> = None;
    let mut in_text = false;
    let mut buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"comment" => {
                    let mut r = None;
                    for a in e.attributes().filter_map(|a| a.ok()) {
                        if a.key.local_name().as_ref() == b"ref" {
                            r = a.unescape_value().map(|v| v.into_owned()).ok();
                        }
                    }
                    cur_ref = r;
                    buf.clear();
                }
                b"text" => in_text = true,
                _ => {}
            },
            Ok(Event::Text(e)) if in_text => {
                if let Ok(t) = e.unescape() {
                    buf.push_str(&t);
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"text" => in_text = false,
                b"comment" => {
                    if let Some(r) = cur_ref.take() {
                        if !buf.is_empty() {
                            out.push((r, buf.clone()));
                        }
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}
