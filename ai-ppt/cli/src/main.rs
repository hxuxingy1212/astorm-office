//! json2pptx 命令行工具
//!
//! 无状态设计：命令之间不共享任何隐藏状态。
//! - unpack / repack：输入输出由参数显式指定。
//! - view / edit / render / query / validate / dump / extract：输入可以是 PPTX
//!   文件（解包到一次性临时目录，命令结束后自动删除）或 unpack 产物目录。
//!
//! 输出契约与 astorm-office 另两个 CLI 一致（docs/cli-conventions.md）：
//! stdout 只输出数据，状态/进度走 stderr；`--json` 时写盘命令向 stdout 输出
//! 结果 JSON，stderr 的事件与错误也结构化为 JSON 行。
//!
//! 子命令：unpack / repack / view / edit / render / query / validate / dump /
//! extract / raw / raw-set / schema / templates

mod batch;
mod browser;
mod capabilities;
mod dump;
mod edit;
mod extract;
mod help;
mod mcp;
mod merge_cmd;
mod path;
mod product;
mod query;
mod render;
mod serve;
mod validate;
mod view;

use clap::{Parser, Subcommand};
use office_core::{
    path_str, same_file, with_stem_ext, CliError, Output, EXIT_FAILURE, EXIT_ISSUES,
};
use serde_json::json;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "json2pptx",
    version,
    about = "JSON 与 PPTX 双向转换工具",
    disable_help_subcommand = true
)]
struct Cli {
    /// 输出路径（unpack/repack/edit/render/extract/raw-set 共用）
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
    /// 旧版 PowerPoint（.ppt, MS-PPT）→ 新格式 .pptx（纯 Rust office_oxide）
    Convert {
        /// 输入的旧版文件（.ppt）
        input: PathBuf,
        /// 转换引擎：auto / libreoffice / rust
        #[arg(long, default_value = "auto")]
        engine: String,
    },
    /// 将 PPTX 解包为可编辑中间产物（presentation.json + ppt/slides/*.json + ppt/media/）
    Unpack {
        /// 输入的 PPTX 文件路径
        input: PathBuf,
        /// 输出单个自包含 JSON（分片内联、媒体转 data URI；预览/交付形态，编辑请用目录形态）
        #[arg(long)]
        inline: bool,
    },
    /// 从 unpack 产物目录重建 PPTX
    Repack {
        /// 输入的 unpack 产物目录
        input: PathBuf,
    },
    /// 第一层视图：text（文本+媒体）/ layout（组件布局）
    ///
    /// 输入是 PPTX 文件（一次性解包到临时目录）或 unpack 产物目录。
    View {
        /// 输入的 PPTX 文件或 unpack 产物目录
        input: PathBuf,
        /// 元素路径（如 /slide[1]）
        path: String,
        /// 视图模式
        mode: ViewMode,
    },
    /// 第二层精准修改：get / set / add / remove
    ///
    /// 输入是产物目录时原地修改；输入是 PPTX 时，写操作（set/add/remove）
    /// 必须配合 -o 输出新 PPTX（一次性修改，不回写原文件）。
    Edit {
        /// 输入的 PPTX 文件或 unpack 产物目录
        input: PathBuf,
        /// 元素路径（如 /slide[1]/text[2]）
        path: String,
        #[command(subcommand)]
        action: EditAction,
    },
    /// 将指定幻灯片渲染为 PNG 图片
    ///
    /// 输入是 PPTX 文件（一次性解包到临时目录）或 unpack 产物目录。
    Render {
        /// 输入的 PPTX 文件或 unpack 产物目录
        input: PathBuf,
        /// 幻灯片路径（如 /slide[1]）
        path: String,
        /// 渲染引擎：browser（复用 web 前端，默认）/ native（Rust resvg）
        #[arg(long, value_enum, default_value_t = RenderEngine::Browser)]
        engine: RenderEngine,
        /// 渲染缩放（browser 引擎 = 图像像素倍率，1 = 96dpi）
        #[arg(long, default_value_t = 1.0)]
        scale: f64,
    },
    /// 按选择器查询元素（类 CSS 选择器）
    ///
    /// 输入是 PPTX 文件或 unpack 产物目录。
    Query {
        /// 输入的 PPTX 文件或 unpack 产物目录
        input: PathBuf,
        /// 选择器，如 `slide[1] shape[name=Title]` / `text:contains("Hello")`
        selector: String,
    },
    /// 校验演示文稿结构（越界/空文本/缺失图片/表格列数等），问题非空时退出码为 3
    Validate {
        /// 输入的 PPTX 文件或 unpack 产物目录
        input: PathBuf,
    },
    /// 导出可回放的编辑指令 JSON（dump）
    Dump {
        /// 输入的 PPTX 文件或 unpack 产物目录
        input: PathBuf,
    },
    /// 从存量 PPTX 提取主题风格模板（template.json + TEMPLATE.md）
    Extract {
        /// 输入的 PPTX 文件或 unpack 产物目录
        input: PathBuf,
        /// 模板名（默认取文件名）
        #[arg(long)]
        name: Option<String>,
    },
    /// 原始 OOXML：读取包内部件（字节原样输出，字面路径）
    Raw { input: PathBuf, part: String },
    /// 原始 OOXML：整部件替换（-o 必填，不覆盖原文件；`--file -` 读 stdin）
    RawSet {
        input: PathBuf,
        part: String,
        /// 新内容文件（- 表示 stdin）
        #[arg(long)]
        file: String,
    },
    /// 常驻编辑服务：stdin 逐行 JSON 操作（serve <产物目录>）
    Serve {
        /// unpack 产物目录
        input: PathBuf,
    },
    /// MCP 服务（stdio + JSON-RPC 2.0）
    Mcp,
    /// 用 JSON 数据替换 {{key}} 占位符（产物目录原地填充）
    Merge {
        /// unpack 产物目录
        input: PathBuf,
        /// 数据 JSON（对象字符串）
        #[arg(long)]
        data: String,
    },
    /// 回放 dump 风格编辑指令（默认遇错即停；--force 跳过继续）
    Batch {
        /// 输入的产物目录（.pptx 需先 unpack）
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
        /// 元素名（slide/text/shape/line/image/table/chart/group/line_chart/...）
        topic: Option<String>,
    },
    /// 查看 JSON Schema 文档
    Schema,
    /// 列出所有可用模板
    Templates,
}

