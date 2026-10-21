//! 格式分发：`astorm-office <格式> <命令...>` → 底层四 CLI。
//!
//! 统一 CLI 不重新实现任何文档能力，只做三件事：
//! 1. **格式 → 二进制**的映射（含别名与从参数里的文件扩展名推断）；
//! 2. **二进制定位**（环境变量 → 与自己同目录 → PATH，见 [`resolve_bin`]）；
//! 3. **原样转发**命令行与退出码（stdout/stderr 直通，不做包装）。

use office_core::CliError;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 四个底层格式 CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Word（.docx）：json2docx
    Word,
    /// Excel（.xlsx）：json2xlsx
    Excel,
    /// PowerPoint（.pptx）：json2pptx
    Ppt,
    /// PDF（.pdf）：json2pdf
    Pdf,
}

impl Format {
    /// 底层二进制名（不含平台后缀）
    pub fn bin_name(self) -> &'static str {
        match self {
            Format::Word => "json2docx",
            Format::Excel => "json2xlsx",
            Format::Ppt => "json2pptx",
            Format::Pdf => "json2pdf",
        }
    }

    /// 规范格式名（用于报告与提示）
    pub fn name(self) -> &'static str {
        match self {
            Format::Word => "word",
            Format::Excel => "excel",
            Format::Ppt => "ppt",
            Format::Pdf => "pdf",
        }
    }

    /// 覆盖二进制路径的环境变量（桌面端/自定义安装用）
    pub fn env_var(self) -> &'static str {
        match self {
            Format::Word => "ASTORM_JSON2DOCX",
            Format::Excel => "ASTORM_JSON2XLSX",
            Format::Ppt => "ASTORM_JSON2PPTX",
            Format::Pdf => "ASTORM_JSON2PDF",
        }
    }

    /// 该格式支持的文件扩展名（用于参数推断，小写、不含点）
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Format::Word => &["docx", "doc", "docm"],
            Format::Excel => &["xlsx", "xls", "xlsm", "csv", "tsv"],
            Format::Ppt => &["pptx", "ppt"],
            Format::Pdf => &["pdf"],
        }
    }

    /// 四个格式的固定顺序（帮助/列表用）
    pub fn all() -> [Format; 4] {
        [Format::Word, Format::Excel, Format::Ppt, Format::Pdf]
    }
}

/// 格式别名全表（第一个为规范名）。覆盖用户口语：ppt/word/docs/pdf/excel…
pub const ALIASES: &[(&str, Format)] = &[
    ("word", Format::Word),
    ("docx", Format::Word),
    ("doc", Format::Word),
    ("docs", Format::Word),
    ("word文档", Format::Word),
    ("excel", Format::Excel),
    ("xlsx", Format::Excel),
    ("xls", Format::Excel),
    ("sheet", Format::Excel),
    ("sheets", Format::Excel),
    ("表格", Format::Excel),
    ("ppt", Format::Ppt),
    ("pptx", Format::Ppt),
    ("slides", Format::Ppt),
    ("presentation", Format::Ppt),
    ("演示文稿", Format::Ppt),
    ("pdf", Format::Pdf),
    ("json2docx", Format::Word),
    ("json2xlsx", Format::Excel),
    ("json2pptx", Format::Ppt),
    ("json2pdf", Format::Pdf),
];

/// 按别名解析格式
pub fn parse_format(token: &str) -> Option<Format> {
    let t = token.trim().to_ascii_lowercase();
    ALIASES.iter().find(|(a, _)| *a == t).map(|(_, f)| *f)
}

/// 别名展示串（帮助与报错用）：`word|docx|doc|docs`
pub fn alias_hint(f: Format) -> String {
    ALIASES
        .iter()
        .filter(|(_, x)| *x == f)
        // 只展示人类常用的前几个别名，二进制名不重复宣传
        .take(4)
        .map(|(a, _)| *a)
        .collect::<Vec<_>>()
        .join("|")
}

