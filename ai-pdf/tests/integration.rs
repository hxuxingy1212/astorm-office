//! 端到端集成测试：手工构造最小合法 PDF（含正确 xref 偏移），
//! 验证文本提取、元数据、页操作（rotate/split/merge/clean）全链路。

use json2pdf::{checks, meta, page_ops, qa, text};
use lopdf::Document;
use serde_json::json;

/// 构造一个 N 页 PDF，每页给一段内容流（用 Helvetica /F1）。
fn build_pdf(pages_content: &[&str]) -> Vec<u8> {
    let mut objs: Vec<(u32, String)> = Vec::new(); // (obj_num, body)
    let n = pages_content.len();

    // 1: catalog
    objs.push((1, "<< /Type /Catalog /Pages 2 0 R >>".into()));
    // 2: pages
    let kids: Vec<String> = (0..n)
        .map(|i| format!("{} 0 R", 3 + i as u32 * 2))
        .collect();
    objs.push((
        2,
        format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")),
    ));

    let font_num = 3 + n as u32 * 2;
    for (i, content) in pages_content.iter().enumerate() {
        let page_num = 3 + i as u32 * 2;
        let content_num = page_num + 1;
        objs.push((
            page_num,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                 /Resources << /Font << /F1 {font_num} 0 R >> >> /Contents {content_num} 0 R >>"
            ),
        ));
        objs.push((
            content_num,
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        ));
    }
    objs.push((
        font_num,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
    ));

    // 组装 + xref
    let mut out = String::from("%PDF-1.7\n");
    let mut offsets: Vec<(u32, u64)> = Vec::new();
    for (num, body) in &objs {
        offsets.push((*num, out.len() as u64));
        out.push_str(&format!("{num} 0 obj\n{body}\nendobj\n"));
    }
    let max = objs.iter().map(|(n, _)| *n).max().unwrap_or(0);
    let xref_start = out.len();
    out.push_str(&format!("xref\n0 {}\n", max + 1));
    out.push_str("0000000000 65535 f \n");
    for num in 1..=max {
        let off = offsets
            .iter()
            .find(|(n, _)| n == &num)
            .map(|(_, o)| *o)
            .unwrap_or(0);
        out.push_str(&format!("{off:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_start}\n%%EOF",
        max + 1
    ));
    out.into_bytes()
}

fn fixture_pair() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.pdf");
    let b = dir.path().join("b.pdf");
    std::fs::write(
        &a,
        build_pdf(&[
            "BT /F1 12 Tf 72 720 Td (Hello World) Tj 0 -20 Td (Second line) Tj ET",
            "", // 空白页（clean 测试用）
        ]),
    )
    .unwrap();
    std::fs::write(
        &b,
        build_pdf(&["BT /F1 12 Tf 72 700 Td (Page B one) Tj ET", "q Q"]),
    )
    .unwrap();
    (dir, a, b)
}

#[test]
fn extract_text_two_lines() {
    let (dir, a, _) = fixture_pair();
    let doc = Document::load(&a).unwrap();
    let pts = text::extract_all(&doc).unwrap();
    assert_eq!(pts.len(), 2);
    let page1 = &pts[0];
    assert_eq!(page1.char_count, 22);
    let t = page1.text();
    assert!(t.contains("Hello World"), "got: {t:?}");
    assert!(t.contains("Second line"), "got: {t:?}");
    // 第二页空白
    assert_eq!(pts[1].char_count, 0);
    assert!(!pts[1].has_drawings);
    let _ = dir;
}

#[test]
fn meta_get_and_set_roundtrip() {
    let (dir, a, _) = fixture_pair();
    let got = meta::get(&a).unwrap();
    assert_eq!(got["pages"], 2);
    assert_eq!(got["pdf_version"], "1.7");
    assert_eq!(got["first_page_size"]["width"], 612.0);

    let out = dir.path().join("meta.pdf");
    let mut data = serde_json::Map::new();
    data.insert("Title".into(), serde_json::json!("集成测试 标题"));
    data.insert("Author".into(), serde_json::json!("Z.ai"));
    meta::set(&a, &out, &data).unwrap();

    let re = meta::get(&out).unwrap();
    assert_eq!(re["info"]["Title"], "集成测试 标题");
    assert_eq!(re["info"]["Author"], "Z.ai");
}

