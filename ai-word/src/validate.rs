//! 轻量结构校验：产物目录 / docx 包的一致性问题（缺件、坏 XML、悬空关系/媒体）

use crate::model::blocks::Block;
use crate::model::Document;
use std::collections::HashSet;
use std::path::Path;

/// 校验 docx 包：解压后检查必需部件、关系目标、重复条目
pub fn validate_docx(path: &Path) -> Vec<String> {
    let mut issues = Vec::new();
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => return vec![format!("无法打开文件: {e}")],
    };
    let mut zip = match zip::ZipArchive::new(file) {
        Ok(z) => z,
        Err(e) => return vec![format!("非有效 ZIP/docx: {e}")],
    };
    let mut names: HashSet<String> = HashSet::new();
    let mut blobs: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
    for i in 0..zip.len() {
        match zip.by_index(i) {
            Ok(mut e) => {
                let name = e.name().to_string();
                if !names.insert(name.clone()) {
                    issues.push(format!("重复条目: {name}"));
                }
                let mut buf = Vec::new();
                use std::io::Read;
                let _ = e.read_to_end(&mut buf);
                blobs.insert(name, buf);
            }
            Err(e) => issues.push(format!("读取条目失败: {e}")),
        }
    }
    if !names.contains("[Content_Types].xml") {
        issues.push("缺少 [Content_Types].xml".into());
    } else if let Some(b) = blobs.get("[Content_Types].xml") {
        check_xml("[Content_Types].xml", b, &mut issues);
    }
    if !names.contains("word/document.xml") {
        issues.push("缺少 word/document.xml".into());
    } else if let Some(b) = blobs.get("word/document.xml") {
        check_xml("word/document.xml", b, &mut issues);
    }
    // 关系目标存在性
    for (name, bytes) in &blobs {
        if !name.ends_with(".rels") {
            continue;
        }
        let base = rels_base(name);
        if let Ok(root) = crate::parse::xmltree::parse_xml(&String::from_utf8_lossy(bytes)) {
            for rel in root.children_named("Relationship") {
                if rel.attr("TargetMode") == Some("External") {
                    continue;
                }
                let Some(target) = rel.attr("Target") else {
                    continue;
                };
                let resolved = resolve_target(&base, target);
                if !names.contains(&resolved) {
                    issues.push(format!("关系目标缺失: {name} -> {target}"));
                }
            }
        } else {
            check_xml(name, bytes, &mut issues);
        }
    }
    issues
}

/// 校验产物目录：document.json、分片、媒体引用
pub fn validate_product(root: &Path) -> Vec<String> {
    let mut issues = Vec::new();
    let doc_path = root.join("document.json");
    let txt = match std::fs::read_to_string(&doc_path) {
        Ok(t) => t,
        Err(e) => return vec![format!("缺少/无法读取 document.json: {e}")],
    };
    let mut value: serde_json::Value = match serde_json::from_str(&txt) {
        Ok(v) => v,
        Err(e) => return vec![format!("document.json 解析失败: {e}")],
    };
    // parts
    let part_paths: Vec<String> = value
        .get("parts")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let mut parts: Vec<crate::model::Part> = Vec::new();
    for p in &part_paths {
        match std::fs::read_to_string(root.join(p)) {
            Ok(t) => match serde_json::from_str::<crate::model::Part>(&t) {
                Ok(part) => parts.push(part),
                Err(e) => issues.push(format!("分片解析失败 {p}: {e}")),
            },
            Err(e) => issues.push(format!("分片缺失 {p}: {e}")),
        }
    }
    if let Some(obj) = value.as_object_mut() {
        obj.insert("parts".to_string(), serde_json::Value::Array(vec![]));
    }
    if let Ok(mut doc) = serde_json::from_value::<Document>(value) {
        let parts_final = if parts.is_empty() {
            std::mem::take(&mut doc.parts)
        } else {
            parts
        };
        // 媒体引用检查
        let mut refs: Vec<String> = Vec::new();
        for part in &parts_final {
            collect_image_refs(&part.blocks, &mut refs);
        }
        for src in refs {
            if src.starts_with("http://") || src.starts_with("https://") {
                continue;
            }
            if !root.join(&src).exists() {
                issues.push(format!("媒体缺失: {src}"));
            }
        }
    } else {
        issues.push("document.json 顶层模型解析失败".into());
    }
    issues
}

fn check_xml(name: &str, bytes: &[u8], issues: &mut Vec<String>) {
    let s = String::from_utf8_lossy(bytes);
    if crate::parse::xmltree::parse_xml(&s).is_err() {
        issues.push(format!("XML 解析失败: {name}"));
    }
}

