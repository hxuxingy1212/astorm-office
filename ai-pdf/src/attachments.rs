//! PDF 内嵌文件（附件）：`/EmbeddedFiles` 名字树 + 页面 `FileAttachment` 注释。
//!
//! - [`list`]：只读清单（文件名/MIME/大小/说明），不解码载荷；
//! - [`extract`]：解码（`/Filter` 由 lopdf 处理，含 Flate/Brotli 等）并落盘，
//!   文件名做路径清洗、目标目录里不覆盖已有文件（走 `office_core::PayloadSink`）。
//!
//! 载荷来源（PDF 里附件挂两处，都读）：
//! 1. 目录 `/Names → /EmbeddedFiles` 名字树（`/Names` 数组或 `/Kids` 嵌套，Adobe/Word/LaTeX 走这条）；
//! 2. 页面注释 `/Annots → /Subtype /FileAttachment → /FS`（个别工具走这条）。
//!
//! 名字树里的条目是 `/Filespec` 字典：文件名在 `/UF`（Unicode）或 `/F`，
//! 载荷在 `/EF → /F`（或 `/UF`）指向的 `/EmbeddedFile` 流。

use crate::model::PdfAttachment;
use anyhow::Result;
use lopdf::{Dictionary, Document, Object, ObjectId};
use std::path::Path;

/// 名字树 / 注释里的一条附件（含载荷流的对象号，供 extract 用）
struct Entry {
    info: PdfAttachment,
    stream: Option<ObjectId>,
}

/// 附件清单（不落盘、不解码载荷）
pub fn list(doc: &Document) -> Vec<PdfAttachment> {
    walk(doc).into_iter().map(|e| e.info).collect()
}

/// 导出附件到目录，返回公共资源清单，并保留 `bytes` / `mime` 兼容字段。
///
/// 目标目录已有同名文件时不覆盖（自动 `名字-2.ext`）；同一附件（同对象号）只导出一次。
pub fn extract(doc: &Document, out_dir: &Path) -> Result<Vec<serde_json::Value>> {
    let sink = office_core::PayloadSink::new(out_dir);
    extract_with_sink(doc, &sink)
}

/// Export through a caller-owned sink to retain structured errors and strict reports.
pub fn extract_with_sink(
    doc: &Document,
    sink: &office_core::PayloadSink,
) -> Result<Vec<serde_json::Value>> {
    use office_core::ExportErrorCode;
    let mut descriptions = std::collections::HashMap::new();
    for (index, e) in walk(doc).into_iter().enumerate() {
        let label = e
            .stream
            .map(|id| format!("pdf-obj-{}-{}", id.0, id.1))
            .unwrap_or_else(|| format!("pdf-attachment-{}", index + 1));
        let _source = sink.at_source(format!("attachment[{}]/{label}", index + 1));
        descriptions
            .entry(label.clone())
            .or_insert(e.info.description.clone());
        match e.stream {
            None => {
                sink.report_failure_typed(
                    &e.info.name,
                    &label,
                    ExportErrorCode::PayloadMissing,
                    "附件未引用载荷流",
                    e.info.mime.as_deref(),
                );
            }
            Some(id) => match doc.get_object(id).and_then(Object::as_stream) {
                Err(error) => {
                    sink.report_failure_typed(
                        &e.info.name,
                        &label,
                        ExportErrorCode::PayloadMissing,
                        error.to_string(),
                        e.info.mime.as_deref(),
                    );
                }
                Ok(stream) => {
                    let bytes = if stream.dict.get(b"Filter").is_err() {
                        Ok(stream.content.clone())
                    } else {
                        stream.decompressed_content()
                    };
                    match bytes {
                        Ok(bytes) => {
                            let _ = sink.try_link_for_bytes_typed(
                                &e.info.name,
                                &bytes,
                                &label,
                                e.info.mime.as_deref(),
                            );
                        }
                        Err(error) => {
                            sink.report_failure_typed(
                                &e.info.name,
                                &label,
                                ExportErrorCode::InvalidPayload,
                                error.to_string(),
                                e.info.mime.as_deref(),
                            );
                        }
                    }
                }
            },
        }
    }
    Ok(sink
        .entries()
        .into_iter()
        .map(|entry| {
            let mut value = serde_json::to_value(&entry).expect("resource manifest serializes");
            value["mime"] = serde_json::json!(entry.resource_type);
            value["bytes"] = if entry.kind == "file" {
                serde_json::json!(entry.size)
            } else {
                serde_json::Value::Null
            };
            value["description"] =
                serde_json::json!(descriptions.get(&entry.src).cloned().flatten());
            value
        })
        .collect())
}

