//! Product-directory intermediate format (JSON-first authoring).
//!
//! A product directory mirrors the xlsx package layout:
//!
//! ```text
//! book/
//! ├── workbook.json            # top-level meta/styles + `sheets` path array
//! └── xl/
//!     └── worksheets/
//!         ├── sheet1.json
//!         └── sheet2.json
//! ```

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::generate::{generate, GenerateResult};
use crate::model::style::{Font, StyleDef};
use crate::model::workbook::{Meta, NamedRange, Workbook};
use crate::model::Sheet;
use crate::parse::parse;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ProductWorkbook {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub date1904: bool,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub styles: HashMap<String, StyleDef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_font: Option<Font>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tab: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub named_ranges: Vec<NamedRange>,
    /// Relative paths, e.g. `["xl/worksheets/sheet1.json"]`.
    pub sheets: Vec<String>,
    /// 原样保留的“不透明”部件（VBA、透视/切片缓存、customXml 等）的包内路径。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_parts: Vec<String>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone)]
pub struct UnpackResult {
    pub dir: String,
    pub sheets: usize,
}

/// Check whether a path is a product directory.
pub fn is_product_dir(path: &Path) -> bool {
    path.join("workbook.json").is_file()
}

/// Unpack an `.xlsx` into a product directory.
pub fn unpack(input: &Path, out_dir: &Path) -> Result<UnpackResult> {
    let wb = parse(input)?;
    std::fs::create_dir_all(out_dir.join("xl/worksheets"))?;

    let mut paths = Vec::with_capacity(wb.sheets.len());
    for (i, sheet) in wb.sheets.iter().enumerate() {
        let rel = format!("xl/worksheets/sheet{}.json", i + 1);
        std::fs::write(out_dir.join(&rel), serde_json::to_string_pretty(sheet)?)?;
        paths.push(rel);
    }

    if !crate::legacy::is_cfb(input) {
        extract_media(input, out_dir)?;
    }

    if let Some(theme) = &wb.theme {
        let dest = out_dir.join("xl/theme/theme1.xml");
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, theme)?;
    }

    // 旧版 .xls（OLE2）没有 OOXML 的 extra parts，跳过提取
    let extra_parts = if crate::legacy::is_cfb(input) {
        Vec::new()
    } else {
        extract_extra_parts(input, out_dir)?
    };

    let product = ProductWorkbook {
        meta: wb.meta.clone(),
        date1904: wb.date1904,
        styles: wb.styles.clone(),
        default_font: wb.default_font.clone(),
        active_tab: wb.active_tab,
        named_ranges: wb.named_ranges.clone(),
        sheets: paths,
        extra_parts,
    };
    std::fs::write(
        out_dir.join("workbook.json"),
        serde_json::to_string_pretty(&product)?,
    )?;

    Ok(UnpackResult {
        dir: out_dir.display().to_string(),
        sheets: wb.sheets.len(),
    })
}

/// Copy `xl/media/*` entries from the source xlsx into the product directory.
fn extract_media(input: &Path, out_dir: &Path) -> Result<()> {
    use std::io::Read;

    let file = std::fs::File::open(input)?;
    let mut zip = zip::ZipArchive::new(file)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_string();
        if !name.starts_with("xl/media/") || name.ends_with('/') {
            continue;
        }
        let dest = out_dir.join(&name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        std::fs::write(&dest, bytes)?;
    }
    Ok(())
}

/// Load a product directory into a [`Workbook`].
pub fn load_product(dir: &Path) -> Result<Workbook> {
    let txt = std::fs::read_to_string(dir.join("workbook.json")).map_err(|_| {
        Error::InvalidInput(format!(
            "{} 不是产物目录（缺少 workbook.json）",
            dir.display()
        ))
    })?;
    let product: ProductWorkbook = serde_json::from_str(&txt)?;

    let mut sheets = Vec::with_capacity(product.sheets.len());
    for rel in &product.sheets {
        let t = std::fs::read_to_string(dir.join(rel))?;
        let sheet: Sheet = serde_json::from_str(&t)?;
        sheets.push(sheet);
    }

    let theme_path = dir.join("xl/theme/theme1.xml");
    let theme = if theme_path.is_file() {
        Some(std::fs::read_to_string(&theme_path)?)
    } else {
        None
    };

    Ok(Workbook {
        meta: product.meta,
        date1904: product.date1904,
        styles: product.styles,
        default_font: product.default_font,
        active_tab: product.active_tab,
        named_ranges: product.named_ranges,
        sheets,
        theme,
    })
}

