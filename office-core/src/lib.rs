//! astorm-office 共享基础层。
//!
//! 三个文档 CLI（json2docx / json2xlsx / json2pptx）共用的底层设施：
//! - [`guards`]：一次性临时目录/文件守卫（RAII 清理）
//! - [`opc`]：OPC（OOXML 包）通用部件读写，与具体文档格式无关
//! - [`output`]：统一 CLI 输出契约（stdout 只放数据、stderr 放状态、--json 全结构化）
//! - [`pathutil`]：路径与字符串转换辅助
//! - [`soffice`]：LibreOffice 无头转换引擎（旧格式 → 新格式的最高保真路径）
//!
//! 退出码约定（三个 CLI 一致）：
//! `0` 成功；`1` 运行时错误；`2` 用法错误（clap 默认）；`3` 命令成功但产物存在校验问题。

pub mod batch;
pub mod chrome;
pub mod error;
pub mod guards;
pub mod merge;
pub mod opc;
pub mod output;
pub mod pathutil;
pub mod soffice;
pub mod suggest;

pub use error::CliError;
pub use guards::{FileGuard, TempGuard};
pub use output::{Output, EXIT_FAILURE, EXIT_ISSUES, EXIT_SUCCESS, EXIT_USAGE};
pub use pathutil::{path_str, same_file, stem_dir, with_stem_ext};
