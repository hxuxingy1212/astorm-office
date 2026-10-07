//! 修改层：get / set / add / remove（对齐 ai-ppt/cli 的 edit.rs）
//!
//! 基于 unpack 产物目录：读页面 JSON → 定位 → 修改 → 写回。
//! 修改后可用 repack 命令重建 PDF。PDF 产物目录颜色**带 `#`**。

use office_core::{suggest, CliError};
use serde_json::json;
use std::path::Path;

use crate::capabilities as caps;
use crate::path::{self, Resolved};
use crate::product;

type Result<T> = std::result::Result<T, CliError>;
use json2pdf::model::{DocumentModel, Element, PageModel};

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

/// 输出节点 JSON
pub fn get(
    doc: &DocumentModel,
    pages: &[PageModel],
    resolved: &Resolved,
) -> Result<serde_json::Value> {
    let page_no = resolved.page_idx + 1;
    let page = &pages[resolved.page_idx];
    if resolved.chain.is_empty() {
        let (w, h) = product::page_size(doc, page);
        let elements: Vec<serde_json::Value> = page
            .elements
            .iter()
            .enumerate()
            .map(|(i, el)| {
                let chain = [i];
                let mut n = json!({
                    "path": path::element_path(page_no, &page.elements, &chain),
                    "type": path::type_name(el),
                });
                if let Some(t) = path::text_of(el) {
                    n["text"] = json!(t);
                }
                n
            })
            .collect();
        return Ok(json!({
            "page": page_no,
            "width": w,
            "height": h,
            "rotation": page.rotation,
            "crop": page.crop,
            "elements": elements,
        }));
    }
    let el = path::get_element(pages, resolved)?;
    let mut node = serde_json::to_value(el).map_err(|e| CliError::new(e.to_string()))?;
    let summary = json!({
        "path": path::element_path(page_no, &page.elements, &resolved.chain),
        "type": path::type_name(el),
    });
    if let (Some(obj), Some(sum)) = (node.as_object_mut(), summary.as_object()) {
        // path/type 放最前（保留其余字段）
        let mut out = serde_json::Map::new();
        for (k, v) in sum {
            out.insert(k.clone(), v.clone());
        }
        for (k, v) in obj {
            out.insert(k.clone(), v.clone());
        }
        node = serde_json::Value::Object(out);
    }
    Ok(node)
}

// === set ===

/// 颜色规范化：带 `#`；`none` → None（不填充/不描边）
fn as_color(value: &str) -> Option<String> {
    let v = value.trim();
    if v.eq_ignore_ascii_case("none") || v.is_empty() {
        return None;
    }
    Some(if v.starts_with('#') {
        v.to_string()
    } else {
        format!("#{v}")
    })
}

fn parse_f64(key: &str, value: &str) -> Result<f64> {
    value
        .parse()
        .map_err(|_| CliError::new(format!("{key} 必须是数字: {value}")))
}

