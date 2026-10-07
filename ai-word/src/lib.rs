//! json2docx 库入口
//!
//! 提供 JSON 与 DOCX（Word）的双向转换：将 JSON 描述生成为 .docx，
//! 或将 .docx 解析回 JSON 数据模型（回环）。
//!
//! 架构延续 ai-ppt / json2pptx：JSON 优先、产物目录中间形态、无状态 CLI。

pub mod error;
pub mod extract;
pub mod generate;
pub mod legacy;
pub mod merge;
pub mod model;
pub mod parse;
pub mod utils;
pub mod validate;

pub use error::Error;
pub use extract::{describe_template, extract_template, ExtractedTemplate};
pub use generate::{generate, repack, GenerateResult};
pub use merge::merge_document;
pub use model::Document;
pub use parse::{parse, unpack, UnpackResult};
pub use validate::{content_warnings, validate_docx, validate_product};
