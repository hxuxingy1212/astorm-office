//! 回归语料：对 `tests/corpus/*.xlsx` 逐一做 解析 → 生成 → 再解析。
//!
//! 这些样本覆盖真实世界的边界情况（UTF-16 部件、`\` 路径分隔符、根级包、
//! 1904 日期系统、indexed 颜色、twoCellAnchor 图表/图片等），来源见
//! `tests/corpus/SOURCES.md`。

use std::io::Read;
use std::path::{Path, PathBuf};

use json2xlsx::model::Workbook;
use json2xlsx::{generate, parse};
use zip::ZipArchive;

fn corpus_files() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("读取 {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .map(|e| e.eq_ignore_ascii_case("xlsx") || e.eq_ignore_ascii_case("xlsm"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files
}

/// 把 xlsx 内的媒体部件解出到 `dir`，并把工作簿里的图片 `src` 指向解出后的
/// 实际文件——纯 `parse` 不落地媒体，`generate` 需要真实字节（正常流程是
/// unpack→repack）。
fn materialize_media(wb: &mut Workbook, xlsx: &Path, dir: &Path) {
    let has_images = wb.sheets.iter().any(|s| !s.images.is_empty());
    let has_background = wb.sheets.iter().any(|s| s.background.is_some());
    if !has_images && !has_background {
        return;
    }
    let file = std::fs::File::open(xlsx).expect("open xlsx");
    let mut zip = ZipArchive::new(file).expect("zip");
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).expect("zip entry");
        let name = entry.name().replace('\\', "/");
        if !name.starts_with("xl/media/") || entry.is_dir() {
            continue;
        }
        let out = dir.join(&name);
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).unwrap();
        std::fs::write(&out, buf).unwrap();
    }
    for sheet in &mut wb.sheets {
        for img in &mut sheet.images {
            img.src = dir.join(&img.src).to_string_lossy().into_owned();
        }
        if let Some(bg) = sheet.background.clone() {
            sheet.background = Some(dir.join(bg).to_string_lossy().into_owned());
        }
    }
}

#[test]
fn corpus_roundtrips() {
    let files = corpus_files();
    assert!(!files.is_empty(), "tests/corpus 下没有样本");

    let base = std::env::temp_dir().join(format!("json2xlsx_corpus_{}", std::process::id()));
    for (i, f) in files.iter().enumerate() {
        let label = f
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        let mut wb = parse(f).unwrap_or_else(|e| panic!("parse {label}: {e}"));

        let dir = base.join(format!("{i}"));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(&dir);
        materialize_media(&mut wb, f, &dir);

        let out = dir.join("rt.xlsx");
        generate(&wb, &out).unwrap_or_else(|e| panic!("generate {label}: {e}"));

        let again = parse(&out).unwrap_or_else(|e| panic!("reparse {label}: {e}"));
        assert_eq!(
            again.sheets.len(),
            wb.sheets.len(),
            "工作表数量不一致: {label}"
        );
        if wb.theme.is_some() {
            assert!(again.theme.is_some(), "主题未保留: {label}");
        }
        let shapes: usize = wb.sheets.iter().map(|s| s.shapes.len()).sum();
        let shapes2: usize = again.sheets.iter().map(|s| s.shapes.len()).sum();
        assert_eq!(shapes, shapes2, "形状数量不一致: {label}");
        let charts: usize = wb.sheets.iter().map(|s| s.charts.len()).sum();
        let charts2: usize = again.sheets.iter().map(|s| s.charts.len()).sum();
        assert_eq!(charts, charts2, "图表数量不一致: {label}");

        let tables: usize = wb.sheets.iter().map(|s| s.tables.len()).sum();
        let tables2: usize = again.sheets.iter().map(|s| s.tables.len()).sum();
        assert_eq!(tables, tables2, "表格数量不一致: {label}");

        let images: usize = wb.sheets.iter().map(|s| s.images.len()).sum();
        let images2: usize = again.sheets.iter().map(|s| s.images.len()).sum();
        assert_eq!(images, images2, "图片数量不一致: {label}");

        let comments = |w: &Workbook| -> usize {
            w.sheets
                .iter()
                .flat_map(|s| s.rows.iter())
                .flat_map(|r| r.cells.iter())
                .filter(|c| c.comment.is_some())
                .count()
        };
        assert_eq!(comments(&wb), comments(&again), "批注数量不一致: {label}");

        let backgrounds =
            |w: &Workbook| -> usize { w.sheets.iter().filter(|s| s.background.is_some()).count() };
        assert_eq!(
            backgrounds(&wb),
            backgrounds(&again),
            "背景图数量不一致: {label}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    let _ = std::fs::remove_dir_all(&base);
    eprintln!("corpus: {} 个样本全部通过", files.len());
}
