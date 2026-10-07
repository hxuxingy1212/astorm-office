//! 分组元素解析
//!
//! 从 p:grpSp 节点递归解析分组（嵌套子元素、旋转）。

use crate::model::elements::*;
use quick_xml::events::Event;
use quick_xml::Reader;

use super::slide::parse_element_start;

pub(super) fn parse_grp_sp(
    reader: &mut Reader<&[u8]>,
    r_id_map: &std::collections::HashMap<String, String>,
    inherit: Option<&crate::parse::inherit::Inheritance>,
    charts: &std::collections::HashMap<String, String>,
    slide_xml: &str,
) -> Option<Element> {
    let mut depth = 1u32;
    let mut position = Position {
        x: 0.0,
        y: 0.0,
        w: 0.0,
        h: 0.0,
    };
    let mut name = None;
    let mut id = None;
    let mut rotation = None;
    let mut children: Vec<Element> = Vec::new();
    let mut in_grp_sp_pr = false;
    let mut got_xfrm = false;

    let mut buf = Vec::new();
    loop {
        let ev_pos = reader.buffer_position() as usize;
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let n = super::tag(e).to_string();
                match n.as_str() {
                    "p:cNvPr" => {
                        name = super::parse_name_attr(e);
                        id = super::parse_id_attr(e);
                    }
                    "p:grpSpPr" => {
                        in_grp_sp_pr = true;
                    }
                    "a:xfrm" if in_grp_sp_pr && !got_xfrm => {
                        got_xfrm = true;
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
                    "a:off" if in_grp_sp_pr => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:ext" if in_grp_sp_pr => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "p:sp" | "p:pic" | "p:graphicFrame" | "p:grpSp" => {
                        // 子元素由 parse_element_start 完整解析（含其 End 事件），
                        // 此处不递增 depth；depth 仅跟踪 group 自身结束
                        if let Some(child) = parse_element_start(
                            e, r_id_map, reader, inherit, charts, slide_xml, ev_pos,
                        ) {
                            children.push(child);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                // 自闭合标签（cNvPr / off / ext / xfrm 均为 Empty 事件）
                let n = super::tag(e).to_string();
                match n.as_str() {
                    "p:cNvPr" => {
                        name = super::parse_name_attr(e);
                        id = super::parse_id_attr(e);
                    }
                    "a:xfrm" if in_grp_sp_pr && !got_xfrm => {
                        got_xfrm = true;
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
                    "a:off" if in_grp_sp_pr => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    "a:ext" if in_grp_sp_pr => {
                        super::parse_pos_attrs(e, &mut position);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let n = super::tag(e).to_string();
                match n.as_str() {
                    "p:grpSpPr" => {
                        in_grp_sp_pr = false;
                    }
                    "p:grpSp" => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {
                        // End of child element
                    }
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    Some(Element::Group(GroupElement {
        position,
        name,
        id,
        rotation,
        children,
        animations: None,
    }))
}
