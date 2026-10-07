//! 元素路径解析（对齐 ai-ppt/cli 的 path.rs）
//!
//! 树形路径语法（1-based 索引，按元素类型计数）：
//! ```text
//! /page[1]                        # 页（页级属性 w/h/rotation/crop）
//! /page[1]/text[2]                # 第 2 个文本元素
//! /page[1]/group[1]/text[1]       # 分组内嵌套元素（递归）
//! ```
//! PDF 模型无 name/id 元数据，寻址为纯索引制（`unpack` 分片里元素顺序即寻址序）。

use json2pdf::model::{Element, PageModel};
use office_core::{suggest, CliError};

type Result<T> = std::result::Result<T, CliError>;

/// 元素类型清单（能力表引用；未知类型报错时给纠错建议）
pub const ELEMENT_TYPES: &[&str] = &[
    "text",
    "rect",
    "pattern_rect",
    "path",
    "polyline",
    "shading",
    "image",
    "group",
];

/// 元素类型名（serde tag 与寻址段同名，snake_case）
pub fn type_name(el: &Element) -> &'static str {
    match el {
        Element::Text { .. } => "text",
        Element::Rect { .. } => "rect",
        Element::PatternRect { .. } => "pattern_rect",
        Element::Path { .. } => "path",
        Element::Polyline { .. } => "polyline",
        Element::Shading(_) => "shading",
        Element::Image { .. } => "image",
        Element::Group { .. } => "group",
    }
}

/// 分组 children（唯一可下钻的容器）
pub fn children(el: &Element) -> Option<&Vec<Element>> {
    match el {
        Element::Group { children, .. } => Some(children),
        _ => None,
    }
}

pub fn children_mut(el: &mut Element) -> Option<&mut Vec<Element>> {
    match el {
        Element::Group { children, .. } => Some(children),
        _ => None,
    }
}

/// 元素的文本内容（仅 text）
pub fn text_of(el: &Element) -> Option<&str> {
    match el {
        Element::Text { text, .. } => Some(text),
        _ => None,
    }
}

/// 元素几何（有则返回；text 无 w/h，path/polyline/shading 无独立 x/y）
pub fn geometry(el: &Element) -> (Option<f64>, Option<f64>, Option<f64>, Option<f64>) {
    match el {
        Element::Text { x, y, .. } => (Some(*x), Some(*y), None, None),
        Element::Rect { x, y, w, h, .. }
        | Element::PatternRect { x, y, w, h, .. }
        | Element::Image { x, y, w, h, .. } => (Some(*x), Some(*y), Some(*w), Some(*h)),
        _ => (None, None, None, None),
    }
}

/// 路径段：类型名 + 1-based 索引
#[derive(Debug, Clone)]
pub struct Segment {
    pub type_name: String,
    pub index: usize, // 1-based
    /// 段是否显式写了索引（`/page` vs `/page[1]`；add page 时区分文档容器与页）
    pub explicit_index: bool,
}

/// 解析后的元素路径
#[derive(Debug, Clone)]
pub struct ElementPath {
    /// 页索引（1-based）
    pub page: usize,
    /// 首段是否显式带索引
    pub page_explicit: bool,
    /// 元素段（从 page 下的元素开始）
    pub elements: Vec<Segment>,
}

/// 解析 `/page[N]/type[M]/...` 路径
pub fn parse(path: &str) -> Result<ElementPath> {
    let trimmed = path.trim();
    if !trimmed.starts_with('/') {
        return Err(CliError::new(format!("路径必须以 / 开头: {path}")));
    }
    let parts: Vec<&str> = trimmed[1..].split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return Err(CliError::new("路径为空").suggest("形如 /page[1] 或 /page[1]/text[2]"));
    }

    let (page, page_explicit) = {
        let seg = parse_segment(parts[0])?;
        if seg.type_name != "page" {
            return Err(
                CliError::new(format!("路径第一段必须是 page，实际: {}", seg.type_name))
                    .suggest(suggest::did_you_mean(&seg.type_name, &["page"], 4)),
            );
        }
        (seg.index, seg.explicit_index)
    };

    let mut elements = Vec::new();
    for part in &parts[1..] {
        elements.push(parse_segment(part)?);
    }

    Ok(ElementPath {
        page,
        page_explicit,
        elements,
    })
}

/// 解析单个段：`类型[索引]`（索引可省略 = 1）
fn parse_segment(part: &str) -> Result<Segment> {
    let Some(open) = part.find('[') else {
        return Ok(Segment {
            type_name: part.to_string(),
            index: 1,
            explicit_index: false,
        });
    };
    let close = part
        .rfind(']')
        .ok_or_else(|| CliError::new(format!("路径段缺少 ]: {part}")))?;
    if close != part.len() - 1 {
        return Err(CliError::new(format!("路径段格式错误: {part}")));
    }
    let type_name = part[..open].to_string();
    let inner = &part[open + 1..close];
    let idx: usize = inner
        .parse()
        .map_err(|_| CliError::new(format!("索引必须为正整数: {part}")))?;
    if idx == 0 {
        return Err(CliError::new(format!("索引从 1 开始: {part}")));
    }
    Ok(Segment {
        type_name,
        index: idx,
        explicit_index: true,
    })
}

