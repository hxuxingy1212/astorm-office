//! 批量执行：一个脚本里的多条命令顺序跑完，一份报告说清每步结果。
//!
//! 脚本形态（`batch` 子命令）：
//!
//! ```text
//! # 文本脚本（.txt / 无扩展名 / - 读 stdin）：一行一条，'#' 开头为注释
//! word unpack thesis.docx -o prod/
//! word view prod/ / text
//! excel import data.csv -o book.xlsx
//! pdf extract text report.pdf --format markdown
//! ```
//!
//! ```json
//! { "version": 1, "commands": [
//!   { "format": "word", "args": ["unpack", "thesis.docx", "-o", "prod/"] },
//!   { "format": "word", "args": ["view", "prod/", "/", "text"] },
//!   { "args": ["import", "data.csv", "-o", "book.xlsx"] }
//! ]}
//! ```
//!
//! - 每步 `format` 可省（从 `args` 里的文件扩展名推断）；
//! - 字符串步骤（`"word unpack a.docx"`）按 shell 风格切词；
//! - 任一步失败默认停下（`--force` 或步骤级 `continue_on_error: true` 可继续）；
//! - 不做隐式依赖：步骤之间通过文件/产物目录传递结果，与单命令完全等价。

use crate::dispatch::{self, Format};
use office_core::CliError;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::time::Instant;

/// 一步命令
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    /// 显式格式（未写则从 args 推断）
    pub format: Option<Format>,
    /// 转发给底层 CLI 的参数
    pub args: Vec<String>,
    /// 该步失败后是否继续（覆盖全局 `--force`）
    pub continue_on_error: bool,
    /// 原始 JSON（报告里回显；字符串步骤为 JSON 字符串）
    pub raw: Value,
}

/// 单步执行结果
#[derive(Debug, Serialize, Clone)]
pub struct StepReport {
    pub index: usize,
    pub format: String,
    pub bin: String,
    /// 二进制定位方式：env / sibling / path
    pub source: String,
    pub args: Vec<String>,
    /// 空跑（--dry-run）时不执行
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// 捕获的 stdout（仅 --json 报告里带回；默认模式直通到终端）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 整批执行报告
#[derive(Debug, Serialize)]
pub struct BatchReport {
    pub ok: bool,
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    /// 因失败提前停止时，指向那一歩的序号
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stopped_at: Option<usize>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub dry_run: bool,
    pub duration_ms: u64,
    pub steps: Vec<StepReport>,
}

/// 执行选项
pub struct BatchOpts {
    /// 遇错继续（等价于每步 continue_on_error）
    pub force: bool,
    /// 只打印将执行的命令，不真的跑
    pub dry_run: bool,
    /// 捕获各步输出并输出报告 JSON（默认：直通 stdout/stderr，报告走 stderr）
    pub json: bool,
    pub quiet: bool,
}

/// 解析脚本内容（自动识别 JSON / 文本）
pub fn parse_script(content: &str) -> Result<Vec<Step>, CliError> {
    let trimmed = content.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        let v: Value = serde_json::from_str(content).map_err(|e| {
            CliError::with_code("json", format!("脚本不是合法 JSON: {e}")).suggest(
                "JSON 脚本形如 {\"commands\":[{\"format\":\"word\",\"args\":[\"unpack\",…]}]}",
            )
        })?;
        return parse_steps_json(&v);
    }
    parse_script_text(content)
}

/// 文本脚本：一行一条命令，`#` 注释与空行跳过
pub fn parse_script_text(content: &str) -> Result<Vec<Step>, CliError> {
    let mut steps = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let tokens =
            split_shellish(line).map_err(|e| CliError::new(format!("第 {} 行: {e}", i + 1)))?;
        steps.push(step_from_tokens(tokens, json!(line)));
    }
    Ok(steps)
}

