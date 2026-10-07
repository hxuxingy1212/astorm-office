//! 分层 help：`help` / `help <元素>` / `help --json`（能力表驱动，与实现同源）。

use office_core::{suggest, CliError, Output};
use serde_json::json;

use crate::capabilities as caps;

/// 执行 help
pub fn run(topic: Option<&str>, out: &Output) -> Result<(), CliError> {
    match topic {
        Some(t) => {
            let el = resolve(t)?;
            if out.json_mode() {
                out.emit_json(&element_value(el))?;
            } else {
                print_element(el);
            }
            Ok(())
        }
        None => {
            if out.json_mode() {
                out.emit_json(&overview_value())?;
            } else {
                print_overview();
            }
            Ok(())
        }
    }
}

/// 元素名 → 能力条目（含纠错建议）
pub fn resolve(topic: &str) -> Result<&'static caps::ElementCaps, CliError> {
    let t = match topic.trim() {
        "chapter" | "parts" => "part",
        "paragraphs" => "paragraph",
        "headings" => "heading",
        "tables" => "table",
        "lists" => "list",
        "images" => "image",
        other => other,
    };
    caps::ELEMENTS
        .iter()
        .find(|e| e.element == t)
        .ok_or_else(|| {
            let names: Vec<&str> = caps::ELEMENTS.iter().map(|e| e.element).collect();
            CliError::new(format!("未知元素: {t}（可用: {}）", names.join("/")))
                .suggest(suggest::did_you_mean(t, &names, names.len()))
        })
}

/// 单元素能力 JSON
pub fn element_value(el: &caps::ElementCaps) -> serde_json::Value {
    let props = if el.element == "part" {
        caps::PART_PROPS
    } else {
        caps::PARAGRAPH_PROPS
    };
    json!({
        "tool": "json2docx",
        "element": el.element,
        "path": el.path,
        "note": el.note,
        "props": props.iter().map(|p| json!({
            "name": p.name,
            "example": format!("--prop {}={}", p.name, p.sample),
            "note": p.note,
            "verbs": ["set", "add"],
        })).collect::<Vec<_>>(),
    })
}

/// 概览能力 JSON
pub fn overview_value() -> serde_json::Value {
    json!({
        "tool": "json2docx",
        "elements": caps::ELEMENTS.iter().map(|e| json!({
            "element": e.element, "path": e.path, "note": e.note,
            "props": if e.element == "part" {
                caps::PART_PROPS.iter().map(|p| p.name).collect::<Vec<_>>()
            } else {
                caps::PARAGRAPH_PROPS.iter().map(|p| p.name).collect::<Vec<_>>()
            },
        })).collect::<Vec<_>>(),
        "add_types": caps::ADD_TYPES.iter().map(|(t, n)| json!({"type": t, "note": n})).collect::<Vec<_>>(),
        "view_modes": caps::VIEW_MODES.iter().map(|(t, n)| json!({"mode": t, "note": n})).collect::<Vec<_>>(),
        "batch_ops": caps::BATCH_OPS.iter().map(|(t, n)| json!({"op": t, "note": n})).collect::<Vec<_>>(),
    })
}

fn print_element(el: &caps::ElementCaps) {
    println!("{} — {}", el.element, el.path);
    println!("  {}", el.note);
    println!();
    println!("  可 set/add 的属性：");
    let props = if el.element == "part" {
        caps::PART_PROPS
    } else {
        caps::PARAGRAPH_PROPS
    };
    for p in props {
        println!(
            "    {:<20} {:<28} {}",
            p.name,
            format!("示例 {}", p.sample),
            p.note
        );
    }
}

fn print_overview() {
    println!("json2docx 能力速查（help <元素> 看单元素属性；--json 输出机器可读）\n");
    println!("元素寻址（段落支持 @paraId= 稳定寻址）：");
    for e in caps::ELEMENTS {
        println!("  {:<14} {:<44} {}", e.element, e.path, e.note);
    }
    println!("\nadd 类型（edit <输入> <路径> add --type T）：");
    for (t, n) in caps::ADD_TYPES {
        println!("  {t:<14} {n}");
    }
    println!("\nview 模式：");
    for (t, n) in caps::VIEW_MODES {
        println!("  {t:<8} {n}");
    }
    println!("\nbatch 支持的 op：");
    for (t, n) in caps::BATCH_OPS {
        println!("  {t:<8} {n}");
    }
}
