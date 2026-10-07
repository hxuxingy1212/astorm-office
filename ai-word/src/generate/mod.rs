//! 生成模块：JSON 模型 → DOCX
//!
//! 管线：JSON → Rust 模型 → WordprocessingML 字符串（手写 + 转义）→ OPC ZIP → .docx

pub mod aux;
pub mod body;
pub mod chart;
pub mod math;
pub mod package;

use crate::error::Result;
use crate::model::blocks::*;
use crate::model::Document;
use crate::utils::{detect_image_extension, image_size, load_image_bytes};
use std::collections::HashMap;

/// 生成结果
pub struct GenerateResult {
    pub path: String,
    pub parts: usize,
}

/// 媒体文件
pub(crate) struct MediaFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// 生成上下文：承载关系 ID 映射、媒体与页眉页脚信息
pub(crate) struct GenCtx {
    pub doc: Document,
    pub media: Vec<MediaFile>,
    /// 图片源 → 媒体文件名
    pub img_file: HashMap<String, String>,
    /// 图片源 → rId
    pub img_rid: HashMap<String, String>,
    /// 图片源 → (宽, 高) pt
    pub img_size: HashMap<String, (f64, f64)>,
    /// 加载失败的图片源（生成时跳过，保证损坏文档也能回环）
    pub img_failed: std::collections::HashSet<String>,
    /// 附件：嵌入文件（embeddings/<name> 的字节）
    pub embeddings: Vec<(String, Vec<u8>)>,
    pub attach_embed_rid: HashMap<String, String>,
    pub attach_icon_rid: HashMap<String, String>,
    pub attach_size: HashMap<String, (f64, f64)>,
    pub attach_progid: HashMap<String, String>,
    pub attach_name: HashMap<String, String>,
    pub attach_label: HashMap<String, String>,
    pub attach_failed: std::collections::HashSet<String>,
    /// 超链接 URL → rId
    pub hyper_rid: HashMap<String, String>,
    /// 页眉部件：(文件名, 内容)
    pub header_parts: Vec<(String, String)>,
    pub header_rids: Vec<String>,
    /// 页脚部件：(文件名, 内容)
    pub footer_parts: Vec<(String, String)>,
    pub footer_rids: Vec<String>,
    /// 每个分片的页眉引用：(类型 default|first|even, rId)
    pub part_header_refs: Vec<Vec<(String, String)>>,
    /// 每个分片的页脚引用
    pub part_footer_refs: Vec<Vec<(String, String)>>,
    /// 每个分片是否启用“首页不同”
    pub title_pages: Vec<bool>,
    /// 是否启用“奇偶页不同”
    pub even_and_odd: bool,
    /// 水印文本（注入页眉）
    pub watermark: Option<String>,
    /// 图表部件：(文件名, XML)
    pub chart_parts: Vec<(String, String)>,
    /// 图表 rId
    pub chart_rids: Vec<String>,
    /// 脚注文本（按出现顺序，id 从 1 开始）
    pub footnotes: Vec<String>,
    /// 是否存在脚注
    pub any_footnotes: bool,
    /// 尾注文本
    pub endnotes: Vec<String>,
    /// 是否存在尾注
    pub any_endnotes: bool,
    /// 批注 (作者, 文本)
    pub comments: Vec<(String, String)>,
    /// 是否存在批注
    pub any_comments: bool,
    pub rels: Vec<Rel>,
    pub next_rid: u32,
    // 每文档生成计数器（原为全局原子量；改为 ctx 内 Cell，保证同一进程并发
    // generate 互不干扰——全局量曾导致并行生成时图表 rId 错配、id 碰撞）
    pub next_draw_id: std::cell::Cell<i32>,
    pub fig_seq: std::cell::Cell<u32>,
    pub tbl_seq: std::cell::Cell<u32>,
    pub footnote_seq: std::cell::Cell<u32>,
    pub endnote_seq: std::cell::Cell<u32>,
    pub bookmark_id: std::cell::Cell<i32>,
    pub comment_seq: std::cell::Cell<u32>,
    pub chart_seq: std::cell::Cell<u32>,
    pub revision_id: std::cell::Cell<i32>,
    /// 自定义列表编号：list_key → numId
    pub list_numid: HashMap<String, i64>,
    /// 自定义编号定义：(numId, 9 级 (fmt, lvlText, start, font))
    pub num_defs: Vec<(i64, Vec<LevelDef>)>,
    /// 未建模原始块的关系重映射：原 rId → 新 rId
    pub raw_rel_map: HashMap<String, String>,
}