#[test]
fn brand_writes_producer() {
    let (_dir, a, _) = fixture_pair();
    let v = meta::brand(std::slice::from_ref(&a), None, Some("T")).unwrap();
    assert_eq!(v["branded"], 1);
    let got = meta::get(&a).unwrap();
    assert_eq!(got["info"]["Producer"], "http://z.ai");
    assert_eq!(got["info"]["Title"], "T");
}

#[test]
fn rotate_and_crop() {
    let (dir, a, _) = fixture_pair();
    let out = dir.path().join("rot.pdf");
    let v = page_ops::rotate(&a, 90, &out, Some(&[1])).unwrap();
    assert_eq!(v["rotated_pages"], json!([1]));
    let doc = Document::load(&out).unwrap();
    let pid = *doc.get_pages().get(&1).unwrap();
    let rot = doc
        .get_dictionary(pid)
        .unwrap()
        .get(b"Rotate")
        .and_then(lopdf::Object::as_i64)
        .unwrap();
    assert_eq!(rot, 90);

    let out2 = dir.path().join("crop.pdf");
    page_ops::crop(&a, [10.0, 20.0, 300.0, 400.0], &out2, None).unwrap();
    let doc2 = Document::load(&out2).unwrap();
    let pid2 = *doc2.get_pages().get(&1).unwrap();
    let mb = doc2
        .get_dictionary(pid2)
        .unwrap()
        .get(b"MediaBox")
        .and_then(lopdf::Object::as_array)
        .unwrap();
    assert_eq!(mb.len(), 4);
}

#[test]
fn split_and_merge_roundtrip() {
    let (dir, a, b) = fixture_pair();
    // split
    let split_dir = dir.path().join("split");
    let v = page_ops::split(&a, &split_dir).unwrap();
    assert_eq!(v["total_pages"], 2);
    let files: Vec<String> = serde_json::from_value(v["files"].clone()).unwrap();
    assert_eq!(files.len(), 2);
    for f in &files {
        let doc = Document::load(f).unwrap();
        assert_eq!(doc.get_pages().len(), 1);
    }
    // 拆出的第 1 页文本仍可提取
    let doc = Document::load(&files[0]).unwrap();
    let pts = text::extract_all(&doc).unwrap();
    assert!(pts[0].text().contains("Hello World"));

    // merge
    let merged = dir.path().join("merged.pdf");
    let mv = page_ops::merge(&[a.clone(), b.clone()], &merged).unwrap();
    assert_eq!(mv["total_pages"], 4);
    let doc = Document::load(&merged).unwrap();
    assert_eq!(doc.get_pages().len(), 4);
    let pts = text::extract_all(&doc).unwrap();
    assert!(
        pts[0].text().contains("Hello World"),
        "page1 after merge: {:?}",
        pts[0].text()
    );
    assert!(
        pts[2].text().contains("Page B one"),
        "page3 after merge: {:?}",
        pts[2].text()
    );
    // 合并后页尺寸保留
    let pid = *doc.get_pages().get(&1).unwrap();
    let (w, h, _) = page_ops::page_mediabox(&doc, pid).unwrap();
    assert_eq!((w as i64, h as i64), (612, 792));
}

#[test]
fn clean_removes_blank_page() {
    let (dir, a, _) = fixture_pair();
    let out = dir.path().join("cleaned.pdf");
    let v = page_ops::clean(&a, &out).unwrap();
    assert_eq!(v["removed_pages"], json!([2]));
    let doc = Document::load(&out).unwrap();
    assert_eq!(doc.get_pages().len(), 1);
    let pts = text::extract_all(&doc).unwrap();
    assert!(pts[0].text().contains("Hello World"));
}

