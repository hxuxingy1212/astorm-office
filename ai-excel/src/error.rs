use std::fmt;

/// Library error type.
#[derive(Debug)]
pub enum Error {
    Json(serde_json::Error),
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    Xml(String),
    InvalidInput(String),
    /// 文档已加密（OOXML 密码保护），CLI 层映射为 `encrypted` 错误码
    Encrypted(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Json(e) => write!(f, "JSON 错误: {e}"),
            Error::Io(e) => write!(f, "IO 错误: {e}"),
            Error::Zip(e) => write!(f, "ZIP 错误: {e}"),
            Error::Xml(e) => write!(f, "XML 错误: {e}"),
            Error::InvalidInput(e) => write!(f, "输入错误: {e}"),
            Error::Encrypted(e) => write!(f, "{e}"),
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

impl From<zip::result::ZipError> for Error {
    fn from(e: zip::result::ZipError) -> Self {
        Error::Zip(e)
    }
}

impl From<quick_xml::Error> for Error {
    fn from(e: quick_xml::Error) -> Self {
        Error::Xml(e.to_string())
    }
}

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
            Error::Encrypted(_) => "encrypted",
        };
        CliError::with_code(code, e.to_string())
    }
}
