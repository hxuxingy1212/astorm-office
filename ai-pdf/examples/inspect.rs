//! 调试工具：dump 每页字体字典的关键编码字段。
//! 用法：cargo run -p json2pdf --example inspect -- <pdf>

fn main() {
    let path = std::env::args().nth(1).expect("usage: inspect <pdf>");
    let doc = lopdf::Document::load(path).unwrap();
    for (pno, pid) in doc.get_pages() {
        let fonts = doc.get_page_fonts(pid).unwrap_or_default();
        for (name, font) in fonts {
            let subtype = font
                .get(b"Subtype")
                .and_then(lopdf::Object::as_name_str)
                .unwrap_or("?");
            let base = font
                .get(b"BaseFont")
                .and_then(lopdf::Object::as_name_str)
                .unwrap_or("?");
            let enc = match font.get(b"Encoding") {
                Ok(lopdf::Object::Name(n)) => format!("Name({})", String::from_utf8_lossy(n)),
                Ok(lopdf::Object::Dictionary(d)) => {
                    let diff = d.get(b"Differences").is_ok();
                    let base = d
                        .get(b"BaseEncoding")
                        .and_then(lopdf::Object::as_name_str)
                        .ok();
                    format!("Dict(base={base:?}, differences={diff})")
                }
                other => format!("{other:?}"),
            };
            let has_tounicode = match font.get(b"ToUnicode") {
                Ok(lopdf::Object::Reference(id)) => doc.get_object(*id).is_ok(),
                Ok(lopdf::Object::Stream(_)) => true,
                _ => false,
            };
            let to_unicode_len = match font.get_deref(b"ToUnicode", &doc) {
                Ok(lopdf::Object::Stream(s)) => s.get_plain_content().map(|c| c.len()).unwrap_or(0),
                _ => 0,
            };
            println!(
                "page {pno} /{}: subtype={subtype} base={base} enc={enc} toUnicode={has_tounicode} (len={to_unicode_len})",
                String::from_utf8_lossy(&name)
            );
            // ToUnicode 解码链验证：用 json2pdf 自研解析器探测
            if to_unicode_len > 0 {
                if let Ok(lopdf::Object::Stream(s)) = font.get_deref(b"ToUnicode", &doc) {
                    let content = s.get_plain_content().unwrap_or_default();
                    println!(
                        "  CMap head: {}",
                        String::from_utf8_lossy(&content[..content.len().min(240)])
                            .replace('\n', " | ")
                    );
                    match json2pdf::text::parse_tounicode_cmap(&content) {
                        Some((code_bytes, map)) => {
                            let probe: Vec<String> = [0x58u32, 0x44u32, 0x07u32]
                                .iter()
                                .map(|c| {
                                    format!("{c:04x}→{}", map.get(c).cloned().unwrap_or_default())
                                })
                                .collect();
                            println!(
                                "  my-cmap OK code_bytes={code_bytes} entries={} probes: {probe:?}",
                                map.len()
                            );
                        }
                        None => println!("  my-cmap FAILED to parse"),
                    }
                }
            }
            // get_font_encoding 结果
            match font.get_font_encoding(&doc) {
                Ok(enc) => println!("  get_font_encoding: {:?}", enc),
                Err(e) => println!("  get_font_encoding FAILED: {e:?}"),
            }
            // resolve_font 变体
            let dec = json2pdf::text::resolve_font(font, &doc);
            println!(
                "  resolve_font variant: {}",
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
}
// 追加：resolve_font 变体诊断
