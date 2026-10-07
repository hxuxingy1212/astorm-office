//! OPC（OOXML 包）通用部件读写。
//!
//! 按字面包路径读写 zip 条目，与具体文档格式（docx/xlsx/pptx）无关；
//! 各 CLI 的 `raw` / `raw-set` 命令基于此实现。条目名匹配不区分大小写
//! （与主流 OOXML 实现的宽容行为一致）。

use std::io::{Read, Write};
use std::path::Path;

type Result<T> = std::io::Result<T>;

fn open_archive(pkg: &Path) -> Result<zip::ZipArchive<std::fs::File>> {
    let file = std::fs::File::open(pkg)?;
    Ok(zip::ZipArchive::new(file)?)
}

/// 读取部件原始字节；部件不存在返回 `None`。
pub fn read_part(pkg: &Path, part: &str) -> Result<Option<Vec<u8>>> {
    let path = part.trim_start_matches('/').replace('\\', "/");
    let mut zip = open_archive(pkg)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.name().replace('\\', "/").eq_ignore_ascii_case(&path) {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            return Ok(Some(buf));
        }
    }
    Ok(None)
}

/// 列出包内全部部件路径（保持包内原始顺序）。
pub fn list_parts(pkg: &Path) -> Result<Vec<String>> {
    let mut zip = open_archive(pkg)?;
    let mut names = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        names.push(zip.by_index(i)?.name().replace('\\', "/"));
    }
    Ok(names)
}

/// 用新内容替换（或新增）部件，重写整个包到 `out`，不改动输入文件。
pub fn write_part(pkg: &Path, out: &Path, part: &str, content: &[u8]) -> Result<()> {
    let path = part.trim_start_matches('/').replace('\\', "/");
    let mut zip = open_archive(pkg)?;
    let mut entries: Vec<(String, Vec<u8>)> = Vec::with_capacity(zip.len() + 1);
    let mut replaced = false;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
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

    let file = std::fs::File::create(out)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in &entries {
        writer.start_file(name.as_str(), options)?;
        writer.write_all(data)?;
    }
    writer.finish()?;
    Ok(())
}
