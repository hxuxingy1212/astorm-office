//! 路径与字符串转换辅助。

use std::path::{Path, PathBuf};

/// 路径转字符串：非 UTF-8 路径做 lossy 转换而不是 panic。
pub fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// 以输入文件的主名 + 指定扩展名派生输出路径（如 `a/b.pptx` + `html` → `a/b.html`）。
pub fn with_stem_ext(input: &Path, ext: &str) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "out".to_string());
    input.with_file_name(format!("{stem}.{ext}"))
}

/// 以输入路径去掉扩展名派生输出目录（unpack 默认值，如 `a/b.docx` → `a/b`）。
pub fn stem_dir(input: &Path) -> PathBuf {
    input.with_extension("")
}

/// 判断两个路径是否指向同一文件（尽量 canonicalize，失败时退回字面比较）。
pub fn same_file(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => a == b,
    }
}
