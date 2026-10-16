//! 块 / 分片 / 整篇 → Markdown（结构感知的纯文本视图）
//!
//! 供 `view <文档> [路径] text` 使用：标题层级（`#`）、列表缩进与嵌套、
//! GFM 表格、代码围栏、公式、图片链接、题注、图表数据都保留在纯文本里，
//! 输出可直接当 Markdown 阅读或落盘。
//!
//! 设计取向：**只表达结构，不表达样式**（字体/颜色/字号属于 layout 与节点级
//! `edit get` 的职责）。格式细节（colspan、边框、行距）在纯文本里无法表达，
//! 一律不输出，避免制造虚假信息。

use crate::model::blocks::*;
use crate::model::{Document, Part};
use office_core::attachment::{file_name_of, is_url};
use office_core::{PayloadSink, PayloadSinks};

/// Which tracked changes are visible in the text view. Defaults to the final document.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RevisionMode {
    #[default]
    Final,
    Original,
    Markup,
}

/// Rendering state is shared by all parts and nested blocks so note IDs remain unique.
struct TextContext {
    revision: RevisionMode,
    notes: Vec<String>,
}

impl TextContext {
    fn runs(&mut self, runs: &[Run], sinks: PayloadSinks<'_>) -> String {
        let mut out = String::new();
        for (index, run) in runs.iter().enumerate() {
            let _source = sinks.at_source(format!("run[{}]", index + 1));
            let revision = run.revision.as_deref();
            if (self.revision == RevisionMode::Final && revision == Some("del"))
                || (self.revision == RevisionMode::Original && revision == Some("ins"))
            {
                continue;
            }
            let mut text = run.text.clone();
            if let Some(image) = &run.image {
                text.push_str(&image_markdown(image, sinks.media));
            }
            if let Some(url) = &run.hyperlink {
                text = format!(
                    "[{}]({})",
                    text.replace('[', "\\[").replace(']', "\\]"),
                    url.replace(' ', "%20")
                        .replace('(', "%28")
                        .replace(')', "%29")
                );
            }
            if self.revision == RevisionMode::Markup {
                match revision {
                    Some("del") => text = format!("<del>{text}</del>"),
                    Some("ins") => text = format!("<ins>{text}</ins>"),
                    _ => {}
                }
            }
            out.push_str(&text);
            for (kind, note) in [
                ("脚注", &run.footnote),
                ("尾注", &run.endnote),
                ("批注", &run.comment),
            ] {
                if let Some(note) = note {
                    let author = if kind == "批注" {
                        run.comment_author
                            .as_deref()
                            .map(|a| format!("（{a}）"))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    self.notes.push(format!("{kind}{author}：{note}"));
                    out.push_str(&format!("[^note-{}]", self.notes.len()));
                }
            }
        }
        out
    }

    fn prepare(&mut self, block: &mut Block, sinks: PayloadSinks<'_>) {
        match block {
            Block::Paragraph(p) => {
                if let Some(runs) = p.runs.take() {
                    p.text = Some(self.runs(&runs, sinks));
                }
            }
            Block::Heading { text, runs, .. } => {
                if let Some(runs) = runs.take() {
                    *text = self.runs(&runs, sinks);
                }
            }
            Block::List(l) => {
                for (index, item) in l.items.iter_mut().enumerate() {
                    let _source = sinks.at_source(format!("item[{}]", index + 1));
                    self.prepare_blocks(&mut item.blocks, sinks);
                }
            }
            Block::Table(t) => {
                for (ri, row) in t.rows.iter_mut().enumerate() {
                    let _row = sinks.at_source(format!("row[{}]", ri + 1));
                    for (ci, cell) in row.cells.iter_mut().enumerate() {
                        let _cell = sinks.at_source(format!("cell[{}]", ci + 1));
                        self.prepare_blocks(&mut cell.blocks, sinks);
                    }
                }
            }
            Block::Sdt(s) => self.prepare_blocks(&mut s.blocks, sinks),
            _ => {}
        }
    }

    fn prepare_blocks(&mut self, blocks: &mut [Block], sinks: PayloadSinks<'_>) {
        for (index, block) in blocks.iter_mut().enumerate() {
            let _source = sinks.at_source(format!("block[{}]", index + 1));
            self.prepare(block, sinks);
        }
    }

    fn finish(&self, mut text: String) -> String {
        for (i, note) in self.notes.iter().enumerate() {
            text.push_str(&format!(
                "\n\n[^note-{}]: {}",
                i + 1,
                note.replace('\n', "\n    ")
            ));
        }
        if !text.is_empty() {
            text = format!("{}\n", text.trim_end());
        }
        text
    }
}

/// Render blocks with explicit tracked-change semantics, including nested content.
pub fn blocks_markdown_with_options(
    blocks: &[Block],
    sinks: PayloadSinks<'_>,
    revision: RevisionMode,
) -> String {
    let mut context = TextContext {
        revision,
        notes: Vec::new(),
    };
    let mut prepared = blocks.to_vec();
    context.prepare_blocks(&mut prepared, sinks);
    context.finish(render_blocks(&prepared, sinks))
}

/// Render the whole document, sharing note references across parts.
pub fn document_markdown_with_options(
    doc: &Document,
    sinks: PayloadSinks<'_>,
    revision: RevisionMode,
) -> String {
    let mut context = TextContext {
        revision,
        notes: Vec::new(),
    };
    let parts: Vec<_> = doc
        .parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            let _source = sinks.at_source(format!("part[{}]", index + 1));
            let mut prepared = part.blocks.clone();
            context.prepare_blocks(&mut prepared, sinks);
            render_blocks(&prepared, sinks).trim_end().to_string()
        })
        .filter(|s| !s.is_empty())
        .collect();
    context.finish(parts.join("\n\n"))
}

