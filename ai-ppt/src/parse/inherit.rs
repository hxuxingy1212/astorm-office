//! 版式 / 母版占位符继承解析
//!
//! 幻灯片中的占位符（`p:sp` 内含 `p:ph`）经常不声明位置与文字属性，
//! 而是从对应的 slideLayout / slideMaster 继承。本模块从母版与版式中
//! 抽取占位符几何与默认文字样式，供幻灯片解析时回填。

use crate::model::elements::Position;
use crate::utils::constants::emu_to_inch;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// 占位符 / 文本层级的默认样式
#[derive(Debug, Clone, Default)]
pub(crate) struct PhStyle {
    /// 继承的几何位置（英寸）
    pub pos: Option<Position>,
    /// 默认字号（磅）
    pub font_size: Option<f64>,
    /// 默认文字颜色
    pub color: Option<String>,
    /// 默认字体
    pub font_family: Option<String>,
    /// 加粗
    pub bold: Option<bool>,
    /// 斜体
    pub italic: Option<bool>,
    /// 是否带项目符号
    pub bullet: Option<bool>,
}

impl PhStyle {
    fn merge_under(&mut self, parent: &PhStyle) {
        if self.pos.is_none() {
            self.pos = parent.pos.clone();
        }
        if self.font_size.is_none() {
            self.font_size = parent.font_size;
        }
        if self.color.is_none() {
            self.color = parent.color.clone();
        }
        if self.font_family.is_none() {
            self.font_family = parent.font_family.clone();
        }
        if self.bold.is_none() {
            self.bold = parent.bold;
        }
        if self.italic.is_none() {
            self.italic = parent.italic;
        }
        if self.bullet.is_none() {
            self.bullet = parent.bullet;
        }
    }
}

/// 幻灯片的继承上下文（版式覆盖母版）
#[derive(Debug, Clone, Default)]
pub(crate) struct Inheritance {
    /// 占位符样式，key = "type|idx"
    pub ph: HashMap<String, PhStyle>,
    pub title: PhStyle,
    pub body: PhStyle,
    pub other: PhStyle,
    /// 版式背景（幻灯片未设置背景时使用）
    pub background: Option<serde_json::Value>,
}

impl Inheritance {
    /// 用母版补齐版式缺失的属性（版式覆盖母版）
    pub fn overlay_master(&mut self, master: &Inheritance) {
        for (key, style) in self.ph.iter_mut() {
            if let Some(m) = master.ph.get(key) {
                style.merge_under(m);
            }
        }
        for (k, v) in &master.ph {
            self.ph.entry(k.clone()).or_insert_with(|| v.clone());
        }
        // txStyles 仅在母版定义
        self.title = master.title.clone();
        self.body = master.body.clone();
        self.other = master.other.clone();
        if self.background.is_none() {
            self.background = master.background.clone();
        }
    }

    /// 综合查找占位符样式：精确 type|idx → 仅 type，再叠加母版文本样式
    pub fn resolve(&self, ph_type: &str, ph_idx: &str) -> PhStyle {
        // 省略 type 的占位符按规范默认视为 body
        let t = if ph_type.is_empty() { "body" } else { ph_type };
        let mut s = self
            .ph
            .get(&format!("{}|{}", t, ph_idx))
            .or_else(|| {
                // 退化为仅匹配类型（idx 不同，如 body 1 vs body 2）
                self.ph.iter().find_map(|(k, v)| {
                    let (kt, _) = k.split_once('|')?;
                    (kt == t).then_some(v)
                })
            })
            .cloned()
            .unwrap_or_default();
        s.merge_under(self.master_style(t));
        s
    }

    /// 母版文本样式（标题/正文/其他）
    pub fn master_style(&self, ph_type: &str) -> &PhStyle {
        match ph_type {
            "title" | "ctrTitle" => &self.title,
            "body" | "subTitle" | "obj" | "" => &self.body,
            _ => &self.other,
        }
    }
}

/// 解析版式 XML 的占位符与背景
pub(crate) fn parse_layout(xml: &str) -> Inheritance {
    let mut inh = Inheritance::default();
    inh.ph = parse_placeholders(xml);
    inh.background = super::background::parse_bg_only(xml);
    inh
}

