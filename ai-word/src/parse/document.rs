//! 解析 document.xml 及相关部件为 Document 模型

use crate::error::Result;
use crate::model::blocks::*;
use crate::model::style::{Margins, PageSetup, Style, Theme};
use crate::model::{Document, Meta, Part, Section};
use crate::parse::xmltree::{parse_xml, XmlNode};
use crate::utils::units::*;
use std::collections::{BTreeMap, HashMap};

/// 部件字节集合：part 路径 → 内容
pub type Package = HashMap<String, Vec<u8>>;

fn text_of(files: &Package, name: &str) -> Option<String> {
    files
        .get(name)
        .map(|b| String::from_utf8_lossy(b).to_string())
}

/// 关系映射：rId → (target, external, type)
struct Rels {
    map: HashMap<String, (String, bool, String)>,
}

impl Rels {
    fn parse(files: &Package, part: &str) -> Rels {
        let mut map = HashMap::new();
        if let Some(txt) = text_of(files, part) {
            if let Ok(root) = parse_xml(&txt) {
                for rel in root.children_named("Relationship") {
                    let id = rel.attr("Id").unwrap_or("").to_string();
                    let target = rel.attr("Target").unwrap_or("").to_string();
                    let external = rel.attr("TargetMode") == Some("External");
                    let rtype = rel.attr("Type").unwrap_or("").to_string();
                    map.insert(id, (target, external, rtype));
                }
            }
        }
        Rels { map }
    }

    fn target(&self, id: &str) -> Option<(&str, bool)> {
        self.map.get(id).map(|(t, e, _)| (t.as_str(), *e))
    }

    fn rel_type(&self, id: &str) -> Option<&str> {
        self.map.get(id).map(|(_, _, t)| t.as_str())
    }
}

/// 列表编号层级定义
#[derive(Debug, Clone, Default)]
pub(crate) struct NumLevel {
    pub fmt: String,
    pub text: String,
    pub start: i64,
    pub font: Option<String>,
}

/// 列表编号定义（抽象编号）
#[derive(Debug, Clone, Default)]
pub(crate) struct NumDef {
    pub levels: std::collections::BTreeMap<u8, NumLevel>,
}

/// 解析上下文
struct PCtx<'a> {
    rels: &'a Rels,
    numbering: &'a HashMap<String, NumDef>,
    styles: &'a BTreeMap<String, Style>,
    notes: &'a HashMap<String, String>,
    enotes: &'a HashMap<String, String>,
    comments: &'a HashMap<String, (String, String)>,
    files: &'a Package,
}

/// 解析 VML 长度（pt/in/cm/mm/px）为 pt
fn parse_vml_len(v: &str) -> Option<f64> {
    let v = v.trim();
    let (num, unit) = if let Some(n) = v.strip_suffix("pt") {
        (n, "pt")
    } else if let Some(n) = v.strip_suffix("in") {
        (n, "in")
    } else if let Some(n) = v.strip_suffix("cm") {
        (n, "cm")
    } else if let Some(n) = v.strip_suffix("mm") {
        (n, "mm")
    } else if let Some(n) = v.strip_suffix("px") {
        (n, "px")
    } else {
        (v, "pt")
    };
    let n: f64 = num.trim().parse().ok()?;
    Some(match unit {
        "in" => n * 72.0,
        "cm" => n * 72.0 / 2.54,
        "mm" => n * 72.0 / 25.4,
        "px" => n * 72.0 / 96.0,
        _ => n,
    })
}

/// 归一化关系目标为产物内相对路径（word/media/xxx）
fn normalize_media_target(target: &str) -> String {
    let t = target.trim_start_matches('/');
    if t.starts_with("word/") {
        t.to_string()
    } else {
        format!("word/{t}")
    }
}

/// 解析主文档部件（从 _rels/.rels 的 officeDocument 关系；默认 word/document.xml）
pub(crate) fn main_document_part(files: &Package) -> String {
    if let Some(txt) = text_of(files, "_rels/.rels") {
        if let Ok(root) = parse_xml(&txt) {
            for rel in root.children_named("Relationship") {
                let ty = rel.attr("Type").unwrap_or("");
                if ty.ends_with("/officeDocument") {
                    if let Some(t) = rel.attr("Target") {
                        let t = t.trim_start_matches('/').to_string();
                        if files.contains_key(&t) {
                            return t;
                        }
                    }
                }
            }
        }
    }
    "word/document.xml".to_string()
}

/// 由主部件路径推导其 rels 路径（word/document2.xml → word/_rels/document2.xml.rels）
fn rels_path_for(main: &str) -> String {
    match main.rfind('/') {
        Some(i) => format!("{}/_rels/{}.rels", &main[..i], &main[i + 1..]),
        None => "_rels/.rels".to_string(),
    }
}

/// 解析整份文档
pub fn parse_document(files: &Package) -> Result<Document> {
    let main = main_document_part(files);
    let rels_path = rels_path_for(&main);
    let rels = Rels::parse(files, &rels_path);
    let numbering = parse_numbering(files);
    let notes = parse_footnotes(files);
    let enotes = parse_endnotes(files);
    let comments = parse_comments(files);
    let styles = parse_styles(files);
    let pctx = PCtx {
        rels: &rels,
        numbering: &numbering,
        styles: &styles,
        notes: &notes,
        enotes: &enotes,
        comments: &comments,
        files,
    };

    let mut meta = Meta::default();
    let mut page = PageSetup::default();
    // 若文档完全没有 sectPr，则视为未指定纸张尺寸
    page.size_specified = Some(false);
    let mut background: Option<String> = None;
    let mut parts: Vec<Part> = Vec::new();

    if let Some(doc_txt) = text_of(files, &main) {
        let root = parse_xml(&doc_txt)?;
        if let Some(bg) = root
            .child("w:background")
            .and_then(|b| b.attr("w:color"))
            .filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("auto"))
        {
            background = Some(bg.trim_start_matches('#').to_string());
        }
        if let Some(body) = root.child("w:body") {
            let mut segments: Vec<(Option<Section>, Vec<Block>)> = Vec::new();
            let mut seg: Vec<&XmlNode> = Vec::new();
            let mut body_sect: Option<&XmlNode> = None;
            let mut marker_found = false;

            for c in &body.children {
                if c.name == "w:sectPr" {
                    body_sect = Some(c);
                    continue;
                }
                if let Some(sp) = marker_sect_pr(c) {
                    marker_found = true;
                    // 分节符段落本身可能也含文本，先纳入本段再收尾
                    seg.push(c);
                    let blocks = parse_blocks(&seg, &pctx);
                    segments.push((Some(section_from(files, &rels, sp)), blocks));
                    seg.clear();
                    continue;
                }
                seg.push(c);
            }
            let last_blocks = parse_blocks(&seg, &pctx);

            if marker_found {
                let last_section = body_sect.map(|s| section_from(files, &rels, s));
                segments.push((last_section, last_blocks));
                if let Some(s) = body_sect {
                    page = parse_sect_pr(s);
                }
                parts = segments
                    .into_iter()
                    .map(|(section, blocks)| Part { section, blocks })
                    .collect();
            } else {
                // 无显式分节：按一级标题分片，节属性统一放在首片
                let section = body_sect.map(|s| section_from(files, &rels, s));
                if let Some(s) = body_sect {
                    page = parse_sect_pr(s);
                }
                parts = split_parts(last_blocks);
                if let Some(sec) = section {
                    // body sectPr 是“最后一节”的属性，挂到末片避免多出分节符
                    if let Some(last) = parts.last_mut() {
                        last.section = Some(sec);
                    }
                }
            }
        }
    }

    if let Some(core) = text_of(files, "docProps/core.xml") {
        if let Ok(root) = parse_xml(&core) {
            meta.title = root
                .child("dc:title")
                .map(|n| n.text_content())
                .filter(|s| !s.is_empty());
            meta.author = root
                .child("dc:creator")
                .map(|n| n.text_content())
                .filter(|s| !s.is_empty());
            meta.subject = root
                .child("dc:subject")
                .map(|n| n.text_content())
                .filter(|s| !s.is_empty());
            if let Some(k) = root.child("cp:keywords").map(|n| n.text_content()) {
                if !k.is_empty() {
                    meta.keywords = k.split(',').map(|s| s.trim().to_string()).collect();
                }
            }
        }
    }

    let theme = parse_theme(files);
    let theme_xml = text_of(files, "word/theme/theme1.xml");
    let defaults = parse_doc_defaults(files);
    let watermark = parse_watermark(files);
    let passthrough = collect_passthrough(files, &referenced_media(&parts));
    let raw_styles = parse_raw_styles(files);
    let even_and_odd = text_of(files, "word/settings.xml")
        .and_then(|t| parse_xml(&t).ok())
        .map(|r| r.child("w:evenAndOddHeaders").is_some());
    let default_tab_stop = text_of(files, "word/settings.xml")
        .and_then(|t| parse_xml(&t).ok())
        .and_then(|r| {
            r.child("w:defaultTabStop")
                .and_then(|d| d.attr("w:val"))
                .and_then(|v| v.parse::<i64>().ok())
        });

    Ok(Document {
        meta,
        page,
        theme,
        styles,
        defaults,
        template: None,
        watermark,
        background,
        parts,
        passthrough,
        raw_styles,
        default_tab_stop,
        theme_xml,
        even_and_odd,
    })
}