/// 在元素列表中按 类型[索引] 定位（索引 1-based，按该类型计数）
fn locate_in(elements: &[Element], seg: &Segment) -> Result<usize> {
    if !seg.type_name.is_empty() && !ELEMENT_TYPES.contains(&seg.type_name.as_str()) {
        return Err(CliError::new(format!("未知元素类型: {}", seg.type_name))
            .suggest(suggest::did_you_mean(&seg.type_name, ELEMENT_TYPES, 10)));
    }
    let mut seen = 0;
    for (i, el) in elements.iter().enumerate() {
        if type_name(el) == seg.type_name {
            seen += 1;
            if seen == seg.index {
                return Ok(i);
            }
        }
    }
    let count = elements
        .iter()
        .filter(|e| type_name(e) == seg.type_name)
        .count();
    Err(CliError::new(format!(
        "未找到第 {} 个 {} 元素（该类型共 {count} 个）",
        seg.index, seg.type_name
    ))
    .suggest(if count == 0 {
        "该类型元素不存在；用 view /page[N] layout 查看现有元素类型".to_string()
    } else {
        format!("索引范围 1~{count}")
    }))
}

/// 校验路径并返回定位信息
pub struct Resolved {
    /// 页在 pages 数组中的 0-based 索引
    pub page_idx: usize,
    /// 从 page.elements 到目标元素的索引链（逐层 group children）
    pub chain: Vec<usize>,
}

/// 解析并校验路径，返回定位信息
pub fn resolve(pages: &[PageModel], path: &ElementPath) -> Result<Resolved> {
    if path.page < 1 || path.page > pages.len() {
        return Err(CliError::new(format!(
            "页面索引越界: /page[{}]（共 {} 页）",
            path.page,
            pages.len()
        ))
        .suggest(format!("索引范围 1~{}", pages.len())));
    }
    let page_idx = path.page - 1;
    let mut chain = Vec::new();
    let mut current: &[Element] = &pages[page_idx].elements;
    for (pos, seg) in path.elements.iter().enumerate() {
        let idx = locate_in(current, seg)?;
        chain.push(idx);
        if let Some(kids) = children(&current[idx]) {
            current = kids;
        } else if pos + 1 < path.elements.len() {
            return Err(CliError::new(format!(
                "路径中间节点 {} 不是分组，无法继续下钻",
                seg.type_name
            )));
        }
    }
    Ok(Resolved { page_idx, chain })
}

/// 沿索引链获取元素（只读）
pub fn get_element<'a>(pages: &'a [PageModel], resolved: &Resolved) -> Result<&'a Element> {
    if resolved.chain.is_empty() {
        return Err(CliError::new("路径未定位到元素（仅 page 级）"));
    }
    let mut current: &[Element] = &pages[resolved.page_idx].elements;
    let mut el = &current[resolved.chain[0]];
    for &idx in &resolved.chain[1..] {
        let kids = children(el).ok_or_else(|| CliError::new("中间节点不是分组"))?;
        current = kids;
        el = &current[idx];
    }
    Ok(el)
}

/// 沿索引链获取元素的可变引用
pub fn get_element_mut<'a>(
    pages: &'a mut [PageModel],
    resolved: &Resolved,
) -> Result<&'a mut Element> {
    if resolved.chain.is_empty() {
        return Err(CliError::new("路径未定位到元素（仅 page 级）"));
    }
    let mut current: &mut [Element] = &mut pages[resolved.page_idx].elements;
    let mut el = &mut current[resolved.chain[0]];
    for &idx in &resolved.chain[1..] {
        let kids = match children_mut(el) {
            Some(k) => k,
            None => return Err(CliError::new("中间节点不是分组")),
        };
        current = kids;
        el = &mut current[idx];
    }
    Ok(el)
}

/// 获取目标元素的父容器（page.elements 或某个 group 的 children）
pub fn parent_elements<'a>(
    pages: &'a mut [PageModel],
    resolved: &Resolved,
) -> Result<&'a mut Vec<Element>> {
    let page_idx = resolved.page_idx;
    match resolved.chain.len() {
        0 | 1 => Ok(&mut pages[page_idx].elements),
        _ => {
            let mut current: &mut Vec<Element> = &mut pages[page_idx].elements;
            for &idx in &resolved.chain[..resolved.chain.len() - 1] {
                let kids = match children_mut(&mut current[idx]) {
                    Some(k) => k,
                    None => return Err(CliError::new("中间节点不是分组")),
                };
                current = kids;
            }
            Ok(current)
        }
    }
}

/// 元素的 CLI 路径（view/get 输出用；chain 为 page.elements 起的完整索引链）
pub fn element_path(page_no: usize, elements: &[Element], chain: &[usize]) -> String {
    let mut p = format!("/page[{page_no}]");
    let mut current = elements;
    for (depth, &idx) in chain.iter().enumerate() {
        let t = type_name(&current[idx]);
        let n = current
            .iter()
            .take(idx + 1)
            .filter(|e| type_name(e) == t)
            .count();
        p.push_str(&format!("/{t}[{n}]"));
        if depth + 1 < chain.len() {
            match children(&current[idx]) {
                Some(kids) => current = kids,
                None => break,
            }
        }
    }
    p
}
