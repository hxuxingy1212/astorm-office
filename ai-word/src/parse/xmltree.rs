//! 最小 XML 树解析（基于 quick-xml），用于宽容地读取 OOXML 部件

use crate::error::{Error, Result};
use quick_xml::events::Event;
use std::collections::HashMap;

/// XML 节点（保留带前缀的完整标签名，如 `w:p`）
#[derive(Debug, Clone, Default)]
pub struct XmlNode {
    pub name: String,
    pub attrs: HashMap<String, String>,
    pub children: Vec<XmlNode>,
    pub text: String,
}

impl XmlNode {
    fn new(name: &str) -> Self {
        XmlNode {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// 第一个指定名字的子节点
    pub fn child(&self, name: &str) -> Option<&XmlNode> {
        self.children.iter().find(|c| c.name == name)
    }

    /// 所有指定名字的子节点
    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlNode> {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// 属性（带前缀，如 `w:val`）
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(|s| s.as_str())
    }

    /// 递归拼接文本
    pub fn text_content(&self) -> String {
        let mut s = self.text.clone();
        for c in &self.children {
            s.push_str(&c.text_content());
        }
        s
    }

    /// 序列化为 XML 字符串（用于未建模内容的原样透传）
    pub fn to_xml(&self) -> String {
        let mut out = String::new();
        self.write_xml(&mut out);
        out
    }

    fn write_xml(&self, out: &mut String) {
        out.push('<');
        out.push_str(&self.name);
        for (k, v) in &self.attrs {
            out.push(' ');
            out.push_str(k);
            out.push_str("=\"");
            escape_into(v, out, true);
            out.push('"');
        }
        if self.children.is_empty() && self.text.is_empty() {
            out.push_str("/>");
            return;
        }
        out.push('>');
        escape_into(&self.text, out, false);
        for c in &self.children {
            c.write_xml(out);
        }
        out.push_str("</");
        out.push_str(&self.name);
        out.push('>');
    }

    /// 是否存在指定名称的后代（限深度以避免过深扫描）
    pub fn has_descendant(&self, name: &str) -> bool {
        self.children
            .iter()
            .any(|c| c.name == name || c.has_descendant(name))
    }
}

fn escape_into(s: &str, out: &mut String, attr: bool) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
}

/// 解析 XML 字符串为节点树，返回根节点
pub fn parse_xml(xml: &str) -> Result<XmlNode> {
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let node = make_node(&e)?;
                stack.push(node);
            }
            Ok(Event::Empty(e)) => {
                let node = make_node(&e)?;
                if let Some(top) = stack.last_mut() {
                    top.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            Ok(Event::End(_)) => {
                if let Some(node) = stack.pop() {
                    if let Some(top) = stack.last_mut() {
                        top.children.push(node);
                    } else {
                        root = Some(node);
                    }
                }
            }
            Ok(Event::Text(e)) => {
                let t = match e.unescape() {
                    Ok(v) => v.to_string(),
                    Err(_) => String::from_utf8_lossy(e.as_ref()).to_string(),
                };
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t);
                }
            }
            Ok(Event::CData(e)) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(Error::Xml(e)),
        }
    }
    root.ok_or_else(|| Error::InvalidInput("XML 为空".to_string()))
}

fn make_node(e: &quick_xml::events::BytesStart) -> Result<XmlNode> {
    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
    let mut node = XmlNode::new(&name);
    for attr in e.attributes() {
        let attr = match attr {
            Ok(a) => a,
            Err(_) => continue,
        };
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let val = match attr.unescape_value() {
            Ok(v) => v.to_string(),
            Err(_) => String::from_utf8_lossy(&attr.value).to_string(),
        };
        node.attrs.insert(key, val);
    }
    Ok(node)
}
