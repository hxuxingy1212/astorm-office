//! 针对真实语料暴露的结构做回归：内容控件 / 域 / 嵌套表格 sdt

use json2docx::model::blocks::Block;
use std::io::Write;

/// 用给定 document.xml 造一个最小 docx
fn make_docx(document_xml: &str, path: &str) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    zip.start_file("word/document.xml", opts).unwrap();
    zip.write_all(document_xml.as_bytes()).unwrap();
    zip.finish().unwrap();
}

fn tmp(name: &str) -> String {
    std::env::temp_dir()
        .join(format!("json2docx-corpus-{}-{}", std::process::id(), name))
        .to_string_lossy()
        .to_string()
}

const NS: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
                  xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
                  xmlns:v=\"urn:schemas-microsoft-com:vml\" \
                  xmlns:o=\"urn:schemas-microsoft-com:office:office\"";

#[test]
fn content_controls_and_fields() {
    let xml = format!(
        "<w:document {NS}><w:body>\
         <w:sdt><w:sdtContent><w:p><w:r><w:t>控件文本</w:t></w:r></w:p></w:sdtContent></w:sdt>\
         <w:tbl><w:sdt><w:sdtContent><w:tr><w:tc><w:p><w:r><w:t>格子</w:t></w:r></w:p></w:tc></w:tr></w:sdtContent></w:sdt></w:tbl>\
         <w:p><w:fldSimple w:instr=\"DATE\"><w:r><w:t>2020-01-01</w:t></w:r></w:fldSimple></w:p>\
         <w:p><w:r><w:t>普通段落</w:t></w:r></w:p>\
         </w:body></w:document>"
    );
    let path = tmp("sdt.docx");
    make_docx(&xml, &path);

    let doc = json2docx::parse(&path).expect("parse");
    let types: Vec<&str> = doc.all_blocks().map(|b| b.type_name()).collect();
    assert!(types.contains(&"paragraph"), "应有段落");
    assert!(types.contains(&"table"), "应有表格");

    let text: String = doc
        .all_blocks()
        .map(|b| b.plain_text())
        .collect::<Vec<_>>()
        .join("|");
    assert!(text.contains("控件文本"), "内容控件文本丢失: {text}");
    assert!(text.contains("格子"), "表格单元格 sdt 文本丢失: {text}");
    assert!(text.contains("2020-01-01"), "fldSimple 文本丢失: {text}");
    assert!(text.contains("普通段落"));

    // 表格确实解析出单元格
    let table = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .unwrap();
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.rows[0].cells.len(), 1);
}

#[test]
fn vml_textbox_and_shape() {
    let xml = format!(
        "<w:document {NS}><w:body>\
         <w:p><w:r><w:pict>\
           <v:shape id=\"tb1\" type=\"#_x0000_t202\" \
             style=\"position:absolute;margin-left:10pt;margin-top:20pt;width:100pt;height:30pt\" \
             fillcolor=\"#FFF2CC\">\
             <v:textbox><w:txbxContent><w:p><w:r><w:t>框内文字</w:t></w:r></w:p></w:txbxContent></v:textbox>\
           </v:shape>\
         </w:pict></w:r></w:p>\
         <w:p><w:r><w:pict>\
           <v:line from=\"0,0\" to=\"100,50\" style=\"position:absolute\">\
             <v:stroke color=\"#000000\" weight=\"2pt\" endarrow=\"block\"/>\
           </v:line>\
         </w:pict></w:r></w:p>\
         </w:body></w:document>"
    );
    let path = tmp("vml.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let types: Vec<&str> = doc.all_blocks().map(|b| b.type_name()).collect();
    assert!(types.contains(&"textbox"), "应解析出文本框: {types:?}");
    assert!(types.contains(&"shape"), "应解析出形状/线条: {types:?}");

    let tb = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::TextBox(t) => Some(t),
            _ => None,
        })
        .unwrap();
    assert_eq!(tb.text, "框内文字");
    assert_eq!(tb.fill.as_deref(), Some("FFF2CC"));
    assert_eq!(tb.width, Some(100.0));
    assert_eq!(tb.vml, Some(true));

    let sh = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Shape(s) => Some(s),
            _ => None,
        })
        .unwrap();
    assert_eq!(sh.shape_type, "line");
    assert_eq!(sh.width, Some(100.0));
    assert_eq!(sh.height, Some(50.0));

    // 生成 → 再解析，形状/文本框保留
    let out = tmp("vml_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let doc2 = json2docx::parse(&out).expect("parse2");
    let t2: Vec<&str> = doc2.all_blocks().map(|b| b.type_name()).collect();
    assert!(t2.contains(&"shape"), "回环应保留形状: {t2:?}");
}

