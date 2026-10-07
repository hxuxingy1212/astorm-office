//! 第二层精准修改：get / set / add / remove（对应 ai-ppt 的 edit.rs）

use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use crate::path::Located;
use json2docx::model::blocks::*;
use json2docx::model::{Document, Section};
use serde_json::Value;
use std::collections::HashMap;

/// 解析 --prop k=v 列表
pub fn parse_props(props: &[String]) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for p in props {
        let (k, v) = p
            .split_once('=')
            .ok_or_else(|| CliError::new(format!("属性格式应为 k=v: {p}")))?;
        map.insert(k.trim().to_string(), v.to_string());
    }
    Ok(map)
}

fn num(props: &HashMap<String, String>, key: &str) -> Option<f64> {
    props
        .get(key)
        .and_then(|s| json2docx::utils::units::parse_length(s))
}
fn bool_of(props: &HashMap<String, String>, key: &str) -> Option<bool> {
    props
        .get(key)
        .map(|s| s.eq_ignore_ascii_case("true") || s == "1")
}
fn int_of(props: &HashMap<String, String>, key: &str) -> Option<i64> {
    props.get(key).and_then(|s| s.parse().ok())
}
fn s_of<'a>(props: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    props.get(key).map(|s| s.as_str())
}

/// get：输出节点 JSON
/// 文档级属性（batch 的 `set /` 指令）：styles/page/theme/template/watermark/background
pub fn apply_doc_props(doc: &mut Document, props: &HashMap<String, String>) -> Result<Value> {
    if let Some(v) = s_of(props, "styles") {
        doc.styles = serde_json::from_str(v)
            .map_err(|e| CliError::new(format!("styles JSON 解析失败: {e}")))?;
    }
    if let Some(v) = s_of(props, "page") {
        doc.page = serde_json::from_str(v)
            .map_err(|e| CliError::new(format!("page JSON 解析失败: {e}")))?;
    }
    if let Some(v) = s_of(props, "theme") {
        doc.theme = serde_json::from_str(v)
            .map_err(|e| CliError::new(format!("theme JSON 解析失败: {e}")))?;
    }
    if let Some(v) = s_of(props, "template") {
        doc.template = Some(v.to_string());
    }
    if let Some(v) = s_of(props, "watermark") {
        doc.watermark = if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        };
    }
    if let Some(v) = s_of(props, "background") {
        doc.background = if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        };
    }
    Ok(serde_json::json!({ "doc": true, "updated": true }))
}

pub fn get(doc: &Document, loc: &Located) -> Result<Value> {
    if loc.is_block() {
        let b = crate::path::block_ref(doc, loc)?;
        Ok(serde_json::to_value(b)?)
    } else {
        let part = &doc.parts[loc.part];
        Ok(serde_json::to_value(part)?)
    }
}

/// set：修改属性
pub fn set(doc: &mut Document, loc: &Located, props: &HashMap<String, String>) -> Result<Value> {
    if loc.is_block() {
        // 未知属性严格校验（自愈建议），避免拼错被静默忽略
        let known = crate::capabilities::prop_names_for("block");
        for k in props.keys() {
            if !known.contains(&k.as_str()) {
                return Err(CliError::new(format!("不支持的属性: {k}"))
                    .suggest(office_core::suggest::did_you_mean(k, &known, 10)));
            }
        }
        let b = crate::path::block_mut(doc, loc)?;
        set_block(b, props)?;
        Ok(serde_json::to_value(&*b)?)
    } else {
        let known = crate::capabilities::prop_names_for("part");
        for k in props.keys() {
            if !known.contains(&k.as_str()) {
                return Err(CliError::new(format!("分片不支持的属性: {k}"))
                    .suggest(office_core::suggest::did_you_mean(k, &known, 6)));
            }
        }
        set_part(doc, loc.part, props)?;
        Ok(serde_json::json!({ "part": loc.part + 1, "updated": true }))
    }
}

