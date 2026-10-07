use lopdf::{Document, Object};
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let doc = Document::load(&path).unwrap();
    let pages = doc.get_pages();
    println!("页数 {}", pages.len());
    for (pno, id) in pages.iter().take(2) {
        let content = doc.get_page_content(*id).unwrap_or_default();
        println!("page {pno}: content {} 字节", content.len());
        if let Ok(d) = doc.get_dictionary(*id) {
            if let Ok(c) = d.get(b"Contents") {
                if let Ok(cid) = c.as_reference() {
                    if let Ok(Object::Stream(s)) = doc.get_object(cid) {
                        println!("  stream dict: {:?} content={} ", s.dict, s.content.len());
                    }
                }
            }
        }
    }
}