#[test]
fn table_fidelity() {
    let xml = format!(
        "<w:document {NS}><w:body><w:tbl>\
         <w:tblPr><w:tblStyle w:val=\"LightShading\"/>\
           <w:tblBorders>\
             <w:top w:val=\"double\" w:sz=\"12\" w:space=\"0\" w:color=\"C00000\"/>\
             <w:left w:val=\"double\" w:sz=\"12\" w:space=\"0\" w:color=\"C00000\"/>\
             <w:bottom w:val=\"double\" w:sz=\"12\" w:space=\"0\" w:color=\"C00000\"/>\
             <w:right w:val=\"double\" w:sz=\"12\" w:space=\"0\" w:color=\"C00000\"/>\
           </w:tblBorders>\
           <w:tblInd w:w=\"200\" w:type=\"dxa\"/></w:tblPr>\
         <w:tblGrid><w:gridCol w:w=\"2000\"/><w:gridCol w:w=\"3000\"/></w:tblGrid>\
         <w:tr><w:trPr><w:trHeight w:val=\"600\" w:hRule=\"exact\"/></w:trPr>\
           <w:tc><w:tcPr><w:vAlign w:val=\"center\"/>\
             <w:tcBorders><w:top w:val=\"none\" w:sz=\"0\" w:color=\"auto\"/></w:tcBorders>\
             <w:tcMar><w:top w:w=\"100\" w:type=\"dxa\"/></w:tcMar></w:tcPr>\
             <w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>\
           <w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc>\
         </w:tr></w:tbl></w:body></w:document>"
    );
    let path = tmp("table.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let t = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table");
    assert_eq!(t.style.as_deref(), Some("LightShading"));
    let bd = t.border.as_ref().expect("border");
    assert_eq!(bd.style.as_deref(), Some("double"));
    assert_eq!(bd.color.as_deref(), Some("C00000"));
    assert_eq!(bd.width, Some(1.5));
    assert_eq!(t.indent, Some(10.0));
    assert_eq!(t.rows[0].height, Some(30.0));
    assert_eq!(t.rows[0].height_rule.as_deref(), Some("exact"));
    assert_eq!(t.rows[0].cells[0].valign.as_deref(), Some("center"));
    assert_eq!(t.rows[0].cells[0].margin, Some(5.0));
    assert_eq!(
        t.rows[0].cells[0]
            .border
            .as_ref()
            .and_then(|b| b.style.as_deref()),
        Some("none")
    );

    // 生成 → 再解析，属性保留
    let out = tmp("table_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let doc2 = json2docx::parse(&out).expect("parse2");
    let t2 = doc2
        .all_blocks()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .unwrap();
    assert_eq!(t2.style.as_deref(), Some("LightShading"));
    assert_eq!(t2.border.as_ref().and_then(|b| b.width), Some(1.5));
    assert_eq!(t2.rows[0].height, Some(30.0));
    assert_eq!(t2.rows[0].cells[0].valign.as_deref(), Some("center"));
}

#[test]
fn run_symbol_spacing_caps() {
    let xml = format!(
        "<w:document {NS}><w:body><w:p>\
         <w:r><w:rPr><w:caps/><w:smallCaps/><w:spacing w:val=\"40\"/></w:rPr>\
           <w:t>abc</w:t></w:r>\
         <w:r><w:sym w:font=\"Wingdings\" w:char=\"F0B7\"/></w:r>\
         </w:p></w:body></w:document>"
    );
    let path = tmp("runprops.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let runs: Vec<json2docx::model::Run> = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Paragraph(p) => Some(p.runs.clone().unwrap_or_default()),
            _ => None,
        })
        .expect("paragraph");
    let caps_run = runs
        .iter()
        .find(|r| r.text.contains("abc"))
        .expect("caps run");
    assert_eq!(caps_run.caps, Some(true));
    assert_eq!(caps_run.small_caps, Some(true));
    assert_eq!(caps_run.spacing, Some(2.0));
    assert!(runs
        .iter()
        .any(|r| r.symbol.as_deref() == Some("Wingdings:F0B7")));

    let out = tmp("runprops_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let xml2 = read_part(&out, "word/document.xml");
    assert!(xml2.contains("w:sym"), "应保留符号: {xml2}");
    assert!(xml2.contains("w:caps") && xml2.contains("w:smallCaps"));
}

fn read_part(path: &str, part: &str) -> String {
    use std::io::Read;
    let f = std::fs::File::open(path).unwrap();
    let mut z = zip::ZipArchive::new(f).unwrap();
    let mut s = String::new();
    z.by_name(part).unwrap().read_to_string(&mut s).unwrap();
    s
}

#[test]
fn page_border_roundtrip() {
    let xml = format!(
        "<w:document {NS}><w:body><w:p><w:r><w:t>hi</w:t></w:r></w:p>\
         <w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/>\
           <w:pgBorders w:offsetFrom=\"page\">\
             <w:top w:val=\"double\" w:sz=\"24\" w:space=\"24\" w:color=\"C00000\"/>\
             <w:left w:val=\"double\" w:sz=\"24\" w:space=\"24\" w:color=\"C00000\"/>\
             <w:bottom w:val=\"double\" w:sz=\"24\" w:space=\"24\" w:color=\"C00000\"/>\
             <w:right w:val=\"double\" w:sz=\"24\" w:space=\"24\" w:color=\"C00000\"/>\
           </w:pgBorders>\
         </w:sectPr></w:body></w:document>"
    );
    let path = tmp("pageborder.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let mut pages: Vec<&json2docx::model::PageSetup> = vec![&doc.page];
    for p in &doc.parts {
        if let Some(s) = &p.section {
            if let Some(pg) = &s.page {
                pages.push(pg);
            }
        }
    }
    let pb = pages
        .iter()
        .find_map(|p| p.page_border.as_ref())
        .expect("page border");
    assert_eq!(pb.style.as_deref(), Some("double"));
    assert_eq!(pb.color.as_deref(), Some("C00000"));
    assert_eq!(pb.width, Some(3.0));

    let out = tmp("pageborder_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let xml2 = read_part(&out, "word/document.xml");
    assert!(xml2.contains("w:pgBorders"), "应保留页面边框: {xml2}");
}

fn make_docx_multi(parts: &[(&str, &str)], path: &str) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    for (name, content) in parts {
        zip.start_file(*name, opts).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

#[test]
fn list_numbering_fidelity() {
    let document = format!(
        "<w:document {NS}><w:body>\
         <w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"5\"/></w:numPr></w:pPr>\
           <w:r><w:t>第一</w:t></w:r></w:p>\
         <w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"5\"/></w:numPr></w:pPr>\
           <w:r><w:t>第二</w:t></w:r></w:p>\
         </w:body></w:document>"
    );
    let numbering = format!(
        "<w:numbering {NS}>\
         <w:abstractNum w:abstractNumId=\"7\"><w:multiLevelType w:val=\"hybridMultilevel\"/>\
           <w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"lowerLetter\"/>\
             <w:lvlText w:val=\"%1)\"/><w:lvlJc w:val=\"left\"/></w:lvl>\
         </w:abstractNum>\
         <w:num w:numId=\"5\"><w:abstractNumId w:val=\"7\"/></w:num>\
         </w:numbering>"
    );
    let path = tmp("numbering.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/numbering.xml", &numbering),
        ],
        &path,
    );
    let doc = json2docx::parse(&path).expect("parse");
    let list = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::List(l) => Some(l),
            _ => None,
        })
        .expect("list");
    assert!(list.ordered);
    assert_eq!(list.items[0].num_format.as_deref(), Some("lowerLetter"));
    assert_eq!(list.items[0].lvl_text.as_deref(), Some("%1)"));

    let out = tmp("numbering_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let num_xml = read_part(&out, "word/numbering.xml");
    assert!(
        num_xml.contains("w:val=\"lowerLetter\""),
        "应保留编号格式: {num_xml}"
    );
    assert!(
        num_xml.contains("w:lvlText w:val=\"%1)\""),
        "应保留编号模板: {num_xml}"
    );
    let doc_xml = read_part(&out, "word/document.xml");
    assert!(
        doc_xml.contains("w:numId w:val=\"3\""),
        "应引用自定义 numId: {doc_xml}"
    );
}

#[test]
fn smartart_text_preserved() {
    let document = format!(
        "<w:document {NS} xmlns:dgm=\"http://schemas.openxmlformats.org/drawingml/2006/diagram\">\
         <w:body><w:p><w:r><w:drawing><wp:inline>\
           <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/diagram\">\
             <dgm:relIds r:dm=\"rId9\" r:lo=\"rId10\" r:qs=\"rId11\" r:cs=\"rId12\"/>\
           </a:graphicData></a:graphic>\
         </wp:inline></w:drawing></w:r></w:p></w:body></w:document>"
    );
    let rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId9\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramData\" Target=\"diagrams/data1.xml\"/>\
       </Relationships>";
    let data =
        "<dgm:dataModel xmlns:dgm=\"http://schemas.openxmlformats.org/drawingml/2006/diagram\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">\
         <dgm:ptLst>\
           <dgm:pt><dgm:t><a:t>流程A</a:t></dgm:t></dgm:pt>\
           <dgm:pt><dgm:t><a:t>流程B</a:t></dgm:t></dgm:pt>\
         </dgm:ptLst></dgm:dataModel>";
    let path = tmp("smartart.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/_rels/document.xml.rels", rels),
            ("word/diagrams/data1.xml", data),
        ],
        &path,
    );
    let doc = json2docx::parse(&path).expect("parse");
    // SmartArt 现以原始 drawing XML 保留（含关系），而非降级为文本
    let raw = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Raw(r) => Some(r),
            _ => None,
        })
        .expect("raw block");
    assert!(
        raw.xml.contains("dgm:relIds"),
        "应保留 drawing XML: {}",
        raw.xml
    );
    assert!(!raw.rels.is_empty(), "应保留关系");
    // 目录部件被透传
    assert!(
        doc.passthrough
            .iter()
            .any(|p| p.path == "word/diagrams/data1.xml"),
        "应透传 diagrams 部件"
    );
    let out = tmp("smartart_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(
        dx.contains("dgm:relIds") && dx.contains("graphicData"),
        "应输出 SmartArt: {dx}"
    );
    assert!(dx.contains("xmlns:dgm="), "应声明 dgm 命名空间");
    let rels_out = read_part(&out, "word/_rels/document.xml.rels");
    assert!(
        rels_out.contains("diagramData"),
        "应重建 diagram 关系: {rels_out}"
    );
    let _ = read_part(&out, "word/diagrams/data1.xml");
}

