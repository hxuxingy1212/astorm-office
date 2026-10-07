//! json2xlsx — JSON ⇄ XLSX command line tool.
//!
//! 无状态设计。输出契约与 astorm-office 另两个 CLI 一致（docs/cli-conventions.md）：
//! stdout 只输出数据，状态/进度走 stderr；`--json` 时写盘命令向 stdout 输出结果
//! JSON，stderr 的事件与错误也结构化为 JSON 行。错误带 `suggestion`（自愈建议）。

mod batch;
mod capabilities;
mod dump;
mod help;
mod import;
mod mcp;
mod ops;
mod path;
mod query;
mod render;
mod serve;
mod validate;

use clap::{Parser, Subcommand};
use json2xlsx::Workbook;
use office_core::{
    same_file, stem_dir, with_stem_ext, CliError, Output, EXIT_FAILURE, EXIT_ISSUES,
};
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "json2xlsx",
    version,
    about = "JSON <-> XLSX 双向转换工具",
    disable_help_subcommand = true
)]
struct Cli {
    /// 输出路径（unpack/repack/edit/render/extract/raw-set/import/merge/batch 共用）
    #[arg(short, long, global = true)]
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
    /// xlsx → 产物目录（workbook.json + xl/worksheets/sheetN.json）
    Unpack { input: PathBuf },
    /// 旧版 Excel（.xls, BIFF8）→ 新格式 xlsx（auto 检测到 LibreOffice 用之，保真最高；rust 为纯 Rust calamine，丢样式）
    Convert {
        /// 输入的旧版文件（.xls）
        input: PathBuf,
        /// 转换引擎：auto / libreoffice / rust
        #[arg(long, default_value = "auto")]
        engine: String,
    },
    /// 产物目录 → xlsx
    Repack { input: PathBuf },
    /// 只读查看：text（值/公式）/ layout（结构树）；旧值 values/structure 兼容
    View {
        input: PathBuf,
        #[arg(default_value = "/sheet[1]")]
        path: String,
        #[arg(default_value = "text")]
        mode: String,
    },
    /// 精准修改：get / set / add / remove
    Edit {
        input: PathBuf,
        path: String,
        #[command(subcommand)]
        action: EditAction,
    },
    /// 渲染预览：HTML（缺省）或 PNG（无头 Chrome）
    Render {
        input: PathBuf,
        /// 输出格式（缺省按 -o 扩展名推断，其次 html）
        #[arg(long, value_enum, default_value_t = RenderFormat::Auto)]
        format: RenderFormat,
    },
    /// 导入 CSV/TSV 为 xlsx（-o 指定输出）
    Import {
        input: PathBuf,
        /// 分隔符（缺省自动推断 ，或 \t / ;）
        #[arg(long)]
        delim: Option<String>,
        /// 首行为表头
        #[arg(long)]
        header: bool,
        /// 工作表名
        #[arg(long)]
        sheet: Option<String>,
    },
    /// 模板数据填充：把 {{key}} 替换为 JSON 数据
    Merge {
        input: PathBuf,
        /// 数据 JSON（对象字符串）
        #[arg(short = 'd', long)]
        data: String,
    },
    /// 回放 dump 指令（默认遇错停止；--force 跳过错误继续）
    Batch {
        input: PathBuf,
        /// 指令 JSON 文件（与 --commands 二选一；都缺省时读 stdin）
        #[arg(long)]
        input_file: Option<PathBuf>,
        /// 内联指令 JSON
        #[arg(long)]
        commands: Option<String>,
        /// 跳过错误继续执行
        #[arg(long)]
        force: bool,
    },
    /// 原始 OOXML：读取部件（字节原样输出）
    Raw { input: PathBuf, part: String },
    /// 原始 OOXML：整部件替换（-o 必填，不覆盖原文件）
    RawSet {
        input: PathBuf,
        part: String,
        /// 新内容文件（- 表示 stdin）
        #[arg(long)]
        file: String,
    },
    /// 从存量 Excel 提取样式模板（等价于旧的 `template extract`）
    Extract {
        input: PathBuf,
        /// 模板名（默认取文件名）
        #[arg(long)]
        name: Option<String>,
    },
    /// （兼容旧写法）template extract
    Template {
        #[command(subcommand)]
        action: TemplateAction,
    },
    /// 结构校验：问题非空时退出码为 3
    Validate { input: PathBuf },
    /// 序列化为可回放的编辑指令（batch JSON）
    Dump { input: PathBuf },
    /// 元素查询：sheets / sheet[N] / sheet[name=X] / sheet:empty / cell:contains("文本")
    Query { input: PathBuf, selector: String },
    /// 分层能力帮助：help [元素] [--json]
    Help {
        /// 元素名（cell/sheet/range/row/col；缺省为概览）
        topic: Option<String>,
    },
    /// 常驻编辑服务：加载一次，stdin 逐行 JSON 操作
    Serve { input: PathBuf },
    /// 启动 MCP 服务（stdio JSON-RPC）
    Mcp,
    /// 打印 JSON Schema 指引（--json 输出机器可读 schema）
    Schema,
    /// 列出内置模板
    Templates,
}