/// Build an `.xlsx` from a product directory.
pub fn repack(dir: &Path, output: &Path) -> Result<GenerateResult> {
    let mut wb = load_product(dir)?;
    absolutize_images(&mut wb, dir);
    let mut result = generate(&wb, output)?;
    // 回写保留的“不透明”部件（VBA、透视/切片缓存、customXml 等）。
    let extras = read_extra_parts(dir)?;
    if !extras.is_empty() {
        append_extra_parts(dir, output, &extras)?;
        result.path = output.display().to_string();
    }
    Ok(result)
}

/// 读取 workbook.json 中记录的保留部件列表。
fn read_extra_parts(dir: &Path) -> Result<Vec<String>> {
    let txt = std::fs::read_to_string(dir.join("workbook.json")).unwrap_or_default();
    let product: ProductWorkbook = serde_json::from_str(&txt).unwrap_or_default();
    Ok(product.extra_parts)
}

/// 生成包未覆盖的部件路径（返回是否为“已生成/拥有”的部件）。
fn owned_part(name: &str) -> bool {
    let n = name.replace('\\', "/").to_ascii_lowercase();
    if n.ends_with('/') {
        return true;
    }
    // 图表样式/配色及其关系由 rels 隐式引用，模型不重建：作为不透明部件保留。
    // VML 关系同理（可能含非注释控件）。
    if n.starts_with("xl/charts/_rels/")
        || n.starts_with("xl/charts/colors")
        || n.starts_with("xl/charts/style")
        || n.contains("/_rels/vmldrawing")
    {
        return false;
    }
    matches!(
        n.as_str(),
        "[content_types].xml"
            | "_rels/.rels"
            | "docprops/core.xml"
            | "xl/workbook.xml"
            | "xl/_rels/workbook.xml.rels"
            | "xl/styles.xml"
            | "xl/sharedstrings.xml"
            | "xl/theme/theme1.xml"
    ) || n.starts_with("xl/worksheets/")
        || n.starts_with("xl/media/")
        || n.starts_with("xl/drawings/")
        || n.starts_with("xl/charts/")
        || n.starts_with("xl/tables/")
        || n.starts_with("xl/comments")
}

/// 复制 zip 中未被生成覆盖的部件到产物目录，返回其路径列表。
fn extract_extra_parts(input: &Path, out_dir: &Path) -> Result<Vec<String>> {
    use std::io::Read;

    let file = std::fs::File::open(input)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        entries.push((name, bytes));
    }

    // 图表工作表的依赖（drawing/chart/media）需保留（这些原本属于“已生成”前缀）。
    let deps = chartsheet_deps(&entries);

    // 抽取 <pivotCaches> 与其工作簿关系，回写时注入（否则 LO/Excel 无法关联透视缓存）。
    if let Some((caches, rels)) = pivot_info(&entries) {
        let obj = serde_json::json!({ "caches": caches, "rels": rels });
        std::fs::write(out_dir.join("_pivot.json"), obj.to_string())?;
    }

    let mut extras = Vec::new();
    for (name, bytes) in &entries {
        // 单独保存原始 [Content_Types].xml，用于回写时补齐保留部件的类型声明。
        if name.eq_ignore_ascii_case("[Content_Types].xml") {
            std::fs::write(out_dir.join("_orig_content_types.xml"), bytes)?;
            continue;
        }
        // 图表工作表本身与其关系文件由模型原样回写，不作为额外部件。
        if name.starts_with("xl/chartsheets/") {
            continue;
        }
        if owned_part(name) && !deps.contains(name) {
            continue;
        }
        let dest = out_dir.join(name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, bytes)?;
        extras.push(name.clone());
    }
    Ok(extras)
}

