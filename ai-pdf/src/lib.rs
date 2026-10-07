//! json2pdf — Rust 实现的 PDF 工具箱：JSON 产物目录双向解析（unpack / repack）
//! + ZCode 官方 pdf 技能核心 CLI 能力的移植。产物目录设计对齐 ai-ppt（json2pptx）。
//!
//! 模块划分与原插件 scripts/ 的对应关系：
//! - `unpack` / `repack` / `model` ← JSON 产物目录双向解析（本工具新增能力）
//! - `text` / `images`   ← pdf.py 的 extract.* （pdfplumber/pikepdf）
//! - `page_ops`          ← pdf.py 的 pages.*
//! - `meta`              ← pdf.py 的 meta.*
//! - `forms`             ← pdf.py 的 form.info / form.fill（pikepdf 路线）
//! - `convert`           ← pdf.py 的 convert.office / convert.html / convert.latex
//! - `palette` / `design`← design_engine.py（HSL 调色板 / SVG 背景 / 布局 / 关键词推导）
//! - `sanitize`          ← pdf.py 的 code.sanitize / content.sanitize
//! - `checks`            ← pdf.py 的 font.check / toc.check（字符级近似）
//! - `qa`                ← pdf_qa.py 的可移植子集
//!
//! CLI 输出契约走 office-core 的 `Output`（docs/cli-conventions.md）。

pub mod checks;
pub mod color;
pub mod complexity;
pub mod convert;
pub mod cross;
pub mod design;
pub mod fonts;
pub mod forms;
pub mod images;
pub mod layout;
pub mod markdown;
pub mod meta;
pub mod model;
pub mod ocr;
#[cfg(target_os = "macos")]
pub mod ocr_mac;
pub mod page_ops;
pub mod palette;
pub mod qa;
pub mod repack;
pub mod sanitize;
pub mod text;
pub mod unpack;
pub mod util;
