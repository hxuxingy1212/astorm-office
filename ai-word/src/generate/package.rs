//! OPC 打包：内容类型、关系、ZIP 写入

use crate::error::Result;
use crate::generate::GenCtx;
use crate::utils::constants::*;
use crate::utils::xml::{declaration, el, esc_attr};
use std::io::Write;

/// [Content_Types].xml
pub(crate) fn content_types(ctx: &GenCtx) -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!("<Types xmlns=\"{NS_CT}\">"));
    out.push_str("<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>");
    out.push_str("<Default Extension=\"xml\" ContentType=\"application/xml\"/>");
    // 媒体扩展名
    let mut exts: Vec<String> = ctx
        .media
        .iter()
        .filter_map(|m| m.name.rsplit('.').next().map(|s| s.to_lowercase()))
        .collect();
    exts.sort();
    exts.dedup();
    for ext in exts {
        let ct = match ext.as_str() {
            "png" => "image/png",
            "jpeg" | "jpg" => "image/jpeg",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "webp" => "image/webp",
            "emf" => "image/x-emf",
            "wmf" => "image/x-wmf",
            "tif" | "tiff" => "image/tiff",
            "svg" => "image/svg+xml",
            _ => "application/octet-stream",
        };
        out.push_str(&format!(
            "<Default Extension=\"{}\" ContentType=\"{}\"/>",
            ext, ct
        ));
    }
    // 附件（OLE 嵌入文件）扩展名
    let mut eexts: Vec<String> = ctx
        .embeddings
        .iter()
        .filter_map(|(n, _)| n.rsplit('.').next().map(|s| s.to_lowercase()))
        .collect();
    eexts.sort();
    eexts.dedup();
    for ext in eexts {
        let ct = match ext.as_str() {
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "xls" => "application/vnd.ms-excel",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "doc" => "application/msword",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "ppt" => "application/vnd.ms-powerpoint",
            "pdf" => "application/pdf",
            _ => "application/vnd.openxmlformats-officedocument.oleObject",
        };
        out.push_str(&format!(
            "<Default Extension=\"{}\" ContentType=\"{}\"/>",
            ext, ct
        ));
    }
    let overrides = [
        ("/word/document.xml", CT_DOCUMENT),
        ("/word/styles.xml", CT_STYLES),
        ("/word/settings.xml", CT_SETTINGS),
        ("/word/numbering.xml", CT_NUMBERING),
        ("/word/fontTable.xml", CT_FONT_TABLE),
        ("/word/theme/theme1.xml", CT_THEME),
        ("/word/webSettings.xml", CT_WEB_SETTINGS),
        ("/docProps/core.xml", CT_CORE),
        ("/docProps/app.xml", CT_APP),
    ];
    for (name, ct) in overrides {
        out.push_str(&format!(
            "<Override PartName=\"{name}\" ContentType=\"{ct}\"/>"
        ));
    }
    for i in 0..ctx.header_parts.len() {
        out.push_str(&format!(
            "<Override PartName=\"/word/header{}.xml\" ContentType=\"{CT_HEADER}\"/>",
            i + 1
        ));
    }
    for i in 0..ctx.footer_parts.len() {
        out.push_str(&format!(
            "<Override PartName=\"/word/footer{}.xml\" ContentType=\"{CT_FOOTER}\"/>",
            i + 1
        ));
    }
    if ctx.any_footnotes {
        out.push_str(&format!(
            "<Override PartName=\"/word/footnotes.xml\" ContentType=\"{CT_FOOTNOTES}\"/>"
        ));
    }
    if ctx.any_endnotes {
        out.push_str(&format!(
            "<Override PartName=\"/word/endnotes.xml\" ContentType=\"{CT_ENDNOTES}\"/>"
        ));
    }
    if ctx.any_comments {
        out.push_str(&format!(
            "<Override PartName=\"/word/comments.xml\" ContentType=\"{CT_COMMENTS}\"/>"
        ));
    }
    for i in 0..ctx.chart_parts.len() {
        out.push_str(&format!(
            "<Override PartName=\"/word/charts/chart{}.xml\" ContentType=\"{CT_CHART}\"/>",
            i + 1
        ));
    }
    // 透传部件的内容类型
    for p in &ctx.doc.passthrough {
        if let Some(ct) = &p.content_type {
            out.push_str(&format!(
                "<Override PartName=\"/{}\" ContentType=\"{}\"/>",
                esc_attr(&p.path),
                esc_attr(ct)
            ));
        }
    }
    out.push_str("</Types>");
    out
}

