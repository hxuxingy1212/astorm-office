//! astorm-office：astorm-office 四格式 CLI 的统一入口。
//!
//! 本 CLI **不重新实现文档能力**，只做分发与批处理：
//!
//! ```text
//! astorm-office <格式> <命令...>              # 转发给对应底层 CLI
//! astorm-office <命令...>                     # 省略格式：从参数里的文件扩展名推断
//! astorm-office formats                      # 列出格式别名与底层 CLI 解析路径
//! astorm-office batch <脚本|-> [--force]      # 批量执行多条命令（文本或 JSON 脚本）
//! ```
//!
//! 格式别名：`word|docx|doc|docs` · `excel|xlsx|xls|sheet` · `ppt|pptx|slides` · `pdf`。
//! 转发时 stdout/stderr/退出码原样透传（子命令的 `--json` 等参数照常生效）；
//! 只有 `batch` 的旗标（`--force`/`--dry-run`/`--json`/`--quiet`）属于本 CLI 自己。
//!
//! 契约与其他四 CLI 一致（docs/cli-conventions.md）：stdout 只出数据、状态走 stderr、
//! 退出码 0 成功 / 1 失败 / 2 用法错误。

mod batch;
mod dispatch;

use clap::{Args, Parser, Subcommand};
use dispatch::Format;
use office_core::{output, CliError, Output, EXIT_FAILURE, EXIT_SUCCESS};
use serde_json::json;

/// 统一 CLI 的用法说明（帮助尾注）
const AFTER_HELP: &str = "\
格式别名：word|docx|doc|docs · excel|xlsx|xls|sheet · ppt|pptx|slides|presentation · pdf
（也接受底层二进制名：json2docx / json2xlsx / json2pptx / json2pdf）

示例：
  astorm-office word unpack thesis.docx -o prod/      # → json2docx unpack …
  astorm-office word view prod/ / text                # → json2docx view …（整篇 Markdown）
  astorm-office excel import data.csv -o book.xlsx    # → json2xlsx import …
  astorm-office ppt view deck.pptx / layout           # → json2pptx view …
  astorm-office pdf extract text r.pdf --format markdown
  astorm-office view thesis.docx / text               # 省略格式：按 .docx 自动分发
  astorm-office formats                               # 看格式别名与底层 CLI 路径
  astorm-office batch steps.txt                       # 批量执行（一行一条命令）
  astorm-office batch --commands '[{\"format\":\"word\",\"args\":[\"unpack\",\"a.docx\",\"-o\",\"prod/\"]}]'

转发模式：格式后面的参数原封不动交给底层 CLI（含 --json/--quiet/-o 等）。
批处理：脚本每行一条命令（'#' 注释），或用 JSON {\"commands\":[{\"format\",…,\"args\":[…]}]}。";

#[derive(Parser)]
#[command(
    name = "astorm-office",
    version,
    about = "astorm-office 统一 CLI：按格式分发到 json2docx/json2xlsx/json2pptx/json2pdf，并支持批量执行",
    after_help = AFTER_HELP,
    arg_required_else_help = true,
    disable_help_subcommand = true
)]
struct Cli {
    /// 强制指定格式（也可写成第一个位置参数：astorm-office word …）
    #[arg(short = 'f', long = "format", global = true)]
    format: Option<String>,

    #[command(subcommand)]
    command: Option<Top>,
}

#[derive(Subcommand)]
enum Top {
    /// 列出格式别名与各底层 CLI 的解析路径（--json 机器可读）
    Formats(FormatsArgs),
    /// 批量执行：顺序跑完脚本里的多条命令，输出逐步报告
    Batch(BatchArgs),
    /// 任意格式与命令（转发给底层 CLI，见 --help 示例）
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Args)]
struct FormatsArgs {
    /// 结构化输出
    #[arg(long)]
    json: bool,
}

#[derive(Args)]
struct BatchArgs {
    /// 脚本文件（`.json` 走 JSON，其余按文本行；`-` 读 stdin）
    script: Option<String>,
    /// 同上，显式写法
    #[arg(long = "input-file")]
    input_file: Option<String>,
    /// 内联脚本（JSON 数组 / {\"commands\":[…]}/ 多行文本）
    #[arg(long)]
    commands: Option<String>,
    /// 遇错继续（默认停在第 1 个失败处）
    #[arg(long)]
    force: bool,
    /// 只打印将执行的命令，不真的执行
    #[arg(long = "dry-run")]
    dry_run: bool,
    /// 结构化报告：捕获各步输出，stdout 只输出报告 JSON
    #[arg(long)]
    json: bool,
    /// 静默：不输出逐步状态（不改 stdout）
    #[arg(long)]
    quiet: bool,
}

fn main() {
    let cli = Cli::parse();
    let code = match run(&cli) {
        Ok(code) => code,
        Err(e) => {
            let out = Output::new(false, false, false);
            out.error_full(&e.code, &e.message, None, e.suggestion.as_deref());
            EXIT_FAILURE
        }
    };
    std::process::exit(code);
}

fn run(cli: &Cli) -> Result<i32, CliError> {
    match &cli.command {
        Some(Top::Formats(args)) => formats(args),
        Some(Top::Batch(args)) => run_batch(args),
        Some(Top::External(args)) => passthrough(cli.format.as_deref(), args),
        None => Ok(EXIT_SUCCESS), // clap 的 arg_required_else_help 已挡住
    }
}