/// JSON 步骤：接受裸数组或 `{"commands": [...]}` / `{"steps": [...]}`
pub fn parse_steps_json(v: &Value) -> Result<Vec<Step>, CliError> {
    let arr = match v {
        Value::Object(o) => o
            .get("commands")
            .or_else(|| o.get("steps"))
            .cloned()
            .ok_or_else(|| {
                CliError::new("JSON 脚本缺少 commands/steps 数组")
                    .suggest("{\"version\":1,\"commands\":[…]} 或直接给数组")
            })?,
        Value::Array(a) => Value::Array(a.clone()),
        other => {
            return Err(CliError::new(format!(
                "JSON 脚本应为数组或对象，实际: {}",
                json_type_name(other)
            )))
        }
    };
    let Value::Array(items) = arr else {
        return Err(CliError::new("commands 应为数组"));
    };
    let mut steps = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let step = parse_step(item).map_err(|e| {
            let mut err = CliError::new(format!("第 {} 条命令: {}", i + 1, e.message));
            err.suggestion = e.suggestion;
            err
        })?;
        steps.push(step);
    }
    Ok(steps)
}

/// 单条 JSON 步骤
fn parse_step(item: &Value) -> Result<Step, CliError> {
    match item {
        Value::String(s) => {
            let tokens = split_shellish(s)?;
            Ok(step_from_tokens(tokens, item.clone()))
        }
        Value::Object(o) => {
            let format = match o.get("format").or_else(|| o.get("fmt")) {
                Some(Value::String(s)) => Some(dispatch::parse_format(s).ok_or_else(|| {
                    CliError::new(format!("未知格式: {s}")).suggest(format!(
                        "可用：{}",
                        Format::all()
                            .iter()
                            .map(|f| dispatch::alias_hint(*f))
                            .collect::<Vec<_>>()
                            .join(" · ")
                    ))
                })?),
                Some(other) => {
                    return Err(CliError::new(format!(
                        "format 应为字符串，实际: {}",
                        json_type_name(other)
                    )))
                }
                None => None,
            };
            let args = match o.get("args").or_else(|| o.get("argv")) {
                Some(Value::Array(a)) => a
                    .iter()
                    .map(|x| match x {
                        Value::String(s) => Ok(s.clone()),
                        other => Ok(other.to_string()),
                    })
                    .collect::<Result<Vec<String>, CliError>>()?,
                Some(Value::String(s)) => split_shellish(s)?,
                Some(other) => {
                    return Err(CliError::new(format!(
                        "args 应为数组或字符串，实际: {}",
                        json_type_name(other)
                    )))
                }
                None => return Err(
                    CliError::new("缺少 args（要转发给底层 CLI 的参数）").suggest(
                        "{\"format\":\"word\",\"args\":[\"unpack\",\"a.docx\",\"-o\",\"prod/\"]}",
                    ),
                ),
            };
            if args.is_empty() {
                return Err(CliError::new("args 不能为空"));
            }
            let continue_on_error = o
                .get("continue_on_error")
                .or_else(|| o.get("continueOnError"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            Ok(Step {
                format,
                args,
                continue_on_error,
                raw: item.clone(),
            })
        }
        other => Err(CliError::new(format!(
            "命令应为对象或字符串，实际: {}",
            json_type_name(other)
        ))),
    }
}

/// 由切好的词构造步骤：首词是格式别名则作为格式，否则整串都是参数（格式后推断）
fn step_from_tokens(tokens: Vec<String>, raw: Value) -> Step {
    let mut tokens = tokens;
    let format = if tokens.is_empty() {
        None
    } else {
        match dispatch::parse_format(&tokens[0]) {
            Some(f) => {
                tokens.remove(0);
                Some(f)
            }
            None => None,
        }
    };
    Step {
        format,
        args: tokens,
        continue_on_error: false,
        raw,
    }
}

/// shell 风格切词：支持单/双引号与双引号内反斜杠转义
pub fn split_shellish(line: &str) -> Result<Vec<String>, CliError> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has_token = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                has_token = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(c) => cur.push(c),
                        None => {
                            return Err(CliError::new("单引号没有闭合")
                                .suggest("成对使用引号，或用 '\\'' 转义"))
                        }
                    }
                }
            }
            '"' => {
                has_token = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(e @ ('"' | '\\')) => cur.push(e),
                            Some(other) => {
                                cur.push('\\');
                                cur.push(other);
                            }
                            None => return Err(CliError::new("双引号没有闭合")),
                        },
                        Some(c) => cur.push(c),
                        None => return Err(CliError::new("双引号没有闭合")),
                    }
                }
            }
            c if c.is_whitespace() => {
                if has_token {
                    out.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            c => {
                has_token = true;
                cur.push(c);
            }
        }
    }
    if has_token {
        out.push(cur);
    }
    Ok(out)
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "布尔",
        Value::Number(_) => "数字",
        Value::String(_) => "字符串",
        Value::Array(_) => "数组",
        Value::Object(_) => "对象",
    }
}

