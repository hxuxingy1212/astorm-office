//! 解析模块：DOCX → JSON 模型
//!
//! 管线：.docx → ZIP 解压 → XML 流式解析（quick-xml）→ Rust 模型 → JSON

pub mod document;
pub mod math;
pub mod xmltree;

use crate::error::Result;
use crate::model::Document;
use document::Package;
use std::io::Read;

/// 解包结果
pub struct UnpackResult {
    pub path: String,
    pub parts: usize,
}

/// 解析 DOCX 为 Document 模型
pub fn parse(input: &str) -> Result<Document> {
    // 旧版 .doc（OLE2/CFB）自动识别：先转 .docx 再解析
    if crate::legacy::is_cfb(std::path::Path::new(input)) {
        if office_core::opc::looks_encrypted_cfb(std::path::Path::new(input)) {
            return Err(crate::error::Error::Encrypted(
                "文档已加密（OOXML 密码保护），暂不支持".into(),
            ));
        }
        let docx_bytes = crate::legacy::doc_to_docx_bytes(std::path::Path::new(input))?;
        let tmp = std::env::temp_dir().join(format!(
            "json2docx-legacy-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(&tmp, &docx_bytes)?;
        let doc = parse(&tmp.to_string_lossy())?;
        let _ = std::fs::remove_file(&tmp);
        return Ok(doc);
    }
    let files = read_zip(input)?;
    document::parse_document(&files)
}

/// 将 DOCX 解包为可编辑产物目录
pub fn unpack(input: &str, out_dir: &str) -> Result<UnpackResult> {
    // 旧版 .doc（OLE2/CFB）：自动转换成 .docx 后再解包（媒体一并来自转换产物）
    if crate::legacy::is_cfb(std::path::Path::new(input)) {
        if office_core::opc::looks_encrypted_cfb(std::path::Path::new(input)) {
            return Err(crate::error::Error::Encrypted(
                "文档已加密（OOXML 密码保护），暂不支持".into(),
            ));
        }
        let tmp = std::env::temp_dir().join(format!(
            "json2docx-legacy-unpack-{}-{}.docx",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let bytes = crate::legacy::doc_to_docx_bytes(std::path::Path::new(input))?;
        std::fs::write(&tmp, &bytes)?;
        let r = unpack(&tmp.to_string_lossy(), out_dir);
        let _ = std::fs::remove_file(&tmp);
        return r;
    }
    let files = read_zip(input)?;
    let doc = document::parse_document(&files)?;

    let root = std::path::Path::new(out_dir);
    std::fs::create_dir_all(root.join("word").join("parts"))?;
    std::fs::create_dir_all(root.join("word").join("media"))?;

    // 写入分片
    let mut part_paths = Vec::new();
    for (i, part) in doc.parts.iter().enumerate() {
        let rel = format!("word/parts/part{}.json", i + 1);
        let json = serde_json::to_string_pretty(part)?;
        std::fs::write(root.join(&rel), json)?;
        part_paths.push(rel);
    }

    // 写入 document.json（parts 替换为路径数组）
    let mut value = serde_json::to_value(&doc)?;
    if let Some(obj) = value.as_object_mut() {
        obj.insert(
            "parts".to_string(),
            serde_json::Value::Array(
                part_paths
                    .iter()
                    .map(|p| serde_json::Value::String(p.clone()))
                    .collect(),
            ),
        );
    }
    std::fs::write(
        root.join("document.json"),
        serde_json::to_string_pretty(&value)?,
    )?;

    // 提取媒体与嵌入附件（供 repack 相对路径解析）
    for (name, bytes) in &files {
        for dir in ["word/media/", "word/embeddings/"] {
            if let Some(rest) = name.strip_prefix(dir) {
                if !rest.is_empty() {
                    let sub = dir.trim_start_matches("word/").trim_end_matches('/');
                    let p = root.join("word").join(sub).join(rest);
                    if let Some(parent) = p.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(p, bytes)?;
                }
            }
        }
    }

    Ok(UnpackResult {
        path: out_dir.to_string(),
        parts: doc.parts.len(),
    })
}

/// 读取 ZIP 全部条目
pub(crate) fn read_zip(input: &str) -> Result<Package> {
    let file = std::fs::File::open(input)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut map: Package = std::collections::HashMap::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        map.insert(name, buf);
    }
    Ok(map)
}