/// 遍历两处附件来源
fn walk(doc: &Document) -> Vec<Entry> {
    let mut out = Vec::new();
    if let Ok(catalog) = doc.catalog() {
        // /Names 与 /EmbeddedFiles 在真实文件里常是间接引用，统一 resolve
        if let Some(names) = catalog
            .get(b"Names")
            .ok()
            .and_then(|o| resolve_dict(doc, o))
        {
            if let Some(tree) = names
                .get(b"EmbeddedFiles")
                .ok()
                .and_then(|o| resolve_dict(doc, o))
            {
                walk_name_tree(doc, tree, &mut out);
            }
        }
    }
    for (_, page_id) in doc.get_pages() {
        let Ok(annots) = doc.get_page_annotations(page_id) else {
            continue;
        };
        for a in annots {
            let is_file = a
                .get(b"Subtype")
                .ok()
                .and_then(|o| o.as_name().ok())
                .map(|n| n == b"FileAttachment")
                .unwrap_or(false);
            if !is_file {
                continue;
            }
            // 附件注释：/FS 指向 filespec；个别文件把内容直接放在注释的 /Contents
            if let Some(spec) = a.get(b"FS").ok().and_then(|o| resolve_dict(doc, o)) {
                if let Some(entry) = entry_from_filespec(doc, spec) {
                    out.push(entry);
                    continue;
                }
            }
            if let Ok(text) = a.get(b"Contents").and_then(Object::as_str) {
                let name = a
                    .get(b"T")
                    .ok()
                    .and_then(|o| o.as_str().ok())
                    .map(|s| sanitize_name(&String::from_utf8_lossy(s)))
                    .unwrap_or_else(|| "attachment.txt".to_string());
                out.push(Entry {
                    info: PdfAttachment {
                        name,
                        src: None,
                        mime: None,
                        size: Some(text.len() as u64),
                        description: None,
                    },
                    stream: None,
                });
            }
        }
    }
    out
}

/// 名字树：`/Names [(名) ref …]` 与 `/Kids [dict …]` 两种形态，递归下钻
fn walk_name_tree(doc: &Document, tree: &Dictionary, out: &mut Vec<Entry>) {
    if let Ok(kids) = tree.get(b"Kids").and_then(Object::as_array) {
        for kid in kids {
            if let Some(d) = resolve_dict(doc, kid) {
                walk_name_tree(doc, d, out);
            }
        }
    }
    let Ok(names) = tree.get(b"Names").and_then(Object::as_array) else {
        return;
    };
    // [ (名) filespec-ref (名2) filespec-ref2 … ]
    for pair in names.chunks(2) {
        let Some(spec_obj) = pair.get(1) else {
            continue;
        };
        let Some(spec) = resolve_dict(doc, spec_obj) else {
            continue;
        };
        if let Some(mut entry) = entry_from_filespec(doc, spec) {
            // 名字树里的键名优先于 filespec 里的文件名（键名通常就是用户看到的文件名）
            if let Some(key) = pair.first().and_then(|o| o.as_str().ok()) {
                let k = sanitize_name(&decode_pdf_name(key));
                if !k.is_empty() {
                    entry.info.name = k;
                }
            }
            out.push(entry);
        }
    }
}

