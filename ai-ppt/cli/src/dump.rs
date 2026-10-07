//! 序列化为可回放的编辑指令（batch JSON）
//!
//! 将演示文稿转成一组 `add`/`set` 指令，便于「读取样例 → 修改 → 回放」。
//! 结构与 `dump` 输出约定：
//! ```json
//! { "version": 1, "size": {...}, "commands": [ {"op":"add","path":"/slide[1]","type":"shape","props":{...}} ] }
//! ```

use json2pptx::model::Slide;
use serde_json::json;

/// 生成可回放指令
pub fn run(slides: &[Slide], width: f64, height: f64) -> serde_json::Value {
    let mut commands = Vec::new();
    for (i, slide) in slides.iter().enumerate() {
        let sp = format!("/slide[{}]", i + 1);
        let mut slide_props = serde_json::Map::new();
        if let Some(v) = &slide.background {
            slide_props.insert("background".into(), v.clone());
        }
        if let Some(t) = &slide.transition {
            slide_props.insert(
                "transition".into(),
                serde_json::to_value(t).unwrap_or(serde_json::Value::Null),
            );
        }
        if let Some(notes) = &slide.notes {
            slide_props.insert("notes".into(), json!(notes));
        }
        if !slide_props.is_empty() {
            commands.push(json!({ "op": "set", "path": sp, "props": slide_props }));
        }
        for el in &slide.elements {
            let props = serde_json::to_value(el).unwrap_or(serde_json::Value::Null);
            commands.push(json!({
                "op": "add",
                "path": sp,
                "type": el.type_name(),
                "props": props,
            }));
        }
    }
    json!({
        "version": 1,
        "size": { "width": width, "height": height },
        "commands": commands,
    })
}
