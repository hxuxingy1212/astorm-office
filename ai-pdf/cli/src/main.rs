//! json2pdf 命令行工具 — PDF 与 JSON 产物目录双向转换 + PDF 工具集。
//!
//! 输出契约与 astorm-office 另三个 CLI 一致（docs/cli-conventions.md）：
//! stdout 只输出数据（JSON 结果 / 人读指引文本），状态/进度走 stderr；
//! `--json` 时写盘命令向 stdout 输出结果 JSON，stderr 的事件与错误也结构化为 JSON 行。
//! 退出码：0 成功 / 1 运行时错误 / 2 用法错误 / 3 命令成功但产物存在校验问题。
//!
//! 子命令采用分组两层结构（`json2pdf extract text`），
//! 同时兼容原 pdf.py 的点分写法（`json2pdf extract.text`）。

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
mod validate;
mod view;

use clap::{Parser, Subcommand};
use json2pdf::{
    checks, convert, design, forms, images, meta, page_ops, palette, qa, repack, sanitize, text,
    unpack, util,
};
use office_core::{CliError, Output, EXIT_FAILURE, EXIT_ISSUES, EXIT_SUCCESS};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "json2pdf",
    version,
    about = "PDF 工具集：产物目录双向解析、PDF ↔ Office（docx/pptx/xlsx）互转、内容提取（版式重建/markdown/OCR）与质检",
    disable_help_subcommand = true
)]
struct Cli {
    /// 输出路径（unpack/repack/pages/meta set/form fill/convert 等写盘命令共用）
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
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 依赖探测
    #[command(subcommand)]
    Env(EnvCmd),
    /// 内容提取
    #[command(subcommand)]
    Extract(ExtractCmd),
    /// 页操作
    #[command(subcommand)]
    Pages(PagesCmd),
    /// 元数据
    #[command(subcommand)]
    Meta(MetaCmd),
    /// AcroForm 表单
    #[command(subcommand)]
    Form(FormCmd),
    /// 格式转换（Office→PDF 引擎链 / PDF→Office 产物映射 / HTML / LaTeX）
    #[command(subcommand)]
    Convert(ConvertCmd),
    /// 调色板生成
    #[command(subcommand)]
    Palette(PaletteCmd),
    /// 设计引擎：SVG 背景 / 布局 / 关键词推导 / 审计
    #[command(subcommand)]
    Design(DesignCmd),
    /// 生成脚本清洗（HTML 实体 / \uXXXX / 上下标 / 符号回退，原地写回）
    #[command(subcommand)]
    Code(CodeCmd),
    /// 文本内容清洗（默认干跑报告，--apply 写回）
    #[command(subcommand)]
    Content(ContentCmd),
    /// 质检（发现问题退出码 3）
    #[command(subcommand)]
    Check(CheckCmd),
    /// 成品 PDF 体检（发现问题退出码 3）
    Qa {
        /// PDF 文件（可多个）
        files: Vec<PathBuf>,
        /// 边距对称检查跳过第 1 页（封面）
        #[arg(long)]
        skip_cover: bool,
    },
    /// 正向解析：PDF → 产物目录（document.json + pages/*.json + media/ + fonts/）
    Unpack {
        /// 输入 PDF
        pdf: PathBuf,
        /// 输出单个自包含 JSON（分片内联、媒体转 data URI；预览/交付形态，编辑请用目录形态）
        #[arg(long)]
        inline: bool,
    },
    /// 逆向生成：产物目录 → PDF（可从零手写 JSON 构建；警告为回退提示，不影响退出码）
    Repack {
        /// 产物目录
        dir: PathBuf,
    },
    /// 第一层视图：text（文本+媒体）/ layout（元素布局树）
    ///
    /// 输入是 PDF 文件（一次性解包到临时目录）或 unpack 产物目录。
    View {
        /// 输入的 PDF 文件或 unpack 产物目录
        input: PathBuf,
        /// 页面路径（如 /page[1]）
        page_path: String,
        /// 视图模式
        mode: ViewMode,
    },
    /// 第二层精准修改：get / set / add / remove
    ///
    /// 输入是产物目录时原地修改分片 JSON；输入是 PDF 时，写操作（set/add/remove）
    /// 必须配合 -o 输出新 PDF（一次性修改，不回写原文件）。
    Edit {
        /// 输入的 PDF 文件或 unpack 产物目录
        input: PathBuf,
        /// 元素路径（如 /page[1]/text[2]；add page 用 /page）
        page_path: String,
        #[command(subcommand)]
        action: EditAction,
    },
    /// 将页面渲染为 HTML / PNG / PDF（-o 扩展名决定格式，PNG/PDF 需 Chrome）
    Render {
        /// 输入的 PDF 文件或 unpack 产物目录
        input: PathBuf,
        /// 页面路径（如 /page[1]；PNG 单页必填，HTML/PDF 缺省全部页）
        page_path: Option<String>,
        /// 渲染缩放（图像像素倍率，1 = 96dpi）
        #[arg(long, default_value_t = 1.0)]
        scale: f64,
    },
    /// 校验产物目录结构（颜色/引用/越界/旋转），error 级问题非空时退出码为 3
    Validate {
        /// 输入的 PDF 文件或 unpack 产物目录
        input: PathBuf,
    },
    /// 导出可回放的编辑指令 JSON（页面级，页面 id 幂等）
    Dump {
        /// 输入的 PDF 文件或 unpack 产物目录
        input: PathBuf,
    },
    /// 回放 dump 风格编辑指令（默认遇错即停；--force 跳过继续；空目录可引导创建）
    Batch {
        /// 输入的产物目录（PDF 请先 unpack；不存在的空目录会自动创建 document.json）
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
    /// 常驻编辑服务：stdin 逐行 JSON 操作（serve <产物目录>）
    Serve {
        /// unpack 产物目录
        input: PathBuf,
    },
    /// MCP 服务（stdio + JSON-RPC 2.0）
    Mcp,
    /// 能力速查：help / help <元素>；--json 输出机器可读能力 schema
    Help {
        /// 元素名（page/text/rect/image/polyline/path/shading/pattern_rect/group）
        topic: Option<String>,
    },
    /// 产物目录 JSON schema：人读指引；--json 输出机器可读 schema
    Schema,
}

/// view 视图模式
#[derive(clap::ValueEnum, Clone)]
enum ViewMode {
    /// 文本 + 媒体（去图形，媒体为绝对路径）
    Text,
    /// 元素布局树（类型/几何/文字摘要/CLI 路径）
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
    /// 在路径下新增元素（--type page 时路径为 /page，追加整页）
    Add {
        /// 元素类型（text/rect/polyline/image/group/page）
        #[arg(long)]
        r#type: String,
        /// 属性列表 k=v
        #[arg(long)]
        prop: Vec<String>,
    },
    /// 删除路径指向的元素（/page[N] 删除整页）
    Remove,
}

#[derive(Subcommand)]
enum EnvCmd {
    /// 探测外部依赖（soffice / chrome / tectonic）
    Check,
}

#[derive(Subcommand)]
enum ExtractCmd {
    /// 按页提取纯文本（--layout 版式重建 / --format markdown / --ocr 扫描件识别）
    Text {
        file: PathBuf,
        /// 页码，如 "1,3,5-8"；缺省全部
        #[arg(short, long)]
        pages: Option<String>,
        /// 版式重建输出：XY-cut 列识别 + 空格锚点对齐（多栏/财报/论文）
        #[arg(long)]
        layout: bool,
        /// 输出格式：text（默认）/ markdown（标题层级/列表/段落/表格）
        #[arg(long)]
        format: Option<String>,
        /// 对扫描页/无文本层页送 OCR：显式 HTTP 服务（--ocr-server-url / UNIOCR_URL）
        /// 优先 → 本机原生（macOS 系统 Vision，免安装）→ tesseract 兜底
        #[arg(long)]
        ocr: bool,
        /// OCR 语言（ISO 639-1 或 tesseract 语言码）
        #[arg(long)]
        ocr_language: Option<String>,
        /// OCR HTTP 服务地址（liteparse 规范，缺省读 UNIOCR_URL 环境变量）
        #[arg(long)]
        ocr_server_url: Option<String>,
        /// OCR 渲染 DPI（越大越慢越准）
        #[arg(long, default_value_t = 150.0)]
        ocr_dpi: f64,
    },
    /// 表格提取（行聚类近似）
    Table {
        file: PathBuf,
        #[arg(short, long)]
        pages: Option<String>,
    },
    /// 导出内嵌图片到目录（-o，缺省当前目录）
    Image { file: PathBuf },
}

#[derive(Subcommand)]
enum PagesCmd {
    /// 合并多个 PDF（-o 必填）
    Merge { files: Vec<PathBuf> },
    /// 逐页拆分到目录（-o，缺省当前目录）
    Split { file: PathBuf },
    /// 旋转（90/180/270）
    Rotate {
        file: PathBuf,
        deg: i64,
        #[arg(short, long)]
        pages: Option<String>,
    },
    /// 裁剪（pt 坐标 l,b,r,t）
    Crop {
        file: PathBuf,
        /// "l,b,r,t"
        box_spec: String,
        #[arg(short, long)]
        pages: Option<String>,
    },
    /// 删除空白页
    Clean { file: PathBuf },
}

#[derive(Subcommand)]
enum MetaCmd {
    /// 读取元数据
    Get { file: PathBuf },
    /// 写入元数据（-d '{"Title":"...","Author":"..."}'，自动更新 ModDate）
    Set {
        file: PathBuf,
        /// JSON 字符串
        #[arg(short, long)]
        data: String,
    },
    /// 批量写入 Z.ai 品牌元数据（多文件原地，单文件可用 -o）
    Brand {
        files: Vec<PathBuf>,
        #[arg(short, long)]
        title: Option<String>,
    },
}

#[derive(Subcommand)]
enum FormCmd {
    /// 字段树分析
    Info { file: PathBuf },
    /// 填写表单（-d '{"字段名":"值"}'）
    Fill {
        file: PathBuf,
        #[arg(short, long)]
        data: String,
    },
}

#[derive(Subcommand)]
enum ConvertCmd {
    /// Office → PDF。--engine：auto（默认，探测本机 soffice；缺 → native 自研）/
    /// soffice（LibreOffice 保真，未装报错）/ native（自研简化排版）
    Office {
        file: PathBuf,
        #[arg(long, default_value = "auto")]
        engine: String,
    },
    /// PDF → Word（产物目录映射：标题/段落/列表/表格/图片，语义优先）
    Docx { file: PathBuf },
    /// PDF → PowerPoint（页→slide 绝对定位近无损）
    Pptx { file: PathBuf },
    /// PDF → Excel（识别表格逐张成 sheet；无表格页单列文本）
    Xlsx { file: PathBuf },
    /// HTML → PDF（Chromium headless）
    Html {
        file: PathBuf,
        /// 渲染前注入的额外 CSS
        #[arg(long)]
        css: Option<PathBuf>,
    },
    /// LaTeX → PDF（tectonic）
    Latex {
        file: PathBuf,
        /// 编译轮数
        #[arg(short, long, default_value_t = 1)]
        runs: u32,
        #[arg(short, long)]
        keep_logs: bool,
    },
}

#[derive(Subcommand)]
enum PaletteCmd {
    /// 单套调色板（bg/mid/accent/text/muted/surface）
    Generate {
        /// 标题（自动派生 intent，可被 --intent 覆盖）
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value = "neutral")]
        intent: String,
        #[arg(long, default_value = "minimal")]
        mode: String,
        #[arg(long, default_value = "auto")]
        harmony: String,
        /// 输出格式：json | css | python
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// 级联调色板（12 角色 × 5 面积层级 + 语义色）
    Cascade {
        #[arg(long)]
        title: Option<String>,
        #[arg(long, default_value = "neutral")]
        intent: String,
        #[arg(long, default_value = "minimal")]
        mode: String,
        #[arg(long, default_value = "auto")]
        harmony: String,
        /// 输出格式：summary | json | css | reportlab
        #[arg(long, default_value = "summary")]
        format: String,
        #[arg(long)]
        seed: Option<u64>,
    },
}

