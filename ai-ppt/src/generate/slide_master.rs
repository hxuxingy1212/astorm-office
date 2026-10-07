//! 幻灯片母版和版式生成模块
//!
//! 生成 slideMaster.xml（母版幻灯片）和 slideLayout.xml（空白版式）。
//! 母版定义了幻灯片的全局颜色映射和默认样式。

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// 生成 ppt/slideMasters/slideMaster1.xml
pub fn generate_slide_master() -> String {
    format!(
        "{}<p:sldMaster xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"\n\
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"\n\
         xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\">\n\
         <p:cSld>\n\
         <p:bg>\n\
         <p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef>\n\
         </p:bg>\n\
         <p:spTree>\n\
         <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\n\
         <p:grpSpPr>\n\
         <a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm>\n\
         </p:grpSpPr>\n\
         </p:spTree>\n\
         </p:cSld>\n\
         <p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\"\n\
         accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>\n\
         <p:sldLayoutIdLst>\n\
         <p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/>\n\
         </p:sldLayoutIdLst>\n\
         </p:sldMaster>",
        XML_DECL,
    )
}

pub fn generate_slide_layout() -> String {
    format!(
        "{}<p:sldLayout xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"\n\
         xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"\n\
         xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\"\n\
         type=\"blank\" preserve=\"1\">\n\
         <p:cSld name=\"Blank\">\n\
         <p:spTree>\n\
         <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\n\
         <p:grpSpPr>\n\
         <a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm>\n\
         </p:grpSpPr>\n\
         </p:spTree>\n\
         </p:cSld>\n\
         <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>\n\
         </p:sldLayout>",
        XML_DECL,
    )
}