/// 单个分片 → Markdown
pub fn part_markdown(part: &Part) -> String {
    blocks_markdown(&part.blocks)
}

/// 单个分片 → Markdown（带附件导出：链接指向导出后的路径/URL）
pub fn part_markdown_with(part: &Part, sinks: PayloadSinks<'_>) -> String {
    blocks_markdown_with(&part.blocks, sinks)
}

/// 整篇文档 → Markdown（分片之间空行分隔；空分片静默跳过）
///
/// 输出是**纯 Markdown**：不带任何 `<!-- … -->` 注释标记（分片边界是内部实现，
/// 在 Word 里并不可见）。要按分片定位用 `view <路径> layout` 或 `edit`。
pub fn document_markdown(doc: &Document) -> String {
    document_markdown_with(doc, PayloadSinks::default())
}

/// 整篇文档 → Markdown（带附件导出）
pub fn document_markdown_with(doc: &Document, sinks: PayloadSinks<'_>) -> String {
    document_markdown_with_options(doc, sinks, RevisionMode::Final)
}

/// 块序列 → Markdown（块之间空行分隔；空块跳过）
pub fn blocks_markdown(blocks: &[Block]) -> String {
    blocks_markdown_with(blocks, PayloadSinks::default())
}

/// 块序列 → Markdown（带附件导出）
pub fn blocks_markdown_with(blocks: &[Block], sinks: PayloadSinks<'_>) -> String {
    blocks_markdown_with_options(blocks, sinks, RevisionMode::Final)
}