/// `/Filespec` → 附件条目（文件名 /EF 载荷流 / 类型 / 大小 / 说明）
fn entry_from_filespec(doc: &Document, spec: &Dictionary) -> Option<Entry> {
    let str_of = |key: &[u8]| -> Option<String> {
        spec.get(key)
            .ok()
            .and_then(|o| o.as_str().ok())
            .map(decode_pdf_name)
    };
    let name = str_of(b"UF")
        .or_else(|| str_of(b"F"))
        .map(|s| sanitize_name(&s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "attachment".to_string());
    let ef = spec.get(b"EF").ok().and_then(|o| resolve_dict(doc, o));
    let stream_obj = ef.and_then(|d| {
        d.get(b"UF")
            .or_else(|_| d.get(b"F"))
            .ok()
            .and_then(|o| o.as_reference().ok())
    });
    let mime = stream_obj
        .and_then(|id| doc.get_object(id).ok())
        .and_then(|o| o.as_stream().ok())
        .and_then(|s| s.dict.get(b"Subtype").ok())
        .and_then(|o| o.as_name().ok())
        .map(|n| {
            String::from_utf8_lossy(n)
                .replace("#2F", "/")
                .replace("#20", " ")
        });
    let size = stream_obj
        .and_then(|id| doc.get_object(id).ok())
        .and_then(|o| o.as_stream().ok())
        .and_then(|s| {
            s.dict
                .get(b"Params")
                .ok()
                .and_then(|o| resolve_dict(doc, o))
        })
        .and_then(|p| p.get(b"Size").ok().and_then(|o| o.as_i64().ok()))
        .map(|v| v as u64);
    Some(Entry {
        info: PdfAttachment {
            name,
            src: None,
            mime,
            size,
            description: str_of(b"Desc"),
        },
        stream: stream_obj,
    })
}

/// 把可能是引用/直接字典的对象解析成字典
fn resolve_dict<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a Dictionary> {
    match o {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => doc.get_object(*id).ok().and_then(|x| x.as_dict().ok()),
        _ => None,
    }
}

/// PDF 字符串按 UTF-16BE（BOM）或 PDFDocEncoding/Latin-1 解出可读文件名
fn decode_pdf_name(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| ((c[0] as u16) << 8) | (*c.get(1).unwrap_or(&0) as u16))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    // 先按 UTF-8 试，失败再退回 Latin-1（PDFDocEncoding 的 ASCII 区一致）
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|b| *b as char).collect(),
    }
}

