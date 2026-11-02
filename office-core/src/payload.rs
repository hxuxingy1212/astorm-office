//! 大 JSON / 长文本载荷的统一加载约定：CLI 参数一律收束为**文件路径**，
//! 不再把大 JSON / 长文本内联进命令行（agent 集成时省上下文，也绕开 argv 长度上限）。
//!
//! 约定：
//! - `-` = stdin（读文本/JSON 来源）；
//! - `@file` = 从文件读长文本（仅 `--prop` 值；`@@` 转义字面 `@`，`@-` = stdin）。
//!
//! [`expand_props`] 只应在 **argv 入口**调用（main 里解析 `--prop` 之前）；
//! batch 脚本 / serve / mcp 通道的载荷本身已在文件或结构化管道里，不再展开。

use crate::CliError;
use serde_json::Value;
use std::path::Path;

/// 读文本来源：`-` = stdin，否则为文件路径。
pub fn read_text(spec: &str) -> Result<String, CliError> {
    if spec == "-" {
        let mut s = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut s)
            .map_err(|e| CliError::with_code("io", format!("读取 stdin 失败: {e}")))?;
        return Ok(s);
    }
    let path = Path::new(spec);
    if !path.is_file() {
        // 迁移提示：老用法把内联 JSON 当参数传入（`{`/`[` 开头），明说不再支持
        let t = spec.trim_start();
        if t.starts_with('{') || t.starts_with('[') {
            return Err(CliError::new(format!("参数不是文件: {spec}")).suggest(
                "该参数现在是文件路径（不再接受内联 JSON）：先把 JSON 写入文件，\
                 或用 `-` 从 stdin 传入后再调用",
            ));
        }
        return Err(CliError::with_code("io", format!("文件不存在: {spec}")));
    }
    std::fs::read_to_string(path)
        .map_err(|e| CliError::with_code("io", format!("读取 {spec} 失败: {e}")))
}

/// 读 JSON 来源：文件路径或 `-`（stdin），内容必须是合法 JSON。
pub fn read_json(spec: &str) -> Result<Value, CliError> {
    let raw = read_text(spec)?;
    serde_json::from_str(&raw).map_err(|e| {
        CliError::with_code("json", format!("JSON 解析失败: {e}"))
            .suggest(format!("确认 {spec} 的内容是合法 JSON（`-` 表示 stdin）"))
    })
}

/// `--prop k=v` 的值加载：`@file` / `@-`（stdin）读长文本，其余原样。
fn expand_prop_value(value: &str) -> Result<String, CliError> {
    let rest = match value.strip_prefix('@') {
        Some(rest) => rest,
        None => return Ok(value.to_string()),
    };
    if let Some(literal) = rest.strip_prefix('@') {
        return Ok(format!("@{literal}"));
    }
    read_text(rest)
}

/// 展开 argv 里 `--prop k=v` 列表的值（仅 CLI 入口调用，见模块注释）。
pub fn expand_props(props: &[String]) -> Result<Vec<String>, CliError> {
    props
        .iter()
        .map(|p| match p.split_once('=') {
            Some((k, v)) if v.starts_with('@') => Ok(format!("{k}={}", expand_prop_value(v)?)),
            _ => Ok(p.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_text_rejects_inline_json_with_migration_hint() {
        let err = read_json(r#"{"a":1}"#).unwrap_err();
        assert_eq!(err.code, "invalid_input");
        assert!(
            err.suggestion.as_deref().unwrap().contains("文件路径"),
            "{err}"
        );
        assert_eq!(read_text("/nonexistent/plain.txt").unwrap_err().code, "io");
    }

    #[test]
    fn expand_props_reads_long_text_from_file() {
        let dir = std::env::temp_dir().join(format!("astorm-payload-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("body.txt");
        std::fs::write(&f, "很长的正文").unwrap();
        let path = f.to_str().unwrap();

        let expanded = expand_props(&[
            format!("text=@{path}"),
            "bold=true".into(),
            "sig=@@zhang".into(),
        ])
        .unwrap();
        assert_eq!(expanded[0], "text=很长的正文");
        assert_eq!(expanded[1], "bold=true");
        assert_eq!(expanded[2], "sig=@zhang");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
