//! 格式转换 — 复刻 pdf.py convert.office / convert.html / convert.latex 与 env.check：
//! 外部二进制（soffice / chromium / tectonic）探测 + 带超时执行。

use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// PATH + 常见安装位探测。
pub fn find_binary(name: &str, extra: &[&str]) -> Option<PathBuf> {
    for candidate in extra {
        let p = PathBuf::from(candidate);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let p = dir.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn find_soffice() -> Option<PathBuf> {
    find_binary(
        "soffice",
        &[
            "/Applications/LibreOffice.app/Contents/MacOS/soffice",
            "/usr/bin/soffice",
            "/usr/local/bin/soffice",
            "/usr/lib/libreoffice/program/soffice",
            "/opt/libreoffice/program/soffice",
            "/snap/bin/libreoffice",
        ],
    )
}

pub fn find_chrome() -> Option<PathBuf> {
    find_binary(
        "chromium",
        &[
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium-browser",
            "/usr/local/bin/chrome",
        ],
    )
    .or_else(|| find_binary("google-chrome", &[]))
    .or_else(|| find_binary("chrome", &[]))
}

pub fn find_tectonic() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let home_bin = format!("{home}/tectonic");
    find_binary("tectonic", &[&home_bin])
}

/// 带超时执行（soffice 挂起是常见故障，原版用 120s）。
/// 带超时执行外部命令，返回（是否成功，合并日志）。CLI 层渲染复用。
pub fn run_with_timeout(bin: &Path, args: &[&str], timeout: Duration) -> Result<(bool, String)> {
    let mut child = Command::new(bin)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn {} failed: {e}", bin.display()))?;
    let start = Instant::now();
    loop {
        match child.try_wait()? {
            Some(status) => {
                let out = child.wait_with_output()?;
                let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
                combined.push_str(&String::from_utf8_lossy(&out.stderr));
                return Ok((status.success(), combined));
            }
            None => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    bail!(
                        "command timed out after {}s: {} {:?}",
                        timeout.as_secs(),
                        bin.display(),
                        args
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

const SOFFICE_MISSING: &str = "LibreOffice (soffice) not found. It is a hard, non-substitutable \
requirement for Office→PDF conversion. Install it (macOS: brew install --cask libreoffice; \
Debian/Ubuntu: sudo apt install libreoffice-core; or from the mirror), then make sure `soffice` \
is on PATH.";

const OFFICE_EXTS: &[&str] = &[
    "docx", "doc", "odt", "rtf", "pptx", "ppt", "odp", "xlsx", "xls", "ods", "csv", "txt", "html",
    "htm",
];

pub fn convert_office(file: &Path, out: Option<&Path>, engine: &str) -> Result<Value> {
    let ext = file
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if !OFFICE_EXTS.contains(&ext.as_str()) {
        bail!("unsupported input type .{ext} (supported: {OFFICE_EXTS:?})");
    }
    // 引擎链：显式 native 跳过探测；auto/soffice 按本机探测决定
    let soffice = if engine == "native" {
        None
    } else {
        find_soffice()
    };
    if let Some(soffice) = soffice {
        let abs = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
        let outdir = abs.parent().unwrap_or(Path::new(".")).to_path_buf();
        let expected = outdir.join(format!(
            "{}.pdf",
            file.file_stem().unwrap_or_default().to_string_lossy()
        ));
        let args = [
            "--headless".to_string(),
            "--norestore".to_string(),
            "--convert-to".to_string(),
            "pdf".to_string(),
            "--outdir".to_string(),
            outdir.display().to_string(),
            abs.display().to_string(),
        ];
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (ok, log) = run_with_timeout(&soffice, &arg_refs, Duration::from_secs(120))?;
        if !ok || !expected.exists() {
            bail!("soffice conversion failed. log: {}", log.trim());
        }
        let final_path = match out {
            Some(o) => {
                std::fs::rename(&expected, o)?;
                o.to_path_buf()
            }
            None => expected,
        };
        Ok(json!({
            "input": file.display().to_string(),
            "output": final_path.display().to_string(),
            "engine": "libreoffice",
        }))
    } else {
        if engine == "soffice" {
            // 显式指定 soffice 而本机没有 → 如实报错（不静默回落）
            let _ = anyhow::anyhow!("{SOFFICE_MISSING}");
            bail!("{SOFFICE_MISSING}");
        }
        // native 自研渲染兜底（LibreOffice 缺席时）
        let out_path = out
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| file.with_extension("pdf"));
        let pages = crate::cross::office_to_pdf(file, &out_path)?;
        Ok(json!({
            "input": file.display().to_string(),
            "output": out_path.display().to_string(),
            "engine": "native",
            "pages": pages,
        }))
    }
}

pub fn convert_html(file: &Path, out: Option<&Path>, css: Option<&Path>) -> Result<Value> {
    let chrome = find_chrome().ok_or_else(|| {
        anyhow::anyhow!("chromium/chrome not found. Install Chromium (`npx playwright install chromium` puts it under the playwright cache) or Google Chrome, then retry.")
    })?;
    let out_path = out
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| file.with_extension("pdf"));

    // css 注入：追加 <style> 到 head（原版 html2pdf-next.js 在渲染前注入）
    let html = std::fs::read_to_string(file)?;
    let html = if let Some(css_path) = css {
        let css_text = std::fs::read_to_string(css_path)?;
        let style = format!("<style>{css_text}</style>");
        if let Some(pos) = html.rfind("</head>") {
            format!("{}{style}{}", &html[..pos], &html[pos..])
        } else {
            format!("{style}{html}")
        }
    } else {
        html
    };
    let tmp_html = file.with_extension("json2pdf-tmp.html");
    std::fs::write(&tmp_html, html)?;
    let abs = tmp_html.canonicalize().unwrap_or(tmp_html.clone());

    let print_target = out_path.display().to_string();
    let args = [
        "--headless".to_string(),
        "--disable-gpu".to_string(),
        "--no-sandbox".to_string(),
        "--no-pdf-header-footer".to_string(),
        format!("--print-to-pdf={print_target}"),
        format!("file://{}", abs.display()),
    ];
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (ok, log) = run_with_timeout(&chrome, &arg_refs, Duration::from_secs(120))?;
    let _ = std::fs::remove_file(&tmp_html);
    if !ok || !out_path.exists() {
        bail!("chromium print-to-pdf failed. log: {}", log.trim());
    }
    Ok(json!({
        "input": file.display().to_string(),
        "output": out_path.display().to_string(),
        "engine": "chromium",
        "css_injected": css.is_some(),
    }))
}

pub fn convert_latex(tex: &Path, runs: u32, _keep_logs: bool) -> Result<Value> {
    let tectonic = find_tectonic().ok_or_else(|| {
        anyhow::anyhow!("tectonic not found. Install it (curl -fsSL https://drop-sh.fullyjustified.net | sh) and ensure `tectonic` is on PATH.")
    })?;
    let abs = tex.canonicalize().unwrap_or_else(|_| tex.to_path_buf());
    let mut last_log = String::new();
    let mut ok = false;
    for _ in 0..runs.max(1) {
        let args = [
            "-X".to_string(),
            "compile".to_string(),
            abs.display().to_string(),
        ];
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (run_ok, log) = run_with_timeout(&tectonic, &arg_refs, Duration::from_secs(120))?;
        ok = run_ok;
        last_log = log;
        if !run_ok {
            break;
        }
    }
    let pdf = tex.with_extension("pdf");
    if !ok || !pdf.exists() {
        bail!("tectonic compile failed. log: {}", last_log.trim());
    }
    // 日志分类（Overfull/Underfull → layout 提示）
    let layout: Vec<&str> = last_log
        .lines()
        .filter(|l| l.contains("Overfull") || l.contains("Underfull"))
        .take(10)
        .collect();
    Ok(json!({
        "input": tex.display().to_string(),
        "output": pdf.display().to_string(),
        "engine": "tectonic",
        "runs": runs.max(1),
        "layout_warnings": layout,
    }))
}

/// env.check — 探测外部依赖（原版还探测 Python 包，Rust 版只需二进制）。
pub fn env_check() -> Value {
    let entry = |p: Option<PathBuf>| match p {
        Some(path) => json!({"found": true, "path": path.display().to_string()}),
        None => json!({"found": false}),
    };
    json!({
        "soffice": entry(find_soffice()),
        "chrome": entry(find_chrome()),
        "tectonic": entry(find_tectonic()),
        "core": { "status": "ok", "engine": "lopdf" },
    })
}
