//! 统一 CLI 输出契约。
//!
//! 契约（三个 CLI 一致，详见根目录 `docs/cli-conventions.md`）：
//! - stdout 永远只有数据（JSON 结果 / 原始部件字节），可被 `jq` 直接解析；
//! - stderr 承载人类可读状态/进度；`--json` 时 stderr 事件与错误也结构化为 JSON 行；
//! - 默认模式错误为中文一行（`错误: ...`），可附 `提示: ...`；
//! - 退出码：0 成功 / 1 运行时错误 / 2 用法错误 / 3 产物有校验问题。

use serde::Serialize;
use std::io::Write;

/// 成功
pub const EXIT_SUCCESS: i32 = 0;
/// 运行时错误（IO/解析/参数逻辑错误等）
pub const EXIT_FAILURE: i32 = 1;
/// 用法错误（clap 默认行为，此常量仅作文档化）
pub const EXIT_USAGE: i32 = 2;
/// 命令成功但产物存在校验问题（如 repack 后 validate 有 issues）
pub const EXIT_ISSUES: i32 = 3;

/// CLI 输出通道。
pub struct Output {
    json: bool,
    quiet: bool,
    verbose: bool,
}

impl Output {
    pub fn new(json: bool, quiet: bool, verbose: bool) -> Self {
        Self {
            json,
            quiet,
            verbose,
        }
    }

    /// 是否处于 --json 模式（决定写盘命令是否向 stdout 输出结果 JSON）
    pub fn json_mode(&self) -> bool {
        self.json
    }

    pub fn verbose(&self) -> bool {
        self.verbose
    }

    /// 状态/进度信息 → stderr；`--quiet` 抑制，`--json` 时输出 JSON 事件行。
    pub fn status(&self, message: &str) {
        if self.quiet {
            return;
        }
        let mut err = std::io::stderr().lock();
        if self.json {
            let line = serde_json::json!({ "event": "status", "message": message });
            let _ = writeln!(err, "{line}");
        } else {
            let _ = writeln!(err, "{message}");
        }
    }

    /// 数据 → stdout（美化 JSON）。任何模式下都只输出这一个 JSON 文档。
    pub fn emit_json<S: Serialize>(&self, value: &S) -> std::io::Result<()> {
        let mut out = std::io::stdout().lock();
        writeln!(
            out,
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_default()
        )
    }

    /// 写盘类命令在 `--json` 模式下向 stdout 输出结果 JSON（含输出路径等）。
    /// 默认模式下不产生 stdout 输出（状态走 [`Output::status`]）。
    pub fn emit_result<S: Serialize>(&self, value: &S) -> std::io::Result<()> {
        if self.json {
            self.emit_json(value)?;
        }
        Ok(())
    }

    /// 错误 → stderr；`--json` 时输出 `{"ok":false,"error":{code,message,hint,suggestion}}`。
    pub fn error(&self, code: &str, message: &str, hint: Option<&str>) {
        self.error_full(code, message, hint, None);
    }

    /// 带 suggestion 的错误（agent 自愈：给出纠正建议/合法取值范围）。
    pub fn error_full(
        &self,
        code: &str,
        message: &str,
        hint: Option<&str>,
        suggestion: Option<&str>,
    ) {
        let mut err = std::io::stderr().lock();
        if self.json {
            let line = serde_json::json!({
                "ok": false,
                "error": {
                    "code": code,
                    "message": message,
                    "hint": hint,
                    "suggestion": suggestion,
                }
            });
            let _ = writeln!(err, "{line}");
        } else {
            let _ = writeln!(err, "错误: {message}");
            if let Some(h) = hint {
                let _ = writeln!(err, "提示: {h}");
            }
            if let Some(s) = suggestion {
                let _ = writeln!(err, "建议: {s}");
            }
        }
    }
}