fn render_blocks(blocks: &[Block], sinks: PayloadSinks<'_>) -> String {
    let rendered: Vec<String> = blocks
        .iter()
        .enumerate()
        .map(|(index, b)| {
            let _source = sinks.at_source(format!("block[{}]", index + 1));
            render_block(b, sinks)
        })
        .filter(|s| !s.trim().is_empty())
        .collect();
    let mut out = rendered.join("\n\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// 附件链接：导出到 sink 目录后返回新路径；URL 与载荷缺失时保留原引用。
///
/// 载荷来源（解析时已保留）：包内相对路径（`word/embeddings/x.xlsx`，相对产物根）、
/// 绝对路径、或 http(s) URL。
fn attach_link(sink: &PayloadSink, a: &AttachmentBlock) -> String {
    let name = a
        .name
        .clone()
        .unwrap_or_else(|| file_name_of(&a.src).to_string());
    if is_url(&a.src) {
        return sink.link_for_url(&name, &a.src);
    }
    sink.link_for(&name, &a.src)
}

/// 单个块 → Markdown 片段（不含尾随空行）
pub fn block_markdown(b: &Block) -> String {
    block_markdown_with(b, PayloadSinks::default())
}

/// 单个块 → Markdown 片段（带附件导出：attachment 的链接指向导出后的路径/URL）
pub fn block_markdown_with(b: &Block, sinks: PayloadSinks<'_>) -> String {
    blocks_markdown_with_options(std::slice::from_ref(b), sinks, RevisionMode::Final)
        .trim_end()
        .to_string()
}

fn render_block(b: &Block, sinks: PayloadSinks<'_>) -> String {
    match b {
        Block::Heading { level, text, .. } => {
            let n = (*level).clamp(1, 6) as usize;
            format!("{} {}", "#".repeat(n), text.trim())
        }
        Block::Paragraph(p) => paragraph_text(p).trim().to_string(),
        Block::List(l) => list_markdown(l, sinks),
        Block::Table(t) => table_markdown(t, sinks),
        Block::Image(i) => image_markdown(i, sinks.media),
        Block::Formula(f) => {
            if f.display {
                format!("$$\n{}\n$$", f.latex)
            } else {
                format!("${}$", f.latex)
            }
        }
        Block::Code(c) => {
            let lang = c.lang.as_deref().unwrap_or("");
            let text = c.text.trim_end_matches('\n');
            format!("```{lang}\n{text}\n```")
        }
        Block::Quote(q) => quote_lines(&q.text),
        Block::Toc(t) => toc_markdown(t),
        Block::Bibliography(bib) => bibliography_markdown(bib),
        Block::Caption(c) => c.text.trim().to_string(),
        Block::Chart(ch) => chart_markdown(ch),
        Block::TextBox(t) => t.text.trim().to_string(),
        Block::Shape(s) => s.text.as_deref().unwrap_or("").trim().to_string(),
        Block::Attachment(a) => {
            let label = a.name.as_deref().unwrap_or(&a.src);
            let target = match sinks.attachments {
                Some(s) => attach_link(s, a),
                None => a.src.clone(),
            };
            let mut out = format!("[{label}]({target})");
            if let Some(l) = &a.label {
                out.push_str(&format!("（{l}）"));
            }
            out
        }
        Block::Sdt(c) => render_blocks(&c.blocks, sinks).trim_end().to_string(),
        // 未建模的原始 XML 片段在纯文本里无处安放：有 SmartArt 文本时列出，否则跳过
        Block::Raw(r) => match &r.diagram {
            Some(d) if !d.texts.is_empty() => d
                .texts
                .iter()
                .map(|t| format!("- {t}"))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        },
        // 分页在 Markdown 里就是一条分隔线（列分隔不是页边界，纯文本里跳过）
        Block::PageBreak => "---".to_string(),
        Block::ColumnBreak => String::new(),
    }
}

fn list_markdown(l: &ListBlock, sinks: PayloadSinks<'_>) -> String {
    let mut out = String::new();
    let mut n = 0i64;
    for (index, item) in l.items.iter().enumerate() {
        let _source = sinks.at_source(format!("item[{}]", index + 1));
        let indent = "  ".repeat(item.level as usize);
        let marker = if l.ordered {
            n += 1;
            format!("{}.", item.start.unwrap_or(1) + n - 1)
        } else {
            "-".to_string()
        };
        // 首块接在标记后，其余块按标记宽度续行缩进（嵌套列表据此保持层级）
        let cont = " ".repeat(indent.len() + marker.len() + 1);
        let mut first = true;
        for (bi, blk) in item.blocks.iter().enumerate() {
            let _block = sinks.at_source(format!("block[{}]", bi + 1));
            let text = render_block(blk, sinks);
            if text.trim().is_empty() {
                continue;
            }
            let body = if first {
                format!("{indent}{marker} ")
            } else {
                format!("{indent}{cont}")
            };
            out.push_str(&with_continuation(&text, &cont, &body));
            out.push('\n');
            first = false;
        }
    }
    out.trim_end().to_string()
}

fn table_markdown(t: &TableBlock, sinks: PayloadSinks<'_>) -> String {
    let cols = t.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }
    let cell_text = |ri: usize, ci: usize, c: &TableCell| -> String {
        let _row = sinks.at_source(format!("row[{}]", ri + 1));
        let _cell = sinks.at_source(format!("cell[{}]", ci + 1));
        c.blocks
            .iter()
            .enumerate()
            .map(|(bi, block)| {
                let _block = sinks.at_source(format!("block[{}]", bi + 1));
                render_block(block, sinks)
            })
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ")
            .replace('|', "\\|")
            .replace('\n', " ")
            .trim()
            .to_string()
    };
    let row_line = |cells: &[String]| -> String {
        let mut cells = cells.to_vec();
        cells.resize(cols, String::new());
        format!("| {} |", cells.join(" | "))
    };

    let mut out = String::new();
    if let Some(cap) = &t.caption {
        out.push_str(cap.trim());
        out.push_str("\n\n");
    }
    if t.header_row && !t.rows.is_empty() {
        out.push_str(&row_line(
            &t.rows[0]
                .cells
                .iter()
                .enumerate()
                .map(|(ci, cell)| cell_text(0, ci, cell))
                .collect::<Vec<_>>(),
        ));
    } else {
        // 无表头：留空表头，避免把第一行数据伪装成表头
        out.push_str(&row_line(&vec![String::new(); cols]));
    }
    out.push('\n');
    out.push_str(&row_line(&vec!["---".to_string(); cols]));
    let body = if t.header_row {
        &t.rows[1.min(t.rows.len())..]
    } else {
        &t.rows[..]
    };
    for (index, r) in body.iter().enumerate() {
        let ri = index + usize::from(t.header_row);
        out.push('\n');
        out.push_str(&row_line(
            &r.cells
                .iter()
                .enumerate()
                .map(|(ci, cell)| cell_text(ri, ci, cell))
                .collect::<Vec<_>>(),
        ));
    }
    out
}

fn image_markdown(i: &ImageBlock, media: Option<&PayloadSink>) -> String {
    let alt = i.alt.as_deref().or(i.caption.as_deref()).unwrap_or("image");
    // 带 --media：图片导出到指定目录，链接指向导出后的路径；URL 原样保留
    let target = match media {
        Some(sink) => {
            let name = file_name_of(&i.src).to_string();
            if is_url(&i.src) {
                sink.link_for_url(&name, &i.src)
            } else {
                sink.link_for(&name, &i.src)
            }
        }
        None => i.src.clone(),
    };
    let mut out = format!("![{alt}]({target})");
    if let Some(cap) = &i.caption {
        if i.alt.as_deref() != Some(cap.as_str()) {
            out.push('\n');
            out.push_str(cap.trim());
        }
    }
    out
}

fn quote_lines(text: &str) -> String {
    text.trim_end()
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                ">".to_string()
            } else {
                format!("> {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn toc_markdown(t: &TocBlock) -> String {
    let mut out = format!("**{}**", t.title.trim());
    if t.entries.is_empty() {
        return out; // 无缓存条目时只留标题（域结果由 Word 更新，不在纯文本里编造）
    }
    for e in &t.entries {
        let indent = "  ".repeat(e.level.saturating_sub(1) as usize);
        out.push_str(&format!("\n{indent}- {}", e.text.trim()));
    }
    out
}

fn bibliography_markdown(b: &BibliographyBlock) -> String {
    let mut out = String::new();
    if let Some(title) = &b.title {
        if !title.trim().is_empty() {
            out.push_str(&format!("**{}**\n\n", title.trim()));
        }
    }
    for (i, e) in b.entries.iter().enumerate() {
        let mut line = String::new();
        if !e.authors.is_empty() {
            line.push_str(&format!("{}. ", e.authors.join(", ")));
        }
        line.push_str(e.title.trim());
        if let Some(c) = &e.container {
            line.push_str(&format!(". {c}"));
        }
        if let Some(y) = e.year {
            line.push_str(&format!(", {y}"));
        }
        line.push('.');
        if let Some(u) = &e.url {
            line.push_str(&format!(" {u}"));
        }
        out.push_str(&format!("{}. {line}\n", i + 1));
    }
    out.trim_end().to_string()
}

/// 图表 → 标题 + 分类 × 系列数据表（纯文本里唯一可读的图表表达）
fn chart_markdown(ch: &ChartBlock) -> String {
    let mut out = String::new();
    if let Some(t) = &ch.title {
        if !t.trim().is_empty() {
            out.push_str(&format!("**{}**\n\n", t.trim()));
        }
    }
    if ch.series.is_empty() {
        return out.trim_end().to_string(); // 无数据系列：只留标题，不编造也不留注释
    }
    let n = ch
        .categories
        .len()
        .max(ch.series.iter().map(|s| s.values.len()).max().unwrap_or(0));
    let mut header = vec!["分类".to_string()];
    header.extend(ch.series.iter().map(|s| {
        if s.name.trim().is_empty() {
            ch.chart_type.clone()
        } else {
            s.name.trim().replace('|', "\\|")
        }
    }));
    out.push_str(&format!("| {} |\n", header.join(" | ")));
    out.push_str(&format!("| {} |\n", vec!["---"; header.len()].join(" | ")));
    for i in 0..n {
        let cat = ch
            .categories
            .get(i)
            .cloned()
            .unwrap_or_else(|| (i + 1).to_string());
        let mut cells = vec![cat.replace('|', "\\|")];
        for s in &ch.series {
            cells.push(
                s.values
                    .get(i)
                    .map(|v| trim_num(*v))
                    .unwrap_or_else(|| "—".to_string()),
            );
        }
        out.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    out.trim_end().to_string()
}

/// 数值去掉无意义的尾随零（92.0 → 92，92.50 → 92.5）
fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// 给多行文本的首行加 `body` 前缀、其余行加 `cont` 前缀（空行不加）
fn with_continuation(text: &str, cont: &str, body: &str) -> String {
    let mut lines = text.lines();
    let mut out = match lines.next() {
        None => return String::new(),
        Some(first) => format!("{body}{first}"),
    };
    for l in lines {
        out.push('\n');
        if !l.trim().is_empty() {
            out.push_str(cont);
            out.push_str(l);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracked_changes_and_notes_survive_nested_blocks_without_mutating_model() {
        let doc: Document = serde_json::from_value(serde_json::json!({
            "parts": [
                {"blocks": [{"type":"heading", "level":1, "text":"stale heading", "runs":[
                    {"text":"旧标题", "revision":"del"}, {"text":"新标题", "revision":"ins"}
                ]}, {"type":"table", "rows":[{"cells":[{"blocks":[
                    {"type":"paragraph", "text":"stale paragraph", "runs":[
                        {"text":"废止条款", "revision":"del", "comment":"删除原因"},
                        {"text":"生效条款", "revision":"ins", "footnote":"口径说明"},
                        {"text":"出处", "hyperlink":"https://example.com/a (b)", "endnote":"参考材料", "comment":"核对来源\n第二行", "comment_author":"张三"}
                    ]}
                ]}]}]}]},
                {"blocks":[{"type":"list", "ordered":false, "items":[{"blocks":[
                    {"type":"paragraph", "runs":[{"text":"补充", "footnote":"第二分片"}]}
                ]}]}]}
            ]
        })).unwrap();
        let before = doc.clone();
        let final_md = document_markdown(&doc);
        assert!(final_md.contains("# 新标题"), "{final_md}");
        assert!(
            !final_md.contains("废止条款")
                && !final_md.contains("删除原因")
                && !final_md.contains("stale")
        );
        assert!(final_md.contains("生效条款[^note-1]"), "{final_md}");
        assert!(final_md.contains("[出处](https://example.com/a%20%28b%29)"));
        assert!(final_md.contains("[^note-3]: 批注（张三）：核对来源\n    第二行"));
        assert!(
            final_md.contains("补充[^note-4]") && final_md.contains("[^note-4]: 脚注：第二分片")
        );
        let original =
            document_markdown_with_options(&doc, PayloadSinks::default(), RevisionMode::Original);
        assert!(
            original.contains("旧标题")
                && original.contains("废止条款")
                && original.contains("删除原因")
        );
        assert!(!original.contains("生效条款") && !original.contains("口径说明"));
        let markup =
            document_markdown_with_options(&doc, PayloadSinks::default(), RevisionMode::Markup);
        assert!(markup.contains("<del>废止条款</del>") && markup.contains("<ins>生效条款</ins>"));
        assert_eq!(
            serde_json::to_value(&doc).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
    }

    fn para(text: &str) -> Block {
        Block::Paragraph(Paragraph {
            text: Some(text.to_string()),
            ..Default::default()
        })
    }

    #[test]
    fn heading_and_paragraph_levels() {
        let blocks = vec![
            Block::Heading {
                level: 2,
                text: "绪论".into(),
                numbering: None,
                align: None,
                runs: None,
            },
            para("正文"),
        ];
        let md = blocks_markdown(&blocks);
        assert_eq!(md, "## 绪论\n\n正文\n");
    }

    #[test]
    fn nested_list_keeps_levels() {
        let item = |level: u8, text: &str| ListItem {
            blocks: vec![para(text)],
            level,
            ..Default::default()
        };
        let md = block_markdown(&Block::List(ListBlock {
            ordered: false,
            items: vec![item(0, "研究背景"), item(1, "子项"), item(0, "主要贡献")],
        }));
        assert_eq!(md, "- 研究背景\n  - 子项\n- 主要贡献");
    }

    #[test]
    fn ordered_list_numbers_items() {
        let item = |text: &str| ListItem {
            blocks: vec![para(text)],
            level: 0,
            ..Default::default()
        };
        let md = block_markdown(&Block::List(ListBlock {
            ordered: true,
            items: vec![item("第一步"), item("第二步")],
        }));
        assert_eq!(md, "1. 第一步\n2. 第二步");
    }

    #[test]
    fn table_uses_header_row_and_escapes_pipes() {
        let cell = |t: &str| TableCell {
            blocks: vec![para(t)],
            ..Default::default()
        };
        let table = TableBlock {
            header_row: true,
            caption: Some("表 1  实验对比".into()),
            rows: vec![
                TableRow {
                    cells: vec![cell("方法"), cell("R@1")],
                    ..Default::default()
                },
                TableRow {
                    cells: vec![cell("Ours|A"), cell("79.6")],
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let md = block_markdown(&Block::Table(table));
        assert!(md.starts_with("表 1  实验对比\n\n| 方法 | R@1 |\n| --- | --- |"));
        assert!(md.contains(r"| Ours\|A | 79.6 |"));
    }

    #[test]
    fn list_item_with_nested_list_stays_inside_item() {
        let nested = Block::List(ListBlock {
            ordered: false,
            items: vec![ListItem {
                blocks: vec![para("嵌套")],
                level: 0,
                ..Default::default()
            }],
        });
        let md = block_markdown(&Block::List(ListBlock {
            ordered: false,
            items: vec![ListItem {
                blocks: vec![para("外层"), nested],
                level: 0,
                ..Default::default()
            }],
        }));
        assert_eq!(md, "- 外层\n  - 嵌套");
    }

    #[test]
    fn chart_renders_data_table() {
        let ch = ChartBlock {
            chart_type: "column".into(),
            title: Some("月度收入".into()),
            categories: vec!["1月".into(), "2月".into()],
            series: vec![ChartSeries {
                name: "收入".into(),
                values: vec![92.0, 88.5],
                color: None,
            }],
            ..Default::default()
        };
        let md = block_markdown(&Block::Chart(ch));
        assert!(md.contains("| 分类 | 收入 |"));
        assert!(md.contains("| 1月 | 92 |"));
        assert!(md.contains("| 2月 | 88.5 |"));
    }

    #[test]
    fn toc_renders_cached_entries_by_level() {
        let toc = TocBlock {
            title: "目录".into(),
            levels: None,
            is_static: None,
            entries: vec![
                TocEntry {
                    level: 1,
                    text: "第一章".into(),
                },
                TocEntry {
                    level: 2,
                    text: "1.1 背景".into(),
                },
            ],
        };
        let md = block_markdown(&Block::Toc(toc));
        assert_eq!(md, "**目录**\n- 第一章\n  - 1.1 背景");

        let empty = TocBlock {
            title: "目录".into(),
            ..Default::default()
        };
        let md = block_markdown(&Block::Toc(empty));
        assert_eq!(md, "**目录**"); // 无缓存条目时不编造条目、不留注释
    }

    #[test]
    fn document_markdown_joins_parts_without_markers() {
        let doc = Document {
            parts: vec![Part::new(vec![para("一")]), Part::new(vec![para("二")])],
            ..Default::default()
        };
        // 纯 Markdown：不带 `<!-- part N -->` 之类的注释锚点
        assert_eq!(document_markdown(&doc), "一\n\n二\n");
        // 空分片静默跳过
        let doc2 = Document {
            parts: vec![
                Part::new(vec![]),
                Part::new(vec![para("只此一段")]),
                Part::new(vec![]),
            ],
            ..Default::default()
        };
        assert_eq!(document_markdown(&doc2), "只此一段\n");
    }
}
