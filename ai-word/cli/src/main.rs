//! json2docx：JSON 与 DOCX 双向转换工具
//!
//! 无状态设计（对齐 ai-ppt / ai-excel）：命令之间不共享隐藏状态。
//! 输出契约与 astorm-office 另两个 CLI 一致（docs/cli-conventions.md）：
//! stdout 只输出数据，状态/进度走 stderr；`--json` 时写盘命令向 stdout 输出
//! 结果 JSON，stderr 的事件与错误也结构化为 JSON 行。
//!
//! 子命令：unpack / repack / view / edit / render / validate / dump / raw /
//! raw-set / schema / templates / extract / merge / mcp / serve

mod batch;
mod capabilities;
mod dump;
mod edit;
mod help;
mod mcp;
mod path;
mod product;
mod render;
mod serve;
mod view;

use json2docx::legacy;
use office_core::CliError;

type Result<T> = std::result::Result<T, CliError>;
use clap::{Parser, Subcommand};
use office_core::{path_str, same_file, with_stem_ext, Output, EXIT_FAILURE, EXIT_ISSUES};
use serde_json::json;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "json2docx",
    version,
    about = "JSON 与 DOCX 双向转换工具",
    disable_help_subcommand = true
)]
struct Cli {
    /// 输出路径（unpack/repack/edit/render/extract/merge/raw-set 共用）
    #[arg(short = 'o', long, global = true)]
    output: Option<PathBuf>,
    /// 结构化输出：写盘命令向 stdout 输出结果 JSON，状态/错误改走 JSON 行
    #[arg(long, global = true)]
    json: bool,
    /// 静默：不输出状态信息
    #[arg(long, global = true)]
    quiet: bool,
    /// 详细输出
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 旧版 Word（.doc, MS-DOC）→ 新格式 .docx（auto 检测到 LibreOffice 用之，保真最高；rust 为纯 Rust rwml）
    Convert {
        /// 输入的旧版文件（.doc）
        input: PathBuf,
        /// 转换引擎：auto / libreoffice / rust
        #[arg(long, default_value = "auto")]
        engine: String,
    },
    /// 将 DOCX 解包为可编辑中间产物（document.json + word/parts/*.json + word/media/）
    Unpack {
        /// 输入的 DOCX 文件路径
        input: PathBuf,
        /// 输出单个自包含 JSON（分片内联、媒体转 data URI；预览/交付形态，编辑请用目录形态）
        #[arg(long)]
        inline: bool,
    },
    /// 从产物目录重建 DOCX（自动校验；有校验问题时退出码 3）
    Repack {
        /// 输入的产物目录（含 document.json）
        input: PathBuf,
    },
    /// 第一层视图：text（文本+媒体）/ layout（块结构树）
    View {
        /// 输入的 DOCX 或产物目录
        input: PathBuf,
        /// 元素路径（如 /part[1]）
        path: String,
        /// 视图模式
        mode: ViewMode,
    },
    /// 第二层精准修改：get / set / add / remove
    Edit {
        /// 输入的 DOCX 或产物目录
        input: PathBuf,
        /// 元素路径（如 /part[1]/paragraph[2]）
        path: String,
        #[command(subcommand)]
        action: EditAction,
    },
    /// 将分片渲染/导出为 HTML / PDF / PNG
    Render {
        /// 输入的 DOCX 或产物目录
        input: PathBuf,
        /// 元素路径（如 /part[1]，`/` 表示整篇）
        path: String,
        /// 输出格式（默认按扩展名推断，其次 html）
        #[arg(long, value_enum, default_value_t = RenderFormat::Auto)]
        format: RenderFormat,
    },
    /// 包结构校验（.docx 或产物目录），问题非空时退出码为 3
    Validate {
        /// 输入的 DOCX 文件或产物目录
        input: PathBuf,
    },
    /// 序列化为可回放的编辑指令（batch JSON）
    Dump {
        /// 输入的 DOCX 或产物目录
        input: PathBuf,
    },
    /// 原始 OPC：读取包内部件（字节原样输出，字面路径）
    Raw {
        input: PathBuf,
        part: String,
    },
    /// 原始 OPC：整部件替换（-o 必填，不覆盖原文件；`--file -` 读 stdin）
    RawSet {
        input: PathBuf,
        part: String,
        /// 新内容文件（- 表示 stdin）
        #[arg(long)]
        file: String,
    },
    /// 打印 JSON Schema 文档指引（--json 时输出内置 schema 指引 JSON）
    Schema,
    /// 列出所有可用模板
    Templates,
    /// 从一份 DOCX 提取风格样式模板（template.json + TEMPLATE.md）
    Extract {
        /// 输入的 DOCX 文件路径
        input: PathBuf,
        /// 模板名（默认取文件名）
        #[arg(long)]
        name: Option<String>,
    },
    /// 启动 MCP 服务（stdio JSON-RPC）
    /// 回放 dump 风格编辑指令（默认遇错即停；--force 跳过继续）
    Batch {
        /// 输入的产物目录（.docx 请先 unpack）
        input: PathBuf,
        /// 指令 JSON 文件（- 表示 stdin）
        #[arg(long)]
        input_file: Option<String>,
        /// 内联指令 JSON
        #[arg(long)]
        commands: Option<String>,
        /// 遇错继续（报告仍标 ok=false，退出码 1）
        #[arg(long)]
        force: bool,
    },
    /// 能力速查：help / help <元素>；--json 输出机器可读能力 schema
    Help {
        /// 元素名（heading/paragraph/list/table/image/formula/code/quote/caption/toc/...）
        topic: Option<String>,
    },
    Mcp,
    /// 常驻编辑服务：加载一次，stdin 逐行 JSON 操作
    Serve {
        /// 输入的 DOCX 或产物目录
        input: PathBuf,
    },
    /// 模板数据填充：把 {{key}} 替换为 JSON 数据
    Merge {
        /// 输入的 DOCX 或产物目录
        input: PathBuf,
        /// 数据 JSON（对象字符串）
        #[arg(short = 'd', long)]
        data: String,
    },
}

