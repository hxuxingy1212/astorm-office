//! json2xlsx — JSON ↔ XLSX bidirectional conversion.
//!
//! Architecture mirrors json2pptx: a single serde data model is the pivot,
//! `generate` writes OOXML parts into a ZIP and `parse` reads them back.

pub mod detect;
pub mod error;
pub mod eval;
pub mod generate;
pub mod legacy;
pub mod markdown;
pub mod merge;
pub mod model;
pub mod parse;
pub mod product;
pub mod raw;
pub mod template;
pub mod utils;

pub use error::{Error, Result};
pub use generate::{generate, GenerateResult};
pub use legacy::is_cfb;
pub use legacy::maybe_legacy_workbook;
pub use legacy::xls_to_workbook;
pub use markdown::{
    sheet_markdown, sheet_markdown_with, used_range, window_markdown, workbook_markdown,
    workbook_markdown_with,
};
pub use merge::merge_workbook;
pub use model::Workbook;
pub use office_core::{ExportedAttachment, PayloadSink, PayloadSinks};
pub use parse::parse;
pub use product::{
    extract_embeddings_to, extract_payloads_to, is_product_dir, load_product, repack, save_product,
    unpack, UnpackResult,
};
pub use template::{extract_template, render_markdown, skeleton_workbook, Template};
