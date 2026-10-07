//! unpack 产物目录操作（对齐 ai-ppt/cli 的 product.rs）
//!
//! view / edit / render / batch 的输入可以是 PDF 文件（一次性解包到临时目录）或
//! unpack 产物目录（直接读写）。本模块不维护跨调用状态：产物由用户显式创建
//! （`unpack`），修改即是对用户可见文件的修改。

use json2pdf::model::{DocumentModel, PageModel};
use office_core::CliError;
use std::path::{Path, PathBuf};

type Result<T> = std::result::Result<T, CliError>;

/// 判断目录是否为 unpack 产物根（含 document.json）
pub fn is_product_dir(path: &Path) -> bool {
    path.is_dir() && path.join("document.json").is_file()
}

/// 读取顶层 document.json
pub fn load_document(product_root: &Path) -> Result<DocumentModel> {
    let text = std::fs::read_to_string(product_root.join("document.json"))
        .map_err(|e| CliError::new(format!("读取 document.json 失败: {e}")))?;
    serde_json::from_str(&text).map_err(|e| CliError::new(format!("解析 document.json 失败: {e}")))
}

/// 读取产物中的全部页面（按 document.json 的 pages 清单）
pub fn load_pages(product_root: &Path) -> Result<Vec<PageModel>> {
    let doc = load_document(product_root)?;
    let mut pages = Vec::with_capacity(doc.pages.len());
    for rel in &doc.pages {
        let text = std::fs::read_to_string(product_root.join(rel))
            .map_err(|e| CliError::new(format!("读取 {rel} 失败: {e}")))?;
        let page: PageModel = serde_json::from_str(&text)
            .map_err(|e| CliError::new(format!("解析 {rel} 失败: {e}")))?;
        pages.push(page);
    }
    Ok(pages)
}

/// 写回顶层 document.json
pub fn save_document(product_root: &Path, doc: &DocumentModel) -> Result<()> {
    let json = serde_json::to_string_pretty(doc).map_err(|e| CliError::new(e.to_string()))?;
    std::fs::write(product_root.join("document.json"), json)
        .map_err(|e| CliError::with_code("io", format!("写 document.json 失败: {e}")))?;
    Ok(())
}

/// 按清单序号写回单个页面（0-based）
pub fn save_page(product_root: &Path, page_idx: usize, page: &PageModel) -> Result<()> {
    let doc = load_document(product_root)?;
    let rel = doc
        .pages
        .get(page_idx)
        .ok_or_else(|| CliError::new(format!("页面索引越界: {}", page_idx + 1)))?;
    write_page_file(product_root, rel, page)
}

/// 写回全部页面与 document.json（serve / batch 用）
pub fn save_all(product_root: &Path, doc: &DocumentModel, pages: &[PageModel]) -> Result<()> {
    for (i, p) in pages.iter().enumerate() {
        if let Some(rel) = doc.pages.get(i) {
            write_page_file(product_root, rel, p)?;
        }
    }
    save_document(product_root, doc)?;
    Ok(())
}

fn write_page_file(product_root: &Path, rel: &str, page: &PageModel) -> Result<()> {
    let path = product_root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CliError::with_code("io", format!("创建目录失败: {e}")))?;
    }
    let json = serde_json::to_string_pretty(page).map_err(|e| CliError::new(e.to_string()))?;
    std::fs::write(&path, json)
        .map_err(|e| CliError::with_code("io", format!("写 {rel} 失败: {e}")))?;
    Ok(())
}

/// 为文档追加一页：分配 pages/page-NNN.json 文件名并登记清单
pub fn append_page(
    product_root: &Path,
    doc: &mut DocumentModel,
    page: &PageModel,
) -> Result<String> {
    let mut n = doc.pages.len() + 1;
    let rel = loop {
        let candidate = format!("pages/page-{n:03}.json");
        if !doc.pages.contains(&candidate) && !product_root.join(&candidate).exists() {
            break candidate;
        }
        n += 1;
    };
    write_page_file(product_root, &rel, page)?;
    doc.pages.push(rel.clone());
    save_document(product_root, doc)?;
    Ok(rel)
}

/// 删除一页：移出清单并删除分片文件
pub fn remove_page(product_root: &Path, doc: &mut DocumentModel, page_idx: usize) -> Result<()> {
    let rel = doc
        .pages
        .get(page_idx)
        .ok_or_else(|| CliError::new(format!("页面索引越界: {}", page_idx + 1)))?
        .clone();
    doc.pages.remove(page_idx);
    save_document(product_root, doc)?;
    let _ = std::fs::remove_file(product_root.join(&rel));
    Ok(())
}

/// 页面有效尺寸（页面覆盖 > 顶层 page_size）
pub fn page_size(doc: &DocumentModel, page: &PageModel) -> (f64, f64) {
    (
        page.width.unwrap_or(doc.page_size.width),
        page.height.unwrap_or(doc.page_size.height),
    )
}

/// 将媒体 src 解析为绝对路径（URL 保留原样）
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

/// 输入解析：产物目录直接用；PDF 文件解包到一次性临时目录（guard drop 自动删除）
pub fn resolve_input(input: &Path) -> Result<(PathBuf, Option<office_core::TempGuard>)> {
    if is_product_dir(input) {
        return Ok((input.to_path_buf(), None));
    }
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "doc".to_string());
    let tmp = std::env::temp_dir().join(format!("json2pdf-{}-{stem}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    json2pdf::unpack::unpack(input, &tmp)
        .map_err(|e| CliError::new(format!("解包 {} 失败: {e}", input.display())))?;
    Ok((tmp.clone(), Some(office_core::TempGuard { path: tmp })))
}