/// 收集所有图表工作表所引用的 drawing / chart / media 包内路径。
fn chartsheet_deps(entries: &[(String, Vec<u8>)]) -> std::collections::HashSet<String> {
    use std::collections::{HashMap, HashSet};

    let map: HashMap<&str, &[u8]> = entries
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    let mut deps = HashSet::new();
    for (name, bytes) in entries {
        if !(name.starts_with("xl/chartsheets/_rels/") && name.ends_with(".rels")) {
            continue;
        }
        let text = String::from_utf8_lossy(bytes);
        for target in rel_targets(&text) {
            let p = crate::parse::drawing::resolve_part("xl/chartsheets", &target);
            deps.insert(p.clone());
            let base = p.rsplit('/').next().unwrap_or("");
            let drels = format!("xl/drawings/_rels/{base}.rels");
            if let Some(db) = map.get(drels.as_str()) {
                deps.insert(drels.clone());
                for t2 in rel_targets(&String::from_utf8_lossy(db)) {
                    let p2 = crate::parse::drawing::resolve_part("xl/drawings", &t2);
                    deps.insert(p2);
                }
            }
        }
    }
    deps
}

/// 从 `.rels` XML 中提取所有 `Target="..."`。
fn rel_targets(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = xml[pos..].find("Target=\"") {
        let start = pos + i + "Target=\"".len();
        if let Some(j) = xml[start..].find('"') {
            out.push(xml[start..start + j].to_string());
            pos = start + j + 1;
        } else {
            break;
        }
    }
    out
}

/// 从工作簿中抽取 `<pivotCaches>` 与其 pivotCacheDefinition 关系。
fn pivot_info(entries: &[(String, Vec<u8>)]) -> Option<(String, Vec<(String, String)>)> {
    let wb = entries.iter().find(|(n, _)| n == "xl/workbook.xml")?;
    let s = String::from_utf8_lossy(&wb.1);
    let start = s.find("<pivotCaches")?;
    let rel_end = s[start..].find("</pivotCaches>")?;
    let end = start + rel_end + "</pivotCaches>".len();
    let caches = s[start..end].to_string();

    let rels_b = entries
        .iter()
        .find(|(n, _)| n == "xl/_rels/workbook.xml.rels")?
        .1
        .clone();
    let rels_s = String::from_utf8_lossy(&rels_b).into_owned();
    let mut rels = Vec::new();
    for frag in relationship_frags(&rels_s) {
        if attr(&frag, "Type")
            .map(|t| t.contains("pivotCacheDefinition"))
            .unwrap_or(false)
        {
            if let (Some(id), Some(target)) = (attr(&frag, "Id"), attr(&frag, "Target")) {
                rels.push((id, target));
            }
        }
    }
    if rels.is_empty() {
        return None;
    }
    Some((caches, rels))
}

/// 拆分出所有 `<Relationship .../>` 片段。
fn relationship_frags(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = xml[pos..].find("<Relationship") {
        let start = pos + i;
        let end = xml[start..]
            .find('>')
            .map(|e| start + e)
            .unwrap_or(xml.len());
        out.push(xml[start..end].to_string());
        pos = end + 1;
    }
    out
}

