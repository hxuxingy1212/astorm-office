//! Parse an xlsx and print its JSON model.
//!
//! Usage: cargo run --example read -- /tmp/demo.xlsx

use std::path::Path;

fn main() {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/json2xlsx_demo.xlsx".to_string());
    let wb = json2xlsx::parse(Path::new(&path)).expect("parse");
    println!("{}", serde_json::to_string_pretty(&wb).unwrap());
}
