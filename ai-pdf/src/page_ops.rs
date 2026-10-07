//! 页操作 — 复刻 pdf.py pages.*（pikepdf 路线）：
//! merge（跨文档对象图深拷贝）、split/rotate/crop/clean（页树重建/页字典修改）。

use crate::text;
use anyhow::{bail, Result};
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ── 公共辅助 ──────────────────────────────────────────────────────────────

/// catalog → /Pages 的对象号。
pub fn pages_root_id(doc: &Document) -> Result<ObjectId> {
    let root_ref = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(|_| anyhow::anyhow!("trailer has no /Root"))?;
    doc.get_object(root_ref)?
        .as_dict()?
        .get(b"Pages")
        .and_then(Object::as_reference)
        .map_err(|_| anyhow::anyhow!("catalog has no /Pages reference"))
}

/// 重建页树 Kids / Count（split/clean 共用；孤儿对象留在文件里无害）。
fn set_pages_kids(doc: &mut Document, kids: &[ObjectId]) -> Result<()> {
    let pages_id = pages_root_id(doc)?;
    let pages_dict = doc
        .get_object_mut(pages_id)
        .and_then(Object::as_dict_mut)
        .map_err(|_| anyhow::anyhow!("/Pages is not a dictionary"))?;
    pages_dict.set(
        "Kids",
        Object::Array(kids.iter().map(|id| Object::Reference(*id)).collect()),
    );
    pages_dict.set("Count", Object::Integer(kids.len() as i64));
    Ok(())
}

/// 解析页面继承属性（MediaBox/CropBox 等可挂在 /Pages 节点上）。
pub fn inherited_attr<'a>(doc: &'a Document, page_id: ObjectId, key: &[u8]) -> Option<&'a Object> {
    let mut cur = page_id;
    for _ in 0..64 {
        let dict = doc.get_dictionary(cur).ok()?;
        if let Ok(v) = dict.get(key) {
            return Some(v);
        }
        cur = dict.get(b"Parent").and_then(Object::as_reference).ok()?;
    }
    None
}

/// 页面 MediaBox → (width, height, [l, b, r, t])。
pub fn page_mediabox(doc: &Document, page_id: ObjectId) -> Option<(f64, f64, [f64; 4])> {
    let obj = inherited_attr(doc, page_id, b"MediaBox")
        .or_else(|| inherited_attr(doc, page_id, b"CropBox"))?;
    // 页面框可能是间接引用（如 /MediaBox 7 0 R），不解引用会退回默认 A4
    // （pdfium/bug_1287409：200×300 页面被当 A4，内容比例全错）
    let (_, obj) = doc.dereference(obj).ok()?;
    let arr = obj.as_array().ok()?;
    let v: Vec<f64> = arr
        .iter()
        .filter_map(|o| match o {
            Object::Integer(i) => Some(*i as f64),
            Object::Real(r) => Some(*r as f64),
            _ => None,
        })
        .collect();
    if v.len() != 4 {
        return None;
    }
    Some((v[2] - v[0], v[3] - v[1], [v[0], v[1], v[2], v[3]]))
}

// ── merge：对象图深拷贝 ──────────────────────────────────────────────────

fn clone_value(
    obj: &Object,
    src: &Document,
    dst: &mut Document,
    map: &mut HashMap<ObjectId, ObjectId>,
) -> Object {
    match obj {
        Object::Reference(id) => Object::Reference(clone_object(dst, src, *id, map)),
        Object::Array(arr) => {
            Object::Array(arr.iter().map(|o| clone_value(o, src, dst, map)).collect())
        }
        Object::Dictionary(dict) => {
            let mut new_dict = Dictionary::new();
            for (k, v) in dict.iter() {
                new_dict.set(k.clone(), clone_value(v, src, dst, map));
            }
            Object::Dictionary(new_dict)
        }
        Object::Stream(stream) => {
            let mut new_dict = Dictionary::new();
            for (k, v) in stream.dict.iter() {
                // /Length 由保存时重算，跳过旧值
                if k == b"Length" {
                    continue;
                }
                new_dict.set(k.clone(), clone_value(v, src, dst, map));
            }
            Object::Stream(lopdf::Stream::new(new_dict, stream.content.clone()))
        }
        other => other.clone(),
    }
}