/// 修改页级属性
pub fn apply_page_prop(page: &mut PageModel, key: &str, value: &str) -> Result<()> {
    match key {
        "width" => page.width = Some(parse_f64(key, value)?),
        "height" => page.height = Some(parse_f64(key, value)?),
        "rotation" => {
            let v: u32 = value
                .parse()
                .map_err(|_| CliError::new(format!("rotation 必须是整数: {value}")))?;
            if !matches!(v, 0 | 90 | 180 | 270) {
                return Err(CliError::new(format!("rotation 仅支持 0/90/180/270: {v}")));
            }
            page.rotation = v;
        }
        "crop" => {
            let v: serde_json::Value = serde_json::from_str(value).map_err(|e| {
                CliError::new(format!("crop JSON 解析失败: {e}"))
                    .suggest(r#"格式: --prop crop=[50,60,545,780]（[x0,y0,x1,y1]，pt）"#)
            })?;
            let arr: [f64; 4] = serde_json::from_value(v.clone()).map_err(|_| {
                CliError::new("crop 必须是 4 元素数组")
                    .suggest(r#"格式: --prop crop=[50,60,545,780]（[x0,y0,x1,y1]，pt）"#)
            })?;
            page.crop = Some(arr);
        }
        _ => {
            let names = caps::prop_names_for("page");
            return Err(CliError::new(format!("不支持的页面属性: {key}"))
                .suggest(suggest::did_you_mean(key, &names, 8)));
        }
    }
    Ok(())
}

/// 修改元素属性（变体逐一定位；能力表驱动纠错建议）
fn apply_element_prop(el: &mut Element, key: &str, value: &str) -> Result<()> {
    let t = path::type_name(el);
    match (key, el) {
        // === 通用几何 ===
        (
            "x",
            Element::Text { x, .. }
            | Element::Rect { x, .. }
            | Element::PatternRect { x, .. }
            | Element::Image { x, .. },
        ) => {
            *x = parse_f64(key, value)?;
        }
        (
            "y",
            Element::Text { y, .. }
            | Element::Rect { y, .. }
            | Element::PatternRect { y, .. }
            | Element::Image { y, .. },
        ) => {
            *y = parse_f64(key, value)?;
        }
        (
            "w",
            Element::Rect { w, .. } | Element::PatternRect { w, .. } | Element::Image { w, .. },
        ) => {
            *w = parse_f64(key, value)?;
        }
        (
            "h",
            Element::Rect { h, .. } | Element::PatternRect { h, .. } | Element::Image { h, .. },
        ) => {
            *h = parse_f64(key, value)?;
        }
        (
            "rotation",
            Element::Text { rotation, .. }
            | Element::Rect { rotation, .. }
            | Element::Image { rotation, .. },
        ) => {
            *rotation = parse_f64(key, value)?;
        }
        (
            "alpha",
            Element::Text { alpha, .. }
            | Element::Rect { alpha, .. }
            | Element::PatternRect { alpha, .. }
            | Element::Path { alpha, .. }
            | Element::Image { alpha, .. }
            | Element::Group { alpha, .. },
        ) => {
            let v = parse_f64(key, value)?;
            if !(0.0..=1.0).contains(&v) {
                return Err(CliError::new("alpha 取值范围 0-1").suggest("如 --prop alpha=0.5"));
            }
            *alpha = v;
        }
        // === text ===
        ("text", Element::Text { text, .. }) => *text = value.to_string(),
        ("size", Element::Text { size, .. }) => *size = parse_f64(key, value)?,
        ("font", Element::Text { font, .. }) => *font = value.to_string(),
        ("color", Element::Text { color, .. }) => match as_color(value) {
            Some(c) => *color = c,
            None => return Err(CliError::new("color 不能为 none（文本必须有颜色）")),
        },
        ("bold", Element::Text { bold, .. }) => *bold = parse_bool(key, value)?,
        ("italic", Element::Text { italic, .. }) => *italic = parse_bool(key, value)?,
        ("stroke_color", Element::Text { stroke_color, .. }) => *stroke_color = as_color(value),
        // === rect / path ===
        ("fill", Element::Rect { fill, .. } | Element::Path { fill, .. }) => {
            *fill = as_color(value)
        }
        ("stroke", Element::Rect { stroke, .. } | Element::Path { stroke, .. }) => {
            *stroke = as_color(value)
        }
        ("stroke", Element::Polyline { stroke, .. }) => match as_color(value) {
            Some(c) => *stroke = c,
            None => return Err(CliError::new("stroke 不能为 none（polyline 必须有描边色）")),
        },
        (
            "line_width",
            Element::Rect { line_width, .. }
            | Element::Path { line_width, .. }
            | Element::Polyline { line_width, .. },
        ) => {
            *line_width = parse_f64(key, value)?;
        }
        // === polyline ===
        ("points", Element::Polyline { points, .. }) => *points = parse_points(value)?,
        ("close", Element::Polyline { close, .. }) => *close = parse_bool(key, value)?,
        // === image ===
        ("src", Element::Image { src, .. }) => *src = value.to_string(),
        _ => {
            let names = caps::prop_names_for(t);
            return Err(CliError::new(format!("{t} 元素不支持的属性: {key}"))
                .suggest(suggest::did_you_mean(key, &names, 10)));
        }
    }
    Ok(())
}

fn parse_bool(key: &str, value: &str) -> Result<bool> {
    value
        .parse()
        .map_err(|_| CliError::new(format!("{key} 必须是 true/false: {value}")))
}

/// 解析顶点列表："x1,y1 x2,y2"（空格分隔对）或 "x1,y1,x2,y2"（逗号偶数个）
fn parse_points(s: &str) -> Result<Vec<[f64; 2]>> {
    let nums: Result<Vec<f64>> = s
        .split([',', ' '])
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            p.trim()
                .parse::<f64>()
                .map_err(|_| CliError::new(format!("顶点必须是数字: {p}")))
        })
        .collect();
    let nums = nums?;
    if nums.is_empty() || nums.len() % 2 != 0 {
        return Err(CliError::new(format!(
            "points 必须成对出现（x1,y1 x2,y2）: {s}"
        )));
    }
    Ok(nums.chunks(2).map(|c| [c[0], c[1]]).collect())
}

