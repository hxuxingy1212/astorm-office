//! 从存量 PPTX 提取主题风格模板（template.json + TEMPLATE.md）。
//!
//! 与 json2docx `extract` / json2xlsx `extract` 对齐：从现有文档中提取可复用
//! 的风格要素（主题色板、字体、画布尺寸），供 agent 生成同类风格的新演示文稿。

use std::fs;
use std::path::Path;

use json2pptx::model::Presentation;

/// 用途说明（键 → 含义），用于生成 TEMPLATE.md
const COLOR_ROLES: &[(&str, &str)] = &[
    ("dk1", "深色（正文/文字主色）"),
    ("lt1", "浅色（背景主色）"),
    ("dk2", "深色辅助"),
    ("lt2", "浅色辅助"),
    ("accent1", "强调色 1（图表/形状默认）"),
    ("accent2", "强调色 2"),
    ("accent3", "强调色 3"),
    ("accent4", "强调色 4"),
    ("accent5", "强调色 5"),
    ("accent6", "强调色 6"),
    ("hlink", "超链接"),
    ("folHlink", "已访问超链接"),
];

/// 提取模板并写入 `dir`（template.json + TEMPLATE.md），返回 (目录, 颜色数)
pub fn write_template_dir(
    pres: &Presentation,
    dir: &Path,
    name: &str,
    source: &str,
) -> std::io::Result<(std::path::PathBuf, usize)> {
    fs::create_dir_all(dir)?;

    let theme = pres.theme.as_ref();
    let colors = theme.and_then(|t| t.colors.as_ref());
    let color_count = colors.map(|c| c.len()).unwrap_or(0);

    let template = serde_json::json!({
        "name": name,
        "source": source,
        "slide_size": { "width": pres.width, "height": pres.height },
        "theme": {
            "colors": colors,
            "major_font": theme.and_then(|t| t.major_font.clone()),
            "minor_font": theme.and_then(|t| t.minor_font.clone()),
        },
        "template": pres.template,
    });
    fs::write(
        dir.join("template.json"),
        serde_json::to_string_pretty(&template).unwrap_or_default(),
    )?;

    fs::write(dir.join("TEMPLATE.md"), markdown(pres, name, source))?;
    Ok((dir.to_path_buf(), color_count))
}

fn markdown(pres: &Presentation, name: &str, source: &str) -> String {
    let theme = pres.theme.as_ref();
    let colors = theme.and_then(|t| t.colors.as_ref());
    let mut md = String::new();
    md.push_str(&format!("# 风格模板：{name}\n\n"));
    md.push_str(&format!(
        "> 由 `json2pptx extract` 从 `{source}` 提取。\n\n"
    ));
    md.push_str("## 画布尺寸\n\n");
    md.push_str(&format!(
        "- 宽：{} 英寸\n- 高：{} 英寸\n\n",
        pres.width, pres.height
    ));
    md.push_str("## 主题色板\n\n");
    if let Some(colors) = colors {
        md.push_str("| 键 | 色值 | 用途 |\n|---|---|---|\n");
        for (key, role) in COLOR_ROLES {
            if let Some(v) = colors.get(*key) {
                md.push_str(&format!("| {key} | #{v} | {role} |\n"));
            }
        }
        for (key, v) in colors {
            if !COLOR_ROLES.iter().any(|(k, _)| k == key) {
                md.push_str(&format!("| {key} | #{v} | （其他） |\n"));
            }
        }
    } else {
        md.push_str("（源文件未包含主题色板）\n");
    }
    md.push_str("\n## 字体\n\n");
    if let Some(t) = theme {
        if let Some(f) = &t.major_font {
            md.push_str(&format!("- 标题字体（major）：{f}\n"));
        }
        if let Some(f) = &t.minor_font {
            md.push_str(&format!("- 正文字体（minor）：{f}\n"));
        }
    }
    md.push_str("\n## 用法\n\n");
    md.push_str("在 presentation.json 顶层加入：\n\n```json\n\"theme\": {\n  \"colors\": { \"dk1\": \"...\", \"lt1\": \"...\", \"accent1\": \"...\", ... },\n  \"major_font\": \"...\",\n  \"minor_font\": \"...\"\n}\n```\n\n");
    md.push_str("然后 `json2pptx repack` 重建；色值不带 `#`。\n");
    md
}