/// 单级编号定义：(fmt, lvlText, start, font)
pub(crate) type LevelDef = (String, String, i64, Option<String>);

pub(crate) struct Rel {
    pub id: String,
    pub rel_type: String,
    pub target: String,
    pub external: bool,
}

pub(crate) fn default_fmt_gen(ordered: bool, lvl: u8) -> String {
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

pub(crate) fn default_text_gen(ordered: bool, lvl: u8) -> String {
    if !ordered {
        "•".to_string()
    } else {
        format!("%{}.", lvl + 1)
    }
}

/// 列表自定义编号的稳定 key（无自定义返回 None）
pub(crate) fn list_key(l: &crate::model::blocks::ListBlock) -> Option<String> {
    let mut any = false;
    let mut parts: Vec<String> = Vec::new();
    for lvl in 0..9u8 {
        if let Some(it) = l.items.iter().find(|i| i.level == lvl) {
            if it.num_format.is_some() || it.lvl_text.is_some() || it.start.is_some() {
                any = true;
            }
            parts.push(format!(
                "{}:{}:{}:{}",
                it.num_format.clone().unwrap_or_default(),
                it.lvl_text.clone().unwrap_or_default(),
                it.start.unwrap_or(0),
                it.num_font.clone().unwrap_or_default()
            ));
        } else {
            parts.push("-".to_string());
        }
    }
    if any {
        Some(parts.join("|"))
    } else {
        None
    }
}

type NumLevelGen = Vec<(String, String, i64, Option<String>)>;

fn collect_numbering(doc: &Document) -> (HashMap<String, i64>, Vec<(i64, NumLevelGen)>) {
    let mut map: HashMap<String, i64> = HashMap::new();
    let mut defs: Vec<(i64, NumLevelGen)> = Vec::new();
    let mut next_id = 3i64;
    let mut visit = |l: &crate::model::blocks::ListBlock,
                     map: &mut HashMap<String, i64>,
                     defs: &mut Vec<(i64, NumLevelGen)>| {
        let Some(key) = list_key(l) else { return };
        if map.contains_key(&key) {
            return;
        }
        let id = next_id;
        next_id += 1;
        let mut levels: NumLevelGen = Vec::new();
        for lvl in 0..9u8 {
            let it = l.items.iter().find(|i| i.level == lvl);
            let fmt = it
                .and_then(|i| i.num_format.clone())
                .unwrap_or_else(|| default_fmt_gen(l.ordered, lvl));
            let text = it
                .and_then(|i| i.lvl_text.clone())
                .unwrap_or_else(|| default_text_gen(l.ordered, lvl));
            let start = it.and_then(|i| i.start).unwrap_or(1);
            let font = it.and_then(|i| i.num_font.clone());
            levels.push((fmt, text, start, font));
        }
        map.insert(key, id);
        defs.push((id, levels));
    };
    for part in &doc.parts {
        walk_lists(&part.blocks, &mut |l| visit(l, &mut map, &mut defs));
    }
    (map, defs)
}

/// 把 SmartArt 可编辑文本写回 diagram 数据部件（透传部件）
fn apply_diagram_edits(doc: &mut Document) {
    use crate::model::blocks::Block;
    let mut edits: Vec<(String, Vec<String>)> = Vec::new();
    fn walk(bs: &[Block], out: &mut Vec<(String, Vec<String>)>) {
        for b in bs {
            match b {
                Block::Raw(r) => {
                    if let Some(d) = &r.diagram {
                        out.push((d.data.clone(), d.texts.clone()));
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
    for part in &doc.parts {
        walk(&part.blocks, &mut edits);
    }
    for (path, texts) in edits {
        if let Some(p) = doc.passthrough.iter_mut().find(|p| p.path == path) {
            if let Some(bytes) = crate::utils::b64::decode(&p.data) {
                let s = String::from_utf8_lossy(&bytes).to_string();
                let new = replace_at_texts(&s, &texts);
                p.data = crate::utils::b64::encode(new.as_bytes());
            }
        }
    }
}

/// 顺序替换 `<a:t>…</a:t>` 文本
fn replace_at_texts(xml: &str, texts: &[String]) -> String {
    let mut out = String::with_capacity(xml.len());
    let mut rest = xml;
    let mut i = 0usize;
    const OPEN: &str = "<a:t>";
    const CLOSE: &str = "</a:t>";
    while let Some(start) = rest.find(OPEN) {
        let after = &rest[start + OPEN.len()..];
        let Some(end) = after.find(CLOSE) else { break };
        out.push_str(&rest[..start + OPEN.len()]);
        if i < texts.len() {
            out.push_str(&crate::utils::xml::esc(&texts[i]));
            i += 1;
        } else {
            out.push_str(&after[..end]);
        }
        out.push_str(CLOSE);
        rest = &after[end + CLOSE.len()..];
    }
    out.push_str(rest);
    out
}

/// 收集所有未建模原始块的待注册关系
fn collect_raw_rels(doc: &Document) -> Vec<(String, Option<String>, String, bool)> {
    use crate::model::blocks::Block;
    fn walk(bs: &[Block], out: &mut Vec<(String, Option<String>, String, bool)>) {
        for b in bs {
            match b {
                Block::Raw(r) => {
                    for rel in &r.rels {
                        out.push((
                            rel.id.clone(),
                            rel.rel_type.clone(),
                            rel.target.clone(),
                            rel.external,
                        ));
                    }
                }
                Block::List(l) => {
                    for it in &l.items {
                        walk(&it.blocks, out);
                    }
                }
                Block::Table(t) => {
                    for row in &t.rows {
                        for cell in &row.cells {
                            walk(&cell.blocks, out);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    for part in &doc.parts {
        walk(&part.blocks, &mut out);
    }
    out
}

/// 递归遍历所有列表块（含表格单元格 / 列表项内部）
fn walk_lists(
    blocks: &[crate::model::blocks::Block],
    f: &mut impl FnMut(&crate::model::blocks::ListBlock),
) {
    use crate::model::blocks::Block;
    for b in blocks {
        match b {
            Block::List(l) => {
                f(l);
                for it in &l.items {
                    walk_lists(&it.blocks, f);
                }
            }
            Block::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        walk_lists(&cell.blocks, f);
                    }
                }
            }
            _ => {}
        }
    }
}

impl GenCtx {
    fn alloc_rid(&mut self) -> String {
        let id = format!("rId{}", self.next_rid);
        self.next_rid += 1;
        id
    }

    fn add_rel(&mut self, rel_type: &str, target: &str, external: bool) -> String {
        let id = self.alloc_rid();
        self.rels.push(Rel {
            id: id.clone(),
            rel_type: rel_type.to_string(),
            target: target.to_string(),
            external,
        });
        id
    }
}

/// 从 JSON 模型生成 DOCX
pub fn generate(doc: &Document, output: &str) -> Result<GenerateResult> {
    let mut doc = doc.clone();
    if let Some(tpl) = doc.template.as_deref() {
        if let Some(preset) = crate::model::TemplatePreset::from_name(tpl) {
            preset.apply(&mut doc);
        }
    }
    if doc.parts.is_empty() {
        // 容错：空文档仍生成可打开的 docx（空正文）
        doc.parts.push(crate::model::Part::default());
    }
    apply_diagram_edits(&mut doc);
    let watermark = doc.watermark.clone();
    let charts = collect_charts(&doc);

    // 扫描媒体 / 超链接 / 页眉页脚
    let mut srcs: Vec<String> = Vec::new();
    let mut urls: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();
    let mut enotes: Vec<String> = Vec::new();
    let mut comments: Vec<(String, String)> = Vec::new();
    for part in &doc.parts {
        scan_part(
            part,
            &mut srcs,
            &mut urls,
            &mut notes,
            &mut enotes,
            &mut comments,
        );
    }
    // 逐节页眉/页脚（default / first / even）
    #[derive(Clone)]
    struct HfSpec {
        kind: &'static str,
        header: Option<String>,
        header_blocks: Option<Vec<crate::model::blocks::Block>>,
        footer: bool,
        footer_format: String,
        footer_blocks: Option<Vec<crate::model::blocks::Block>>,
    }
    let mut part_specs: Vec<Vec<HfSpec>> = Vec::new();
    let mut title_pages: Vec<bool> = Vec::new();
    let mut even_and_odd = false;
    for p in &doc.parts {
        let s = p.section.clone().unwrap_or_default();
        let fmt = s
            .footer_format
            .clone()
            .unwrap_or_else(|| "page".to_string());
        let default_header_text = s
            .header
            .clone()
            .or_else(|| watermark.as_ref().map(|_| String::new()));
        let mut specs = vec![HfSpec {
            kind: "default",
            header: default_header_text,
            header_blocks: s.header_blocks.clone(),
            footer: s.footer_page_number.unwrap_or(false),
            footer_format: fmt.clone(),
            footer_blocks: s.footer_blocks.clone(),
        }];
        let has_first = s.title_page.unwrap_or(false)
            && (s.first_header.is_some()
                || s.first_header_blocks.is_some()
                || s.first_footer_page_number.is_some());
        if has_first {
            specs.push(HfSpec {
                kind: "first",
                header: s.first_header.clone(),
                header_blocks: s.first_header_blocks.clone(),
                footer: s.first_footer_page_number.unwrap_or(false),
                footer_format: fmt.clone(),
                footer_blocks: None,
            });
        }
        if doc.even_and_odd.unwrap_or(false)
            && (s.even_header.is_some()
                || s.even_header_blocks.is_some()
                || s.even_footer_page_number.is_some())
        {
            specs.push(HfSpec {
                kind: "even",
                header: s.even_header.clone(),
                header_blocks: s.even_header_blocks.clone(),
                footer: s.even_footer_page_number.unwrap_or(false),
                footer_format: fmt.clone(),
                footer_blocks: None,
            });
            even_and_odd = true;
        }
        title_pages.push(has_first);
        part_specs.push(specs);
    }

    // 唯一页眉部件（按 kind+文本）与页脚部件（按 格式）
    let mut header_keys: Vec<(String, String)> = Vec::new(); // (kind, text)
    let mut header_block_srcs: Vec<Option<Vec<crate::model::blocks::Block>>> = Vec::new();
    let mut footer_formats: Vec<String> = Vec::new();
    let mut footer_block_srcs: Vec<Option<Vec<crate::model::blocks::Block>>> = Vec::new();
    for specs in &part_specs {
        for sp in specs {
            let htext = sp.header.clone().or_else(|| {
                sp.header_blocks.as_ref().map(|bs| {
                    bs.iter()
                        .map(|b| b.plain_text())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            });
            if let Some(text) = htext {
                if !header_keys.iter().any(|(k, t)| k == sp.kind && t == &text) {
                    header_keys.push((sp.kind.to_string(), text));
                    header_block_srcs.push(sp.header_blocks.clone());
                }
            }
            if sp.footer && !footer_formats.iter().any(|f| f == &sp.footer_format) {
                footer_formats.push(sp.footer_format.clone());
                footer_block_srcs.push(sp.footer_blocks.clone());
            }
        }
    }

    let any_footnotes = !notes.is_empty();
    let any_endnotes = !enotes.is_empty();
    let any_comments = !comments.is_empty();

    let (list_numid, num_defs) = collect_numbering(&doc);

    let mut ctx = GenCtx {
        doc,
        media: Vec::new(),
        img_file: HashMap::new(),
        img_rid: HashMap::new(),
        img_size: HashMap::new(),
        img_failed: std::collections::HashSet::new(),
        embeddings: Vec::new(),
        attach_embed_rid: HashMap::new(),
        attach_icon_rid: HashMap::new(),
        attach_size: HashMap::new(),
        attach_progid: HashMap::new(),
        attach_name: HashMap::new(),
        attach_label: HashMap::new(),
        attach_failed: std::collections::HashSet::new(),
        hyper_rid: HashMap::new(),
        header_parts: Vec::new(),
        header_rids: Vec::new(),
        footer_parts: Vec::new(),
        footer_rids: Vec::new(),
        part_header_refs: Vec::new(),
        part_footer_refs: Vec::new(),
        title_pages,
        even_and_odd,
        watermark: watermark.clone(),
        chart_parts: Vec::new(),
        chart_rids: Vec::new(),
        footnotes: notes,
        any_footnotes,
        endnotes: enotes,
        any_endnotes,
        comments,
        any_comments,
        rels: Vec::new(),
        next_rid: 1,
        next_draw_id: std::cell::Cell::new(1),
        fig_seq: std::cell::Cell::new(0),
        tbl_seq: std::cell::Cell::new(0),
        footnote_seq: std::cell::Cell::new(0),
        endnote_seq: std::cell::Cell::new(0),
        bookmark_id: std::cell::Cell::new(1),
        comment_seq: std::cell::Cell::new(0),
        chart_seq: std::cell::Cell::new(0),
        revision_id: std::cell::Cell::new(100),
        list_numid,
        num_defs,
        raw_rel_map: HashMap::new(),
    };

    // 固定关系
    ctx.add_rel(crate::utils::constants::REL_STYLES, "styles.xml", false);
    ctx.add_rel(crate::utils::constants::REL_SETTINGS, "settings.xml", false);
    ctx.add_rel(
        crate::utils::constants::REL_NUMBERING,
        "numbering.xml",
        false,
    );
    ctx.add_rel(
        crate::utils::constants::REL_FONT_TABLE,
        "fontTable.xml",
        false,
    );
    ctx.add_rel(
        crate::utils::constants::REL_THEME,
        "theme/theme1.xml",
        false,
    );
    ctx.add_rel(
        crate::utils::constants::REL_WEB_SETTINGS,
        "webSettings.xml",
        false,
    );

    // 未建模原始块的关系：统一分配新 rId 并注册
    let raw_rels = collect_raw_rels(&ctx.doc);
    for (id, rtype, target, external) in raw_rels {
        if ctx.raw_rel_map.contains_key(&id) {
            continue;
        }
        let ty = rtype.unwrap_or_default();
        let rid = ctx.add_rel(&ty, &target, external);
        ctx.raw_rel_map.insert(id, rid);
    }
    // 构建页眉/页脚部件内容（可用 ctx 渲染块）
    for (i, (_kind, text)) in header_keys.iter().enumerate() {
        let blocks = header_block_srcs.get(i).and_then(|b| b.as_deref());
        let xml = aux::header_xml_content(&ctx, text, blocks, ctx.watermark.as_deref());
        ctx.header_parts.push((format!("header{}.xml", i + 1), xml));
    }
    for (i, fmt) in footer_formats.iter().enumerate() {
        let blocks = footer_block_srcs.get(i).and_then(|b| b.as_deref());
        let xml = aux::footer_xml_content(&ctx, fmt, blocks);
        ctx.footer_parts.push((format!("footer{}.xml", i + 1), xml));
    }

    // 页眉 / 页脚部件关系
    let header_names: Vec<String> = ctx.header_parts.iter().map(|(n, _)| n.clone()).collect();
    for name in &header_names {
        let rid = ctx.add_rel(crate::utils::constants::REL_HEADER, name, false);
        ctx.header_rids.push(rid);
    }
    let footer_names: Vec<String> = ctx.footer_parts.iter().map(|(n, _)| n.clone()).collect();
    for name in &footer_names {
        let rid = ctx.add_rel(crate::utils::constants::REL_FOOTER, name, false);
        ctx.footer_rids.push(rid);
    }
    // 每分片的引用
    for specs in &part_specs {
        let mut hrefs = Vec::new();
        let mut frefs = Vec::new();
        for sp in specs {
            // 与 header_keys 构造保持一致：优先文本，否则由块推导的纯文本
            let htext = sp.header.clone().or_else(|| {
                sp.header_blocks.as_ref().map(|bs| {
                    bs.iter()
                        .map(|b| b.plain_text())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
            });
            if let Some(text) = htext {
                if let Some(idx) = header_keys
                    .iter()
                    .position(|(k, t)| k == sp.kind && t == &text)
                {
                    hrefs.push((sp.kind.to_string(), ctx.header_rids[idx].clone()));
                }
            }
            if sp.footer {
                if let Some(idx) = footer_formats.iter().position(|f| f == &sp.footer_format) {
                    frefs.push((sp.kind.to_string(), ctx.footer_rids[idx].clone()));
                }
            }
        }
        ctx.part_header_refs.push(hrefs);
        ctx.part_footer_refs.push(frefs);
    }
    if any_footnotes {
        ctx.add_rel(
            crate::utils::constants::REL_FOOTNOTES,
            "footnotes.xml",
            false,
        );
    }
    if any_endnotes {
        ctx.add_rel(crate::utils::constants::REL_ENDNOTES, "endnotes.xml", false);
    }
    if any_comments {
        ctx.add_rel(crate::utils::constants::REL_COMMENTS, "comments.xml", false);
    }
    // 图表部件
    let chart_xmls: Vec<String> = charts.iter().map(chart::chart_xml).collect();
    for (i, xml) in chart_xmls.into_iter().enumerate() {
        let name = format!("charts/chart{}.xml", i + 1);
        let rid = ctx.add_rel(crate::utils::constants::REL_CHART, &name, false);
        ctx.chart_rids.push(rid);
        ctx.chart_parts.push((format!("chart{}.xml", i + 1), xml));
    }

    // 图片
    let mut seen_img: Vec<String> = Vec::new();
    // 透传保留的原始媒体名（避免生成文件名冲突导致 ZIP 重复条目）
    let mut used_media: std::collections::HashSet<String> = ctx
        .doc
        .passthrough
        .iter()
        .filter_map(|p| p.path.strip_prefix("word/media/").map(|s| s.to_string()))
        .collect();
    for src in srcs {
        if ctx.img_rid.contains_key(&src) {
            continue;
        }
        let bytes = match load_image_bytes(&src) {
            Ok(b) => b,
            Err(_) => {
                ctx.img_failed.insert(src.clone());
                continue;
            }
        };
        let ext = detect_image_extension(&src, &bytes);
        let (pw, ph) = image_size(&bytes).unwrap_or((200, 150));
        let mut idx = ctx.media.len() + 1;
        let name = loop {
            let cand = format!("image{idx}.{ext}");
            if !used_media.contains(&cand) {
                break cand;
            }
            idx += 1;
        };
        used_media.insert(name.clone());
        ctx.media.push(MediaFile {
            name: name.clone(),
            bytes,
        });
        let rid = ctx.add_rel(
            crate::utils::constants::REL_IMAGE,
            &format!("media/{name}"),
            false,
        );
        ctx.img_file.insert(src.clone(), name);
        ctx.img_rid.insert(src.clone(), rid);
        // 默认按 96dpi 换算为 pt
        ctx.img_size.insert(
            src.clone(),
            (pw as f64 * 72.0 / 96.0, ph as f64 * 72.0 / 96.0),
        );
        seen_img.push(src);
    }

    // 超链接
    for url in urls {
        if ctx.hyper_rid.contains_key(&url) {
            continue;
        }
        let rid = ctx.add_rel(crate::utils::constants::REL_HYPERLINK, &url, true);
        ctx.hyper_rid.insert(url, rid);
    }

    // 附件（OLE 嵌入对象）
    for a in collect_attachments(&ctx.doc) {
        if ctx.attach_embed_rid.contains_key(&a.src) {
            continue;
        }
        let passthrough_has = ctx.doc.passthrough.iter().any(|p| p.path == a.src);
        let bytes = match load_image_bytes(&a.src) {
            Ok(b) => b,
            Err(_) if passthrough_has => Vec::new(),
            Err(_) => {
                ctx.attach_failed.insert(a.src.clone());
                continue;
            }
        };
        let name = a
            .name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| a.src.rsplit('/').next().unwrap_or("embedded").to_string());
        let embed_part = format!("embeddings/{name}");
        let rid = ctx.add_rel(crate::utils::constants::REL_OLE_OBJECT, &embed_part, false);
        ctx.attach_embed_rid.insert(a.src.clone(), rid);
        if !bytes.is_empty() {
            ctx.embeddings.push((name.clone(), bytes));
        }
        ctx.attach_name.insert(a.src.clone(), name);
        ctx.attach_progid.insert(
            a.src.clone(),
            a.prog_id.clone().unwrap_or_else(|| prog_id_for(&a.src)),
        );

        // 预览图标 → media（保留真实格式，如 wmf/emf，避免类型不符导致渲染失败）
        let icon_path = a.icon.clone();
        let preview = match icon_path.as_deref().and_then(|p| load_image_bytes(p).ok()) {
            Some(b) => b,
            None => placeholder_icon_png(),
        };
        let ext = match &icon_path {
            Some(p) if !crate::utils::detect_image_extension(p, &preview).is_empty() => {
                crate::utils::detect_image_extension(p, &preview)
            }
            _ => "png".to_string(),
        };
        let pidx = ctx.media.len() + 1;
        let pname = format!("attach_icon{pidx}.{ext}");
        ctx.media.push(MediaFile {
            name: pname.clone(),
            bytes: preview,
        });
        let prid = ctx.add_rel(
            crate::utils::constants::REL_IMAGE,
            &format!("media/{pname}"),
            false,
        );
        ctx.attach_icon_rid.insert(a.src.clone(), prid);
        let w = a.width.unwrap_or(72.0);
        let h = a.height.unwrap_or(72.0);
        ctx.attach_size.insert(a.src.clone(), (w, h));
        ctx.attach_label
            .insert(a.src.clone(), a.label.clone().unwrap_or_default());
    }

    // 组装各部件
    let document_xml = body::document_xml(&ctx);
    let styles_xml = aux::styles_xml(&ctx);
    let numbering_xml = aux::numbering_xml(&ctx);
    let settings_xml = aux::settings_xml(&ctx);
    let theme_xml = aux::theme_xml(&ctx);
    let font_table_xml = aux::font_table_xml(&ctx);
    let web_settings_xml = aux::web_settings_xml();
    let core_xml = aux::core_xml(&ctx);
    let app_xml = aux::app_xml(&ctx);
    let header_parts: Vec<(String, String)> = ctx.header_parts.clone();
    let footer_parts: Vec<(String, String)> = ctx.footer_parts.clone();
    let footnotes_xml = if ctx.any_footnotes {
        Some(aux::footnotes_xml(&ctx))
    } else {
        None
    };
    let endnotes_xml = if ctx.any_endnotes {
        Some(aux::endnotes_xml(&ctx))
    } else {
        None
    };
    let comments_xml = if ctx.any_comments {
        Some(aux::comments_xml(&ctx))
    } else {
        None
    };
    let content_types = package::content_types(&ctx);
    let root_rels = package::root_rels();
    let doc_rels = package::document_rels(&ctx);

    // 打包
    let parts_count = ctx.doc.parts.len();
    package::write_zip(
        output,
        &[
            ("word/document.xml", document_xml),
            ("word/styles.xml", styles_xml),
            ("word/numbering.xml", numbering_xml),
            ("word/settings.xml", settings_xml),
            (
                "word/theme/theme1.xml",
                ctx.doc.theme_xml.clone().unwrap_or(theme_xml),
            ),
            ("word/fontTable.xml", font_table_xml),
            ("word/webSettings.xml", web_settings_xml),
            ("docProps/core.xml", core_xml),
            ("docProps/app.xml", app_xml),
        ],
        &header_parts,
        &footer_parts,
        footnotes_xml,
        endnotes_xml,
        comments_xml,
        &ctx.chart_parts,
        &ctx,
        content_types,
        root_rels,
        doc_rels,
    )?;

    Ok(GenerateResult {
        path: output.to_string(),
        parts: parts_count,
    })
}

/// 从产物目录重建 DOCX
pub fn repack(product_root: &str, output: &str) -> Result<GenerateResult> {
    let root = std::path::Path::new(product_root);
    let doc_path = root.join("document.json");
    let mut doc_json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&doc_path)?)?;

    // parts 支持两种形态：
    //  1) 文件路径数组（unpack 产物）：["word/parts/part1.json", ...]
    //  2) 内联对象数组（单文件 document.json）：[{ "blocks": [...] }, ...]
    let parts_value = doc_json.get("parts").cloned();
    let part_paths: Vec<String> = parts_value
        .as_ref()
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let inline_parts: Vec<crate::model::Part> = parts_value
        .as_ref()
        .and_then(|v| v.as_array())
        .filter(|a| a.iter().any(|e| e.is_object()))
        .map(|a| {
            a.iter()
                .filter_map(|e| serde_json::from_value(e.clone()).ok())
                .collect()
        })
        .unwrap_or_default();

    if let Some(obj) = doc_json.as_object_mut() {
        obj.insert("parts".to_string(), serde_json::Value::Array(vec![]));
    }

    let mut doc: Document = serde_json::from_value(doc_json)?;

    let mut loaded = Vec::new();
    if !part_paths.is_empty() {
        for p in &part_paths {
            let part: crate::model::Part =
                serde_json::from_str(&std::fs::read_to_string(root.join(p))?)?;
            loaded.push(part);
        }
    } else if !inline_parts.is_empty() {
        loaded = inline_parts;
    }
    if !loaded.is_empty() {
        doc.parts = loaded;
    }

    // 产物中的图片 src 为相对产物根，生成前解析为绝对路径
    absolutize_images(&mut doc, root);

    generate(&doc, output)
}

/// 把产物根内的相对图片 src 重写为绝对路径（URL 与绝对路径保持不变）
fn absolutize_images(doc: &mut Document, root: &std::path::Path) {
    fn rewrite(b: &mut Block, root: &std::path::Path) {
        match b {
            Block::Image(img) => {
                let src = &img.src;
                let is_url = src.starts_with("http://") || src.starts_with("https://");
                if !is_url && std::path::Path::new(src).is_relative() {
                    img.src = root.join(src).to_string_lossy().to_string();
                }
            }
            Block::Attachment(a) => {
                let is_url = a.src.starts_with("http://") || a.src.starts_with("https://");
                if !is_url && std::path::Path::new(&a.src).is_relative() {
                    a.src = root.join(&a.src).to_string_lossy().to_string();
                }
                if let Some(icon) = &a.icon {
                    let is_url = icon.starts_with("http://") || icon.starts_with("https://");
                    if !is_url && std::path::Path::new(icon).is_relative() {
                        a.icon = Some(root.join(icon).to_string_lossy().to_string());
                    }
                }
            }
            Block::Paragraph(p) => {
                if let Some(runs) = &mut p.runs {
                    for r in runs {
                        if let Some(img) = &mut r.image {
                            let is_url =
                                img.src.starts_with("http://") || img.src.starts_with("https://");
                            if !is_url && std::path::Path::new(&img.src).is_relative() {
                                img.src = root.join(&img.src).to_string_lossy().to_string();
                            }
                        }
                    }
                }
            }
            Block::List(l) => {
                for it in &mut l.items {
                    for b in &mut it.blocks {
                        rewrite(b, root);
                    }
                }
            }
            Block::Table(t) => {
                for r in &mut t.rows {
                    for c in &mut r.cells {
                        for b in &mut c.blocks {
                            rewrite(b, root);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    for part in &mut doc.parts {
        for b in &mut part.blocks {
            rewrite(b, root);
        }
    }
}

/// 递归扫描块中的图片源与超链接
fn scan_part(
    part: &crate::model::Part,
    srcs: &mut Vec<String>,
    urls: &mut Vec<String>,
    notes: &mut Vec<String>,
    enotes: &mut Vec<String>,
    comments: &mut Vec<(String, String)>,
) {
    for b in &part.blocks {
        scan_block(b, srcs, urls, notes, enotes, comments);
    }
}

/// 按生成顺序收集全部图表块
fn collect_charts(doc: &Document) -> Vec<ChartBlock> {
    fn walk(b: &Block, out: &mut Vec<ChartBlock>) {
        match b {
            Block::Chart(c) => out.push(c.clone()),
            Block::List(l) => {
                for it in &l.items {
                    for b in &it.blocks {
                        walk(b, out);
                    }
                }
            }
            Block::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        for b in &c.blocks {
                            walk(b, out);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for part in &doc.parts {
        for b in &part.blocks {
            walk(b, &mut out);
        }
    }
    out
}

/// 收集全部附件块（按生成顺序）
fn collect_attachments(doc: &Document) -> Vec<AttachmentBlock> {
    fn walk(b: &Block, out: &mut Vec<AttachmentBlock>) {
        match b {
            Block::Attachment(a) => out.push(a.clone()),
            Block::List(l) => {
                for it in &l.items {
                    for b in &it.blocks {
                        walk(b, out);
                    }
                }
            }
            Block::Table(t) => {
                for r in &t.rows {
                    for c in &r.cells {
                        for b in &c.blocks {
                            walk(b, out);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for part in &doc.parts {
        for b in &part.blocks {
            walk(b, &mut out);
        }
    }
    out
}

/// 按扩展名推断 OLE ProgID
fn prog_id_for(src: &str) -> String {
    let ext = src.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "xlsx" => "Excel.Sheet.12",
        "xls" => "Excel.Sheet.8",
        "docx" => "Word.Document.12",
        "doc" => "Word.Document.8",
        "pptx" => "PowerPoint.Show.12",
        "ppt" => "PowerPoint.Show.8",
        "pdf" => "AcroExch.Document.11",
        _ => "Package",
    }
    .to_string()
}

/// 生成占位预览图标（灰色 64×64 PNG）
fn placeholder_icon_png() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(64, 64, image::Rgba([180, 186, 194, 255]));
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .ok();
    bytes
}

fn scan_block(
    b: &Block,
    srcs: &mut Vec<String>,
    urls: &mut Vec<String>,
    notes: &mut Vec<String>,
    enotes: &mut Vec<String>,
    comments: &mut Vec<(String, String)>,
) {
    match b {
        Block::Image(i) => srcs.push(i.src.clone()),
        Block::List(l) => {
            for it in &l.items {
                for b in &it.blocks {
                    scan_block(b, srcs, urls, notes, enotes, comments);
                }
            }
        }
        Block::Table(t) => {
            for r in &t.rows {
                for c in &r.cells {
                    for b in &c.blocks {
                        scan_block(b, srcs, urls, notes, enotes, comments);
                    }
                }
            }
        }
        Block::Paragraph(p) => {
            if let Some(runs) = &p.runs {
                for r in runs {
                    if let Some(img) = &r.image {
                        srcs.push(img.src.clone());
                    }
                    if let Some(u) = &r.hyperlink {
                        if !u.starts_with('#') {
                            urls.push(u.clone());
                        }
                    }
                    if let Some(n) = &r.footnote {
                        notes.push(n.clone());
                    }
                    if let Some(n) = &r.endnote {
                        enotes.push(n.clone());
                    }
                    if let Some(c) = &r.comment {
                        let author = r
                            .comment_author
                            .clone()
                            .unwrap_or_else(|| "json2docx".to_string());
                        comments.push((author, c.clone()));
                    }
                }
            }
        }
        _ => {}
    }
}