/// `formats`：格式 → 底层 CLI 的映射与解析结果（诊断用）
fn formats(args: &FormatsArgs) -> Result<i32, CliError> {
    let mut rows = Vec::new();
    for f in Format::all() {
        let resolved = dispatch::resolve_bin(f);
        let (path, source, ok) = match &resolved {
            Ok((p, s)) => (p.display().to_string(), s.as_str().to_string(), true),
            Err(e) => (e.message.clone(), String::new(), false),
        };
        rows.push(json!({
            "format": f.name(),
            "aliases": dispatch::alias_hint(f),
            "bin": f.bin_name(),
            "env": f.env_var(),
            "extensions": f.extensions(),
            "path": path,
            "source": source,
            "ok": ok,
        }));
    }
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "formats": rows,
                "usage": {
                    "dispatch": "astorm-office <格式> <命令...>",
                    "infer": "astorm-office <命令...>（从文件扩展名推断格式）",
                    "batch": "astorm-office batch <脚本|->"
                }
            }))
            .unwrap_or_default()
        );
    } else {
        println!("格式分发表（astorm-office <格式> <命令...>）：\n");
        println!("  {:<28} {:<12} 解析路径", "格式（别名）", "底层 CLI");
        for r in &rows {
            let mark = if r["ok"].as_bool().unwrap_or(false) {
                ""
            } else {
                "  ← 未找到"
            };
            println!(
                "  {:<28} {:<12} {}{}",
                r["aliases"].as_str().unwrap_or(""),
                r["bin"].as_str().unwrap_or(""),
                r["path"].as_str().unwrap_or(""),
                mark
            );
        }
        println!("\n可用子命令：formats（本表） · batch（批量执行） · <格式> <命令...>（转发）");
        println!("环境变量覆盖：ASTORM_JSON2DOCX / ASTORM_JSON2XLSX / ASTORM_JSON2PPTX / ASTORM_JSON2PDF");
    }
    Ok(EXIT_SUCCESS)
}

/// `<格式> <命令...>`：转发（stdio 与退出码原样透传）
fn passthrough(explicit: Option<&str>, args: &[String]) -> Result<i32, CliError> {
    if args.is_empty() {
        return Err(CliError::new("缺少要转发的命令").suggest(
            "用法：astorm-office <格式> <命令...>，如 astorm-office word view a.docx / text",
        ));
    }
    // 第一个参数是格式别名 → 它是格式；否则整串都是底层 CLI 的参数，格式走 -f 或推断
    let (token, rest) = match dispatch::parse_format(&args[0]) {
        Some(_) => (Some(args[0].as_str()), &args[1..]),
        None => (explicit, args),
    };
    // -f 与位置格式同时给出且不一致：明说，不猜
    if let Some(tok) = token {
        let from_token = dispatch::parse_format(tok);
        if let Some(f) = explicit.and_then(dispatch::parse_format) {
            if from_token.is_some() && from_token != Some(f) {
                return Err(CliError::new(format!(
                    "格式冲突：位置参数 {} 与 --format {} 不一致",
                    tok,
                    f.name()
                ))
                .suggest("只保留一个，或删掉 --format/-f"));
            }
        }
    }
    let format = dispatch::resolve_format(token, rest)?;
    let code = dispatch::run_passthrough(format, rest)?;
    Ok(code)
}

/// `batch`：批量执行
fn run_batch(args: &BatchArgs) -> Result<i32, CliError> {
    let source = args
        .script
        .clone()
        .or_else(|| args.input_file.clone())
        .unwrap_or_else(|| "-".to_string());
    // 交互式终端下又没有脚本来源 → 直接给用法，别干等着读 stdin
    if args.commands.is_none()
        && args.script.is_none()
        && args.input_file.is_none()
        && std::io::IsTerminal::is_terminal(&std::io::stdin())
    {
        return Err(CliError::new("batch 需要脚本来源")
            .suggest("astorm-office batch steps.txt / --input-file steps.json / --commands '<json>' / 管道输入"));
    }
    let steps = batch::load_script(&source, args.commands.as_deref())?;
    if steps.is_empty() {
        return Err(CliError::new("脚本里没有可执行的命令").suggest(
            "文本脚本一行一条：`word unpack a.docx -o prod/`；JSON 脚本用 commands 数组",
        ));
    }
    let opts = batch::BatchOpts {
        force: args.force,
        dry_run: args.dry_run,
        json: args.json,
        quiet: args.quiet,
    };
    let report = batch::run(&steps, &opts)?;

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).unwrap_or_default()
        );
    } else {
        // 默认模式：子命令输出已直通，这里只补一份人读汇总（走 stderr）
        if !args.quiet {
            let mut err = std::io::stderr().lock();
            use std::io::Write;
            let _ = writeln!(
                err,
                "批处理{}：{} 步，成功 {}，失败 {}{}（{} ms）",
                if opts.dry_run { "（空跑）" } else { "" },
                report.total,
                report.succeeded,
                report.failed,
                match report.stopped_at {
                    Some(i) => format!("，停在第 {i} 步"),
                    None => String::new(),
                },
                report.duration_ms
            );
            for s in report.steps.iter().filter(|s| !s.ok) {
                let _ = writeln!(
                    err,
                    "  第 {} 步失败（{} {}）：{}",
                    s.index,
                    s.format,
                    s.args.join(" "),
                    s.error.clone().unwrap_or_else(|| format!(
                        "退出码 {}",
                        s.exit_code
                            .map(|c| c.to_string())
                            .unwrap_or_else(|| "?".into())
                    ))
                );
            }
        }
    }
    Ok(if report.ok {
        EXIT_SUCCESS
    } else {
        EXIT_FAILURE
    })
}

// 让 `output` 常量在未使用时不报警（保持与 office-core 契约一致的引用点）
#[allow(dead_code)]
const _EXIT_CODES: (i32, i32) = (output::EXIT_SUCCESS, output::EXIT_ISSUES);