/// 解析母版 XML 的占位符与文本样式（txStyles）
pub(crate) fn parse_master(xml: &str) -> Inheritance {
    let mut inh = Inheritance::default();
    inh.ph = parse_placeholders(xml);
    let (title, body, other) = parse_tx_styles(xml);
    inh.title = title;
    inh.body = body;
    inh.other = other;
    inh.background = super::background::parse_bg_only(xml);
    inh
}

/// 抽取 `<p:sp>` 占位符（含 p:ph）的位置与文字默认样式
fn parse_placeholders(xml: &str) -> HashMap<String, PhStyle> {
    let mut map = HashMap::new();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut in_sp = false;
    let mut sp_depth = 0i32;
    let mut ph_type: Option<String> = None;
    let mut ph_idx = String::new();
    let mut pos = Position::default();
    let mut has_xfrm = false;
    let mut style = PhStyle::default();
    let mut in_def_rpr = false;
    let mut def_depth = 0i32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = crate::utils::xml::raw_local_name(e.name().as_ref());
                match name.as_str() {
                    "sp" => {
                        in_sp = true;
                        sp_depth = 1;
                        ph_type = None;
                        ph_idx.clear();
                        pos = Position::default();
                        has_xfrm = false;
                        style = PhStyle::default();
                    }
                    _ if in_sp => {
                        sp_depth += 1;
                        if in_def_rpr {
                            def_depth += 1;
                        }
                        apply_sp_start(
                            &name,
                            e,
                            &mut ph_type,
                            &mut ph_idx,
                            &mut pos,
                            &mut has_xfrm,
                            &mut style,
                            &mut in_def_rpr,
                            &mut def_depth,
                        );
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = crate::utils::xml::raw_local_name(e.name().as_ref());
                if in_sp {
                    apply_sp_start(
                        &name,
                        e,
                        &mut ph_type,
                        &mut ph_idx,
                        &mut pos,
                        &mut has_xfrm,
                        &mut style,
                        &mut in_def_rpr,
                        &mut def_depth,
                    );
                }
            }
            Ok(Event::End(ref e)) => {
                let name = crate::utils::xml::raw_local_name(e.name().as_ref());
                if !in_sp {
                    continue;
                }
                if in_def_rpr {
                    def_depth -= 1;
                    if def_depth <= 0 {
                        in_def_rpr = false;
                    }
                }
                if name == "sp" {
                    sp_depth -= 1;
                    if sp_depth <= 0 {
                        if let Some(t) = ph_type.take() {
                            if has_xfrm {
                                style.pos = Some(pos.clone());
                            }
                            map.entry(format!("{}|{}", t, ph_idx))
                                .or_insert(style.clone());
                        }
                        in_sp = false;
                    }
                } else {
                    sp_depth -= 1;
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

#[allow(clippy::too_many_arguments)]
fn apply_sp_start(
    name: &str,
    e: &quick_xml::events::BytesStart,
    ph_type: &mut Option<String>,
    ph_idx: &mut String,
    pos: &mut Position,
    has_xfrm: &mut bool,
    style: &mut PhStyle,
    in_def_rpr: &mut bool,
    def_depth: &mut i32,
) {
    match name {
        "ph" => {
            let mut t = None;
            for a in e.attributes().flatten() {
                match a.key.as_ref() {
                    b"type" => {
                        t = Some(String::from_utf8_lossy(&a.value).to_string());
                    }
                    b"idx" => {
                        *ph_idx = String::from_utf8_lossy(&a.value).to_string();
                    }
                    _ => {}
                }
            }
            *ph_type = Some(t.unwrap_or_default());
        }
        "off" => {
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
        "ext" => {
            *has_xfrm = true;
            for a in e.attributes().flatten() {
                if let Ok(v) = String::from_utf8_lossy(&a.value).parse::<i64>() {
                    match a.key.as_ref() {
                        b"cx" => pos.w = emu_to_inch(v),
                        b"cy" => pos.h = emu_to_inch(v),
                        _ => {}
                    }
                }
            }
        }
        "defRPr" | "rPr" => {
            *in_def_rpr = true;
            *def_depth = 1;
            for a in e.attributes().flatten() {
                let v = String::from_utf8_lossy(&a.value).to_string();
                match a.key.as_ref() {
                    b"sz" => {
                        if let Ok(sz) = v.parse::<f64>() {
                            if style.font_size.is_none() {
                                style.font_size = Some(sz / 100.0);
                            }
                        }
                    }
                    b"b" if style.bold.is_none() => {
                        style.bold = Some(v == "1" || v == "true");
                    }
                    b"i" if style.italic.is_none() => {
                        style.italic = Some(v == "1" || v == "true");
                    }
                    _ => {}
                }
            }
        }
        "srgbClr" | "schemeClr" if *in_def_rpr => {
            for a in e.attributes().flatten() {
                if a.key.as_ref() == b"val" && style.color.is_none() {
                    style.color = Some(String::from_utf8_lossy(&a.value).to_string());
                }
            }
        }
        "latin" if *in_def_rpr => {
            for a in e.attributes().flatten() {
                if a.key.as_ref() == b"typeface" && style.font_family.is_none() {
                    style.font_family = Some(String::from_utf8_lossy(&a.value).to_string());
                }
            }
        }
        "buChar" | "buAutoNum" => style.bullet = Some(true),
        "buNone" => style.bullet = Some(false),
        _ => {}
    }
}

/// 抽取母版 `<p:txStyles>` 中的 titleStyle/bodyStyle/otherStyle（lvl1 defRPr）
fn parse_tx_styles(xml: &str) -> (PhStyle, PhStyle, PhStyle) {
    let mut title = PhStyle::default();
    let mut body = PhStyle::default();
    let mut other = PhStyle::default();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut current: Option<u8> = None; // 1=title 2=body 3=other
    let mut in_def_rpr = false;
    let mut depth = 0i32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = crate::utils::xml::raw_local_name(e.name().as_ref());
                if in_def_rpr {
                    depth += 1;
                    capture_style_child(
                        &name,
                        e,
                        target_mut(&mut title, &mut body, &mut other, current),
                    );
                    continue;
                }
                match name.as_str() {
                    "titleStyle" => current = Some(1),
                    "bodyStyle" => current = Some(2),
                    "otherStyle" => current = Some(3),
                    "lvl1pPr" | "defRPr" => {
                        if let Some(t) = target_mut(&mut title, &mut body, &mut other, current) {
                            if name == "defRPr" {
                                capture_rpr_attrs(e, t);
                                in_def_rpr = true;
                                depth = 1;
                            }
                        }
                    }
                    "buChar" | "buAutoNum" => {
                        if let Some(t) = target_mut(&mut title, &mut body, &mut other, current) {
                            t.bullet = Some(true);
                        }
                    }
                    "buNone" => {
                        if let Some(t) = target_mut(&mut title, &mut body, &mut other, current) {
                            t.bullet = Some(false);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = crate::utils::xml::raw_local_name(e.name().as_ref());
                if name == "buChar" || name == "buAutoNum" {
                    if let Some(t) = target_mut(&mut title, &mut body, &mut other, current) {
                        t.bullet = Some(true);
                    }
                } else if name == "buNone" {
                    if let Some(t) = target_mut(&mut title, &mut body, &mut other, current) {
                        t.bullet = Some(false);
                    }
                } else if in_def_rpr {
                    capture_style_child(
                        &name,
                        e,
                        target_mut(&mut title, &mut body, &mut other, current),
                    );
                } else if name == "defRPr" {
                    if let Some(t) = target_mut(&mut title, &mut body, &mut other, current) {
                        capture_rpr_attrs(e, t);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let name = crate::utils::xml::raw_local_name(e.name().as_ref());
                if in_def_rpr {
                    depth -= 1;
                    if depth <= 0 {
                        in_def_rpr = false;
                    }
                }
                match name.as_str() {
                    "titleStyle" | "bodyStyle" | "otherStyle" => current = None,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    (title, body, other)
}

fn target_mut<'a>(
    title: &'a mut PhStyle,
    body: &'a mut PhStyle,
    other: &'a mut PhStyle,
    current: Option<u8>,
) -> Option<&'a mut PhStyle> {
    match current {
        Some(1) => Some(title),
        Some(2) => Some(body),
        Some(3) => Some(other),
        _ => None,
    }
}

fn capture_rpr_attrs(e: &quick_xml::events::BytesStart, style: &mut PhStyle) {
    for a in e.attributes().flatten() {
        let v = String::from_utf8_lossy(&a.value).to_string();
        match a.key.as_ref() {
            b"sz" if style.font_size.is_none() => {
                if let Ok(sz) = v.parse::<f64>() {
                    style.font_size = Some(sz / 100.0);
                }
            }
            b"b" if style.bold.is_none() => {
                style.bold = Some(v == "1" || v == "true");
            }
            b"i" if style.italic.is_none() => {
                style.italic = Some(v == "1" || v == "true");
            }
            _ => {}
        }
    }
}

fn capture_style_child(name: &str, e: &quick_xml::events::BytesStart, style: Option<&mut PhStyle>) {
    let Some(style) = style else { return };
    match name {
        "srgbClr" | "schemeClr" => {
            for a in e.attributes().flatten() {
                if a.key.as_ref() == b"val" && style.color.is_none() {
                    style.color = Some(String::from_utf8_lossy(&a.value).to_string());
                }
            }
        }
        "latin" => {
            for a in e.attributes().flatten() {
                if a.key.as_ref() == b"typeface" && style.font_family.is_none() {
                    style.font_family = Some(String::from_utf8_lossy(&a.value).to_string());
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAYOUT: &str = r#"<p:sldLayout xmlns:p="urn:p" xmlns:a="urn:a">
      <p:cSld><p:spTree>
        <p:sp><p:nvSpPr><p:cNvPr id="2" name="Title"/><p:cNvSpPr/><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr>
          <p:spPr><a:xfrm><a:off x="914400" y="457200"/><a:ext cx="10000000" cy="1000000"/></a:xfrm></p:spPr>
        </p:sp>
        <p:sp><p:nvSpPr><p:cNvPr id="3" name="Body"/><p:nvPr><p:ph type="body" idx="1"/></p:nvPr></p:nvSpPr>
          <p:spPr><a:xfrm><a:off x="914400" y="1828800"/><a:ext cx="8000000" cy="4000000"/></a:xfrm></p:spPr>
          <p:txBody><a:lstStyle><a:lvl1pPr><a:defRPr sz="2400"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:defRPr></a:lvl1pPr></a:lstStyle></p:txBody>
        </p:sp>
      </p:spTree></p:cSld>
    </p:sldLayout>"#;

    const MASTER: &str = r#"<p:sldMaster xmlns:p="urn:p" xmlns:a="urn:a">
      <p:txStyles>
        <p:titleStyle><a:lvl1pPr><a:defRPr sz="4400"><a:solidFill><a:schemeClr val="tx1"/></a:solidFill></a:defRPr></a:lvl1pPr></p:titleStyle>
        <p:bodyStyle><a:lvl1pPr><a:defRPr sz="1800"/></a:lvl1pPr></p:bodyStyle>
        <p:otherStyle><a:lvl1pPr><a:defRPr sz="1000"/></a:lvl1pPr></p:otherStyle>
      </p:txStyles>
    </p:sldMaster>"#;

    #[test]
    fn resolves_layout_then_master() {
        let master = parse_master(MASTER);
        let mut layout = parse_layout(LAYOUT);
        layout.overlay_master(&master);

        // 标题：版式提供几何，母版 txStyles 提供字号
        let title = layout.resolve("title", "");
        assert!((title.pos.as_ref().unwrap().x - 1.0).abs() < 0.01);
        assert_eq!(title.font_size, Some(44.0));

        // 正文：版式自身的字号/颜色优先于母版
        let body = layout.resolve("body", "1");
        assert_eq!(body.font_size, Some(24.0));
        assert_eq!(body.color.as_deref(), Some("tx1"));
    }

    #[test]
    fn master_style_fallback() {
        let master = parse_master(MASTER);
        let layout = parse_layout("<p:sldLayout xmlns:p=\"urn:p\"/>");
        let mut inh = layout;
        inh.overlay_master(&master);
        assert_eq!(inh.resolve("body", "1").font_size, Some(18.0));
        assert_eq!(inh.resolve("other", "").font_size, Some(10.0));
    }
}
