//! render（HTML/PNG 预览）契约测试：媒体可加载、列宽/行高生效、数字格式与对齐正确。
//! 语料来自 ai-excel/tests/corpus（真实开源项目夹具）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_json2xlsx"))
}

fn corpus(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/corpus")
        .join(name)
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("json2xlsx_render").join(format!(
        "{}_{}",
        name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 渲染为 HTML，返回 HTML 内容
fn render_html(input: &Path, out: &Path) -> String {
    let r = cli()
        .args([
            "render",
            input.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .expect("CLI 执行失败");
    assert_eq!(
        r.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&r.stderr)
    );
    std::fs::read_to_string(out).unwrap()
}

#[test]
fn images_are_referenced_as_loadable_absolute_paths() {
    let dir = temp_dir("media");
    let html = render_html(
        &corpus("libxlsxwriter__image_twocell.xlsx"),
        &dir.join("p.html"),
    );
    assert!(
        html.contains("file://") && html.contains("/xl/media/image1.png"),
        "图片应解析为可加载的绝对路径:\n{}",
        &html[..html.len().min(600)]
    );
    assert!(
        !html.contains("src=\"xl/media/"),
        "不应保留相对包内路径（否则浏览器 404）"
    );
}

#[test]
fn sheet_background_is_tiled_and_resolved() {
    let dir = temp_dir("bg");
    let html = render_html(
        &corpus("rust_xlsxwriter__background_with_comments.xlsx"),
        &dir.join("p.html"),
    );
    assert!(
        html.contains("background-image") && html.contains("file://"),
        "工作表背景图应平铺且路径已解析"
    );
}

#[test]
fn column_widths_and_row_heights_follow_the_file() {
    let dir = temp_dir("widths");
    let html = render_html(&corpus("poiji__number_format.xlsx"), &dir.join("p.html"));
    // 该文件的列定义：A=8.83 字符、B=15 字符 → 像素宽度不同且 B 更宽
    assert!(html.contains("<colgroup>"), "应输出列宽定义");
    let widths: Vec<u32> = html
        .split("<col style=\"width:")
        .skip(1)
        .filter_map(|s| s.split("px").next())
        .filter_map(|s| s.parse().ok())
        .take(2)
        .collect();
    assert_eq!(widths.len(), 2, "应有两列宽度");
    assert!(
        widths[1] > widths[0],
        "B 列（15 字符）应宽于 A 列: {widths:?}"
    );
    assert!(html.contains("<tr style=\"height:"), "应输出行高");
}

#[test]
fn number_formats_and_default_alignment_match_excel() {
    let dir = temp_dir("numfmt");
    let html = render_html(&corpus("poiji__number_format.xlsx"), &dir.join("p.html"));
    // 会计格式：负数显示为括号
    assert!(html.contains("(50.00)"), "负数应使用会计括号格式");
    assert!(html.contains("(65.00)"));
    // 数字默认右对齐
    assert!(html.contains("text-align:right"), "数字应默认右对齐");
    assert!(!html.contains("-50.00"), "不应退化为减号形式");
}

#[test]
fn png_render_produces_real_image_and_keeps_html() {
    let dir = temp_dir("png");
    let png = dir.join("p.png");
    let r = cli()
        .args([
            "render",
            corpus("libxlsxwriter__image_twocell.xlsx")
                .to_str()
                .unwrap(),
            "--format",
            "png",
            "-o",
            png.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(
        r.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&r.stderr)
    );
    let bytes = std::fs::read(&png).expect("PNG 未生成");
    assert_eq!(&bytes[..4], b"\x89PNG");
    let html = png.with_extension("html");
    assert!(html.exists(), "应保留同名 HTML 预览");
}

#[test]
fn wide_sheets_are_clipped_to_one_page_width() {
    let dir = temp_dir("pagewidth");
    // 该文件 20+ 列、列宽 10 字符 → 一页放不下，应按页宽截断
    let html = render_html(&corpus("exceljs__many_columns.xlsx"), &dir.join("p.html"));
    let cols = html.matches("<col style=\"width:").count();
    assert!(
        (1..20).contains(&cols),
        "应截断到一页宽度（实际保留 {cols} 列）"
    );
    assert!(html.contains("已按一页宽度截断"), "应有截断提示");

    // 窄表不受影响
    let html = render_html(&corpus("poiji__number_format.xlsx"), &dir.join("q.html"));
    assert!(!html.contains("已按一页宽度截断"), "窄表不应截断");
}

#[test]
fn rerender_to_same_path_overwrites_previous_png() {
    // 回归：产物已存在时不得把旧文件当成本次输出（Chrome 轮询判定）
    let dir = temp_dir("rerender");
    let product = dir.join("book");
    let r = cli()
        .args([
            "unpack",
            corpus("poiji__number_format.xlsx").to_str().unwrap(),
            "-o",
            product.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(0));

    let png = dir.join("preview.png");
    let render = || {
        let r = cli()
            .args([
                "render",
                product.to_str().unwrap(),
                "--format",
                "png",
                "-o",
                png.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert_eq!(
            r.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&r.stderr)
        );
    };

    render();
    let first = std::fs::read(&png).unwrap();
    // 改一个单元格后再渲染到同一路径：图像内容必须变化
    let r = cli()
        .args([
            "edit",
            product.to_str().unwrap(),
            "/sheet[1]/cell[A1]",
            "set",
            "--prop",
            "value=RERENDER-CHECK",
        ])
        .output()
        .unwrap();
    assert_eq!(
        r.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&r.stderr)
    );
    render();
    let second = std::fs::read(&png).unwrap();
    assert_ne!(first, second, "第二次渲染应覆盖旧 PNG，而不是复用旧产物");
}
