//! 元素查询（类 CSS 选择器）
//!
//! 支持的选择器语法（单个复合选择器）：
//! ```text
//! slide[2]                     # 限定第 2 张幻灯片
//! shape                        # 按元素类型
//! shape[name=Title 1]          # 按名称
//! text:contains("Hello")       # 文本包含
//! text:empty                   # 文本为空
//! [name=Logo]                  # 仅按名称
//! ```
//! 多个条件可组合：`slide[1] shape[name=Title]`

use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use json2pptx::model::elements::Element;
use json2pptx::model::Slide;
use serde_json::json;

/// 解析后的选择器
#[derive(Debug, Default)]
pub struct Selector {
    pub slide: Option<usize>,
    pub kind: Option<String>,
    pub name: Option<String>,
    pub contains: Option<String>,
    pub empty: bool,
}

/// 解析选择器字符串
pub fn parse_selector(s: &str) -> Result<Selector> {
    let mut sel = Selector::default();
    // 先处理 slide[...] 前缀
    let rest = if let Some(idx) = s.find("slide[") {
        if idx != 0 {
            return Err(CliError::new("slide 限定必须位于选择器开头"));
        }
        let open = s.find('[').unwrap();
        let close = s
            .find(']')
            .ok_or_else(|| CliError::new("slide[...] 缺少 ]"))?;
        let n: usize = s[open + 1..close]
            .parse()
            .map_err(|_| "slide 索引必须为正整数".to_string())?;
        if n == 0 {
            return Err(CliError::new("slide 索引从 1 开始"));
        }
        sel.slide = Some(n);
        s[close + 1..].trim_start()
    } else {
        s.trim()
    };

    if rest.is_empty() {
        return Ok(sel);
    }

    // 元素类型（直到第一个 [ 或 :）
    let type_end = rest.find(['[', ':']).unwrap_or(rest.len());
    let kind = rest[..type_end].trim();
    // 允许通配 *
    if !kind.is_empty() && kind != "*" {
        sel.kind = Some(kind.to_string());
    }

    // 解析后续 [k=v] / :contains("x") / :empty
    let mut tail = &rest[type_end..];
    while !tail.is_empty() {
        if let Some(inner) = tail.strip_prefix('[') {
            let close = inner
                .find(']')
                .ok_or_else(|| CliError::new("选择器缺少 ]"))?;
            let body = &inner[..close];
            if let Some(v) = body.strip_prefix("name=") {
                sel.name = Some(v.trim_matches('"').to_string());
            } else if let Some(v) = body.strip_prefix("text=") {
                sel.contains = Some(v.trim_matches('"').to_string());
            } else {
                return Err(CliError::new(format!("不支持的属性选择器: [{body}]")));
            }
            tail = &inner[close + 1..];
        } else if let Some(inner) = tail.strip_prefix(":contains(") {
            let close = inner
                .find(')')
                .ok_or_else(|| CliError::new(":contains(...) 缺少 )"))?;
            sel.contains = Some(inner[..close].trim_matches('"').to_string());
            tail = &inner[close + 1..];
        } else if let Some(rest2) = tail.strip_prefix(":empty") {
            sel.empty = true;
            tail = rest2;
        } else if tail.starts_with(char::is_whitespace) {
            tail = tail.trim_start();
        } else {
            return Err(CliError::new(format!("无法解析选择器片段: {tail}")));
        }
    }
    Ok(sel)
}

/// 判断元素是否匹配选择器
fn matches(el: &Element, sel: &Selector) -> bool {
    if let Some(kind) = &sel.kind {
        if el.type_name() != kind {
            return false;
        }
    }
    if let Some(name) = &sel.name {
        if el.name() != Some(name.as_str()) {
            return false;
        }
    }
    let text = el.text_content().unwrap_or_default();
    if let Some(c) = &sel.contains {
        if !text.contains(c.as_str()) {
            return false;
        }
    }
    if sel.empty && !text.is_empty() {
        return false;
    }
    true
}

/// 递归收集元素路径（1-based，按类型计数）
fn walk<'a>(elements: &'a [Element], prefix: &str, out: &mut Vec<(String, &'a Element)>) {
    let mut counters: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for el in elements {
        let c = counters.entry(el.type_name()).or_insert(0);
        *c += 1;
        let path = format!("{}/{}[{}]", prefix, el.type_name(), c);
        out.push((path.clone(), el));
        if let Element::Group(g) = el {
            walk(&g.children, &path, out);
        }
    }
}

/// 执行查询，返回匹配结果
pub fn run(slides: &[Slide], sel: &Selector) -> Vec<serde_json::Value> {
    let mut results = Vec::new();
    for (i, slide) in slides.iter().enumerate() {
        let slide_no = i + 1;
        if let Some(want) = sel.slide {
            if want != slide_no {
                continue;
            }
        }
        let mut found: Vec<(String, &Element)> = Vec::new();
        walk(
            &slide.elements,
            &format!("/slide[{}]", slide_no),
            &mut found,
        );
        for (path, el) in found {
            if matches(el, sel) {
                let mut node = json!({
                    "path": path,
                    "type": el.type_name(),
                    "name": el.name().unwrap_or_default(),
                });
                if let Some(t) = el.text_content() {
                    node["text"] = json!(t);
                }
                let p = el.position();
                node["position"] = json!({ "x": p.x, "y": p.y, "w": p.w, "h": p.h });
                results.push(node);
            }
        }
    }
    results
}
