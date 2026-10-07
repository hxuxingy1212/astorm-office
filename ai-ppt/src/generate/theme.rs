//! 主题 XML 生成模块
//!
//! 生成 ppt/theme/theme1.xml 文件，定义颜色方案和字体方案。
//! 颜色方案包含 dk1/dk2, lt1/lt2, accent1~6, hlink, folHlink 共 12 种颜色。

use crate::model::Theme;

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// 仅接受 6 位十六进制；否则回退默认值（避免 windowText 等系统色名污染 srgbClr）
fn sanitize(v: &str, default: &str) -> String {
    let h = v.trim_start_matches('#');
    if h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()) {
        h.to_uppercase()
    } else {
        default.to_string()
    }
}

/// 生成 ppt/theme/theme1.xml
pub fn generate(theme: Option<&Theme>) -> String {
    let colors = theme.and_then(|t| t.colors.as_ref());
    let get = |key: &str, default: &str| -> String {
        sanitize(
            colors
                .and_then(|c| c.get(key))
                .map(String::as_str)
                .unwrap_or(default),
            default,
        )
    };
    let dk1 = get("dk1", "000000");
    let lt1 = get("lt1", "FFFFFF");
    let dk2 = get("dk2", "44546A");
    let lt2 = get("lt2", "E7E6E6");
    let a1 = get("accent1", "4472C4");
    let a2 = get("accent2", "ED7D31");
    let a3 = get("accent3", "A5A5A5");
    let a4 = get("accent4", "FFC000");
    let a5 = get("accent5", "5B9BD5");
    let a6 = get("accent6", "70AD47");
    let hlink = get("hlink", "0563C1");
    let fol = get("folHlink", "954F72");

    let major = theme
        .and_then(|t| t.major_font.as_deref())
        .unwrap_or("Calibri Light");
    let minor = theme
        .and_then(|t| t.minor_font.as_deref())
        .unwrap_or("Calibri");

    format!(
        "{}<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"json2pptx Theme\">\n\
         <a:themeElements>\n\
         <a:clrScheme name=\"json2pptx\">\n\
         <a:dk1><a:srgbClr val=\"{}\"/></a:dk1>\n\
         <a:lt1><a:srgbClr val=\"{}\"/></a:lt1>\n\
         <a:dk2><a:srgbClr val=\"{}\"/></a:dk2>\n\
         <a:lt2><a:srgbClr val=\"{}\"/></a:lt2>\n\
         <a:accent1><a:srgbClr val=\"{}\"/></a:accent1>\n\
         <a:accent2><a:srgbClr val=\"{}\"/></a:accent2>\n\
         <a:accent3><a:srgbClr val=\"{}\"/></a:accent3>\n\
         <a:accent4><a:srgbClr val=\"{}\"/></a:accent4>\n\
         <a:accent5><a:srgbClr val=\"{}\"/></a:accent5>\n\
         <a:accent6><a:srgbClr val=\"{}\"/></a:accent6>\n\
         <a:hlink><a:srgbClr val=\"{}\"/></a:hlink>\n\
         <a:folHlink><a:srgbClr val=\"{}\"/></a:folHlink>\n\
         </a:clrScheme>\n\
         <a:fontScheme name=\"json2pptx\">\n\
         <a:majorFont><a:latin typeface=\"{}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont>\n\
         <a:minorFont><a:latin typeface=\"{}\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont>\n\
         </a:fontScheme>\n\
         <a:fmtScheme name=\"json2pptx\">\n\
         <a:fillStyleLst>\n\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\n\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\n\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\n\
         </a:fillStyleLst>\n\
         <a:lnStyleLst>\n\
         <a:ln w=\"6350\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>\n\
         <a:ln w=\"12700\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>\n\
         <a:ln w=\"19050\" cap=\"flat\" cmpd=\"sng\" algn=\"ctr\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln>\n\
         </a:lnStyleLst>\n\
         <a:effectStyleLst>\n\
         <a:effectStyle><a:effectLst/></a:effectStyle>\n\
         <a:effectStyle><a:effectLst/></a:effectStyle>\n\
         <a:effectStyle><a:effectLst/></a:effectStyle>\n\
         </a:effectStyleLst>\n\
         <a:bgFillStyleLst>\n\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\n\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\n\
         <a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill>\n\
         </a:bgFillStyleLst>\n\
         </a:fmtScheme>\n\
         </a:themeElements>\n\
         <a:objectDefaults/>\n\
         <a:extraClrSchemeLst/>\n\
         </a:theme>",
        XML_DECL, dk1, lt1, dk2, lt2, a1, a2, a3, a4, a5, a6, hlink, fol, major, minor,
    )
}