/// 从页眉中解析 VML 水印文本
fn parse_watermark(files: &Package) -> Option<String> {
    fn find(node: &XmlNode) -> Option<String> {
        if node.name == "v:textpath" {
            if let Some(s) = node.attr("string") {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
        }
        for c in &node.children {
            if let Some(s) = find(c) {
                return Some(s);
            }
        }
        None
    }
    for (name, bytes) in files {
        if !name.starts_with("word/header") {
            continue;
        }
        if let Ok(root) = parse_xml(&String::from_utf8_lossy(bytes)) {
            if let Some(wm) = find(&root) {
                return Some(wm);
            }
        }
    }
    None
}

fn read_header_footer(files: &Package, path: &str) -> Option<String> {
    let txt = text_of(files, path)?;
    let root = parse_xml(&txt).ok()?;
    let s = root.text_content();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// 超链接默认色（生成时注入）在解析时归零，保持回环稳定
fn normalize_link_color(run: &mut Run) {
    if run.hyperlink.is_some() && run.color.as_deref() == Some("0563C1") {
        run.color = None;
    }
}

/// 段落中是否内嵌节属性（分节符）
/// 解析页眉/页脚部件为块（保留段落格式/表格；不含依赖部件的图片）
fn read_hf_blocks(files: &Package, path: &str) -> Option<Vec<Block>> {
    let txt = text_of(files, path)?;
    let root = parse_xml(&txt).ok()?;
    let empty_rels = Rels {
        map: HashMap::new(),
    };
    let empty_num: HashMap<String, NumDef> = HashMap::new();
    let empty: HashMap<String, String> = HashMap::new();
    let empty2: HashMap<String, (String, String)> = HashMap::new();
    let empty_styles: BTreeMap<String, Style> = BTreeMap::new();
    let pctx = PCtx {
        rels: &empty_rels,
        numbering: &empty_num,
        styles: &empty_styles,
        notes: &empty,
        enotes: &empty,
        comments: &empty2,
        files,
    };
    let children: Vec<&XmlNode> = root
        .children
        .iter()
        .filter(|c| c.name == "w:p" || c.name == "w:tbl")
        .collect();
    let blocks = parse_blocks(&children, &pctx);
    let mut kept: Vec<Block> = Vec::new();
    for b in blocks {
        match b {
            Block::List(l) => {
                // 页眉无 numbering 部件，列表展开为普通段落
                for it in l.items {
                    kept.extend(it.blocks);
                }
            }
            Block::Paragraph(_)
            | Block::Table(_)
            | Block::Caption(_)
            | Block::Quote(_)
            | Block::Code(_)
            | Block::Formula(_)
            | Block::TextBox(_)
            | Block::Shape(_) => kept.push(b),
            _ => {}
        }
    }
    if kept.is_empty() {
        None
    } else {
        Some(kept)
    }
}

fn marker_sect_pr(p: &XmlNode) -> Option<&XmlNode> {
    if p.name != "w:p" {
        return None;
    }
    p.child("w:pPr").and_then(|ppr| ppr.child("w:sectPr"))
}

/// 从 sectPr 构造 Section（含首页/奇偶页页眉页脚）
fn section_from(files: &Package, rels: &Rels, sect: &XmlNode) -> Section {
    let mut s = Section {
        page: Some(parse_sect_pr(sect)),
        ..Default::default()
    };
    if let Some(t) = sect
        .child("w:type")
        .and_then(|t| t.attr("w:val"))
        .filter(|v| !v.is_empty())
    {
        s.section_type = Some(t.to_string());
    }
    for r in sect.children_named("w:headerReference") {
        let kind = r.attr("w:type").unwrap_or("default");
        let target = r
            .attr("r:id")
            .and_then(|id| rels.target(id))
            .map(|(t, _)| t.to_string());
        let text = target
            .as_deref()
            .and_then(|t| read_header_footer(files, &format!("word/{t}")));
        let blocks = target
            .as_deref()
            .and_then(|t| read_hf_blocks(files, &format!("word/{t}")));
        match kind {
            "first" => {
                s.first_header = text;
                s.first_header_blocks = blocks;
            }
            "even" => {
                s.even_header = text;
                s.even_header_blocks = blocks;
            }
            _ => {
                s.header = text;
                s.header_blocks = blocks;
            }
        }
    }
    for r in sect.children_named("w:footerReference") {
        match r.attr("w:type").unwrap_or("default") {
            "first" => s.first_footer_page_number = Some(true),
            "even" => s.even_footer_page_number = Some(true),
            _ => s.footer_page_number = Some(true),
        }
        // 页脚块（默认页脚）
        if let Some(t) = r
            .attr("r:id")
            .and_then(|id| rels.target(id))
            .map(|(t, _)| t.to_string())
        {
            if !r.attr("w:type").map(|k| k != "default").unwrap_or(false) {
                s.footer_blocks = read_hf_blocks(files, &format!("word/{t}"));
            }
        }
        // 检测页码格式（page_of 含 NUMPAGES）
        if s.footer_format.is_none() {
            if let Some(txt) = r
                .attr("r:id")
                .and_then(|id| rels.target(id))
                .and_then(|(t, _)| text_of(files, &format!("word/{t}")))
            {
                if txt.contains("NUMPAGES") {
                    s.footer_format = Some("page_of".to_string());
                }
            }
        }
    }
    // 首页不同（w:titlePg）
    let title_pg = sect.child("w:titlePg").is_some();
    s.title_page = Some(title_pg);
    if title_pg && s.first_footer_page_number.is_none() {
        s.first_footer_page_number = Some(false);
    }
    s.page_number_start = sect
        .child("w:pgNumType")
        .and_then(|p| p.attr("w:start"))
        .and_then(|v| v.parse().ok());
    s.page_number_format = sect
        .child("w:pgNumType")
        .and_then(|p| p.attr("w:fmt"))
        .filter(|v| !v.is_empty() && *v != "decimal")
        .map(|v| v.to_string());
    s
}

/// 按 level-1 标题把块切成多个 Part
fn split_parts(blocks: Vec<Block>) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    let mut current: Vec<Block> = Vec::new();
    let mut started = false;
    for b in blocks {
        let is_h1 = matches!(b, Block::Heading { level: 1, .. });
        if is_h1 && started {
            parts.push(Part {
                section: None,
                blocks: std::mem::take(&mut current),
            });
        }
        if is_h1 {
            started = true;
        }
        current.push(b);
    }
    if !current.is_empty() || parts.is_empty() {
        parts.push(Part {
            section: None,
            blocks: current,
        });
    }
    parts
}

// ---------- 块解析 ----------

fn parse_blocks(children: &[&XmlNode], ctx: &PCtx) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < children.len() {
        let c = children[i];
        match c.name.as_str() {
            "w:p" => {
                let style = style_id(c);
                // 目录（live 域）：吸附其后的 TOC1-3 缓存行作为目录项
                if is_toc(c) {
                    let mut entries: Vec<crate::model::blocks::TocEntry> = Vec::new();
                    let mut j = i + 1;
                    while j < children.len() {
                        if let Some(lvl) = style_id(children[j]).as_deref().and_then(toc_level) {
                            let t = paragraph_text_of(children[j]).trim().to_string();
                            if !t.is_empty() {
                                entries.push(crate::model::blocks::TocEntry {
                                    level: lvl,
                                    text: t,
                                });
                            }
                            j += 1;
                            continue;
                        }
                        break;
                    }
                    blocks.push(Block::Toc(TocBlock {
                        title: "目录".to_string(),
                        levels: Some(vec![1, 2, 3]),
                        is_static: None,
                        entries,
                    }));
                    i = j;
                    continue;
                }
                // Code 分组
                if style.as_deref() == Some("Code") {
                    let mut text = String::new();
                    while i < children.len() && style_id(children[i]).as_deref() == Some("Code") {
                        let t = paragraph_text_of(children[i]);
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&t);
                        i += 1;
                    }
                    blocks.push(Block::Code(CodeBlock { lang: None, text }));
                    continue;
                }
                // 列表分组（标题样式不并入列表）
                let is_heading = style
                    .as_deref()
                    .map(|s| s.to_ascii_lowercase().starts_with("heading"))
                    .unwrap_or(false);
                if let Some((num_id, _ilvl)) = num_pr(c) {
                    if is_heading {
                        blocks.extend(parse_paragraph_multi(c, ctx));
                        i += 1;
                        continue;
                    }
                    let ordered = is_ordered(ctx.numbering.get(&num_id));
                    let mut items = Vec::new();
                    while i < children.len() {
                        if let Some((nid, lvl)) = num_pr(children[i]) {
                            if nid != num_id {
                                break;
                            }
                            let lvl_n: u8 = lvl.parse().unwrap_or(0);
                            let text = paragraph_text_of(children[i]);
                            let (num_format, lvl_text, start, num_font) =
                                custom_numbering(ctx.numbering.get(&num_id), lvl_n, ordered);
                            let mut para = Paragraph::text(text);
                            parse_paragraph_props(children[i], &mut para);
                            para.style =
                                style_id(children[i]).filter(|s| ctx.styles.contains_key(s));
                            items.push(ListItem {
                                blocks: vec![Block::Paragraph(para)],
                                level: lvl_n,
                                num_format,
                                lvl_text,
                                start,
                                num_font,
                            });
                            i += 1;
                            continue;
                        }
                        // 同列表样式（样式挂接编号，无段落 numPr）
                        let sty = style_id(children[i]);
                        let is_h = sty
                            .as_deref()
                            .map(|s| s.to_ascii_lowercase().starts_with("heading"))
                            .unwrap_or(false);
                        if !is_h && style_list_kind(sty.as_deref()) == Some(ordered) {
                            let mut para = Paragraph::text(paragraph_text_of(children[i]));
                            parse_paragraph_props(children[i], &mut para);
                            para.style = sty.clone().filter(|s| ctx.styles.contains_key(s));
                            let (num_format, lvl_text, start, num_font) =
                                style_list_numbering(ctx, sty.as_deref(), ordered);
                            items.push(ListItem {
                                blocks: vec![Block::Paragraph(para)],
                                level: 0,
                                num_format,
                                lvl_text,
                                start,
                                num_font,
                            });
                            i += 1;
                            continue;
                        }
                        break;
                    }
                    blocks.push(Block::List(ListBlock { ordered, items }));
                    continue;
                }
                // 样式挂接编号的列表（无段落 numPr）
                if !is_heading {
                    if let Some(ordered) = style_list_kind(style.as_deref()) {
                        if num_pr(c).is_none() {
                            let mut items = Vec::new();
                            while i < children.len() {
                                let sty = style_id(children[i]);
                                let is_h = sty
                                    .as_deref()
                                    .map(|s| s.to_ascii_lowercase().starts_with("heading"))
                                    .unwrap_or(false);
                                if num_pr(children[i]).is_some()
                                    || is_h
                                    || style_list_kind(sty.as_deref()) != Some(ordered)
                                {
                                    break;
                                }
                                let mut para = Paragraph::text(paragraph_text_of(children[i]));
                                parse_paragraph_props(children[i], &mut para);
                                para.style = sty.clone().filter(|s| ctx.styles.contains_key(s));
                                let (num_format, lvl_text, start, num_font) =
                                    style_list_numbering(ctx, sty.as_deref(), ordered);
                                items.push(ListItem {
                                    blocks: vec![Block::Paragraph(para)],
                                    level: 0,
                                    num_format,
                                    lvl_text,
                                    start,
                                    num_font,
                                });
                                i += 1;
                            }
                            blocks.push(Block::List(ListBlock { ordered, items }));
                            continue;
                        }
                    }
                }
                blocks.extend(parse_paragraph_multi(c, ctx));
            }
            "w:tbl" => {
                blocks.push(parse_table(c, ctx));
            }
            "m:oMathPara" | "m:oMath" => {
                blocks.push(Block::Formula(FormulaBlock {
                    latex: crate::parse::math::omml_to_latex(c),
                    display: c.name == "m:oMathPara",
                    number: None,
                }));
            }
            // 块级内容控件（SDT）
            "w:sdt" => {
                let props = parse_sdt_props(c);
                let inner: Vec<&XmlNode> = match c.child("w:sdtContent") {
                    Some(sc) => sc.children.iter().collect(),
                    None => c.children.iter().collect(),
                };
                let inner_blocks = parse_blocks(&inner, ctx);
                if inner_blocks.is_empty() {
                    blocks.extend(parse_blocks(&inner, ctx));
                } else {
                    blocks.push(Block::Sdt(ContentControlBlock {
                        props,
                        blocks: inner_blocks,
                    }));
                }
            }
            // 自定义 XML / 修订：递归解析其块级子元素
            "w:customXml" | "w:ins" | "w:del" | "w:moveFrom" | "w:moveTo" => {
                let inner: Vec<&XmlNode> = match c.child("w:sdtContent") {
                    Some(sc) => sc.children.iter().collect(),
                    None => c.children.iter().collect(),
                };
                blocks.extend(parse_blocks(&inner, ctx));
            }
            _ => {}
        }
        i += 1;
    }
    fold_captions(blocks)
}

/// 把紧跟图/表后的题注段合并回图/表的 caption 字段
fn fold_captions(blocks: Vec<Block>) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    for b in blocks {
        if let Block::Caption(c) = &b {
            match out.last_mut() {
                Some(Block::Table(t)) if t.caption.is_none() => {
                    t.caption = Some(c.text.clone());
                    continue;
                }
                Some(Block::Image(i)) if i.caption.is_none() => {
                    i.caption = Some(c.text.clone());
                    continue;
                }
                _ => {}
            }
        }
        out.push(b);
    }
    out
}

/// 解析内容控件属性（w:sdtPr）
fn parse_sdt_props(sdt: &XmlNode) -> SdtProps {
    let mut p = SdtProps::default();
    let Some(pr) = sdt.child("w:sdtPr") else {
        return p;
    };
    let val = |n: Option<&XmlNode>| n.and_then(|x| x.attr("w:val")).map(|s| s.to_string());
    p.alias = val(pr.child("w:alias"));
    p.tag = val(pr.child("w:tag"));
    p.lock = val(pr.child("w:lock"));
    p.placeholder = pr
        .child("w:placeholder")
        .and_then(|x| x.child("w:docPart"))
        .and_then(|x| x.attr("w:val"))
        .map(|s| s.to_string());
    for t in [
        "w:text",
        "w:richText",
        "w:dropDownList",
        "w:comboBox",
        "w:date",
        "w:checkbox",
        "w:picture",
    ] {
        if let Some(node) = pr.child(t) {
            p.sdt_type = Some(t.trim_start_matches("w:").to_string());
            for it in node.children_named("w:listItem") {
                if let Some(d) = it.attr("w:displayText").or_else(|| it.attr("w:value")) {
                    p.items.push(d.to_string());
                }
            }
            break;
        }
    }
    p
}

/// 解析表单域数据（w:ffData）
fn parse_form_field(ff: &XmlNode) -> FormField {
    let name = ff
        .child("w:name")
        .and_then(|n| n.attr("w:val"))
        .map(|s| s.to_string());
    if let Some(cb) = ff.child("w:checkBox") {
        let checked = cb
            .child("w:checked")
            .and_then(|c| c.attr("w:val"))
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        let default = cb
            .child("w:default")
            .and_then(|c| c.attr("w:val"))
            .map(|v| v.to_string());
        return FormField {
            kind: "checkbox".into(),
            name,
            checked: checked.or(default.as_deref().map(|d| d == "1")),
            default,
            items: Vec::new(),
        };
    }
    if let Some(dd) = ff.child("w:dropDownList") {
        let mut items = Vec::new();
        for e in dd.children_named("w:listEntry") {
            if let Some(v) = e.attr("w:val") {
                items.push(v.to_string());
            }
        }
        let default = dd
            .child("w:default")
            .and_then(|c| c.attr("w:val"))
            .map(|v| v.to_string());
        return FormField {
            kind: "dropdown".into(),
            name,
            checked: None,
            default,
            items,
        };
    }
    let default = ff
        .child("w:textInput")
        .and_then(|t| t.child("w:default"))
        .and_then(|c| c.attr("w:val"))
        .map(|v| v.to_string());
    FormField {
        kind: "text".into(),
        name,
        checked: None,
        default,
        items: Vec::new(),
    }
}

fn style_id(p: &XmlNode) -> Option<String> {
    p.child("w:pPr")
        .and_then(|ppr| ppr.child("w:pStyle"))
        .and_then(|s| s.attr("w:val"))
        .map(|s| s.to_string())
}

fn num_pr(p: &XmlNode) -> Option<(String, String)> {
    let ppr = p.child("w:pPr")?;
    let numpr = ppr.child("w:numPr")?;
    let num_id = numpr.child("w:numId")?.attr("w:val")?.to_string();
    if num_id == "0" {
        // numId=0 表示取消编号
        return None;
    }
    let ilvl = numpr
        .child("w:ilvl")
        .and_then(|n| n.attr("w:val"))
        .unwrap_or("0")
        .to_string();
    Some((num_id, ilvl))
}

fn paragraph_text_of(p: &XmlNode) -> String {
    let mut s = String::new();
    for c in &p.children {
        if c.name != "w:pPr" {
            collect_text(c, &mut s);
        }
    }
    s
}

/// 递归提取任意层级的文本（穿透 sdt/fldSimple/smartTag/文本框等包装）
fn collect_text(n: &XmlNode, s: &mut String) {
    match n.name.as_str() {
        "w:t" | "w:delText" => s.push_str(&n.text_content()),
        "w:br" | "w:cr" => s.push('\n'),
        "w:tab" => s.push('\t'),
        "w:rt" => {}
        _ => {
            for c in &n.children {
                collect_text(c, s);
            }
        }
    }
}

fn run_text(r: &XmlNode) -> String {
    let mut s = String::new();
    for c in &r.children {
        collect_text(c, &mut s);
    }
    s
}

/// 递归穿透包装元素（sdt / fldSimple / smartTag / customXml / textbox）收集 run
fn append_wrapped_runs(
    n: &XmlNode,
    ctx: &PCtx,
    runs: &mut Vec<Run>,
    pending_bookmark: &mut Option<String>,
    comment: Option<&str>,
) {
    match n.name.as_str() {
        "w:r" => {
            if let Some(mut run) = parse_run(n, ctx.rels) {
                if let Some(bm) = pending_bookmark.take() {
                    run.bookmark = Some(bm);
                }
                if let Some(cid) = comment {
                    if let Some((author, text)) = ctx.comments.get(cid) {
                        run.comment = Some(text.clone());
                        run.comment_author = Some(author.clone());
                    }
                }
                runs.push(run);
            }
        }
        "w:bookmarkStart" => {
            *pending_bookmark = n.attr("w:name").map(|s| s.to_string());
        }
        // 行内内容控件：把属性附加到其内产生的 run
        "w:sdt" => {
            let props = parse_sdt_props(n);
            let start = runs.len();
            let inner: Vec<&XmlNode> = match n.child("w:sdtContent") {
                Some(sc) => sc.children.iter().collect(),
                None => n.children.iter().collect(),
            };
            for c in inner {
                append_wrapped_runs(c, ctx, runs, pending_bookmark, comment);
            }
            for r in runs[start..].iter_mut() {
                if r.sdt.is_none() {
                    r.sdt = Some(props.clone());
                }
            }
        }
        // 简单域（w:fldSimple）
        "w:fldSimple" => {
            let instr = n.attr("w:instr").map(|s| s.trim().to_string());
            let start = runs.len();
            for c in &n.children {
                append_wrapped_runs(c, ctx, runs, pending_bookmark, comment);
            }
            if let Some(ins) = instr {
                if !ins.is_empty() {
                    for r in runs[start..].iter_mut() {
                        if r.field.is_none() {
                            r.field = Some(ins.clone());
                        }
                    }
                }
            }
        }
        "w:t" | "w:delText" | "w:instrText" | "w:fldChar" => {}
        _ => {
            for c in &n.children {
                append_wrapped_runs(c, ctx, runs, pending_bookmark, comment);
            }
        }
    }
}

/// 收集段落 run（用于标题保留 run 级格式）
fn simple_runs(p: &XmlNode, ctx: &PCtx) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut bm = None;
    for c in &p.children {
        if c.name == "w:pPr" {
            continue;
        }
        append_wrapped_runs(c, ctx, &mut runs, &mut bm, None);
    }
    runs
}