#[test]
fn ruby_roundtrip() {
    let xml = format!(
        "<w:document {NS}><w:body><w:p><w:r><w:ruby>\
         <w:rubyPr><w:rubyAlign w:val=\"distributeSpace\"/></w:rubyPr>\
         <w:rt><w:r><w:t>とうきょう</w:t></w:r></w:rt>\
         <w:rubyBase><w:r><w:t>東京</w:t></w:r></w:rubyBase>\
         </w:ruby></w:r></w:p></w:body></w:document>"
    );
    let path = tmp("ruby.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let run = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Paragraph(p) => p.runs.as_ref().and_then(|rs| rs.first().cloned()),
            _ => None,
        })
        .expect("run");
    assert_eq!(run.text, "東京");
    assert_eq!(run.ruby.as_deref(), Some("とうきょう"));

    let out = tmp("ruby_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(dx.contains("w:ruby"), "应保留注音: {dx}");
    assert!(dx.contains("とうきょう") && dx.contains("東京"));
}

#[test]
fn drop_cap_roundtrip() {
    let xml = format!(
        "<w:document {NS}><w:body><w:p><w:pPr>\
         <w:framePr w:dropCap=\"drop\" w:lines=\"3\" w:wrap=\"around\"/></w:pPr>\
         <w:r><w:rPr><w:sz w:val=\"96\"/></w:rPr><w:t>T</w:t></w:r>\
         <w:r><w:t>he quick brown fox</w:t></w:r></w:p></w:body></w:document>"
    );
    let path = tmp("dropcap.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let para = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Paragraph(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(para.drop_cap, Some(3));
    let out = tmp("dropcap_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(dx.contains("w:dropCap=\"drop\""), "应保留首字下沉: {dx}");
}

