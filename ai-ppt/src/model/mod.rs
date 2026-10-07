//! 数据模型模块
//!
//! 定义了所有 JSON 数据模型结构，使用 serde 实现序列化/反序列化。
//! 该模块是项目的核心，生成和解析模块都基于此处的数据结构。

/// 基础元素模型：文本、形状、图片、表格、分组等
pub mod elements;
/// 演示文稿顶层结构：Presentation, Meta
pub mod presentation;
/// 幻灯片模型：包含元素列表和背景/过渡
pub mod slide;
/// 模板预设：6 种内置主题模板
pub mod template;
/// 主题模型：颜色方案和字体
pub mod theme;
/// 幻灯片过渡动画模型
pub mod transition;

pub use elements::*;
pub use presentation::{Meta, Presentation};
pub use slide::Slide;
pub use template::TemplatePreset;
pub use theme::Theme;
pub use transition::Transition;