/// 文件名清洗：去掉路径分隔符与控制字符（附件名可能来自外部文档，不能直接当落盘名）
pub(crate) fn sanitize_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_start_matches('.').to_string();
    if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Stream};

    /// 造一份含内嵌文件的 PDF（名字树 + Flate 压缩 + 中文名）
    fn pdf_with_attachment(payload: &[u8]) -> Document {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let content_id = doc.add_object(Stream::new(Dictionary::new(), b"BT ET".to_vec()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 200.into(), 200.into()],
            "Contents" => content_id,
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let mut ef = Stream::new(
            dictionary! { "Type" => "EmbeddedFile", "Subtype" => Object::Name(b"text/plain".to_vec()) },
            payload.to_vec(),
        );
        ef.compress().unwrap();
        let ef_id = doc.add_object(ef);
        let spec_id = doc.add_object(dictionary! {
            "Type" => "Filespec",
            "F" => Object::string_literal("说明.txt"),
            "UF" => Object::string_literal("说明.txt"),
            "Desc" => Object::string_literal("测试附件"),
            "EF" => dictionary! { "F" => ef_id },
        });
        let names_id = doc.add_object(dictionary! {
            "Names" => vec![Object::string_literal("说明.txt"), spec_id.into()],
        });
        let cat_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "Names" => dictionary! { "EmbeddedFiles" => names_id },
        });
        doc.trailer.set("Root", cat_id);
        doc
    }

    #[test]
    fn lists_and_extracts_embedded_files() {
        // 载荷要够大才真的被 Flate 压缩（lopdf 的 compress() 只在更小时才加 /Filter）
        let payload = "hello-attachment-中文-".repeat(200);
        let payload = payload.as_bytes();
        let doc = pdf_with_attachment(payload);

        // 清单：名字/说明/MIME 都读出来（不解码载荷）
        let list = list(&doc);
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(list[0].name, "说明.txt");
        assert_eq!(list[0].description.as_deref(), Some("测试附件"));
        assert_eq!(list[0].mime.as_deref(), Some("text/plain"));

        // 导出：Flate 解出来的字节与原始载荷一致
        let dir = std::env::temp_dir().join(format!("j2p-att-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let out = extract(&doc, &dir).unwrap();
        assert_eq!(out[0]["kind"], "file", "{out:?}");
        assert_eq!(out[0]["bytes"], payload.len() as u64);
        assert!(
            list[0].size.is_none() || list[0].size == Some(payload.len() as u64),
            "清单里的 size 若给出应与实际一致：{:?}",
            list[0].size
        );
        let written = dir.join("说明.txt");
        assert_eq!(std::fs::read(&written).unwrap(), payload);
        // 二次导出：不覆盖，落成 说明-2.txt
        let out2 = extract(&doc, &dir).unwrap();
        assert!(
            out2[0]["path"].as_str().unwrap().ends_with("说明-2.txt"),
            "{out2:?}"
        );
        assert_eq!(
            std::fs::read(&written).unwrap(),
            payload,
            "原文件不该被覆盖"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn strict_pdf_export_reports_directory_and_decode_failures() {
        let mut doc = pdf_with_attachment(b"abc");
        let dir = std::env::temp_dir().join(format!("j2p-strict-att-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let blocked = dir.join("blocked");
        std::fs::write(&blocked, b"existing").unwrap();
        let sink = office_core::PayloadSink::new(blocked.join("out")).with_strict(true);
        let entries = extract_with_sink(&doc, &sink).unwrap();
        assert_eq!(entries[0]["error"]["code"], "create_dir_failed");
        assert!(sink.check().is_err());
        let stream_id = walk(&doc)[0].stream.unwrap();
        doc.get_object_mut(stream_id)
            .unwrap()
            .as_stream_mut()
            .unwrap()
            .dict
            .set("Filter", Object::Name(b"UnsupportedFilter".to_vec()));
        let sink = office_core::PayloadSink::new(dir.join("out")).with_strict(true);
        let entries = extract_with_sink(&doc, &sink).unwrap();
        assert_eq!(entries[0]["error"]["code"], "invalid_payload");
        assert_eq!(
            entries[0]["source_locations"][0],
            format!("/attachment[1]/pdf-obj-{}-{}", stream_id.0, stream_id.1)
        );
        assert!(sink.check().is_err());
        assert!(!sink.dir().exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// 回环保真：unpack 落盘 → repack 重建 /EmbeddedFiles → 再读，附件名/说明/MIME/字节都不丢
    #[test]
    fn unpack_repack_roundtrip_keeps_attachments() {
        let payload = "roundtrip-payload-中文-".repeat(120);
        let payload = payload.as_bytes();
        let mut doc = pdf_with_attachment(payload);
        let base = std::env::temp_dir().join(format!("j2p-att-rt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let src_pdf = base.join("in.pdf");
        doc.save(&src_pdf).unwrap();

        // ① unpack：载荷落到 attachments/ 并记进 document.json
        let prod = base.join("prod");
        crate::unpack::unpack(&src_pdf, &prod).unwrap();
        let model: crate::model::DocumentModel =
            serde_json::from_str(&std::fs::read_to_string(prod.join("document.json")).unwrap())
                .unwrap();
        assert_eq!(model.attachments.len(), 1, "{:?}", model.attachments);
        assert_eq!(
            model.attachments[0].src.as_deref(),
            Some("attachments/说明.txt")
        );
        assert_eq!(
            std::fs::read(prod.join("attachments/说明.txt")).unwrap(),
            payload
        );

        // ② repack：重建 /EmbeddedFiles
        let out_pdf = base.join("out.pdf");
        let r = crate::repack::repack_model(&model, &prod, &out_pdf).unwrap();
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);

        // ③ 再读：清单与载荷一致
        let again = lopdf::Document::load(&out_pdf).unwrap();
        let list = list(&again);
        assert_eq!(list.len(), 1, "{list:?}");
        assert_eq!(list[0].name, "说明.txt");
        assert_eq!(list[0].description.as_deref(), Some("测试附件"));
        assert_eq!(list[0].mime.as_deref(), Some("text/plain"));
        let dir = base.join("out");
        let rec = extract(&again, &dir).unwrap();
        assert_eq!(rec[0]["kind"], "file", "{rec:?}");
        assert_eq!(
            std::fs::read(dir.join("说明.txt")).unwrap(),
            payload,
            "回环后载荷字节应一致"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn sanitizes_hostile_names() {
        assert_eq!(sanitize_name("../../etc/passwd"), "_.._etc_passwd");
        assert_eq!(sanitize_name("a/b\\c:d"), "a_b_c_d");
        assert_eq!(sanitize_name("   "), "attachment");
        assert_eq!(sanitize_name(".hidden"), "hidden");
    }

    #[test]
    fn decodes_utf16_names() {
        // FE FF 前缀 → UTF-16BE（U+8BF4 U+660E = "说明"）
        let bytes = [0xFE, 0xFF, 0x8B, 0xF4, 0x66, 0x0E];
        assert_eq!(decode_pdf_name(&bytes), "说明");
        // 无 BOM：UTF-8 优先，失败退回 Latin-1
        assert_eq!(decode_pdf_name(b"plain.txt"), "plain.txt");
        assert_eq!(decode_pdf_name(&[0xD3, 0xEB]), "Óë");
    }
}
