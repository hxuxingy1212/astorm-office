use std::collections::HashMap;

use crate::utils::xml::esc_xml;

/// Shared string table: deduplicates strings across the whole workbook.
#[derive(Default)]
pub struct SharedStrings {
    list: Vec<String>,
    map: HashMap<String, u32>,
}

impl SharedStrings {
    pub fn intern(&mut self, s: &str) -> u32 {
        if let Some(&i) = self.map.get(s) {
            return i;
        }
        let i = self.list.len() as u32;
        self.list.push(s.to_string());
        self.map.insert(s.to_string(), i);
        i
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
        out.push_str(&format!(
            "<sst xmlns=\"{}\" count=\"{}\" uniqueCount=\"{}\">",
            crate::utils::constants::NS_MAIN,
            self.list.len(),
            self.list.len()
        ));
        for s in &self.list {
            out.push_str("<si><t xml:space=\"preserve\">");
            out.push_str(&esc_xml(s));
            out.push_str("</t></si>");
        }
        out.push_str("</sst>");
        out
    }
}
