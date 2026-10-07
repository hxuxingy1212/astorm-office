//! 演示文稿级 XML 生成模块
//!
//! 生成 presentation.xml（幻灯片列表、尺寸）、
//! presProps.xml（演示文稿属性）、viewProps.xml（视图属性）、
//! tableStyles.xml（表格样式）、core.xml（核心文档属性）、app.xml（应用程序属性）。

use crate::model::Presentation;
use crate::utils::constants::inch_to_emu;

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// 生成 ppt/presentation.xml 主文件
pub fn generate(model: &Presentation) -> String {
    let w = inch_to_emu(model.width);
    let h = inch_to_emu(model.height);
    let slide_count = model.slides.len();

    let mut slide_id_list = String::new();
    for i in 0..slide_count {
        slide_id_list.push_str(&format!(
            "    <p:sldId id=\"{}\" r:id=\"rId{}\"/>\n",
            256 + i,
            i + 2
        ));
    }

    // notesMasterIdLst：与 presentation.xml.rels 的 rId 对齐（slide_count + 2）
    let notes_master_rid = slide_count as u32 + 2;
    let notes_master_lst = format!(
        "<p:notesMasterIdLst>\n    <p:notesMasterId r:id=\"rId{}\"/>\n    </p:notesMasterIdLst>\n",
        notes_master_rid
    );

    // notesIdLst：每张有备注的幻灯片一条（id 与幻灯片 sldId 一致）
    let mut notes_id_lst = String::new();
    let mut rid = notes_master_rid + 1;
    let mut notes_entries = Vec::new();
    for (i, slide) in model.slides.iter().enumerate() {
        if slide.notes.is_some() {
            notes_entries.push(format!(
                "    <p:notesId id=\"{}\" r:id=\"rId{}\"/>\n",
                256 + i,
                rid
            ));
            rid += 1;
        }
    }
    if !notes_entries.is_empty() {
        notes_id_lst = format!(
            "<p:notesIdLst>\n{}</p:notesIdLst>\n",
            notes_entries.concat()
        );
    }

    format!(
        "{}<p:presentation xmlns:a=\"{}\" xmlns:r=\"{}\" xmlns:p=\"{}\" saveSubsetFonts=\"1\">\n\
         <p:sldMasterIdLst>\n\
         <p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/>\n\
         </p:sldMasterIdLst>\n\
         {}\
         <p:sldIdLst>\n\
         {}  </p:sldIdLst>\n\
         {}\
         <p:sldSz cx=\"{}\" cy=\"{}\"/>\n\
         <p:notesSz cx=\"{}\" cy=\"{}\"/>\n\
         </p:presentation>",
        XML_DECL,
        "http://schemas.openxmlformats.org/drawingml/2006/main",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        "http://schemas.openxmlformats.org/presentationml/2006/main",
        notes_master_lst,
        slide_id_list,
        notes_id_lst,
        w,
        h,
        inch_to_emu(7.5),
        inch_to_emu(10.0),
    )
}

pub fn generate_pres_props() -> String {
    format!(
        "{}<p:presentationPr xmlns:a=\"{}\" xmlns:r=\"{}\" xmlns:p=\"{}\"/>",
        XML_DECL,
        "http://schemas.openxmlformats.org/drawingml/2006/main",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        "http://schemas.openxmlformats.org/presentationml/2006/main",
    )
}

pub fn generate_view_props() -> String {
    format!(
        "{}<p:viewPr xmlns:a=\"{}\" xmlns:r=\"{}\" xmlns:p=\"{}\">\n\
         <p:normalViewPr><p:restoredLeft sz=\"15620\"/><p:restoredTop sz=\"94660\"/></p:normalViewPr>\n\
         </p:viewPr>",
        XML_DECL,
        "http://schemas.openxmlformats.org/drawingml/2006/main",
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        "http://schemas.openxmlformats.org/presentationml/2006/main",
    )
}

pub fn generate_table_styles() -> String {
    format!(
        "{}<a:tblStyleLst xmlns:a=\"{}\" def=\"{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}\"/>",
        XML_DECL, "http://schemas.openxmlformats.org/drawingml/2006/main",
    )
}

pub fn generate_core_props(meta: Option<&crate::model::Meta>) -> String {
    let m = meta.cloned().unwrap_or_default();
    let now = chrono_now();
    format!(
        "{}<cp:coreProperties xmlns:cp=\"{}\" xmlns:dc=\"{}\" xmlns:dcterms=\"{}\" xmlns:xsi=\"{}\">\n\
         <dc:title>{}</dc:title>\n\
         <dc:creator>{}</dc:creator>\n\
         <cp:lastModifiedBy>{}</cp:lastModifiedBy>\n\
         <dcterms:created xsi:type=\"dcterms:W3CDTF\">{}</dcterms:created>\n\
         <dcterms:modified xsi:type=\"dcterms:W3CDTF\">{}</dcterms:modified>\n\
         <cp:revision>1</cp:revision>\n\
         </cp:coreProperties>",
        XML_DECL,
        "http://schemas.openxmlformats.org/package/2006/metadata/core-properties",
        "http://purl.org/dc/elements/1.1/",
        "http://purl.org/dc/terms/",
        "http://www.w3.org/2001/XMLSchema-instance",
        m.title.as_deref().unwrap_or("Presentation"),
        m.author.as_deref().unwrap_or("json2pptx"),
        m.last_modified_by
            .as_deref()
            .or(m.author.as_deref())
            .unwrap_or("json2pptx"),
        now, now,
    )
}

pub fn generate_app_props(meta: Option<&crate::model::Meta>, slide_count: u32) -> String {
    let m = meta.cloned().unwrap_or_default();
    let company = m
        .company
        .as_deref()
        .map(|c| {
            format!(
                "<Company>{}</Company>\n         ",
                crate::utils::xml::esc_xml(c)
            )
        })
        .unwrap_or_default();
    format!(
        "{}<Properties xmlns=\"{}\">\n\
         <Application>{}</Application>\n\
         {}<Slides>{}</Slides>\n\
         <ScaleCrop>false</ScaleCrop>\n\
         <LinksUpToDate>false</LinksUpToDate>\n\
         <SharedDoc>false</SharedDoc>\n\
         <HyperlinksChanged>false</HyperlinksChanged>\n\
         </Properties>",
        XML_DECL,
        "http://schemas.openxmlformats.org/officeDocument/2006/extended-properties",
        crate::utils::xml::esc_xml(m.application.as_deref().unwrap_or("json2pptx")),
        company,
        slide_count,
    )
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs();
    let days = secs / 86400;
    let time = secs % 86400;
    let year = 1970 + (days as f64 / 365.25) as u64;
    format!(
        "{}-01-01T{:02}:{:02}:{:02}Z",
        year,
        time / 3600,
        (time % 3600) / 60,
        time % 60
    )
}
