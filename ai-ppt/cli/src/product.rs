//! unpack 产物目录操作
//!
//! view / edit / render 的输入可以是 PPTX 文件（一次性解包到临时目录）或
//! unpack 产物目录（直接读写）。本模块只处理产物目录本身，不维护任何
//! 跨调用状态：产物由用户显式创建（`unpack`），修改即是对用户可见文件的修改。

use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use std::path::Path;

/// 判断目录是否为 unpack 产物根（含 presentation.json）
pub fn is_product_dir(path: &Path) -> bool {
    path.is_dir() && path.join("presentation.json").is_file()
}

/// 读取产物中的全部幻灯片（按 presentation.json 的 slides 清单）
pub fn load_slides(product_root: &Path) -> Result<Vec<json2pptx::model::Slide>> {
    let pres_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(product_root.join("presentation.json"))
            .map_err(|e| CliError::new(format!("读取 presentation.json 失败: {e}")))?,
    )
    .map_err(|e| CliError::new(format!("解析 presentation.json 失败: {e}")))?;

    let slide_paths: Vec<String> = pres_json["slides"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut slides = Vec::new();
    for rel in &slide_paths {
        let slide: json2pptx::model::Slide = serde_json::from_str(
            &std::fs::read_to_string(product_root.join(rel))
                .map_err(|e| CliError::new(format!("读取 {rel} 失败: {e}")))?,
        )
        .map_err(|e| CliError::new(format!("解析 {rel} 失败: {e}")))?;
        slides.push(slide);
    }
    Ok(slides)
}

/// 将幻灯片写回产物（按 slides 清单中的对应路径）
pub fn save_slide(
    product_root: &Path,
    slide_idx: usize, // 0-based
    slide: &json2pptx::model::Slide,
) -> Result<()> {
    let pres_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(product_root.join("presentation.json"))
            .map_err(|e| CliError::new(format!("读取 presentation.json 失败: {e}")))?,
    )
    .map_err(|e| CliError::new(format!("解析 presentation.json 失败: {e}")))?;

    let rel = pres_json["slides"]
        .as_array()
        .and_then(|a| a.get(slide_idx))
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("幻灯片索引越界: {slide_idx}"))?;

    let json = serde_json::to_string_pretty(slide).map_err(|e| e.to_string())?;
    std::fs::write(product_root.join(rel), json).map_err(|e| e.to_string())?;
    Ok(())
}

/// 将全部幻灯片写回产物（serve / batch 用）
pub fn save_slides(product_root: &Path, slides: &[json2pptx::model::Slide]) -> Result<()> {
    for (i, s) in slides.iter().enumerate() {
        save_slide(product_root, i, s)?;
    }
    Ok(())
}

/// 读取产物中的演示文稿尺寸（宽、高，英寸）
pub fn presentation_size(product_root: &Path) -> Result<(f64, f64)> {
    let pres_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(product_root.join("presentation.json"))
            .map_err(|e| CliError::new(format!("读取 presentation.json 失败: {e}")))?,
    )
    .map_err(|e| CliError::new(format!("解析 presentation.json 失败: {e}")))?;
    let w = pres_json["width"]
        .as_f64()
        .ok_or_else(|| CliError::new("presentation.json 缺少 width"))?;
    let h = pres_json["height"]
        .as_f64()
        .ok_or_else(|| CliError::new("presentation.json 缺少 height"))?;
    Ok((w, h))
}

/// 构造与 web 后端一致的 overview：元数据 + 每页幻灯片 JSON + 媒体清单
pub fn overview_json(product_root: &Path) -> Result<serde_json::Value> {
    let read = |rel: &str| -> Result<serde_json::Value> {
        let txt = std::fs::read_to_string(product_root.join(rel))
            .map_err(|e| CliError::new(format!("读取 {} 失败: {e}", rel)))?;
        serde_json::from_str(&txt).map_err(|e| CliError::new(format!("解析 {} 失败: {e}", rel)))
    };

    let pres = read("presentation.json")?;
    let slide_paths: Vec<String> = pres["slides"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut slides = Vec::new();
    for (i, rel) in slide_paths.iter().enumerate() {
        slides.push(serde_json::json!({
            "index": i + 1,
            "data": read(rel)?,
        }));
    }

    let mut media: Vec<String> = Vec::new();
    let media_dir = product_root.join("ppt").join("media");
    if let Ok(entries) = std::fs::read_dir(&media_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str().map(String::from) {
                media.push(format!("ppt/media/{name}"));
            }
        }
    }
    media.sort();

    Ok(serde_json::json!({
        "name": pres["name"].as_str().unwrap_or(""),
        "width": pres["width"],
        "height": pres["height"],
        "meta": pres["meta"],
        "theme": pres["theme"],
        "slides": slides,
        "media": media,
    }))
}

/// 将媒体 src 解析为绝对路径（URL 保留原样）
///
/// 产物中的 src 为 `ppt/media/...`（相对产物根），
/// 这里返回产物根下该文件的绝对路径。
pub fn media_abs_path(product_root: &Path, src: &str) -> String {
    if src.starts_with("http://") || src.starts_with("https://") {
        return src.to_string();
    }
    product_root
        .join(src)
        .canonicalize()
        .unwrap_or_else(|_| product_root.join(src))
        .to_string_lossy()
        .to_string()
}