#[test]
fn run_text_effects_roundtrip() {
    let xml = format!(
        "<w:document {NS}><w:body><w:p>\
         <w:r><w:rPr><w:outline/><w:shadow/><w:emboss/><w:imprint/><w:caps/></w:rPr>\
           <w:t>Effect</w:t></w:r></w:p></w:body></w:document>"
    );
    let path = tmp("effects.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let run = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Paragraph(p) => p.runs.as_ref().and_then(|rs| rs.first().cloned()),
            _ => None,
        })
        .expect("run");
    assert_eq!(run.outline, Some(true));
    assert_eq!(run.shadow, Some(true));
    assert_eq!(run.emboss, Some(true));
    assert_eq!(run.imprint, Some(true));
    let out = tmp("effects_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    for tag in ["w:outline", "w:shadow", "w:emboss", "w:imprint"] {
        assert!(dx.contains(tag), "缺少 {tag}: {dx}");
    }
}

#[test]
fn page_background_roundtrip() {
    let xml = format!(
        "<w:document {NS}><w:background w:color=\"D9E2F3\"/><w:body>\
         <w:p><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"
    );
    let path = tmp("bg.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    assert_eq!(doc.background.as_deref(), Some("D9E2F3"));
    let out = tmp("bg_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(
        dx.contains("<w:background w:color=\"D9E2F3\"/>"),
        "应保留背景: {dx}"
    );
    let sx = read_part(&out, "word/settings.xml");
    assert!(
        sx.contains("w:displayBackgroundShape"),
        "应启用背景显示: {sx}"
    );
}

#[test]
fn wordart_text_preserved() {
    let xml = format!(
        "<w:document {NS}><w:body><w:p><w:r><w:pict>\
         <v:shape id=\"wa\" type=\"#_x0000_t136\" style=\"width:200pt;height:60pt\">\
           <v:textpath style=\"font-family:&quot;宋体&quot;\" string=\"标题艺术字\"/>\
         </v:shape></w:pict></w:r></w:p></w:body></w:document>"
    );
    let path = tmp("wordart.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let sh = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Shape(s) => Some(s),
            _ => None,
        })
        .expect("shape");
    assert_eq!(sh.text.as_deref(), Some("标题艺术字"));
}

#[test]
fn continuous_section_type() {
    let xml = format!(
        "<w:document {NS}><w:body>\
         <w:p><w:r><w:t>a</w:t></w:r></w:p>\
         <w:p><w:pPr><w:sectPr><w:type w:val=\"continuous\"/>\
           <w:pgSz w:w=\"11906\" w:h=\"16838\"/></w:sectPr></w:pPr>\
           <w:r><w:t>b</w:t></w:r></w:p>\
         <w:p><w:r><w:t>c</w:t></w:r></w:p></w:body></w:document>"
    );
    let path = tmp("sect.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let st = doc
        .parts
        .iter()
        .filter_map(|p| p.section.as_ref())
        .find_map(|s| s.section_type.clone());
    assert_eq!(st.as_deref(), Some("continuous"));
    let out = tmp("sect_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(
        dx.contains("<w:type w:val=\"continuous\"/>"),
        "应保留分节类型: {dx}"
    );
}

#[test]
fn doc_defaults_roundtrip() {
    let document = format!(
        "<w:document {NS}><w:body><w:p><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"
    );
    let styles = format!(
        "<w:styles {NS}><w:docDefaults>\
         <w:rPrDefault><w:rPr><w:sz w:val=\"22\"/><w:szCs w:val=\"22\"/></w:rPr></w:rPrDefault>\
         <w:pPrDefault><w:pPr><w:spacing w:after=\"200\" w:line=\"276\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault>\
         </w:docDefaults>\
         <w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>\
         </w:styles>"
    );
    let path = tmp("defaults.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/styles.xml", &styles),
        ],
        &path,
    );
    let doc = json2docx::parse(&path).expect("parse");
    let d = doc.defaults.as_ref().expect("defaults");
    assert_eq!(d.font_size, Some(11.0));
    assert_eq!(d.space_after, Some(10.0));
    assert!((d.line_spacing.unwrap() - 1.15).abs() < 0.01);
    let out = tmp("defaults_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let sx = read_part(&out, "word/styles.xml");
    assert!(sx.contains("w:after=\"200\""), "应保留默认段后距: {sx}");
    assert!(sx.contains("w:line=\"276\""), "应保留默认行距: {sx}");
}