/// 顺序执行脚本，产出报告。
///
/// - 默认停在第 1 个失败处（`--force` 或步骤 `continue_on_error` 继续）；
/// - `--json` 捕获每步 stdout/stderr 进报告；否则直通终端；
/// - `--dry-run` 只解析与定位，不执行。
pub fn run(steps: &[Step], opts: &BatchOpts) -> Result<BatchReport, CliError> {
    let started = Instant::now();
    let mut reports: Vec<StepReport> = Vec::with_capacity(steps.len());
    let mut stopped_at = None;

    for (i, step) in steps.iter().enumerate() {
        let index = i + 1;
        let format = match step.format {
            Some(f) => f,
            None => match dispatch::infer_format(&step.args) {
                Some(f) => f,
                None => {
                    let msg = format!(
                        "无法从参数推断格式: {}",
                        step.args.first().cloned().unwrap_or_default()
                    );
                    reports.push(StepReport {
                        index,
                        format: String::new(),
                        bin: String::new(),
                        source: String::new(),
                        args: step.args.clone(),
                        dry_run: opts.dry_run,
                        ok: false,
                        exit_code: None,
                        stdout: None,
                        stderr: None,
                        error: Some(msg),
                    });
                    stopped_at = Some(index);
                    break;
                }
            },
        };

        let (bin, source) = dispatch::resolve_bin(format)?;
        let bin_display = bin.display().to_string();
        if !opts.quiet {
            status(
                opts,
                &format!(
                    "[{index}/{}] {} {}",
                    steps.len(),
                    format.name(),
                    step.args.join(" ")
                ),
            );
        }

        if opts.dry_run {
            reports.push(StepReport {
                index,
                format: format.name().to_string(),
                bin: bin_display,
                source: source.as_str().to_string(),
                args: step.args.clone(),
                dry_run: true,
                ok: true,
                exit_code: None,
                stdout: None,
                stderr: None,
                error: None,
            });
            continue;
        }

        let mut cmd = Command::new(&bin);
        cmd.args(&step.args);
        let (ok, exit_code, stdout, stderr, error) = if opts.json {
            match cmd.output() {
                Ok(out) => (
                    out.status.success(),
                    out.status.code(),
                    Some(String::from_utf8_lossy(&out.stdout).to_string()),
                    Some(String::from_utf8_lossy(&out.stderr).to_string()),
                    None,
                ),
                Err(e) => (
                    false,
                    None,
                    None,
                    None,
                    Some(format!("执行 {} 失败: {e}", bin.display())),
                ),
            }
        } else {
            match cmd.status() {
                Ok(st) => (st.success(), st.code(), None, None, None),
                Err(e) => (
                    false,
                    None,
                    None,
                    None,
                    Some(format!("执行 {} 失败: {e}", bin.display())),
                ),
            }
        };

        reports.push(StepReport {
            index,
            format: format.name().to_string(),
            bin: bin_display,
            source: source.as_str().to_string(),
            args: step.args.clone(),
            dry_run: false,
            ok,
            exit_code,
            stdout,
            stderr,
            error,
        });

        if !(ok || opts.force || step.continue_on_error) {
            stopped_at = Some(index);
            break;
        }
    }

    let failed = reports.iter().filter(|r| !r.ok).count();
    let report = BatchReport {
        ok: failed == 0,
        total: steps.len(),
        succeeded: reports.iter().filter(|r| r.ok).count(),
        failed,
        stopped_at,
        dry_run: opts.dry_run,
        duration_ms: started.elapsed().as_millis() as u64,
        steps: reports,
    };
    Ok(report)
}

/// 批量模式状态行 → stderr（`--json` 时也输出 JSON 事件行，与其他 CLI 契约一致）
fn status(opts: &BatchOpts, msg: &str) {
    if opts.json {
        eprintln!("{}", json!({"event": "status", "message": msg}));
    } else {
        eprintln!("{msg}");
    }
}