/// 从参数里推断格式：取第一个能判定的参数。
///
/// 判定顺序（同一参数内）：文件扩展名 → 末段名是格式别名
/// （产物目录常以格式命名：`prod/word`、`samples/xlsx`）。
///
/// 例：`["unpack", "a.docx", "-o", "prod"]` → word；
/// `["repack", "prod", "-o", "out.xlsx"]` → excel；
/// `["import", "data.csv", "-o", "out.xlsx"]` → excel（csv 也算 excel 的入口）；
/// `["view", "samples/xlsx", "/", "text"]` → excel（末段名是别名）。
pub fn infer_format(args: &[String]) -> Option<Format> {
    for a in args {
        if a.starts_with('-') {
            continue;
        }
        if let Some(f) = format_from_token(a) {
            return Some(f);
        }
    }
    None
}

/// 单个参数 → 格式（扩展名优先，其次末段名别名）
fn format_from_token(token: &str) -> Option<Format> {
    let ext = Path::new(token)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    if let Some(ext) = ext {
        if let Some(f) = Format::all()
            .into_iter()
            .find(|f| f.extensions().contains(&ext.as_str()))
        {
            return Some(f);
        }
    }
    let last = token
        .trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token);
    parse_format(last)
}

/// 解析格式：显式别名优先，其次参数推断。
pub fn resolve_format(token: Option<&str>, args: &[String]) -> Result<Format, CliError> {
    if let Some(t) = token {
        if let Some(f) = parse_format(t) {
            return Ok(f);
        }
        return Err(CliError::new(format!("未知格式: {t}")).suggest(format!(
            "可用格式：{}（也支持 --format/-f 指定）",
            Format::all()
                .iter()
                .map(|f| alias_hint(*f))
                .collect::<Vec<_>>()
                .join(" · ")
        )));
    }
    infer_format(args).ok_or_else(|| {
        CliError::new("无法从参数推断格式（既没有 .docx/.xlsx/.pptx/.pdf 文件，也没有以格式命名的目录）")
            .suggest(format!(
                "显式指定格式：astorm-office {} <命令...>（或用 --format/-f）；                 也支持 astorm-office formats 先看格式表",
                alias_hint(Format::Word)
            ))
    })
}

/// 底层 CLI 的定位方式（报告里回显，便于诊断）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinSource {
    /// 环境变量显式指定
    Env,
    /// 与 astorm-office 同目录（office-kit 的 bin/ 布局）
    Sibling,
    /// PATH 查找
    Path,
}

impl BinSource {
    pub fn as_str(self) -> &'static str {
        match self {
            BinSource::Env => "env",
            BinSource::Sibling => "sibling",
            BinSource::Path => "path",
        }
    }
}

/// 定位底层 CLI 可执行文件。
///
/// 顺序：`ASTORM_JSON2XXX` 环境变量 → 与自己同目录（kit 的 `bin/`）→ PATH。
/// 找不到时报错并给出三条出路（装 kit / 设环境变量 / 本地构建）。
pub fn resolve_bin(f: Format) -> Result<(PathBuf, BinSource), CliError> {
    if let Some(p) = std::env::var_os(f.env_var()) {
        let path = PathBuf::from(&p);
        if path.is_file() {
            return Ok((path, BinSource::Env));
        }
        return Err(CliError::new(format!(
            "{} 指向的可执行文件不存在: {}",
            f.env_var(),
            path.display()
        ))
        .suggest("去掉该环境变量，或指向真实的 CLI 可执行文件"));
    }

    let exe_name = format!("{}{}", f.bin_name(), std::env::consts::EXE_SUFFIX);
    if let Ok(me) = std::env::current_exe() {
        if let Some(dir) = me.parent() {
            let cand = dir.join(&exe_name);
            if cand.is_file() {
                return Ok((cand, BinSource::Sibling));
            }
        }
    }

    if let Some((path, src)) = search_path(&exe_name) {
        return Ok((path, src));
    }

    Err(
        CliError::new(format!("找不到底层 CLI：{exe_name}（格式 {}）", f.name())).suggest(format!(
            "① 安装官方 office-kit（bin/ 里含四个 CLI 与 astorm-office）；\
         ② 或设 {} 指向可执行文件；\
         ③ 开发时先 `cargo build --release -p {}-cli`",
            f.env_var(),
            match f {
                Format::Word => "json2docx",
                Format::Excel => "json2xlsx",
                Format::Ppt => "json2pptx",
                Format::Pdf => "json2pdf",
            }
        )),
    )
}

/// 在 PATH 里查找可执行文件（避免引入额外依赖）
fn search_path(exe_name: &str) -> Option<(PathBuf, BinSource)> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let cand = dir.join(exe_name);
        if cand.is_file() {
            return Some((cand, BinSource::Path));
        }
    }
    None
}