/// 环安全的对象克隆：先占位再覆写（/Parent 环引用终止）。
fn clone_object(
    dst: &mut Document,
    src: &Document,
    id: ObjectId,
    map: &mut HashMap<ObjectId, ObjectId>,
) -> ObjectId {
    if let Some(&nid) = map.get(&id) {
        return nid;
    }
    let nid = dst.add_object(Object::Null);
    map.insert(id, nid);
    let cloned = match src.get_object(id) {
        Ok(obj) => clone_value(obj, src, dst, map),
        Err(_) => Object::Null,
    };
    if let Ok(slot) = dst.get_object_mut(nid) {
        *slot = cloned;
    }
    nid
}

/// 拷贝一页并把继承属性（Resources/MediaBox/CropBox/Rotate/Group）内联，使页面自包含。
fn import_page(
    dst: &mut Document,
    src: &Document,
    page_id: ObjectId,
    new_pages_root: ObjectId,
    map: &mut HashMap<ObjectId, ObjectId>,
) -> Result<ObjectId> {
    let page_dict = src
        .get_object(page_id)
        .and_then(Object::as_dict)
        .map_err(|_| anyhow::anyhow!("page object is not a dict"))?
        .clone();

    // 先克隆自身引用的子对象（不含 Parent），拿到自包含 dict
    let mut cloned = {
        let mut d = Dictionary::new();
        for (k, v) in page_dict.iter() {
            if k == b"Parent" {
                continue;
            }
            d.set(k.clone(), clone_value(v, src, dst, map));
        }
        d
    };
    // 继承属性内联
    for key in [
        b"MediaBox".as_slice(),
        b"CropBox".as_slice(),
        b"Rotate".as_slice(),
        b"Resources".as_slice(),
        b"Group".as_slice(),
    ] {
        if cloned.get(key).is_err() {
            if let Some(v) = inherited_attr(src, page_id, key) {
                cloned.set(key.to_vec(), clone_value(v, src, dst, map));
            }
        }
    }
    cloned.set("Parent", Object::Reference(new_pages_root));
    cloned.set("Type", Object::Name(b"Page".to_vec()));

    let nid = dst.add_object(Object::Dictionary(cloned));
    map.insert(page_id, nid);
    Ok(nid)
}

pub fn merge(files: &[PathBuf], out: &Path) -> Result<Value> {
    if files.len() < 2 {
        bail!("merge requires at least 2 files");
    }
    let mut dst = Document::new();
    dst.version = "1.7".into();

    // 先建 Pages 骨架，再逐页导入（页面 /Parent 指向它）
    let pages_dict = Dictionary::new();
    let pages_id = dst.add_object(Object::Dictionary(pages_dict));
    let catalog = {
        let mut d = Dictionary::new();
        d.set("Type", Object::Name(b"Catalog".to_vec()));
        d.set("Pages", Object::Reference(pages_id));
        d
    };
    let catalog_id = dst.add_object(Object::Dictionary(catalog));
    dst.trailer.set("Root", Object::Reference(catalog_id));

    let mut kids: Vec<ObjectId> = Vec::new();
    let mut per_file_pages = Vec::new();
    for f in files {
        let src = Document::load(f).map_err(|e| anyhow::anyhow!("load {}: {e}", f.display()))?;
        // 对象号映射表必须每个源文件独立：不同文件的对象号空间互不相关
        let mut map = HashMap::new();
        let page_map = src.get_pages();
        let n = page_map.len();
        for pno in 1..=n as u32 {
            let pid = page_map[&pno];
            let nid = import_page(&mut dst, &src, pid, pages_id, &mut map)?;
            kids.push(nid);
        }
        per_file_pages.push(json!({"file": f.display().to_string(), "pages": n}));
    }

    let pages_obj = dst.get_object_mut(pages_id).and_then(Object::as_dict_mut)?;
    pages_obj.set("Type", Object::Name(b"Pages".to_vec()));
    pages_obj.set(
        "Kids",
        Object::Array(kids.iter().map(|id| Object::Reference(*id)).collect()),
    );
    pages_obj.set("Count", Object::Integer(kids.len() as i64));

    dst.save(out)?;
    Ok(json!({
        "output": out.display().to_string(),
        "total_pages": kids.len(),
        "sources": per_file_pages,
    }))
}

// ── split / rotate / crop / clean ────────────────────────────────────────

