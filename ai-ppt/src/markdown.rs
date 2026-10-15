//! 幻灯片 / 演示文稿 → Markdown（结构感知的纯文本视图）
//!
//! 供 `view <文件> [路径] text` 使用：元素按阅读顺序展开，表格/图表成分、
//! 组件数据、分组嵌套、演讲者备注都保留在纯文本里。图片输出为 `![名称](绝对路径)`
//! （由调用方提供路径解析，产物目录内媒体解析为绝对路径，URL 原样保留）。
//!
//! 设计取向：只表达结构（分组层级、表格行列、图表数据），不表达样式
//! （字体/颜色/坐标属于 layout 视图与节点级 `edit get`）。

use crate::model::elements::{ChartElement, ChartSeries, Element, TableElement, TextContent};
use crate::model::Slide;
use office_core::attachment::{file_name_of, is_url};
use office_core::{PayloadSink, PayloadSinks};
use serde_json::Value;

type SrcResolver<'a> = &'a dyn Fn(&str) -> String;

/// 单页幻灯片 → Markdown
pub fn slide_markdown(slide: &Slide, resolve_src: SrcResolver) -> String {
    slide_markdown_with(slide, resolve_src, PayloadSinks::default())
}

/// 单页幻灯片 → Markdown（带附件导出：内嵌对象落到指定目录，引用指向导出后的路径）
pub fn slide_markdown_with(
    slide: &Slide,
    resolve_src: SrcResolver,
    sinks: PayloadSinks<'_>,
) -> String {
    let mut out = elements_markdown(&slide.elements, 0, resolve_src, sinks);
    if let Some(notes) = &slide.notes {
        if !notes.trim().is_empty() {
            out.push(quote(&format!("备注：{notes}")));
        }
    }
    join_blocks(&out)
}

/// 整个演示文稿 → Markdown（每页一个 `## 幻灯片 N` 小节）
///
/// 页边界用**标题**表达（幻灯片是看得见的页单位，不是内部实现），
/// 输出里不出现 `<!-- … -->` 注释标记。
pub fn deck_markdown(slides: &[Slide], resolve_src: SrcResolver) -> String {
    deck_markdown_with(slides, resolve_src, PayloadSinks::default())
}

/// 整个演示文稿 → Markdown（带附件导出）
pub fn deck_markdown_with(
    slides: &[Slide],
    resolve_src: SrcResolver,
    sinks: PayloadSinks<'_>,
) -> String {
    let mut out = String::new();
    for (i, s) in slides.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&format!("## 幻灯片 {}\n\n", i + 1));
        out.push_str(&slide_markdown_with(s, resolve_src, sinks));
    }
    out
}

/// 元素序列 → Markdown 块（每个元素一段，空块跳过）
fn elements_markdown(
    elements: &[Element],
    depth: usize,
    resolve_src: SrcResolver,
    sinks: PayloadSinks<'_>,
) -> Vec<String> {
    let mut out = Vec::new();
    for el in elements {
        out.extend(element_markdown(el, depth, resolve_src, sinks));
    }
    out
}

