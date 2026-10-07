//! Shared constants for XLSX OOXML generation/parsing.

pub const NS_MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
pub const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
pub const NS_PKG_REL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
pub const NS_CONTENT_TYPES: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
pub const NS_CP: &str = "http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
pub const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
pub const NS_DCTERMS: &str = "http://purl.org/dc/terms/";
pub const NS_XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
pub const NS_EP: &str = "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties";

/// Relationship types.
pub const REL_OFFICE_DOCUMENT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
pub const REL_CORE_PROPS: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";
pub const REL_EXTENDED_PROPS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties";
pub const REL_WORKSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet";
pub const REL_CHARTSHEET: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chartsheet";
pub const REL_STYLES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles";
pub const REL_SHARED_STRINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings";
pub const REL_THEME: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
pub const REL_DRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing";
pub const REL_IMAGE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
pub const REL_CHART: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart";
pub const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
pub const REL_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/table";
pub const REL_PIVOT_TABLE: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotTable";
pub const REL_PIVOT_CACHE_DEF: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheDefinition";
pub const REL_PIVOT_CACHE_REC: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/pivotCacheRecords";
pub const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
pub const REL_VMLDRAWING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/vmlDrawing";

/// Content types.
pub const CT_WORKBOOK: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml";
pub const CT_WORKSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml";
pub const CT_STYLES: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml";
pub const CT_SHARED_STRINGS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml";
pub const CT_DRAWING: &str = "application/vnd.openxmlformats-officedocument.drawing+xml";
pub const CT_CHARTSHEET: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.chartsheet+xml";
pub const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
pub const CT_CHART: &str = "application/vnd.openxmlformats-officedocument.drawingml.chart+xml";
pub const CT_TABLE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.table+xml";
pub const CT_PIVOT_TABLE: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotTable+xml";
pub const CT_PIVOT_CACHE_DEF: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheDefinition+xml";
pub const CT_PIVOT_CACHE_REC: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.pivotCacheRecords+xml";
pub const CT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.spreadsheetml.comments+xml";
pub const CT_VML: &str = "application/vnd.openxmlformats-officedocument.vmlDrawing";
pub const CT_CORE_PROPS: &str = "application/vnd.openxmlformats-package.core-properties+xml";
pub const CT_EXTENDED_PROPS: &str =
    "application/vnd.openxmlformats-officedocument.extended-properties+xml";
pub const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
pub const CT_XML: &str = "application/xml";

// Slicer (pivot-backed).
pub const REL_SLICER_CACHE: &str =
    "http://schemas.microsoft.com/office/2007/relationships/slicerCache";
pub const REL_SLICERS: &str = "http://schemas.microsoft.com/office/2007/relationships/slicer";
pub const CT_SLICER_CACHE: &str = "application/vnd.ms-excel.slicerCache+xml";
pub const CT_SLICERS: &str = "application/vnd.ms-excel.slicer+xml";
pub const CT_CHART_EX: &str = "application/vnd.ms-office.chartex+xml";
pub const REL_OLE_OBJECT: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/oleObject";
pub const CT_OLE: &str = "application/vnd.openxmlformats-officedocument.oleObject";