/// 把 pivotCaches 与其关系注入生成的工作簿（重编号 rId 避免冲突）。
fn patch_pivot(entries: &mut [(String, Vec<u8>)], caches: &str, rels: &[(String, String)]) {
    let rels_name = "xl/_rels/workbook.xml.rels";
    let wb_name = "xl/workbook.xml";

    // 现有最大 rId 编号。
    let mut max_id = 0u32;
    if let Some((_, b)) = entries.iter().find(|(n, _)| n == rels_name) {
        let s = String::from_utf8_lossy(b);
        for frag in relationship_frags(&s) {
            if let Some(id) =
                attr(&frag, "Id").and_then(|x| x.strip_prefix("rId")?.parse::<u32>().ok())
            {
                max_id = max_id.max(id);
            }
        }
    }

    let mut new_rels = String::new();
    let mut new_caches = caches.to_string();
    for (old, target) in rels {
        max_id += 1;
        let new_id = format!("rId{max_id}");
        new_rels.push_str(&format!(
            "<Relationship Id=\"{new_id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition\" Target=\"{target}\"/>"
        ));
        new_caches = new_caches.replace(&format!("r:id=\"{old}\""), &format!("r:id=\"{new_id}\""));
    }

    if let Some((_, b)) = entries.iter_mut().find(|(n, _)| n == rels_name) {
        let mut s = String::from_utf8_lossy(b).into_owned();
        if let Some(p) = s.rfind("</Relationships>") {
            s.insert_str(p, &new_rels);
        }
        *b = s.into_bytes();
    }
    if let Some((_, b)) = entries.iter_mut().find(|(n, _)| n == wb_name) {
        let mut s = String::from_utf8_lossy(b).into_owned();
        // 插入到 </workbook>（或 <extLst>）之前。
        let pos = s
            .find("<extLst")
            .or_else(|| s.rfind("</workbook>"))
            .unwrap_or(s.len());
        s.insert_str(pos, &new_caches);
        *b = s.into_bytes();
    }
}

