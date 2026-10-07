//! 跨格式转换：PDF ↔ Office 三格式（docx/pptx/xlsx）。
//!
//! 架构内双向映射，**产物目录是统一中间形态**（对齐 unpack/repack 的
//! 单一事实源设计），全程不依赖 LibreOffice：
//!
//! - **PDF → Office**（`convert docx/pptx/xlsx`）：`unpack` 产物目录
//!   （图片落 media/、meta/页几何直接复用）+ 行级数据（text::extract_pages，
//!   同一设计的产物）→ 结构推断（正文基准字号分层/列表标记，表格复用
//!   detect_table_rows 行聚类）→ `json2docx::Document` /
//!   `json2pptx::Presentation` / `json2xlsx::Workbook` → 各自 `generate()`。
//!   产物目录的 media 嵌入 office 文档，转换完临时目录自动清理。
//! - **Office → PDF**（`convert office --engine auto`）：soffice 探测到 →
//!   LibreOffice 引擎（样式保真）；未安装 → `native`：office JSON 模型 →
//!   临时产物目录 → `repack()`（自研简化排版，见 `office_to_pdf` doc）。
//!
//! 保真口径与 unpack 相同：**语义优先，非像素锁定**——旋转文本、裁剪、
//! 渐变/图案填充、复杂矢量路径在首版以简化形态表达或跳过。

use crate::model::{DocumentModel, Element, PageModel};
use crate::text::{detect_table_rows, extract_pages, PageText, TableMatch};
use anyhow::{anyhow, Result};
use json2docx::model::blocks::Block;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

// ── 公共输入：产物目录 + 行级数据 ───────────────────────────────────────

/// 打开待转换的 PDF：临时产物目录（图片已提取）+ 每页行数据与表格。
pub struct PdfSource {
    /// 产物目录根（media/ 内是已提取的图片文件）
    pub dir: PathBuf,
    pub model: DocumentModel,
    pub pages: Vec<PageData>,
}

pub struct PageData {
    pub width: f64,
    pub height: f64,
    /// 产物目录页面模型（image/rect 元素在此，src 为产物根相对路径）
    pub page: PageModel,
    pub pt: PageText,
    pub tables: Vec<TableMatch>,
}

impl Drop for PdfSource {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// unpack 到临时目录并装配行级数据（转换结束 Drop 自动清理）。
pub fn open_source(pdf: &Path) -> Result<PdfSource> {
    let dir = std::env::temp_dir().join(format!(
        "json2pdf-cross-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_millis())
            .unwrap_or(0)
    ));
    crate::unpack::unpack(pdf, &dir)?;
    let model: DocumentModel = serde_json::from_str(
        &std::fs::read_to_string(dir.join("document.json"))
            .map_err(|e| anyhow!("read document.json: {e}"))?,
    )
    .map_err(|e| anyhow!("parse document.json: {e}"))?;

    let mut doc = lopdf::Document::load(pdf).map_err(|e| anyhow!("load pdf: {e}"))?;
    if doc.is_encrypted() && doc.decrypt("").is_err() {
        anyhow::bail!("document is encrypted with a non-empty password — unsupported");
    }
    let page_nums: Vec<u32> = (1..=doc.get_pages().len() as u32).collect();
    let pts = extract_pages(&doc, &page_nums)?;
    if model.pages.len() != pts.len() {
        return Err(anyhow!(
            "产物目录页数 {} 与提取页数 {} 不一致",
            model.pages.len(),
            pts.len()
        ));
    }
    let all_tables = detect_table_rows(&pts);

    let mut pages = Vec::new();
    for (path, pt) in model.pages.iter().zip(pts) {
        let page_json: PageModel = serde_json::from_str(
            &std::fs::read_to_string(dir.join(path)).map_err(|e| anyhow!("read {}: {e}", path))?,
        )
        .map_err(|e| anyhow!("parse {}: {e}", path))?;
        let (w, h) = (
            page_json.width.unwrap_or(model.page_size.width),
            page_json.height.unwrap_or(model.page_size.height),
        );
        let tables = all_tables
            .iter()
            .filter(|t| t.page == pt.page)
            .cloned()
            .collect();
        pages.push(PageData {
            width: w,
            height: h,
            page: page_json,
            pt,
            tables,
        });
    }
    Ok(PdfSource { dir, model, pages })
}

// ── 页面事件统一排序（表格/图片/正文行按纵向位置交错出阅读序）──────────

enum Ev {
    Table(usize),
    Image(usize),
    Text(usize),
}

fn page_events(pd: &PageData) -> (Vec<Ev>, BTreeSet<usize>) {
    let mut evs: Vec<(f64, u8, Ev)> = Vec::new();
    for (i, t) in pd.tables.iter().enumerate() {
        let top = t
            .line_ids
            .iter()
            .flatten()
            .filter_map(|&id| pd.pt.lines.get(id).map(|l| l.y))
            .fold(f64::NEG_INFINITY, f64::max);
        if top.is_finite() {
            evs.push((top, 1, Ev::Table(i)));
        }
    }
    for (i, el) in pd.page.elements.iter().enumerate() {
        if let Element::Image { y, h, .. } = el {
            evs.push((y + h, 2, Ev::Image(i)));
        }
    }
    for (i, l) in pd.pt.lines.iter().enumerate() {
        evs.push((l.y, 3, Ev::Text(i)));
    }
    // y 降序（向下阅读）；同 y 时表格先于图片先于文本
    evs.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
    });
    (evs.into_iter().map(|(_, _, e)| e).collect(), {
        pd.tables
            .iter()
            .flat_map(|t| t.line_ids.iter().flatten().copied())
            .collect()
    })
}

/// 产物根相对路径 → 绝对路径（office generate 时按文件读取嵌入）
fn abs_media(dir: &Path, rel: &str) -> String {
    if Path::new(rel).is_absolute() {
        rel.to_string()
    } else {
        dir.join(rel).display().to_string()
    }
}

fn hex6(c: &str) -> String {
    let s = c.trim_start_matches('#');
    if s.len() == 6 {
        format!("#{s}")
    } else if s.len() == 3 {
        let e: String = s.chars().flat_map(|ch| [ch, ch]).collect();
        format!("#{e}")
    } else {
        "#000000".to_string()
    }
}

/// 正文基准字号 = 全页 item 字号**众数**（平数取小：正文行总是最多的，
/// 中位数在表格/标题主导的小页上会翻车——如 [14,12] 两行页取到 14）
pub fn body_size(pt: &PageText) -> f64 {
    use std::collections::BTreeMap;
    let mut counts: BTreeMap<u32, usize> = BTreeMap::new();
    for it in pt.lines.iter().flat_map(|l| &l.items) {
        // 半 pt 归桶避免浮点噪声
        *counts.entry((it.size * 2.0).round() as u32).or_insert(0) += 1;
    }
    if counts.is_empty() {
        return 12.0;
    }
    let best = counts
        .iter()
        .max_by_key(|(k, c)| (**c, -(**k as i64)))
        .map(|(k, _)| *k)
        .unwrap_or(24);
    best as f64 / 2.0
}

