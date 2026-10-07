//! 元素路径解析
//!
//! 树形路径语法（1-based 索引，按元素类型计数）：
//! ```text
//! /slide[1]                      # 幻灯片
//! /slide[1]/text[2]              # 第 2 个文本元素
//! /slide[1]/group[1]/shape[1]    # 分组内嵌套元素（递归）
//! /slide[1]/progress_bar[1]      # 高级组件（旧 camelCase 写法仍兼容）
//! ```

use json2pptx::model::elements::Element;
use json2pptx::model::Slide;
use office_core::{suggest, CliError};

type Result<T> = std::result::Result<T, CliError>;

/// 元素类型清单（能力表引用；未知类型报错时给纠错建议）
pub const ELEMENT_TYPES: &[&str] = &[
    "text",
    "shape",
    "line",
    "image",
    "icon",
    "formula",
    "table",
    "chart",
    "comment",
    "equation",
    "opaque",
    "group",
    "progress_bar",
    "progress_ring",
    "bar_chart",
    "line_chart",
    "pie_chart",
    "ring_chart",
    "kpi_card",
    "rating_stars",
    "timeline",
    "process_flow",
];

/// 路径段：类型名 + 1-based 索引，或 `@name=` 选择器
#[derive(Debug, Clone)]
pub struct Segment {
    pub type_name: String,
    pub index: usize, // 1-based（0 表示使用 name/id 选择器）
    /// `[@name=X]` 选择器
    pub name: Option<String>,
    /// `[@id=N]` 稳定 ID 选择器（cNvPr/@id，插入删除不漂移）
    pub id: Option<u32>,
}

/// 解析后的元素路径
#[derive(Debug, Clone)]
pub struct ElementPath {
    /// 幻灯片索引（1-based）
    pub slide: usize,
    /// 元素段（从 slide 下的元素开始）
    pub elements: Vec<Segment>,
}

/// 解析 `/slide[N]/type[M]/...` 路径
pub fn parse(path: &str) -> Result<ElementPath> {
    let trimmed = path.trim();
    if !trimmed.starts_with('/') {
        return Err(CliError::new(format!("路径必须以 / 开头: {path}")));
    }
    let parts: Vec<&str> = trimmed[1..].split('/').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return Err(CliError::new("路径为空"));
    }

    let slide_seg = parse_segment(parts[0])?;
    if slide_seg.type_name != "slide" {
        return Err(CliError::new(format!(
            "路径第一段必须是 slide，实际: {}",
            slide_seg.type_name
        ))
        .suggest(suggest::did_you_mean(&slide_seg.type_name, &["slide"], 4)));
    }

    let mut elements = Vec::new();
    for part in &parts[1..] {
        elements.push(parse_segment(part)?);
    }

    Ok(ElementPath {
        slide: slide_seg.index,
        elements,
    })
}