fn parse_paragraph(p: &XmlNode, ctx: &PCtx) -> Block {
    let style = style_id(p);

    // 分页符
    let has_page_break = p.children_named("w:r").any(|r| {
        r.children_named("w:br")
            .any(|b| b.attr("w:type") == Some("page"))
    });
    if has_page_break && run_text_all(p).trim().is_empty() {
        return Block::PageBreak;
    }

    // 公式
    if p.child("m:oMathPara").is_some() || p.child("m:oMath").is_some() {
        let node = p
            .child("m:oMathPara")
            .and_then(|n| n.child("m:oMath"))
            .or_else(|| p.child("m:oMath"));
        let latex = node
            .map(crate::parse::math::omml_to_latex)
            .unwrap_or_default();
        return Block::Formula(FormulaBlock {
            latex,
            display: p.child("m:oMathPara").is_some(),
            number: None,
        });
    }

    // SmartArt（dgm:）→ 提取文本，避免内容丢失
    if let Some(b) = parse_smartart(p, ctx) {
        return b;
    }

    // 附件（OLE 嵌入对象）
    if let Some(ole) = find_descendant(p, "o:OLEObject") {
        if let Some(rid) = ole.attr("r:id") {
            if let Some((target, _)) = ctx.rels.target(rid) {
                let src = normalize_media_target(target);
                let name = target.rsplit('/').next().map(|s| s.to_string());
                let prog_id = ole.attr("ProgID").map(|s| s.to_string());
                let icon = find_descendant(p, "v:imagedata")
                    .and_then(|im| im.attr("r:id"))
                    .and_then(|id| ctx.rels.target(id))
                    .map(|(t, _)| normalize_media_target(t));
                // 预览尺寸取自 v:shape 的 style（width/height）
                let mut width = None;
                let mut height = None;
                if let Some(sh) = find_descendant(p, "v:shape") {
                    if let Some(style) = sh.attr("style") {
                        let get = |k: &str| -> Option<f64> {
                            style
                                .split(';')
                                .map(|s| s.trim())
                                .find(|s| s.starts_with(k))
                                .and_then(|s| s.split_once(':').map(|x| x.1))
                                .and_then(|v| parse_vml_len(v.trim()))
                        };
                        width = get("width");
                        height = get("height");
                    }
                }
                return Block::Attachment(AttachmentBlock {
                    src,
                    name,
                    prog_id,
                    icon,
                    width,
                    height,
                    label: None,
                });
            }
        }
    }

    // VML 文本框（w:pict → v:shape → v:textbox）
    if let Some(tb) = parse_vml_textbox(p) {
        return Block::TextBox(tb);
    }
    // VML 图片（w:pict → v:shape → v:imagedata）
    if let Some(img) = parse_vml_image(p, ctx.rels) {
        return Block::Image(img);
    }
    // VML 自选图形/线条（非文本框）
    if let Some(sh) = parse_vml_shape(p) {
        return Block::Shape(sh);
    }

    // 文本框
    if let Some(tb) = parse_textbox(p) {
        return Block::TextBox(tb);
    }
    // DrawingML 形状（wps，非文本框）
    if let Some(sh) = parse_wps_shape(p) {
        return Block::Shape(sh);
    }

    // 图表
    if let Some(chart) = parse_chart(p, ctx.rels, ctx.files) {
        return Block::Chart(chart);
    }

    // 图片（仅当段落无文本时才提升为图片块；含文本的行内图片由 run.image 承载）
    if paragraph_text_of(p).trim().is_empty() {
        if let Some(img) = parse_image(p, ctx.rels) {
            return Block::Image(img);
        }
    }

    // 其他未建模 drawing（墨迹/画布/3D 等）：原样保留
    if let Some(b) = parse_unmodeled_drawing(p, ctx) {
        return b;
    }

    // 目录
    if is_toc(p) {
        return Block::Toc(TocBlock {
            title: "目录".to_string(),
            levels: Some(vec![1, 2, 3]),
            is_static: None,
            entries: Vec::new(),
        });
    }

    // 标题
    if let Some(s) = &style {
        if let Some(level) = heading_level(s) {
            let runs = simple_runs(p, ctx);
            let has_fmt = runs.iter().any(|r| !r.is_plain());
            return Block::Heading {
                level,
                text: paragraph_text_of(p),
                numbering: None,
                align: p
                    .child("w:pPr")
                    .and_then(|ppr| ppr.child("w:jc"))
                    .and_then(|j| j.attr("w:val"))
                    .filter(|v| *v != "left")
                    .map(|v| {
                        if v == "both" {
                            "justify".to_string()
                        } else {
                            v.to_string()
                        }
                    }),
                runs: if has_fmt { Some(runs) } else { None },
            };
        }
        if s == "Caption" {
            return Block::Caption(CaptionBlock {
                text: paragraph_text_of(p),
                of: None,
            });
        }
        if s == "Quote" {
            return Block::Quote(QuoteBlock {
                text: paragraph_text_of(p),
            });
        }
    }

    // 普通段落：还原 runs（含书签、脚注、REF/PAGEREF 域）
    let mut runs: Vec<Run> = Vec::new();
    let mut pending_bookmark: Option<String> = None;
    let mut active_comment: Option<String> = None;
    let mut field_instr: Option<String> = None;
    let mut field_cached = String::new();
    let mut pending_ff: Option<FormField> = None;
    for c in &p.children {
        match c.name.as_str() {
            "w:bookmarkStart" => {
                pending_bookmark = c.attr("w:name").map(|s| s.to_string());
            }
            "w:bookmarkEnd" => {}
            "w:commentRangeStart" => {
                active_comment = c.attr("w:id").map(|s| s.to_string());
            }
            "w:commentRangeEnd" => {
                active_comment = None;
            }
            "w:ins" | "w:del" => {
                let (rev, default_author) = if c.name == "w:ins" {
                    ("ins", "json2docx")
                } else {
                    ("del", "json2docx")
                };
                let author = c.attr("w:author").unwrap_or(default_author).to_string();
                for rc in &c.children {
                    match rc.name.as_str() {
                        "w:r" => {
                            if let Some(mut run) = parse_run(rc, ctx.rels) {
                                run.revision = Some(rev.to_string());
                                run.revision_author = Some(author.clone());
                                runs.push(run);
                            }
                        }
                        "w:hyperlink" => {
                            let url = rc
                                .attr("r:id")
                                .and_then(|id| ctx.rels.target(id))
                                .map(|(t, _)| t.to_string())
                                .or_else(|| rc.attr("w:anchor").map(|a| format!("#{a}")));
                            for r2 in rc.children_named("w:r") {
                                if let Some(mut run) = parse_run(r2, ctx.rels) {
                                    run.hyperlink = url.clone();
                                    normalize_link_color(&mut run);
                                    run.revision = Some(rev.to_string());
                                    run.revision_author = Some(author.clone());
                                    runs.push(run);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            "w:r" => {
                // 脚注引用
                if let Some(fr) = find_descendant(c, "w:footnoteReference") {
                    if let Some(id) = fr.attr("w:id") {
                        if let Some(note) = ctx.notes.get(id) {
                            if let Some(last) = runs.last_mut() {
                                last.footnote = Some(note.clone());
                            } else {
                                let mut run = Run::plain("");
                                run.footnote = Some(note.clone());
                                runs.push(run);
                            }
                        }
                    }
                    continue;
                }
                // 尾注引用
                if let Some(er) = find_descendant(c, "w:endnoteReference") {
                    if let Some(id) = er.attr("w:id") {
                        if let Some(note) = ctx.enotes.get(id) {
                            if let Some(last) = runs.last_mut() {
                                last.endnote = Some(note.clone());
                            } else {
                                let mut run = Run::plain("");
                                run.endnote = Some(note.clone());
                                runs.push(run);
                            }
                        }
                    }
                    continue;
                }
                // 域处理
                if find_descendant(c, "w:fldChar").is_some() {
                    let fld = find_descendant(c, "w:fldChar").unwrap();
                    match fld.attr("w:fldCharType") {
                        Some("begin") => {
                            field_instr = Some(String::new());
                            field_cached.clear();
                            pending_ff = find_descendant(c, "w:ffData").map(parse_form_field);
                        }
                        Some("end") => {
                            if let Some(instr) = field_instr.take() {
                                if let Some((target, pageref)) = parse_ref_instr(&instr) {
                                    let mut run = Run::plain(field_cached.trim());
                                    run.ref_target = Some(target);
                                    if pageref {
                                        run.pageref = Some(true);
                                    }
                                    runs.push(run);
                                } else {
                                    // 其他域（IF/DOCPROPERTY/STYLEREF/MERGEFIELD/DATE/表单域等）
                                    let mut run = Run::plain(field_cached.trim());
                                    let ins = instr.trim();
                                    if !ins.is_empty() {
                                        run.field = Some(ins.to_string());
                                    }
                                    if let Some(ff) = pending_ff.take() {
                                        run.form_field = Some(ff);
                                    }
                                    runs.push(run);
                                }
                            }
                            field_cached.clear();
                        }
                        _ => {}
                    }
                    continue;
                }
                if let Some(it) = find_descendant(c, "w:instrText") {
                    if let Some(instr) = &mut field_instr {
                        instr.push_str(&it.text_content());
                    }
                    continue;
                }
                if field_instr.is_some() {
                    // 域结果缓存
                    field_cached.push_str(&run_text(c));
                    continue;
                }
                if let Some(run) = parse_run(c, ctx.rels) {
                    let mut run = run;
                    if let Some(bm) = pending_bookmark.take() {
                        run.bookmark = Some(bm);
                    }
                    if let Some(cid) = &active_comment {
                        if let Some((author, text)) = ctx.comments.get(cid) {
                            run.comment = Some(text.clone());
                            run.comment_author = Some(author.clone());
                        }
                    }
                    runs.push(run);
                }
            }
            "w:hyperlink" => {
                let url = c
                    .attr("r:id")
                    .and_then(|id| ctx.rels.target(id))
                    .map(|(t, _)| t.to_string())
                    .or_else(|| c.attr("w:anchor").map(|a| format!("#{a}")));
                for r in c.children_named("w:r") {
                    if let Some(mut run) = parse_run(r, ctx.rels) {
                        run.hyperlink = url.clone();
                        normalize_link_color(&mut run);
                        runs.push(run);
                    }
                }
            }
            // 内容控件 / 域 / 智能标记 / 文本框等包装：递归提取其中的 run
            _ => {
                append_wrapped_runs(
                    c,
                    ctx,
                    &mut runs,
                    &mut pending_bookmark,
                    active_comment.as_deref(),
                );
            }
        }
    }

    let mut para = Paragraph::default();
    parse_paragraph_props(p, &mut para);
    para.para_id = p.attr("w14:paraId").map(|s| s.to_string());
    // 保留段落样式（仅当该样式会被输出，避免悬空引用）
    para.style = style_id(p).filter(|s| ctx.styles.contains_key(s));
    if runs.len() == 1 {
        let run = runs.into_iter().next().unwrap();
        let special = !run_promotable(&run);
        if run.is_plain() {
            para.text = Some(run.text);
        } else if !special {
            // 单 run 带格式 → 提升到段落级
            para.text = Some(run.text);
            para.bold = run.bold;
            para.italic = run.italic;
            para.font_size = run.font_size;
            para.color = run.color;
            para.font_family = run.font_family;
        } else {
            para.runs = Some(vec![run]);
        }
    } else if !runs.is_empty() {
        // 保留有内容或特殊元素的 run
        let kept: Vec<Run> = runs
            .into_iter()
            .filter(|r| {
                !r.text.is_empty()
                    || r.footnote.is_some()
                    || r.endnote.is_some()
                    || r.comment.is_some()
                    || r.revision.is_some()
                    || r.bookmark.is_some()
                    || r.ref_target.is_some()
                    || r.symbol.is_some()
                    || r.image.is_some()
                    || r.field.is_some()
                    || r.form_field.is_some()
                    || r.lang.is_some()
                    || r.rtl.is_some()
            })
            .collect();
        if kept.len() == 1 {
            let run = kept.into_iter().next().unwrap();
            if run.is_plain() {
                para.text = Some(run.text);
            } else {
                para.runs = Some(vec![run]);
            }
        } else {
            para.runs = Some(kept);
        }
    } else {
        para.text = Some(String::new());
    }
    Block::Paragraph(para)
}

/// 解析 REF/PAGEREF 域指令，返回 (书签名, 是否 PAGEREF)
fn parse_ref_instr(instr: &str) -> Option<(String, bool)> {
    let trimmed = instr.trim();
    let (pageref, rest) = if let Some(r) = trimmed.strip_prefix("PAGEREF") {
        (true, r)
    } else if let Some(r) = trimmed.strip_prefix("REF") {
        (false, r)
    } else {
        return None;
    };
    let name = rest.split_whitespace().next()?.to_string();
    if name.is_empty() {
        None
    } else {
        Some((name, pageref))
    }
}

fn run_text_all(p: &XmlNode) -> String {
    p.children_named("w:r")
        .map(run_text)
        .collect::<Vec<_>>()
        .join("")
}

/// 单 run 是否只含可映射到段落级的格式（可安全提升为段落 text）
fn run_promotable(r: &Run) -> bool {
    r.underline.is_none()
        && r.strike.is_none()
        && r.highlight.is_none()
        && r.shading.is_none()
        && r.superscript.is_none()
        && r.subscript.is_none()
        && r.code.is_none()
        && r.hyperlink.is_none()
        && r.footnote.is_none()
        && r.endnote.is_none()
        && r.comment.is_none()
        && r.revision.is_none()
        && r.bookmark.is_none()
        && r.ref_target.is_none()
        && r.symbol.is_none()
        && r.image.is_none()
        && r.sdt.is_none()
        && r.field.is_none()
        && r.lang.is_none()
        && r.lang_ea.is_none()
        && r.rtl.is_none()
        && r.form_field.is_none()
        && r.east_asia_font.is_none()
        && r.cs_font.is_none()
        && r.caps.is_none()
        && r.small_caps.is_none()
        && r.spacing.is_none()
        && r.ruby.is_none()
        && r.outline.is_none()
        && r.shadow.is_none()
        && r.emboss.is_none()
        && r.imprint.is_none()
}

/// 读取段落级属性（对齐/缩进/间距）
fn parse_paragraph_props(p: &XmlNode, para: &mut Paragraph) {
    let Some(ppr) = p.child("w:pPr") else {
        return;
    };
    if let Some(jc) = ppr.child("w:jc").and_then(|j| j.attr("w:val")) {
        if jc != "left" {
            para.align = Some(if jc == "both" {
                "justify".to_string()
            } else {
                jc.to_string()
            });
        }
    }
    if let Some(ind) = ppr.child("w:ind") {
        if let Some(v) = ind.attr("w:firstLine").and_then(|v| v.parse::<i64>().ok()) {
            if v != 0 {
                para.first_line_indent = Some(twip_to_pt(v));
            }
        }
        if let Some(v) = ind.attr("w:left").and_then(|v| v.parse::<i64>().ok()) {
            if v != 0 {
                para.indent = Some(twip_to_pt(v));
            }
        }
        if let Some(v) = ind.attr("w:hanging").and_then(|v| v.parse::<i64>().ok()) {
            if v != 0 {
                para.hanging_indent = Some(twip_to_pt(v));
            }
        }
    }
    if let Some(sp) = ppr.child("w:spacing") {
        // 显式 0 也要保留（避免继承 docDefaults 的非零段距）
        if let Some(v) = sp.attr("w:before").and_then(|v| v.parse::<i64>().ok()) {
            para.space_before = Some(twip_to_pt(v));
        }
        if let Some(v) = sp.attr("w:after").and_then(|v| v.parse::<i64>().ok()) {
            para.space_after = Some(twip_to_pt(v));
        }
        if sp
            .attr("w:beforeAutospacing")
            .map(|v| v != "0")
            .unwrap_or(false)
        {
            para.before_autospacing = Some(true);
        }
        if sp
            .attr("w:afterAutospacing")
            .map(|v| v != "0")
            .unwrap_or(false)
        {
            para.after_autospacing = Some(true);
        }
        if let Some(v) = sp.attr("w:line").and_then(|v| v.parse::<f64>().ok()) {
            if v != 0.0 {
                let rule = sp.attr("w:lineRule").unwrap_or("auto");
                if rule == "auto" {
                    para.line_spacing = Some(v / LINE_SINGLE);
                } else {
                    para.line_pt = Some(twip_to_pt(v as i64));
                    para.line_rule = Some(rule.to_string());
                }
            }
        }
    }
    if let Some(v) = ppr.child("w:autoSpaceDE").and_then(|n| n.attr("w:val")) {
        para.auto_space_de = Some(v != "0");
    }
    if let Some(v) = ppr.child("w:autoSpaceDN").and_then(|n| n.attr("w:val")) {
        para.auto_space_dn = Some(v != "0");
    }
    if let Some(v) = ppr.child("w:adjustRightInd").and_then(|n| n.attr("w:val")) {
        para.adjust_right_ind = Some(v != "0");
    }
    if let Some(v) = ppr.child("w:snapToGrid").and_then(|n| n.attr("w:val")) {
        para.snap_to_grid = Some(v != "0");
    }
    if ppr.child("w:bidi").is_some() {
        para.rtl = Some(true);
    }
    if ppr.child("w:keepNext").is_some() {
        para.keep_next = Some(true);
    }
    if ppr.child("w:keepLines").is_some() {
        para.keep_lines = Some(true);
    }
    if ppr.child("w:pageBreakBefore").is_some() {
        para.page_break_before = Some(true);
    }
    if let Some(fp) = ppr.child("w:framePr") {
        if fp.attr("w:dropCap").map(|v| v != "none").unwrap_or(false) {
            let lines = fp
                .attr("w:lines")
                .and_then(|v| v.parse::<u8>().ok())
                .unwrap_or(2);
            if lines > 0 {
                para.drop_cap = Some(lines);
            }
        }
    }
    if let Some(fill) = ppr
        .child("w:shd")
        .and_then(|s| s.attr("w:fill"))
        .filter(|f| *f != "auto")
    {
        para.shading = Some(fill.to_string());
    }
    if let Some(color) = ppr
        .child("w:pBdr")
        .and_then(|b| b.child("w:bottom"))
        .and_then(|bd| bd.attr("w:color"))
        .filter(|c| *c != "auto")
    {
        para.border = Some(color.to_string());
    }
    if let Some(tabs) = ppr.child("w:tabs") {
        let pts: Vec<crate::model::blocks::TabStop> = tabs
            .children_named("w:tab")
            .map(|t| crate::model::blocks::TabStop {
                pos: t
                    .attr("w:pos")
                    .and_then(|v| v.parse::<i64>().ok())
                    .map(twip_to_pt)
                    .unwrap_or(0.0),
                align: t
                    .attr("w:val")
                    .filter(|v| *v != "left")
                    .map(|v| v.to_string()),
                leader: t
                    .attr("w:leader")
                    .filter(|v| *v != "none")
                    .map(|v| v.to_string()),
            })
            .collect();
        if !pts.is_empty() {
            para.tabs = Some(pts);
        }
    }
}

fn parse_run(r: &XmlNode, rels: &Rels) -> Option<Run> {
    // 注音（w:ruby）：text 为基底，ruby 为注音
    if let Some(ruby) = r.child("w:ruby") {
        let base = ruby.child("w:rubyBase").map(run_text).unwrap_or_default();
        let rt = ruby.child("w:rt").map(run_text).unwrap_or_default();
        if !base.is_empty() || !rt.is_empty() {
            return Some(Run {
                text: base,
                ruby: Some(rt),
                ..Default::default()
            });
        }
    }
    let text = run_text(r);
    let rpr = r.child("w:rPr");
    let mut run = Run {
        text,
        ..Default::default()
    };
    if let Some(rpr) = rpr {
        if rpr.child("w:b").is_some() {
            run.bold = Some(true);
        }
        if rpr.child("w:i").is_some() {
            run.italic = Some(true);
        }
        if let Some(u) = rpr.child("w:u") {
            if u.attr("w:val") != Some("none") {
                run.underline = Some(true);
            }
        }
        if rpr.child("w:strike").is_some() {
            run.strike = Some(true);
        }
        if let Some(c) = rpr.child("w:color").and_then(|c| c.attr("w:val")) {
            if c != "auto" {
                run.color = Some(c.to_string());
            }
        }
        if let Some(h) = rpr.child("w:highlight").and_then(|c| c.attr("w:val")) {
            run.highlight = Some(h.to_string());
        }
        if let Some(fill) = rpr
            .child("w:shd")
            .and_then(|s| s.attr("w:fill"))
            .filter(|f| *f != "auto")
        {
            run.shading = Some(fill.to_string());
        }
        if let Some(sz) = rpr.child("w:sz").and_then(|s| s.attr("w:val")) {
            if let Ok(hp) = sz.parse::<i64>() {
                run.font_size = Some(half_to_pt(hp));
            }
        }
        if let Some(fonts) = rpr.child("w:rFonts") {
            if let Some(v) = fonts
                .attr("w:ascii")
                .or_else(|| fonts.attr("w:hAnsi"))
                .filter(|s| !s.is_empty())
            {
                run.font_family = Some(v.to_string());
            }
            if let Some(v) = fonts.attr("w:eastAsia").filter(|s| !s.is_empty()) {
                run.east_asia_font = Some(v.to_string());
            }
            if let Some(v) = fonts.attr("w:cs").filter(|s| !s.is_empty()) {
                run.cs_font = Some(v.to_string());
            }
        }
        if let Some(va) = rpr.child("w:vertAlign").and_then(|v| v.attr("w:val")) {
            if va == "superscript" {
                run.superscript = Some(true);
            } else if va == "subscript" {
                run.subscript = Some(true);
            }
        }
        if rpr.child("w:caps").is_some() {
            run.caps = Some(true);
        }
        if rpr.child("w:smallCaps").is_some() {
            run.small_caps = Some(true);
        }
        if rpr.child("w:outline").is_some() {
            run.outline = Some(true);
        }
        if rpr.child("w:shadow").is_some() {
            run.shadow = Some(true);
        }
        if rpr.child("w:emboss").is_some() {
            run.emboss = Some(true);
        }
        if rpr.child("w:imprint").is_some() {
            run.imprint = Some(true);
        }
        if rpr.child("w:rtl").is_some() {
            run.rtl = Some(true);
        }
        if let Some(l) = rpr.child("w:lang") {
            if let Some(v) = l.attr("w:val").filter(|v| !v.is_empty()) {
                run.lang = Some(v.to_string());
            }
            if let Some(v) = l.attr("w:eastAsia").filter(|v| !v.is_empty()) {
                run.lang_ea = Some(v.to_string());
            }
        }
        if let Some(sp) = rpr
            .child("w:spacing")
            .and_then(|s| s.attr("w:val"))
            .and_then(|v| v.parse::<i64>().ok())
        {
            if sp != 0 {
                run.spacing = Some(sp as f64 / 20.0);
            }
        }
    }
    // 行内图片（图标/图元随文本流）
    if let Some(img) = inline_image_from_run(r, rels) {
        run.image = Some(Box::new(img));
    }
    // 行内符号
    if let Some(sym) = r.child("w:sym") {
        let font = sym.attr("w:font").unwrap_or("").to_string();
        let ch = sym.attr("w:char").unwrap_or("").to_string();
        if !ch.is_empty() {
            run.symbol = Some(format!("{font}:{ch}"));
        }
    }
    Some(run)
}

fn parse_image(p: &XmlNode, rels: &Rels) -> Option<ImageBlock> {
    let mut drawings = Vec::new();
    find_all(p, "w:drawing", &mut drawings);
    let drawing = drawings
        .into_iter()
        .find(|d| find_descendant(d, "a:blip").is_some())
        .or_else(|| find_descendant(p, "w:drawing"))?;
    let align = p
        .child("w:pPr")
        .and_then(|ppr| ppr.child("w:jc"))
        .and_then(|jc| jc.attr("w:val"))
        .filter(|v| *v != "left")
        .map(|v| v.to_string());
    image_from_drawing(drawing, rels, align)
}

/// 行内图片（run 内的 drawing）
fn inline_image_from_run(r: &XmlNode, rels: &Rels) -> Option<ImageBlock> {
    let mut drawings = Vec::new();
    find_all(r, "w:drawing", &mut drawings);
    let drawing = drawings
        .into_iter()
        .find(|d| find_descendant(d, "a:blip").is_some())?;
    image_from_drawing(drawing, rels, None)
}

fn image_from_drawing(drawing: &XmlNode, rels: &Rels, align: Option<String>) -> Option<ImageBlock> {
    let blip = find_descendant(drawing, "a:blip")?;
    let rid = blip.attr("r:embed")?;
    let (target, _) = rels.target(rid)?;
    let src = normalize_media_target(target);
    let extent = find_descendant(drawing, "wp:extent");
    let (width, height) = extent
        .and_then(|e| {
            let cx: i64 = e.attr("cx")?.parse().ok()?;
            let cy: i64 = e.attr("cy")?.parse().ok()?;
            Some((emu_to_pt(cx), emu_to_pt(cy)))
        })
        .unwrap_or((0.0, 0.0));
    let alt = find_descendant(drawing, "wp:docPr")
        .and_then(|d| d.attr("descr"))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    // 浮动图片
    let anchor = drawing.child("wp:anchor");
    let (wrap, x, y, behind_text, align) = if let Some(a) = anchor {
        let behind = a.attr("behindDoc") == Some("1");
        let pos = |which: &str| -> Option<f64> {
            a.child(which)
                .and_then(|ph| ph.child("wp:posOffset"))
                .and_then(|o| o.text_content().trim().parse::<i64>().ok())
                .map(emu_to_pt)
        };
        let wrap = if a.child("wp:wrapNone").is_some() {
            "none"
        } else if a.child("wp:wrapTopAndBottom").is_some() {
            "topAndBottom"
        } else if a.child("wp:wrapTight").is_some() {
            "tight"
        } else if a.child("wp:wrapThrough").is_some() {
            "through"
        } else {
            "square"
        };
        (
            Some(wrap.to_string()),
            pos("wp:positionH"),
            pos("wp:positionV"),
            Some(behind),
            None,
        )
    } else {
        // 内联：居中为默认，只有非居中才显式记录
        (None, None, None, None, align)
    };

    // 裁剪与旋转
    let crop = find_descendant(drawing, "a:srcRect").and_then(|sr| {
        let g = |k: &str| {
            sr.attr(k)
                .and_then(|v| v.parse::<f64>().ok())
                .unwrap_or(0.0)
        };
        let c = [
            g("l") / 1000.0,
            g("t") / 1000.0,
            g("r") / 1000.0,
            g("b") / 1000.0,
        ];
        if c.iter().any(|v| *v != 0.0) {
            Some(c)
        } else {
            None
        }
    });
    let rotation = find_descendant(drawing, "a:xfrm")
        .and_then(|x| x.attr("rot"))
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v != 0.0)
        .map(|v| v / 60000.0);

    Some(ImageBlock {
        src,
        width: Some(width),
        height: Some(height),
        align,
        caption: None,
        alt,
        wrap,
        x,
        y,
        behind_text,
        crop,
        rotation,
    })
}

/// 其他未建模内容（未知 drawing / ActiveX 控件 / mc:AlternateContent）：保留原始 XML
fn parse_unmodeled_drawing(p: &XmlNode, ctx: &PCtx) -> Option<Block> {
    let has_drawing = find_descendant(p, "w:drawing").is_some();
    let has_control = find_descendant(p, "w:control").is_some();
    let has_alt = find_descendant(p, "mc:AlternateContent").is_some();
    if !has_drawing && !has_control && !has_alt {
        return None;
    }
    // 已知 drawing 类型（picture/chart/wordprocessingShape/diagram）由专门解析处理
    if has_drawing {
        if let Some(gd) =
            find_descendant(p, "w:drawing").and_then(|d| find_descendant(d, "a:graphicData"))
        {
            let uri = gd.attr("uri").unwrap_or("");
            let known = [
                "drawingml/2006/picture",
                "drawingml/2006/chart",
                "drawingml/2006/wordprocessingShape",
                "drawingml/2006/diagram",
            ];
            if uri.is_empty() || known.iter().any(|k| uri.contains(k)) {
                return None;
            }
        }
    }
    let rels = collect_raw_rels(p, ctx.rels);
    Some(Block::Raw(RawBlock {
        xml: p.to_xml(),
        rels,
        diagram: None,
    }))
}

/// 解析 SmartArt（dgm:）：保留原始 drawing XML + 关系，使其可原样渲染
fn parse_smartart(p: &XmlNode, ctx: &PCtx) -> Option<Block> {
    let gd = find_descendant(p, "a:graphicData")?;
    if !gd
        .attr("uri")
        .map(|u| u.contains("diagram"))
        .unwrap_or(false)
    {
        return None;
    }
    let rels = collect_raw_rels(p, ctx.rels);
    if rels.is_empty() {
        return None;
    }
    // 可编辑数据：解析 r:dm 指向的 diagram 数据部件文本
    let diagram = find_descendant(p, "dgm:relIds")
        .and_then(|ri| ri.attr("r:dm"))
        .and_then(|dm| ctx.rels.target(dm))
        .map(|(t, _)| normalize_media_target(t))
        .and_then(|part| {
            let txt = text_of(ctx.files, &part)?;
            let root = parse_xml(&txt).ok()?;
            let mut texts = Vec::new();
            collect_dgm_text(&root, &mut texts);
            Some(DiagramInfo { data: part, texts })
        });
    Some(Block::Raw(RawBlock {
        xml: p.to_xml(),
        rels,
        diagram,
    }))
}

/// 收集节点内所有 r:xxx 引用的关系
fn collect_raw_rels(p: &XmlNode, rels: &Rels) -> Vec<RawRel> {
    const KEYS: [&str; 8] = [
        "r:id", "r:embed", "r:link", "r:dm", "r:lo", "r:qs", "r:cs", "r:pict",
    ];
    let mut ids: Vec<String> = Vec::new();
    fn walk(n: &XmlNode, keys: &[&str], out: &mut Vec<String>) {
        for (k, v) in &n.attrs {
            if keys.contains(&k.as_str()) && !out.contains(v) {
                out.push(v.clone());
            }
        }
        for c in &n.children {
            walk(c, keys, out);
        }
    }
    walk(p, &KEYS, &mut ids);
    let mut out = Vec::new();
    for id in ids {
        if let Some((target, external)) = rels.target(&id) {
            out.push(RawRel {
                id: id.clone(),
                rel_type: rels.rel_type(&id).map(|s| s.to_string()),
                target: target.to_string(),
                external,
            });
        }
    }
    out
}

/// 解析 SmartArt 文本（备用：无法保留原始 XML 时提取文本）
#[allow(dead_code)]
fn parse_smartart_text(p: &XmlNode, ctx: &PCtx) -> Option<Block> {
    let gd = find_descendant(p, "a:graphicData")?;
    if !gd
        .attr("uri")
        .map(|u| u.contains("diagram"))
        .unwrap_or(false)
    {
        return None;
    }
    let rel_ids = find_descendant(p, "dgm:relIds")?;
    let dm = rel_ids.attr("r:dm")?;
    let (target, _) = ctx.rels.target(dm)?;
    let part = normalize_media_target(target);
    let txt = text_of(ctx.files, &part)?;
    let root = parse_xml(&txt).ok()?;
    let mut lines: Vec<String> = Vec::new();
    collect_dgm_text(&root, &mut lines);
    if lines.is_empty() {
        return None;
    }
    Some(Block::Paragraph(Paragraph {
        runs: Some(vec![Run::plain(lines.join("\n"))]),
        ..Default::default()
    }))
}

/// 收集绘图组节点（同类型可多组）
fn group_nodes<'a>(plot: Option<&'a XmlNode>, elems: &[&str]) -> Vec<&'a XmlNode> {
    match plot {
        Some(p) => p
            .children
            .iter()
            .filter(|c| elems.contains(&c.name.as_str()))
            .collect(),
        None => Vec::new(),
    }
}

/// 读取类别缓存（strLit/strRef->strCache/numRef->numCache）
fn read_cat_cache(cat: Option<&XmlNode>) -> Vec<String> {
    let Some(cat) = cat else { return Vec::new() };
    let pts = cat
        .child("c:strLit")
        .or_else(|| cat.child("c:strRef").and_then(|r| r.child("c:strCache")))
        .or_else(|| cat.child("c:numRef").and_then(|r| r.child("c:numCache")))
        .or_else(|| cat.child("c:numLit"));
    match pts {
        Some(n) => n
            .children_named("c:pt")
            .filter_map(|p| p.child("c:v").map(|v| v.text_content()))
            .collect(),
        None => Vec::new(),
    }
}

/// 读取数值缓存（numLit/numRef->numCache）
fn read_num_cache(val: Option<&XmlNode>) -> Vec<f64> {
    let Some(val) = val else { return Vec::new() };
    let pts = val
        .child("c:numLit")
        .or_else(|| val.child("c:numRef").and_then(|r| r.child("c:numCache")));
    match pts {
        Some(n) => n
            .children_named("c:pt")
            .filter_map(|p| {
                p.child("c:v")
                    .and_then(|v| v.text_content().trim().parse::<f64>().ok())
            })
            .collect(),
        None => Vec::new(),
    }
}

/// 递归收集 SmartArt 数据中的 `dgm:t` 文本
fn collect_dgm_text(n: &XmlNode, out: &mut Vec<String>) {
    if n.name == "dgm:t" {
        let t = n.text_content().trim().to_string();
        if !t.is_empty() {
            out.push(t);
        }
        return;
    }
    for c in &n.children {
        collect_dgm_text(c, out);
    }
}

/// 解析 VML 文本框（w:pict → v:shape → v:textbox）
fn parse_vml_textbox(p: &XmlNode) -> Option<TextBoxBlock> {
    let pict = find_descendant(p, "w:pict")?;
    let shape = pict.child("v:shape")?;
    let txbx = find_descendant(shape, "v:textbox")?;
    let content = find_descendant(txbx, "w:txbxContent")?;
    let text = content.text_content().trim().to_string();

    let style = shape.attr("style").unwrap_or("");
    let get = |k: &str| -> Option<f64> {
        style
            .split(';')
            .map(|s| s.trim())
            .find(|s| s.starts_with(k))
            .and_then(|s| s.split_once(':').map(|x| x.1))
            .map(|v| v.trim().trim_end_matches("pt").to_string())
            .and_then(|v| v.parse::<f64>().ok())
    };
    let abs = style.replace(' ', "").contains("position:absolute");
    let fill = shape
        .attr("fillcolor")
        .map(|s| s.trim_start_matches('#').to_string())
        .or_else(|| {
            find_descendant(shape, "v:fill")
                .and_then(|f| f.attr("color"))
                .map(|s| s.trim_start_matches('#').to_string())
        });
    let line = find_descendant(shape, "v:stroke")
        .and_then(|s| s.attr("color"))
        .map(|s| s.trim_start_matches('#').to_string());
    let align = find_descendant(content, "w:p")
        .and_then(|par| par.child("w:pPr"))
        .and_then(|ppr| ppr.child("w:jc"))
        .and_then(|jc| jc.attr("w:val"))
        .map(|v| {
            if v == "both" {
                "justify".to_string()
            } else {
                v.to_string()
            }
        });

    Some(TextBoxBlock {
        text,
        width: get("width"),
        height: get("height"),
        fill,
        line,
        align,
        wrap: if abs {
            Some("square".to_string())
        } else {
            None
        },
        x: if abs { get("margin-left") } else { None },
        y: if abs { get("margin-top") } else { None },
        font_size: None,
        color: None,
        vml: Some(true),
    })
}

/// 解析 VML 图片（w:pict → v:shape/v:rect → v:imagedata）
fn parse_vml_image(p: &XmlNode, rels: &Rels) -> Option<ImageBlock> {
    let pict = find_descendant(p, "w:pict")?;
    // 图片可经 v:imagedata（图片填充）或 v:fill（纹理/框架填充）的 r:id 引用
    let imagedata = find_descendant(pict, "v:imagedata");
    let rid = imagedata
        .and_then(|i| i.attr("r:id"))
        .or_else(|| find_descendant(pict, "v:fill").and_then(|f| f.attr("r:id")))?;
    let (target, _) = rels.target(rid)?;
    let src = normalize_media_target(target);
    // 尺寸/位置取自外层 shape/rect 的 style
    let shape = pict.children.iter().find(|c| {
        matches!(
            c.name.as_str(),
            "v:shape" | "v:rect" | "v:roundrect" | "v:oval"
        )
    });
    let style = shape.and_then(|s| s.attr("style")).unwrap_or("");
    let get = |k: &str| -> Option<f64> {
        style
            .split(';')
            .map(|s| s.trim())
            .find(|s| s.starts_with(k))
            .and_then(|s| s.split_once(':').map(|x| x.1))
            .map(|v| v.trim().trim_end_matches("pt").to_string())
            .and_then(|v| v.parse::<f64>().ok())
    };
    let width = get("width");
    let height = get("height");
    let abs = style.replace(' ', "").contains("position:absolute");
    let x = get("margin-left");
    let y = get("margin-top");
    let alt = imagedata
        .and_then(|i| i.attr("o:title"))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Some(ImageBlock {
        src,
        width,
        height,
        align: None,
        caption: None,
        alt,
        wrap: if abs {
            Some("square".to_string())
        } else {
            None
        },
        x: if abs { x } else { None },
        y: if abs { y } else { None },
        behind_text: None,
        crop: None,
        rotation: None,
    })
}

/// 解析 VML 自选图形/线条（非文本框）
fn parse_vml_shape(p: &XmlNode) -> Option<ShapeBlock> {
    let pict = find_descendant(p, "w:pict")?;
    if find_descendant(pict, "v:textbox").is_some() {
        return None;
    }
    let shape = pict.children.iter().find(|c| {
        matches!(
            c.name.as_str(),
            "v:rect" | "v:roundrect" | "v:oval" | "v:line" | "v:shape"
        )
    })?;
    let stype = match shape.name.as_str() {
        "v:rect" => "rect",
        "v:roundrect" => "roundRect",
        "v:oval" => "ellipse",
        "v:line" => "line",
        _ => "rect",
    }
    .to_string();

    let style = shape.attr("style").unwrap_or("");
    let get = |k: &str| -> Option<f64> {
        style
            .split(';')
            .map(|s| s.trim())
            .find(|s| s.starts_with(k))
            .and_then(|s| s.split_once(':').map(|x| x.1))
            .map(|v| v.trim().trim_end_matches("pt").to_string())
            .and_then(|v| v.parse::<f64>().ok())
    };
    let mut width = get("width");
    let mut height = get("height");
    let mut x = get("margin-left");
    let mut y = get("margin-top");
    if shape.name == "v:line" {
        let pt = |s: Option<&str>| -> Option<(f64, f64)> {
            let s = s?;
            let mut it = s.split(',');
            Some((
                it.next()?.trim().parse().ok()?,
                it.next()?.trim().parse().ok()?,
            ))
        };
        if let (Some((x1, y1)), Some((x2, y2))) = (pt(shape.attr("from")), pt(shape.attr("to"))) {
            width = Some((x2 - x1).abs().max(1.0));
            height = Some((y2 - y1).abs().max(1.0));
            x = Some(x1.min(x2));
            y = Some(y1.min(y2));
        }
    }
    let fill = shape
        .attr("fillcolor")
        .map(|s| s.trim_start_matches('#').to_string())
        .or_else(|| {
            find_descendant(shape, "v:fill")
                .and_then(|f| f.attr("color"))
                .map(|s| s.trim_start_matches('#').to_string())
        });
    let stroke = find_descendant(shape, "v:stroke");
    let line = stroke
        .and_then(|s| s.attr("color"))
        .map(|s| s.trim_start_matches('#').to_string());
    let line_width = stroke
        .and_then(|s| s.attr("weight"))
        .and_then(|v| v.trim_end_matches("pt").parse::<f64>().ok());
    let abs = style.replace(' ', "").contains("position:absolute");
    // 艺术字（WordArt）：v:shape → v:textpath string
    let text = find_descendant(shape, "v:textpath")
        .and_then(|tp| tp.attr("string"))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Some(ShapeBlock {
        shape_type: stype,
        width,
        height,
        fill,
        line,
        line_width,
        rotation: None,
        text,
        text_color: None,
        font_size: None,
        align: None,
        wrap: if abs {
            Some("square".to_string())
        } else {
            None
        },
        x: if abs { x } else { None },
        y: if abs { y } else { None },
    })
}

/// 解析 DrawingML 预设形状（wps，非文本框）
fn parse_wps_shape(p: &XmlNode) -> Option<ShapeBlock> {
    let wsp = find_descendant(p, "wps:wsp")?;
    if find_descendant(wsp, "wps:txbx").is_some() {
        return None;
    }
    let sp_pr = wsp.child("wps:spPr")?;
    let prst = sp_pr
        .child("a:prstGeom")
        .and_then(|g| g.attr("prst"))
        .unwrap_or("rect")
        .to_string();
    let drawing = find_descendant(p, "w:drawing")?;
    let (width, height) = find_descendant(drawing, "wp:extent")
        .and_then(|e| {
            let cx: i64 = e.attr("cx")?.parse().ok()?;
            let cy: i64 = e.attr("cy")?.parse().ok()?;
            Some((emu_to_pt(cx), emu_to_pt(cy)))
        })
        .unwrap_or((120.0, 80.0));
    let rot = sp_pr
        .child("a:xfrm")
        .and_then(|x| x.attr("rot"))
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v / 60000.0);
    let fill = sp_pr
        .child("a:solidFill")
        .and_then(|f| f.child("a:srgbClr"))
        .and_then(|c| c.attr("val"))
        .map(|s| s.to_string());
    let ln = sp_pr.child("a:ln");
    let line = ln
        .and_then(|l| l.child("a:solidFill"))
        .and_then(|f| f.child("a:srgbClr"))
        .and_then(|c| c.attr("val"))
        .map(|s| s.to_string());
    let line_width = ln
        .and_then(|l| l.attr("w"))
        .and_then(|v| v.parse::<i64>().ok())
        .map(emu_to_pt);
    let (x, y, wrap) = if let Some(a) = drawing.child("wp:anchor") {
        let pos = |which: &str| -> Option<f64> {
            a.child(which)
                .and_then(|ph| ph.child("wp:posOffset"))
                .and_then(|o| o.text_content().trim().parse::<i64>().ok())
                .map(emu_to_pt)
        };
        (
            pos("wp:positionH"),
            pos("wp:positionV"),
            Some("square".to_string()),
        )
    } else {
        (None, None, None)
    };
    Some(ShapeBlock {
        shape_type: prst,
        width: Some(width),
        height: Some(height),
        fill,
        line,
        line_width,
        rotation: rot,
        text: None,
        text_color: None,
        font_size: None,
        align: None,
        wrap,
        x,
        y,
    })
}

/// 解析文本框（wps:txbx）
fn parse_textbox(p: &XmlNode) -> Option<TextBoxBlock> {
    let drawing = p.child("w:r")?.child("w:drawing")?;
    let txbx = find_descendant(drawing, "wps:txbx")?;
    let text = txbx.text_content().trim().to_string();
    let extent = find_descendant(drawing, "wp:extent");
    let (width, height) = extent
        .and_then(|e| {
            let cx: i64 = e.attr("cx")?.parse().ok()?;
            let cy: i64 = e.attr("cy")?.parse().ok()?;
            Some((emu_to_pt(cx), emu_to_pt(cy)))
        })
        .unwrap_or((200.0, 60.0));
    let sp_pr = find_descendant(drawing, "wps:spPr");
    let fill = sp_pr
        .and_then(|s| s.child("a:solidFill"))
        .and_then(|f| f.child("a:srgbClr"))
        .and_then(|c| c.attr("val"))
        .map(|v| v.to_string());
    let line = sp_pr
        .and_then(|s| s.child("a:ln"))
        .and_then(|l| l.child("a:solidFill"))
        .and_then(|f| f.child("a:srgbClr"))
        .and_then(|c| c.attr("val"))
        .map(|v| v.to_string());
    let align = find_descendant(txbx, "w:p")
        .and_then(|par| par.child("w:pPr"))
        .and_then(|ppr| ppr.child("w:jc"))
        .and_then(|jc| jc.attr("w:val"))
        .filter(|v| *v != "left")
        .map(|v| v.to_string());
    let rpr = find_descendant(txbx, "w:rPr");
    let font_size = rpr
        .and_then(|r| r.child("w:sz"))
        .and_then(|s| s.attr("w:val"))
        .and_then(|v| v.parse::<i64>().ok())
        .map(half_to_pt);
    let color = rpr
        .and_then(|r| r.child("w:color"))
        .and_then(|c| c.attr("w:val"))
        .filter(|v| *v != "auto")
        .map(|v| v.to_string());
    let (wrap, x, y) = if let Some(a) = drawing.child("wp:anchor") {
        let pos = |which: &str| -> Option<f64> {
            a.child(which)
                .and_then(|ph| ph.child("wp:posOffset"))
                .and_then(|o| o.text_content().trim().parse::<i64>().ok())
                .map(emu_to_pt)
        };
        (
            Some("square".to_string()),
            pos("wp:positionH"),
            pos("wp:positionV"),
        )
    } else {
        (None, None, None)
    };
    Some(TextBoxBlock {
        text,
        width: Some(width),
        height: Some(height),
        fill,
        line,
        align,
        wrap,
        x,
        y,
        font_size,
        color,
        vml: None,
    })
}

/// 解析图表元素（内联 drawing → c:chart → chart part）
fn parse_chart(p: &XmlNode, rels: &Rels, files: &Package) -> Option<ChartBlock> {
    let drawing = p.child("w:r")?.child("w:drawing")?;
    let c = find_descendant(drawing, "c:chart")?;
    let rid = c.attr("r:id")?;
    let (target, _) = rels.target(rid)?;
    let xml = text_of(files, &format!("word/{target}"))?;
    let root = parse_xml(&xml).ok()?;
    let extent = find_descendant(drawing, "wp:extent");
    let (width, height) = extent
        .and_then(|e| {
            let cx: i64 = e.attr("cx")?.parse().ok()?;
            let cy: i64 = e.attr("cy")?.parse().ok()?;
            Some((emu_to_pt(cx), emu_to_pt(cy)))
        })
        .unwrap_or((360.0, 200.0));
    let align = p
        .child("w:pPr")
        .and_then(|ppr| ppr.child("w:jc"))
        .and_then(|jc| jc.attr("w:val"))
        .filter(|v| *v != "center")
        .map(|v| v.to_string());
    Some(build_chart_block(&root, width, height, align))
}

fn build_chart_block(root: &XmlNode, width: f64, height: f64, align: Option<String>) -> ChartBlock {
    let chart = root.child("c:chart");
    let plot = chart.and_then(|c| c.child("c:plotArea"));
    // 选择主图类型，并收集**所有同类型**绘图组（合并多组系列）
    let (chart_type, group_elems): (String, Vec<&XmlNode>) = {
        if let Some(n) = plot.and_then(|p| p.child("c:barChart")) {
            let dir = n
                .child("c:barDir")
                .and_then(|d| d.attr("val"))
                .unwrap_or("col");
            (
                if dir == "bar" { "bar" } else { "column" }.to_string(),
                group_nodes(plot, &["c:barChart"]),
            )
        } else {
            let pick = |ty: &str, elems: &[&str]| -> (String, Vec<&XmlNode>) {
                let g = group_nodes(plot, elems);
                if g.is_empty() {
                    (String::new(), Vec::new())
                } else {
                    (ty.to_string(), g)
                }
            };
            let mut r = pick("line", &["c:lineChart"]);
            if r.0.is_empty() {
                r = pick("area", &["c:areaChart"]);
            }
            if r.0.is_empty() {
                r = pick("pie", &["c:pieChart"]);
            }
            if r.0.is_empty() {
                r = pick("doughnut", &["c:doughnutChart"]);
            }
            if r.0.is_empty() {
                ("column".to_string(), Vec::new())
            } else {
                r
            }
        }
    };
    let chart_node = group_elems.first().copied();

    let grouping = chart_node
        .and_then(|n| n.child("c:grouping"))
        .and_then(|g| g.attr("val"))
        .map(|v| v.to_string());
    let title = chart
        .and_then(|c| c.child("c:title"))
        .map(|t| t.text_content().trim().to_string())
        .filter(|s| !s.is_empty());

    let mut categories: Vec<String> = Vec::new();
    let mut series: Vec<ChartSeries> = Vec::new();
    for node in &group_elems {
        for ser in node.children_named("c:ser") {
            let name = ser
                .child("c:tx")
                .and_then(|tx| tx.child("c:v"))
                .map(|v| v.text_content())
                .or_else(|| {
                    ser.child("c:tx")
                        .and_then(|tx| tx.child("c:strRef"))
                        .and_then(|r| r.child("c:strCache"))
                        .and_then(|c| c.children_named("c:pt").next())
                        .and_then(|p| p.child("c:v"))
                        .map(|v| v.text_content())
                })
                .unwrap_or_default();
            if categories.is_empty() {
                categories = read_cat_cache(ser.child("c:cat"));
            }
            let values: Vec<f64> = read_num_cache(ser.child("c:val"));
            let color = ser
                .child("c:spPr")
                .and_then(|sp| sp.child("a:solidFill"))
                .and_then(|f| f.child("a:srgbClr"))
                .and_then(|c| c.attr("val"))
                .map(|v| v.to_string());
            series.push(ChartSeries {
                name,
                values,
                color,
            });
        }
    }

    let legend = match chart.and_then(|c| c.child("c:legend")) {
        Some(l) => Some(
            l.child("c:legendPos")
                .and_then(|p| p.attr("val"))
                .unwrap_or("b")
                .to_string(),
        ),
        None => Some("none".to_string()),
    };
    let axis_title = |ax: &str| -> Option<String> {
        plot.and_then(|p| p.child(ax))
            .and_then(|a| a.child("c:title"))
            .map(|t| t.text_content().trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let x_title = axis_title("c:catAx");
    let y_title = axis_title("c:valAx");
    let (y_min, y_max) = plot
        .and_then(|p| p.child("c:valAx"))
        .and_then(|a| a.child("c:scaling"))
        .map(|sc| {
            let g = |k: &str| {
                sc.child(k)
                    .and_then(|n| n.attr("val"))
                    .and_then(|v| v.parse::<f64>().ok())
            };
            (g("c:min"), g("c:max"))
        })
        .unwrap_or((None, None));
    let data_labels = chart_node
        .map(|n| {
            n.children_named("c:ser")
                .any(|s| s.child("c:dLbls").is_some())
        })
        .filter(|b| *b);

    ChartBlock {
        chart_type,
        title,
        categories,
        series,
        width: Some(width),
        height: Some(height),
        align,
        legend,
        x_title,
        y_title,
        y_min,
        y_max,
        data_labels,
        grouping,
    }
}

fn find_descendant<'a>(node: &'a XmlNode, name: &str) -> Option<&'a XmlNode> {
    if node.name == name {
        return Some(node);
    }
    for c in &node.children {
        if let Some(found) = find_descendant(c, name) {
            return Some(found);
        }
    }
    None
}

/// 收集所有后代节点（含自身以外的匹配）
fn find_all<'a>(node: &'a XmlNode, name: &str, out: &mut Vec<&'a XmlNode>) {
    if node.name == name {
        out.push(node);
        return;
    }
    for c in &node.children {
        find_all(c, name, out);
    }
}

/// 解析一个段落：无文本且同时含多种图形（VML + DrawingML 图片）时产出多个图片块
fn parse_paragraph_multi(p: &XmlNode, ctx: &PCtx) -> Vec<Block> {
    if paragraph_text_of(p).trim().is_empty() {
        let mut imgs: Vec<Block> = Vec::new();
        let mut srcs: Vec<String> = Vec::new();
        if let Some(i) = parse_vml_image(p, ctx.rels) {
            srcs.push(i.src.clone());
            imgs.push(Block::Image(i));
        }
        if let Some(d) = parse_image(p, ctx.rels) {
            if !srcs.contains(&d.src) {
                imgs.push(Block::Image(d));
            }
        }
        if imgs.len() >= 2 {
            return imgs;
        }
    }
    vec![parse_paragraph(p, ctx)]
}

/// 目录样式名 → 级别（TOC1/TOC2/TOC3）
fn toc_level(style: &str) -> Option<u8> {
    let s = style.to_ascii_lowercase();
    if let Some(rest) = s.strip_prefix("toc") {
        if let Some(d) = rest.chars().next().and_then(|c| c.to_digit(10)) {
            if (1..=3).contains(&d) {
                return Some(d as u8);
            }
        }
    }
    None
}

/// 样式挂接编号 → 列表项格式
fn style_list_numbering(
    ctx: &PCtx,
    style: Option<&str>,
    ordered: bool,
) -> (Option<String>, Option<String>, Option<i64>, Option<String>) {
    if let Some(st) = style.and_then(|s| ctx.styles.get(s)) {
        if let Some(nid) = &st.num_id {
            return custom_numbering(ctx.numbering.get(nid), 0, ordered);
        }
    }
    (None, None, None, None)
}

/// 样式挂接的列表种类：Some(true)=有序，Some(false)=无序
fn style_list_kind(style: Option<&str>) -> Option<bool> {
    let s = style?.to_ascii_lowercase();
    if s.starts_with("listnumber") || s.starts_with("list number") {
        Some(true)
    } else if s.starts_with("listbullet") || s.starts_with("list bullet") {
        Some(false)
    } else {
        None
    }
}

fn is_toc(p: &XmlNode) -> bool {
    // 仅将带 live TOC 域的段落识别为目录；静态目录行（TOC1 样式）作为普通段落保留其文本
    p.children.iter().any(|c| {
        find_descendant(c, "w:instrText")
            .map(|n| n.text_content().contains("TOC"))
            .unwrap_or(false)
    })
}

fn heading_level(style: &str) -> Option<u8> {
    let s = style.strip_prefix("Heading")?;
    s.parse::<u8>().ok()
}

/// 展开表格行（穿透 sdt / 修订 / 自定义 XML 包装）
fn table_rows(t: &XmlNode) -> Vec<&XmlNode> {
    let mut out = Vec::new();
    for c in &t.children {
        match c.name.as_str() {
            "w:tr" => out.push(c),
            "w:sdt" | "w:ins" | "w:del" | "w:customXml" => {
                let inner: Vec<&XmlNode> = match c.child("w:sdtContent") {
                    Some(sc) => sc.children.iter().collect(),
                    None => c.children.iter().collect(),
                };
                for x in inner {
                    if x.name == "w:tr" {
                        out.push(x);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// 展开表格单元格（穿透 sdt / 修订 包装）
fn row_cells(tr: &XmlNode) -> Vec<&XmlNode> {
    let mut out = Vec::new();
    for c in &tr.children {
        match c.name.as_str() {
            "w:tc" => out.push(c),
            "w:sdt" | "w:ins" | "w:del" | "w:customXml" => {
                let inner: Vec<&XmlNode> = match c.child("w:sdtContent") {
                    Some(sc) => sc.children.iter().collect(),
                    None => c.children.iter().collect(),
                };
                for x in inner {
                    if x.name == "w:tc" {
                        out.push(x);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn parse_table(t: &XmlNode, ctx: &PCtx) -> Block {
    let mut rows: Vec<TableRow> = Vec::new();
    let mut first = true;
    let mut header_row = false;
    // 列 → 当前纵向合并的 (行下标, 该行 cells 下标)
    let mut vmerge_owner: Vec<Option<(usize, usize)>> = Vec::new();

    for tr in table_rows(t) {
        let mut cells: Vec<TableCell> = Vec::new();
        let mut col = 0usize;
        for tc in row_cells(tr) {
            let tcpr = tc.child("w:tcPr");
            let colspan = tcpr
                .and_then(|p| p.child("w:gridSpan"))
                .and_then(|g| g.attr("w:val"))
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(1)
                .max(1) as usize;
            let vmerge = tcpr.and_then(|p| p.child("w:vMerge"));
            let is_continue = vmerge
                .map(|v| v.attr("w:val") != Some("restart"))
                .unwrap_or(false);
            let is_restart = vmerge
                .map(|v| v.attr("w:val") == Some("restart"))
                .unwrap_or(false);

            // 扩展 owner 表
            while vmerge_owner.len() < col + colspan {
                vmerge_owner.push(None);
            }

            if is_continue {
                // 归入上方 origin，行跨 +1
                for &(orow, ocell) in vmerge_owner[col..col + colspan].iter().flatten() {
                    let span = rows[orow].cells[ocell].rowspan.unwrap_or(1) + 1;
                    rows[orow].cells[ocell].rowspan = Some(span);
                }
                col += colspan;
                continue;
            }

            let fill = tcpr
                .and_then(|p| p.child("w:shd"))
                .and_then(|s| s.attr("w:fill"))
                .filter(|f| *f != "auto")
                .map(|f| f.to_string());
            let cell_children: Vec<&XmlNode> = tc
                .children
                .iter()
                .filter(|c| c.name == "w:p" || c.name == "w:tbl")
                .collect();
            let blocks = parse_blocks(&cell_children, ctx);
            let cell = TableCell {
                blocks,
                colspan: if colspan > 1 {
                    Some(colspan as u32)
                } else {
                    None
                },
                rowspan: if is_restart { Some(1) } else { None },
                fill,
                valign: tcpr
                    .and_then(|p| p.child("w:vAlign"))
                    .and_then(|v| v.attr("w:val"))
                    .filter(|v| *v != "top")
                    .map(|v| v.to_string()),
                border: tcpr
                    .and_then(|p| p.child("w:tcBorders"))
                    .and_then(parse_border),
                margin: tcpr
                    .and_then(|p| p.child("w:tcMar"))
                    .and_then(|m| {
                        m.child("w:left")
                            .or_else(|| m.child("w:right"))
                            .or_else(|| m.child("w:top"))
                            .and_then(|e| e.attr("w:w"))
                            .and_then(|v| v.parse::<i64>().ok())
                    })
                    .map(twip_to_pt),
            };
            let idx = cells.len();
            if is_restart {
                vmerge_owner[col..col + colspan].fill(Some((rows.len(), idx)));
            } else {
                vmerge_owner[col..col + colspan].fill(None);
            }
            cells.push(cell);
            col += colspan;
        }
        // 判定表头：首行带 tblHeader
        if first {
            header_row = tr
                .child("w:trPr")
                .and_then(|p| p.child("w:tblHeader"))
                .is_some();
            first = false;
        }
        let trpr = tr.child("w:trPr");
        let theight = trpr.and_then(|p| p.child("w:trHeight"));
        let height = theight
            .and_then(|h| h.attr("w:val"))
            .and_then(|v| v.parse::<i64>().ok())
            .map(twip_to_pt)
            .filter(|h| *h > 0.0);
        let height_rule = theight
            .and_then(|h| h.attr("w:hRule"))
            .filter(|r| !r.eq_ignore_ascii_case("auto"))
            .map(|r| r.to_string());
        rows.push(TableRow {
            cells,
            height,
            height_rule,
        });
    }

    // 清理 rowspan=1 的标记（无实际合并）
    for row in &mut rows {
        for cell in &mut row.cells {
            if cell.rowspan == Some(1) {
                cell.rowspan = None;
            }
        }
    }

    // 列宽：全 0 或等宽视为未显式指定（生成器默认等分铺满）
    let raw: Vec<i64> = t
        .child("w:tblGrid")
        .map(|g| {
            g.children_named("w:gridCol")
                .filter_map(|c| c.attr("w:w").and_then(|v| v.parse::<i64>().ok()))
                .collect()
        })
        .unwrap_or_default();
    let all_equal = raw.len() > 1 && raw.iter().all(|w| *w == raw[0]);
    let widths = if raw.is_empty() || raw.iter().all(|w| *w == 0) || all_equal {
        None
    } else {
        Some(raw.iter().map(|w| twip_to_pt(*w)).collect())
    };

    let tblpr = t.child("w:tblPr");
    let align = tblpr
        .and_then(|p| p.child("w:jc"))
        .and_then(|j| j.attr("w:val"))
        .filter(|v| *v != "center")
        .map(|v| v.to_string());
    let style = tblpr
        .and_then(|p| p.child("w:tblStyle"))
        .and_then(|s| s.attr("w:val"))
        .filter(|v| *v != "TableGrid")
        .map(|v| v.to_string());
    let border = tblpr
        .and_then(|p| p.child("w:tblBorders"))
        .and_then(parse_border);
    let rtl = tblpr.and_then(|p| p.child("w:bidiVisual")).map(|_| true);
    let width_pct = tblpr
        .and_then(|p| p.child("w:tblW"))
        .filter(|w| w.attr("w:type") == Some("pct"))
        .and_then(|w| w.attr("w:w"))
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| v / 50.0);
    let layout = tblpr
        .and_then(|p| p.child("w:tblLayout"))
        .and_then(|l| l.attr("w:type"))
        .map(|v| v.to_string());
    let tblw_twip = tblpr
        .and_then(|p| p.child("w:tblW"))
        .filter(|w| w.attr("w:type").map(|t| t == "dxa").unwrap_or(true))
        .and_then(|w| w.attr("w:w"))
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0);
    let indent = tblpr
        .and_then(|p| p.child("w:tblInd"))
        .and_then(|i| i.attr("w:w"))
        .and_then(|v| v.parse::<i64>().ok())
        .map(twip_to_pt)
        .filter(|v| *v != 0.0);

    let width = tblw_twip
        .filter(|v| *v != raw.iter().sum::<i64>())
        .map(twip_to_pt);
    Block::Table(TableBlock {
        header_row,
        widths,
        rows,
        caption: None,
        font_size: None,
        align,
        style,
        border,
        indent,
        rtl,
        width_pct,
        layout,
        width,
    })
}

/// 解析边框节点（表格 `w:tblBorders` / 单元格 `w:tcBorders`）。
/// 与默认灰线（single/4/808080）一致时返回 None 以保持简洁。
fn parse_border(n: &XmlNode) -> Option<Border> {
    let edges = [
        "w:top",
        "w:left",
        "w:bottom",
        "w:right",
        "w:insideH",
        "w:insideV",
    ];
    let first = edges.iter().find_map(|e| n.child(e))?;
    let val = first.attr("w:val").unwrap_or("single");
    let sz = first.attr("w:sz").and_then(|v| v.parse::<f64>().ok());
    let color = first.attr("w:color").unwrap_or("auto");
    if val == "single" && sz == Some(4.0) && color.eq_ignore_ascii_case("808080") {
        return None;
    }
    if val == "none" || val == "nil" {
        let all_none = edges.iter().all(|e| {
            n.child(e)
                .map(|x| matches!(x.attr("w:val"), Some("none") | Some("nil")))
                .unwrap_or(true)
        });
        return if all_none {
            Some(Border {
                style: Some("none".to_string()),
                ..Default::default()
            })
        } else {
            None
        };
    }
    Some(Border {
        style: Some(val.to_string()),
        color: if color.eq_ignore_ascii_case("auto") {
            None
        } else {
            Some(color.trim_start_matches('#').to_string())
        },
        width: sz.map(|s| s / 8.0),
    })
}

// ---------- 页面/样式/主题/元数据 ----------

fn parse_sect_pr(sect: &XmlNode) -> PageSetup {
    let mut page = PageSetup::default();
    if sect.child("w:pgSz").is_none() {
        // 无显式页面尺寸：标记为未指定（生成时省略 pgSz，保持与原文档一致）
        page.size = "Letter".to_string();
        page.size_specified = Some(false);
    }
    if let Some(pgsz) = sect.child("w:pgSz") {
        if let Some(w) = pgsz.attr("w:w").and_then(|v| v.parse::<i64>().ok()) {
            let h = pgsz
                .attr("w:h")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(w);
            // 标准纸张（twip 精确匹配，纵横皆可）
            const NAMED: &[(&str, i64, i64)] = &[
                ("A4", 11906, 16838),
                ("A3", 16838, 23811),
                ("Letter", 12240, 15840),
            ];
            let mut matched = false;
            for (name, pw, ph) in NAMED {
                if w == *pw && h == *ph {
                    page.size = name.to_string();
                    matched = true;
                    break;
                } else if w == *ph && h == *pw {
                    page.size = name.to_string();
                    page.orientation = "landscape".to_string();
                    matched = true;
                    break;
                }
            }
            if !matched {
                // 保留精确尺寸，避免归一化漂移
                page.size = "custom".to_string();
                page.width = Some(twip_to_pt(w));
                page.height = Some(twip_to_pt(h));
                page.orientation = if w > h { "landscape" } else { "portrait" }.to_string();
            }
            if pgsz.attr("w:orient") == Some("landscape") {
                page.orientation = "landscape".to_string();
            }
        }
    }
    if let Some(m) = sect.child("w:pgMar") {
        let get = |k: &str| {
            m.attr(k)
                .and_then(|v| v.parse::<i64>().ok())
                .map(twip_to_pt)
        };
        page.gutter = get("w:gutter").filter(|v| *v != 0.0);
        page.margins = Margins {
            top: get("w:top").unwrap_or(72.0),
            bottom: get("w:bottom").unwrap_or(72.0),
            left: get("w:left").unwrap_or(90.0),
            right: get("w:right").unwrap_or(90.0),
            header: get("w:header").unwrap_or(42.0),
            footer: get("w:footer").unwrap_or(42.0),
        };
    }
    if let Some(cols) = sect.child("w:cols") {
        if let Some(n) = cols.attr("w:num").and_then(|v| v.parse::<u8>().ok()) {
            if n > 1 {
                page.columns = Some(n);
                page.column_space = cols
                    .attr("w:space")
                    .and_then(|v| v.parse::<i64>().ok())
                    .map(twip_to_pt);
            }
        }
    }
    // 页面边框
    if let Some(pb) = sect.child("w:pgBorders") {
        let first = ["w:top", "w:left", "w:bottom", "w:right"]
            .iter()
            .find_map(|e| pb.child(e));
        if let Some(edge) = first {
            let val = edge.attr("w:val").unwrap_or("single");
            if val != "none" && val != "nil" {
                page.page_border = Some(Border {
                    style: Some(val.to_string()),
                    color: edge
                        .attr("w:color")
                        .filter(|c| !c.eq_ignore_ascii_case("auto"))
                        .map(|c| c.trim_start_matches('#').to_string()),
                    width: edge
                        .attr("w:sz")
                        .and_then(|v| v.parse::<f64>().ok())
                        .map(|s| s / 8.0),
                });
            }
        }
    }
    // 文档网格
    if let Some(dg) = sect.child("w:docGrid") {
        page.doc_grid = Some(dg.to_xml());
    }
    if sect.child("w:bidi").is_some() {
        page.rtl = Some(true);
    }
    if sect.child("w:rtlGutter").is_some() {
        page.rtl_gutter = Some(true);
    }
    page
}

/// 解析脚注：id → 文本
fn parse_footnotes(files: &Package) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Some(txt) = text_of(files, "word/footnotes.xml") {
        if let Ok(root) = parse_xml(&txt) {
            for f in root.children_named("w:footnote") {
                // 跳过 separator / continuationSeparator
                if f.attr("w:type").is_some() {
                    continue;
                }
                if let Some(id) = f.attr("w:id") {
                    out.insert(id.to_string(), f.text_content().trim().to_string());
                }
            }
        }
    }
    out
}

/// 解析尾注：id → 文本
fn parse_endnotes(files: &Package) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Some(txt) = text_of(files, "word/endnotes.xml") {
        if let Ok(root) = parse_xml(&txt) {
            for f in root.children_named("w:endnote") {
                if f.attr("w:type").is_some() {
                    continue;
                }
                if let Some(id) = f.attr("w:id") {
                    out.insert(id.to_string(), f.text_content().trim().to_string());
                }
            }
        }
    }
    out
}

/// 解析批注：id → (作者, 文本)
fn parse_comments(files: &Package) -> HashMap<String, (String, String)> {
    let mut out = HashMap::new();
    if let Some(txt) = text_of(files, "word/comments.xml") {
        if let Ok(root) = parse_xml(&txt) {
            for c in root.children_named("w:comment") {
                if let Some(id) = c.attr("w:id") {
                    let author = c.attr("w:author").unwrap_or("").to_string();
                    out.insert(
                        id.to_string(),
                        (author, c.text_content().trim().to_string()),
                    );
                }
            }
        }
    }
    out
}

fn is_ordered(def: Option<&NumDef>) -> bool {
    def.and_then(|d| d.levels.get(&0).or_else(|| d.levels.values().next()))
        .map(|l| l.fmt != "bullet")
        .unwrap_or(false)
}

fn default_fmt(ordered: bool, lvl: u8) -> String {
    if !ordered {
        "bullet".to_string()
    } else {
        match lvl % 3 {
            0 => "decimal",
            1 => "lowerLetter",
            _ => "lowerRoman",
        }
        .to_string()
    }
}

fn default_text(ordered: bool, lvl: u8) -> String {
    if !ordered {
        "•".to_string()
    } else {
        format!("%{}.", lvl + 1)
    }
}

/// 将 Wingdings/Symbol 私有区符号归一化为常见 Unicode（避免依赖字体）
fn normalize_bullet(s: &str) -> String {
    let mapped: String = s
        .chars()
        .map(|c| match c {
            '\u{F0B7}' | '\u{F0A7}' => '•',
            '\u{F0A8}' => '▪',
            '\u{F075}' => '❖',
            '\u{F0D8}' => '➢',
            '\u{F0FC}' => '✔',
            other => other,
        })
        .collect();
    mapped
}

/// 计算某列表项的“自定义编号”信息；与默认生成保持一致时返回 (None,None,None,None)
fn custom_numbering(
    def: Option<&NumDef>,
    lvl: u8,
    ordered: bool,
) -> (Option<String>, Option<String>, Option<i64>, Option<String>) {
    let level = def.and_then(|d| d.levels.get(&lvl));
    let fmt = level
        .map(|l| l.fmt.clone())
        .unwrap_or_else(|| default_fmt(ordered, lvl));
    let raw_text = level
        .map(|l| l.text.clone())
        .unwrap_or_else(|| default_text(ordered, lvl));
    let start = level.map(|l| l.start).unwrap_or(1);
    let font = level.and_then(|l| l.font.clone());
    let text = if fmt == "bullet" {
        normalize_bullet(&raw_text)
    } else {
        raw_text
    };
    let custom =
        fmt != default_fmt(ordered, lvl) || text != default_text(ordered, lvl) || start != 1;
    if !custom {
        return (None, None, None, None);
    }
    let font = if fmt == "bullet" { font } else { None };
    (
        Some(fmt),
        Some(text),
        if start != 1 { Some(start) } else { None },
        font,
    )
}

fn parse_numbering(files: &Package) -> HashMap<String, NumDef> {
    let mut abstracts: HashMap<String, NumDef> = HashMap::new();
    let mut out: HashMap<String, NumDef> = HashMap::new();
    if let Some(txt) = text_of(files, "word/numbering.xml") {
        if let Ok(root) = parse_xml(&txt) {
            for an in root.children_named("w:abstractNum") {
                let id = an.attr("w:abstractNumId").unwrap_or("").to_string();
                let mut def = NumDef::default();
                for l in an.children_named("w:lvl") {
                    let Some(ilvl) = l.attr("w:ilvl").and_then(|v| v.parse::<u8>().ok()) else {
                        continue;
                    };
                    let fmt = l
                        .child("w:numFmt")
                        .and_then(|f| f.attr("w:val"))
                        .unwrap_or("bullet")
                        .to_string();
                    let text = l
                        .child("w:lvlText")
                        .and_then(|f| f.attr("w:val"))
                        .unwrap_or("")
                        .to_string();
                    let start = l
                        .child("w:start")
                        .and_then(|f| f.attr("w:val"))
                        .and_then(|v| v.parse::<i64>().ok())
                        .unwrap_or(1);
                    let font = l
                        .child("w:rPr")
                        .and_then(|r| r.child("w:rFonts"))
                        .and_then(|f| f.attr("w:ascii").or_else(|| f.attr("w:hAnsi")))
                        .map(|v| v.to_string());
                    def.levels.insert(
                        ilvl,
                        NumLevel {
                            fmt,
                            text,
                            start,
                            font,
                        },
                    );
                }
                abstracts.insert(id, def);
            }
            for num in root.children_named("w:num") {
                let id = num.attr("w:numId").unwrap_or("").to_string();
                if let Some(a) = num.child("w:abstractNumId").and_then(|n| n.attr("w:val")) {
                    let mut def = abstracts.get(a).cloned().unwrap_or_default();
                    // 应用 lvlOverride（startOverride / 内嵌 lvl）
                    for ov in num.children_named("w:lvlOverride") {
                        let Some(ilvl) = ov.attr("w:ilvl").and_then(|v| v.parse::<u8>().ok())
                        else {
                            continue;
                        };
                        if let Some(so) = ov
                            .child("w:startOverride")
                            .and_then(|s| s.attr("w:val"))
                            .and_then(|v| v.parse::<i64>().ok())
                        {
                            let entry = def.levels.entry(ilvl).or_default();
                            entry.start = so;
                        }
                        if let Some(l) = ov.child("w:lvl") {
                            let entry = def.levels.entry(ilvl).or_default();
                            if let Some(f) = l.child("w:numFmt").and_then(|f| f.attr("w:val")) {
                                entry.fmt = f.to_string();
                            }
                            if let Some(t) = l.child("w:lvlText").and_then(|f| f.attr("w:val")) {
                                entry.text = t.to_string();
                            }
                            if let Some(st) = l
                                .child("w:start")
                                .and_then(|f| f.attr("w:val"))
                                .and_then(|v| v.parse::<i64>().ok())
                            {
                                entry.start = st;
                            }
                        }
                    }
                    out.insert(id, def);
                }
            }
        }
    }
    out
}

/// 解析 styles.xml 的 w:docDefaults（默认字体/字号与段落间距/行距）
fn parse_doc_defaults(files: &Package) -> Option<Style> {
    let txt = text_of(files, "word/styles.xml")?;
    let root = parse_xml(&txt).ok()?;
    let dd = root.child("w:docDefaults")?;
    let mut st = Style::default();
    if let Some(rpr) = dd.child("w:rPrDefault").and_then(|r| r.child("w:rPr")) {
        if rpr.child("w:b").is_some() {
            st.bold = Some(true);
        }
        if rpr.child("w:i").is_some() {
            st.italic = Some(true);
        }
        if let Some(c) = rpr
            .child("w:color")
            .and_then(|c| c.attr("w:val"))
            .filter(|c| *c != "auto")
        {
            st.color = Some(c.to_string());
        }
        if let Some(sz) = rpr
            .child("w:sz")
            .and_then(|s| s.attr("w:val"))
            .and_then(|v| v.parse::<i64>().ok())
        {
            st.font_size = Some(half_to_pt(sz));
        }
        if let Some(f) = rpr.child("w:rFonts") {
            if let Some(a) = f
                .attr("w:ascii")
                .or_else(|| f.attr("w:hAnsi"))
                .filter(|s| !s.is_empty())
            {
                st.font_family = Some(a.to_string());
            }
            if let Some(e) = f.attr("w:eastAsia").filter(|s| !s.is_empty()) {
                st.east_asia_font = Some(e.to_string());
            }
        }
    }
    if let Some(ppr) = dd.child("w:pPrDefault").and_then(|p| p.child("w:pPr")) {
        if let Some(sp) = ppr.child("w:spacing") {
            if let Some(l) = sp.attr("w:line").and_then(|v| v.parse::<f64>().ok()) {
                if l != 0.0 {
                    let rule = sp.attr("w:lineRule").unwrap_or("auto");
                    if rule == "auto" {
                        st.line_spacing = Some(l / LINE_SINGLE);
                    } else {
                        st.line_pt = Some(twip_to_pt(l as i64));
                        st.line_rule = Some(rule.to_string());
                    }
                }
            }
            if let Some(b) = sp.attr("w:before").and_then(|v| v.parse::<i64>().ok()) {
                st.space_before = Some(twip_to_pt(b));
            }
            if let Some(a) = sp.attr("w:after").and_then(|v| v.parse::<i64>().ok()) {
                st.space_after = Some(twip_to_pt(a));
            }
        }
        if let Some(jc) = ppr.child("w:jc").and_then(|j| j.attr("w:val")) {
            st.align = Some(jc.to_string());
        }
    }
    if st == Style::default() {
        None
    } else {
        Some(st)
    }
}

/// 将样式名/Id 归一化为规范 id（兼容本地化 styleId，如 "10"/"a"）
fn normalize_style_id(raw_id: &str, name: &str) -> String {
    let lname = name.to_ascii_lowercase();
    if let Some(rest) = lname.strip_prefix("heading") {
        let lvl = rest
            .trim()
            .chars()
            .find(|c| c.is_ascii_digit())
            .and_then(|c| c.to_digit(10))
            .unwrap_or(1)
            .clamp(1, 9);
        format!("Heading{lvl}")
    } else if lname == "normal" {
        "Normal".to_string()
    } else if lname == "title" {
        "Title".to_string()
    } else if lname == "subtitle" {
        "Subtitle".to_string()
    } else if lname.contains("caption") {
        "Caption".to_string()
    } else if lname.contains("quote") {
        "Quote".to_string()
    } else if lname.contains("code") {
        "Code".to_string()
    } else if lname.contains("reference") || lname.contains("bibliography") {
        "Reference".to_string()
    } else {
        raw_id.to_string()
    }
}

fn parse_styles(files: &Package) -> BTreeMap<String, Style> {
    let mut out = BTreeMap::new();
    let Some(txt) = text_of(files, "word/styles.xml") else {
        return out;
    };
    let Ok(root) = parse_xml(&txt) else {
        return out;
    };
    // 第一遍：raw styleId → 规范 id（用于解析 basedOn 引用）
    let mut id_map: HashMap<String, String> = HashMap::new();
    for s in root.children_named("w:style") {
        if s.attr("w:type") != Some("paragraph") {
            continue;
        }
        let Some(raw_id) = s.attr("w:styleId") else {
            continue;
        };
        let name = s
            .child("w:name")
            .and_then(|n| n.attr("w:val"))
            .unwrap_or("")
            .to_string();
        id_map.insert(raw_id.to_string(), normalize_style_id(raw_id, &name));
    }
    for s in root.children_named("w:style") {
        if s.attr("w:type") != Some("paragraph") {
            continue;
        }
        let Some(raw_id) = s.attr("w:styleId") else {
            continue;
        };
        let id = id_map
            .get(raw_id)
            .cloned()
            .unwrap_or_else(|| raw_id.to_string());
        let mut st = Style::default();
        if let Some(b) = s
            .child("w:basedOn")
            .and_then(|n| n.attr("w:val"))
            .filter(|v| !v.is_empty() && *v != "Normal")
        {
            st.based_on = Some(id_map.get(b).cloned().unwrap_or_else(|| b.to_string()));
        }
        if let Some(ppr) = s.child("w:pPr") {
            if ppr.child("w:contextualSpacing").is_some() {
                st.contextual_spacing = Some(true);
            }
            if ppr.child("w:bidi").is_some() {
                st.rtl = Some(true);
            }
            if let Some(nid) = ppr
                .child("w:numPr")
                .and_then(|n| n.child("w:numId"))
                .and_then(|n| n.attr("w:val"))
                .filter(|v| *v != "0" && !v.is_empty())
            {
                st.num_id = Some(nid.to_string());
            }
            if let Some(v) = ppr.child("w:autoSpaceDE").and_then(|n| n.attr("w:val")) {
                st.auto_space_de = Some(v != "0");
            }
            if let Some(v) = ppr.child("w:autoSpaceDN").and_then(|n| n.attr("w:val")) {
                st.auto_space_dn = Some(v != "0");
            }
            if let Some(v) = ppr.child("w:adjustRightInd").and_then(|n| n.attr("w:val")) {
                st.adjust_right_ind = Some(v != "0");
            }
            if let Some(v) = ppr.child("w:snapToGrid").and_then(|n| n.attr("w:val")) {
                st.snap_to_grid = Some(v != "0");
            }
            if let Some(jc) = ppr.child("w:jc").and_then(|j| j.attr("w:val")) {
                st.align = Some(jc.to_string());
            }
            if let Some(ind) = ppr.child("w:ind") {
                if let Some(v) = ind.attr("w:firstLine").and_then(|v| v.parse::<i64>().ok()) {
                    st.first_line_indent = Some(twip_to_pt(v));
                }
                if let Some(v) = ind.attr("w:left").and_then(|v| v.parse::<i64>().ok()) {
                    if v != 0 {
                        st.indent = Some(twip_to_pt(v));
                    }
                }
                if let Some(v) = ind.attr("w:hanging").and_then(|v| v.parse::<i64>().ok()) {
                    if v != 0 {
                        st.hanging_indent = Some(twip_to_pt(v));
                    }
                }
            }
            if let Some(tabs) = ppr.child("w:tabs") {
                let pos: Vec<crate::model::blocks::TabStop> = tabs
                    .children_named("w:tab")
                    .map(|t| crate::model::blocks::TabStop {
                        pos: t
                            .attr("w:pos")
                            .and_then(|v| v.parse::<i64>().ok())
                            .map(twip_to_pt)
                            .unwrap_or(0.0),
                        align: t
                            .attr("w:val")
                            .filter(|v| *v != "left")
                            .map(|v| v.to_string()),
                        leader: t
                            .attr("w:leader")
                            .filter(|v| *v != "none")
                            .map(|v| v.to_string()),
                    })
                    .collect();
                if !pos.is_empty() {
                    st.tabs = Some(pos);
                }
            }
            if let Some(sp) = ppr.child("w:spacing") {
                if let Some(l) = sp.attr("w:line").and_then(|v| v.parse::<f64>().ok()) {
                    let rule = sp.attr("w:lineRule").unwrap_or("auto");
                    if rule == "auto" {
                        st.line_spacing = Some(l / LINE_SINGLE);
                    } else {
                        st.line_pt = Some(twip_to_pt(l as i64));
                        st.line_rule = Some(rule.to_string());
                    }
                }
                if let Some(b) = sp.attr("w:before").and_then(|v| v.parse::<i64>().ok()) {
                    st.space_before = Some(twip_to_pt(b));
                }
                if let Some(a) = sp.attr("w:after").and_then(|v| v.parse::<i64>().ok()) {
                    st.space_after = Some(twip_to_pt(a));
                }
                if sp
                    .attr("w:beforeAutospacing")
                    .map(|v| v != "0")
                    .unwrap_or(false)
                {
                    st.before_autospacing = Some(true);
                }
                if sp
                    .attr("w:afterAutospacing")
                    .map(|v| v != "0")
                    .unwrap_or(false)
                {
                    st.after_autospacing = Some(true);
                }
            }
        }
        let rpr = s
            .child("w:pPr")
            .and_then(|p| p.child("w:rPr"))
            .or_else(|| s.child("w:rPr"));
        if let Some(rpr) = rpr {
            if rpr.child("w:b").is_some() {
                st.bold = Some(true);
            }
            if rpr.child("w:i").is_some() {
                st.italic = Some(true);
            }
            if let Some(u) = rpr.child("w:u") {
                if u.attr("w:val").map(|v| v != "none").unwrap_or(true) {
                    st.underline = Some(true);
                }
            }
            if rpr.child("w:strike").is_some() {
                st.strike = Some(true);
            }
            if let Some(c) = rpr.child("w:color").and_then(|c| c.attr("w:val")) {
                if c != "auto" {
                    st.color = Some(c.to_string());
                }
            }
            if let Some(sz) = rpr.child("w:sz").and_then(|s| s.attr("w:val")) {
                if let Ok(hp) = sz.parse::<i64>() {
                    st.font_size = Some(half_to_pt(hp));
                }
            }
            if let Some(fonts) = rpr.child("w:rFonts") {
                if let Some(a) = fonts
                    .attr("w:ascii")
                    .or_else(|| fonts.attr("w:hAnsi"))
                    .filter(|s| !s.is_empty())
                {
                    st.font_family = Some(a.to_string());
                }
                if let Some(e) = fonts.attr("w:eastAsia").filter(|s| !s.is_empty()) {
                    st.east_asia_font = Some(e.to_string());
                }
                if let Some(v) = fonts.attr("w:cs").filter(|s| !s.is_empty()) {
                    st.cs_font = Some(v.to_string());
                }
            }
        }
        out.insert(id.to_string(), st);
    }
    out
}

/// 收集未建模部件（原样透传）：SmartArt diagrams、customXml、字体、glossary 等
/// 收集模型已引用的媒体路径（生成器会重写这些，透传时跳过）
fn referenced_media(parts: &[Part]) -> std::collections::HashSet<String> {
    use crate::model::blocks::Block;
    fn walk(bs: &[Block], out: &mut std::collections::HashSet<String>) {
        for b in bs {
            match b {
                Block::Image(i) => {
                    out.insert(i.src.clone());
                }
                Block::Attachment(a) => {
                    out.insert(a.src.clone());
                }
                Block::Paragraph(p) => {
                    if let Some(rs) = &p.runs {
                        for r in rs {
                            if let Some(img) = &r.image {
                                out.insert(img.src.clone());
                            }
                        }
                    }
                }
                Block::List(l) => {
                    for it in &l.items {
                        walk(&it.blocks, out);
                    }
                }
                Block::Table(t) => {
                    for row in &t.rows {
                        for c in &row.cells {
                            walk(&c.blocks, out);
                        }
                    }
                }
                Block::Sdt(c) => walk(&c.blocks, out),
                _ => {}
            }
        }
    }
    let mut out = std::collections::HashSet::new();
    for part in parts {
        walk(&part.blocks, &mut out);
    }
    out
}

fn collect_passthrough(
    files: &Package,
    referenced: &std::collections::HashSet<String>,
) -> Vec<crate::model::document::PassthroughPart> {
    use crate::model::document::PassthroughPart;
    // 由本工具重新生成的部件（不透传）
    fn is_regenerated(name: &str) -> bool {
        if name == "[Content_Types].xml"
            || name == "_rels/.rels"
            || name == "docProps/core.xml"
            || name == "docProps/app.xml"
        {
            return true;
        }
        // 被模型引用的媒体由生成器重写，跳过；未被引用的原样透传
        if name.starts_with("word/media/") {
            return false;
        }
        let base = name.strip_prefix("word/").unwrap_or(name);
        if base.starts_with("_rels/") {
            // 仅保留“非已知部件”自身的 rels；已知部件的 rels 由生成器重建
            let rel_of = base.trim_start_matches("_rels/").trim_end_matches(".rels");
            return is_known_part(rel_of);
        }
        if base.starts_with("parts/") || base.starts_with("header") || base.starts_with("footer") {
            return true;
        }
        if base.starts_with("charts/") {
            return true;
        }
        is_known_part(base)
    }
    fn is_known_part(name: &str) -> bool {
        matches!(
            name,
            "document.xml"
                | "styles.xml"
                | "settings.xml"
                | "numbering.xml"
                | "webSettings.xml"
                | "fontTable.xml"
                | "theme/theme1.xml"
                | "footnotes.xml"
                | "endnotes.xml"
                | "comments.xml"
        ) || name.starts_with("header")
            || name.starts_with("footer")
            || name.starts_with("parts/")
    }
    let ct = parse_content_types_map(files);
    let mut out = Vec::new();
    for (name, bytes) in files {
        if name.ends_with('/') || is_regenerated(name) {
            continue;
        }
        // 模型已引用的媒体由生成器重写，避免重复；未引用的原样透传
        if name.starts_with("word/media/") && referenced.contains(name) {
            continue;
        }
        out.push(PassthroughPart {
            path: name.clone(),
            content_type: content_type_for(name, &ct),
            data: crate::utils::b64::encode(bytes),
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// 解析 [Content_Types].xml 为 Override 映射（PartName → ContentType）与 Default 扩展名
fn parse_content_types_map(files: &Package) -> (HashMap<String, String>, HashMap<String, String>) {
    let mut overrides = HashMap::new();
    let mut defaults = HashMap::new();
    if let Some(txt) = text_of(files, "[Content_Types].xml") {
        if let Ok(root) = parse_xml(&txt) {
            for d in root.children_named("Default") {
                if let (Some(ext), Some(ct)) = (d.attr("Extension"), d.attr("ContentType")) {
                    defaults.insert(ext.to_ascii_lowercase(), ct.to_string());
                }
            }
            for o in root.children_named("Override") {
                if let (Some(p), Some(ct)) = (o.attr("PartName"), o.attr("ContentType")) {
                    overrides.insert(p.trim_start_matches('/').to_string(), ct.to_string());
                }
            }
        }
    }
    (overrides, defaults)
}

fn content_type_for(
    name: &str,
    maps: &(HashMap<String, String>, HashMap<String, String>),
) -> Option<String> {
    let (overrides, defaults) = maps;
    if let Some(ct) = overrides.get(name) {
        return Some(ct.clone());
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    defaults.get(&ext).cloned()
}

/// 解析非段落样式（表格/字符/编号等）为原样 XML，供 styles.xml 回放
fn parse_raw_styles(files: &Package) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let Some(txt) = text_of(files, "word/styles.xml") else {
        return out;
    };
    let Ok(root) = parse_xml(&txt) else {
        return out;
    };
    // 生成器已产出的样式 id，避免重复
    let generated = |id: &str| {
        id == "Normal"
            || id.starts_with("Heading")
            || matches!(
                id,
                "Title"
                    | "Subtitle"
                    | "TOCHeading"
                    | "TOC1"
                    | "TOC2"
                    | "TOC3"
                    | "Caption"
                    | "Quote"
                    | "Code"
                    | "Reference"
                    | "Hyperlink"
                    | "CodeChar"
                    | "TableGrid"
                    | "TableGridLight"
                    | "FootnoteText"
                    | "FootnoteReference"
                    | "EndnoteText"
                    | "EndnoteReference"
                    | "CommentText"
                    | "CommentReference"
                    | "InsertedText"
                    | "DeletedText"
            )
    };
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for s in root.children_named("w:style") {
        if s.attr("w:type") == Some("paragraph") {
            continue;
        }
        let Some(id) = s.attr("w:styleId") else {
            continue;
        };
        if id.is_empty() || generated(id) || !seen.insert(id.to_string()) {
            continue;
        }
        out.push(s.to_xml());
    }
    out
}

fn parse_theme(files: &Package) -> Theme {
    let mut theme = Theme::default();
    let Some(txt) = text_of(files, "word/theme/theme1.xml") else {
        return theme;
    };
    let Ok(root) = parse_xml(&txt) else {
        return theme;
    };
    if let Some(scheme) = root
        .child("a:themeElements")
        .and_then(|e| e.child("a:fontScheme"))
    {
        let tf = |n: Option<&XmlNode>| -> Option<String> {
            n.and_then(|x| x.attr("typeface"))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        };
        if let Some(major) = scheme.child("a:majorFont").and_then(|m| m.child("a:latin")) {
            theme.major_font = tf(Some(major));
        }
        if let Some(minor) = scheme.child("a:minorFont").and_then(|m| m.child("a:latin")) {
            theme.minor_font = tf(Some(minor));
        }
        if let Some(ea) = scheme.child("a:minorFont").and_then(|m| m.child("a:ea")) {
            theme.east_asia_font = tf(Some(ea));
        }
    }
    theme
}