#[test]
fn font_check_clean_fixture() {
    let (_dir, a, _) = fixture_pair();
    let (v, code) = checks::font_check(&a).unwrap();
    assert_eq!(code, 0);
    assert_eq!(v["clean"], true);
}

#[test]
fn toc_check_detects_missing_toc() {
    let (_dir, a, _) = fixture_pair();
    let (v, code) = checks::toc_check(&a).unwrap();
    assert_eq!(code, 1);
    assert_eq!(v["status"], "fail");
    assert!(v["issues"][0]["code"] == "TOC_NOT_FOUND");
}

#[test]
fn toc_check_passes_valid_toc() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("toc.pdf");
    std::fs::write(
        &f,
        build_pdf(&[
            // 第 1 页：封面（TOC 不应在第 1 页）
            "BT /F1 24 Tf 200 700 Td (Annual Report) Tj ET",
            // 第 2 页：目录
            "BT /F1 14 Tf 72 720 Td (Contents) Tj ET",
            "BT /F1 12 Tf 72 680 Td (Introduction .......... 1) Tj 0 -20 Td (Methods .......... 5) Tj 0 -20 Td (Results .......... 9) Tj ET",
            // 第 3 页：正文
            "BT /F1 12 Tf 72 720 Td (Introduction body) Tj ET",
        ]),
    )
    .unwrap();
    let (v, code) = checks::toc_check(&f).unwrap();
    assert_eq!(code, 0, "issues: {}", v["issues"]);
    assert_eq!(v["status"], "pass");
    let entries: &serde_json::Value = &v["entries"];
    assert!(entries
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["title"] == "Introduction" && e["page"] == 1));
}

#[test]
fn qa_fails_on_blank_page() {
    let (_dir, a, _b) = fixture_pair();
    let (v, code) = qa::qa(std::slice::from_ref(&a), false).unwrap();
    // a.pdf 第 2 页是空白页 → blank_page fail → 退出码 1
    assert_eq!(code, 1);
    assert_eq!(v["overall"], "fail");
    let file_report = &v["files"][0];
    assert_eq!(file_report["status"], "fail");
    let fails: &Vec<serde_json::Value> = file_report["fails"].as_array().unwrap();
    assert!(
        fails
            .iter()
            .any(|f| f["check"] == "blank_page" && f["page"] == 2),
        "fails: {fails:?}"
    );
}

// ── OCR 引擎链 ─────────────────────────────────────────────────────────────

#[test]
fn ocr_engine_chain_prefers_local_native_over_tesseract() {
    // 显式 URL → HTTP 服务（liteparse 规范）
    let e = json2pdf::ocr::engine(Some("http://127.0.0.1:9"), None).unwrap();
    assert_eq!(e.name(), "http");

    // 无显式配置时：macOS 优先系统 Vision（tesseract 只在其不可用时兜底）
    let saved = std::env::var("UNIOCR_URL").ok();
    std::env::remove_var("UNIOCR_URL");
    let e = json2pdf::ocr::engine(None, None).unwrap();
    let expected = if cfg!(target_os = "macos") {
        "vision"
    } else {
        "tesseract"
    };
    assert_eq!(e.name(), expected);
    if let Some(v) = saved {
        std::env::set_var("UNIOCR_URL", v);
    }
}

