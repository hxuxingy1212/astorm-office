//! 演讲者备注部件生成模块
//!
//! 生成 notesMaster（备注母版）与 notesSlide（单页备注）XML。
//! 备注幻灯片通过 presentation.xml 的 notesIdLst 与幻灯片关联。

use crate::model::Slide;
use crate::utils::xml::esc_xml;

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// 有备注的幻灯片索引列表（与幻灯片编号对应）
pub fn notes_slide_indexes(slides: &[Slide]) -> Vec<usize> {
    slides
        .iter()
        .enumerate()
        .filter(|(_, s)| s.notes.is_some())
        .map(|(i, _)| i)
        .collect()
}

/// 生成 ppt/notesMasters/notesMaster1.xml
pub fn generate_notes_master() -> String {
    format!(
        "{}<p:notesMaster xmlns:a=\"{}\" xmlns:r=\"{}\" xmlns:p=\"{}\">\n\
         <p:cSld>\n\
         <p:spTree>\n\
         <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\n\
         <p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/>\n\
         <a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>\n\
         </p:spTree>\n\
         </p:cSld>\n\
         <p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>\n\
         <p:notesStyle>\n\
         <a:lvl1pPr marL=\"0\" indent=\"0\" algn=\"l\"><a:defRPr sz=\"1200\"/></a:lvl1pPr>\n\
         </p:notesStyle>\n\
         </p:notesMaster>",
        XML_DECL,
        "http://schemas.openxmlformats.org/drawingml/2006/main",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        "http://schemas.openxmlformats.org/presentationml/2006/main",
    )
}

/// 生成 ppt/notesSlides/notesSlideN.xml
///
/// 结构：p:notes > p:cSld > p:spTree 内一个 body 占位文本框，逐行一个段落。
pub fn generate_notes_slide(slide_idx: usize, notes: &str) -> String {
    let id = 256 + slide_idx as u32;
    let mut paras = String::new();
    for line in notes.lines() {
        paras.push_str(&format!(
            "<a:p><a:r><a:rPr lang=\"en-US\" sz=\"1200\"/><a:t>{}</a:t></a:r></a:p>\n",
            esc_xml(line)
        ));
    }

    format!(
        "{}<p:notes xmlns:a=\"{}\" xmlns:r=\"{}\" xmlns:p=\"{}\">\n\
         <p:cSld>\n\
         <p:spTree>\n\
         <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\n\
         <p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/>\n\
         <a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>\n\
         <p:sp>\n\
         <p:nvSpPr>\n\
         <p:cNvPr id=\"2\" name=\"Notes Placeholder {}\"/>\n\
         <p:cNvSpPr/>\n\
         <p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr>\n\
         </p:nvSpPr>\n\
         <p:spPr/>\n\
         <p:txBody>\n\
         <a:bodyPr/>\n\
         <a:lstStyle/>\n\
         {}\
         </p:txBody>\n\
         </p:sp>\n\
         </p:spTree>\n\
         </p:cSld>\n\
         <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>\n\
         </p:notes>",
        XML_DECL,
        "http://schemas.openxmlformats.org/drawingml/2006/main",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        "http://schemas.openxmlformats.org/presentationml/2006/main",
        id,
        paras,
    )
}