fn set_block(b: &mut Block, props: &HashMap<String, String>) -> Result<()> {
    match b {
        Block::Heading {
            level,
            text,
            align: halign,
            ..
        } => {
            if let Some(t) = s_of(props, "text") {
                *text = t.to_string();
            }
            if let Some(l) = int_of(props, "level") {
                *level = l.clamp(1, 9) as u8;
            }
            if let Some(a) = s_of(props, "align") {
                *halign = Some(a.to_string());
            }
        }
        Block::Paragraph(p) => {
            if let Some(t) = s_of(props, "text") {
                p.text = Some(t.to_string());
                p.runs = None;
            }
            if let Some(v) = s_of(props, "style") {
                p.style = Some(v.to_string());
            }
            if let Some(v) = s_of(props, "align") {
                p.align = Some(v.to_string());
            }
            if let Some(v) = bool_of(props, "bold") {
                p.bold = Some(v);
            }
            if let Some(v) = bool_of(props, "italic") {
                p.italic = Some(v);
            }
            if let Some(v) = num(props, "font_size") {
                p.font_size = Some(v);
            }
            if let Some(v) = s_of(props, "color") {
                p.color = Some(v.to_string());
            }
            if let Some(v) = s_of(props, "font_family") {
                p.font_family = Some(v.to_string());
            }
            if let Some(v) = num(props, "first_line_indent") {
                p.first_line_indent = Some(v);
            }
            if let Some(v) = num(props, "indent") {
                p.indent = Some(v);
            }
            if let Some(v) = num(props, "hanging_indent") {
                p.hanging_indent = Some(v);
            }
            if let Some(v) = num(props, "line_spacing") {
                p.line_spacing = Some(v);
            } else if let Some(v) = s_of(props, "line_spacing") {
                if let Some(m) = json2docx::utils::units::parse_line_spacing(v) {
                    p.line_spacing = Some(m);
                }
            }
            if let Some(v) = num(props, "space_before") {
                p.space_before = Some(v);
            }
            if let Some(v) = num(props, "space_after") {
                p.space_after = Some(v);
            }
            if let Some(v) = bool_of(props, "keep_next") {
                p.keep_next = Some(v);
            }
            if let Some(v) = bool_of(props, "keep_lines") {
                p.keep_lines = Some(v);
            }
            if let Some(v) = bool_of(props, "page_break_before") {
                p.page_break_before = Some(v);
            }
            if let Some(v) = s_of(props, "shading") {
                p.shading = Some(v.to_string());
            }
            if let Some(v) = s_of(props, "border") {
                p.border = Some(v.to_string());
            }
            if let Some(v) = s_of(props, "tabs") {
                let tabs: Vec<json2docx::model::TabStop> = v
                    .split(',')
                    .filter_map(|s| json2docx::utils::units::parse_length(s.trim()))
                    .map(|pos| json2docx::model::TabStop {
                        pos,
                        ..Default::default()
                    })
                    .collect();
                if !tabs.is_empty() {
                    p.tabs = Some(tabs);
                }
            }
        }
        Block::Image(i) => {
            if let Some(v) = s_of(props, "src") {
                i.src = v.to_string();
            }
            if let Some(v) = num(props, "width") {
                i.width = Some(v);
            }
            if let Some(v) = num(props, "height") {
                i.height = Some(v);
            }
            if let Some(v) = s_of(props, "alt") {
                i.alt = Some(v.to_string());
            }
            if let Some(v) = s_of(props, "caption") {
                i.caption = Some(v.to_string());
            }
            if let Some(v) = s_of(props, "align") {
                i.align = Some(v.to_string());
            }
            if let Some(v) = s_of(props, "wrap") {
                i.wrap = Some(v.to_string());
            }
            if let Some(v) = num(props, "x") {
                i.x = Some(v);
            }
            if let Some(v) = num(props, "y") {
                i.y = Some(v);
            }
            if let Some(v) = bool_of(props, "behind_text") {
                i.behind_text = Some(v);
            }
        }
        Block::Formula(f) => {
            if let Some(v) = s_of(props, "latex") {
                f.latex = v.to_string();
            }
            if let Some(v) = bool_of(props, "display") {
                f.display = v;
            }
            if let Some(v) = s_of(props, "number") {
                f.number = Some(v.to_string());
            }
        }
        Block::Caption(c) => {
            if let Some(v) = s_of(props, "text") {
                c.text = v.to_string();
            }
        }
        Block::Code(c) => {
            if let Some(v) = s_of(props, "text") {
                c.text = v.to_string();
            }
            if let Some(v) = s_of(props, "lang") {
                c.lang = Some(v.to_string());
            }
        }
        Block::Quote(q) => {
            if let Some(v) = s_of(props, "text") {
                q.text = v.to_string();
            }
        }
        _ => {
            return Err(CliError::new(format!(
                "类型 {} 暂不支持 set",
                b.type_name()
            )))
        }
    }
    Ok(())
}

