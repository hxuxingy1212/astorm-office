//! CLI 层统一错误：携带错误码与 agent 自愈建议（suggestion）。
//!
//! 库层错误（json2docx/json2xlsx/json2pptx 的 `Error`）在各 CLI 内通过
//! `From` 转换进入此类型，统一获得错误码；属性/类型/取值/索引类错误
//! 在产生处附带 `suggestion`（纠错与合法取值范围），由 CLI 输出到
//! `--json` 错误的 `error.suggestion` 字段。

use std::fmt;

/// CLI 错误：`code` 供机器判别，`message` 供人读，`suggestion` 供 agent 自愈。
#[derive(Debug, Clone)]
pub struct CliError {
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
}

impl CliError {
    /// 默认错误码 `invalid_input`
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_input".to_string(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// 指定错误码
    pub fn with_code(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// 附加建议（链式）
    pub fn suggest(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CliError {}

impl From<String> for CliError {
    fn from(s: String) -> Self {
        CliError::new(s)
    }
}

impl From<&str> for CliError {
    fn from(s: &str) -> Self {
        CliError::new(s)
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::with_code("io", e.to_string())
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        CliError::with_code("json", e.to_string())
    }
}