/// 类型名归一化：camelCase → snake_case（旧路径写法 `progressBar[1]` 仍可寻址）
fn normalize_type(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// 解析单个段：`类型[索引]` / `类型[@name=X]` / `类型`
fn parse_segment(part: &str) -> Result<Segment> {
    let Some(open) = part.find('[') else {
        // 无索引：默认取第一个
        return Ok(Segment {
            type_name: normalize_type(part),
            index: 1,
            name: None,
            id: None,
        });
    };
    let close = part
        .rfind(']')
        .ok_or_else(|| CliError::new(format!("路径段缺少 ]: {part}")))?;
    if close != part.len() - 1 {
        return Err(CliError::new(format!("路径段格式错误: {part}")));
    }
    let type_name = normalize_type(&part[..open]);
    let inner = &part[open + 1..close];
    if let Some(name) = inner.strip_prefix("@name=") {
        return Ok(Segment {
            type_name,
            index: 0,
            name: Some(name.to_string()),
            id: None,
        });
    }
    if let Some(id) = inner.strip_prefix("@id=") {
        let id: u32 = id
            .parse()
            .map_err(|_| CliError::new(format!("非法 @id: {part}")))?;
        return Ok(Segment {
            type_name,
            index: 0,
            name: None,
            id: Some(id),
        });
    }
    let idx: usize = inner
        .parse()
        .map_err(|_| CliError::new(format!("索引必须为正整数: {part}")))?;
    if idx == 0 {
        return Err(CliError::new(format!("索引从 1 开始: {part}")));
    }
    Ok(Segment {
        type_name,
        index: idx,
        name: None,
        id: None,
    })
}

/// 在元素列表中按 类型[索引] / @name 定位（索引 1-based，按该类型计数）
fn locate_in(elements: &[Element], seg: &Segment) -> Result<usize> {
    let known = |t: &str| ELEMENT_TYPES.contains(&t);
    if let Some(want) = seg.id {
        return elements
            .iter()
            .position(|el| {
                (seg.type_name.is_empty() || el.type_name() == seg.type_name)
                    && el.id() == Some(want)
            })
            .ok_or_else(|| {
                CliError::new(format!(
                    "未找到 @id={want} 的 {} 元素（稳定 ID 来自 unpack 的原始文档；新增元素会自动分配）",
                    seg.type_name
                ))
            });
    }
    if let Some(name) = &seg.name {
        return elements
            .iter()
            .position(|el| {
                (seg.type_name.is_empty() || el.type_name() == seg.type_name)
                    && el.name() == Some(name.as_str())
            })
            .ok_or_else(|| {
                let names: Vec<&str> = elements.iter().filter_map(|e| e.name()).collect();
                CliError::new(format!("未找到 name=\"{name}\" 的 {} 元素", seg.type_name)).suggest(
                    if names.is_empty() {
                        "当前幻灯片没有带 name 的元素；新增元素可用 --prop name=X 命名".to_string()
                    } else {
                        format!("现有 name: {}", names.join(" / "))
                    },
                )
            });
    }
    if !seg.type_name.is_empty() && !known(&seg.type_name) {
        return Err(CliError::new(format!("未知元素类型: {}", seg.type_name))
            .suggest(suggest::did_you_mean(&seg.type_name, ELEMENT_TYPES, 10)));
    }
    let mut seen = 0;
    for (i, el) in elements.iter().enumerate() {
        if el.type_name() == seg.type_name {
            seen += 1;
            if seen == seg.index {
                return Ok(i);
            }
        }
    }
    let count = elements
        .iter()
        .filter(|e| e.type_name() == seg.type_name)
        .count();
    Err(CliError::new(format!(
        "未找到第 {} 个 {} 元素（该类型共 {count} 个）",
        seg.index, seg.type_name
    ))
    .suggest(if count == 0 {
        "该类型元素不存在；用 view /slide[N] layout 查看现有元素类型".to_string()
    } else {
        format!("索引范围 1~{count}")
    }))
}

/// 校验路径并返回定位信息（slide 0-based 索引 + 元素索引链）
pub struct Resolved {
    /// slide 在 slides 数组中的 0-based 索引
    pub slide_idx: usize,
    /// 从 slide.elements 到目标元素的索引链（逐层 children）
    pub chain: Vec<usize>,
}

/// 解析并校验路径，返回定位信息
pub fn resolve(slides: &[Slide], path: &ElementPath) -> Result<Resolved> {
    if path.slide < 1 || path.slide > slides.len() {
        return Err(CliError::new(format!(
            "幻灯片索引越界: /slide[{}]（共 {} 张）",
            path.slide,
            slides.len()
        ))
        .suggest(format!("索引范围 1~{}", slides.len())));
    }
    let slide_idx = path.slide - 1;
    let mut chain = Vec::new();
    let mut current: &[Element] = &slides[slide_idx].elements;
    for (pos, seg) in path.elements.iter().enumerate() {
        let idx = locate_in(current, seg)?;
        chain.push(idx);
        if let Some(children) = current[idx].children() {
            current = children;
        } else if pos + 1 < path.elements.len() {
            return Err(CliError::new(format!(
                "路径中间节点 {} 不是分组，无法继续下钻",
                seg.type_name
            )));
        }
    }
    Ok(Resolved { slide_idx, chain })
}

/// 沿索引链获取元素（只读）
pub fn get_element<'a>(slides: &'a [Slide], resolved: &Resolved) -> Result<&'a Element> {
    if resolved.chain.is_empty() {
        return Err(CliError::new("路径未定位到元素（仅 slide 级）"));
    }
    let mut current: &[Element] = &slides[resolved.slide_idx].elements;
    let mut el = &current[resolved.chain[0]];
    for &idx in &resolved.chain[1..] {
        let children = el
            .children()
            .ok_or_else(|| "中间节点不是分组".to_string())?;
        current = children;
        el = &current[idx];
    }
    Ok(el)
}

/// 沿索引链获取元素的可变引用（只读链从 slide.elements 开始）
pub fn get_element_mut<'a>(
    slides: &'a mut [Slide],
    resolved: &Resolved,
) -> Result<&'a mut Element> {
    if resolved.chain.is_empty() {
        return Err(CliError::new("路径未定位到元素（仅 slide 级）"));
    }
    let mut current: &mut [Element] = &mut slides[resolved.slide_idx].elements;
    let mut el = &mut current[resolved.chain[0]];
    for &idx in &resolved.chain[1..] {
        let children = match el {
            Element::Group(g) => &mut g.children,
            _ => return Err(CliError::new("中间节点不是分组")),
        };
        current = children;
        el = &mut current[idx];
    }
    Ok(el)
}

/// 获取目标元素的父容器（slide.elements 或某个 group 的 children）
pub fn parent_elements<'a>(
    slides: &'a mut [Slide],
    resolved: &Resolved,
) -> Result<&'a mut Vec<Element>> {
    let slide_idx = resolved.slide_idx;
    match resolved.chain.len() {
        // 0：路径仅 /slide[N]（add 到幻灯片根部）；1：目标元素的父容器即 slide.elements
        0 | 1 => Ok(&mut slides[slide_idx].elements),
        _ => {
            // 逐层下钻到目标元素所在容器（chain 最后一层之前的所有层）
            let mut current: &mut Vec<Element> = &mut slides[slide_idx].elements;
            for &idx in &resolved.chain[..resolved.chain.len() - 1] {
                let children = match &mut current[idx] {
                    Element::Group(g) => &mut g.children,
                    _ => return Err(CliError::new("中间节点不是分组")),
                };
                current = children;
            }
            Ok(current)
        }
    }
}
