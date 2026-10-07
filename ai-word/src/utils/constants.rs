//! OOXML 命名空间与常量

/// WordprocessingML 主命名空间
pub const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
/// 关系命名空间
pub const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
/// 包关系命名空间
pub const NS_PR: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
/// 内容类型命名空间
pub const NS_CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
/// DrawingML 主命名空间
pub const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
/// WordprocessingDrawing
pub const NS_WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
/// DrawingML Picture
pub const NS_PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
/// Office Math (OMML)
pub const NS_M: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";
/// Word 2010 扩展
pub const NS_W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
/// core properties 命名空间
pub const NS_CP: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
/// Dublin Core
pub const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
/// DCTERMS
pub const NS_DCTERMS: &str = "http://purl.org/dc/terms/";
/// XSI
pub const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
/// app 扩展属性
pub const NS_EP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";

/// 关系类型
pub const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub const REL_CORE_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
pub const REL_EXTENDED_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
pub const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
pub const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
pub const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
pub const REL_FONT_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable";
pub const REL_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
pub const REL_WEB_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/webSettings";
pub const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
pub const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
pub const REL_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
pub const REL_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
pub const REL_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
pub const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
pub const REL_CHART: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
pub const REL_OLE_OBJECT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/oleObject";

/// 文档主部件内容类型
pub const CT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
pub const CT_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
pub const CT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
pub const CT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
pub const CT_FONT_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml";
pub const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
pub const CT_WEB_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.webSettings+xml";
pub const CT_CORE: &str = "application/vnd.openxmlformats-package.core-properties+xml";
pub const CT_APP: &str = "application/vnd.openxmlformats-officedocument.extended-properties+xml";
pub const CT_HEADER: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
pub const CT_FOOTER: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
pub const CT_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
pub const CT_ENDNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
pub const CT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
pub const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
