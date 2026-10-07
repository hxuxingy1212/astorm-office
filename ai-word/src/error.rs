//! 库级错误类型

use std::fmt;

/// json2docx 统一错误类型
#[derive(Debug)]
pub enum Error {
    /// JSON 序列化/反序列化错误
    Json(serde_json::Error),
    /// IO 错误
    Io(std::io::Error),
    /// ZIP 读写错误
    Zip(zip::result::ZipError),
    /// XML 解析错误
    Xml(quick_xml::Error),
    /// 输入非法（模型/路径/属性）
    InvalidInput(String),
    /// 图片加载失败
    ImageLoad(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Json(e) => write!(f, "JSON 错误: {e}"),
            Error::Io(e) => write!(f, "IO 错误: {e}"),
            Error::Zip(e) => write!(f, "ZIP 错误: {e}"),
            Error::Xml(e) => write!(f, "XML 错误: {e}"),
            Error::InvalidInput(s) => write!(f, "输入非法: {s}"),
            Error::ImageLoad(s) => write!(f, "图片加载失败: {s}"),
        }
    }
}

impl std::error::Error for Error {}

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
impl From<zip::result::ZipError> for Error {
    fn from(e: zip::result::ZipError) -> Self {
        Error::Zip(e)
    }
}
impl From<quick_xml::Error> for Error {
    fn from(e: quick_xml::Error) -> Self {
        Error::Xml(e)
    }
}

/// 库级 Result
pub type Result<T> = std::result::Result<T, Error>;

use office_core::CliError;

impl From<Error> for CliError {
    fn from(e: Error) -> Self {
        let code = match &e {
            Error::Json(_) => "json",
            Error::Io(_) => "io",
            Error::Zip(_) => "zip",
            Error::Xml(_) => "xml",
            Error::InvalidInput(_) => "invalid_input",
            Error::ImageLoad(_) => "image_load",
        };
        CliError::with_code(code, e.to_string())
    }
}
