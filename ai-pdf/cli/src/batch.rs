//! batch：回放 dump 风格的编辑指令（默认遇错即停，--force 跳过继续）
//!
//! 页面级 add（type=page）按页面 id 幂等原位替换；元素级 add 走
//! 整元素反序列化（dump 回放保真）或 k=v 构造（手写指令）。
//! 报告格式与三个 Office CLI 一致（office-core::batch::Report）。

use json2pdf::model::{DocumentModel, Element, PageModel};
use office_core::batch::{props_as_kv, Report, Step};
use serde_json::json;
use std::path::Path;

use crate::{edit, path as ppath, product, view};

type Result<T> = std::result::Result<T, office_core::CliError>;

/// 顺序执行指令；`stop_on_error` = true 时遇错即停（默认），false 对应 --force
pub fn run_steps(
    doc: &mut DocumentModel,
    root: &Path,
    pages: &mut Vec<PageModel>,
    steps: &[Step],
    stop_on_error: bool,
) -> serde_json::Value {
    let mut report = Report::default();
    for step in steps {
        match apply(doc, root, pages, step) {
            Ok(result) => report.push_ok(step, result),
            Err(e) => {
                report.push_err(step, &e.message, e.suggestion.as_deref());
                if stop_on_error {
                    break;
                }
            }
        }
    }
    report.to_value()
}

fn apply(
    doc: &mut DocumentModel,
    root: &Path,
    pages: &mut Vec<PageModel>,
    step: &Step,
) -> Result<serde_json::Value> {
    let op = step.op.as_str();
    match op {
        "add"
            if step.etype.as_deref() == Some("page")
                || step.props.get("type").and_then(|t| t.as_str()) == Some("page") =>
        {
            add_page(doc, root, pages, step)
        }
        _ => {
            let path = step.path.as_deref().ok_or_else(|| {
                office_core::CliError::new(format!("第 {} 条指令缺少 path", step.index))
                    .suggest(r#"格式: {"op":"set","path":"/page[1]/text[1]","props":{...}}"#)
            })?;
            let ep = ppath::parse(path)?;
            match op {
                "add" => add_element(doc, root, pages, &ep, step),
                "set" | "remove" | "get" | "view" => {
                    let resolved = ppath::resolve(pages, &ep)?;
                    match op {
                        "set" => {
                            let kv = props_as_kv(&step.props);
                            let props = edit::parse_props(&kv)?;
                            edit::set(pages, &resolved, &props)
                        }
                        "remove" => edit::remove(doc, root, pages, &resolved),
                        "get" => edit::get(doc, pages, &resolved),
                        "view" => {
                            let mode = step
                                .props
                                .get("mode")
                                .and_then(|m| m.as_str())
                                .unwrap_or("text");
                            match mode {
                                "text" => Ok(view::view_text(
                                    doc,
                                    &pages[resolved.page_idx],
                                    resolved.page_idx + 1,
                                    root,
                                )),
                                "layout" => Ok(view::view_layout(
                                    doc,
                                    &pages[resolved.page_idx],
                                    resolved.page_idx + 1,
                                )),
                                other => Err(office_core::CliError::new(format!(
                                    "未知 view 模式: {other}"
                                ))
                                .suggest("可用: text / layout")),
                            }
                        }
                        _ => unreachable!(),
                    }
                }
                other => Err(office_core::CliError::new(format!("未知 op: {other}"))
                    .suggest("可用: set / add / remove / get / view")),
            }
        }
    }
}

/// 整页 add：props 反序列化为 PageModel；id 相同的既有页原位替换（dump 回放幂等）
fn add_page(
    doc: &mut DocumentModel,
    root: &Path,
    pages: &mut Vec<PageModel>,
    step: &Step,
) -> Result<serde_json::Value> {
    let page: PageModel = serde_json::from_value(serde_json::Value::Object(step.props.clone()))
        .map_err(|e| {
            office_core::CliError::new(format!("page props 反序列化失败: {e}"))
                .suggest("dump 输出的整页 props 可直接回放；手写请给 {\"elements\":[...]}")
        })?;
    if let Some(id) = page.id {
        if let Some(pos) = pages.iter().position(|p| p.id == Some(id)) {
            pages[pos] = page;
            product::save_page(root, pos, &pages[pos])?;
            return Ok(json!({ "added": "page", "page": pos + 1, "replaced": true }));
        }
    }
    let mut page = page;
    if page.id.is_none() {
        page.id = Some(pages.iter().filter_map(|p| p.id).max().unwrap_or(0) + 1);
    }
    let rel = product::append_page(root, doc, &page)?;
    pages.push(page);
    Ok(json!({ "added": "page", "page": pages.len(), "file": rel }))
}

/// 元素级 add：props 优先整元素反序列化（dump 保真回放），失败再按 k=v 构造
fn add_element(
    _doc: &mut DocumentModel,
    root: &Path,
    pages: &mut [PageModel],
    ep: &ppath::ElementPath,
    step: &Step,
) -> Result<serde_json::Value> {
    let etype = step
        .etype
        .as_deref()
        .or_else(|| step.props.get("type").and_then(|t| t.as_str()))
        .unwrap_or("text");
    if etype == "page" {
        // /page[N] 容器上的 add page：指向错误
        return Err(office_core::CliError::new(
            "add page 的 path 必须是 /page（页面只能追加到文档）",
        ));
    }
    let resolved = ppath::resolve(pages, ep)?;
    let page_no = resolved.page_idx + 1;
    let built = if let Ok(el) =
        serde_json::from_value::<Element>(serde_json::Value::Object(step.props.clone()))
    {
        el
    } else {
        let kv = props_as_kv(&step.props);
        let props = edit::parse_props(&kv)?;
        edit::build_element(etype, &props)?
    };
    let t = ppath::type_name(&built);
    // 容器路径前缀
    let mut prefix = format!("/page[{page_no}]");
    {
        let page = &pages[resolved.page_idx];
        let mut current: &[Element] = &page.elements;
        for &idx in &resolved.chain {
            let el = &current[idx];
            let tn = ppath::type_name(el);
            let n = current
                .iter()
                .take(idx + 1)
                .filter(|e| ppath::type_name(e) == tn)
                .count();
            prefix.push_str(&format!("/{tn}[{n}]"));
            current = ppath::children(el).unwrap_or(&page.elements);
        }
    }
    let container = {
        let page_idx = resolved.page_idx;
        if resolved.chain.is_empty() {
            &mut pages[page_idx].elements
        } else {
            let mut cur: &mut Vec<Element> = &mut pages[page_idx].elements;
            for &idx in &resolved.chain {
                cur = ppath::children_mut(&mut cur[idx])
                    .ok_or_else(|| office_core::CliError::new("add 目标必须是 page 或 group"))?;
            }
            cur
        }
    };
    container.push(built);
    let idx = container
        .iter()
        .filter(|e| ppath::type_name(e) == t)
        .count();
    product::save_page(root, resolved.page_idx, &pages[resolved.page_idx])?;
    Ok(json!({ "added": etype, "path": format!("{prefix}/{t}[{idx}]") }))
}
