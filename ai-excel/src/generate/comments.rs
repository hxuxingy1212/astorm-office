//! Legacy cell comments (notes): `xl/commentsN.xml` + `xl/drawings/vmlDrawingN.vml`.

use crate::error::Result;
use crate::model::sheet::Sheet;
use crate::utils::a1::parse_cell_ref;
use crate::utils::constants::NS_MAIN;
use crate::utils::xml::esc_xml;

pub struct CommentsPart {
    pub comments_name: String,
    pub comments_xml: String,
    pub vml_name: String,
    pub vml_xml: String,
}

/// Build the comments + VML parts for a sheet, or `None` when it has none.
pub fn build_sheet_comments(sheet: &Sheet, no: usize) -> Result<Option<CommentsPart>> {
    let mut items: Vec<(String, String)> = Vec::new();
    for row in &sheet.rows {
        for cell in &row.cells {
            if let (Some(reference), Some(text)) = (&cell.reference, &cell.comment) {
                if !text.is_empty() {
                    items.push((reference.clone(), text.clone()));
                }
            }
        }
    }
    if items.is_empty() {
        return Ok(None);
    }
    items.sort_by(|a, b| {
        let ka = parse_cell_ref(&a.0).unwrap_or((u32::MAX, u32::MAX));
        let kb = parse_cell_ref(&b.0).unwrap_or((u32::MAX, u32::MAX));
        (ka.1, ka.0).cmp(&(kb.1, kb.0))
    });

    let mut comments_xml = String::new();
    comments_xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n");
    comments_xml.push_str(&format!("<comments xmlns=\"{NS_MAIN}\">"));
    comments_xml.push_str("<authors><author>json2xlsx</author></authors><commentList>");
    for (reference, text) in &items {
        comments_xml.push_str(&format!(
            "<comment ref=\"{}\" authorId=\"0\"><text><t xml:space=\"preserve\">{}</t></text></comment>",
            esc_xml(reference),
            esc_xml(text)
        ));
    }
    comments_xml.push_str("</commentList></comments>");

    let mut vml = String::new();
    vml.push_str("<xml xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" xmlns:x=\"urn:schemas-microsoft-com:office:excel\">\n");
    vml.push_str(
        "<o:shapelayout v:ext=\"edit\"><o:idmap v:ext=\"edit\" data=\"1\"/></o:shapelayout>\n",
    );
    vml.push_str("<v:shapetype id=\"_x0000_t202\" coordsize=\"21600,21600\" o:spt=\"202\" path=\"m,l,21600r21600,l21600,xe\"><v:stroke joinstyle=\"miter\"/><v:path gradientshapeok=\"t\" o:connecttype=\"rect\"/></v:shapetype>\n");
    for (i, (reference, _)) in items.iter().enumerate() {
        let (col, row) = parse_cell_ref(reference).unwrap_or((1, 1));
        let c0 = col.saturating_sub(1);
        let r0 = row.saturating_sub(1);
        let shape_id = 1025 + i;
        let anchor = format!("{c0}, 15, {r0}, 2, {}, 15, {}, 4", c0 + 3, r0 + 4);
        vml.push_str(&format!(
            "<v:shape id=\"_x0000_s{shape_id}\" type=\"#_x0000_t202\" style=\"position:absolute;margin-left:59.25pt;margin-top:1.5pt;width:108pt;height:59.25pt;z-index:1;visibility:hidden\" fillcolor=\"#ffffe1\" o:insetmode=\"auto\">\
             <v:fill color2=\"#ffffe1\"/><v:shadow on=\"t\" color=\"black\" obscured=\"t\"/><v:path o:connecttype=\"none\"/>\
             <v:textbox style=\"mso-direction-alt:auto\"><div style=\"text-align:left\"></div></v:textbox>\
             <x:ClientData ObjectType=\"Note\"><x:MoveWithCells/><x:SizeWithCells/><x:Anchor>{anchor}</x:Anchor><x:AutoFill>False</x:AutoFill><x:Row>{r0}</x:Row><x:Column>{c0}</x:Column></x:ClientData>\
             </v:shape>\n"
        ));
    }
    vml.push_str("</xml>");

    Ok(Some(CommentsPart {
        comments_name: format!("comments{no}.xml"),
        comments_xml,
        vml_name: format!("vmlDrawing{no}.vml"),
        vml_xml: vml,
    }))
}