#[test]
fn vml_image_parsed() {
    let document = format!(
        "<w:document {NS}><w:body><w:p><w:r><w:pict>\
         <v:shape id=\"i1\" type=\"#_x0000_t75\" style=\"width:200pt;height:100pt\">\
           <v:imagedata r:id=\"rId7\" o:title=\"pic\"/>\
         </v:shape></w:pict></w:r></w:p></w:body></w:document>"
    );
    let rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId7\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"media/image1.png\"/>\
       </Relationships>";
    let path = tmp("vmimg.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/_rels/document.xml.rels", rels),
            ("word/media/image1.png", "PNG"),
        ],
        &path,
    );
    let doc = json2docx::parse(&path).expect("parse");
    let img = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Image(i) => Some(i),
            _ => None,
        })
        .expect("image");
    assert_eq!(img.src, "word/media/image1.png");
    assert_eq!(img.width, Some(200.0));
    assert_eq!(img.height, Some(100.0));
}

#[test]
fn style_basedon_indent_roundtrip() {
    let document = format!(
        "<w:document {NS}><w:body><w:p><w:pPr><w:pStyle w:val=\"MyList\"/></w:pPr><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"
    );
    let styles = format!(
        "<w:styles {NS}>\
         <w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/></w:style>\
         <w:style w:type=\"paragraph\" w:styleId=\"MyList\"><w:name w:val=\"My List\"/>\
           <w:basedOn w:val=\"Normal\"/><w:pPr><w:ind w:left=\"720\" w:hanging=\"360\"/>\
           <w:contextualSpacing/></w:pPr></w:style>\
         </w:styles>"
    );
    let path = tmp("styledef.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/styles.xml", &styles),
        ],
        &path,
    );
    let doc = json2docx::parse(&path).expect("parse");
    let st = doc.styles.get("MyList").expect("style");
    assert_eq!(st.indent, Some(36.0));
    assert_eq!(st.hanging_indent, Some(18.0));
    assert_eq!(st.contextual_spacing, Some(true));
    let out = tmp("styledef_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let sx = read_part(&out, "word/styles.xml");
    assert!(sx.contains("w:styleId=\"MyList\""), "应保留样式: {sx}");
    assert!(
        sx.contains("w:left=\"720\"") && sx.contains("w:hanging=\"360\""),
        "应保留缩进: {sx}"
    );
    assert!(
        sx.contains("w:contextualSpacing"),
        "应保留 contextualSpacing: {sx}"
    );
}

#[test]
fn toc_cached_entries() {
    let xml = format!(
        "<w:document {NS}><w:body>\
         <w:p><w:pPr><w:pStyle w:val=\"TOCHeading\"/></w:pPr><w:r><w:t>Contents</w:t></w:r></w:p>\
         <w:p><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>\
              <w:r><w:instrText xml:space=\"preserve\"> TOC \\o \"1-3\" </w:instrText></w:r>\
              <w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>\
              <w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>\
         <w:p><w:pPr><w:pStyle w:val=\"TOC1\"/></w:pPr><w:r><w:t>第一章</w:t></w:r></w:p>\
         <w:p><w:pPr><w:pStyle w:val=\"TOC2\"/></w:pPr><w:r><w:t>1.1 小节</w:t></w:r></w:p>\
         <w:p><w:r><w:t>正文</w:t></w:r></w:p>\
         </w:body></w:document>"
    );
    let path = tmp("toc.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let toc = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Toc(t) => Some(t),
            _ => None,
        })
        .expect("toc");
    assert_eq!(toc.entries.len(), 2);
    assert_eq!(toc.entries[0].level, 1);
    assert_eq!(toc.entries[1].text, "1.1 小节");
    let out = tmp("toc_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(dx.contains("w:styleId=\"TOC1\"") || dx.contains("w:pStyle w:val=\"TOC1\""));
    assert!(dx.contains("第一章") && dx.contains("1.1 小节"));
}

#[test]
fn content_controls_sdt() {
    let xml = format!(
        "<w:document {NS}><w:body>\
         <w:sdt><w:sdtPr><w:alias w:val=\"MyBlock\"/><w:tag w:val=\"t1\"/><w:dropDownList>\
           <w:listItem w:displayText=\"甲\" w:value=\"a\"/><w:listItem w:displayText=\"乙\" w:value=\"b\"/>\
         </w:dropDownList></w:sdtPr>\
           <w:sdtContent><w:p><w:r><w:t>块内容</w:t></w:r></w:p></w:sdtContent></w:sdt>\
         <w:p><w:sdt><w:sdtPr><w:alias w:val=\"Inline\"/><w:text/></w:sdtPr>\
           <w:sdtContent><w:r><w:t>行内控件文本</w:t></w:r></w:sdtContent></w:sdt></w:p>\
         </w:body></w:document>"
    );
    let path = tmp("sdt2.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    // 块级 SDT
    let sdt = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Sdt(c) => Some(c),
            _ => None,
        })
        .expect("sdt block");
    assert_eq!(sdt.props.alias.as_deref(), Some("MyBlock"));
    assert_eq!(sdt.props.sdt_type.as_deref(), Some("dropDownList"));
    assert_eq!(sdt.props.items.len(), 2);
    // 行内 SDT 属性落到 run
    let inline = doc.all_blocks().find_map(|b| match b {
        Block::Paragraph(p) => p
            .runs
            .as_ref()
            .and_then(|rs| rs.iter().find(|r| r.sdt.is_some()).cloned()),
        _ => None,
    });
    assert!(inline.is_some(), "应为 run 保留行内 sdt 属性");

    let out = tmp("sdt2_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(dx.contains("<w:sdt"), "应生成 sdt: {dx}");
    assert!(dx.contains("w:alias w:val=\"MyBlock\"") && dx.contains("<w:dropDownList>"));
    assert!(dx.contains("行内控件文本"));
}

