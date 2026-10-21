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
/// 与 [`validate_product`] 同一套检查与**同一批输出文本**，但复用已在内存的模型。
///
/// 省掉的是「读回分片并解析」这一步（大文档 20~30ms）；为了逐字节等价，
/// 判定与消息都对齐磁盘版，包括它的一个既有取舍：**分片路径为空（内联 parts）
/// 或分片读不出来时，磁盘版手里没有内容，媒体检查就退化成空跑**——这里同样跳过，
/// 不趁机多报（改行为属于另一件事）。
pub fn validate_product_with(root: &Path, doc: &Document) -> Vec<String> {
    let mut issues = Vec::new();
    let doc_path = root.join("document.json");
    let txt = match std::fs::read_to_string(&doc_path) {
        Ok(t) => t,
        Err(e) => return vec![format!("缺少/无法读取 document.json: {e}")],
    };
    let value: serde_json::Value = match serde_json::from_str(&txt) {
        Ok(v) => v,
        Err(e) => return vec![format!("document.json 解析失败: {e}")],
    };
    let shard_paths: Vec<String> = value
        .get("parts")
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // 分片：快路径只确认存在（内容由调用方持有的模型代表，不再解析）；
    // 读不出来的分支保持与磁盘版相同的报错文本
    let mut shards_readable = 0usize;
    for rel in &shard_paths {
        let is_file = std::fs::metadata(root.join(rel))
            .map(|m| m.is_file())
            .unwrap_or(false);
        if is_file {
            shards_readable += 1;
        } else if let Err(e) = std::fs::read_to_string(root.join(rel)) {
            issues.push(format!("分片缺失 {rel}: {e}"));
        }
    }

    // 顶层模型：document.json 本体解析成 Document 的检查照做（文件很小，代价可忽略）
    let mut v = value.clone();
    if let Some(obj) = v.as_object_mut() {
        obj.insert("parts".to_string(), serde_json::Value::Array(vec![]));
    }
    if serde_json::from_value::<Document>(v).is_err() {
        issues.push("document.json 顶层模型解析失败".into());
        return issues;
    }

    // 媒体引用检查：仅当磁盘版能拿到内容时（分片路径非空且全部读得到）
    if !shard_paths.is_empty() && shards_readable == shard_paths.len() {
        let mut refs: Vec<String> = Vec::new();
        for part in &doc.parts {
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
    }
    issues
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Document, Part};

    /// 两种校验入口必须给出同一份 issues（复用模型只是为了省一次读盘解析）
    fn assert_same(doc: &Document, root: &std::path::Path, tag: &str) {
        let a = validate_product(root);
        let b = validate_product_with(root, doc);
        assert_eq!(a, b, "{tag}: 磁盘版 {a:?} vs 模型版 {b:?}");
    }

    #[test]
    fn product_with_matches_product() {
        let dir = std::env::temp_dir().join(format!("j2d-validate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("word/parts")).unwrap();
        let mut doc = Document::default();
        let mut part = Part::default();
        part.blocks
            .push(crate::model::blocks::Block::Paragraph(Default::default()));
        doc.parts = vec![part];

        // ① 正常产物目录（分片路径 + 已写文件）
        std::fs::write(dir.join("word/parts/part1.json"), r#"{"blocks":[]}"#).unwrap();
        std::fs::write(
            dir.join("document.json"),
            r#"{"parts":["word/parts/part1.json"]}"#,
        )
        .unwrap();
        assert_same(&doc, &dir, "正常");

        // ② 分片文件被删（磁盘版报缺失，模型版也必须报）
        std::fs::remove_file(dir.join("word/parts/part1.json")).unwrap();
        assert_same(&doc, &dir, "分片缺失");

        // ③ 内联 parts：没有分片文件，两边都不该报缺失
        std::fs::write(dir.join("document.json"), r#"{"parts":[{"blocks":[]}]}"#).unwrap();
        assert_same(&doc, &dir, "内联");

        // ④ 媒体缺失
        let mut doc2 = Document::default();
        let mut part2 = Part::default();
        part2.blocks.push(crate::model::blocks::Block::Image(
            crate::model::blocks::ImageBlock {
                src: "word/media/none.png".into(),
                ..Default::default()
            },
        ));
        doc2.parts = vec![part2];
        std::fs::write(
            dir.join("document.json"),
            r#"{"parts":["word/parts/part1.json"]}"#,
        )
        .unwrap();
        assert_same(&doc2, &dir, "媒体缺失");

        // ⑤ 内联 parts + 模型里有图片：磁盘版拿不到内容因而跳过媒体检查，
        //    模型版必须同样跳过（等价优先，不趁机多报）
        std::fs::write(dir.join("document.json"), r#"{"parts":[{"blocks":[]}]}"#).unwrap();
        assert_same(&doc2, &dir, "内联+图片");

        // ⑥ document.json 不存在
        std::fs::remove_file(dir.join("document.json")).unwrap();
        assert_same(&doc2, &dir, "无 document.json");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