/// 脚本来源：文件（`.` 开头为 JSON，其余为文本；`-` 读 stdin）
pub fn load_script(source: &str, commands: Option<&str>) -> Result<Vec<Step>, CliError> {
    if let Some(c) = commands {
        // 内联命令：兼容 `{"commands":[…]}`、裸数组、以及单行文本
        let trimmed = c.trim_start();
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return parse_script(c);
        }
        return parse_script_text(c);
    }
    let content = if source == "-" {
        let mut buf = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)
            .map_err(|e| CliError::with_code("io", format!("读取 stdin 失败: {e}")))?;
        buf
    } else {
        let p = Path::new(source);
        if !p.is_file() {
            return Err(CliError::new(format!("脚本文件不存在: {source}"))
                .suggest("写脚本文件，或 `--commands '<json>'` 内联给命令"));
        }
        std::fs::read_to_string(p)
            .map_err(|e| CliError::with_code("io", format!("读取脚本失败: {e}")))?
    };
    // 内容自适应：`{`/`[` 开头按 JSON，其余按文本行（扩展名只是提示，不做强绑定）
    parse_script(&content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn text_script_skips_comments_and_blanks() {
        let steps = parse_script_text(
            "# 准备\nword unpack a.docx -o prod/\n\n   \nexcel import d.csv -o b.xlsx\n",
        )
        .unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].format, Some(Format::Word));
        assert_eq!(steps[0].args, s(&["unpack", "a.docx", "-o", "prod/"]));
        assert_eq!(steps[1].format, Some(Format::Excel));
    }

    #[test]
    fn text_script_without_format_token_infers_later() {
        let steps = parse_script_text("view thesis.docx / text\n").unwrap();
        assert_eq!(steps[0].format, None);
        assert_eq!(dispatch::infer_format(&steps[0].args), Some(Format::Word));
    }

    #[test]
    fn quoted_arguments_survive_splitting() {
        let tokens = split_shellish(r#"edit prod/ '/part[1]' set --prop "text=两 个词""#).unwrap();
        assert_eq!(
            tokens,
            s(&["edit", "prod/", "/part[1]", "set", "--prop", "text=两 个词"])
        );
        assert!(split_shellish("view 'unclosed").is_err());
    }

    #[test]
    fn json_script_accepts_object_steps() {
        let v: Value = serde_json::from_str(
            r#"{"version":1,"commands":[
                {"format":"word","args":["unpack","a.docx","-o","prod/"]},
                {"format":"ppt","args":["view","d.pptx","/","text"],"continue_on_error":true},
                {"args":["extract","text","r.pdf","--format","markdown"]}
            ]}"#,
        )
        .unwrap();
        let steps = parse_steps_json(&v).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].format, Some(Format::Word));
        assert!(steps[1].continue_on_error);
        assert_eq!(steps[2].format, None);
        assert_eq!(dispatch::infer_format(&steps[2].args), Some(Format::Pdf));
    }

    #[test]
    fn json_step_string_form_and_errors() {
        let steps = parse_steps_json(&json!(["word view prod/ / text"])).unwrap();
        assert_eq!(steps[0].format, Some(Format::Word));
        assert_eq!(steps[0].args, s(&["view", "prod/", "/", "text"]));

        let err = parse_steps_json(&json!([{"format": "word"}])).unwrap_err();
        assert!(err.message.contains("args"), "{}", err.message);

        let err = parse_steps_json(&json!([{"format": "nope", "args": ["view"]}])).unwrap_err();
        assert!(err.message.contains("未知格式"), "{}", err.message);

        let err = parse_steps_json(&json!([42])).unwrap_err();
        assert!(err.message.contains("对象或字符串"), "{}", err.message);
    }

    #[test]
    fn script_autodetect_json_vs_text() {
        let json_steps = parse_script(r#"[{"format":"pdf","args":["qa","a.pdf"]}]"#).unwrap();
        assert_eq!(json_steps.len(), 1);
        let text_steps = parse_script("pdf qa a.pdf\n").unwrap();
        assert_eq!(text_steps.len(), 1);
        assert_eq!(text_steps[0].format, Some(Format::Pdf));
    }
}