#[derive(Subcommand)]
enum TemplateAction {
    /// 提取样式模板 → template.json + TEMPLATE.md + skeleton/ 产物目录
    Extract {
        input: PathBuf,
        /// 模板名（默认取文件名）
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum EditAction {
    Get,
    Set {
        #[arg(long = "prop")]
        props: Vec<String>,
    },
    Add {
        #[arg(long = "type")]
        etype: String,
        #[arg(long = "prop")]
        props: Vec<String>,
    },
    Remove,
}

#[derive(clap::ValueEnum, Clone, PartialEq)]
enum RenderFormat {
    /// 按 -o 扩展名推断，其次 html
    Auto,
    Html,
    Png,
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

fn run(cli: &Cli, out: &Output) -> Result<i32, CliError> {
    let output = &cli.output;
    match &cli.command {
        Command::Unpack { input } => {
            let dir = output.clone().unwrap_or_else(|| stem_dir(input));
            let r = json2xlsx::unpack(input, &dir)?;
            out.status(&format!("已解包: {} ({} 个工作表)", r.dir, r.sheets));
            out.emit_result(&json!({ "ok": true, "output": r.dir, "sheets": r.sheets }))?;
        }
        Command::Repack { input } => {
            let xlsx = output
                .clone()
                .unwrap_or_else(|| with_stem_ext(input, "xlsx"));
            let r = json2xlsx::repack(input, &xlsx)?;
            out.status(&format!("已生成: {} ({} 个工作表)", r.path, r.sheets));
            out.emit_result(&json!({ "ok": true, "output": r.path, "sheets": r.sheets }))?;
        }
        Command::View { input, path, mode } => {
            let wb = load_input(input)?;
            let p = path::parse(path)?;
            let value = match mode.as_str() {
                "text" | "values" => ops::view_values(&wb, &p)?,
                "layout" | "structure" => ops::view_structure(&wb, &p)?,
                other => {
                    let mut e = CliError::new(format!("未知 view 模式: {other}"));
                    e.suggestion = Some(office_core::suggest::did_you_mean(
                        other,
                        &capabilities::VIEW_MODES
                            .iter()
                            .map(|(m, _)| *m)
                            .collect::<Vec<_>>(),
                        4,
                    ));
                    return Err(e);
                }
            };
            out.emit_json(&value)?;
        }
        Command::Edit {
            input,
            path,
            action,
        } => {
            let p = path::parse(path)?;
            let is_dir = json2xlsx::is_product_dir(input);
            match action {
                EditAction::Get => {
                    let wb = load_input(input)?;
                    out.emit_json(&ops::get(&wb, &p)?)?;
                }
                EditAction::Set { props } => {
                    let props = ops::parse_props(props);
                    apply_edit(out, output.as_ref(), input, is_dir, |wb| {
                        ops::set(wb, &p, &props)
                    })?;
                }
                EditAction::Add { etype, props } => {
                    let props = ops::parse_props(props);
                    apply_edit(out, output.as_ref(), input, is_dir, |wb| {
                        ops::add(wb, &p, etype, &props)
                    })?;
                }
                EditAction::Remove => {
                    apply_edit(out, output.as_ref(), input, is_dir, |wb| {
                        ops::remove(wb, &p)
                    })?;
                }
            }
        }
        Command::Render { input, format } => {
            let ext = output
                .as_ref()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let fmt = match format {
                RenderFormat::Auto => {
                    if ext == "png" {
                        RenderFormat::Png
                    } else {
                        RenderFormat::Html
                    }
                }
                other => other.clone(),
            };
            let (root, _guard) = resolve_render_root(input)?;
            let wb = json2xlsx::load_product(&root)?;
            let (html, report) = render::workbook_html(&wb, Some(&root));
            if report.clipped_total() > 0 {
                let detail = report
                    .clipped
                    .iter()
                    .map(|(name, n)| format!("{name}:{n} 列"))
                    .collect::<Vec<_>>()
                    .join("、");
                out.status(&format!(
                    "提示: 已按一页宽度截断（{detail}）；完整内容见 xlsx 或打印设置调整纸张/缩放"
                ));
            }
            match fmt {
                RenderFormat::Png => {
                    let png = output
                        .clone()
                        .unwrap_or_else(|| with_stem_ext(input, "png"));
                    let html_path = png.with_extension("html");
                    let page_px = render::page_width_for(&wb);
                    std::fs::write(&html_path, html)?;
                    office_core::chrome::html_to_png(&html_path, &png, page_px).map_err(|e| {
                        CliError::new(e).suggest("可改用 `--format html` 输出 HTML 预览")
                    })?;
                    out.status(&format!("已渲染: {}", png.display()));
                    out.emit_result(
                        &json!({ "ok": true, "output": png.display().to_string(), "format": "png" }),
                    )?;
                }
                _ => {
                    let html_path = output
                        .clone()
                        .unwrap_or_else(|| with_stem_ext(input, "html"));
                    std::fs::write(&html_path, html)?;
                    out.status(&format!("已生成预览: {}", html_path.display()));
                    out.emit_result(&json!({
                        "ok": true, "output": html_path.display().to_string(), "format": "html"
                    }))?;
                }
            }
        }
        Command::Import {
            input,
            delim,
            header,
            sheet,
        } => {
            let xlsx = output
                .as_ref()
                .ok_or_else(|| CliError::new("import 需要 -o 指定输出 xlsx 路径"))?;
            let wb = import::csv_to_workbook(input, delim.as_deref(), *header, sheet.clone())?;
            json2xlsx::generate(&wb, xlsx)?;
            let n = wb.sheets.len();
            out.status(&format!("已导入: {} ({} 个工作表)", xlsx.display(), n));
            out.emit_result(
                &json!({ "ok": true, "output": xlsx.display().to_string(), "sheets": n }),
            )?;
        }
        Command::Merge { input, data } => {
            let is_dir = json2xlsx::is_product_dir(input);
            let mut wb = load_input(input)?;
            let jd: serde_json::Value = serde_json::from_str(data).map_err(|e| {
                CliError::new(format!("数据 JSON 解析失败: {e}"))
                    .suggest("data 应为 JSON 对象字符串，如 '{\"name\":\"张三\"}'")
            })?;
            let n = json2xlsx::merge_workbook(&mut wb, &jd);
            let xlsx = match output {
                Some(o) => o.clone(),
                None if is_dir => {
                    // 产物目录：原地保存 + 默认重建同名 xlsx
                    json2xlsx::save_product(&wb, input)?;
                    with_stem_ext(input, "xlsx")
                }
                None => {
                    return Err(CliError::new("merge 写 .xlsx 必须指定 -o <输出.xlsx>")
                        .suggest("或先 unpack 到产物目录，对目录 merge"))
                }
            };
            json2xlsx::generate(&wb, &xlsx)?;
            out.status(&format!("已生成: {}（替换 {} 处）", xlsx.display(), n));
            out.emit_result(&json!({
                "ok": true, "output": xlsx.display().to_string(), "replacements": n
            }))?;
        }
        Command::Batch {
            input,
            input_file,
            commands,
            force,
        } => {
            let raw = read_batch_commands(input_file.as_deref(), commands.as_deref())?;
            let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
                CliError::new(format!("指令 JSON 解析失败: {e}"))
                    .suggest("指令格式: {\"commands\":[{\"op\":\"set\",\"path\":\"/sheet[1]/cell[A1]\",\"props\":{...}}]}")
            })?;
            let steps = office_core::batch::parse_steps(&v).map_err(CliError::new)?;
            let is_dir = json2xlsx::is_product_dir(input);
            let mut wb = load_input(input)?;
            let report = batch::run_steps(&mut wb, &steps, !force);
            let failed = report["failed"].as_u64().unwrap_or(0);
            if failed == 0 {
                match output {
                    Some(o) => {
                        json2xlsx::generate(&wb, o)?;
                    }
                    None if is_dir => {
                        json2xlsx::save_product(&wb, input)?;
                    }
                    None => {
                        return Err(CliError::new(
                            "batch 写 .xlsx 必须指定 -o <输出.xlsx>（无状态模式不原地改写）",
                        )
                        .suggest("或先 unpack 到产物目录，对目录 batch"))
                    }
                }
            }
            out.emit_json(&report)?;
            if failed > 0 {
                return Ok(EXIT_FAILURE);
            }
        }
        Command::Raw { input, part } => {
            let content = json2xlsx::raw::read_part(input, part)?
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
            let xlsx = output.as_ref().ok_or_else(|| {
                CliError::new("raw-set 写 xlsx 必须指定 -o <输出.xlsx>（不覆盖原文件）")
            })?;
            if same_file(input, xlsx) {
                return Err(CliError::new("-o 不能与输入文件相同：raw-set 不覆盖原文件"));
            }
            let content = if file == "-" {
                use std::io::Read;
                let mut buf = Vec::new();
                std::io::stdin().read_to_end(&mut buf)?;
                buf
            } else {
                std::fs::read(file)?
            };
            json2xlsx::raw::write_part(input, xlsx, part, &content)?;
            out.status(&format!("已替换部件: {} → {}", part, xlsx.display()));
            out.emit_result(&json!({
                "ok": true, "replaced": part, "output": xlsx.display().to_string()
            }))?;
        }
        Command::Extract { input, name } => {
            do_extract(out, output.as_ref(), input, name.as_deref())?;
        }
        Command::Template { action } => match action {
            TemplateAction::Extract { input, name } => {
                out.status("提示: `template extract` 已更名为顶层 `extract`，旧写法仍兼容。");
                do_extract(out, output.as_ref(), input, name.as_deref())?;
            }
        },
        Command::Validate { input } => {
            let wb = load_input(input)?;
            let issues = validate::run(&wb);
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
            let wb = load_input(input)?;
            out.emit_json(&dump::run(&wb))?;
        }
        Command::Query { input, selector } => {
            let wb = load_input(input)?;
            out.emit_json(&query::run(&wb, selector)?)?;
        }
        Command::Help { topic } => {
            help::run(topic.as_deref(), out)?;
        }
        Command::Serve { input } => {
            serve::run(input)?;
        }
        Command::Mcp => {
            mcp::run()?;
        }
        Command::Convert { input, engine } => {
            if !json2xlsx::legacy::is_cfb(input) {
                return Err(CliError::new("convert 需要 OLE2/CFB 旧版文件（.xls）"));
            }
            let xlsx = output
                .clone()
                .unwrap_or_else(|| with_stem_ext(input, "xlsx"));
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
            let sheets;
            if use_lo {
                office_core::soffice::convert(input, "xlsx", &xlsx)?;
                sheets = None;
                out.status(&format!(
                    "已转换: {} → {}（引擎: libreoffice）",
                    input.display(),
                    xlsx.display()
                ));
            } else {
                let wb = json2xlsx::xls_to_workbook(input)?;
                sheets = Some(wb.sheets.len());
                json2xlsx::generate(&wb, &xlsx)?;
                out.status(&format!(
                    "已转换: {} → {}（引擎: rust）",
                    input.display(),
                    xlsx.display()
                ));
            }
            out.emit_result(&json!({
                "ok": true,
                "input": input.display().to_string(),
                "output": xlsx.display().to_string(),
                "sheets": sheets,
                "engine": if use_lo { "libreoffice" } else { "rust" },
            }))?;
        }
        Command::Schema => {
            if out.json_mode() {
                out.emit_json(&schemars::schema_for!(json2xlsx::Workbook))?;
            } else {
                println!("完整 JSON Schema 见 skill/references/schema.json（由 `schema --json` 生成，勿手改）。");
                println!("工作簿 JSON 字段说明见 skill/SKILL.md 与 skill/references/cli.md。");
                println!("分层能力帮助（元素属性面）用 `json2xlsx help [元素]`。");
            }
        }
        Command::Templates => {
            let lib: serde_json::Value =
                serde_json::from_str(include_str!("../../skill/templates/library.json"))
                    .unwrap_or(serde_json::Value::Null);
            let templates = lib
                .get("templates")
                .and_then(|t| t.as_array())
                .cloned()
                .unwrap_or_default();
            if out.json_mode() {
                out.emit_json(&json!({ "templates": templates }))?;
            } else {
                println!(
                    "内置风格模板库共 {} 套（采样 {} 份真实 Excel 聚类），索引见 skill/templates/README.md：",
                    templates.len(),
                    lib.get("total_sampled").and_then(|v| v.as_u64()).unwrap_or(0)
                );
                for t in &templates {
                    let slug = t.get("slug").and_then(|v| v.as_str()).unwrap_or("?");
                    let cluster = t.get("cluster").and_then(|v| v.as_str()).unwrap_or("");
                    let samples = t.get("sample_count").and_then(|v| v.as_u64()).unwrap_or(0);
                    println!("  {slug:<22} {cluster:<10} 样本 {samples}");
                }
                println!("每套包含 template.json + TEMPLATE.md + skeleton/；用 `extract` 可从存量 xlsx 提取新模板。");
            }
        }
    }
    Ok(office_core::EXIT_SUCCESS)
}

/// 读取 batch 指令来源：--input-file > --commands > stdin
fn read_batch_commands(file: Option<&Path>, inline: Option<&str>) -> Result<String, CliError> {
    if let Some(f) = file {
        return std::fs::read_to_string(f)
            .map_err(|e| CliError::with_code("io", format!("读取指令文件失败: {e}")));
    }
    if let Some(c) = inline {
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

fn do_extract(
    out: &Output,
    output: Option<&PathBuf>,
    input: &Path,
    name: Option<&str>,
) -> Result<(), CliError> {
    let dir = output
        .cloned()
        .unwrap_or_else(|| default_template_dir(input));
    let wb = load_input(input)?;
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "template".into());
    let tname = name.map(|s| s.to_string()).unwrap_or(stem);
    let t = json2xlsx::template::write_template_dir(
        &wb,
        &dir,
        &tname,
        Some(&input.display().to_string()),
    )?;
    out.status(&format!("已提取模板: {}", dir.display()));
    out.status(&format!(
        "  template.json / TEMPLATE.md / skeleton/  (配色 {} 项, 字体 {} 项, 数字格式 {} 项)",
        t.palette.len(),
        t.fonts.len(),
        t.number_formats.len()
    ));
    out.emit_result(&json!({
        "ok": true,
        "output": dir.display().to_string(),
        "name": tname,
        "palette": t.palette.len(),
        "fonts": t.fonts.len(),
        "number_formats": t.number_formats.len(),
    }))?;
    Ok(())
}

fn apply_edit<F>(
    out: &Output,
    output: Option<&PathBuf>,
    input: &Path,
    is_dir: bool,
    f: F,
) -> Result<(), CliError>
where
    F: FnOnce(&mut Workbook) -> Result<serde_json::Value, CliError>,
{
    if is_dir {
        let mut wb = json2xlsx::load_product(input)?;
        let res = f(&mut wb)?;
        json2xlsx::save_product(&wb, input)?;
        out.emit_json(&res)?;
    } else {
        let xlsx = output
            .cloned()
            .ok_or_else(|| CliError::new("对 .xlsx 写入必须指定 -o <输出.xlsx>"))?;
        let mut wb = json2xlsx::parse(input)?;
        let res = f(&mut wb)?;
        json2xlsx::generate(&wb, &xlsx)?;
        out.emit_json(&res)?;
        out.status(&format!("已生成: {}", xlsx.display()));
    }
    Ok(())
}

/// 渲染输入解析：产物目录直接用；.xlsx 解包到一次性临时目录，
/// 让 HTML/PNG 能引用到真实媒体文件（guard 需存活到渲染结束）。
pub(crate) fn resolve_render_root(
    input: &Path,
) -> Result<(PathBuf, Option<office_core::TempGuard>), CliError> {
    if json2xlsx::is_product_dir(input) {
        return Ok((input.to_path_buf(), None));
    }
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "book".to_string());
    let tmp = std::env::temp_dir().join(format!("json2xlsx-render-{}-{stem}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    json2xlsx::unpack(input, &tmp)?;
    Ok((tmp.clone(), Some(office_core::TempGuard { path: tmp })))
}

fn load_input(input: &Path) -> Result<Workbook, CliError> {
    if json2xlsx::is_product_dir(input) {
        Ok(json2xlsx::load_product(input)?)
    } else {
        Ok(json2xlsx::parse(input)?)
    }
}

fn default_template_dir(input: &Path) -> PathBuf {
    let stem = input.file_stem().unwrap_or_default();
    input.with_file_name(format!("{}_template", stem.to_string_lossy()))
}
