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
    let parts: Vec<String> = doc
        .parts
        .iter()
        .map(|p| part_markdown_with(p, sinks))
        .filter(|md| !md.trim().is_empty())
        .map(|md| md.trim_end().to_string())
        .collect();
    let mut out = parts.join("\n\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// 块序列 → Markdown（块之间空行分隔；空块跳过）
pub fn blocks_markdown(blocks: &[Block]) -> String {
    blocks_markdown_with(blocks, PayloadSinks::default())
}

/// 块序列 → Markdown（带附件导出）
pub fn blocks_markdown_with(blocks: &[Block], sinks: PayloadSinks<'_>) -> String {
    let rendered: Vec<String> = blocks
        .iter()
        .map(|b| block_markdown_with(b, sinks))
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
    match b {
        Block::Heading { level, text, .. } => {
            let n = (*level).clamp(1, 6) as usize;
            format!("{} {}", "#".repeat(n), text.trim())
        }
        Block::Paragraph(p) => paragraph_text(p).trim().to_string(),
        Block::List(l) => list_markdown(l, sinks),
        Block::Table(t) => table_markdown(t),
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
        Block::Sdt(c) => blocks_markdown_with(&c.blocks, sinks)
            .trim_end()
            .to_string(),
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
    for item in &l.items {
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
        for blk in &item.blocks {
            let text = block_markdown_with(blk, sinks);
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

fn table_markdown(t: &TableBlock) -> String {
    let cols = t.rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }
    let cell_text = |c: &TableCell| -> String {
        c.blocks
            .iter()
            .map(Block::plain_text)
            .filter(|s| !s.trim().is_empty())
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
            &t.rows[0].cells.iter().map(cell_text).collect::<Vec<_>>(),
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
    for r in body {
        out.push('\n');
        out.push_str(&row_line(
            &r.cells.iter().map(cell_text).collect::<Vec<_>>(),
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