/// _rels/.rels
pub(crate) fn root_rels() -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!("<Relationships xmlns=\"{NS_PR}\">"));
    out.push_str(&format!(
        "<Relationship Id=\"rId1\" Type=\"{REL_OFFICE_DOCUMENT}\" Target=\"word/document.xml\"/>"
    ));
    out.push_str(&format!(
        "<Relationship Id=\"rId2\" Type=\"{REL_CORE_PROPERTIES}\" Target=\"docProps/core.xml\"/>"
    ));
    out.push_str(&format!(
        "<Relationship Id=\"rId3\" Type=\"{REL_EXTENDED_PROPERTIES}\" Target=\"docProps/app.xml\"/>"
    ));
    out.push_str("</Relationships>");
    out
}

/// word/_rels/document.xml.rels
pub(crate) fn document_rels(ctx: &GenCtx) -> String {
    let mut out = String::new();
    out.push_str(declaration());
    out.push_str(&format!("<Relationships xmlns=\"{NS_PR}\">"));
    for rel in &ctx.rels {
        let mode = if rel.external {
            " TargetMode=\"External\""
        } else {
            ""
        };
        out.push_str(&format!(
            "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{}/>",
            rel.id,
            rel.rel_type,
            esc_attr(&rel.target),
            mode
        ));
    }
    out.push_str("</Relationships>");
    out
}

/// 写入 ZIP 包
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_zip(
    output: &str,
    parts: &[(&str, String)],
    header_parts: &[(String, String)],
    footer_parts: &[(String, String)],
    footnotes_xml: Option<String>,
    endnotes_xml: Option<String>,
    comments_xml: Option<String>,
    chart_parts: &[(String, String)],
    ctx: &GenCtx,
    content_types: String,
    root_rels: String,
    doc_rels: String,
) -> Result<()> {
    let file = std::fs::File::create(output)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // [Content_Types].xml 必须最先
    zip.start_file("[Content_Types].xml", opts)?;
    zip.write_all(content_types.as_bytes())?;

    zip.start_file("_rels/.rels", opts)?;
    zip.write_all(root_rels.as_bytes())?;

    for (name, content) in parts {
        zip.start_file(name, opts)?;
        zip.write_all(content.as_bytes())?;
    }

    zip.start_file("word/_rels/document.xml.rels", opts)?;
    zip.write_all(doc_rels.as_bytes())?;

    for (name, content) in header_parts {
        zip.start_file(format!("word/{name}"), opts)?;
        zip.write_all(content.as_bytes())?;
    }
    for (name, content) in footer_parts {
        zip.start_file(format!("word/{name}"), opts)?;
        zip.write_all(content.as_bytes())?;
    }
    if let Some(fn_xml) = footnotes_xml {
        zip.start_file("word/footnotes.xml", opts)?;
        zip.write_all(fn_xml.as_bytes())?;
    }
    if let Some(en_xml) = endnotes_xml {
        zip.start_file("word/endnotes.xml", opts)?;
        zip.write_all(en_xml.as_bytes())?;
    }
    if let Some(cm_xml) = comments_xml {
        zip.start_file("word/comments.xml", opts)?;
        zip.write_all(cm_xml.as_bytes())?;
    }
    for (name, content) in chart_parts {
        zip.start_file(format!("word/charts/{name}"), opts)?;
        zip.write_all(content.as_bytes())?;
    }

    for m in &ctx.media {
        zip.start_file(format!("word/media/{}", m.name), opts)?;
        zip.write_all(&m.bytes)?;
    }
    for (name, bytes) in &ctx.embeddings {
        let path = format!("word/embeddings/{name}");
        // 已由透传保留的同名附件不重复写入
        if ctx.doc.passthrough.iter().any(|p| p.path == path) {
            continue;
        }
        zip.start_file(path, opts)?;
        zip.write_all(bytes)?;
    }

    // 透传部件（原样字节）
    for p in &ctx.doc.passthrough {
        if let Some(bytes) = crate::utils::b64::decode(&p.data) {
            zip.start_file(&p.path, opts)?;
            zip.write_all(&bytes)?;
        }
    }

    zip.finish()?;
    Ok(())
}

// 保留 el 引用避免未使用告警（供未来扩展）
#[allow(dead_code)]
fn _keep_el() -> String {
    el("x", &[], "")
}