fn set_part(doc: &mut Document, part_idx: usize, props: &HashMap<String, String>) -> Result<()> {
    let part = &mut doc.parts[part_idx];
    let section = part.section.get_or_insert_with(Section::default);
    if let Some(v) = s_of(props, "header") {
        section.header = if v.is_empty() {
            None
        } else {
            Some(v.to_string())
        };
    }
    if let Some(v) = bool_of(props, "footer_page_number") {
        section.footer_page_number = Some(v);
    }
    if let Some(v) = s_of(props, "page_size") {
        section.page.get_or_insert_with(Default::default).size = v.to_string();
    }
    if let Some(v) = s_of(props, "orientation") {
        section
            .page
            .get_or_insert_with(Default::default)
            .orientation = v.to_string();
    }
    if let Some(v) = s_of(props, "footer_format") {
        section.footer_format = Some(v.to_string());
    }
    Ok(())
}

/// add：在分片下新增元素
pub fn add(
    doc: &mut Document,
    loc: &Located,
    block_type: &str,
    props: &HashMap<String, String>,
) -> Result<Value> {
    let block = build_block(block_type, props)?;
    let part = &mut doc.parts[loc.part];
    let pos = int_of(props, "index").map(|v| v.max(0) as usize);
    match pos {
        Some(i) if i < part.blocks.len() => part.blocks.insert(i, block),
        _ => part.blocks.push(block),
    }
    Ok(serde_json::json!({
        "part": loc.part + 1,
        "type": block_type,
        "added": true,
    }))
}

