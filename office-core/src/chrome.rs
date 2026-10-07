//! 无头 Chrome 渲染辅助：把本地 HTML 截图为 PNG / 导出 PDF。
//!
//! 供各 CLI 的 `render` 补齐"看"的闭环（无 Office 依赖，需要本机有
//! Chrome/Chromium；macOS 常见路径会被自动探测）。

use std::path::Path;

fn which(cmd: &str) -> Option<String> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let p = dir.join(cmd);
        if p.is_file() {
            return Some(p.to_string_lossy().into_owned());
        }
    }
    None
}

/// 探测 Chrome/Chromium 可执行文件
///
/// `ASTORM_CHROME_BIN` 环境变量最优先（桌面端可传入自带 Chromium，
/// 如 Electron 的 helper；需指向可执行文件）。
pub fn find_chrome() -> Result<String, String> {
    if let Ok(bin) = std::env::var("ASTORM_CHROME_BIN") {
        if Path::new(&bin).is_file() {
            return Ok(bin);
        }
    }
    which("google-chrome")
        .or_else(|| which("chromium"))
        .or_else(|| which("chromium-browser"))
        .or_else(|| which("chrome"))
        .or_else(|| {
            let mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
            if Path::new(mac).exists() {
                Some(mac.to_string())
            } else {
                None
            }
        })
        .ok_or_else(|| {
            "未找到 Chrome/Chromium；无法通过网页导出（可安装或用 --format html）".to_string()
        })
}

fn chrome_capture(args: &[String], url: &str, expected_out: &Path) -> Result<(), String> {
    let chrome = find_chrome()?;
    // 独立 profile：避免并行调用争抢默认用户目录导致长时间阻塞
    let profile = std::env::temp_dir().join(format!(
        "office_chrome_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    // 先删除旧产物：否则「产物稳定即完成」的判定会把上一次的结果误当成本次输出
    let _ = std::fs::remove_file(expected_out);
    let mut child = std::process::Command::new(chrome)
        .args(args)
        .arg(format!("--user-data-dir={}", profile.display()))
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("执行 Chrome 失败: {e}"))?;
    // 部分 Chrome 版本截图/导出完成后进程不退出（输出已落盘）。
    // 因此轮询产物文件：尺寸非空且连续 3 次稳定即视为完成并主动结束进程。
    let started = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(45);
    let mut last_size = 0u64;
    let mut stable = 0u32;
    let mut produced = false;
    let mut timed_out = false;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                produced = std::fs::metadata(expected_out)
                    .map(|m| m.len() > 0)
                    .unwrap_or(false);
                break;
            }
            Ok(None) => {
                let size = std::fs::metadata(expected_out)
                    .map(|m| m.len())
                    .unwrap_or(0);
                if size > 0 && size == last_size {
                    stable += 1;
                    if stable >= 3 {
                        produced = true;
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                } else {
                    stable = 0;
                }
                last_size = size;
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&profile);
                return Err(format!("等待 Chrome 失败: {e}"));
            }
        }
    }
    let _ = std::fs::remove_dir_all(&profile);
    if produced {
        return Ok(());
    }
    if timed_out {
        Err("Chrome 渲染超时（45s）；可改用 --format html 回退".to_string())
    } else {
        Err("Chrome 渲染失败（未生成输出）".to_string())
    }
}

fn file_url(html: &Path) -> Result<String, String> {
    let abs = html
        .canonicalize()
        .map_err(|e| format!("定位 HTML 失败: {e}"))?;
    Ok(format!("file://{}", abs.display()))
}

/// 用无头 Chrome 把 HTML 截图为 PNG（整页，宽度 `width`，倍率 1.5）
pub fn html_to_png(html: &Path, out: &Path, width: u32) -> Result<(), String> {
    let url = file_url(html)?;
    let args: Vec<String> = vec![
        "--headless".into(),
        "--disable-gpu".into(),
        "--hide-scrollbars".into(),
        "--no-first-run".into(),
        "--no-default-browser-check".into(),
        "--disable-extensions".into(),
        "--disable-background-networking".into(),
        "--virtual-time-budget=2000".into(),
        "--force-device-scale-factor=1.5".into(),
        format!("--window-size={width},4000"),
        format!("--screenshot={}", out.display()),
    ];
    chrome_capture(&args, &url, out)
}

/// 用无头 Chrome 把 HTML 导出为 PDF
pub fn html_to_pdf(html: &Path, out: &Path) -> Result<(), String> {
    let url = file_url(html)?;
    let args: Vec<String> = vec![
        "--headless".into(),
        "--disable-gpu".into(),
        "--no-pdf-header-footer".into(),
        format!("--print-to-pdf={}", out.display()),
    ];
    chrome_capture(&args, &url, out)
}