#[test]
fn ocr_vision_engine_recognizes_native_page() {
    // 端到端：真实扫描页位图（tests/assets/ocr-page-1.png，ccitt-g4 渲染产物）
    // → 引擎（尊重探测结果）→ 非空行 + 内容抽查
    let png = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/assets/ocr-page-1.png"
    ))
    .expect("OCR e2e 资产缺失（tests/assets/ocr-page-1.png）");
    if !cfg!(target_os = "macos") {
        return; // 非 mac 无 Vision，链路兜底由 tesseract/HTTP 用例覆盖
    }
    std::env::remove_var("UNIOCR_URL");
    let e = json2pdf::ocr::engine(None, None).unwrap();
    let lines = e.recognize(&png, "eng").expect("Vision recognize");
    assert!(!lines.is_empty(), "Vision 对扫描页应识别出行");
    assert!(lines
        .iter()
        .all(|l| l.bbox.1 >= 0.0 && l.bbox.3 >= l.bbox.1));
    // 内容抽查：该页是 LinnSequencer 扫描页，标题必须被认出
    let joined = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        joined.contains("LinnSequencer"),
        "识别结果异常: {:.120}",
        joined
    );
}

// ── 跨格式转换（convert docx/pptx/xlsx，产物目录为中间形态）───────────────

const CROSS_PAGE: &str = concat!(
    "BT /F1 22 Tf 72 740 Td (Quarterly Report) Tj ET\n",
    "BT /F1 12 Tf 72 700 Td (Intro paragraph line one) Tj ET\n",
    "BT /F1 12 Tf 72 680 Td (and line two continues here) Tj ET\n",
    "BT /F1 12 Tf 72 650 Td (- Bullet one) Tj ET\n",
    "BT /F1 12 Tf 72 630 Td (- Bullet two) Tj ET\n",
    "BT /F1 12 Tf 72 610 Td (1. Ordered one) Tj ET\n",
    "BT /F1 12 Tf 72 590 Td (2. Ordered two) Tj ET\n",
    "BT /F1 12 Tf 130 560 Td (Name) Tj 142 0 Td (Qty) Tj 60 0 Td (Price) Tj ET\n",
    "BT /F1 12 Tf 130 540 Td (Apple) Tj 165 0 Td (12) Tj 66 0 Td (3.50) Tj ET\n",
    "BT /F1 12 Tf 130 520 Td (Pear) Tj 174 0 Td (8) Tj 66 0 Td (2.25) Tj ET\n",
);

fn fixture_cross_pdf(dir: &std::path::Path) -> std::path::PathBuf {
    let pdf = dir.join("cross.pdf");
    std::fs::write(
        &pdf,
        build_pdf(&[CROSS_PAGE, "BT /F1 12 Tf 72 700 Td (Page two) Tj ET"]),
    )
    .unwrap();
    pdf
}

#[test]
fn cross_pdf_to_docx_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = fixture_cross_pdf(dir.path());
    let src = json2pdf::cross::open_source(&pdf).unwrap();
    let doc = json2pdf::cross::to_docx(&src).unwrap();
    let out = dir.path().join("out.docx");
    json2docx::generate(&doc, &out.display().to_string()).unwrap();

    let parsed = json2docx::parse(&out.display().to_string()).unwrap();
    let headings: Vec<&String> = parsed
        .all_blocks()
        .filter_map(|b| match b {
            json2docx::model::blocks::Block::Heading { text, .. } => Some(text),
            _ => None,
        })
        .collect();
    assert!(
        headings.iter().any(|t| t.contains("Quarterly Report")),
        "应识别出标题: {headings:?}"
    );
    let tables = parsed
        .all_blocks()
        .filter(|b| matches!(b, json2docx::model::blocks::Block::Table(_)))
        .count();
    assert_eq!(tables, 1, "应识别出一张表格");
    let cells: Vec<String> = parsed
        .all_blocks()
        .filter_map(|b| match b {
            json2docx::model::blocks::Block::Table(t) => Some(t.rows[0].cells[0].blocks.clone()),
            _ => None,
        })
        .flatten()
        .filter_map(|b| match b {
            json2docx::model::blocks::Block::Paragraph(p) => p.text.clone(),
            _ => None,
        })
        .collect();
    assert!(
        cells.iter().any(|c| c.contains("Name")),
        "表头应有 Name: {cells:?}"
    );
    // 列表 + 分页符 + 元信息
    let lists = parsed
        .all_blocks()
        .filter(|b| matches!(b, json2docx::model::blocks::Block::List(_)))
        .count();
    assert!(lists >= 1, "应有 bullet 列表");
    let breaks = parsed
        .all_blocks()
        .filter(|b| matches!(b, json2docx::model::blocks::Block::PageBreak))
        .count();
    assert_eq!(breaks, 1, "两页 PDF → 1 个 PageBreak");
}