#[derive(Subcommand)]
enum DesignCmd {
    /// 算法化 SVG 背景（stdout 输出 SVG 文本）
    Svg {
        #[arg(long, default_value = "flow")]
        svg_type: String,
        #[arg(long, default_value = "720x960")]
        dimensions: String,
        #[arg(long, default_value = "#8a8a8a")]
        color: String,
    },
    /// 空间张力布局计算
    Layout {
        #[arg(long, default_value = "hero,body,meta")]
        elements: String,
        #[arg(long, default_value = "720x960")]
        dimensions: String,
        #[arg(long, default_value = "offset")]
        style: String,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// 标题关键词 → intent
    Derive { text: String },
    /// 审计调色板 JSON（单套或级联自动识别）
    Audit {
        #[arg(long)]
        palette_json: PathBuf,
    },
}

#[derive(Subcommand)]
enum CodeCmd {
    /// 就地清洗生成脚本
    Sanitize { file: PathBuf },
}

#[derive(Subcommand)]
enum ContentCmd {
    /// 清洗文本内容（默认干跑报告，--apply 写回）
    Sanitize {
        file: PathBuf,
        #[arg(long)]
        apply: bool,
    },
}

#[derive(Subcommand)]
enum CheckCmd {
    /// 字符级扫描（缺字形代理/控制/零宽/Bidi/PUA）
    Font { file: PathBuf },
    /// 目录页码校验
    Toc { file: PathBuf },
}

fn main() {
    // 点分兼容：extract.text → extract text（原 pdf.py 习惯）
    let raw: Vec<String> = std::env::args().collect();
    let mut argv: Vec<String> = Vec::with_capacity(raw.len() + 4);
    for (i, a) in raw.into_iter().enumerate() {
        if i == 0 {
            argv.push(a);
            continue;
        }
        if let Some((group, rest)) = a.split_once('.') {
            if matches!(
                group,
                "extract"
                    | "pages"
                    | "meta"
                    | "form"
                    | "convert"
                    | "palette"
                    | "design"
                    | "code"
                    | "content"
                    | "check"
                    | "env"
            ) && !rest.is_empty()
            {
                argv.push(group.to_string());
                argv.push(rest.to_string());
                continue;
            }
        }
        argv.push(a);
    }

    let cli = match Cli::try_parse_from(argv) {
        Ok(c) => c,
        Err(e) => {
            e.print().unwrap();
            std::process::exit(2);
        }
    };
    let out = Output::new(cli.json, cli.quiet, cli.verbose);
    let code = match dispatch(&cli, &out) {
        Ok(code) => code,
        Err(CmdError(e)) => {
            out.error_full(&e.code, &e.message, None, e.suggestion.as_deref());
            EXIT_FAILURE
        }
    };
    std::process::exit(code);
}

/// dispatch 层错误：CliError 直接透传，anyhow（库层）/io/json 错误统一转 CliError。
/// （CliError 与 anyhow 均为外部类型，orphan rule 下用本地包装实现 From。）
struct CmdError(CliError);

impl From<CliError> for CmdError {
    fn from(e: CliError) -> Self {
        Self(e)
    }
}

impl From<anyhow::Error> for CmdError {
    fn from(e: anyhow::Error) -> Self {
        Self(to_cli_error(e))
    }
}

impl From<std::io::Error> for CmdError {
    fn from(e: std::io::Error) -> Self {
        Self(CliError::with_code("io", e.to_string()))
    }
}

impl From<serde_json::Error> for CmdError {
    fn from(e: serde_json::Error) -> Self {
        Self(CliError::with_code("json", e.to_string()))
    }
}

/// 库层 anyhow 错误 → CliError：io/json 保留类型码，其余按输入非法处理
fn to_cli_error(e: anyhow::Error) -> CliError {
    if let Some(io) = e.downcast_ref::<std::io::Error>() {
        return CliError::with_code("io", io.to_string());
    }
    if let Some(j) = e.downcast_ref::<serde_json::Error>() {
        return CliError::with_code("json", j.to_string());
    }
    // unpack 对加密 PDF 的拒绝（ai-pdf/src/unpack.rs）经 anyhow 丢失类型，按消息归码
    if e.to_string().contains("encrypted") {
        return CliError::with_code("encrypted", e.to_string());
    }
    CliError::new(e.to_string())
}

/// 写盘命令在 --json 模式下的结果 JSON：{"ok":true, ...payload}
fn ok_payload(payload: Value) -> Value {
    if let Value::Object(map) = payload {
        let mut out = serde_json::Map::new();
        out.insert("ok".into(), json!(true));
        for (k, v) in map {
            out.insert(k, v);
        }
        Value::Object(out)
    } else {
        json!({ "ok": true, "result": payload })
    }
}

fn dispatch(cli: &Cli, out: &Output) -> Result<i32, CmdError> {
    let output = &cli.output;
    match &cli.command {
        Cmd::Env(EnvCmd::Check) => {
            out.emit_json(&convert::env_check())?;
        }

        Cmd::Extract(ExtractCmd::Text {
            file,
            pages,
            layout,
            format,
            ocr,
            ocr_language,
            ocr_server_url,
            ocr_dpi,
        }) => {
            let doc = load(file)?;
            let target = resolve_pages(&doc, pages.as_deref())?;
            let pts = text::extract_pages(&doc, &target)?;
            let markdown = format.as_deref() == Some("markdown");
            let engine = if *ocr {
                Some(json2pdf::ocr::engine(ocr_server_url.as_deref(), None)?)
            } else {
                None
            };
            let engine_name = engine.as_ref().map(|e| e.name()).unwrap_or("");
            let language = ocr_language.as_deref().unwrap_or("eng");
            let page_reports: Vec<Value> = pts
                .iter()
                .map(|p| {
                    let complexity = json2pdf::complexity::complexity_json(&p.complexity);
                    // 文本来源：OCR（needs_ocr 页）> markdown > layout > 线性
                    let mut source = "native";
                    let mut text = if markdown {
                        source = "markdown";
                        json2pdf::markdown::page_markdown(p)
                    } else if *layout {
                        source = "layout";
                        json2pdf::layout::project_page(p)
                    } else {
                        p.text()
                    };
                    let mut ocr_info = Value::Null;
                    if let Some(eng) = &engine {
                        if json2pdf::complexity::needs_ocr(&p.complexity.reasons) {
                            source = "ocr";
                            match render_and_ocr(file, p.page, eng, language, *ocr_dpi, markdown) {
                                Ok(ocr_text) => {
                                    text = ocr_text;
                                    ocr_info =
                                        json!({"ok": true, "language": language, "engine": engine_name});
                                }
                                Err(e) => {
                                    ocr_info = json!(
                                        {"ok": false, "language": language, "engine": engine_name, "error": e.to_string()}
                                    );
                                    // native 文本为空的页：保留占位说明
                                    if text.trim().is_empty() {
                                        text = String::new();
                                    }
                                }
                            }
                        }
                    }
                    json!({
                        "page": p.page,
                        "chars": text.chars().count(),
                        "source": source,
                        "text": text,
                        "complexity": complexity,
                        "ocr": ocr_info,
                    })
                })
                .collect();
            out.emit_json(&json!({"file": file.display().to_string(), "pages": page_reports}))?;
        }
        Cmd::Extract(ExtractCmd::Table { file, pages }) => {
            let doc = load(file)?;
            let target = resolve_pages(&doc, pages.as_deref())?;
            let pts = text::extract_pages(&doc, &target)?;
            let tables = text::detect_tables(&pts);
            if tables.is_empty() {
                out.status("未检测到表格（启发式：≥2 列一致行合并）");
            }
            out.emit_json(&json!({"file": file.display().to_string(), "tables": tables}))?;
        }
        Cmd::Extract(ExtractCmd::Image { file }) => {
            let doc = load(file)?;
            let all: Vec<u32> = (1..=doc.get_pages().len() as u32).collect();
            let dir = cli.output.clone().unwrap_or_else(|| PathBuf::from("."));
            let files = images::extract_images(&doc, &all, &dir)?;
            out.status(&format!(
                "已导出 {} 张图片 → {}",
                files.len(),
                dir.display()
            ));
            out.emit_result(&ok_payload(json!({
                "output_dir": dir.display().to_string(),
                "images": files,
            })))?;
        }

        Cmd::Pages(PagesCmd::Merge { files }) => {
            if files.len() < 2 {
                return Err(CliError::new("pages merge 至少需要 2 个输入文件")
                    .suggest("用法: json2pdf pages merge a.pdf b.pdf -o merged.pdf")
                    .into());
            }
            let o = require_output(cli, "pages merge")?;
            let payload = page_ops::merge(files, &o)?;
            out.status(&format!("已合并 {} 个文件 → {}", files.len(), o.display()));
            out.emit_result(&ok_payload(payload))?;
        }
        Cmd::Pages(PagesCmd::Split { file }) => {
            let dir = cli.output.clone().unwrap_or_else(|| PathBuf::from("."));
            let payload = page_ops::split(file, &dir)?;
            out.status(&format!("已逐页拆分 → {}", dir.display()));
            out.emit_result(&ok_payload(payload))?;
        }
        Cmd::Pages(PagesCmd::Rotate { file, deg, pages }) => {
            let doc = load(file)?;
            let target = resolve_pages_opt(&doc, pages.as_deref())?;
            let o = require_output(cli, "pages rotate")?;
            let payload = page_ops::rotate(file, *deg, &o, target.as_deref())?;
            out.status(&format!("已旋转 {}° → {}", deg, o.display()));
            out.emit_result(&ok_payload(payload))?;
        }
        Cmd::Pages(PagesCmd::Crop {
            file,
            box_spec,
            pages,
        }) => {
            let vals: Vec<f64> = box_spec
                .split(',')
                .map(|s| s.trim().parse::<f64>())
                .collect::<std::result::Result<_, _>>()
                .map_err(|_| {
                    CmdError(
                        CliError::new("box 必须是 4 个逗号分隔的数字")
                            .suggest("格式: l,b,r,t（pt），如 --box 50,60,545,780"),
                    )
                })?;
            if vals.len() != 4 {
                return Err(CliError::new("box 必须是 4 个逗号分隔的数字")
                    .suggest("格式: l,b,r,t（pt），如 --box 50,60,545,780")
                    .into());
            }
            let doc = load(file)?;
            let target = resolve_pages_opt(&doc, pages.as_deref())?;
            let o = require_output(cli, "pages crop")?;
            let payload = page_ops::crop(
                file,
                [vals[0], vals[1], vals[2], vals[3]],
                &o,
                target.as_deref(),
            )?;
            out.status(&format!("已裁剪 → {}", o.display()));
            out.emit_result(&ok_payload(payload))?;
        }
        Cmd::Pages(PagesCmd::Clean { file }) => {
            let o = require_output(cli, "pages clean")?;
            let payload = page_ops::clean(file, &o)?;
            out.status(&format!("已生成 → {}", o.display()));
            out.emit_result(&ok_payload(payload))?;
        }

        Cmd::Meta(MetaCmd::Get { file }) => {
            out.emit_json(&meta::get(file)?)?;
        }
        Cmd::Meta(MetaCmd::Set { file, data }) => {
            let map: serde_json::Map<String, Value> = serde_json::from_str(data).map_err(|e| {
                CmdError(
                    CliError::with_code("json", format!("-d 不是合法 JSON: {e}"))
                        .suggest(r#"格式: -d '{"Title":"标题","Author":"作者"}'"#),
                )
            })?;
            let o = require_output(cli, "meta set")?;
            let payload = meta::set(file, &o, &map)?;
            out.status(&format!("已写入元数据 → {}", o.display()));
            out.emit_result(&ok_payload(payload))?;
        }
        Cmd::Meta(MetaCmd::Brand { files, title }) => {
            let payload = meta::brand(files, cli.output.as_deref(), title.as_deref())?;
            out.status(&format!("已写入 {} 个文件的品牌元数据", files.len()));
            out.emit_result(&ok_payload(payload))?;
        }

        Cmd::Form(FormCmd::Info { file }) => {
            out.emit_json(&forms::info(file)?)?;
        }
        Cmd::Form(FormCmd::Fill { file, data }) => {
            let map: serde_json::Map<String, Value> = serde_json::from_str(data).map_err(|e| {
                CmdError(
                    CliError::with_code("json", format!("-d 不是合法 JSON: {e}"))
                        .suggest(r#"格式: -d '{"姓名":"张三"}'（键为字段全限定名）"#),
                )
            })?;
            let o = require_output(cli, "form fill")?;
            let payload = forms::fill(file, &o, &map)?;
            out.status(&format!("已填写表单 → {}", o.display()));
            out.emit_result(&ok_payload(payload))?;
        }

        Cmd::Convert(ConvertCmd::Office { file, engine }) => {
            let o = default_output(cli, file, "pdf");
            let payload = convert::convert_office(file, Some(&o), engine)?;
            out.status(&format!(
                "已转换（{} 引擎）→ {}",
                payload["engine"].as_str().unwrap_or("?"),
                o.display()
            ));
            out.emit_result(&ok_payload(payload))?;
        }
        Cmd::Convert(ConvertCmd::Docx { file }) => {
            let o = default_output(cli, file, "docx");
            let src = json2pdf::cross::open_source(file)?;
            let doc = json2pdf::cross::to_docx(&src)?;
            json2docx::generate(&doc, &o.display().to_string())
                .map_err(|e| CliError::new(format!("生成 docx 失败: {e}")))?;
            out.status(&format!("已转换 {} 页 → {}", src.pages.len(), o.display()));
            out.emit_result(&ok_payload(json!({
                "input": file.display().to_string(),
                "output": o.display().to_string(),
                "pages": src.pages.len(),
                "engine": "native-mapping",
            })))?;
        }
        Cmd::Convert(ConvertCmd::Pptx { file }) => {
            let o = default_output(cli, file, "pptx");
            let src = json2pdf::cross::open_source(file)?;
            let pres = json2pdf::cross::to_pptx(&src)?;
            json2pptx::generate(&pres, &o.display().to_string())
                .map_err(|e| CliError::new(format!("生成 pptx 失败: {e}")))?;
            out.status(&format!("已转换 {} 页 → {}", src.pages.len(), o.display()));
            out.emit_result(&ok_payload(json!({
                "input": file.display().to_string(),
                "output": o.display().to_string(),
                "pages": src.pages.len(),
                "engine": "native-mapping",
            })))?;
        }
        Cmd::Convert(ConvertCmd::Xlsx { file }) => {
            let o = default_output(cli, file, "xlsx");
            let src = json2pdf::cross::open_source(file)?;
            let wb = json2pdf::cross::to_xlsx(&src)?;
            json2xlsx::generate(&wb, &o)
                .map_err(|e| CliError::new(format!("生成 xlsx 失败: {e}")))?;
            out.status(&format!(
                "已转换 {} 页（{} sheets）→ {}",
                src.pages.len(),
                wb.sheets.len(),
                o.display()
            ));
            out.emit_result(&ok_payload(json!({
                "input": file.display().to_string(),
                "output": o.display().to_string(),
                "pages": src.pages.len(),
                "sheets": wb.sheets.len(),
                "engine": "native-mapping",
            })))?;
        }
        Cmd::Convert(ConvertCmd::Html { file, css }) => {
            let o = default_output(cli, file, "pdf");
            let payload = convert::convert_html(file, Some(&o), css.as_deref())?;
            out.status(&format!("已转换 → {}", o.display()));
            out.emit_result(&ok_payload(payload))?;
        }
        Cmd::Convert(ConvertCmd::Latex {
            file,
            runs,
            keep_logs: _,
        }) => {
            let payload = convert::convert_latex(file, *runs, false)?;
            out.status(&format!(
                "已编译 → {}",
                file.with_extension("pdf").display()
            ));
            out.emit_result(&ok_payload(payload))?;
        }

        Cmd::Palette(PaletteCmd::Generate {
            title,
            intent,
            mode,
            harmony,
            format,
            seed,
        }) => {
            let intent = resolve_intent(title.as_deref(), intent);
            let p = palette::generate_color_palette(&intent, mode, Some(harmony.as_str()), *seed);
            match format.as_str() {
                "json" => out.emit_json(&p)?,
                "css" => print!("{}", palette::palette_to_css(&p)),
                "python" => print!("{}", palette::palette_to_reportlab(&p)),
                f => {
                    return Err(CliError::new(format!("未知输出格式 '{f}'"))
                        .suggest("可选值: json | css | python")
                        .into())
                }
            }
        }
        Cmd::Palette(PaletteCmd::Cascade {
            title,
            intent,
            mode,
            harmony,
            format,
            seed,
        }) => {
            let intent = resolve_intent(title.as_deref(), intent);
            let c = palette::generate_cascade_palette(&intent, mode, Some(harmony.as_str()), *seed);
            match format.as_str() {
                "json" => out.emit_json(&c)?,
                "summary" => out.emit_json(&json!({
                    "meta": c["meta"],
                    "cover": c["cover"],
                    "body": c["body"],
                    "charts": c["charts"],
                    "semantic": c["semantic"],
                }))?,
                "css" => print!("{}", palette::cascade_to_css(&c)),
                "reportlab" => print!("{}", palette::cascade_to_reportlab(&c)),
                f => {
                    return Err(CliError::new(format!("未知输出格式 '{f}'"))
                        .suggest("可选值: summary | json | css | reportlab")
                        .into())
                }
            }
        }

        Cmd::Design(DesignCmd::Svg {
            svg_type,
            dimensions,
            color,
        }) => {
            let (w, h) = parse_dims(dimensions);
            let svg = design::generate_generative_svg(svg_type, w as i64, h as i64, color);
            print!("{svg}");
        }
        Cmd::Design(DesignCmd::Layout {
            elements,
            dimensions,
            style,
            seed,
        }) => {
            let (w, h) = parse_dims(dimensions);
            let elems: Vec<String> = elements
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if elems.is_empty() {
                return Err(CliError::new("--elements 不能为空")
                    .suggest("逗号分隔的元素名列表，如 --elements hero,body,meta")
                    .into());
            }
            out.emit_json(&design::calculate_layout(&elems, w, h, style, *seed))?;
        }
        Cmd::Design(DesignCmd::Derive { text }) => {
            out.emit_json(&json!({"text": text, "intent": design::derive_intent(text)}))?;
        }
        Cmd::Design(DesignCmd::Audit { palette_json }) => {
            let raw = std::fs::read_to_string(palette_json)?;
            let v: Value = serde_json::from_str(&raw)?;
            let violations = if v.get("roles").is_some() {
                palette::audit_cascade_palette(&v)
            } else {
                palette::audit_palette(&v)
            };
            out.emit_json(&json!({"clean": violations.is_empty(), "violations": violations}))?;
        }

        Cmd::Code(CodeCmd::Sanitize { file }) => {
            let src = std::fs::read_to_string(file)?;
            let cleaned = sanitize::code_sanitize(&src);
            std::fs::write(file, &cleaned)?;
            out.status(&format!("已清洗 → {}", file.display()));
            out.emit_result(&ok_payload(json!({
                "file": file.display().to_string(),
                "rewritten": true,
            })))?;
        }
        Cmd::Content(ContentCmd::Sanitize { file, apply }) => {
            let src = std::fs::read_to_string(file)?;
            let (cleaned, pua) = sanitize::content_sanitize(&src);
            if *apply {
                std::fs::write(file, &cleaned)?;
                out.status(&format!("已清洗并写回 → {}", file.display()));
            }
            out.emit_json(&json!({
                "file": file.display().to_string(),
                "applied": apply,
                "removed_chars": src.chars().count() - cleaned.chars().count(),
                "pua_removed": pua,
            }))?;
        }

        Cmd::Check(CheckCmd::Font { file }) => {
            let (v, code) = checks::font_check(file)?;
            out.emit_json(&v)?;
            if code != 0 {
                return Ok(EXIT_ISSUES);
            }
        }
        Cmd::Check(CheckCmd::Toc { file }) => {
            let (v, code) = checks::toc_check(file)?;
            out.emit_json(&v)?;
            if code != 0 {
                return Ok(EXIT_ISSUES);
            }
        }

        Cmd::Qa { files, skip_cover } => {
            if files.is_empty() {
                return Err(CliError::new("qa 至少需要一个 PDF 文件")
                    .suggest("用法: json2pdf qa report.pdf（可多个文件）")
                    .into());
            }
            let (v, code) = qa::qa(files, *skip_cover)?;
            out.emit_json(&v)?;
            if code != 0 {
                return Ok(EXIT_ISSUES);
            }
        }

        Cmd::Unpack { pdf, inline } => {
            if *inline {
                let out_file = cli
                    .output
                    .clone()
                    .unwrap_or_else(|| pdf.with_extension("inline.json"));
                let r = office_core::inline::unpack_inline(pdf, &out_file, |i, o| {
                    unpack::unpack(i, o).map(|_| ()).map_err(to_cli_error)
                })?;
                out.status(&format!(
                    "已内联解包 {} 页 → {}",
                    r.shards,
                    r.output.display()
                ));
                out.emit_result(&ok_payload(json!({
                    "ok": true,
                    "output": r.output.display().to_string(),
                    "pages": r.shards,
                    "inline": true
                })))?;
            } else {
                let out_dir = cli
                    .output
                    .clone()
                    .unwrap_or_else(|| pdf.with_extension("").to_path_buf());
                let result = unpack::unpack(pdf, &out_dir)?;
                out.status(&format!(
                    "已解包 {} 页 → {}",
                    result.pages,
                    out_dir.display()
                ));
                out.emit_result(&ok_payload(unpack::summary(&result, &out_dir)))?;
            }
        }
        Cmd::Repack { dir } => {
            let out_pdf = cli
                .output
                .clone()
                .unwrap_or_else(|| dir.with_extension("pdf"));
            let result = repack::repack(dir, &out_pdf)?;
            out.status(&format!(
                "已生成 {}（{} 页，{} 个警告）",
                out_pdf.display(),
                result.pages,
                result.warnings.len()
            ));
            out.emit_result(&ok_payload(json!({
                "output": out_pdf.display().to_string(),
                "pages": result.pages,
                "images": result.images,
                "fonts": result.fonts,
                "warnings": result.warnings,
            })))?;
        }
        Cmd::View {
            input,
            page_path,
            mode,
        } => {
            let (root, _tmp) = product::resolve_input(input)?;
            let doc = product::load_document(&root)?;
            let pages = product::load_pages(&root)?;
            let ep = path::parse(page_path)?;
            let resolved = path::resolve(&pages, &ep)?;
            let value = match mode {
                ViewMode::Text => view::view_text(
                    &doc,
                    &pages[resolved.page_idx],
                    resolved.page_idx + 1,
                    &root,
                ),
                ViewMode::Layout => {
                    view::view_layout(&doc, &pages[resolved.page_idx], resolved.page_idx + 1)
                }
            };
            out.emit_json(&value)?;
        }
        Cmd::Edit {
            input,
            page_path,
            action,
        } => {
            let (root, tmp) = product::resolve_input(input)?;
            let is_write = matches!(
                action,
                EditAction::Set { .. } | EditAction::Add { .. } | EditAction::Remove
            );
            // 输入是 PDF 时产物为一次性临时目录：写操作必须给出 -o 输出新文件
            if is_write && tmp.is_some() && output.is_none() {
                return Err(CliError::new(
                    "修改 PDF 需要 -o <输出.pdf>（无状态模式下不原地改写输入文件）；\
                     或先 `unpack` 到产物目录，再对产物目录执行 edit 原地修改",
                )
                .into());
            }
            let mut doc = product::load_document(&root)?;
            let mut pages = product::load_pages(&root)?;
            let ep = path::parse(page_path)?;
            let (value, touched_page) = match action {
                EditAction::Get => {
                    let resolved = path::resolve(&pages, &ep)?;
                    (edit::get(&doc, &pages, &resolved)?, None)
                }
                EditAction::Set { prop } => {
                    let props = edit::parse_props(prop)?;
                    let resolved = path::resolve(&pages, &ep)?;
                    let r = edit::set(&mut pages, &resolved, &props)?;
                    product::save_page(&root, resolved.page_idx, &pages[resolved.page_idx])?;
                    (r, Some(resolved.page_idx))
                }
                EditAction::Add { r#type, prop } => {
                    let props = edit::parse_props(prop)?;
                    if r#type == "page" {
                        // add page：/page 为文档容器，无需解析到既有页
                        let resolved = path::Resolved {
                            page_idx: 0,
                            chain: vec![],
                        };
                        let r =
                            edit::add(&mut doc, &root, &mut pages, &resolved, &ep, r#type, &props)?;
                        (r, None)
                    } else {
                        let resolved = path::resolve(&pages, &ep)?;
                        let r =
                            edit::add(&mut doc, &root, &mut pages, &resolved, &ep, r#type, &props)?;
                        product::save_page(&root, resolved.page_idx, &pages[resolved.page_idx])?;
                        (r, Some(resolved.page_idx))
                    }
                }
                EditAction::Remove => {
                    let resolved = path::resolve(&pages, &ep)?;
                    let page_idx = resolved.page_idx;
                    let r = edit::remove(&mut doc, &root, &mut pages, &resolved)?;
                    if resolved.chain.is_empty() {
                        product::save_document(&root, &doc)?;
                    } else {
                        product::save_page(&root, page_idx, &pages[page_idx])?;
                    }
                    (r, None)
                }
            };
            // 显式 -o：产物修改后重建 PDF（输入为 PDF 时写操作必填）
            if let Some(out_pdf) = output {
                repack::repack(&root, out_pdf)?;
                out.status(&format!("已生成: {}", out_pdf.display()));
            }
            let _ = touched_page;
            out.emit_json(&value)?;
        }
        Cmd::Render {
            input,
            page_path,
            scale,
        } => {
            let (root, _tmp) = product::resolve_input(input)?;
            let doc = product::load_document(&root)?;
            let pages = product::load_pages(&root)?;
            let page = match page_path {
                Some(p) => {
                    let ep = path::parse(p)?;
                    if !ep.elements.is_empty() {
                        return Err(CliError::new("render 的路径必须是页级（如 /page[1]）").into());
                    }
                    Some(ep.page)
                }
                None => None,
            };
            let fmt = output
                .as_ref()
                .and_then(|o| o.extension().map(|e| e.to_string_lossy().to_lowercase()))
                .unwrap_or_else(|| "html".to_string());
            let out_path = output.clone().unwrap_or_else(|| {
                let stem = input
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "doc".to_string());
                match (fmt.as_str(), page) {
                    ("png", Some(p)) => PathBuf::from(format!("{stem}_page{p}.png")),
                    _ => PathBuf::from(format!("{stem}.{fmt}")),
                }
            });
            match fmt.as_str() {
                "html" => {
                    render::render_html(&doc, &pages, &root, page, &out_path, *scale)?;
                    out.status(&format!("已渲染: {}", out_path.display()));
                    out.emit_result(&ok_payload(render::summary(&out_path, "html", page)))?;
                }
                "png" => {
                    let page_no = page.unwrap_or(1);
                    render::render_png(&doc, &pages, &root, page_no, &out_path, *scale)?;
                    out.status(&format!("已渲染: {}", out_path.display()));
                    out.emit_result(&ok_payload(render::summary(
                        &out_path,
                        "png",
                        Some(page_no),
                    )))?;
                }
                "pdf" => {
                    render::render_pdf(&doc, &pages, &root, &out_path, *scale)?;
                    out.status(&format!("已渲染: {}", out_path.display()));
                    out.emit_result(&ok_payload(render::summary(&out_path, "pdf", None)))?;
                }
                other => {
                    return Err(CliError::new(format!("未知渲染格式: {other}"))
                        .suggest("-o 扩展名决定格式: html / png / pdf")
                        .into())
                }
            }
        }
        Cmd::Validate { input } => {
            let (root, _tmp) = product::resolve_input(input)?;
            let issues = validate::run_on_dir(&root)?;
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
        Cmd::Dump { input } => {
            let (root, _tmp) = product::resolve_input(input)?;
            let doc = product::load_document(&root)?;
            let pages = product::load_pages(&root)?;
            out.emit_json(&dump::run(&doc, &pages))?;
        }
        Cmd::Batch {
            input,
            input_file,
            commands,
            force,
        } => {
            // 产物目录必须存在；不存在的空路径引导创建（dump → batch 从零构建）
            if !product::is_product_dir(input) {
                if input.exists() {
                    let empty = std::fs::read_dir(input)
                        .map(|d| d.filter_map(|e| e.ok()).count())
                        .unwrap_or(usize::MAX);
                    if empty != 0 {
                        return Err(CliError::new("batch 需要产物目录（PDF 请先 unpack）")
                            .suggest("先 json2pdf unpack <文件.pdf> -o <目录>")
                            .into());
                    }
                }
                std::fs::create_dir_all(input)?;
                std::fs::write(
                    input.join("document.json"),
                    serde_json::to_string_pretty(&json!({
                        "version": 2,
                        "page_size": { "width": 595.28, "height": 841.89 },
                        "pages": [],
                    }))
                    .unwrap(),
                )?;
            }
            let raw = read_batch_commands(input_file.as_deref(), commands.as_deref())?;
            let v: Value = serde_json::from_str(&raw).map_err(|e| {
                CliError::new(format!("指令 JSON 解析失败: {e}"))
                    .suggest(r#"指令格式: {"commands":[{"op":"set","path":"/page[1]/text[1]","props":{"text":"新文本"}}]}"#)
            })?;
            let steps = office_core::batch::parse_steps(&v).map_err(CliError::new)?;
            let mut doc = product::load_document(input)?;
            // dump 回放：指令中的 page_size 覆盖空目录引导值
            if doc.pages.is_empty() {
                if let Some(size) = v.get("page_size") {
                    if let (Some(w), Some(h)) = (
                        size.get("width").and_then(|x| x.as_f64()),
                        size.get("height").and_then(|x| x.as_f64()),
                    ) {
                        doc.page_size = json2pdf::model::PageSize {
                            width: w,
                            height: h,
                        };
                    }
                }
            }
            let mut pages = product::load_pages(input)?;
            let report = batch::run_steps(&mut doc, input, &mut pages, &steps, !force);
            let failed = report["failed"].as_u64().unwrap_or(0);
            if failed == 0 {
                product::save_all(input, &doc, &pages)?;
            }
            out.emit_json(&report)?;
            if failed > 0 {
                return Ok(EXIT_FAILURE);
            }
        }
        Cmd::Serve { input } => {
            serve::run(input)?;
        }
        Cmd::Mcp => {
            mcp::run().map_err(CliError::new)?;
        }
        Cmd::Help { topic } => {
            help::run(topic.as_deref(), out)?;
        }
        Cmd::Schema => {
            if out.json_mode() {
                out.emit_json(&product_schema())?;
            } else {
                print!("{SCHEMA_DOC}");
            }
        }
    }
    Ok(EXIT_SUCCESS)
}

/// 对单页栅格化（产物目录 → HTML → Chrome PNG）后送 OCR，返回版式文本。
/// OCR 依赖"看"的能力：本机需 Chrome。
fn render_and_ocr(
    pdf: &Path,
    page: u32,
    engine: &dyn json2pdf::ocr::OcrEngine,
    language: &str,
    dpi: f64,
    markdown: bool,
) -> anyhow::Result<String> {
    let png = std::env::temp_dir().join(format!(
        "json2pdf-ocr-page{}-{}.png",
        page,
        std::process::id()
    ));
    let _ = std::fs::remove_file(&png);
    let out = Output::new(false, true, false); // 静默
    let fmt = "png";
    let scale = (dpi / 96.0).max(0.5);
    // render 单页：借用本文件的 render 分支逻辑（走 CLI 命令路径最省事）
    let status = std::process::Command::new(std::env::current_exe()?)
        .args([
            "render",
            &pdf.display().to_string(),
            &format!("/page[{page}]"),
            "--scale",
            &format!("{scale}"),
            "-o",
            &png.display().to_string(),
            "--quiet",
        ])
        .status()?;
    if !status.success() || !png.exists() {
        anyhow::bail!("OCR 前的页面栅格化失败（需要 Chrome）");
    }
    let png_bytes = std::fs::read(&png)?;
    let _ = std::fs::remove_file(&png);
    let _ = out;
    let _ = fmt;
    let lines = engine.recognize(&png_bytes, language)?;
    if lines.is_empty() {
        return Ok(String::new());
    }
    // 页高：unpack 粒度拿不到 → 按 bbox 最大 y_bottom 估算页面高（OCR bbox 是像素）
    let page_h_px = lines.iter().map(|l| l.bbox.3).fold(0.0f64, f64::max) + 10.0;
    // 排序 + 拼接（layout 模式下按行分列对齐的近似：直接按 y 分行）
    let mut sorted = lines;
    sorted.sort_by(|a, b| {
        a.bbox
            .1
            .partial_cmp(&b.bbox.1)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let _ = page_h_px;
    let scale_back = 72.0 / dpi; // 像素 → pt
    let mut out_text = String::new();
    let mut last_top: Option<f64> = None;
    let mut row: Vec<String> = Vec::new();
    for l in sorted {
        if let Some(t) = last_top {
            if (l.bbox.1 - t).abs() > 12.0 * scale_back * dpi / 96.0 {
                out_text.push_str(&row.join(" "));
                out_text.push('\n');
                row.clear();
            }
        }
        row.push(l.text.clone());
        last_top = Some(l.bbox.1);
    }
    if !row.is_empty() {
        out_text.push_str(&row.join(" "));
        out_text.push('\n');
    }
    if markdown {
        // 简单段落化：连续行合并由调用方的 markdown 管线处理——OCR 路径暂给纯文本
        let _ = markdown;
    }
    Ok(out_text)
}

/// 组合产物目录 schema：document.json 与 pages/page-NNN.json 两个根，
/// 元素与字体等被引用类型在各自 $defs 内。机器可读，字段权威。
fn product_schema() -> Value {
    json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "json2pdf 产物目录",
        "description": "unpack（PDF→JSON）与 repack（JSON→PDF）的统一中间形态：document.json + pages/*.json + media/ + fonts/。坐标一律 pt、y 向上，文本 y 为基线。",
        "document": schemars::schema_for!(json2pdf::model::DocumentModel),
        "page": schemars::schema_for!(json2pdf::model::PageModel),
    })
}

fn require_output(cli: &Cli, cmd: &str) -> Result<PathBuf, CliError> {
    cli.output.clone().ok_or_else(|| {
        CliError::new(format!("{cmd} 需要 -o 显式指定输出（绝不回写输入文件）"))
            .suggest(format!("用法: json2pdf {cmd} … -o <输出路径>"))
    })
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

/// -o 缺省时与输入同目录同名换扩展名
fn default_output(cli: &Cli, input: &Path, ext: &str) -> PathBuf {
    cli.output
        .clone()
        .unwrap_or_else(|| input.with_extension(ext))
}

fn parse_dims(s: &str) -> (f64, f64) {
    let parts: Vec<f64> = s
        .split(['x', '×', '*'])
        .filter_map(|p| p.trim().parse().ok())
        .collect();
    match parts.as_slice() {
        [w, h] => (*w, *h),
        _ => (720.0, 960.0),
    }
}

/// 加载 PDF（加密文档明确拒绝）
fn load(file: &Path) -> Result<lopdf::Document, CliError> {
    let doc = lopdf::Document::load(file).map_err(|e| {
        CliError::with_code("io", format!("load {}: {e}", file.display()))
            .suggest("确认文件存在且是未加密的合法 PDF")
    })?;
    if doc.is_encrypted() {
        return Err(
            CliError::with_code("encrypted", "文档已加密——不支持加密 PDF")
                .suggest("先解密后再处理"),
        );
    }
    Ok(doc)
}

fn resolve_pages(doc: &lopdf::Document, spec: Option<&str>) -> Result<Vec<u32>, CliError> {
    let n = doc.get_pages().len() as u32;
    match spec {
        Some(s) => util::parse_page_spec(s, n).map_err(|e| {
            CliError::new(format!("页码参数非法: {e}")).suggest("页码格式如 1,3,5-8；范围含两端")
        }),
        None => Ok((1..=n).collect()),
    }
}

fn resolve_pages_opt(
    doc: &lopdf::Document,
    spec: Option<&str>,
) -> Result<Option<Vec<u32>>, CliError> {
    match spec {
        Some(s) => Ok(Some(resolve_pages(doc, Some(s))?)),
        None => Ok(None),
    }
}

fn resolve_intent(title: Option<&str>, explicit: &str) -> String {
    if let Some(t) = title {
        if explicit.is_empty() || explicit == "neutral" {
            let derived = design::derive_intent(t);
            if derived != "neutral" {
                return derived;
            }
        }
    }
    if explicit.is_empty() {
        "neutral".into()
    } else {
        explicit.to_string()
    }
}

/// 产物目录 JSON schema 人读指引（json2pdf schema 默认输出；字段权威见 schema --json）。
const SCHEMA_DOC: &str = r##"# json2pdf 产物目录 JSON schema

产物目录是 unpack（PDF→JSON）与 repack（JSON→PDF）的统一中间形态：

```
doc/
├── document.json        # 顶层
├── pages/
│   ├── page-001.json    # 每页一个文件（尺寸/旋转/元素列表）
│   └── page-002.json
├── media/               # 图片实体（image.src 指向这里的相对路径）
└── fonts/               # 内嵌字体实体 + 元数据（font-NNN.ttf / font-NNN.json）
```

## document.json

```json
{
  "version": 2,
  "meta": { "title": "...", "author": "...", "subject": "...", "creator": "..." },
  "page_size": { "width": 595.28, "height": 841.89 },
  "pages": ["pages/page-001.json", "pages/page-002.json"],
  "fonts": ["fonts/font-001.json"]
}
```

- 坐标单位一律 pt（1/72 英寸），y 向上；文本的 y 为基线。
- page_size 为默认页尺寸（A4），页面可覆盖。

## pages/page-NNN.json

```json
{
  "width": 612.0, "height": 792.0,   // 可选，缺省用顶层 page_size
  "rotation": 0,                      // 0/90/180/270
  "elements": [ ... ]                 // 按绘制顺序
}
```

## 元素（elements，type 判别，共 8 种）

```json
{ "type": "text", "text": "你好 world", "x": 72, "y": 720,
  "size": 12, "font": "Helvetica", "color": "#23456e" }
{ "type": "rect", "x": 50, "y": 700, "w": 200, "h": 40,
  "fill": "#ffffff", "stroke": "#000000", "line_width": 1 }
{ "type": "path", "segments": [{"op":"m","points":[[0,0]]},{"op":"c","points":[[1,1],[2,2],[3,3]]}],
  "stroke": "#000000" }
{ "type": "polyline", "points": [[50,600],[150,650]], "stroke": "#000000" }
{ "type": "shading", "kind": "axial", "coords": [0,0,10,0],
  "stops": [{"offset":0,"color":"#ffffff"},{"offset":1,"color":"#000000"}] }
{ "type": "image", "src": "media/img-001.png", "x": 100, "y": 400, "w": 200, "h": 150 }
{ "type": "pattern_rect", "x": 0, "y": 0, "w": 100, "h": 100, "pattern": { ... } }
{ "type": "group", "matrix": [1,0,0,1,0,0], "bbox": [0,0,200,200], "children": [ ... ] }
```

- text.text 中的 `\n` 表示强制换行（行距 1.25 倍字号）；`font_id` 可引用 fonts/ 里的
  内嵌原字体（unpack 自动生成），保证字形与字宽一致。
- font 取值：标准 14（纯 ASCII 零嵌入，Helvetica/Times-Roman/Courier…）或
  `system:<family>`（含非 ASCII 文本时按 family 查找系统 TrueType 整档嵌入）。
- rect/path 的 fill/stroke 置 null 表示不填充/不描边。
- image.src 用产物根相对路径或绝对路径；png/jpg 原生支持，其他类型 repack 时
  以占位图代替并记录 warning（unpack 对不可解码图保留直通参数原样回写）。

完整字段与默认值以 `json2pdf schema --json`（即 skill/references/schema.json）为准。
"##;