/// 行的整行文本（items 按原文顺序拼）
fn line_text(l: &crate::text::Line) -> String {
    l.items
        .iter()
        .map(|it| it.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn line_size(l: &crate::text::Line) -> f64 {
    l.items.iter().map(|i| i.size).fold(0.0, f64::max)
}

/// 列表项识别（bullet / 数字前缀），返回（是否有序，内容）
fn as_list(text: &str) -> Option<(bool, String)> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    for bullet in ["•", "·", "●", "■", "□", "○", "◦", "–", "-", "*"] {
        if let Some(rest) = t.strip_prefix(bullet) {
            let rest = rest.trim();
            if !rest.is_empty() {
                return Some((false, rest.to_string()));
            }
        }
    }
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if (1..=3).contains(&i) && i + 1 < bytes.len() && (bytes[i] == b'.' || bytes[i] == b')') {
        let rest = t[i + 1..].trim();
        if !rest.is_empty() {
            return Some((true, rest.to_string()));
        }
    }
    None
}

// ── PDF → DOCX ─────────────────────────────────────────────────────────

/// PDF → json2docx::Document。页序即分片序（页间 PageBreak）；
/// 标题按字号分层（≥1.9×正文=H1、≥1.6×=H2、≥1.35×=H3）；
/// bullet/数字前缀行识别为列表；表格走 detect_table_rows 行聚类；
/// 图片按原尺寸落块（产物目录 media 文件嵌入 docx）。
pub fn to_docx(src: &PdfSource) -> Result<json2docx::Document> {
    use json2docx::model::blocks::Block;
    use json2docx::model::PageSetup;

    let mut doc = json2docx::Document {
        meta: json2docx::model::Meta {
            title: src.model.meta.as_ref().and_then(|m| m.title.clone()),
            author: src.model.meta.as_ref().and_then(|m| m.author.clone()),
            subject: src.model.meta.as_ref().and_then(|m| m.subject.clone()),
            ..Default::default()
        },
        parts: Vec::new(),
        ..Default::default()
    };
    if let Some(p0) = src.pages.first() {
        // 首页尺寸 → Word 页面（custom；Word 无逐页尺寸概念）
        doc.page = PageSetup {
            size: "custom".to_string(),
            width: Some(p0.width),
            height: Some(p0.height),
            ..Default::default()
        };
    }
    let n = src.pages.len();
    for (i, pd) in src.pages.iter().enumerate() {
        let mut blocks = page_to_docx_blocks(pd, &src.dir);
        if i + 1 < n {
            blocks.push(Block::PageBreak);
        }
        doc.parts.push(json2docx::model::Part::new(blocks));
    }
    Ok(doc)
}

fn page_to_docx_blocks(pd: &PageData, dir: &Path) -> Vec<Block> {
    use json2docx::model::blocks::{Block, ListBlock, ListItem, Paragraph};
    let (events, table_lines) = page_events(pd);

    let mut out: Vec<Block> = Vec::new();
    let body = body_size(&pd.pt);
    // pending 状态机：段落与列表按"开组序号"决定收束时的落块次序
    let mut seq = 0u32;
    let mut para: Vec<String> = Vec::new();
    let mut para_size = 0.0f64;
    let mut para_seq: Option<u32> = None;
    let mut list: Option<(bool, Vec<String>, Option<u32>)> = None;
    let mut prev_y = f64::INFINITY;

    // 按开组序收束 pending（para_seq 与 list_seq 谁先开谁先落）
    macro_rules! flush_pending {
        () => {
            let para_open = para_seq.is_some();
            let list_open = list.is_some();
            if para_open && list_open {
                if para_seq.unwrap() <= list.as_ref().and_then(|l| l.2).unwrap_or(u32::MAX) {
                    flush_para_first!();
                } else {
                    flush_list_first!();
                }
            } else if para_open {
                flush_para_first!();
            } else if list_open {
                flush_list_first!();
            }
        };
    }
    macro_rules! flush_para_first {
        () => {
            if !para.is_empty() {
                out.push(Block::Paragraph(Paragraph {
                    text: Some(para.join(" ")),
                    ..Default::default()
                }));
                para.clear();
                #[allow(unused_assignments)]
                {
                    para_size = 0.0;
                    para_seq = None;
                }
            }
            flush_list!();
        };
    }
    macro_rules! flush_list_first {
        () => {
            if let Some((ordered, items, _)) = list.take() {
                out.push(Block::List(ListBlock {
                    ordered,
                    items: items
                        .into_iter()
                        .map(|t| ListItem {
                            blocks: vec![Block::Paragraph(Paragraph {
                                text: Some(t),
                                ..Default::default()
                            })],
                            ..Default::default()
                        })
                        .collect(),
                }));
            }
            if !para.is_empty() {
                out.push(Block::Paragraph(Paragraph {
                    text: Some(para.join(" ")),
                    ..Default::default()
                }));
                para.clear();
                #[allow(unused_assignments)]
                {
                    para_size = 0.0;
                    para_seq = None;
                }
            }
        };
    }
    macro_rules! flush_para {
        () => {
            flush_para_first!();
        };
    }
    macro_rules! flush_list {
        () => {
            flush_list_first!();
        };
    }

    for ev in events {
        match ev {
            Ev::Table(ti) => {
                flush_pending!();
                out.push(Block::Table(table_to_block(&pd.tables[ti])));
            }
            Ev::Image(ei) => {
                flush_pending!();
                if let Element::Image { src, w, h, .. } = &pd.page.elements[ei] {
                    out.push(Block::Image(json2docx::model::blocks::ImageBlock {
                        src: abs_media(dir, src),
                        width: Some(*w),
                        height: Some(*h),
                        ..Default::default()
                    }));
                }
            }
            Ev::Text(li) => {
                if table_lines.contains(&li) {
                    continue;
                }
                let line = &pd.pt.lines[li];
                let text = line_text(line);
                if text.is_empty() {
                    continue;
                }
                let size = line_size(line);
                // 标题优先于列表判定：整行字号显著大于正文时编号前缀不构成列表
                if size >= body * 1.15 {
                    flush_pending!();
                    let level = if size >= body * 1.9 {
                        1
                    } else if size >= body * 1.5 {
                        2
                    } else {
                        3
                    };
                    out.push(Block::Heading {
                        level,
                        text,
                        numbering: None,
                        align: None,
                        runs: None,
                    });
                    prev_y = line.y;
                    continue;
                }
                if let Some((ordered, content)) = as_list(&text) {
                    flush_para!();
                    match &mut list {
                        // 同类列表项合并进同一 ListBlock
                        Some((o, items, _)) if *o == ordered => items.push(content),
                        _ => {
                            flush_pending!();
                            list = Some((ordered, vec![content], Some(seq)));
                            seq += 1;
                        }
                    }
                    prev_y = line.y;
                    continue;
                }
                // 段落合并：字号跳变 >20% 或行距骤增（>1.9×字号）则收束
                let gap = prev_y - line.y;
                if para_size > 0.0
                    && ((size - para_size).abs() > para_size * 0.2 || gap > size * 1.9)
                {
                    flush_pending!();
                }
                if para.is_empty() {
                    para_size = size;
                    para_seq = Some(seq);
                    seq += 1;
                }
                para.push(text);
                prev_y = line.y;
            }
        }
    }
    flush_pending!();
    out
}

/// TableMatch → json2docx TableBlock（等宽列铺满可用宽，首行做表头）
fn table_to_block(t: &TableMatch) -> json2docx::model::blocks::TableBlock {
    use json2docx::model::blocks::{Paragraph, TableBlock, TableCell, TableRow};
    let cols = t.data.first().map(|r| r.len()).unwrap_or(1).max(1);
    TableBlock {
        header_row: true,
        widths: Some(vec![516.0 / cols as f64; cols]),
        rows: t
            .data
            .iter()
            .map(|cells| TableRow {
                cells: cells
                    .iter()
                    .map(|c| TableCell {
                        blocks: vec![Block::Paragraph(Paragraph {
                            text: Some(c.clone()),
                            ..Default::default()
                        })],
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

// ── PDF → PPTX ─────────────────────────────────────────────────────────

/// PDF → json2pptx::Presentation。页 → slide，绝对定位近无损：
/// 文本（近距行合并为文本框）/图片/矩形按原坐标与字号搬移；
/// 版面尺寸取首页；异构页整体缩放。旋转文本与矢量路径首版跳过。
pub fn to_pptx(src: &PdfSource) -> Result<json2pptx::Presentation> {
    use json2pptx::model::Element as PptElement;

    let Some(p0) = src.pages.first() else {
        return Ok(json2pptx::Presentation {
            slides: Vec::new(),
            ..minimal_presentation()
        });
    };
    let (pw, ph) = (p0.width.max(1.0), p0.height.max(1.0));
    let mut pres = json2pptx::Presentation {
        width: pw / 72.0,
        height: ph / 72.0,
        slides: Vec::new(),
        meta: src.model.meta.as_ref().map(|m| json2pptx::model::Meta {
            title: m.title.clone(),
            author: m.author.clone(),
            ..Default::default()
        }),
        ..minimal_presentation()
    };
    let src_dir = src.dir.clone();
    for pd in &src.pages {
        let sw = (pd.width / pw).max(0.01); // 页 → 版面比例（>1 = 页更大，需缩小）
                                            // 矩形先画（垫底），再图片，再文本（保持常见 z 序）
        let mut shapes: Vec<PptElement> = Vec::new();
        for el in &pd.page.elements {
            if let Element::Rect {
                x, y, w, h, fill, ..
            } = el
            {
                if *w * *h >= pd.width * pd.height * 0.96 {
                    continue;
                }
                shapes.push(PptElement::Shape(
                    json2pptx::model::elements::ShapeElement {
                        shape_type: "rect".to_string(),
                        position: pd_position(pd, pw, ph, *x, *y, *w, (*h).max(1.0)),
                        fill: fill
                            .as_ref()
                            .map(|c| json2pptx::model::elements::Fill::Solid(hex6(c))),
                        ..Default::default()
                    },
                ));
            }
        }
        let mut images: Vec<PptElement> = Vec::new();
        for el in &pd.page.elements {
            if let Element::Image {
                src, x, y, w, h, ..
            } = el
            {
                images.push(PptElement::Image(
                    json2pptx::model::elements::ImageElement {
                        src: abs_media(&src_dir, src),
                        position: pd_position(pd, pw, ph, *x, *y, *w, (*h).max(1.0)),
                        ..Default::default()
                    },
                ));
            }
        }
        let mut texts: Vec<PptElement> = Vec::new();
        for (x0, x1, y_top_base, h_pt, size, text) in text_groups(pd) {
            texts.push(PptElement::Text(json2pptx::model::elements::TextElement {
                text: json2pptx::model::elements::TextContent::Simple(text),
                position: json2pptx::model::elements::Position {
                    x: (x0 / 72.0) / sw,
                    y: ((pd.height - y_top_base) / 72.0) / sw,
                    w: ((x1 - x0) / 72.0).max(0.4) / sw,
                    h: (h_pt / 72.0).max(0.2) / sw,
                },
                font_size: Some(size / sw),
                color: Some("#1a1a1a".to_string()),
                ..Default::default()
            }));
        }
        pres.slides.push(json2pptx::model::Slide {
            elements: [shapes, images, texts].concat(),
            background: page_background(pd).map(serde_json::Value::String),
            transition: None,
            notes: None,
        });
    }
    Ok(pres)
}

/// Presentation 其余字段的最小构造值
fn minimal_presentation() -> json2pptx::Presentation {
    json2pptx::Presentation {
        width: 13.333,
        height: 7.5,
        meta: None,
        template: None,
        theme: None,
        slides: Vec::new(),
        table_styles: None,
    }
}

fn pd_position(
    pd: &PageData,
    pw: f64,
    ph: f64,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
) -> json2pptx::model::elements::Position {
    let pw = pw.max(1.0);
    let ph = ph.max(1.0);
    let sw = (pd.width / pw).max(0.01);
    let sh = (pd.height / ph).max(0.01);
    json2pptx::model::elements::Position {
        x: x / 72.0 / sw,
        y: (pd.height - y - h) / 72.0 / sh,
        w: w / 72.0 / sw,
        h: h / 72.0 / sh,
    }
}

/// 文本分组产出：(x0, x1, 顶基线 y, 高度 pt, 字号, 内容 \n 连接)
fn text_groups(pd: &PageData) -> Vec<(f64, f64, f64, f64, f64, String)> {
    let mut sorted: Vec<usize> = (0..pd.pt.lines.len()).collect();
    sorted.sort_by(|&a, &b| {
        pd.pt.lines[b]
            .y
            .partial_cmp(&pd.pt.lines[a].y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for i in sorted {
        let l = &pd.pt.lines[i];
        let append = match groups.last_mut() {
            Some(g) => {
                let prev = &pd.pt.lines[*g.last().unwrap()];
                (prev.y - l.y) <= (line_size(prev) + line_size(l)) * 0.9
            }
            None => false,
        };
        if append {
            groups.last_mut().unwrap().push(i);
        } else {
            groups.push(vec![i]);
        }
    }
    groups
        .into_iter()
        .filter(|g| !g.is_empty())
        .map(|g| {
            let mut x0 = f64::INFINITY;
            let mut x1 = f64::NEG_INFINITY;
            let mut y_top = f64::NEG_INFINITY;
            let mut y_bot = f64::INFINITY;
            let mut size = 0.0f64;
            let mut lines: Vec<String> = Vec::new();
            for &i in &g {
                let l = &pd.pt.lines[i];
                for it in &l.items {
                    x0 = x0.min(it.x);
                    x1 = x1.max(it.end_x);
                }
                y_top = y_top.max(l.y);
                y_bot = y_bot.min(l.y - line_size(l));
                size = size.max(line_size(l));
                let t = line_text(l);
                if !t.is_empty() {
                    lines.push(t);
                }
            }
            (
                x0,
                x1,
                y_top,
                y_top - y_bot + size * 0.5,
                size,
                lines.join("\n"),
            )
        })
        .collect()
}

/// 全页底色矩形（≥96% 页面）作为幻灯片背景色
fn page_background(pd: &PageData) -> Option<String> {
    pd.page.elements.iter().find_map(|el| match el {
        Element::Rect {
            x,
            y,
            w,
            h,
            fill: Some(c),
            ..
        } if *x <= 1.0 && *y <= 1.0 && *w >= pd.width * 0.96 && *h >= pd.height * 0.96 => {
            Some(hex6(c))
        }
        _ => None,
    })
}

// ── PDF → XLSX ─────────────────────────────────────────────────────────

/// PDF → json2xlsx::Workbook。每张识别表格一个 sheet（`P{页}T{序}`，首行
/// 表头加粗）；无表格的文本页 → 单列 sheet（`Page {n}`）；数字尝试转数值。
pub fn to_xlsx(src: &PdfSource) -> Result<json2xlsx::Workbook> {
    let mut wb = json2xlsx::Workbook::default();
    wb.meta = Some(json2xlsx::model::Meta {
        title: src.model.meta.as_ref().and_then(|m| m.title.clone()),
        author: src.model.meta.as_ref().and_then(|m| m.author.clone()),
    });
    wb.styles.insert(
        "hdr".to_string(),
        json2xlsx::model::StyleDef {
            font: Some(json2xlsx::model::Font {
                bold: true,
                ..Default::default()
            }),
            ..Default::default()
        },
    );
    for pd in &src.pages {
        if pd.pt.lines.is_empty() && pd.tables.is_empty() {
            continue;
        }
        if pd.tables.is_empty() {
            wb.sheets.push(text_page_sheet(pd));
        }
        for (ti, t) in pd.tables.iter().enumerate() {
            let mut rows = Vec::new();
            for (r, cells) in t.data.iter().enumerate() {
                rows.push(xlsx_row(
                    pd,
                    cells,
                    (r + 1) as u32,
                    if r == 0 { Some("hdr") } else { None },
                ));
            }
            wb.sheets.push(json2xlsx::model::Sheet {
                name: format!("P{}T{}", pd.pt.page, ti + 1),
                rows,
                ..Default::default()
            });
        }
    }
    Ok(wb)
}

fn text_page_sheet(pd: &PageData) -> json2xlsx::model::Sheet {
    let mut sorted: Vec<usize> = (0..pd.pt.lines.len()).collect();
    sorted.sort_by(|&a, &b| {
        pd.pt.lines[b]
            .y
            .partial_cmp(&pd.pt.lines[a].y)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut rows = Vec::new();
    for (r, &i) in sorted.iter().enumerate() {
        let text = line_text(&pd.pt.lines[i]);
        if text.is_empty() {
            continue;
        }
        rows.push(xlsx_row(pd, &[text], (r + 1) as u32, None));
    }
    json2xlsx::model::Sheet {
        name: format!("Page {}", pd.pt.page),
        rows,
        ..Default::default()
    }
}

fn xlsx_row(
    _pd: &PageData,
    cells: &[String],
    index: u32,
    style: Option<&str>,
) -> json2xlsx::model::Row {
    json2xlsx::model::Row {
        index: Some(index),
        cells: cells
            .iter()
            .enumerate()
            .filter(|(_, c)| !c.trim().is_empty())
            .map(|(c, v)| json2xlsx::model::Cell {
                reference: Some(a1_ref(index, c + 1)),
                value: Some(cell_value(v)),
                style: style.map(|s| json2xlsx::model::CellStyleRef::Named(s.to_string())),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

fn cell_value(text: &str) -> serde_json::Value {
    let t = text.trim();
    // 纯数字（含负/小数/千分位）→ 数值
    let numeric = t.replace(',', "");
    if matches!(
        t.as_bytes().first(),
        Some(b'0'..=b'9') | Some(b'-') | Some(b'+')
    ) && !t.contains(' ')
        && numeric.parse::<f64>().is_ok()
        && t.chars().any(|c| c.is_ascii_digit())
    {
        if let Ok(n) = numeric.parse::<f64>() {
            return serde_json::json!(n);
        }
    }
    serde_json::Value::String(t.to_string())
}

fn a1_ref(row: u32, col: usize) -> String {
    col_letter(col as u32) + &row.to_string()
}

/// 1 基列号 → 字母（"A"=1）
fn col_letter(mut col: u32) -> String {
    let mut name = String::new();
    while col > 0 {
        name.insert(0, (b'A' + ((col - 1) % 26) as u8) as char);
        col = (col - 1) / 26;
    }
    name
}

// ── Office → PDF（native 自研渲染兜底）─────────────────────────────────

/// office 文档 → PDF（自研简化排版，`convert office --engine native`）。
///
/// 保真边界（LibreOffice 缺席时的可读兜底，非打印级）：
/// - docx：标题字号分层/段落折行（CJK 1em、拉丁 Helvetica 分组估宽）/
///   列表悬挂缩进/表格等宽栅格/图片按文件嵌入（Parse 需要 unpack 出的
///   `word/media` 文件）；公式/图表/目录域等复杂块以文本兜底或跳过。
/// - pptx：绝对定位近无损（文本框/矩形/图片；含 slide 背景色）。
/// - xlsx：值渲染（列等宽、行高 17pt、栅格线；公式取缓存值样式直出）。
pub fn office_to_pdf(input: &Path, out: &Path) -> Result<usize> {
    let ext = input
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let unpack_dir = std::env::temp_dir().join(format!(
        "json2pdf-native-unpack-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ));
    match ext.as_str() {
        "docx" => {
            let _ = json2docx::unpack(
                &input.display().to_string(),
                &unpack_dir.display().to_string(),
            );
            let doc = json2docx::parse(&input.display().to_string())?;
            let r = docx_to_pdf(&doc, &unpack_dir, out);
            let _ = std::fs::remove_dir_all(&unpack_dir);
            r
        }
        "pptx" | "ppt" => {
            let _ = json2pptx::unpack(
                &input.display().to_string(),
                &unpack_dir.display().to_string(),
            );
            let pres = json2pptx::parse(&input.display().to_string())?;
            let r = pptx_to_pdf(&pres, &unpack_dir, out);
            let _ = std::fs::remove_dir_all(&unpack_dir);
            r
        }
        "xlsx" | "xlsm" | "xls" => {
            let wb = json2xlsx::parse(input)?;
            xlsx_to_pdf(&wb, out)
        }
        _ => Err(anyhow!("unsupported input type .{ext}")),
    }
}

/// 渲染结果装配：写入临时产物目录并 repack（返回页数）。
fn write_product(
    dir: &Path,
    mut model: DocumentModel,
    pages: Vec<crate::model::PageModel>,
    out: &Path,
) -> Result<usize> {
    std::fs::create_dir_all(dir.join("pages")).map_err(|e| anyhow!("mkdir: {e}"))?;
    for (i, p) in pages.iter().enumerate() {
        let rel = format!("pages/page-{:03}.json", i + 1);
        std::fs::write(
            dir.join(&rel),
            serde_json::to_string_pretty(p).map_err(|e| anyhow!(e))?,
        )
        .map_err(|e| anyhow!("write {}: {e}", rel))?;
        model.pages.push(rel);
    }
    std::fs::write(
        dir.join("document.json"),
        serde_json::to_string_pretty(&model).map_err(|e| anyhow!(e))?,
    )?;
    crate::repack::repack(dir, out)?;
    Ok(pages.len())
}

/// docx ImageBlock.src（word/media/xxx）→ 复制到临时产物 media/，返回 "media/xxx"
fn copy_media(dir: &Path, src: &str, builder_dir: &Path) -> Option<String> {
    let file = dir.join(src);
    if !file.exists() {
        return None;
    }
    let name = file.file_name()?.to_string_lossy().to_string();
    let media_dir = builder_dir.join("media");
    let _ = std::fs::create_dir_all(&media_dir);
    let dest = media_dir.join(&name);
    std::fs::copy(&file, &dest).ok()?;
    Some(format!("media/{name}"))
}

fn temp_build_dir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "json2pdf-native-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0)
    ))
}

/// 文本宽度（pt）＝ em 宽估计 × 字号
fn text_width_pt(text: &str, size: f64) -> f64 {
    crate::text::text_width_em(text) * size
}

/// 贪心折行：拉丁在空格断、CJK 逐字断；超长英文单词强制断
fn wrap_text(text: &str, size: f64, max_w: f64) -> Vec<String> {
    if max_w <= 20.0 || text.is_empty() {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0.0;
    for ch in text.chars() {
        let cw = crate::text::text_width_em(&ch.to_string()) * size;
        if cur_w + cw > max_w && !cur.is_empty() {
            if ch.is_ascii_whitespace() {
                lines.push(std::mem::take(&mut cur));
                cur_w = 0.0;
                continue;
            }
            // 尝试在最近的空格处回退断行（拉丁长词更自然）
            if let Some(pos) = cur.rfind(' ') {
                if pos > 0 {
                    let rest = cur.split_off(pos + 1);
                    cur_w = text_width_pt(rest.trim_start(), size);
                    lines.push(std::mem::take(&mut cur));
                    cur = rest.trim_start().to_string();
                    cur.push(ch);
                    cur_w += cw;
                    continue;
                }
            }
            lines.push(std::mem::take(&mut cur));
            cur_w = 0.0;
        }
        cur.push(ch);
        cur_w += cw;
    }
    if !cur.trim().is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// 文本字体选择：WinAnsi 可编码（含 •/–/© 等）→ Helvetica 零嵌入；
/// 其余非 ASCII → 宋体（fonts.rs 平台候选表解析本机 CJK TrueType）
fn pick_font(text: &str) -> &'static str {
    if text
        .chars()
        .all(|c| crate::repack::winansi_byte(c).is_some())
    {
        "Helvetica"
    } else {
        "system:宋体"
    }
}

/// 文本块推进（在 cur 页写行，返回新 y 光标）；空间不足则开新页。
/// Element::Text 的最简构造（默认填充文本态字段）
#[allow(clippy::too_many_arguments)]
fn text_el(
    text: String,
    x: f64,
    y: f64,
    size: f64,
    font: &str,
    color: &str,
) -> crate::model::Element {
    crate::model::Element::Text {
        text,
        x,
        y,
        size,
        font: font.to_string(),
        font_id: None,
        color: color.to_string(),
        rotation: 0.0,
        mirror: false,
        codes: None,
        code_bytes: 1,
        render_mode: 0,
        stroke_color: None,
        stroke_width: 1.0,
        word_spacing: 0.0,
        char_spacing: 0.0,
        h_scale: 1.0,
        tm: None,
        alpha: 1.0,
        blend: None,
        smask: None,
        bold: false,
        italic: false,
    }
}

/// Element::Rect 的最简构造
#[allow(clippy::too_many_arguments)]
fn rect_el(
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    fill: Option<String>,
    stroke: Option<String>,
    line_width: f64,
) -> crate::model::Element {
    crate::model::Element::Rect {
        x,
        y,
        w,
        h,
        fill,
        stroke,
        line_width,
        rotation: 0.0,
        alpha: 1.0,
        stroke_alpha: None,
        blend: None,
        smask: None,
    }
}

/// Element::Image 的最简构造
fn image_el(src: String, x: f64, y: f64, w: f64, h: f64) -> crate::model::Element {
    crate::model::Element::Image {
        src,
        preview: None,
        x,
        y,
        w,
        h,
        rotation: 0.0,
        alpha: 1.0,
        blend: None,
        clip: None,
        smask: None,
        smask_params: None,
        passthrough: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn flow_text(
    pages: &mut Vec<crate::model::PageModel>,
    ph: f64,
    top: f64,
    bottom: f64,
    x: f64,
    y: &mut f64,
    lines: &[String],
    size: f64,
    color: &str,
    align: Option<&str>,
    usable_w: f64,
) {
    for line in lines {
        let h = size * 1.38;
        if *y < bottom + h {
            let mut np = crate::model::PageModel::default();
            np.width = pages.first().and_then(|p| p.width);
            np.height = Some(ph);
            pages.push(np);
            *y = ph - top;
        }
        let w = text_width_pt(line, size);
        let lx = match align {
            Some("center") => x + (usable_w - w).max(0.0) / 2.0,
            Some("right") => x + (usable_w - w).max(0.0),
            _ => x,
        };
        let el = text_el(line.clone(), lx, *y - h, size, pick_font(line), color);
        pages.last_mut().unwrap().elements.push(el);
        *y -= h;
    }
}

/// docx → PDF（流式简化排版）。
fn docx_to_pdf(doc: &json2docx::Document, unpacked: &Path, out: &Path) -> Result<usize> {
    let build_dir = temp_build_dir("docx");
    let _ = std::fs::create_dir_all(&build_dir);
    use json2docx::model::blocks::Block;
    let (pw, ph) = doc.page.size_pt();
    let (mt, mb, ml, mr) = (
        doc.page.margins.top,
        doc.page.margins.bottom,
        doc.page.margins.left,
        doc.page.margins.right,
    );
    let usable = (pw - ml - mr).max(100.0);
    let model = DocumentModel {
        page_size: crate::model::PageSize {
            width: pw,
            height: ph,
        },
        meta: doc.meta.title.clone().map(|t| crate::model::Meta {
            title: Some(t),
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut pages: Vec<crate::model::PageModel> = Vec::new();
    let mut np = crate::model::PageModel::default();
    np.width = Some(pw);
    np.height = Some(ph);
    pages.push(np);
    let mut y = ph - mt;

    let body = doc
        .defaults
        .as_ref()
        .and_then(|s| s.font_size)
        .unwrap_or(11.0);
    for part in &doc.parts {
        for block in &part.blocks {
            match block {
                Block::PageBreak => {
                    let mut np = crate::model::PageModel::default();
                    np.width = Some(pw);
                    np.height = Some(ph);
                    pages.push(np);
                    y = ph - mt;
                }
                Block::Heading {
                    level, text, align, ..
                } => {
                    let size = body
                        * match level {
                            1 => 1.8,
                            2 => 1.45,
                            3 => 1.2,
                            4 => 1.1,
                            5 => 1.0,
                            _ => 0.95,
                        };
                    let lines = wrap_text(text, size, usable);
                    flow_text(
                        &mut pages,
                        ph,
                        mt,
                        mb,
                        ml,
                        &mut y,
                        &lines,
                        size,
                        "#111111",
                        align.as_deref(),
                        usable,
                    );
                    y -= size * 0.35;
                }
                Block::Paragraph(p) => {
                    let text = p.text.clone().unwrap_or_default();
                    let lines = wrap_text(&text, body, usable - p.indent.unwrap_or(0.0));
                    let indent = ml + p.indent.unwrap_or(0.0);
                    flow_text(
                        &mut pages,
                        ph,
                        mt,
                        mb,
                        indent,
                        &mut y,
                        &lines,
                        body,
                        "#1a1a1a",
                        p.align.as_deref(),
                        usable,
                    );
                }
                Block::Quote(q) => {
                    let lines = wrap_text(&q.text, body, usable - 24.0);
                    y -= body * 0.4;
                    flow_text(
                        &mut pages,
                        ph,
                        mt,
                        mb,
                        ml + 24.0,
                        &mut y,
                        &lines,
                        body,
                        "#555555",
                        None,
                        usable - 24.0,
                    );
                }
                Block::Caption(c) => {
                    let lines = wrap_text(&c.text, body * 0.9, usable);
                    flow_text(
                        &mut pages,
                        ph,
                        mt,
                        mb,
                        ml,
                        &mut y,
                        &lines,
                        body * 0.9,
                        "#555555",
                        Some("center"),
                        usable,
                    );
                }
                Block::Code(c) => {
                    for line in c.text.lines() {
                        let size = body * 0.95;
                        if y < mb + size * 1.38 {
                            let mut np = crate::model::PageModel::default();
                            np.width = Some(pw);
                            np.height = Some(ph);
                            pages.push(np);
                            y = ph - mt;
                        }
                        let el = text_el(
                            line.to_string(),
                            ml + 12.0,
                            y - size * 1.38,
                            size,
                            "Courier",
                            "#333333",
                        );
                        pages.last_mut().unwrap().elements.push(el);
                        y -= size * 1.38;
                    }
                }
                Block::List(l) => {
                    for (i, item) in l.items.iter().enumerate() {
                        let item_text = item
                            .blocks
                            .iter()
                            .filter_map(|b| match b {
                                Block::Paragraph(p) => p.text.clone(),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        let prefix = if l.ordered {
                            format!("{}. ", i + 1)
                        } else {
                            "•  ".to_string()
                        };
                        let indent = 16.0 + item.level as f64 * 18.0;
                        let lines =
                            wrap_text(&format!("{prefix}{item_text}"), body, usable - indent);
                        flow_text(
                            &mut pages,
                            ph,
                            mt,
                            mb,
                            ml + indent,
                            &mut y,
                            &lines,
                            body,
                            "#1a1a1a",
                            None,
                            usable - indent,
                        );
                        y -= body * 0.25;
                    }
                }
                Block::Table(t) => {
                    let cols = t.rows.first().map(|r| r.cells.len()).unwrap_or(1).max(1);
                    let given = t.widths.clone();
                    let widths: Vec<f64> = match &given {
                        Some(ws) if ws.len() == cols => {
                            let s: f64 = ws.iter().sum();
                            ws.iter().map(|w| w / s * usable).collect()
                        }
                        _ => vec![usable / cols as f64; cols],
                    };
                    let cell_size = t.font_size.unwrap_or(body * 0.95);
                    for row in &t.rows {
                        // 行高 = 单元格最大折行数
                        let cell_lines: Vec<Vec<String>> = row
                            .cells
                            .iter()
                            .enumerate()
                            .map(|(ci, c)| {
                                let text = c
                                    .blocks
                                    .iter()
                                    .filter_map(|b| match b {
                                        Block::Paragraph(p) => p.text.clone(),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ");
                                wrap_text(
                                    &text,
                                    cell_size,
                                    widths.get(ci).copied().unwrap_or(72.0) - 8.0,
                                )
                            })
                            .collect();
                        let row_h = cell_lines.iter().map(|ls| ls.len()).max().unwrap_or(1) as f64
                            * cell_size
                            * 1.3
                            + 5.0;
                        if y - row_h < mb {
                            let mut np = crate::model::PageModel::default();
                            np.width = Some(pw);
                            np.height = Some(ph);
                            pages.push(np);
                            y = ph - mt;
                        }
                        let mut x = ml;
                        let row_top = y;
                        for (ci, cells_l) in cell_lines.iter().enumerate() {
                            let w = widths[ci];
                            let row = row.cells.get(ci);
                            let fill = row.and_then(|c| c.fill.clone());
                            let el = rect_el(
                                x,
                                row_top - row_h,
                                w,
                                row_h,
                                fill,
                                Some("#999999".to_string()),
                                0.7,
                            );
                            pages.last_mut().unwrap().elements.push(el);
                            let mut ty = row_top - 5.0 - cell_size * 1.1;
                            for line in cells_l {
                                let el = text_el(
                                    line.clone(),
                                    x + 4.0,
                                    ty,
                                    cell_size,
                                    pick_font(line),
                                    "#111111",
                                );
                                pages.last_mut().unwrap().elements.push(el);
                                ty -= cell_size * 1.3;
                            }
                            x += w;
                        }
                        y -= row_h;
                    }
                    y -= body * 0.5;
                }
                Block::Image(img) => {
                    // 产物相对路径 word/media/xxx → 复制进临时产物 media/
                    if let Some(rel) = copy_media(unpacked, &img.src, &build_dir) {
                        let (w, h) = (
                            img.width.unwrap_or(240.0).min(usable),
                            img.height.unwrap_or(180.0),
                        );
                        if y - h < mb {
                            let mut np = crate::model::PageModel::default();
                            np.width = Some(pw);
                            np.height = Some(ph);
                            pages.push(np);
                            y = ph - mt;
                        }
                        let el = image_el(rel, ml, y - h, w, h);
                        pages.last_mut().unwrap().elements.push(el);
                        y -= h + body * 0.5;
                    }
                }
                Block::Toc(t) => {
                    let text = t
                        .entries
                        .iter()
                        .map(|e| e.text.clone())
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !text.is_empty() {
                        let lines = wrap_text(&text, body, usable);
                        flow_text(
                            &mut pages, ph, mt, mb, ml, &mut y, &lines, body, "#1a1a1a", None,
                            usable,
                        );
                    }
                }
                Block::Bibliography(_)
                | Block::Formula(_)
                | Block::Chart(_)
                | Block::TextBox(_)
                | Block::Attachment(_)
                | Block::Shape(_)
                | Block::Raw(_)
                | Block::Sdt(_) => {
                    // 首版兜底边界：复杂块（公式/图表/图形/嵌入）native 渲染跳过，
                    // 保真需求走 --engine soffice（本机 LibreOffice）
                }
            }
        }
    }
    write_product(&build_dir, model, pages, out)
}

/// 段落文本拼合（TextContent 的多段结构 → 换行连接）
fn ppt_text(content: &json2pptx::model::elements::TextContent) -> String {
    match content {
        json2pptx::model::elements::TextContent::Simple(s) => s.clone(),
        json2pptx::model::elements::TextContent::Paragraphs(ps) => ps
            .iter()
            .filter_map(|p| p.text.clone())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// pptx → PDF（绝对定位近无损）。
fn pptx_to_pdf(pres: &json2pptx::Presentation, unpacked: &Path, out: &Path) -> Result<usize> {
    let build_dir = temp_build_dir("pptx");
    let _ = std::fs::create_dir_all(build_dir.join("media"));
    let (pw, ph) = (pres.width * 72.0, pres.height * 72.0);
    let model = DocumentModel {
        page_size: crate::model::PageSize {
            width: pw,
            height: ph,
        },
        meta: pres
            .meta
            .as_ref()
            .and_then(|m| m.title.clone())
            .map(|t| crate::model::Meta {
                title: Some(t),
                ..Default::default()
            }),
        ..Default::default()
    };
    let mut pages: Vec<crate::model::PageModel> = Vec::new();

    for slide in &pres.slides {
        let mut page = crate::model::PageModel {
            width: Some(pw),
            height: Some(ph),
            ..Default::default()
        };
        // 背景（字符串=纯色）：全页矩形垫底
        if let Some(bg) = &slide.background {
            if let Some(c) = bg.as_str() {
                let el = rect_el(0.0, 0.0, pw, ph, Some(hex6(c)), None, 0.0);
                page.elements.push(el);
            }
        }
        use json2pptx::model::Element as PptElement;
        let px = |v: f64| v * 72.0;
        for el in &slide.elements {
            match el {
                PptElement::Text(t) => {
                    let text = ppt_text(&t.text);
                    if text.trim().is_empty() {
                        continue;
                    }
                    let size = t.font_size.unwrap_or(12.0).max(4.0);
                    let x = px(t.position.x);
                    let top = ph - px(t.position.y);
                    let color = t
                        .color
                        .clone()
                        .map(|c| hex6(&c))
                        .unwrap_or_else(|| "#1a1a1a".to_string());
                    let gap = size * 1.3;
                    for (i, line) in text.split('\n').enumerate() {
                        if line.trim().is_empty() {
                            continue;
                        }
                        page.elements.push(text_el(
                            line.trim_start().to_string(),
                            x + 8.0,
                            top - size - i as f64 * gap,
                            size,
                            pick_font(line),
                            &color,
                        ));
                    }
                }
                PptElement::Image(img) => {
                    // src 由打包器解析；unpack 出的产物中可能直接存在
                    if let Some(rel) = copy_ppt_media(unpacked, &img.src, &build_dir) {
                        page.elements.push(image_el(
                            rel,
                            px(img.position.x),
                            ph - px(img.position.y) - px(img.position.h).max(1.0),
                            px(img.position.w),
                            px(img.position.h).max(1.0),
                        ));
                    }
                }
                PptElement::Shape(sh) if sh.shape_type == "rect" => {
                    let fill = match &sh.fill {
                        Some(json2pptx::model::elements::Fill::Solid(c)) => Some(hex6(c)),
                        _ => None,
                    };
                    page.elements.push(rect_el(
                        px(sh.position.x),
                        ph - px(sh.position.y) - px(sh.position.h).max(1.0),
                        px(sh.position.w),
                        px(sh.position.h).max(1.0),
                        fill,
                        None,
                        0.0,
                    ));
                }
                _ => {
                    // 首版兜底边界：矢量形状/样式派生元素（style 子类）等跳过，
                    // 保真需求走 --engine soffice（本机 LibreOffice）
                }
            }
        }
        pages.push(page);
    }
    write_product(&build_dir, model, pages, out)
}

/// pptx 图片 src → 在 unpack 产物中定位文件并复制（返回 "media/xxx"）。
/// unpack 后 src 形如 "pptx/media/image1.png"（相对产物根）或绝对路径。
fn copy_ppt_media(unpacked: &Path, src: &str, build_dir: &Path) -> Option<String> {
    let mut file = PathBuf::from(src);
    if !file.exists() {
        file = unpacked.join(src);
    }
    if !file.exists() {
        // 尝试 media/ 直接命中
        let base = file.file_name()?;
        let cand = unpacked.join("pptx").join("media").join(base);
        if cand.exists() {
            file = cand;
        } else {
            return None;
        }
    }
    let name = file.file_name()?.to_string_lossy().to_string();
    let media_dir = build_dir.join("media");
    let _ = std::fs::create_dir_all(&media_dir);
    let dest = media_dir.join(&name);
    std::fs::copy(&file, &dest).ok()?;
    Some(format!("media/{name}"))
}

/// xlsx → PDF（值渲染：等宽列、栅格线、行高 17pt，逐 sheet 分页）。
/// 列宽取 sheet.columns 声明（字符宽 × 7pt + 5），缺省 82pt；数字右对齐。
fn xlsx_to_pdf(wb: &json2xlsx::Workbook, out: &Path) -> Result<usize> {
    let build_dir = temp_build_dir("xlsx");
    let _ = std::fs::create_dir_all(build_dir.join("media"));
    let (pw, ph) = (842.0, 595.0); // A4 横放
    let (mt, mb, ml, _mr) = (54.0, 54.0, 48.0, 48.0);
    let model = DocumentModel {
        page_size: crate::model::PageSize {
            width: pw,
            height: ph,
        },
        meta: wb
            .meta
            .as_ref()
            .and_then(|m| m.title.clone())
            .map(|t| crate::model::Meta {
                title: Some(t),
                ..Default::default()
            }),
        ..Default::default()
    };
    let mut pages: Vec<crate::model::PageModel> = Vec::new();
    let row_h = 17.0;
    let header_h = 20.0;
    let rows_per_page = (((ph - mt - mb - header_h) / row_h) as usize).max(1);

    for sheet in &wb.sheets {
        let mut rows: Vec<&json2xlsx::model::Row> = sheet.rows.iter().collect();
        rows.sort_by_key(|r| r.index.unwrap_or(u32::MAX));
        if rows.is_empty() {
            let mut page = crate::model::PageModel {
                width: Some(pw),
                height: Some(ph),
                ..Default::default()
            };
            let el = text_el(
                sheet.name.clone(),
                ml,
                ph - mt - 14.0,
                14.0,
                "Helvetica",
                "#666666",
            );
            page.elements.push(el);
            pages.push(page);
            continue;
        }
        // 列宽：sheet.columns 按顺序覆盖（首列起 1 基连排），缺省 82pt
        let default_w = 82.0;
        let mut col_w: std::collections::BTreeMap<usize, f64> = std::collections::BTreeMap::new();
        for (i, col) in sheet.columns.iter().enumerate() {
            if let Some(w) = col.width {
                col_w.insert(i + 1, w * 7.0 + 5.0);
            }
        }
        let max_col = rows
            .iter()
            .map(|r| r.cells.iter().map(cell_col).max().unwrap_or(1))
            .chain(std::iter::once(1))
            .max()
            .unwrap_or(1);
        // 分页维度：列页 × 行页（行页优先铺满高度）
        let cols_per_page = 12usize;
        let col_pages = max_col.div_ceil(cols_per_page);
        let row_pages = rows.len().div_ceil(rows_per_page);
        let total = col_pages * row_pages;
        for pg in 0..total {
            let (rp, cp) = (pg / cols_per_page, pg % cols_per_page);
            let mut page = crate::model::PageModel {
                width: Some(pw),
                height: Some(ph),
                ..Default::default()
            };
            let title = if total > 1 {
                format!("{} ({}/{})", sheet.name, pg + 1, total)
            } else {
                sheet.name.clone()
            };
            let el = text_el(title, ml, ph - mt - 6.0, 12.0, "Helvetica", "#666666");
            page.elements.push(el);
            let col0 = cp * cols_per_page; // 0 基横向切片
            let y0 = ph - mt - header_h;
            let mut x = ml;
            for c in 0..cols_per_page {
                let ci = col0 + c;
                if ci >= max_col {
                    break;
                }
                let w = col_w.get(&(ci + 1)).copied().unwrap_or(default_w);
                let el = rect_el(
                    x,
                    y0 - row_h,
                    w,
                    row_h,
                    Some("#F2F2F2".to_string()),
                    Some("#CCCCCC".to_string()),
                    0.6,
                );
                page.elements.push(el);
                let el = text_el(
                    col_letter((ci + 1) as u32),
                    x + 4.0,
                    y0 - row_h + 4.0,
                    8.0,
                    "Helvetica",
                    "#666666",
                );
                page.elements.push(el);
                x += w;
            }
            for (ri, row) in rows
                .iter()
                .enumerate()
                .skip(rp * rows_per_page)
                .take(rows_per_page)
            {
                let top = y0 - (ri % rows_per_page + 1) as f64 * row_h;
                let idx = row.index.unwrap_or((ri + 1) as u32);
                let el = rect_el(
                    ml - 26.0,
                    top,
                    26.0,
                    row_h,
                    Some("#F2F2F2".to_string()),
                    Some("#CCCCCC".to_string()),
                    0.6,
                );
                page.elements.push(el);
                let el = text_el(
                    idx.to_string(),
                    ml - 22.0,
                    top + 4.5,
                    8.0,
                    "Helvetica",
                    "#666666",
                );
                page.elements.push(el);
                let mut x = ml;
                for c in 0..cols_per_page {
                    let ci = col0 + c;
                    if ci >= max_col {
                        break;
                    }
                    let w = col_w.get(&(ci + 1)).copied().unwrap_or(default_w);
                    let el = rect_el(x, top, w, row_h, None, Some("#CCCCCC".to_string()), 0.6);
                    page.elements.push(el);
                    if let Some(cell) = row.cells.iter().find(|c2| cell_col(c2) == ci + 1) {
                        let text = cell_text(cell);
                        if !text.is_empty() {
                            let size = 10.5;
                            let x_text = if cell_value_text_is_number(&text) {
                                x + w - text_width_pt(&text, size) - 4.0
                            } else {
                                x + 4.0
                            };
                            let el =
                                text_el(text, x_text, top + 4.5, size, pick_font(""), "#111111");
                            page.elements.push(el);
                        }
                    }
                    x += w;
                }
            }
            pages.push(page);
        }
    }
    write_product(&build_dir, model, pages, out)
}

/// Cell 引用 → 1 基列号
fn cell_col(cell: &json2xlsx::model::Cell) -> usize {
    cell.reference
        .as_deref()
        .and_then(|r| json2xlsx::utils::a1::parse_cell_ref(r).ok())
        .map(|(c, _)| c as usize)
        .unwrap_or(0)
}

/// Cell 值 → 显示文本（数值去掉浮点尾 0；公式以原式为准）
fn cell_text(cell: &json2xlsx::model::Cell) -> String {
    (|| -> Option<String> {
        let v = cell.value.as_ref()?;
        Some(match v {
            serde_json::Value::Number(n) => {
                let f = n.as_f64().unwrap_or(0.0);
                if f.fract().abs() < 1e-9 {
                    format!("{}", f as i64)
                } else {
                    format!("{f}")
                }
            }
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => return None,
            other => other.to_string().trim_matches('"').to_string(),
        })
    })()
    .unwrap_or_default()
}

fn cell_value_text_is_number(s: &str) -> bool {
    s.parse::<f64>().is_ok() && s.chars().any(|c| c.is_ascii_digit())
}