/// 把保留部件追加回生成的 zip，并合并 [Content_Types].xml 中缺失的类型声明。
fn append_extra_parts(dir: &Path, output: &Path, extras: &[String]) -> Result<()> {
    use std::io::{Read, Write};

    // 1) 读出生成包的全部条目
    let file = std::fs::File::open(output)?;
    let mut zip = zip::ZipArchive::new(file)?;
    let mut entries: Vec<(String, Vec<u8>)> = Vec::with_capacity(zip.len() + extras.len());
    for i in 0..zip.len() {
        let mut e = zip.by_index(i)?;
        let name = e.name().to_string();
        let mut buf = Vec::new();
        e.read_to_end(&mut buf)?;
        entries.push((name, buf));
    }
    drop(zip);

    // 2) 合并 [Content_Types].xml
    let ct_path = dir.join("_orig_content_types.xml");
    if let Ok(orig) = std::fs::read_to_string(&ct_path) {
        for (name, data) in entries.iter_mut() {
            if name.eq_ignore_ascii_case("[Content_Types].xml") {
                let base = String::from_utf8_lossy(data).into_owned();
                *data = merge_content_types(&base, &orig).into_bytes();
            }
        }
    }

    // 3) 注入透视缓存关系（<pivotCaches> + workbook rels）
    if let Ok(txt) = std::fs::read_to_string(dir.join("_pivot.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&txt) {
            let caches = v["caches"].as_str().unwrap_or_default().to_string();
            let rels: Vec<(String, String)> = v["rels"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            Some((
                                x.get(0)?.as_str()?.to_string(),
                                x.get(1)?.as_str()?.to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if !caches.is_empty() && !rels.is_empty() {
                patch_pivot(&mut entries, &caches, &rels);
            }
        }
    }

    // 4) 追加保留部件（同名者覆盖生成包中的占位版本）
    for extra in extras {
        let src = dir.join(extra);
        if let Ok(bytes) = std::fs::read(&src) {
            if let Some(slot) = entries.iter_mut().find(|(n, _)| n == extra) {
                slot.1 = bytes;
            } else {
                entries.push((extra.clone(), bytes));
            }
        }
    }

    // 5) 重写 zip
    let out = std::fs::File::create(output)?;
    let mut writer = zip::ZipWriter::new(out);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, data) in &entries {
        writer.start_file(name.as_str(), opts)?;
        writer.write_all(data)?;
    }
    writer.finish()?;
    Ok(())
}

/// 把 `orig` 中「基础包尚不存在」的 Default/Override 追加入 `base`。
fn merge_content_types(base: &str, orig: &str) -> String {
    let (od, oo) = ct_entries(orig);
    let (bd, bo) = ct_entries(base);
    let has_default = |ext: &str| bd.iter().any(|(e, _)| e.eq_ignore_ascii_case(ext));
    let has_override = |part: &str| bo.iter().any(|(p, _)| p.eq_ignore_ascii_case(part));
    let mut add = String::new();
    for (ext, ct) in &od {
        if !has_default(ext) {
            add.push_str(&format!(
                "<Default Extension=\"{ext}\" ContentType=\"{ct}\"/>"
            ));
        }
    }
    for (part, ct) in &oo {
        if !has_override(part) {
            add.push_str(&format!(
                "<Override PartName=\"{part}\" ContentType=\"{ct}\"/>"
            ));
        }
    }
    if let Some(pos) = base.rfind("</Types>") {
        format!("{}{}{}", &base[..pos], add, &base[pos..])
    } else {
        base.to_string()
    }
}

/// `[Content_Types].xml` 的 (键, ContentType) 条目。
type CtTable = Vec<(String, String)>;

/// 解析 `[Content_Types].xml` 的 (Defaults, Overrides) 列表。
fn ct_entries(s: &str) -> (CtTable, CtTable) {
    let mut defaults: CtTable = Vec::new();
    let mut overrides: CtTable = Vec::new();
    for tag in ["<Default", "<Override"] {
        let mut pos = 0;
        while let Some(i) = s[pos..].find(tag) {
            let start = pos + i;
            let end = s[start..].find('>').map(|e| start + e).unwrap_or(s.len());
            let frag = &s[start..end];
            let ext = attr(frag, "Extension");
            let part = attr(frag, "PartName");
            let ct = attr(frag, "ContentType");
            if let (Some(k), Some(v)) = (ext.or(part), ct) {
                if tag == "<Default" {
                    defaults.push((k, v));
                } else {
                    overrides.push((k, v));
                }
            }
            pos = end + 1;
        }
    }
    (defaults, overrides)
}

fn attr(frag: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let i = frag.find(&needle)? + needle.len();
    let j = frag[i..].find('"')? + i;
    Some(frag[i..j].to_string())
}

/// Resolve relative image `src` paths against the product directory root.
pub fn absolutize_images(wb: &mut Workbook, base: &Path) {
    for sheet in &mut wb.sheets {
        for img in &mut sheet.images {
            let p = std::path::Path::new(&img.src);
            if !p.is_absolute() && !img.src.contains("://") {
                img.src = base.join(p).display().to_string();
            }
        }
        if let Some(bg) = &sheet.background {
            let p = std::path::Path::new(bg);
            if !p.is_absolute() && !bg.contains("://") {
                sheet.background = Some(base.join(p).display().to_string());
            }
        }
        for ole in &mut sheet.ole_objects {
            let p = std::path::Path::new(&ole.src);
            if !p.is_absolute() && !ole.src.contains("://") {
                ole.src = base.join(p).display().to_string();
            }
        }
    }
}

/// Write a workbook back into a product directory (in-place editing).
pub fn save_product(wb: &Workbook, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir.join("xl/worksheets"))?;
    let mut paths = Vec::with_capacity(wb.sheets.len());
    for (i, sheet) in wb.sheets.iter().enumerate() {
        let rel = format!("xl/worksheets/sheet{}.json", i + 1);
        std::fs::write(dir.join(&rel), serde_json::to_string_pretty(sheet)?)?;
        paths.push(rel);
    }
    let existing: ProductWorkbook = std::fs::read_to_string(dir.join("workbook.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let product = ProductWorkbook {
        meta: wb.meta.clone(),
        date1904: wb.date1904,
        styles: wb.styles.clone(),
        default_font: wb.default_font.clone(),
        active_tab: wb.active_tab,
        named_ranges: wb.named_ranges.clone(),
        sheets: paths,
        extra_parts: existing.extra_parts,
    };
    std::fs::write(
        dir.join("workbook.json"),
        serde_json::to_string_pretty(&product)?,
    )?;
    Ok(())
}