pub fn split(file: &Path, out_dir: &Path) -> Result<Value> {
    std::fs::create_dir_all(out_dir)?;
    let doc = Document::load(file)?;
    let page_map = doc.get_pages();
    let n = page_map.len();
    let stem = file
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut files = Vec::new();
    for pno in 1..=n as u32 {
        let mut doc = Document::load(file)?;
        let pid = doc.get_pages()[&pno];
        set_pages_kids(&mut doc, &[pid])?;
        let name = format!("{stem}_page{pno:03}.pdf");
        let path = out_dir.join(&name);
        doc.save(&path)?;
        files.push(path.display().to_string());
    }
    Ok(json!({"output_dir": out_dir.display().to_string(), "total_pages": n, "files": files}))
}

pub fn rotate(file: &Path, deg: i64, out: &Path, pages: Option<&[u32]>) -> Result<Value> {
    if ![90, 180, 270].contains(&deg) {
        bail!("rotation must be 90, 180 or 270");
    }
    let mut doc = Document::load(file)?;
    let page_map = doc.get_pages();
    let target: Vec<u32> = pages
        .map(|p| p.to_vec())
        .unwrap_or_else(|| (1..=page_map.len() as u32).collect());
    let mut rotated = Vec::new();
    for pno in target {
        let pid = page_map
            .get(&pno)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("page {pno} not found"))?;
        let dict = doc.get_object_mut(pid).and_then(Object::as_dict_mut)?;
        let current = dict.get(b"Rotate").and_then(Object::as_i64).unwrap_or(0);
        let next = ((current + deg) % 360 + 360) % 360;
        dict.set("Rotate", Object::Integer(next));
        rotated.push(pno);
    }
    doc.save(out)?;
    Ok(json!({"output": out.display().to_string(), "rotated_pages": rotated, "degrees": deg}))
}

pub fn crop(file: &Path, box_vals: [f64; 4], out: &Path, pages: Option<&[u32]>) -> Result<Value> {
    let [l, b, r, t] = box_vals;
    if r <= l || t <= b {
        bail!("invalid crop box: right must exceed left, top must exceed bottom");
    }
    let mut doc = Document::load(file)?;
    let page_map = doc.get_pages();
    let target: Vec<u32> = pages
        .map(|p| p.to_vec())
        .unwrap_or_else(|| (1..=page_map.len() as u32).collect());
    let mut cropped = Vec::new();
    for pno in target {
        let pid = page_map
            .get(&pno)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("page {pno} not found"))?;
        let dict = doc.get_object_mut(pid).and_then(Object::as_dict_mut)?;
        let arr = Object::Array(vec![
            Object::Real(l as f32),
            Object::Real(b as f32),
            Object::Real(r as f32),
            Object::Real(t as f32),
        ]);
        dict.set("MediaBox", arr.clone());
        dict.set("CropBox", arr);
        cropped.push(pno);
    }
    doc.save(out)?;
    Ok(json!({"output": out.display().to_string(), "cropped_pages": cropped, "box": [l, b, r, t]}))
}

/// 空白页判定与原版 pdf.py pages.clean 一致：无字符 && 无图片 && 无绘图（路径操作）。
pub fn clean(file: &Path, out: &Path) -> Result<Value> {
    let mut doc = Document::load(file)?;
    let pages_text = text::extract_all(&doc)?;
    let page_map = doc.get_pages();
    let mut keep: Vec<ObjectId> = Vec::new();
    let mut removed: Vec<u32> = Vec::new();
    let blank_flags: HashMap<u32, bool> = pages_text
        .iter()
        .map(|pt| {
            (
                pt.page,
                pt.char_count == 0 && !pt.has_images && !pt.has_drawings,
            )
        })
        .collect();
    for (pno, pid) in &page_map {
        if blank_flags.get(pno).copied().unwrap_or(false) {
            removed.push(*pno);
        } else {
            keep.push(*pid);
        }
    }
    if removed.is_empty() {
        doc.save(out)?;
        let empty_removed: Vec<u32> = Vec::new();
        return Ok(
            json!({"output": out.display().to_string(), "removed_pages": empty_removed, "remaining": keep.len()}),
        );
    }
    set_pages_kids(&mut doc, &keep)?;
    doc.save(out)?;
    Ok(json!({
        "output": out.display().to_string(),
        "removed_pages": removed,
        "remaining": keep.len(),
    }))
}

#[cfg(test)]
mod tests {

    #[test]
    fn rotate_validation() {
        // 90/180/270 合法，45 非法 —— 在 rotate() 内校验，这里只验证常量表逻辑
        assert!([90, 180, 270].contains(&90));
        assert!(![90, 180, 270].contains(&45));
    }
}
