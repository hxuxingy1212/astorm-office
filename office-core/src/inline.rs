//! 产物目录 → 单文件自包含 JSON（`unpack --inline`）。
//!
//! 把 unpack 产物目录组装成一个自包含 JSON 文件：
//! - 顶层数组分片（presentation.json 的 `slides` / document.json 的 `parts`、`pages` /
//!   workbook.json 的 `sheets`）由相对路径展开为内联对象（web 端 loader 对内联形态直接使用）；
//! - 媒体引用（解析后落在产物目录 `media` 段下的相对路径字符串，如 `ppt/media/img.png`、
//!   `xl/media/image1.png`）改写为 data URI（web 端 media resolver 对 `data:` 直通）；
//!   `fonts/`、`embeddings/` 不内联——预览渲染不消费，转了只是体积膨胀。
//!
//! 单文件产物可直接经 IPC/内存传给 `@astorm/office-viewer` 的 `data` prop，
//! 省去 1+N 次 fetch 与自定义协议；编辑流程请继续用目录形态（repack 只认目录）。

use crate::error::CliError;
use crate::guards::TempGuard;
use base64::Engine as _;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// 顶层的候选文件名（按格式恰有一个）
const TOP_FILES: &[&str] = &["presentation.json", "document.json", "workbook.json"];
/// 顶层的分片数组字段名
const SHARD_KEYS: &[&str] = &["slides", "parts", "sheets", "pages"];

#[derive(Debug)]
pub struct InlineProduct {
    /// 单文件产物（分片已内联、媒体已转 data URI）
    pub json: Value,
    /// 内联的分片数量（slides/parts/sheets/pages 总和）
    pub shards: usize,
}

pub struct InlineOutcome {
    /// 写出的单文件路径
    pub output: PathBuf,
    /// 内联的分片数量
    pub shards: usize,
}

/// 把产物目录组装成自包含 JSON（不写盘）。
pub fn inline_product(dir: &Path) -> Result<InlineProduct, CliError> {
    let top_name = TOP_FILES
        .iter()
        .find(|f| dir.join(f).is_file())
        .ok_or_else(|| {
            CliError::with_code(
                "invalid_input",
                format!(
                    "不是产物目录：{} 下未找到 {}",
                    dir.display(),
                    TOP_FILES.join(" / ")
                ),
            )
        })?;
    let top_text = std::fs::read_to_string(dir.join(top_name)).map_err(CliError::from)?;
    let mut top: Value = serde_json::from_str(&top_text)
        .map_err(|e| CliError::with_code("json", format!("解析 {top_name} 失败: {e}")))?;

    let mut shards = 0usize;
    if let Some(obj) = top.as_object_mut() {
        for key in SHARD_KEYS {
            let Some(Value::Array(items)) = obj.get_mut(*key) else {
                continue;
            };
            for item in items.iter_mut() {
                let Value::String(rel) = item else {
                    continue; // 已内联的形态直接保留
                };
                let text = std::fs::read_to_string(dir.join(rel.as_str()))
                    .map_err(|e| CliError::with_code("io", format!("读取分片 {rel} 失败: {e}")))?;
                let v: Value = serde_json::from_str(&text).map_err(|e| {
                    CliError::with_code("json", format!("解析分片 {rel} 失败: {e}"))
                })?;
                *item = v;
                shards += 1;
            }
        }
    }

    rewrite_media(&mut top, dir);
    Ok(InlineProduct { json: top, shards })
}