fn rels_base(rel_path: &str) -> String {
    // word/_rels/document.xml.rels -> word
    // _rels/.rels -> ""
    if let Some(rest) = rel_path.strip_suffix(".rels") {
        if let Some(idx) = rest.rfind("/_rels/") {
            return rest[..idx].to_string();
        }
        if rest.starts_with("_rels/") {
            return String::new();
        }
    }
    String::new()
}

fn resolve_target(base: &str, target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("word/") || base.is_empty() {
        return target.to_string();
    }
    // 相对 base 解析
    let mut parts: Vec<&str> = if base.is_empty() {
        Vec::new()
    } else {
        base.split('/').collect()
    };
    for seg in target.split('/') {
        match seg {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn collect_image_refs(blocks: &[Block], out: &mut Vec<String>) {
    for b in blocks {
        match b {
            Block::Image(i) => out.push(i.src.clone()),
            Block::Attachment(a) => out.push(a.src.clone()),
            Block::Paragraph(p) => {
                if let Some(runs) = &p.runs {
                    for r in runs {
                        if let Some(img) = &r.image {
                            out.push(img.src.clone());
                        }
                    }
                }
            }
            Block::List(l) => {
                for it in &l.items {
                    collect_image_refs(&it.blocks, out);
                }
            }
            Block::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        collect_image_refs(&cell.blocks, out);
                    }
                }
            }
            Block::Sdt(c) => collect_image_refs(&c.blocks, out),
            _ => {}
        }
    }
}

/// 内容质量告警（启发式，供 `view` 输出 warnings）
pub fn content_warnings(doc: &Document) -> Vec<String> {
    let mut w = Vec::new();
    let mut headings: Vec<u8> = Vec::new();
    let mut empty = 0usize;
    let mut no_alt = 0usize;
    let mut placeholders = 0usize;
    let mut has_toc = false;
    for part in &doc.parts {
        warnings_walk(
            &part.blocks,
            &mut headings,
            &mut empty,
            &mut no_alt,
            &mut placeholders,
            &mut has_toc,
        );
    }
    if empty > 0 {
        w.push(format!(
            "存在 {empty} 个空段落（建议用 space_before/space_after 控制间距）"
        ));
    }
    if no_alt > 0 {
        w.push(format!("{no_alt} 张图片缺少 alt/描述"));
    }
    if placeholders > 0 {
        w.push(format!(
            "发现 {placeholders} 处未解析的占位符（{{{{...}}}}/xxx）"
        ));
    }
    // 标题层级跳级
    let mut prev = 0u8;
    let mut jumps = 0;
    for &lvl in &headings {
        if prev != 0 && lvl > prev + 1 {
            jumps += 1;
        }
        prev = lvl;
    }
    if jumps > 0 {
        w.push(format!("标题层级存在 {jumps} 处跳级（如 H1→H3）"));
    }
    if headings.len() >= 3 && !has_toc {
        w.push("文档有 3+ 个标题但未检测到目录（建议加 TOC）".to_string());
    }
    w
}

fn warnings_walk(
    blocks: &[Block],
    headings: &mut Vec<u8>,
    empty: &mut usize,
    no_alt: &mut usize,
    placeholders: &mut usize,
    has_toc: &mut bool,
) {
    for b in blocks {
        match b {
            Block::Heading { level, text, .. } => {
                headings.push(*level);
                scan_placeholder(text, placeholders);
            }
            Block::Paragraph(p) => {
                let t = p.text.clone().unwrap_or_default();
                let has_img = p
                    .runs
                    .as_ref()
                    .map(|rs| rs.iter().any(|r| r.image.is_some()))
                    .unwrap_or(false);
                if t.trim().is_empty()
                    && p.runs.as_ref().map(|r| r.is_empty()).unwrap_or(true)
                    && !has_img
                {
                    *empty += 1;
                }
                scan_placeholder(&t, placeholders);
                if let Some(rs) = &p.runs {
                    for r in rs {
                        scan_placeholder(&r.text, placeholders);
                    }
                }
            }
            Block::Image(i)
                if i.alt
                    .as_deref()
                    .map(|a| a.trim().is_empty())
                    .unwrap_or(true) =>
            {
                *no_alt += 1;
            }
            Block::Toc(_) => *has_toc = true,
            Block::List(l) => {
                for it in &l.items {
                    warnings_walk(&it.blocks, headings, empty, no_alt, placeholders, has_toc);
                }
            }
            Block::Table(t) => {
                for row in &t.rows {
                    for c in &row.cells {
                        warnings_walk(&c.blocks, headings, empty, no_alt, placeholders, has_toc);
                    }
                }
            }
            Block::Sdt(c) => {
                warnings_walk(&c.blocks, headings, empty, no_alt, placeholders, has_toc)
            }
            _ => {}
        }
    }
}

fn scan_placeholder(s: &str, n: &mut usize) {
    if s.contains("{{") || s.contains("}}") {
        *n += s.matches("{{").count().max(1);
    }
}