fn build_block(t: &str, props: &HashMap<String, String>) -> Result<Block> {
    let text = s_of(props, "text").unwrap_or("").to_string();
    Ok(match t {
        "heading" => Block::Heading {
            level: int_of(props, "level").unwrap_or(1).clamp(1, 9) as u8,
            text,
            numbering: bool_of(props, "numbering"),
            align: s_of(props, "align").map(String::from),
            runs: None,
        },
        "paragraph" => Block::Paragraph(Paragraph {
            text: Some(text),
            align: s_of(props, "align").map(String::from),
            style: s_of(props, "style").map(String::from),
            first_line_indent: num(props, "first_line_indent"),
            ..Default::default()
        }),
        "list" => {
            let ordered = bool_of(props, "ordered").unwrap_or(false);
            let items = text
                .split(';')
                .filter(|s| !s.is_empty())
                .map(|s| ListItem {
                    blocks: vec![Block::Paragraph(Paragraph::text(s.trim()))],
                    level: 0,
                    ..Default::default()
                })
                .collect();
            Block::List(ListBlock { ordered, items })
        }
        "image" => Block::Image(ImageBlock {
            src: s_of(props, "src").unwrap_or("").to_string(),
            width: num(props, "width"),
            height: num(props, "height"),
            align: s_of(props, "align").map(String::from),
            caption: s_of(props, "caption").map(String::from),
            alt: s_of(props, "alt").map(String::from),
            wrap: s_of(props, "wrap").map(String::from),
            x: num(props, "x"),
            y: num(props, "y"),
            behind_text: bool_of(props, "behind_text"),
            crop: None,
            rotation: num(props, "rotation"),
        }),
        "formula" => Block::Formula(FormulaBlock {
            latex: s_of(props, "latex")
                .or(s_of(props, "text"))
                .unwrap_or("")
                .to_string(),
            display: bool_of(props, "display").unwrap_or(true),
            number: s_of(props, "number").map(String::from),
        }),
        "code" => Block::Code(CodeBlock {
            lang: s_of(props, "lang").map(String::from),
            text,
        }),
        "quote" => Block::Quote(QuoteBlock { text }),
        "caption" => Block::Caption(CaptionBlock {
            text,
            of: s_of(props, "of").map(String::from),
        }),
        "toc" => Block::Toc(TocBlock {
            title: if text.is_empty() {
                "目录".to_string()
            } else {
                text
            },
            levels: Some(vec![1, 2, 3]),
            is_static: bool_of(props, "static"),
            entries: Vec::new(),
        }),
        "bibliography" => Block::Bibliography(BibliographyBlock {
            style: s_of(props, "style").unwrap_or("GB/T 7714").to_string(),
            title: s_of(props, "title").map(String::from),
            entries: Vec::new(),
        }),
        "page_break" => Block::PageBreak,
        "table" => {
            let rows = int_of(props, "rows").unwrap_or(1).max(1) as usize;
            let cols = int_of(props, "cols").unwrap_or(1).max(1) as usize;
            let empty = || TableCell {
                blocks: vec![Block::Paragraph(Paragraph::text(""))],
                ..Default::default()
            };
            Block::Table(TableBlock {
                header_row: bool_of(props, "header_row").unwrap_or(false),
                widths: None,
                rows: (0..rows)
                    .map(|_| TableRow {
                        cells: (0..cols).map(|_| empty()).collect(),
                        ..Default::default()
                    })
                    .collect(),
                caption: s_of(props, "caption").map(String::from),
                font_size: None,
                align: s_of(props, "align").map(String::from),
                ..Default::default()
            })
        }
        "shape" => Block::Shape(ShapeBlock {
            shape_type: s_of(props, "shape_type").unwrap_or("rect").to_string(),
            width: num(props, "width"),
            height: num(props, "height"),
            fill: s_of(props, "fill").map(String::from),
            line: s_of(props, "line").map(String::from),
            line_width: num(props, "line_width"),
            rotation: num(props, "rotation"),
            text: s_of(props, "text").map(String::from),
            text_color: s_of(props, "text_color").map(String::from),
            font_size: num(props, "font_size"),
            align: s_of(props, "align").map(String::from),
            wrap: s_of(props, "wrap").map(String::from),
            x: num(props, "x"),
            y: num(props, "y"),
        }),
        "attachment" => Block::Attachment(AttachmentBlock {
            src: s_of(props, "src").unwrap_or("").to_string(),
            name: s_of(props, "name").map(String::from),
            prog_id: s_of(props, "prog_id").map(String::from),
            icon: s_of(props, "icon").map(String::from),
            width: num(props, "width"),
            height: num(props, "height"),
            label: s_of(props, "label").map(String::from),
        }),
        "textbox" => Block::TextBox(TextBoxBlock {
            text,
            width: num(props, "width"),
            height: num(props, "height"),
            fill: s_of(props, "fill").map(String::from),
            line: s_of(props, "line").map(String::from),
            align: s_of(props, "align").map(String::from),
            wrap: s_of(props, "wrap").map(String::from),
            x: num(props, "x"),
            y: num(props, "y"),
            font_size: num(props, "font_size"),
            color: s_of(props, "color").map(String::from),
            vml: bool_of(props, "vml"),
        }),
        "chart" => {
            let values: Vec<f64> = s_of(props, "data")
                .map(|d| {
                    d.split(',')
                        .filter_map(|x| x.trim().parse::<f64>().ok())
                        .collect()
                })
                .unwrap_or_default();
            let categories: Vec<String> = s_of(props, "categories")
                .map(|c| c.split(',').map(|x| x.trim().to_string()).collect())
                .unwrap_or_default();
            let name = s_of(props, "series_name").unwrap_or("系列1").to_string();
            Block::Chart(ChartBlock {
                chart_type: s_of(props, "chart_type").unwrap_or("column").to_string(),
                title: s_of(props, "title").map(String::from),
                categories,
                series: vec![ChartSeries {
                    name,
                    values,
                    ..Default::default()
                }],
                width: num(props, "width"),
                height: num(props, "height"),
                align: s_of(props, "align").map(String::from),
                ..Default::default()
            })
        }
        other => return Err(CliError::new(format!("不支持的元素类型: {other}"))),
    })
}

/// remove：删除定位的块
pub fn remove(doc: &mut Document, loc: &Located) -> Result<Value> {
    if !loc.is_block() {
        return Err(CliError::new(
            "remove 需要指向具体元素（如 /part[1]/paragraph[1]）",
        ));
    }
    let b = crate::path::remove_block(doc, loc)?;
    Ok(serde_json::json!({
        "part": loc.part + 1,
        "removed": b.type_name(),
    }))
}