/// `unpack` 到临时目录，内联成单个自包含 JSON 写到 `out_file`，临时目录自动清理。
///
/// `unpack` 由各 CLI 适配为统一签名 `(输入, 产物目录)`；错误经各 CLI 既有的
/// `From` 转换进 [`CliError`]，保留原始错误码（如 encrypted）。
pub fn unpack_inline(
    input: &Path,
    out_file: &Path,
    unpack: impl Fn(&Path, &Path) -> Result<(), CliError>,
) -> Result<InlineOutcome, CliError> {
    let tmp = std::env::temp_dir().join(format!(
        "office-inline-{}-{}-{}",
        input.file_stem().and_then(|s| s.to_str()).unwrap_or("doc"),
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _guard = TempGuard { path: tmp.clone() };
    unpack(input, &tmp)?;
    let inlined = inline_product(&tmp)?;
    if let Some(parent) = out_file.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(CliError::from)?;
        }
    }
    let json = serde_json::to_string_pretty(&inlined.json)
        .map_err(|e| CliError::with_code("json", e.to_string()))?;
    std::fs::write(out_file, json).map_err(CliError::from)?;
    Ok(InlineOutcome {
        output: out_file.to_path_buf(),
        shards: inlined.shards,
    })
}

/// 递归改写：值是字符串且解析为产物目录下真实存在的 `media` 段文件 → data URI。
fn rewrite_media(v: &mut Value, dir: &Path) {
    match v {
        Value::String(s) => {
            if let Some(uri) = to_data_uri(s, dir) {
                *s = uri;
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|i| rewrite_media(i, dir)),
        Value::Object(map) => map.values_mut().for_each(|x| rewrite_media(x, dir)),
        _ => {}
    }
}

fn to_data_uri(s: &str, dir: &Path) -> Option<String> {
    if s.is_empty()
        || s.starts_with("http://")
        || s.starts_with("https://")
        || s.starts_with("data:")
        || s.starts_with('/')
    {
        return None;
    }
    // 限定 media 段，避免误伤恰好长得像路径的正文文本
    if !s.split(['/', '\\']).any(|seg| seg == "media") {
        return None;
    }
    let path = dir.join(s);
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    Some(format!(
        "data:{};base64,{}",
        mime_for(s),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        "emf" => "image/emf",
        "wmf" => "image/wmf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 搭一个最小 pptx 产物目录：顶层 + 分片 + 媒体
    fn fixture(dir: &Path) {
        std::fs::create_dir_all(dir.join("ppt/slides")).unwrap();
        std::fs::create_dir_all(dir.join("ppt/media")).unwrap();
        std::fs::write(
            dir.join("presentation.json"),
            r#"{"width":13.333,"height":7.5,"slides":["ppt/slides/slide1.json"]}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("ppt/slides/slide1.json"),
            r#"{"elements":[{"type":"image","src":"ppt/media/img.png"}],"notes":"ppt/media/img.png 不存在同名字段时不应误伤正文"}"#,
        )
        .unwrap();
        // 1x1 PNG
        let png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
            0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        std::fs::write(dir.join("ppt/media/img.png"), png).unwrap();
    }

    #[test]
    fn inline_product_expands_shards_and_media() {
        let dir = std::env::temp_dir().join(format!("office-inline-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        fixture(&dir);
        let out = inline_product(&dir).unwrap();
        assert_eq!(out.shards, 1);
        let slides = out.json["slides"].as_array().unwrap();
        assert!(slides[0].is_object(), "分片应展开为对象");
        let src = slides[0]["elements"][0]["src"].as_str().unwrap();
        assert!(
            src.starts_with("data:image/png;base64,"),
            "媒体应转为 data URI，实际: {src}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inline_product_rejects_non_product_dir() {
        let dir = std::env::temp_dir().join(format!("office-inline-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let err = inline_product(&dir).unwrap_err();
        assert_eq!(err.code, "invalid_input");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unpack_inline_roundtrip_writes_single_file() {
        let src = std::env::temp_dir().join(format!("office-inline-rt-src-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&src);
        fixture(&src);
        let out_file =
            std::env::temp_dir().join(format!("inline-rt-out-{}.json", std::process::id()));
        // 模拟真实 unpack：把 fixture 复制到指定的产物目录
        let res = unpack_inline(&src, &out_file, |_, o| {
            fixture(o);
            Ok(())
        })
        .unwrap();
        assert_eq!(res.shards, 1);
        let text = std::fs::read_to_string(&out_file).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert!(v["slides"][0].is_object());
        let _ = std::fs::remove_file(&out_file);
        let _ = std::fs::remove_dir_all(&src);
    }
}
