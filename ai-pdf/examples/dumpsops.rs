//! 调试：dump 指定页内容流操作序列（前 N 条）。
//! 用法：cargo run -p json2pdf --example dumpsops -- <pdf> [page]

use json2pdf::text::tokenize;
use lopdf::Document;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: dumpsops <pdf> [page] [count]");
    let page: u32 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let count: usize = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let doc = Document::load(path).unwrap();
    let pid = doc.get_pages()[&page];
    let content = doc.get_page_content(pid).unwrap();
    let ops = tokenize(&content);
    for op in ops.iter().take(count) {
        let operands: Vec<String> = op
            .operands
            .iter()
            .map(|t| match t {
                json2pdf::text::Tok::Num(n) => format!("{n}"),
                json2pdf::text::Tok::Str(s) => {
                    let hex: Vec<String> = s.iter().map(|b| format!("{b:02x}")).collect();
                    format!("str[{}]", hex.join(" "))
                }
                json2pdf::text::Tok::Name(n) => format!("/{}", String::from_utf8_lossy(n)),
                json2pdf::text::Tok::Array(_) => "[...]".into(),
                json2pdf::text::Tok::Dict(_) => "<<>>".into(),
            })
            .collect();
        println!("{} {}", operands.join(" "), op.operator);
    }
    println!("--- total {} ops", ops.len());
}