/// set 操作：页级或元素级
pub fn set(
    pages: &mut [PageModel],
    resolved: &Resolved,
    props: &[(String, String)],
) -> Result<serde_json::Value> {
    if props.is_empty() {
        return Err(CliError::new("set 至少需要一个 --prop k=v"));
    }
    if resolved.chain.is_empty() {
        let page = &mut pages[resolved.page_idx];
        for (k, v) in props {
            apply_page_prop(page, k, v)?;
        }
    } else {
        let el = path::get_element_mut(pages, resolved)?;
        for (k, v) in props {
            apply_element_prop(el, k, v)?;
        }
    }
    Ok(json!({ "success": true, "updated": props.len() }))
}

// === add ===

/// 单个 k=v 属性 → JSON 值（数字/布尔/JSON 数组自动识别；fill/stroke 的 none → null）
fn prop_to_value(key: &str, v: &str) -> serde_json::Value {
    match key {
        "points" => {
            return parse_points(v)
                .map(|pts| json!(pts))
                .unwrap_or_else(|_| json!(v));
        }
        "crop" | "mediabox" => {
            if let Ok(arr) = serde_json::from_str::<[f64; 4]>(v) {
                return json!(arr);
            }
        }
        "fill" | "stroke" | "stroke_color" if v.eq_ignore_ascii_case("none") => {
            return serde_json::Value::Null;
        }
        _ => {}
    }
    if v == "true" || v == "false" {
        return json!(v == "true");
    }
    if let Ok(n) = v.parse::<f64>() {
        return json!(n);
    }
    if v.starts_with('[') || v.starts_with('{') {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(v) {
            return parsed;
        }
    }
    json!(v)
}

/// 各类型的 add 默认属性（模型必填字段；用户显式 --prop 覆盖默认）
fn add_defaults(type_name: &str) -> &'static [(&'static str, &'static str)] {
    match type_name {
        "text" => &[("x", "72"), ("y", "720")],
        "rect" => &[("x", "50"), ("y", "700"), ("w", "200"), ("h", "40")],
        "image" => &[("x", "100"), ("y", "400"), ("w", "200"), ("h", "150")],
        "polyline" => &[("points", "0,0 100,100")],
        "group" => &[
            ("matrix", "[1,0,0,1,0,0]"),
            ("bbox", "[0,0,200,200]"),
            ("children", "[]"),
        ],
        _ => &[],
    }
}

/// 根据 --type 与属性构造新元素。
/// 走 serde 反序列化：字段默认值与产物目录模型单一来源；
/// path/shading/pattern_rect 的复杂结构也可整体用 JSON 属性给出（batch 回放同路径）。
pub fn build_element(type_name: &str, props: &[(String, String)]) -> Result<Element> {
    if !caps::ADD_TYPES.iter().any(|(t, _)| *t == type_name) {
        let types: Vec<&str> = caps::ADD_TYPES.iter().map(|(t, _)| *t).collect();
        return Err(CliError::new(format!("不支持的 add 类型: {type_name}"))
            .suggest(suggest::did_you_mean(type_name, &types, 10)));
    }
    let mut obj = serde_json::Map::new();
    obj.insert("type".into(), json!(type_name));
    for (k, v) in add_defaults(type_name) {
        obj.insert((*k).to_string(), prop_to_value(k, v));
    }
    for (k, v) in props {
        if k == "type" {
            continue;
        }
        obj.insert(k.clone(), prop_to_value(k, v));
    }
    serde_json::from_value::<Element>(serde_json::Value::Object(obj)).map_err(|e| {
        CliError::new(format!("构造 {type_name} 元素失败: {e}"))
            .suggest("用 json2pdf help <类型> 查看属性；image 必须提供 src，文本建议提供 text/x/y")
    })
}

