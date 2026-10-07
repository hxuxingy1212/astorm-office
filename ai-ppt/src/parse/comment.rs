//! 传统批注解析
//!
//! 解析 `ppt/commentAuthors.xml`（作者表）与 `ppt/comments/commentN.xml`。

use crate::model::elements::{CommentElement, Position};
use crate::utils::constants::emu_to_inch;
use crate::utils::xml::raw_local_name;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// 解析作者表：authorId → name
pub(crate) fn parse_authors(xml: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if xml.is_empty() {
        return map;
    }
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e))
                if raw_local_name(e.name().as_ref()) == "cmAuthor" =>
            {
                let mut id = String::new();
                let mut name = String::new();
                for a in e.attributes().flatten() {
                    match a.key.as_ref() {
                        b"id" => id = String::from_utf8_lossy(&a.value).to_string(),
                        b"name" => name = String::from_utf8_lossy(&a.value).to_string(),
                        _ => {}
                    }
                }
                if !id.is_empty() {
                    map.insert(id, name);
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

/// 解析批注列表
pub(crate) fn parse_comments(xml: &str, authors: &HashMap<String, String>) -> Vec<CommentElement> {
    let mut out = Vec::new();
    if xml.is_empty() {
        return out;
    }
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut in_cm = false;
    let mut author_id = String::new();
    let mut pos = Position::default();
    let mut text = String::new();
    let mut in_text = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => match raw_local_name(e.name().as_ref()).as_str() {
                "cm" => {
                    in_cm = true;
                    author_id.clear();
                    pos = Position::default();
                    text.clear();
                    for a in e.attributes().flatten() {
                        if a.key.as_ref() == b"authorId" {
                            author_id = String::from_utf8_lossy(&a.value).to_string();
                        }
                    }
                }
                "pos" if in_cm => {
                    for a in e.attributes().flatten() {
                        if let Ok(v) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                            match a.key.as_ref() {
                                b"x" => pos.x = emu_to_inch(v),
                                b"y" => pos.y = emu_to_inch(v),
                                _ => {}
                            }
                        }
                    }
                }
                "text" if in_cm => in_text = true,
                _ => {}
            },
            Ok(Event::Empty(ref e)) if raw_local_name(e.name().as_ref()) == "pos" && in_cm => {
                for a in e.attributes().flatten() {
                    if let Ok(v) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                        match a.key.as_ref() {
                            b"x" => pos.x = emu_to_inch(v),
                            b"y" => pos.y = emu_to_inch(v),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Text(ref t)) if in_text => {
                text.push_str(
                    &t.unescape()
                        .unwrap_or_else(|_| std::str::from_utf8(t.as_ref()).unwrap_or("").into()),
                );
            }
            Ok(Event::End(ref e)) => match raw_local_name(e.name().as_ref()).as_str() {
                "text" => in_text = false,
                "cm" if in_cm => {
                    in_cm = false;
                    out.push(CommentElement {
                        position: pos.clone(),
                        author: authors.get(&author_id).cloned(),
                        text: text.trim().to_string(),
                    });
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}