#[test]
fn generic_fields() {
    let xml = format!(
        "<w:document {NS}><w:body>\
         <w:p><w:r><w:fldChar w:fldCharType=\"begin\"/></w:r>\
           <w:r><w:instrText xml:space=\"preserve\"> DOCPROPERTY Title </w:instrText></w:r>\
           <w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>\
           <w:r><w:t>My Title</w:t></w:r>\
           <w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>\
         <w:p><w:fldSimple w:instr=\" STYLEREF 1 \"><w:r><w:t>Chapter 1</w:t></w:r></w:fldSimple></w:p>\
         </w:body></w:document>"
    );
    let path = tmp("fields.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let fields: Vec<String> = doc
        .all_blocks()
        .flat_map(|b| match b {
            Block::Paragraph(p) => p.runs.clone().unwrap_or_default(),
            _ => vec![],
        })
        .filter_map(|r| r.field)
        .collect();
    assert!(
        fields.iter().any(|f| f.contains("DOCPROPERTY")),
        "fields={fields:?}"
    );
    assert!(
        fields.iter().any(|f| f.contains("STYLEREF")),
        "fields={fields:?}"
    );
    let out = tmp("fields_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(
        dx.contains("DOCPROPERTY Title") && dx.contains("STYLEREF 1"),
        "应回写字段: {dx}"
    );
    assert!(dx.contains("fldCharType=\"begin\""));
}

#[test]
fn numbering_start_override() {
    let document = format!(
        "<w:document {NS}><w:body>\
         <w:p><w:pPr><w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"5\"/></w:numPr></w:pPr>\
           <w:r><w:t>第五项</w:t></w:r></w:p>\
         </w:body></w:document>"
    );
    let numbering = format!(
        "<w:numbering {NS}>\
         <w:abstractNum w:abstractNumId=\"3\"><w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/>\
           <w:numFmt w:val=\"decimal\"/><w:lvlText w:val=\"%1.\"/></w:lvl></w:abstractNum>\
         <w:num w:numId=\"5\"><w:abstractNumId w:val=\"3\"/>\
           <w:lvlOverride w:ilvl=\"0\"><w:startOverride w:val=\"5\"/></w:lvlOverride></w:num>\
         </w:numbering>"
    );
    let path = tmp("startov.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/numbering.xml", &numbering),
        ],
        &path,
    );
    let doc = json2docx::parse(&path).expect("parse");
    let item = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::List(l) => l.items.first().cloned(),
            _ => None,
        })
        .expect("list item");
    assert_eq!(item.start, Some(5), "应保留 startOverride");
    let out = tmp("startov_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let nx = read_part(&out, "word/numbering.xml");
    assert!(nx.contains("<w:start w:val=\"5\"/>"), "应回写起始值: {nx}");
}

