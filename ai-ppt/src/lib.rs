//! json2pptx 库入口
//!
//! 提供 JSON 到 PPTX 的双向转换功能。
//! 将 JSON 格式的演示文稿数据模型生成为 .pptx 文件，
//! 或从现有的 .pptx 文件解析回 JSON 数据模型。

/// 库级错误类型
pub mod error;

/// 生成模块：将 JSON 模型生成为 OOXML 格式的 .pptx 文件
pub mod generate;
/// 解析模块：从 .pptx 文件中解析回 JSON 数据模型
pub mod legacy;
pub mod merge;
/// 数据模型模块：定义演示文稿、幻灯片、元素等数据结构
pub mod model;
pub mod parse;
/// 工具模块：单位换算、XML 辅助等通用工具
pub mod utils;

/// 库级错误类型
pub use error::Error;
/// 生成 PPTX 文件的顶层函数
pub use generate::generate;
/// 从 unpack 产物目录重建 PPTX
pub use generate::repack;
/// 生成结果
pub use generate::GenerateResult;
/// 解析 PPTX 文件的顶层函数
pub use legacy::{is_cfb, ppt_to_pptx};
/// 演示文稿数据模型
pub use merge::merge_slides;
pub use model::Presentation;
pub use parse::parse;
/// 将 PPTX 解包为可编辑中间产物
pub use parse::unpack;
/// PPTX 解包结果
pub use parse::UnpackResult;