/// 渲染引擎
#[derive(clap::ValueEnum, Clone, Copy, PartialEq)]
enum RenderEngine {
    /// 无头浏览器截图（复用 web 前端渲染，需 Chrome/Edge）
    Browser,
    /// Rust resvg 矢量近似（不依赖浏览器）
    Native,
}

/// view 视图模式
#[derive(clap::ValueEnum, Clone)]
enum ViewMode {
    /// 文本 + 媒体信息（去样式/图形，媒体为绝对路径）
    Text,
    /// 组件布局（类型/名称/文字/位置，树形）
    Layout,
}

/// edit 操作
#[derive(Subcommand)]
enum EditAction {
    /// 读取节点状态（JSON）
    Get,
    /// 修改属性（--prop k=v，可重复）
    Set {
        /// 属性列表 k=v
        #[arg(long)]
        prop: Vec<String>,
    },
    /// 在路径下新增元素
    Add {
        /// 元素类型（text/shape/line/image/table/group/...）
        #[arg(long)]
        r#type: String,
        /// 属性列表 k=v
        #[arg(long)]
        prop: Vec<String>,
    },
    /// 删除路径指向的元素
    Remove,
}

fn main() {
    let cli = Cli::parse();
    let out = Output::new(cli.json, cli.quiet, cli.verbose);
    let code = match run(&cli, &out) {
        Ok(code) => code,
        Err(e) => {
            out.error_full(err_code(&e), &e.message, None, e.suggestion.as_deref());
            EXIT_FAILURE
        }
    };
    std::process::exit(code);
}

/// CliError.code 是 String；错误码输出需要 &str，借用即可
fn err_code(e: &CliError) -> &str {
    &e.code
}