#[test]
fn cross_pdf_to_pptx_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = fixture_cross_pdf(dir.path());
    let src = json2pdf::cross::open_source(&pdf).unwrap();
    let pres = json2pdf::cross::to_pptx(&src).unwrap();
    assert_eq!(pres.slides.len(), 2);
    // 近无损：字号/坐标搬移
    let s0 = &pres.slides[0];
    let texts: Vec<&json2pptx::model::elements::TextElement> = s0
        .elements
        .iter()
        .filter_map(|e| match e {
            json2pptx::model::Element::Text(t) => Some(t),
            _ => None,
        })
        .collect();
    assert!(
        texts.iter().any(|t| match &t.text {
            json2pptx::model::elements::TextContent::Simple(s) => s.contains("Quarterly Report"),
            _ => false,
        }),
        "标题文本应存在"
    );
    let out = dir.path().join("out.pptx");
    json2pptx::generate(&pres, &out.display().to_string()).unwrap();
    let re = json2pptx::parse(&out.display().to_string()).unwrap();
    assert_eq!(re.slides.len(), 2);
}

#[test]
fn cross_pdf_to_xlsx_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = fixture_cross_pdf(dir.path());
    let src = json2pdf::cross::open_source(&pdf).unwrap();
    let wb = json2pdf::cross::to_xlsx(&src).unwrap();
    let table_sheet = wb
        .sheets
        .iter()
        .find(|s| s.name.starts_with("P1T"))
        .expect("应有表格 sheet");
    let first_row = &table_sheet.rows[0];
    let vals: Vec<String> = first_row
        .cells
        .iter()
        .map(|c| match c.value.as_ref().unwrap() {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect();
    assert!(
        vals.iter().any(|v| v.contains("Name")),
        "表头应有 Name: {vals:?}"
    );
    // 数值识别：第二行 Qty 列应为 number 12
    let nums: Vec<serde_json::Value> = table_sheet.rows[1]
        .cells
        .iter()
        .map(|c| c.value.clone().unwrap())
        .collect();
    assert!(
        nums.iter()
            .any(|v| v.is_number() && v.as_f64() == Some(12.0)),
        "12 应转成数值: {nums:?}"
    );
    let out = dir.path().join("out.xlsx");
    json2xlsx::generate(&wb, &out).unwrap();
    let re = json2xlsx::parse(&out).unwrap();
    assert!(re.sheets.iter().any(|s| s.name.starts_with("P1T")));
}

#[test]
fn cross_docx_to_pdf_native_roundtrip() {
    // docx（由 cross_pdf_to_docx_roundtrip 的产物链生成）→ native 引擎 → PDF
    let dir = tempfile::tempdir().unwrap();
    let pdf = fixture_cross_pdf(dir.path());
    let src = json2pdf::cross::open_source(&pdf).unwrap();
    let doc = json2pdf::cross::to_docx(&src).unwrap();
    let docx = dir.path().join("gen.docx");
    json2docx::generate(&doc, &docx.display().to_string()).unwrap();

    let out = dir.path().join("gen.pdf");
    let pages = json2pdf::cross::office_to_pdf(&docx, &out).unwrap();
    assert_eq!(pages, 2);

    let re = lopdf::Document::load(&out).unwrap();
    let pts = text::extract_all(&re).unwrap();
    let all: String = pts.iter().map(|p| p.text()).collect();
    assert!(all.contains("Quarterly Report"), "标题应保留: {all:.120}");
    assert!(all.contains("Name") && all.contains("Qty"), "表头应保留");
    assert!(all.contains("Page two"), "第二页应在");
}
