//! 内容类型文件生成模块
//!
//! 生成 [Content_Types].xml 文件，声明 OOXML 包中所有部件的 MIME 类型。

use crate::utils::constants::content_types;

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// 生成 [Content_Types].xml
pub fn generate(
    slide_count: u32,
    images: &[String],
    notes_indexes: &[usize],
    chart_count: u32,
    comment_count: u32,
    has_comment_authors: bool,
    opaque_overrides: &[(String, String)],
) -> String {
    let mut xml = String::from(XML_DECL);
    xml.push_str(
        "<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n",
    );
    xml.push_str("  <Default Extension=\"xml\" ContentType=\"application/xml\"/>\n");
    xml.push_str("  <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n");
    xml.push_str("  <Default Extension=\"png\" ContentType=\"image/png\"/>\n");
    xml.push_str("  <Default Extension=\"jpeg\" ContentType=\"image/jpeg\"/>\n");
    xml.push_str("  <Default Extension=\"jpg\" ContentType=\"image/jpeg\"/>\n");
    xml.push_str("  <Default Extension=\"gif\" ContentType=\"image/gif\"/>\n");
    xml.push_str("  <Default Extension=\"svg\" ContentType=\"image/svg+xml\"/>\n");
    xml.push_str("  <Default Extension=\"bmp\" ContentType=\"image/bmp\"/>\n");
    xml.push_str("  <Default Extension=\"webp\" ContentType=\"image/webp\"/>\n");
    xml.push_str("  <Default Extension=\"mp4\" ContentType=\"video/mp4\"/>\n");
    xml.push_str("  <Default Extension=\"m4v\" ContentType=\"video/mp4\"/>\n");
    xml.push_str("  <Default Extension=\"mov\" ContentType=\"video/quicktime\"/>\n");
    xml.push_str("  <Default Extension=\"avi\" ContentType=\"video/avi\"/>\n");
    xml.push_str("  <Default Extension=\"wmv\" ContentType=\"video/x-ms-wmv\"/>\n");
    xml.push_str("  <Default Extension=\"mp3\" ContentType=\"audio/mpeg\"/>\n");
    xml.push_str("  <Default Extension=\"m4a\" ContentType=\"audio/mp4\"/>\n");
    xml.push_str("  <Default Extension=\"wav\" ContentType=\"audio/wav\"/>\n");
    xml.push_str("  <Default Extension=\"wma\" ContentType=\"audio/x-ms-wma\"/>\n");
    xml.push_str(&format!(
        "  <Override PartName=\"/ppt/presentation.xml\" ContentType=\"{}\"/>\n",
        content_types::PRESENTATION
    ));
    xml.push_str(&format!(
        "  <Override PartName=\"/ppt/presProps.xml\" ContentType=\"{}\"/>\n",
        content_types::PRES_PROPS
    ));
    xml.push_str(&format!(
        "  <Override PartName=\"/ppt/viewProps.xml\" ContentType=\"{}\"/>\n",
        content_types::VIEW_PROPS
    ));
    xml.push_str(&format!(
        "  <Override PartName=\"/ppt/tableStyles.xml\" ContentType=\"{}\"/>\n",
        content_types::TABLE_STYLES
    ));
    xml.push_str(&format!(
        "  <Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"{}\"/>\n",
        content_types::THEME
    ));
    xml.push_str(&format!(
        "  <Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"{}\"/>\n",
        content_types::SLIDE_MASTER
    ));
    xml.push_str(&format!(
        "  <Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"{}\"/>\n",
        content_types::SLIDE_LAYOUT
    ));
    xml.push_str(&format!(
        "  <Override PartName=\"/ppt/notesMasters/notesMaster1.xml\" ContentType=\"{}\"/>\n",
        content_types::NOTES_MASTER
    ));
    for &i in notes_indexes {
        xml.push_str(&format!(
            "  <Override PartName=\"/ppt/notesSlides/notesSlide{}.xml\" ContentType=\"{}\"/>\n",
            i + 1,
            content_types::NOTES_SLIDE
        ));
    }
    xml.push_str(&format!(
        "  <Override PartName=\"/docProps/core.xml\" ContentType=\"{}\"/>\n",
        content_types::CORE_PROPS
    ));
    xml.push_str(&format!(
        "  <Override PartName=\"/docProps/app.xml\" ContentType=\"{}\"/>\n",
        content_types::EXT_PROPS
    ));
    for i in 1..=slide_count {
        xml.push_str(&format!(
            "  <Override PartName=\"/ppt/slides/slide{}.xml\" ContentType=\"{}\"/>\n",
            i,
            content_types::SLIDE
        ));
    }
    for i in 1..=chart_count {
        xml.push_str(&format!(
            "  <Override PartName=\"/ppt/charts/chart{}.xml\" ContentType=\"{}\"/>\n",
            i,
            content_types::CHART
        ));
    }
    for i in 1..=comment_count {
        xml.push_str(&format!(
            "  <Override PartName=\"/ppt/comments/comment{}.xml\" ContentType=\"{}\"/>\n",
            i,
            content_types::COMMENT
        ));
    }
    if has_comment_authors {
        xml.push_str(&format!(
            "  <Override PartName=\"/ppt/commentAuthors.xml\" ContentType=\"{}\"/>\n",
            content_types::COMMENT_AUTHORS
        ));
    }
    // 不透明部件的内容类型覆盖
    for (path, ct) in opaque_overrides {
        xml.push_str(&format!(
            "  <Override PartName=\"/{}\" ContentType=\"{}\"/>\n",
            path, ct
        ));
    }
    for img in images {
        let ext = std::path::Path::new(img)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png")
            .to_lowercase();
        if ext == "png"
            || ext == "jpeg"
            || ext == "jpg"
            || ext == "gif"
            || ext == "svg"
            || ext == "bmp"
            || ext == "webp"
        {
            continue;
        }
        let mime = match ext.as_str() {
            "tiff" | "tif" => "image/tiff",
            "ico" => "image/x-icon",
            _ => "image/octet-stream",
        };
        xml.push_str(&format!(
            "  <Default Extension=\"{}\" ContentType=\"{}\"/>\n",
            ext, mime
        ));
    }
    xml.push_str("</Types>");
    xml
}
