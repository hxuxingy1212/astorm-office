fn main() {
    let doc = lopdf::Document::load(std::env::args().nth(1).unwrap()).unwrap();
    let pid = *doc.get_pages().get(&1).unwrap();
    let fonts = doc.get_page_fonts(pid).unwrap_or_default();
    for (name, font) in fonts {
        println!("resource name bytes: {:?}", String::from_utf8_lossy(&name));
        let dec = json2pdf::text::resolve_font(font, &doc);
        println!(
            "  decoder: {}",
            match &dec {
                json2pdf::text::FontDecoder::MyCmap {
                    code_bytes, map, ..
                } => format!("MyCmap(bytes={code_bytes}, entries={})", map.len()),
                json2pdf::text::FontDecoder::DiffTable(_) => "DiffTable".into(),
                json2pdf::text::FontDecoder::Lopdf(_) => "Lopdf".into(),
                json2pdf::text::FontDecoder::Latin1 => "Latin1".into(),
                json2pdf::text::FontDecoder::Cjk(_) => "Cjk".into(),
            }
        );
    }
}
