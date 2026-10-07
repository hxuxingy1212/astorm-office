//! 数据模型模块
//!
//! 使用 serde 定义 JSON 数据模型，是生成与解析共享的核心。
//! 结构对齐 ai-ppt：Document↔Presentation，Part↔Slide，Block↔Element。

pub mod blocks;
pub mod document;
pub mod part;
pub mod style;
pub mod template;

pub use blocks::*;
pub use document::{Document, Meta, PassthroughPart};
pub use part::{Part, Section};
pub use style::{Margins, PageSetup, Style, Theme};
pub use template::TemplatePreset;