fn element_markdown(
    el: &Element,
    depth: usize,
    resolve_src: SrcResolver,
    sinks: PayloadSinks<'_>,
) -> Vec<String> {
    let indent = "  ".repeat(depth);
    let mut blocks: Vec<String> = Vec::new();
    match el {
        Element::Text(t) => blocks.extend(text_content_markdown(&t.text)),
        Element::Shape(s) => {
            if let Some(tc) = &s.text {
                blocks.extend(text_content_markdown(tc));
            }
        }
        Element::Opaque(o) => {
            if let Some(t) = &o.text {
                if !t.trim().is_empty() {
                    blocks.push(t.trim().to_string());
                }
            }
            // 内嵌对象（OLE 等）：`parts` 里包内路径落在 embeddings/ 下的算附件。
            // 只有带 --attachments（sink）时才渲染——默认输出保持不变。
            if let Some(sink) = sinks.attachments {
                for line in attachments_markdown(o, sink) {
                    blocks.push(line);
                }
            }
        }
        Element::Table(t) => {
            if let Some(md) = table_markdown(t) {
                blocks.push(md);
            }
        }
        Element::Image(img) => {
            let alt = img.name.as_deref().unwrap_or("image");
            // 带 --media：图片导出到指定目录，链接指向导出后的路径；URL 原样保留
            let target = match sinks.media {
                Some(sink) => {
                    let name = file_name_of(&img.src).to_string();
                    if is_url(&img.src) {
                        sink.link_for_url(&name, &img.src)
                    } else {
                        sink.link_for(&name, &img.src)
                    }
                }
                None => resolve_src(&img.src),
            };
            blocks.push(format!("![{alt}]({target})"));
        }
        Element::Formula(f) => blocks.push(format!("${}$", f.latex)),
        Element::Equation(e) => {
            if let Some(t) = &e.text {
                if !t.trim().is_empty() {
                    blocks.push(format!("$${t}$$"));
                }
            }
        }
        Element::Comment(c) => {
            let who = c
                .author
                .as_deref()
                .map(|a| format!("{a}："))
                .unwrap_or_default();
            blocks.push(quote(&format!("批注 {who}{}", c.text)));
        }
        Element::Chart(c) => blocks.push(chart_markdown(c)),
        Element::BarChart(c) => blocks.push(series_table(&c.data, c.labels.as_deref(), "数值")),
        Element::LineChart(c) => blocks.push(series_table(&c.data, c.labels.as_deref(), "数值")),
        Element::PieChart(c) => blocks.push(series_table(&c.data, c.labels.as_deref(), "占比")),
        Element::RingChart(c) => blocks.push(series_table(&c.data, c.labels.as_deref(), "占比")),
        Element::ProgressBar(c) => {
            if let Some(v) = c.value {
                blocks.push(format!("- 进度：{}%", trim_num(v)));
            }
        }
        Element::ProgressRing(c) => {
            if let Some(v) = c.value {
                blocks.push(format!("- 进度：{}%", trim_num(v)));
            }
        }
        Element::KpiCard(c) => {
            let mut line = format!("- {}", c.value);
            if let Some(l) = &c.label {
                line.push_str(&format!("（{l}）"));
            }
            if let Some(d) = &c.delta {
                line.push_str(&format!(" Δ{d}"));
            }
            blocks.push(line);
        }
        Element::RatingStars(c) => {
            blocks.push(format!(
                "- 评分：{}/{}",
                trim_num(c.rating),
                c.max.unwrap_or(5)
            ));
        }
        Element::ProcessFlow(c) => {
            blocks.push(
                c.steps
                    .iter()
                    .enumerate()
                    .map(|(i, s)| format!("{}. {s}", i + 1))
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
        }
        Element::Timeline(c) => {
            let items: Vec<String> = c
                .items
                .iter()
                .map(timeline_item)
                .filter(|s| !s.is_empty())
                .map(|s| format!("- {s}"))
                .collect();
            if !items.is_empty() {
                blocks.push(items.join("\n"));
            }
        }
        Element::Group(g) => {
            let mut inner = elements_markdown(&g.children, depth + 1, resolve_src, sinks);
            if inner.is_empty() {
                return Vec::new();
            }
            // 分组名作为小标题，子元素整体缩进一级（保持包含关系可见）
            if let Some(name) = &g.name {
                if !name.trim().is_empty() {
                    blocks.push(format!("{indent}**{}**", name.trim()));
                }
            }
            blocks.extend(inner.drain(..).map(|b| indent_lines(&b, &indent)));
        }
        Element::Line(_) | Element::Icon(_) => {}
    }
    blocks
        .into_iter()
        .filter(|b| !b.trim().is_empty())
        .map(|b| indent_lines(&b, &indent))
        .collect()
}

/// 内嵌对象 → 附件行（每个 embeddings 部件一行）：
/// `[名字](导出后的路径)（附件 · 内容类型）`
fn attachments_markdown(
    o: &crate::model::elements::OpaqueElement,
    sink: &PayloadSink,
) -> Vec<String> {
    let mut out = Vec::new();
    for part in &o.parts {
        if !part.path.contains("/embeddings/") && !part.path.starts_with("embeddings/") {
            continue;
        }
        let name = file_name_of(&part.path).to_string();
        let bytes = match base64_decode(&part.data) {
            Some(b) => b,
            None => {
                out.push(format!(
                    "附件：{name}（载荷解码失败 · {}）",
                    part.content_type
                ));
                continue;
            }
        };
        let link = sink.link_for_bytes(&name, &bytes, &part.path);
        out.push(format!("[{name}]({link})（附件 · {}）", part.content_type));
    }
    out
}

/// 极简 base64 解码（模型里 parts[].data 是 base64；不引入依赖）
fn base64_decode(s: &str) -> Option<Vec<u8>> {
    fn val(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut nbits = 0u32;
    for &c in s.as_bytes() {
        if c == b'=' || c == b'\n' || c == b'\r' {
            continue;
        }
        let v = val(c)?;
        acc = (acc << 6) | v;
        nbits += 6;
        if nbits >= 8 {
            nbits -= 8;
            out.push((acc >> nbits) as u8);
        }
    }
    Some(out)
}

/// 富文本 → Markdown 段落（bullet 段落输出为列表项）
fn text_content_markdown(tc: &TextContent) -> Vec<String> {
    match tc {
        TextContent::Simple(s) => {
            let t = s.trim();
            if t.is_empty() {
                Vec::new()
            } else {
                vec![t.to_string()]
            }
        }
        TextContent::Paragraphs(paras) => {
            let mut out = Vec::new();
            let mut bullets: Vec<String> = Vec::new();
            for p in paras {
                let text = p
                    .text
                    .clone()
                    .or_else(|| {
                        p.runs.as_ref().map(|runs| {
                            runs.iter()
                                .map(|r| r.text.clone())
                                .collect::<Vec<_>>()
                                .join("")
                        })
                    })
                    .unwrap_or_default();
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                if p.bullet == Some(true) {
                    // 连续的项目符号段落合成一个列表块（空行会拆散列表）
                    bullets.push(format!("- {text}"));
                    continue;
                }
                if !bullets.is_empty() {
                    out.push(std::mem::take(&mut bullets).join("\n"));
                }
                out.push(text.to_string());
            }
            if !bullets.is_empty() {
                out.push(bullets.join("\n"));
            }
            out
        }
    }
}

fn table_markdown(t: &TableElement) -> Option<String> {
    let cols = t.rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return None;
    }
    let row_line = |cells: &[String]| -> String {
        let mut cells = cells.to_vec();
        cells.resize(cols, String::new());
        format!("| {} |", cells.join(" | "))
    };
    let cell = |c: &crate::model::elements::Cell| c.text.replace('|', "\\|").replace('\n', " ");
    let mut out = String::new();
    let header_first = t.header_row.unwrap_or(true) && !t.rows.is_empty();
    if header_first {
        out.push_str(&row_line(&t.rows[0].iter().map(cell).collect::<Vec<_>>()));
    } else {
        out.push_str(&row_line(&vec![String::new(); cols]));
    }
    out.push('\n');
    out.push_str(&row_line(&vec!["---".to_string(); cols]));
    let body = if header_first {
        &t.rows[1..]
    } else {
        &t.rows[..]
    };
    for r in body {
        out.push('\n');
        out.push_str(&row_line(&r.iter().map(cell).collect::<Vec<_>>()));
    }
    Some(out)
}

/// 原生图表 → 标题 + 分类 × 系列数据表
fn chart_markdown(c: &ChartElement) -> String {
    let mut out = String::new();
    if let Some(t) = &c.title {
        if !t.trim().is_empty() {
            out.push_str(&format!("**{}**\n\n", t.trim()));
        }
    }
    if c.series.is_empty() {
        return out.trim_end().to_string(); // 无数据系列：只留标题，不编造也不留注释
    }
    let n = c
        .categories
        .len()
        .max(c.series.iter().map(|s| s.values.len()).max().unwrap_or(0));
    let mut header = vec!["分类".to_string()];
    header.extend(c.series.iter().map(series_name));
    out.push_str(&format!("| {} |\n", header.join(" | ")));
    out.push_str(&format!("| {} |\n", vec!["---"; header.len()].join(" | ")));
    for i in 0..n {
        let cat = c
            .categories
            .get(i)
            .cloned()
            .unwrap_or_else(|| (i + 1).to_string());
        let mut cells = vec![cat.replace('|', "\\|")];
        for s in &c.series {
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

fn series_name(s: &ChartSeries) -> String {
    match s.name.as_deref() {
        Some(n) if !n.trim().is_empty() => n.trim().replace('|', "\\|"),
        _ => "数值".to_string(),
    }
}

/// 组件数据（data + labels）→ 两列数据表
fn series_table(data: &[f64], labels: Option<&[String]>, value_header: &str) -> String {
    let mut out = format!("| 标签 | {value_header} |\n| --- | --- |\n");
    for (i, v) in data.iter().enumerate() {
        let label = labels
            .and_then(|l| l.get(i))
            .cloned()
            .unwrap_or_else(|| (i + 1).to_string());
        out.push_str(&format!(
            "| {} | {} |\n",
            label.replace('|', "\\|"),
            trim_num(*v)
        ));
    }
    out.trim_end().to_string()
}

/// 时间轴节点（date/title/desc 等键的宽松提取）
fn timeline_item(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(map) => {
            let get = |k: &str| map.get(k).and_then(|x| x.as_str()).map(|s| s.to_string());
            let mut parts: Vec<String> = Vec::new();
            if let Some(d) = get("date") {
                parts.push(d);
            }
            if let Some(t) = get("title") {
                parts.push(t);
            }
            if let Some(d) = get("desc").or_else(|| get("description")) {
                parts.push(d);
            }
            if parts.is_empty() {
                map.values()
                    .filter_map(|x| x.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
                    .join("：")
            } else {
                parts.join("：")
            }
        }
        other => other.to_string(),
    }
}

fn quote(text: &str) -> String {
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

/// 给多行文本的每一行加缩进（空行不加）
fn indent_lines(text: &str, indent: &str) -> String {
    if indent.is_empty() {
        return text.to_string();
    }
    text.lines()
        .map(|l| {
            if l.trim().is_empty() {
                l.to_string()
            } else {
                format!("{indent}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 块之间空行分隔，末尾补换行；无内容返回空串
fn join_blocks(blocks: &[String]) -> String {
    let rendered: Vec<&String> = blocks.iter().filter(|b| !b.trim().is_empty()).collect();
    if rendered.is_empty() {
        return String::new();
    }
    let mut out = rendered
        .iter()
        .map(|s| s.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n\n");
    out.push('\n');
    out
}

/// 数值去掉无意义的尾随零（92.0 → 92，88.50 → 88.5）
fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn no_src(s: &str) -> String {
        s.to_string()
    }

    /// 由 JSON 构造模型（与 CLI 收到的产物 JSON 同构，省去逐字段填默认值）
    fn slide(v: serde_json::Value) -> Slide {
        serde_json::from_value(v).expect("slide json")
    }

    fn text_el(text: &str) -> serde_json::Value {
        json!({ "type": "text", "text": text, "position": { "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0 } })
    }

    #[test]
    fn simple_text_and_notes() {
        let s = slide(json!({
            "elements": [text_el("标题"), text_el("正文")],
            "notes": "讲稿"
        }));
        let md = slide_markdown(&s, &no_src);
        assert_eq!(md, "标题\n\n正文\n\n> 备注：讲稿\n");
    }

    #[test]
    fn bullet_paragraphs_become_list_items() {
        let s = slide(json!({
            "elements": [{
                "type": "text",
                "position": { "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0 },
                "text": [
                    { "text": "第一条", "bullet": true },
                    { "text": "第二条", "bullet": true }
                ]
            }]
        }));
        assert_eq!(slide_markdown(&s, &no_src), "- 第一条\n- 第二条\n");
    }

    #[test]
    fn table_and_group_nesting() {
        let s = slide(json!({
            "elements": [
                {
                    "type": "table",
                    "position": { "x": 0.0, "y": 0.0, "w": 4.0, "h": 2.0 },
                    "header_row": true,
                    "rows": [
                        [ { "text": "方法" }, { "text": "准确率" } ],
                        [ { "text": "Ours" }, { "text": "92.3%" } ]
                    ]
                },
                {
                    "type": "group",
                    "position": { "x": 0.0, "y": 0.0, "w": 2.0, "h": 1.0 },
                    "name": "组一",
                    "children": [ text_el("组内文本") ]
                }
            ]
        }));
        let md = slide_markdown(&s, &no_src);
        assert!(
            md.contains("| 方法 | 准确率 |\n| --- | --- |\n| Ours | 92.3% |"),
            "{md}"
        );
        assert!(md.contains("**组一**\n\n  组内文本"), "{md}");
    }

    #[test]
    fn deck_marks_slide_boundaries() {
        let slides = vec![
            slide(json!({ "elements": [ text_el("一") ] })),
            slide(json!({ "elements": [ text_el("二") ] })),
        ];
        let md = deck_markdown(&slides, &no_src);
        assert!(md.starts_with("## 幻灯片 1\n\n一\n"), "{md}");
        assert!(md.contains("## 幻灯片 2\n\n二\n"), "{md}");
    }

    #[test]
    fn image_resolver_is_applied() {
        let s = slide(json!({
            "elements": [{
                "type": "image",
                "src": "ppt/media/image1.png",
                "name": "Picture 1",
                "position": { "x": 0.0, "y": 0.0, "w": 1.0, "h": 1.0 }
            }]
        }));
        let md = slide_markdown(&s, &|s: &str| format!("/abs/{s}"));
        assert_eq!(md, "![Picture 1](/abs/ppt/media/image1.png)\n");
    }

    #[test]
    fn component_data_renders_as_table() {
        let s = slide(json!({
            "elements": [{
                "type": "bar_chart",
                "position": { "x": 0.0, "y": 0.0, "w": 4.0, "h": 2.0 },
                "data": [92.0, 88.5],
                "labels": ["一月", "二月"]
            }]
        }));
        let md = slide_markdown(&s, &no_src);
        assert!(md.contains("| 标签 | 数值 |"), "{md}");
        assert!(md.contains("| 一月 | 92 |"), "{md}");
    }
}