#[test]
fn rtl_and_language() {
    let xml = format!(
        "<w:document {NS}><w:body>\
         <w:p><w:pPr><w:bidi/></w:pPr><w:r><w:rPr>\
           <w:rtl/><w:lang w:val=\"ar-SA\" w:eastAsia=\"zh-CN\"/>\
         </w:rPr><w:t>مرحبا</w:t></w:r></w:p>\
         </w:body></w:document>"
    );
    let path = tmp("rtl.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let para = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Paragraph(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(para.rtl, Some(true));
    let run = para.runs.as_ref().unwrap().first().cloned().unwrap();
    assert_eq!(run.rtl, Some(true));
    assert_eq!(run.lang.as_deref(), Some("ar-SA"));
    assert_eq!(run.lang_ea.as_deref(), Some("zh-CN"));
    let out = tmp("rtl_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(
        dx.contains("<w:bidi/>") && dx.contains("<w:rtl/>"),
        "应保留 RTL: {dx}"
    );
    assert!(dx.contains("ar-SA") && dx.contains("zh-CN"));
}

#[test]
fn form_fields() {
    let xml = format!(
        "<w:document {NS}><w:body><w:p>\
         <w:r><w:fldChar w:fldCharType=\"begin\"><w:ffData>\
           <w:name w:val=\"Text1\"/><w:enabled/><w:textInput/></w:ffData></w:fldChar></w:r>\
         <w:r><w:instrText xml:space=\"preserve\"> FORMTEXT </w:instrText></w:r>\
         <w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>\
         <w:r><w:t>hello</w:t></w:r>\
         <w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>\
         <w:p><w:r><w:fldChar w:fldCharType=\"begin\"><w:ffData>\
           <w:name w:val=\"Check1\"/><w:checkBox><w:checked w:val=\"1\"/></w:checkBox>\
         </w:ffData></w:fldChar></w:r>\
         <w:r><w:instrText xml:space=\"preserve\"> FORMCHECKBOX </w:instrText></w:r>\
         <w:r><w:fldChar w:fldCharType=\"separate\"/></w:r>\
         <w:r><w:fldChar w:fldCharType=\"end\"/></w:r></w:p>\
         </w:body></w:document>"
    );
    let path = tmp("form.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let ffs: Vec<json2docx::model::FormField> = doc
        .all_blocks()
        .flat_map(|b| match b {
            Block::Paragraph(p) => p.runs.clone().unwrap_or_default(),
            _ => vec![],
        })
        .filter_map(|r| r.form_field)
        .collect();
    assert_eq!(ffs.len(), 2);
    assert_eq!(ffs[0].kind, "text");
    assert_eq!(ffs[0].name.as_deref(), Some("Text1"));
    assert_eq!(ffs[1].kind, "checkbox");
    assert_eq!(ffs[1].checked, Some(true));
    let out = tmp("form_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(dx.contains("w:ffData") && dx.contains("FORMTEXT") && dx.contains("FORMCHECKBOX"));
}

#[test]
fn page_number_format() {
    let xml = format!(
        "<w:document {NS}><w:body><w:p><w:r><w:t>x</w:t></w:r></w:p>\
         <w:sectPr><w:pgSz w:w=\"11906\" w:h=\"16838\"/>\
           <w:pgNumType w:start=\"1\" w:fmt=\"upperRoman\"/></w:sectPr>\
         </w:body></w:document>"
    );
    let path = tmp("pgnum2.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let sec = doc
        .parts
        .iter()
        .filter_map(|p| p.section.as_ref())
        .find(|s| s.page_number_format.is_some())
        .or(doc.parts.first().and_then(|p| p.section.as_ref()));
    assert_eq!(
        sec.and_then(|s| s.page_number_format.clone()).as_deref(),
        Some("upperRoman")
    );
    let out = tmp("pgnum2_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(dx.contains("w:fmt=\"upperRoman\""), "应保留页码格式: {dx}");
}

#[test]
fn script_fonts_and_bidi_table() {
    let xml = format!(
        "<w:document {NS}><w:body>\
         <w:p><w:r><w:rPr>\
           <w:rFonts w:ascii=\"Calibri\" w:hAnsi=\"Calibri\" w:eastAsia=\"SimSun\" w:cs=\"Arial\"/>\
         </w:rPr><w:t>text</w:t></w:r></w:p>\
         <w:tbl><w:tblPr><w:bidiVisual/></w:tblPr>\
           <w:tr><w:tc><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc></w:tr></w:tbl>\
         </w:body></w:document>"
    );
    let path = tmp("script.docx");
    make_docx(&xml, &path);
    let doc = json2docx::parse(&path).expect("parse");
    let run = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Paragraph(p) => p.runs.as_ref().and_then(|rs| rs.first().cloned()),
            _ => None,
        })
        .expect("run");
    assert_eq!(run.font_family.as_deref(), Some("Calibri"));
    assert_eq!(run.east_asia_font.as_deref(), Some("SimSun"));
    assert_eq!(run.cs_font.as_deref(), Some("Arial"));
    let tbl = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table");
    assert_eq!(tbl.rtl, Some(true));
    let out = tmp("script_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(
        dx.contains("w:eastAsia=\"SimSun\"") && dx.contains("w:cs=\"Arial\""),
        "{dx}"
    );
    assert!(dx.contains("<w:bidiVisual/>"), "{dx}");
}

#[test]
fn smartart_editable_texts() {
    let document = format!(
        "<w:document {NS} xmlns:dgm=\"http://schemas.openxmlformats.org/drawingml/2006/diagram\">\
         <w:body><w:p><w:r><w:drawing><wp:inline>\
           <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/diagram\">\
             <dgm:relIds r:dm=\"rId9\" r:lo=\"rId10\" r:qs=\"rId11\" r:cs=\"rId12\"/>\
           </a:graphicData></a:graphic>\
         </wp:inline></w:drawing></w:r></w:p></w:body></w:document>"
    );
    let rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId9\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/diagramData\" Target=\"diagrams/data1.xml\"/>\
       </Relationships>";
    let data =
        "<dgm:dataModel xmlns:dgm=\"http://schemas.openxmlformats.org/drawingml/2006/diagram\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">\
         <dgm:ptLst><dgm:pt><dgm:t><a:t>旧文本</a:t></dgm:t></dgm:pt></dgm:ptLst></dgm:dataModel>";
    let path = tmp("saedit.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/_rels/document.xml.rels", rels),
            ("word/diagrams/data1.xml", data),
        ],
        &path,
    );
    let mut doc = json2docx::parse(&path).expect("parse");
    // 找到可编辑 SmartArt 并改文本
    let mut edited = false;
    for part in &mut doc.parts {
        for b in &mut part.blocks {
            if let Block::Raw(r) = b {
                if let Some(d) = &mut r.diagram {
                    assert_eq!(d.texts, vec!["旧文本".to_string()]);
                    d.texts = vec!["新文本".to_string()];
                    edited = true;
                }
            }
        }
    }
    assert!(edited, "应解析出可编辑 SmartArt 数据");
    let out = tmp("saedit_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/diagrams/data1.xml");
    assert!(
        dx.contains("新文本") && !dx.contains("旧文本"),
        "数据部件应被改写: {dx}"
    );
}

