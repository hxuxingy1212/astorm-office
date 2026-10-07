//! 修改层：get / set / add / remove
//!
//! 基于 unpack 产物目录：读 slide JSON → 定位 → 修改 → 写回。
//! 修改后可用 repack 命令重建 PPTX。

use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use crate::path::{self, Resolved};
use crate::product;
use json2pptx::model::elements::*;
use json2pptx::model::transition::Transition;
use json2pptx::model::Slide;
use serde_json::json;
use std::path::Path;

/// 解析 k=v 属性列表
pub fn parse_props(props: &[String]) -> Result<Vec<(String, String)>> {
    props
        .iter()
        .map(|p| {
            let (k, v) = p
                .split_once('=')
                .ok_or_else(|| format!("属性格式错误（应为 k=v）: {p}"))?;
            Ok((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

// === get ===

/// 输出节点精简结构（JSON）
pub fn get(slides: &[Slide], resolved: &Resolved) -> Result<serde_json::Value> {
    let slide = &slides[resolved.slide_idx];
    if resolved.chain.is_empty() {
        // slide 级：输出精简幻灯片结构
        let elements: Vec<serde_json::Value> = slide
            .elements
            .iter()
            .map(|el| {
                let mut n = json!({
                    "type": el.type_name(),
                    "name": el.name().unwrap_or_default(),
                    "id": el.id(),
                });
                if let Some(t) = el.text_content() {
                    n["text"] = json!(t);
                }
                n
            })
            .collect();
        return Ok(json!({
            "slide": resolved.slide_idx + 1,
            "background": slide.background,
            "transition": slide.transition,
            "elements": elements,
        }));
    }
    let el = path::get_element(slides, resolved)?;
    let mut node = json!({
        "type": el.type_name(),
        "name": el.name().unwrap_or_default(),
        "id": el.id(),
    });
    if let Some(t) = el.text_content() {
        node["text"] = json!(t);
    }
    let pos = el.position();
    node["position"] = json!({ "x": pos.x, "y": pos.y, "w": pos.w, "h": pos.h });
    Ok(node)
}

// === set ===

/// 修改幻灯片级属性（background / transition）
fn apply_slide_prop(slide: &mut Slide, key: &str, value: &str) -> Result<()> {
    match key {
        "background" => {
            let v = if value.trim_start().starts_with('{') {
                serde_json::from_str(value)
                    .map_err(|e| CliError::new(format!("background JSON 解析失败: {e}")))?
            } else {
                // 纯色：补 # 前缀（模型约定 slide 背景纯色带 #）
                let hex = if value.starts_with('#') {
                    value.to_string()
                } else {
                    format!("#{value}")
                };
                serde_json::Value::String(hex)
            };
            slide.background = Some(v);
        }
        "transition" => {
            let v: Transition = serde_json::from_str(value)
                .map_err(|e| CliError::new(format!("transition JSON 解析失败: {e}")))?;
            slide.transition = Some(v);
        }
        "notes" => {
            slide.notes = Some(value.to_string());
        }
        _ => {
            return Err(CliError::new(format!("不支持的幻灯片属性: {key}")).suggest(
                office_core::suggest::did_you_mean(key, &["background", "transition", "notes"], 4),
            ));
        }
    }
    Ok(())
}

/// 修改元素属性
fn apply_element_prop(el: &mut Element, key: &str, value: &str) -> Result<()> {
    match key {
        "text" => match el {
            Element::Text(t) => t.text = TextContent::Simple(value.to_string()),
            Element::Shape(s) => s.text = Some(TextContent::Simple(value.to_string())),
            _ => return Err(CliError::new("text 属性仅支持 text / shape 元素")),
        },
        "name" => match el {
            Element::Text(t) => t.name = Some(value.to_string()),
            Element::Shape(s) => s.name = Some(value.to_string()),
            Element::Line(l) => l.name = Some(value.to_string()),
            Element::Image(i) => i.name = Some(value.to_string()),
            Element::Table(t) => t.name = Some(value.to_string()),
            Element::Group(g) => g.name = Some(value.to_string()),
            _ => return Err(CliError::new("name 属性不支持该元素")),
        },
        "x" | "y" | "w" | "h" => {
            let v: f64 = value
                .parse()
                .map_err(|_| format!("{key} 必须是数字: {value}"))?;
            let pos = el.position_mut();
            match key {
                "x" => pos.x = v,
                "y" => pos.y = v,
                "w" => pos.w = v,
                _ => pos.h = v,
            }
        }
        "font_size" => {
            let v: f64 = value
                .parse()
                .map_err(|_| format!("font_size 必须是数字: {value}"))?;
            match el {
                Element::Text(t) => t.font_size = Some(v),
                Element::Shape(s) => s.font_size = Some(v),
                _ => return Err(CliError::new("font_size 属性仅支持 text / shape 元素")),
            }
        }
        "bold" => {
            let v: bool = value
                .parse()
                .map_err(|_| format!("bold 必须是 true/false: {value}"))?;
            match el {
                Element::Text(t) => t.bold = Some(v),
                _ => return Err(CliError::new("bold 属性仅支持 text 元素")),
            }
        }
        "color" => match el {
            Element::Text(t) => t.color = Some(value.to_string()),
            Element::Shape(s) => s.color = Some(value.to_string()),
            Element::Line(l) => l.color = Some(value.to_string()),
            _ => return Err(CliError::new("color 属性仅支持 text / shape / line 元素")),
        },
        "align" => match el {
            Element::Text(t) => t.align = Some(value.to_string()),
            Element::Shape(s) => s.align = Some(value.to_string()),
            _ => return Err(CliError::new("align 属性仅支持 text / shape 元素")),
        },
        "rotation" => {
            let v: f64 = value
                .parse()
                .map_err(|_| format!("rotation 必须是数字: {value}"))?;
            match el {
                Element::Shape(s) => s.rotation = Some(v),
                Element::Line(l) => l.rotation = Some(v),
                Element::Image(i) => i.rotation = Some(v),
                Element::Group(g) => g.rotation = Some(v),
                _ => return Err(CliError::new("rotation 属性不支持该元素")),
            }
        }
        "fill" => {
            let color = value.trim_start_matches('#').to_string();
            match el {
                Element::Shape(s) => s.fill = Some(Fill::Solid(color)),
                Element::Text(t) => t.fill = Some(Fill::Solid(color)),
                _ => return Err(CliError::new("fill 属性仅支持 text / shape 元素")),
            }
        }
        "shape_type" => match el {
            Element::Shape(s) => s.shape_type = value.to_string(),
            _ => return Err(CliError::new("shape_type 属性仅支持 shape 元素")),
        },
        "src" => match el {
            Element::Image(i) => i.src = value.to_string(),
            _ => return Err(CliError::new("src 属性仅支持 image 元素")),
        },
        // === line 元素 ===
        "width" => match el {
            Element::Line(l) => {
                let v: f64 = value
                    .parse()
                    .map_err(|_| format!("width 必须是数字: {value}"))?;
                l.width = Some(v);
            }
            _ => return Err(CliError::new("width 属性仅支持 line 元素")),
        },
        "dash" => match el {
            Element::Line(l) => l.dash = Some(value.to_string()),
            _ => return Err(CliError::new("dash 属性仅支持 line 元素")),
        },
        "arrow_start" => match el {
            Element::Line(l) => l.arrow_start = Some(value.to_string()),
            _ => return Err(CliError::new("arrow_start 属性仅支持 line 元素")),
        },
        "arrow_end" => match el {
            Element::Line(l) => l.arrow_end = Some(value.to_string()),
            _ => return Err(CliError::new("arrow_end 属性仅支持 line 元素")),
        },
        "points" => match el {
            Element::Line(l) => l.points = parse_points(value)?,
            _ => return Err(CliError::new("points 属性仅支持 line 元素")),
        },
        "smooth" => {
            let v: bool = value
                .parse()
                .map_err(|_| format!("smooth 必须是 true/false: {value}"))?;
            match el {
                Element::Line(l) => l.smooth = Some(v),
                Element::LineChart(c) => c.smooth = Some(v),
                _ => return Err(CliError::new("smooth 属性仅支持 line / line_chart 元素")),
            }
        }
        // === 图表组件 ===
        "data" => match el {
            Element::LineChart(c) => c.data = parse_csv_f64(value)?,
            Element::PieChart(c) => c.data = parse_csv_f64(value)?,
            Element::RingChart(c) => c.data = parse_csv_f64(value)?,
            _ => {
                return Err(CliError::new(
                    "data 属性仅支持 line_chart / pie_chart / ring_chart 元素",
                ))
            }
        },
        "labels" => match el {
            Element::LineChart(c) => c.labels = parse_csv_str(value),
            Element::PieChart(c) => c.labels = parse_csv_str(value),
            Element::RingChart(c) => c.labels = parse_csv_str(value),
            _ => {
                return Err(CliError::new(
                    "labels 属性仅支持 line_chart / pie_chart / ring_chart 元素",
                ))
            }
        },
        "colors" => match el {
            Element::LineChart(c) => c.colors = parse_csv_str(value),
            Element::PieChart(c) => c.colors = parse_csv_str(value),
            Element::RingChart(c) => c.colors = parse_csv_str(value),
            _ => {
                return Err(CliError::new(
                    "colors 属性仅支持 line_chart / pie_chart / ring_chart 元素",
                ))
            }
        },
        "max" => match el {
            Element::LineChart(c) => {
                let v: f64 = value
                    .parse()
                    .map_err(|_| format!("max 必须是数字: {value}"))?;
                c.max = Some(v);
            }
            _ => return Err(CliError::new("max 属性仅支持 line_chart 元素")),
        },
        "thickness" => match el {
            Element::RingChart(c) => {
                let v: f64 = value
                    .parse()
                    .map_err(|_| format!("thickness 必须是数字: {value}"))?;
                c.thickness = Some(v);
            }
            _ => return Err(CliError::new("thickness 属性仅支持 ring_chart 元素")),
        },
        "show_values" => {
            let v: bool = value
                .parse()
                .map_err(|_| format!("show_values 必须是 true/false: {value}"))?;
            match el {
                Element::LineChart(c) => c.show_values = Some(v),
                Element::PieChart(c) => c.show_values = Some(v),
                Element::RingChart(c) => c.show_values = Some(v),
                _ => {
                    return Err(CliError::new(
                        "show_values 属性仅支持 line_chart / pie_chart / ring_chart 元素",
                    ))
                }
            }
        }
        "axis" => match el {
            Element::LineChart(c) => {
                let v: bool = value
                    .parse()
                    .map_err(|_| format!("axis 必须是 true/false: {value}"))?;
                c.axis = Some(v);
            }
            _ => return Err(CliError::new("axis 属性仅支持 line_chart 元素")),
        },
        "id" => {
            let v: u32 = value
                .parse()
                .map_err(|_| CliError::new(format!("id 必须是正整数: {value}")))?;
            el.set_id(v);
        }
        _ => {
            let names: Vec<&str> = crate::capabilities::prop_names_for(el.type_name());
            return Err(
                CliError::new(format!("{} 元素不支持的属性: {key}", el.type_name()))
                    .suggest(office_core::suggest::did_you_mean(key, &names, 10)),
            );
        }
    }
    Ok(())
}

/// 解析逗号分隔的数值列表（data="10,20,30"）
fn parse_csv_f64(s: &str) -> Result<Vec<f64>> {
    s.split(',')
        .map(|p| {
            p.trim()
                .parse::<f64>()
                .map_err(|_| CliError::new(format!("数值列表格式错误（应为逗号分隔）: {s}")))
        })
        .collect()
}

/// 解析逗号分隔的字符串列表（labels="a,b,c"）
fn parse_csv_str(s: &str) -> Option<Vec<String>> {
    Some(
        s.split(',')
            .map(|p| p.trim().to_string())
            .collect::<Vec<_>>(),
    )
}

/// 解析顶点列表（points="x1,y1,x2,y2" 成对）
fn parse_points(s: &str) -> Result<Vec<(f64, f64)>> {
    let nums = parse_csv_f64(s)?;
    if nums.len() % 2 != 0 {
        return Err(CliError::new(format!(
            "points 必须成对出现（x1,y1,x2,y2）: {s}"
        )));
    }
    Ok(nums.chunks(2).map(|c| (c[0], c[1])).collect())
}

/// set 操作：幻灯片级或元素级
pub fn set(
    slides: &mut [Slide],
    resolved: &Resolved,
    props: &[(String, String)],
) -> Result<serde_json::Value> {
    if props.is_empty() {
        return Err(CliError::new("set 至少需要一个 --prop k=v"));
    }
    if resolved.chain.is_empty() {
        let slide = &mut slides[resolved.slide_idx];
        for (k, v) in props {
            apply_slide_prop(slide, k, v)?;
        }
    } else {
        let el = path::get_element_mut(slides, resolved)?;
        for (k, v) in props {
            apply_element_prop(el, k, v)?;
        }
    }
    Ok(json!({ "success": true, "updated": props.len() }))
}

// === add ===

/// 根据 --type 与属性构造新元素
fn build_element(type_name: &str, props: &[(String, String)]) -> Result<Element> {
    let get = |k: &str| {
        props
            .iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.clone())
    };
    let get_f = |k: &str, default: f64| -> Result<f64> {
        match get(k) {
            Some(v) => v
                .parse()
                .map_err(|_| CliError::new(format!("{k} 必须是数字: {v}"))),
            None => Ok(default),
        }
    };
    let position = Position {
        x: get_f("x", 1.0)?,
        y: get_f("y", 1.0)?,
        w: get_f("w", 4.0)?,
        h: get_f("h", 2.0)?,
    };

    match type_name {
        "text" => Ok(Element::Text(TextElement {
            placeholder: None,
            effects: None,
            text: TextContent::Simple(get("text").unwrap_or_default()),
            position,
            name: get("name"),
            font_size: get("font_size").and_then(|v| v.parse().ok()),
            bold: get("bold").and_then(|v| v.parse().ok()),
            color: get("color"),
            align: get("align"),
            ..Default::default()
        })),
        "shape" => Ok(Element::Shape(ShapeElement {
            placeholder: None,
            effects: None,
            shape_type: get("shape_type").unwrap_or_else(|| "rect".to_string()),
            position,
            name: get("name"),
            fill: get("fill").map(|c| Fill::Solid(c.trim_start_matches('#').to_string())),
            text: get("text").map(TextContent::Simple),
            color: get("color"),
            ..Default::default()
        })),
        "image" => Ok(Element::Image(ImageElement {
            brightness: None,
            contrast: None,
            fill_mode: None,
            tooltip: None,
            media: None,
            src: get("src").ok_or_else(|| CliError::new("image 元素必须提供 src 属性"))?,
            position,
            name: get("name"),
            ..Default::default()
        })),
        "group" => Ok(Element::Group(GroupElement {
            position,
            name: get("name"),
            children: Vec::new(),
            ..Default::default()
        })),
        "line" => Ok(Element::Line(LineElement {
            id: get("id").and_then(|v| v.parse().ok()),
            position,
            name: get("name"),
            points: match get("points") {
                Some(p) => parse_points(&p)?,
                None => Vec::new(),
            },
            color: get("color"),
            width: get("width").and_then(|v| v.parse().ok()),
            dash: get("dash"),
            arrow_start: get("arrow_start"),
            arrow_end: get("arrow_end"),
            smooth: get("smooth").and_then(|v| v.parse().ok()),
            rotation: None,
            animations: None,
        })),
        "line_chart" | "lineChart" => Ok(Element::LineChart(LineChart {
            position,
            data: match get("data") {
                Some(d) => parse_csv_f64(&d)?,
                None => Vec::new(),
            },
            labels: get("labels").and_then(|v| parse_csv_str(&v)),
            colors: get("colors").and_then(|v| parse_csv_str(&v)),
            max: get("max").and_then(|v| v.parse().ok()),
            show_values: get("show_values").and_then(|v| v.parse().ok()),
            axis: get("axis").and_then(|v| v.parse().ok()),
            smooth: get("smooth").and_then(|v| v.parse().ok()),
            label_color: get("label_color"),
            font_size: get("font_size").and_then(|v| v.parse().ok()),
        })),
        "pie_chart" | "pieChart" => Ok(Element::PieChart(PieChart {
            position,
            data: match get("data") {
                Some(d) => parse_csv_f64(&d)?,
                None => Vec::new(),
            },
            labels: get("labels").and_then(|v| parse_csv_str(&v)),
            colors: get("colors").and_then(|v| parse_csv_str(&v)),
            show_values: get("show_values").and_then(|v| v.parse().ok()),
            label_color: get("label_color"),
            font_size: get("font_size").and_then(|v| v.parse().ok()),
        })),
        "ring_chart" | "ringChart" => Ok(Element::RingChart(RingChart {
            position,
            data: match get("data") {
                Some(d) => parse_csv_f64(&d)?,
                None => Vec::new(),
            },
            labels: get("labels").and_then(|v| parse_csv_str(&v)),
            colors: get("colors").and_then(|v| parse_csv_str(&v)),
            thickness: get("thickness").and_then(|v| v.parse().ok()),
            show_values: get("show_values").and_then(|v| v.parse().ok()),
            label_color: get("label_color"),
            font_size: get("font_size").and_then(|v| v.parse().ok()),
        })),
        "progress_bar" | "progressBar" => Ok(Element::ProgressBar(ProgressBar {
            position,
            value: get("value").and_then(|v| v.parse().ok()),
            label: get("label"),
            color: get("color"),
            ..Default::default()
        })),
        other => {
            let types: Vec<&str> = crate::capabilities::ADD_TYPES
                .iter()
                .map(|(t, _)| *t)
                .collect();
            Err(CliError::new(format!("不支持的 add 类型: {other}"))
                .suggest(office_core::suggest::did_you_mean(other, &types, 10)))
        }
    }
}

/// add 操作：在路径指向的容器（slide 或 group）下追加元素
pub fn add(
    slides: &mut [Slide],
    resolved: &Resolved,
    type_name: &str,
    props: &[(String, String)],
) -> Result<serde_json::Value> {
    let mut element = build_element(type_name, props)?;
    // --prop id=N 显式指定稳定 ID（其余由 add_built 自动分配）
    if element.id().is_none() {
        if let Some(id) = props
            .iter()
            .find(|(k, _)| k == "id")
            .and_then(|(_, v)| v.parse::<u32>().ok())
        {
            element.set_id(id);
        }
    }
    add_built(slides, resolved, element, type_name)
}

/// 落容器：补稳定 ID → 同 id 幂等替换，否则追加
pub fn add_built(
    slides: &mut [Slide],
    resolved: &Resolved,
    mut element: Element,
    type_name: &str,
) -> Result<serde_json::Value> {
    // 稳定 ID：幻灯片内现有最大 id + 1（可用 --prop id=N 显式指定）
    if element.id().is_none() {
        let next = next_element_id(slides, resolved);
        element.set_id(next);
    }
    let id = element.id();
    let container = add_container(slides, resolved)?;
    // dump 回放幂等：容器内已有同 id 元素时原位替换（稳定 ID 寻址）
    if let Some(want) = id {
        if let Some(pos) = container.iter().position(|el| el.id() == Some(want)) {
            container[pos] = element;
            return Ok(json!({ "success": true, "added": type_name, "id": id, "replaced": true }));
        }
    }
    container.push(element);
    Ok(json!({ "success": true, "added": type_name, "id": id }))
}

/// 幻灯片内未占用的最小正整数 id（max+1）
fn next_element_id(slides: &[Slide], resolved: &Resolved) -> u32 {
    fn walk(els: &[Element], max: &mut u32) {
        for el in els {
            if let Some(i) = el.id() {
                *max = (*max).max(i);
            }
            if let Some(children) = el.children() {
                walk(children, max);
            }
        }
    }
    let mut max = 1u32;
    walk(&slides[resolved.slide_idx].elements, &mut max);
    max + 1
}

/// add 的目标容器：路径指向的节点本身（slide → slide.elements；group → 其 children）
fn add_container<'a>(slides: &'a mut [Slide], resolved: &Resolved) -> Result<&'a mut Vec<Element>> {
    let slide_idx = resolved.slide_idx;
    if resolved.chain.is_empty() {
        return Ok(&mut slides[slide_idx].elements);
    }
    let mut current: &mut Vec<Element> = &mut slides[slide_idx].elements;
    for &idx in &resolved.chain {
        let children = match &mut current[idx] {
            Element::Group(g) => &mut g.children,
            _ => {
                return Err(CliError::new(
                    "add 目标必须是 slide 或 group（路径中间节点不能是其他元素）",
                ))
            }
        };
        current = children;
    }
    Ok(current)
}

// === remove ===

/// remove 操作：删除路径指向的元素
pub fn remove(slides: &mut [Slide], resolved: &Resolved) -> Result<serde_json::Value> {
    if resolved.chain.is_empty() {
        return Err(CliError::new(
            "remove 需要定位到具体元素（如 /slide[1]/text[2]）",
        ));
    }
    let idx = *resolved
        .chain
        .last()
        .ok_or_else(|| CliError::new("路径未定位到元素"))?;
    let container = path::parent_elements(slides, resolved)?;
    if idx >= container.len() {
        return Err(CliError::new("元素索引越界"));
    }
    let removed = container.remove(idx);
    Ok(json!({
        "success": true,
        "removed": removed.type_name(),
    }))
}

// === 写回 ===

/// 将修改后的幻灯片写回产物
pub fn save(product_root: &Path, slides: &[Slide], resolved: &Resolved) -> Result<()> {
    product::save_slide(
        product_root,
        resolved.slide_idx,
        &slides[resolved.slide_idx],
    )
}
