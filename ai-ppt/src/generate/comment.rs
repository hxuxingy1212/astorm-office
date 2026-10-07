//! 传统批注部件生成
//!
//! 生成 `ppt/commentAuthors.xml` 与 `ppt/comments/commentN.xml`。

use crate::model::elements::CommentElement;
use crate::utils::constants::inch_to_emu;
use crate::utils::xml::esc_xml;
use std::collections::HashMap;

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// 生成作者表
pub fn generate_authors(authors: &[String]) -> String {
    let mut xml = format!(
        "{}<p:cmAuthorLst xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\">",
        XML_DECL
    );
    for (i, name) in authors.iter().enumerate() {
        xml.push_str(&format!(
            "<p:cmAuthor id=\"{}\" name=\"{}\" initials=\"{}\" lastIdx=\"{}\" clrIdx=\"{}\"/>",
            i,
            esc_xml(name),
            esc_xml(&initials(name)),
            i + 1,
            i
        ));
    }
    xml.push_str("</p:cmAuthorLst>");
    xml
}

/// 生成单张幻灯片的批注部件
pub fn generate_comments(comments: &[CommentElement], author_ids: &HashMap<String, u32>) -> String {
    let mut xml = format!(
        "{}<p:cmLst xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\">",
        XML_DECL
    );
    for (i, c) in comments.iter().enumerate() {
        let id = c
            .author
            .as_ref()
            .and_then(|a| author_ids.get(a))
            .copied()
            .unwrap_or(0);
        xml.push_str(&format!(
            "<p:cm authorId=\"{}\" dt=\"2024-01-01T00:00:00\" idx=\"{}\">\
             <p:pos x=\"{}\" y=\"{}\"/>\
             <p:text>{}</p:text>\
             </p:cm>",
            id,
            i + 1,
            inch_to_emu(c.position.x),
            inch_to_emu(c.position.y),
            esc_xml(&c.text)
        ));
    }
    xml.push_str("</p:cmLst>");
    xml
}

fn initials(name: &str) -> String {
    name.chars().take(2).collect()
}