/// 转发执行：stdio 直通，返回子进程退出码（信号终止按 1 处理）。
pub fn run_passthrough(f: Format, args: &[String]) -> Result<i32, CliError> {
    let (bin, _src) = resolve_bin(f)?;
    let status = Command::new(&bin).args(args).status().map_err(|e| {
        CliError::with_code("io", format!("执行 {} 失败: {e}", bin.display()))
            .suggest("确认可执行文件有运行权限（chmod +x）")
    })?;
    Ok(status.code().unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_cover_the_four_spoken_names() {
        // 用户口语：ppt / word / docs / pdf，外加 excel
        assert_eq!(parse_format("ppt"), Some(Format::Ppt));
        assert_eq!(parse_format("word"), Some(Format::Word));
        assert_eq!(parse_format("docs"), Some(Format::Word));
        assert_eq!(parse_format("pdf"), Some(Format::Pdf));
        assert_eq!(parse_format("excel"), Some(Format::Excel));
        // 大小写与空白不敏感
        assert_eq!(parse_format(" PPTX "), Some(Format::Ppt));
        // 二进制名也可当格式写
        assert_eq!(parse_format("json2xlsx"), Some(Format::Excel));
        assert_eq!(parse_format("doc"), Some(Format::Word));
        assert_eq!(parse_format("nope"), None);
    }

    #[test]
    fn bin_names_and_env_vars() {
        assert_eq!(Format::Word.bin_name(), "json2docx");
        assert_eq!(Format::Excel.bin_name(), "json2xlsx");
        assert_eq!(Format::Ppt.bin_name(), "json2pptx");
        assert_eq!(Format::Pdf.bin_name(), "json2pdf");
        assert_eq!(Format::Pdf.env_var(), "ASTORM_JSON2PDF");
    }

    #[test]
    fn infer_from_extension() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(infer_format(&a(&["unpack", "a.docx"])), Some(Format::Word));
        assert_eq!(infer_format(&a(&["unpack", "a.DOCX"])), Some(Format::Word));
        assert_eq!(infer_format(&a(&["unpack", "a.pptx"])), Some(Format::Ppt));
        assert_eq!(infer_format(&a(&["unpack", "a.pdf"])), Some(Format::Pdf));
        // -o 的输出扩展名也能推断（产物目录 → 二进制）
        assert_eq!(
            infer_format(&a(&["repack", "prod", "-o", "out.xlsx"])),
            Some(Format::Excel)
        );
        // CSV 导入属于 excel 入口
        assert_eq!(
            infer_format(&a(&["import", "data.csv", "-o", "out.xlsx"])),
            Some(Format::Excel)
        );
        // 产物目录以格式命名时也能推断（本仓库 samples/ 就是这个布局）
        assert_eq!(
            infer_format(&a(&["view", "samples/xlsx", "/", "text"])),
            Some(Format::Excel)
        );
        assert_eq!(
            infer_format(&a(&["view", "office-web/demo/samples/docx", "/", "text"])),
            Some(Format::Word)
        );
        assert_eq!(infer_format(&a(&["view", "prod/ppt/"])), Some(Format::Ppt));
        // 选项值不以 - 开头，仍会参与扫描：-p 1,3 不拦路
        assert_eq!(
            infer_format(&a(&["view", "book.xlsx"])),
            Some(Format::Excel)
        );
        assert_eq!(infer_format(&a(&["formats"])), None);
        assert_eq!(infer_format(&a(&["unpack", "prod/"])), None);
    }

    #[test]
    fn resolve_format_prefers_explicit_token() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            resolve_format(Some("ppt"), &a(&["a.docx"])).unwrap(),
            Format::Ppt
        );
        assert!(resolve_format(Some("nope"), &a(&["a.docx"])).is_err());
        assert_eq!(
            resolve_format(None, &a(&["view", "a.pdf"])).unwrap(),
            Format::Pdf
        );
        let err = resolve_format(None, &a(&["formats"])).unwrap_err();
        assert!(
            err.suggestion.unwrap().contains("astorm-office"),
            "{}",
            err.message
        );
    }

    #[test]
    fn alias_hint_lists_spoken_names() {
        let h = alias_hint(Format::Word);
        assert!(h.contains("word") && h.contains("docs"), "{h}");
    }
}