fn run(cli: &Cli, out: &Output) -> Result<i32, CliError> {
    let output = &cli.output;
    match &cli.command {
        Command::Convert { input, engine } => {
            if !json2pptx::is_cfb(input) {
                return Err(CliError::new("convert 需要 OLE2/CFB 旧版文件（.ppt）"));
            }
            let out_pptx = output
                .clone()
                .unwrap_or_else(|| with_stem_ext(input, "pptx"));
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
                office_core::soffice::convert(input, "pptx", &out_pptx)?;
                // WMF/EMF 浏览器无法显示：光栅化为 PNG 并改写引用（个别失败保留原样）
                let n = office_core::soffice::rasterize_metafiles(&out_pptx).unwrap_or(0);
                out.status(&format!(
                    "已转换: {} → {}（引擎: libreoffice，光栅化元文件 {n} 张）",
                    input.display(),
                    out_pptx.display()
                ));
            } else {
                json2pptx::ppt_to_pptx(input, &out_pptx)?;
                out.status(&format!(
                    "已转换: {} → {}（引擎: rust）",
                    input.display(),
                    out_pptx.display()
                ));
            }
            out.emit_result(&json!({
                "ok": true,
                "input": input.display().to_string(),
                "output": out_pptx.display().to_string(),
                "engine": if use_lo { "libreoffice" } else { "rust" },
            }))?;
        }
        Command::Unpack { input, inline } => {
            if *inline {
                let out_file = output
                    .clone()
                    .unwrap_or_else(|| input.with_extension("inline.json"));
                let r = office_core::inline::unpack_inline(input, &out_file, |i, o| {
                    json2pptx::unpack(&path_str(i), &path_str(o))
                        .map(|_| ())
                        .map_err(CliError::from)
                })?;
                out.status(&format!(
                    "已内联解包: {} ({} 张幻灯片)",
                    r.output.display(),
                    r.shards
                ));
                out.emit_result(&json!({
                    "ok": true,
                    "output": r.output.display().to_string(),
                    "slides": r.shards,
                    "inline": true
                }))?;
            } else {
                let out_dir = output.clone().unwrap_or_else(|| input.with_extension(""));
                let r = json2pptx::unpack(&path_str(input), &path_str(&out_dir))?;
                out.status(&format!("已解包: {} ({} 张幻灯片)", r.path, r.slides));
                out.emit_result(&json!({ "ok": true, "output": r.path, "slides": r.slides }))?;
            }
        }
        Command::Repack { input } => {
            let out_pptx = output
                .clone()
                .unwrap_or_else(|| with_stem_ext(input, "pptx"));
            let r = json2pptx::repack(&path_str(input), &path_str(&out_pptx))?;
            out.status(&format!("已生成: {} ({} 张幻灯片)", r.path, r.slides));
            out.emit_result(&json!({ "ok": true, "output": r.path, "slides": r.slides }))?;
        }
        Command::View { input, path, mode } => {
            let (root, _tmp) = resolve_input(input)?;
            let slides = product::load_slides(&root)?;
            let ep = path::parse(path)?;
            let resolved = path::resolve(&slides, &ep)?;
            let slide = &slides[resolved.slide_idx];
            let value = match mode {
                ViewMode::Text => view::view_text(slide, resolved.slide_idx + 1, &root),
                ViewMode::Layout => view::view_layout(slide, resolved.slide_idx + 1),
            };
            out.emit_json(&value)?;
        }
        Command::Edit {
            input,
            path,
            action,
        } => {
            let (root, tmp) = resolve_input(input)?;
            let is_write = matches!(
                action,
                EditAction::Set { .. } | EditAction::Add { .. } | EditAction::Remove
            );
            // 输入是 .pptx 时产物为一次性临时目录：写操作必须给出 -o 输出新文件
            if is_write && tmp.is_some() && output.is_none() {
                return Err(CliError::new(
                    "修改 .pptx 需要 -o <输出.pptx>（无状态模式下不原地改写输入文件）；\
                     或先 `unpack` 到产物目录，再对产物目录执行 edit 原地修改",
                ));
            }
            let mut slides = product::load_slides(&root)?;
            let ep = path::parse(path)?;
            let resolved = path::resolve(&slides, &ep)?;
            let value = match action {
                EditAction::Get => edit::get(&slides, &resolved)?,
                EditAction::Set { prop } => {
                    let props = edit::parse_props(prop)?;
                    let r = edit::set(&mut slides, &resolved, &props)?;
                    edit::save(&root, &slides, &resolved)?;
                    r
                }
                EditAction::Add { r#type, prop } => {
                    let props = edit::parse_props(prop)?;
                    let r = edit::add(&mut slides, &resolved, r#type, &props)?;
                    edit::save(&root, &slides, &resolved)?;
                    r
                }
                EditAction::Remove => {
                    let r = edit::remove(&mut slides, &resolved)?;
                    edit::save(&root, &slides, &resolved)?;
                    r
                }
            };
            // 显式 -o：产物修改后重建 PPTX（输入为 .pptx 时写操作必填）
            if let Some(out_pptx) = output {
                json2pptx::repack(&path_str(&root), &path_str(out_pptx))?;
                out.status(&format!("已生成: {}", out_pptx.display()));
            }
            out.emit_json(&value)?;
        }
        Command::Render {
            input,
            path,
            engine,
            scale,
        } => {
            let (root, _tmp) = resolve_input(input)?;
            let slides = product::load_slides(&root)?;
            let (width, height) = product::presentation_size(&root)?;
            let ep = path::parse(path)?;
            let resolved = path::resolve(&slides, &ep)?;
            let slide = &slides[resolved.slide_idx];

            let png_path = output.clone().unwrap_or_else(|| {
                let stem = input
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "slide".to_string());
                PathBuf::from(format!("{stem}_slide{}.png", resolved.slide_idx + 1))
            });

            let engine_name = match engine {
                RenderEngine::Browser => "browser",
                RenderEngine::Native => "native",
            };
            match engine {
                RenderEngine::Browser => {
                    browser::render_slide_browser(
                        &root,
                        resolved.slide_idx + 1,
                        width,
                        height,
                        *scale,
                        &png_path,
                    )?;
                }
                RenderEngine::Native => {
                    let root2 = root.clone();
                    let resolve: render::MediaResolver =
                        std::sync::Arc::new(move |src| product::media_abs_path(&root2, src));
                    render::render_slide_png(slide, width, height, *scale, &resolve, &png_path)?;
                }
            }
            out.status(&format!(
                "已渲染: {} ({})",
                png_path.display(),
                resolved.slide_idx + 1
            ));
            out.emit_result(&json!({
                "ok": true,
                "output": png_path.display().to_string(),
                "slide": resolved.slide_idx + 1,
                "engine": engine_name,
            }))?;
        }
        Command::Query { input, selector } => {
            let (root, _tmp) = resolve_input(input)?;
            let slides = product::load_slides(&root)?;
            let sel = query::parse_selector(selector)?;
            let results = query::run(&slides, &sel);
            out.emit_json(&json!({ "matches": results, "count": results.len() }))?;
        }
        Command::Validate { input } => {
            let (root, _tmp) = resolve_input(input)?;
            let slides = product::load_slides(&root)?;
            let (width, height) = product::presentation_size(&root)?;
            let issues = validate::run(&slides, width, height);
            let n = issues.len();
            let errors = issues
                .iter()
                .filter(|i| i.get("severity").and_then(|s| s.as_str()) == Some("error"))
                .count();
            out.status(&format!(
                "校验完成: {} 个问题（{} error / {} warning·info）",
                if n == 0 {
                    "无".to_string()
                } else {
                    n.to_string()
                },
                errors,
                n - errors
            ));
            out.emit_json(&json!({ "issues": issues, "count": n, "errors": errors }))?;
            if errors > 0 {
                return Ok(EXIT_ISSUES);
            }
        }
        Command::Dump { input } => {
            let (root, _tmp) = resolve_input(input)?;
            let slides = product::load_slides(&root)?;
            let (width, height) = product::presentation_size(&root)?;
            out.emit_json(&dump::run(&slides, width, height))?;
        }
        Command::Extract { input, name } => {
            let dir = output
                .clone()
                .unwrap_or_else(|| default_template_dir(input));
            let pres = load_presentation(input)?;
            let stem = input
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "template".to_string());
            let tname = name.clone().unwrap_or(stem);
            let (dir, colors) =
                extract::write_template_dir(&pres, &dir, &tname, &input.display().to_string())?;
            out.status(&format!(
                "已提取模板: {} ({} 个主题色)",
                dir.display(),
                colors
            ));
            out.emit_result(&json!({
                "ok": true,
                "output": dir.display().to_string(),
                "name": tname,
                "colors": colors,
            }))?;
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
            let out_pptx = output.as_ref().ok_or_else(|| {
                CliError::new("raw-set 写 PPTX 必须指定 -o <输出.pptx>（不覆盖原文件）")
            })?;
            if same_file(input, out_pptx) {
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
            office_core::opc::write_part(input, out_pptx, part, &content)?;
            out.status(&format!("已替换部件: {} → {}", part, out_pptx.display()));
            out.emit_result(
                &json!({ "ok": true, "replaced": part, "output": out_pptx.display().to_string() }),
            )?;
        }
        Command::Serve { input } => {
            serve::run(input)?;
        }
        Command::Mcp => {
            mcp::run()?;
        }
        Command::Merge { input, data } => {
            if !product::is_product_dir(input) {
                return Err(
                    CliError::new("merge 写 .pptx 必须指定产物目录（先 unpack）")
                        .suggest("先 json2pptx unpack <文件.pptx> -o <目录>，再对目录 merge"),
                );
            }
            let jd: serde_json::Value = serde_json::from_str(data).map_err(|e| {
                CliError::new(format!("数据 JSON 解析失败: {e}"))
                    .suggest(r#"data 应为 JSON 对象字符串，如 '{"name":"张三"}'"#)
            })?;
            let n = merge_cmd::run(input, &jd)?;
            out.status(&format!("已填充: {}（替换 {n} 处）", input.display()));
            out.emit_result(&json!({
                "ok": true, "input": input.display().to_string(), "replacements": n
            }))?;
        }
        Command::Batch {
            input,
            input_file,
            commands,
            force,
        } => {
            if !product::is_product_dir(input) {
                return Err(CliError::new("batch 需要产物目录（.pptx 请先 unpack）")
                    .suggest("先 json2pptx unpack <文件.pptx> -o <目录>"));
            }
            let raw = read_batch_commands(input_file.as_deref(), commands.as_deref())?;
            let v: serde_json::Value = serde_json::from_str(&raw).map_err(|e| {
                CliError::new(format!("指令 JSON 解析失败: {e}"))
                    .suggest(r#"指令格式: {"commands":[{"op":"set","path":"/slide[1]/text[1]","props":{"text":"新文本"}}]}"#)
            })?;
            let steps = office_core::batch::parse_steps(&v).map_err(CliError::new)?;
            let mut slides = product::load_slides(input)?;
            let report = batch::run_steps(&mut slides, &steps, !force);
            let failed = report["failed"].as_u64().unwrap_or(0);
            let applied = report["applied"].as_u64().unwrap_or(0);
            if failed == 0 && applied > 0 {
                product::save_slides(input, &slides)?;
            }
            out.emit_json(&report)?;
            if failed > 0 {
                return Ok(EXIT_FAILURE);
            }
        }
        Command::Help { topic } => {
            help::run(topic.as_deref(), out)?;
        }
        Command::Schema => {
            if out.json_mode() {
                out.emit_json(&schemars::schema_for!(json2pptx::model::Presentation))?;
            } else {
                println!("分层能力帮助（元素属性面）用 `json2pptx help [元素]`。");
                println!("完整 JSON Schema 见 skill/references/schema.json（由 `schema --json` 生成，勿手改）。");
                println!("JSON 字段规范详见 docs/JSON格式规范.md。");
            }
        }
        Command::Templates => {
            let presets: Vec<_> = json2pptx::model::TemplatePreset::all()
                .iter()
                .map(|t| json!({ "name": t.name, "description": t.desc, "display": t.display }))
                .collect();
            if out.json_mode() {
                out.emit_json(&json!({ "templates": presets }))?;
            } else {
                println!("可用模板:");
                for t in &presets {
                    println!(
                        "  {:<12} {} ({})",
                        t["name"].as_str().unwrap_or(""),
                        t["description"].as_str().unwrap_or(""),
                        t["display"].as_str().unwrap_or("")
                    );
                }
            }
        }
    }
    Ok(office_core::EXIT_SUCCESS)
}

/// 以 Presentation 模型加载输入（.pptx 或产物目录）
fn load_presentation(input: &Path) -> Result<json2pptx::model::Presentation, CliError> {
    if product::is_product_dir(input) {
        let root = input;
        let slides = product::load_slides(root)?;
        let (width, height) = product::presentation_size(root)?;
        // 产物目录形态：从 presentation.json 直接读
        let pres_path = root.join("presentation.json");
        let text = std::fs::read_to_string(&pres_path)?;
        let mut pres: json2pptx::model::Presentation = serde_json::from_str(&text)?;
        pres.width = width;
        pres.height = height;
        pres.slides = slides;
        Ok(pres)
    } else {
        Ok(json2pptx::parse(&path_str(input))?)
    }
}

/// 将输入解析为产物根目录
///
/// - 输入是 unpack 产物目录（含 presentation.json）→ 直接使用，不清理；
/// - 否则视为 .pptx → 解包到一次性临时目录，命令结束（guard drop）后自动删除。
fn resolve_input(input: &Path) -> Result<(PathBuf, Option<TempGuard>), CliError> {
    if product::is_product_dir(input) {
        return Ok((input.to_path_buf(), None));
    }
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "deck".to_string());
    let tmp = std::env::temp_dir().join(format!("json2pptx-{}-{stem}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    json2pptx::unpack(&path_str(input), &path_str(&tmp))
        .map_err(|e| CliError::new(format!("解包 {} 失败: {e}", input.display())))?;
    Ok((tmp.clone(), Some(TempGuard { path: tmp })))
}

/// 一次性临时产物目录守卫：Drop 时删除
struct TempGuard {
    path: PathBuf,
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn default_template_dir(input: &Path) -> PathBuf {
    let stem = input.file_stem().unwrap_or_default();
    input.with_file_name(format!("{}_template", stem.to_string_lossy()))
}

/// 读取 batch 指令：--input-file > --commands > stdin
fn read_batch_commands(
    input_file: Option<&str>,
    commands: Option<&str>,
) -> Result<String, CliError> {
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