#[derive(clap::ValueEnum, Clone)]
enum ViewMode {
    /// 文本 + 媒体
    Text,
    /// 块结构树
    Layout,
}

/// render 输出格式
#[derive(clap::ValueEnum, Clone, PartialEq)]
enum RenderFormat {
    /// 按输出扩展名推断
    Auto,
    Html,
    Pdf,
    Png,
}

#[derive(Subcommand)]
enum EditAction {
    /// 读取节点状态（JSON）
    Get,
    /// 修改属性（--prop k=v，可重复）
    Set {
        #[arg(long)]
        prop: Vec<String>,
    },
    /// 在分片下新增元素
    Add {
        /// 元素类型（heading/paragraph/list/table/image/formula/code/quote/caption/toc/page_break/...）
        #[arg(long)]
        r#type: String,
        #[arg(long)]
        prop: Vec<String>,
    },
    /// 删除路径指向的元素
    Remove,
}

/// 读取 batch 指令：--input-file > --commands > stdin
fn read_batch_commands(input_file: Option<&str>, commands: Option<&str>) -> Result<String> {
    if let Some(f) = input_file {
        if f == "-" {
            use std::io::Read;
            let mut s = String::new();
            std::io::stdin()
                .read_to_string(&mut s)
                .map_err(|e| CliError::with_code("io", format!("读取 stdin 失败: {e}")))?;
            return Ok(s);
        }
        return std::fs::read_to_string(f)
            .map_err(|e| CliError::with_code("io", format!("读取 {f} 失败: {e}")));
    }
    if let Some(c) = commands {
        return Ok(c.to_string());
    }
    use std::io::Read;
    let mut s = String::new();
    std::io::stdin()
        .read_to_string(&mut s)
        .map_err(|e| CliError::with_code("io", format!("读取 stdin 失败: {e}")))?;
    if s.trim().is_empty() {
        return Err(CliError::new("缺少 batch 指令")
            .suggest("用 --input-file <file> / --commands '<json>' 或从 stdin 传入"));
    }
    Ok(s)
}