#[test]
fn table_pct_and_link_color() {
    let document = format!(
        "<w:document {NS}><w:body>\
         <w:tbl><w:tblPr><w:tblW w:w=\"5000\" w:type=\"pct\"/><w:tblLayout w:type=\"fixed\"/></w:tblPr>\
           <w:tblGrid><w:gridCol w:w=\"2000\"/></w:tblGrid>\
           <w:tr><w:tc><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc></w:tr></w:tbl>\
         <w:p><w:hyperlink r:id=\"rId5\"><w:r><w:t>link</w:t></w:r></w:hyperlink></w:p>\
         </w:body></w:document>"
    );
    let rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId5\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink\" Target=\"https://example.com\" TargetMode=\"External\"/>\
       </Relationships>";
    let path = tmp("pct.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/_rels/document.xml.rels", rels),
        ],
        &path,
    );
    let doc = json2docx::parse(&path).expect("parse");
    let tbl = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Table(t) => Some(t),
            _ => None,
        })
        .expect("table");
    assert_eq!(tbl.width_pct, Some(100.0));
    assert_eq!(tbl.layout.as_deref(), Some("fixed"));
    let out = tmp("pct_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let dx = read_part(&out, "word/document.xml");
    assert!(dx.contains("w:tblW w:w=\"5000\" w:type=\"pct\""), "{dx}");
    assert!(dx.contains("<w:tblLayout w:type=\"fixed\"/>"), "{dx}");
    // 超链接默认色
    assert!(
        dx.contains("w:color w:val=\"0563C1\""),
        "超链接应带默认色: {dx}"
    );
}

#[test]
fn chart_referenced_and_stacked() {
    let document = format!(
        "<w:document {NS} xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">\
         <w:body><w:p><w:r><w:drawing><wp:inline>\
           <wp:extent cx=\"3000000\" cy=\"2000000\"/>\
           <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">\
             <c:chart r:id=\"rId5\"/></a:graphicData></a:graphic>\
         </wp:inline></w:drawing></w:r></w:p></w:body></w:document>"
    );
    let rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId5\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart\" Target=\"charts/chart1.xml\"/>\
       </Relationships>";
    let chart = "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><c:chart><c:plotArea>\
         <c:barChart><c:barDir val=\"col\"/><c:grouping val=\"stacked\"/><c:ser>\
           <c:tx><c:strRef><c:strCache><c:pt idx=\"0\"><c:v>S1</c:v></c:pt></c:strCache></c:strRef></c:tx>\
           <c:cat><c:strRef><c:strCache><c:pt idx=\"0\"><c:v>A</c:v></c:pt><c:pt idx=\"1\"><c:v>B</c:v></c:pt></c:strCache></c:strRef></c:cat>\
           <c:val><c:numRef><c:numCache><c:pt idx=\"0\"><c:v>4.3</c:v></c:pt><c:pt idx=\"1\"><c:v>2.5</c:v></c:pt></c:numCache></c:numRef></c:val>\
         </c:ser><c:axId val=\"1\"/><c:axId val=\"2\"/></c:barChart></c:plotArea></c:chart></c:chartSpace>";
    let path = tmp("chartref.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/_rels/document.xml.rels", rels),
            ("word/charts/chart1.xml", chart),
        ],
        &path,
    );
    let doc = json2docx::parse(&path).expect("parse");
    let c = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Chart(c) => Some(c.clone()),
            _ => None,
        })
        .expect("chart");
    assert_eq!(c.grouping.as_deref(), Some("stacked"));
    assert_eq!(c.categories, vec!["A".to_string(), "B".to_string()]);
    assert_eq!(c.series[0].values, vec![4.3, 2.5]);
    assert_eq!(c.series[0].name, "S1");
    let out = tmp("chartref_out.docx");
    json2docx::generate(&doc, &out).expect("generate");
    let cx = read_part(&out, "word/charts/chart1.xml");
    assert!(
        cx.contains("<c:grouping val=\"stacked\"/>") && cx.contains("<c:overlap val=\"100\"/>"),
        "{cx}"
    );
    assert!(cx.contains(">4.3<") && cx.contains(">A<"), "{cx}");
}

#[test]
fn chart_multiple_groups_merged() {
    let document = format!(
        "<w:document {NS} xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">\
         <w:body><w:p><w:r><w:drawing><wp:inline><wp:extent cx=\"3000000\" cy=\"2000000\"/>\
           <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/chart\">\
             <c:chart r:id=\"rId5\"/></a:graphicData></a:graphic>\
         </wp:inline></w:drawing></w:r></w:p></w:body></w:document>"
    );
    let rels = "<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
         <Relationship Id=\"rId5\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart\" Target=\"charts/chart1.xml\"/>\
       </Relationships>";
    let g = |name: &str, v: f64| {
        format!(
        "<c:lineChart><c:grouping val=\"standard\"/><c:ser><c:tx><c:v>{name}</c:v></c:tx>\
         <c:val><c:numLit><c:pt idx=\"0\"><c:v>{v}</c:v></c:pt></c:numLit></c:val></c:ser></c:lineChart>"
    )
    };
    let chart = format!(
        "<c:chartSpace xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\" \
         xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><c:chart><c:plotArea>{}{}</c:plotArea></c:chart></c:chartSpace>",
        g("一", 1.0),
        g("二", 2.0)
    );
    let path = tmp("multigroup.docx");
    make_docx_multi(
        &[
            ("word/document.xml", &document),
            ("word/_rels/document.xml.rels", rels),
            ("word/charts/chart1.xml", &chart),
        ],
        &path,
    );
    let doc = json2docx::parse(&path).expect("parse");
    let c = doc
        .all_blocks()
        .find_map(|b| match b {
            Block::Chart(c) => Some(c.clone()),
            _ => None,
        })
        .expect("chart");
    assert_eq!(c.series.len(), 2, "应合并两个绘图组: {:?}", c.series);
}
