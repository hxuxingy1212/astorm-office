//! 库级错误类型
//!
//! 统一 generate / parse / extract_media 顶层 API 的错误返回，
//! 替代原先零散的 `Box<dyn std::error::Error>`。

use std::fmt;

/// json2pptx 库级错误
#[derive(Debug)]
pub enum Error {
    /// 输入 JSON 解析失败
    Json(serde_json::Error),
    /// 文件 / ZIP 读写失败
    Io(std::io::Error),
    /// PPTX ZIP 包结构错误
    Zip(zip::result::ZipError),
    /// 无效输入（如空演示文稿、图片缺失）
    InvalidInput(String),
    /// 图片加载失败（本地路径不存在或 URL 下载失败）
    ImageLoad(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Json(e) => write!(f, "JSON 解析失败: {e}"),
            Error::Io(e) => write!(f, "文件读写失败: {e}"),
            Error::Zip(e) => write!(f, "PPTX ZIP 结构错误: {e}"),
            Error::InvalidInput(msg) => write!(f, "无效输入: {msg}"),
            Error::ImageLoad(src) => write!(f, "图片加载失败: {src}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Json(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::Zip(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<quick_xml::Error> for Error {
    fn from(e: quick_xml::Error) -> Self {
        Error::InvalidInput(format!("XML 解析失败: {e}"))
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(e: zip::result::ZipError) -> Self {
        Error::Zip(e)
    }
}

use office_core::CliError;

impl From<Error> for CliError {
    fn from(e: Error) -> Self {
        let code = match &e {
            Error::Json(_) => "json",
            Error::Io(_) => "io",
            Error::Zip(_) => "zip",
            Error::InvalidInput(_) => "invalid_input",
            Error::ImageLoad(_) => "image_load",
        };
        CliError::with_code(code, e.to_string())
    }
}