fn main() {
    let cli = Cli::parse();
    let out = Output::new(cli.json, cli.quiet, cli.verbose);
    let code = match run(&cli, &out) {
        Ok(code) => code,
        Err(e) => {
            out.error_full(&e.code, &e.message, None, e.suggestion.as_deref());
            EXIT_FAILURE
        }
    };
    std::process::exit(code);
}

fn run(cli: &Cli, out: &Output) -> Result<i32> {
    let output = &cli.output;
    match &cli.command {
        Command::Convert { input, engine } => {
            if !legacy::is_cfb(input) {
                return Err(CliError::new("convert 需要 OLE2/CFB 旧版文件（.doc）"));
            }
            let out_docx = output
                .clone()
                .unwrap_or_else(|| with_stem_ext(input, "docx"));
            let use_lo = match engine.as_str() {
                "libreoffice" => true,
                "rust" => false,
                "auto" => office_core::soffice::available(),
                other => {
                    return Err(CliError::new(format!(
                        "未知引擎 {other:?}（可选 auto / libreoffice / rust）"
                    )))
                }
            };
            if use_lo {
                office_core::soffice::convert(input, "docx", &out_docx)?;
                out.status(&format!(
                    "已转换: {} → {}（引擎: libreoffice）",
                    input.display(),
                    out_docx.display()
                ));
            } else {
                let bytes = legacy::doc_to_docx_bytes(input)?;
                std::fs::write(&out_docx, bytes)?;
                out.status(&format!(
                    "已转换: {} → {}（引擎: rust）",
                    input.display(),
                    out_docx.display()
                ));
            }
            out.emit_result(&json!({
                "ok": true,
                "input": input.display().to_string(),
                "output": out_docx.display().to_string(),
                "engine": if use_lo { "libreoffice" } else { "rust" },
            }))?;
        }
        Command::Unpack { input, inline } => {
            if *inline {
                let out_file = output
                    .clone()
                    .unwrap_or_else(|| input.with_extension("inline.json"));
                let r = office_core::inline::unpack_inline(input, &out_file, |i, o| {
                    json2docx::unpack(&path_str(i), &path_str(o))
                        .map(|_| ())
                        .map_err(CliError::from)
                })?;
                out.status(&format!(
                    "已内联解包: {} ({} 个分片)",
                    r.output.display(),
                    r.shards
                ));
                out.emit_result(&json!({
                    "ok": true,
                    "output": r.output.display().to_string(),
                    "parts": r.shards,
                    "inline": true
                }))?;
            } else {
                let out_dir = output.clone().unwrap_or_else(|| input.with_extension(""));
                let r = json2docx::unpack(&path_str(input), &path_str(&out_dir))?;
                out.status(&format!("已解包: {} ({} 个分片)", r.path, r.parts));
                out.emit_result(&json!({ "ok": true, "output": r.path, "parts": r.parts }))?;
            }
        }
        Command::Repack { input } => {
            let out_docx = output
                .clone()
                .unwrap_or_else(|| with_stem_ext(input, "docx"));
            let r = json2docx::repack(&path_str(input), &path_str(&out_docx))?;
            let issues = json2docx::validate_docx(&out_docx);
            let n = issues.len();
            if n == 0 {
                out.status(&format!(
                    "已生成: {} ({} 个分片) · 校验通过",
                    r.path, r.parts
                ));
            } else {
                out.status(&format!(
                    "已生成: {} ({} 个分片) · 校验发现 {} 个问题",
                    r.path, r.parts, n
                ));
                for i in &issues {
                    out.status(&format!("  - {i}"));
                }
            }
            out.emit_result(&json!({
                "ok": true,
                "output": r.path,
                "parts": r.parts,
                "issues": issues,
                "issue_count": n,
            }))?;
            if n > 0 {
                return Ok(EXIT_ISSUES);
            }
        }
        Command::View { input, path, mode } => {
            let (root, _tmp) = product::resolve_input(input)?;
            let doc = product::load_document(&root)?;
            let segs = path::parse(path)?;
            let loc = path::resolve(&doc, &segs)?;
            let value = match mode {
                ViewMode::Text => view::view_text(&doc, loc.part),
                ViewMode::Layout => view::view_layout(&doc, loc.part),
            };
            let issues = json2docx::validate_product(&root);
            let warnings = json2docx::content_warnings(&doc);
            let value = if let serde_json::Value::Object(mut m) = value {
                m.insert("issues".to_string(), serde_json::json!(issues));
                m.insert("warnings".to_string(), serde_json::json!(warnings));
                serde_json::Value::Object(m)
            } else {
                value
            };
            out.emit_json(&value)?;
        }
        Command::Edit {
            input,
            path,
            action,
        } => {
            let (root, tmp) = product::resolve_input(input)?;
            let is_write = matches!(
                action,
                EditAction::Set { .. } | EditAction::Add { .. } | EditAction::Remove
            );
            if is_write && tmp.is_some() && output.is_none() {
                return Err(
                    "修改 .docx 需要 -o <输出.docx>（无状态模式不原地改写输入文件）；\
                     或先 `unpack` 到产物目录，再对产物目录执行 edit"
                        .into(),
                );
            }
            let mut doc = product::load_document(&root)?;
            let segs = path::parse(path)?;
            let loc = path::resolve(&doc, &segs)?;
            let value = match action {
                EditAction::Get => edit::get(&doc, &loc)?,
                EditAction::Set { prop } => {
                    let props = edit::parse_props(prop)?;
                    let r = edit::set(&mut doc, &loc, &props)?;
                    product::save_document(&root, &doc)?;
                    r
                }
                EditAction::Add { r#type, prop } => {
                    let props = edit::parse_props(prop)?;
                    let r = edit::add(&mut doc, &loc, r#type, &props)?;
                    product::save_document(&root, &doc)?;
                    r
                }
                EditAction::Remove => {
                    let r = edit::remove(&mut doc, &loc)?;
                    product::save_document(&root, &doc)?;
                    r
                }
            };
            if let Some(out_docx) = output {
                json2docx::repack(&path_str(&root), &path_str(out_docx))?;
                out.status(&format!("已生成: {}", out_docx.display()));
            }
            out.emit_json(&value)?;
        }
        Command::Batch {
            input,
            input_file,
            commands,
            force,
        } => {
            if !product::is_product_dir(input) {
                return Err(CliError::new("batch 需要产物目录（.docx 请先 unpack）")
                    .suggest("先 json2docx unpack <文件.docx> -o <目录>"));
            }
            let raw = read_batch_commands(input_file.as_deref(), commands.as_deref())?;
            let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
                CliError::new(format!("指令 JSON 解析失败: {e}"))
                    .suggest(r#"指令格式: {"commands":[{"op":"set","path":"/part[1]/paragraph[1]","props":{"text":"新文本"}}]}"#)
            })?;
            let steps = office_core::batch::parse_steps(&v).map_err(CliError::new)?;
            let mut doc = product::load_document(input)?;
            let report = batch::run_steps(&mut doc, &steps, !force);
            let failed = report["failed"].as_u64().unwrap_or(0);
            let applied = report["applied"].as_u64().unwrap_or(0);
            if failed == 0 && applied > 0 {
                product::save_document(input, &doc)?;
            }
            out.emit_json(&report)?;
            if failed > 0 {
                return Ok(EXIT_FAILURE);
            }
        }
        Command::Help { topic } => {
            help::run(topic.as_deref(), out)?;
        }
        Command::Mcp => {
            mcp::run()?;
        }
        Command::Serve { input } => {
            serve::run(input)?;
        }
        Command::Merge { input, data } => {
            let (root, _tmp) = product::resolve_input(input)?;
            let mut doc = product::load_document(&root)?;
            let jd: serde_json::Value = serde_json::from_str(data)
                .map_err(|e| CliError::new(format!("数据 JSON 解析失败: {e}")))?;
            let n = json2docx::merge_document(&mut doc, &jd);
            let out_docx = output.clone().unwrap_or_else(|| {
                let mut p = input.clone();
                let stem = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "out".into());
                p.set_file_name(format!("{stem}_merged.docx"));
                p
            });
            product::save_document(&root, &doc)?;
            json2docx::repack(&path_str(&root), &path_str(&out_docx))?;
            out.status(&format!("已生成: {}（替换 {} 处）", out_docx.display(), n));
            out.emit_result(&json!({
                "ok": true,
                "output": out_docx.display().to_string(),
                "replaced": n,
            }))?;
        }
        Command::Render {
            input,
            path,
            format,
        } => {
            let (root, _tmp) = product::resolve_input(input)?;
            let doc = product::load_document(&root)?;
            let all = path.trim() == "/";
            let loc = if all {
                None
            } else {
                let segs = path::parse(path)?;
                Some(path::resolve(&doc, &segs)?)
            };

            // 目标格式：显式 > 扩展名 > html
            let ext = output
                .as_ref()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let fmt = match format {
                RenderFormat::Auto => match ext.as_str() {
                    "pdf" => RenderFormat::Pdf,
                    "png" => RenderFormat::Png,
                    _ => RenderFormat::Html,
                },
                other => other.clone(),
            };
            let out_path = output.clone().unwrap_or_else(|| {
                let stem = input
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "document".to_string());
                let suffix = match fmt {
                    RenderFormat::Pdf => "pdf",
                    RenderFormat::Png => "png",
                    _ => "html",
                };
                let tag = loc
                    .as_ref()
                    .map(|l| format!("_part{}", l.part + 1))
                    .unwrap_or_else(|| "_all".to_string());
                PathBuf::from(format!("{stem}{tag}.{suffix}"))
            });

            // 生成 HTML（整篇或单分片）
            let make_html = |to: &PathBuf| -> Result<()> {
                match &loc {
                    Some(l) => render::render_html(&doc, l.part, Some(&root), to),
                    None => render::render_all(&doc, Some(&root), to),
                }
            };

            // HTML 路径就地生成；PDF/PNG 需要 docx
            let needs_docx = matches!(fmt, RenderFormat::Pdf | RenderFormat::Png);
            if needs_docx {
                let (docx, guard) = ensure_docx(input.as_path(), &root)?;
                let result: Result<()> =
                    match fmt {
                        RenderFormat::Png => render::convert_with_soffice(&docx, &out_path)
                            .or_else(|e| {
                                let html = out_path.with_extension("html");
                                make_html(&html)?;
                                render::html_to_png_chrome(&html, &out_path).map_err(|e2| {
                                    CliError::new(format!("{e}；Chrome 回退也失败: {e2}"))
                                        .suggest("确认已安装 LibreOffice 或 Chrome 后重试")
                                })
                            }),
                        RenderFormat::Pdf => render::convert_with_soffice(&docx, &out_path)
                            .or_else(|e| {
                                let html = out_path.with_extension("html");
                                make_html(&html)?;
                                render::html_to_pdf_chrome(&html, &out_path).map_err(|e2| {
                                    CliError::new(format!("{e}；Chrome 回退也失败: {e2}"))
                                        .suggest("确认已安装 LibreOffice 或 Chrome 后重试")
                                })
                            }),
                        _ => render::convert_with_soffice(&docx, &out_path),
                    };
                drop(guard);
                result?;
            } else {
                make_html(&out_path)?;
            }
            let fmt_name = match fmt {
                RenderFormat::Pdf => "pdf",
                RenderFormat::Png => "png",
                _ => "html",
            };
            out.status(&format!("已渲染: {}", out_path.display()));
            out.emit_result(&json!({
                "ok": true,
                "output": out_path.display().to_string(),
                "format": fmt_name,
            }))?;
        }
        Command::Validate { input } => {
            let issues = if product::is_product_dir(input) {
                json2docx::validate_product(input)
            } else {
                json2docx::validate_docx(input)
            };
            let n = issues.len();
            out.status(&format!(
                "校验完成: {} 个问题",
                if n == 0 {
                    "无".to_string()
                } else {
                    n.to_string()
                }
            ));
            out.emit_json(&json!({ "issues": issues, "count": n }))?;
            if n > 0 {
                return Ok(EXIT_ISSUES);
            }
        }
        Command::Dump { input } => {
            let doc = load_document_readonly(input)?;
            out.emit_json(&dump::run(&doc))?;
        }
        Command::Raw { input, part } => {
            let content = office_core::opc::read_part(input, part)?
                .ok_or_else(|| CliError::new(format!("部件不存在: {part}")))?;
            match output {
                Some(o) => std::fs::write(o, content)?,
                None => {
                    use std::io::Write;
                    std::io::stdout().write_all(&content)?;
                }
            }
        }
        Command::RawSet { input, part, file } => {
            let out_docx = output.as_ref().ok_or_else(|| {
                "raw-set 写 DOCX 必须指定 -o <输出.docx>（不覆盖原文件）".to_string()
            })?;
            if same_file(input, out_docx) {
                return Err("-o 不能与输入文件相同：raw-set 不覆盖原文件"
                    .to_string()
                    .into());
            }
            let content = if file == "-" {
                use std::io::Read;
                let mut buf = Vec::new();
                std::io::stdin().read_to_end(&mut buf)?;
                buf
            } else {
                std::fs::read(file)?
            };
            office_core::opc::write_part(input, out_docx, part, &content)?;
            out.status(&format!("已替换部件: {} → {}", part, out_docx.display()));
            out.emit_result(
                &json!({ "ok": true, "replaced": part, "output": out_docx.display().to_string() }),
            )?;
        }
        Command::Schema => {
            if out.json_mode() {
                out.emit_json(&schemars::schema_for!(json2docx::model::Document))?;
            } else {
                println!("{}", SCHEMA_GUIDE);
            }
        }
        Command::Templates => {
            let presets: Vec<_> = json2docx::model::TemplatePreset::all()
                .iter()
                .map(|t| json!({ "name": t.name, "description": t.desc, "display": t.display }))
                .collect();
            if out.json_mode() {
                out.emit_json(&json!({ "templates": presets }))?;
            } else {
                println!("可用模板:");
                for t in &presets {
                    println!(
                        "  {:<16} {} ({})",
                        t["name"].as_str().unwrap_or(""),
                        t["description"].as_str().unwrap_or(""),
                        t["display"].as_str().unwrap_or("")
                    );
                }
            }
        }
        Command::Extract { input, name } => {
            let stem = input
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "template".to_string());
            let tname = name.clone().unwrap_or_else(|| stem.clone());
            let dir = output
                .clone()
                .unwrap_or_else(|| PathBuf::from(format!("{stem}_template")));
            std::fs::create_dir_all(&dir)?;
            let ext = json2docx::extract_template(&path_str(input))?;
            let template_json = json2docx::extract::template_json(&ext)?;
            std::fs::write(
                dir.join("template.json"),
                serde_json::to_string_pretty(&template_json)?,
            )?;
            let md = json2docx::describe_template(&ext, &input.display().to_string());
            std::fs::write(dir.join("TEMPLATE.md"), md)?;
            out.status(&format!(
                "已提取模板: {} ({} 个样式) → template.json + TEMPLATE.md",
                dir.display(),
                ext.styles.len()
            ));
            out.emit_result(&json!({
                "ok": true,
                "output": dir.display().to_string(),
                "name": tname,
                "styles": ext.styles.len(),
            }))?;
        }
    }
    Ok(office_core::EXIT_SUCCESS)
}

