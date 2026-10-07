//! 产物目录操作（对应 ai-ppt 的 product.rs）

use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use json2docx::model::{Document, Part};
use std::path::{Path, PathBuf};

// 共享设施来自 office-core（保持 product:: 前缀的既有调用点不变）
pub(crate) use office_core::path_str;
pub use office_core::{FileGuard, TempGuard};

/// 判断目录是否为产物根（含 document.json）
pub fn is_product_dir(path: &Path) -> bool {
    path.is_dir() && path.join("document.json").is_file()
}

/// 读取 document.json 中的 parts 路径清单
pub fn parts_paths(root: &Path) -> Result<Vec<String>> {
    let txt = std::fs::read_to_string(root.join("document.json"))
        .map_err(|e| CliError::new(format!("读取 document.json 失败: {e}")))?;
    let v: serde_json::Value = serde_json::from_str(&txt)
        .map_err(|e| CliError::new(format!("解析 document.json 失败: {e}")))?;
    Ok(v.get("parts")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

/// 读取产物目录为内存 Document
pub fn load_document(root: &Path) -> Result<Document> {
    let txt = std::fs::read_to_string(root.join("document.json"))
        .map_err(|e| CliError::new(format!("读取 document.json 失败: {e}")))?;
    let mut v: serde_json::Value = serde_json::from_str(&txt)
        .map_err(|e| CliError::new(format!("解析 document.json 失败: {e}")))?;
    let paths = parts_paths(root)?;
    // 内联 parts（单文件 document.json）
    let inline_parts: Vec<Part> = v
        .get("parts")
        .and_then(|p| p.as_array())
        .filter(|a| a.iter().any(|e| e.is_object()))
        .map(|a| {
            a.iter()
                .filter_map(|e| serde_json::from_value(e.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    if let Some(obj) = v.as_object_mut() {
        obj.insert("parts".to_string(), serde_json::Value::Array(vec![]));
    }
    let mut doc: Document =
        serde_json::from_value(v).map_err(|e| CliError::new(format!("解析文档模型失败: {e}")))?;
    let mut parts = Vec::new();
    for p in &paths {
        let part: Part = serde_json::from_str(
            &std::fs::read_to_string(root.join(p))
                .map_err(|e| CliError::new(format!("读取 {p} 失败: {e}")))?,
        )
        .map_err(|e| CliError::new(format!("解析 {p} 失败: {e}")))?;
        parts.push(part);
    }
    if !parts.is_empty() {
        doc.parts = parts;
    } else if !inline_parts.is_empty() {
        doc.parts = inline_parts;
    }
    Ok(doc)
}

/// 将内存 Document 的分片写回产物目录
pub fn save_document(root: &Path, doc: &Document) -> Result<()> {
    let paths = parts_paths(root)?;
    if paths.is_empty() {
        // 单文件 document.json：内联回写 parts
        let doc_path = root.join("document.json");
        let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&doc_path)?)?;
        let parts = serde_json::to_value(&doc.parts)?;
        if let Some(obj) = v.as_object_mut() {
            obj.insert("parts".to_string(), parts);
        }
        std::fs::write(&doc_path, serde_json::to_string_pretty(&v)?)?;
        return Ok(());
    }
    for (i, part) in doc.parts.iter().enumerate() {
        let rel = paths
            .get(i)
            .cloned()
            .unwrap_or_else(|| format!("word/parts/part{}.json", i + 1));
        let full = root.join(&rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&full, serde_json::to_string_pretty(part)?)?;
    }
    Ok(())
}

/// 将输入解析为产物根目录
///
/// - 输入是产物目录 → 直接使用，不清理；
/// - 否则视为 .docx → 解包到一次性临时目录，命令结束自动删除。
pub fn resolve_input(input: &Path) -> Result<(PathBuf, Option<TempGuard>)> {
    if is_product_dir(input) {
        return Ok((input.to_path_buf(), None));
    }
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "doc".to_string());
    let tmp = std::env::temp_dir().join(format!("json2docx-{}-{stem}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    json2docx::unpack(&path_str(input), &path_str(&tmp))
        .map_err(|e| CliError::new(format!("解包 {} 失败: {e}", input.display())))?;
    Ok((tmp.clone(), Some(TempGuard { path: tmp })))
}