/// add 的目标容器：路径指向的节点本身（page → page.elements；group → 其 children）
fn add_container<'a>(
    pages: &'a mut [PageModel],
    resolved: &Resolved,
) -> Result<&'a mut Vec<Element>> {
    let page_idx = resolved.page_idx;
    if resolved.chain.is_empty() {
        return Ok(&mut pages[page_idx].elements);
    }
    let mut current: &mut Vec<Element> = &mut pages[page_idx].elements;
    for &idx in &resolved.chain {
        let kids = match path::children_mut(&mut current[idx]) {
            Some(k) => k,
            None => {
                return Err(CliError::new(
                    "add 目标必须是 page 或 group（路径中间节点不能是其他元素）",
                ))
            }
        };
        current = kids;
    }
    Ok(current)
}

/// add 操作：type=page 时 path 必须是 /page（文档容器）；其余向容器追加元素
pub fn add(
    doc: &mut DocumentModel,
    root: &Path,
    pages: &mut Vec<PageModel>,
    resolved: &Resolved,
    ep: &path::ElementPath,
    type_name: &str,
    props: &[(String, String)],
) -> Result<serde_json::Value> {
    if type_name == "page" {
        if !ep.elements.is_empty() {
            return Err(
                CliError::new("add page 的 path 必须是 /page（页面只能追加到文档）").suggest(
                    "用法: json2pdf edit <产物目录> /page add --type page --prop width=595.28",
                ),
            );
        }
        if ep.page_explicit {
            return Err(CliError::new("add page 的 path 不应带索引")
                .suggest("用法: json2pdf edit <产物目录> /page add --type page"));
        }
        let mut page = PageModel::default();
        for (k, v) in props {
            apply_page_prop(&mut page, k, v)?;
        }
        let rel = product::append_page(root, doc, &page)?;
        pages.push(page);
        return Ok(json!({
            "success": true,
            "added": "page",
            "page": pages.len(),
            "file": rel,
        }));
    }

    let page_no = resolved.page_idx + 1;
    let element = build_element(type_name, props)?;
    let t = path::type_name(&element);
    // 容器路径前缀（含 group 链）
    let mut prefix = format!("/page[{page_no}]");
    {
        let page = &pages[resolved.page_idx];
        let mut current: &[Element] = &page.elements;
        for &idx in &resolved.chain {
            let el = &current[idx];
            let tn = path::type_name(el);
            let n = current
                .iter()
                .take(idx + 1)
                .filter(|e| path::type_name(e) == tn)
                .count();
            prefix.push_str(&format!("/{tn}[{n}]"));
            current = path::children(el).unwrap_or(&page.elements); // chain 已校验，必为 group
        }
    }
    let container = add_container(pages, resolved)?;
    container.push(element);
    let idx = container.iter().filter(|e| path::type_name(e) == t).count();
    Ok(json!({
        "success": true,
        "added": type_name,
        "path": format!("{prefix}/{t}[{idx}]"),
    }))
}

// === remove ===

/// remove 操作：删除路径指向的元素；/page[N] 删除整页
pub fn remove(
    doc: &mut DocumentModel,
    root: &Path,
    pages: &mut Vec<PageModel>,
    resolved: &Resolved,
) -> Result<serde_json::Value> {
    if resolved.chain.is_empty() {
        let page_no = resolved.page_idx + 1;
        if pages.len() <= 1 {
            return Err(CliError::new("至少要保留一页，不能删除最后一页"));
        }
        product::remove_page(root, doc, resolved.page_idx)?;
        pages.remove(resolved.page_idx);
        return Ok(json!({ "success": true, "removed": "page", "page": page_no }));
    }
    let idx = *resolved
        .chain
        .last()
        .ok_or_else(|| CliError::new("路径未定位到元素"))?;
    let container = path::parent_elements(pages, resolved)?;
    if idx >= container.len() {
        return Err(CliError::new("元素索引越界"));
    }
    let removed = path::type_name(&container[idx]);
    container.remove(idx);
    Ok(json!({ "success": true, "removed": removed }))
}
