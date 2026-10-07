//! 原始 OOXML 部件访问（整部件读/写，不含 xpath）。

use std::io::{Read, Write};
use std::path::Path;

use crate::error::{Error, Result};

/// 语义别名 → 包内路径。
fn alias(part: &str) -> Option<&'static str> {
    match part.trim_start_matches('/') {
        "workbook" => Some("xl/workbook.xml"),
        "styles" => Some("xl/styles.xml"),
        "sharedStrings" | "sharedstrings" => Some("xl/sharedStrings.xml"),
        "theme" => Some("xl/theme/theme1.xml"),
        "[Content_Types].xml" => Some("[Content_Types].xml"),
        _ => None,
    }
}

/// 把部件引用解析为包内路径（支持别名、工作表名、字面路径）。
pub fn resolve_part(xlsx: &Path, part: &str) -> Result<String> {
    let p = part.trim_start_matches('/').replace('\\', "/");
    if let Some(a) = alias(&p) {
        return Ok(a.to_string());
    }
    if p.contains('/') || p.ends_with(".xml") || p.ends_with(".rels") || p.ends_with(".bin") {
        return Ok(p);
    }
    // 按工作表名解析
    let file = std::fs::File::open(xlsx)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let wb = read_zip(&mut zip, "xl/workbook.xml")?.unwrap_or_default();
    let rid = find_sheet_rid(&wb, &p)
        .ok_or_else(|| Error::InvalidInput(format!("找不到工作表或部件: {part}")))?;
    let rels = read_zip(&mut zip, "xl/_rels/workbook.xml.rels")?.unwrap_or_default();
    let target = find_rel_target(&rels, &rid)
        .ok_or_else(|| Error::InvalidInput(format!("工作表 {p} 无可解析关系")))?;
    Ok(format!("xl/{}", target.trim_start_matches('/')))
}

fn read_zip(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Option<String>> {
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        if e.name().replace('\\', "/").eq_ignore_ascii_case(name) {
            let mut s = String::new();
            e.read_to_string(&mut s)?;
            return Ok(Some(s));
        }
    }
    Ok(None)
}

fn find_sheet_rid(wb_xml: &str, name: &str) -> Option<String> {
    // 找 <sheet name="X" ... r:id="rIdN">
    for tag in wb_xml.split('<') {
        if !tag.starts_with("sheet ") || !tag.contains("name=") {
            continue;
        }
        let nm = attr(tag, "name")?;
        if nm == name {
            return attr(tag, "id");
        }
    }
    None
}

fn find_rel_target(rels: &str, rid: &str) -> Option<String> {
    for tag in rels.split('<') {
        if tag.starts_with("Relationship") && attr(tag, "Id").as_deref() == Some(rid) {
            return attr(tag, "Target");
        }
    }
    None
}

fn attr(tag: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let i = tag.find(&needle)? + needle.len();
    let j = tag[i..].find('"')? + i;
    Some(tag[i..j].to_string())
}

/// 读取部件原始字节（二进制安全）；部件不存在返回 `None`。
pub fn read_part(xlsx: &Path, part: &str) -> Result<Option<Vec<u8>>> {
    let path = resolve_part(xlsx, part)?;
    let file = std::fs::File::open(xlsx)?;
    let mut zip = zip::ZipArchive::new(file)?;
    read_zip_bytes(&mut zip, &path)
}

fn read_zip_bytes(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Result<Option<Vec<u8>>> {
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        if e.name().replace('\\', "/").eq_ignore_ascii_case(name) {
            let mut buf = Vec::new();
            e.read_to_end(&mut buf)?;
            return Ok(Some(buf));
        }
    }
    Ok(None)
}

/// 用新内容替换/新增部件（重写整个 zip 到 `out`，不改动输入 xlsx）。
pub fn write_part(xlsx: &Path, out: &Path, part: &str, content: &[u8]) -> Result<()> {
    let path = resolve_part(xlsx, part)?;
    let file = std::fs::File::open(xlsx)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut entries: Vec<(String, Vec<u8>)> = Vec::with_capacity(zip.len() + 1);
    let mut replaced = false;
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        let name = e.name().to_string();
        let mut buf = Vec::new();
        e.read_to_end(&mut buf)?;
        if name.replace('\\', "/").eq_ignore_ascii_case(&path) {
            buf = content.to_vec();
            replaced = true;
        }
        entries.push((name, buf));
    }
    if !replaced {
        entries.push((path, content.to_vec()));
    }
    drop(zip);

    let out_file = std::fs::File::create(out)?;
    let mut w = zip::ZipWriter::new(out_file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in &entries {
        w.start_file(name.as_str(), opts)?;
        w.write_all(data)?;
    }
    w.finish()?;
    Ok(())
}