/// 只读加载 Document（.docx 临时解包或产物目录），供 dump 使用
fn load_document_readonly(input: &std::path::Path) -> Result<json2docx::model::Document> {
    let (root, _tmp) = product::resolve_input(input)?;
    product::load_document(&root)
}

/// 导出为 PDF/PNG 前确保拿到一个 .docx：
/// 输入是 .docx 直接用；是产物目录则 repack 到临时文件（返回守卫）。
fn ensure_docx(
    input: &std::path::Path,
    root: &std::path::Path,
) -> Result<(PathBuf, Option<product::FileGuard>)> {
    if input.is_file()
        && input
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("docx"))
            .unwrap_or(false)
    {
        return Ok((input.to_path_buf(), None));
    }
    let name = root
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "doc".to_string());
    let tmp = std::env::temp_dir().join(format!(
        "json2docx-export-{}-{name}.docx",
        std::process::id()
    ));
    json2docx::repack(&path_str(root), &path_str(&tmp))
        .map_err(|e| CliError::new(format!("repack 失败: {e}")))?;
    Ok((tmp.clone(), Some(product::FileGuard { path: tmp })))
}

pub fn schema_guide() -> &'static str {
    SCHEMA_GUIDE
}
const SCHEMA_GUIDE: &str = concat!(
    "JSON Schema 说明（json2docx）\n",
    "================================\n",
    "顶层 document.json：\n",
    "  meta     { title, author, subject, keywords[] }\n",
    "  page     { size: A4|Letter|A3|custom, orientation, margins{top,bottom,left,right,header,footer}(pt), columns }\n",
    "  theme    { major_font, minor_font, east_asia_font, heading_font, colors }\n",
    "  styles   { <styleId>: { font_size, bold, italic, color, align, line_spacing, first_line_indent, space_before, space_after, font_family } }\n",
    "  template academic-paper|report|letter|resume|memo\n",
    "  parts    [\"word/parts/partN.json\"]\n",
    "\n分片 part.json：{ section?: {page,header,footer_page_number}, blocks: Block[] }\n",
    "\nBlock 类型（type）：\n",
    "  heading      { level, text, numbering? }\n",
    "  paragraph    { text | runs[], style?, align?, first_line_indent?, indent?, hanging_indent?, line_spacing?, space_before?, space_after?, bold?, italic?, font_size?, color?, font_family? }\n",
    "  list         { ordered, items:[{blocks[], level?}] }\n",
    "  table        { header_row?, widths?[], rows:[{cells:[{blocks[],colspan?,fill?}]}], caption?, font_size? }\n",
    "  image        { src, width?, height?, align?, caption?, alt? }\n",
    "  formula      { latex, display?, number? }\n",
    "  code         { lang?, text }\n",
    "  quote        { text }\n",
    "  toc          { title, levels?[] }\n",
    "  bibliography { style, entries:[{kind?,authors[],title,container?,year?,publisher?,doi?,url?}] }\n",
    "  caption      { text, of? }\n",
    "  page_break   {}\n",
    "\nRun: { text, bold?, italic?, underline?, strike?, color?, highlight?, font_family?, font_size?, superscript?, subscript?, code?, hyperlink? }\n",
    "\n单位：长度/字号 pt；字号也可写 \"14pt\"；行距用倍数（1.5）；页面用 A4/Letter。\n",
    "命令：unpack / repack / view / edit / render / validate / dump / raw / raw-set / schema / templates / extract / merge / mcp / serve"
);
