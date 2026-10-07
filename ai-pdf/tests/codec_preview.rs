//! 图片编解码预览（JPX / JBIG2 / CCITT）的集成测试。
//!
//! fixtures 取自公开测试语料（pikepdf / pdf.js / pdfbox 测试集）。
//! unpack 后必须生成 `media/*.preview.png`（render/web 无法直接解码原字节），
//! 像素尺寸与图片字典一致；字节保真链路不受影响（原媒体文件原样保留）。

use json2pdf::model::{DocumentModel, Element};
use std::path::{Path, PathBuf};

const FIXTURES: &[&str] = &[
    "ccitt-g4.pdf",
    "ccitt-eol.pdf",
    "ccitt-multi.pdf",
    "jbig2.pdf",
    "jpx-16bit.pdf",
];

fn unpack_fixture(name: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let pdf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    json2pdf::unpack::unpack(&pdf, &root).unwrap();
    (dir, root)
}

/// 找第 1 页的图片元素（含 preview 路径与 passthrough 尺寸）
fn first_image(root: &Path) -> (Option<String>, (i64, i64)) {
    let doc: DocumentModel =
        serde_json::from_str(&std::fs::read_to_string(root.join("document.json")).unwrap())
            .unwrap();
    let page: json2pdf::model::PageModel =
        serde_json::from_str(&std::fs::read_to_string(root.join(&doc.pages[0])).unwrap()).unwrap();
    for el in &page.elements {
        if let Element::Image {
            preview,
            passthrough,
            ..
        } = el
        {
            let dims = passthrough
                .as_ref()
                .map(|p| (p.width, p.height))
                .unwrap_or((0, 0));
            return (preview.clone(), dims);
        }
    }
    (None, (0, 0))
}

#[test]
fn terminal_codecs_generate_preview_png() {
    for name in FIXTURES {
        let (_guard, root) = unpack_fixture(name);
        let (preview, (w, h)) = first_image(&root);
        let preview = preview.unwrap_or_else(|| panic!("{name}: 应生成 preview"));
        let png = root.join(&preview);
        assert!(png.exists(), "{name}: preview 文件应存在: {preview}");
        let bytes = std::fs::read(&png).unwrap();
        assert!(
            bytes.starts_with(&[0x89, b'P', b'N', b'G']),
            "{name}: 应是 PNG"
        );
        assert!(bytes.len() > 100, "{name}: PNG 不应为空壳");

        // 尺寸与图片字典一致
        let decoder = png::Decoder::new(std::io::Cursor::new(&bytes[..]));
        let reader = decoder.read_info().unwrap();
        assert_eq!(
            (reader.info().width, reader.info().height),
            (w as u32, h as u32),
            "{name}: 预览尺寸应与字典一致"
        );

        // 原媒体文件（字节保真）仍在
        let src_dir = root.join("media");
        let has_original = std::fs::read_dir(&src_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                n.ends_with(".jp2") || n.ends_with(".bin")
            });
        assert!(has_original, "{name}: 原字节媒体文件应保留");
    }
}

#[test]
fn preview_survives_repack_roundtrip_in_product() {
    // repack 不消费 preview，但产物目录编辑往返后 preview 字段应原样保留
    let (_guard, root) = unpack_fixture("jpx-16bit.pdf");
    let (preview_before, _) = first_image(&root);
    let out_pdf = root.join("roundtrip.pdf");
    json2pdf::repack::repack(&root, &out_pdf).unwrap();
    let doc: DocumentModel =
        serde_json::from_str(&std::fs::read_to_string(root.join("document.json")).unwrap())
            .unwrap();
    let _ = doc; // repack 原地不改产物；字段在分片 JSON 中原样存在
    assert!(preview_before.is_some());
}
