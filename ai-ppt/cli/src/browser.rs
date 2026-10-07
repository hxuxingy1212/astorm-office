//! 浏览器渲染：用无头 Chromium/Edge 截屏，复用 web 前端渲染，输出与网页一致。
//!
//! 流程：启动一个轻量 HTTP 服务（提供 web 产物 + /api/overview + /api/media），
//! 传入 `?render=1&slide=N` 让前端只渲染该页，再用无头浏览器截图。

use std::path::{Path, PathBuf};
use std::process::Command;

const INDEX_HTML: &str = "index.html";

/// 浏览器可执行文件候选
fn find_browser() -> Option<PathBuf> {
    let cands = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    ];
    for c in cands {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn dist_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../web/dist")
        .canonicalize()
        .unwrap_or_else(|_| Path::new("../web/dist").to_path_buf())
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// 启动 HTTP 服务并截屏
pub fn render_slide_browser(
    product_root: &Path,
    slide_no: usize, // 1-based
    width_in: f64,
    height_in: f64,
    scale: f64,
    out_path: &Path,
) -> Result<(), String> {
    let dist = dist_dir();
    if !dist.join(INDEX_HTML).exists() {
        return Err(format!(
            "未找到前端构建产物: {}（请先构建 web，如 `npm run build`）",
            dist.display()
        ));
    }
    let browser = find_browser().ok_or_else(|| {
        "未找到 Chrome/Edge 浏览器。请安装 Chrome 或 Edge，或改用 `--engine native`。".to_string()
    })?;

    // 临时用户数据目录，避免与正在运行的浏览器实例冲突
    let profile = std::env::temp_dir().join(format!("json2pptx-render-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&profile);

    // 单线程 HTTP 服务；用线程处理每个请求，避免阻塞
    let server =
        tiny_http::Server::http("127.0.0.1:0").map_err(|e| format!("启动本地服务失败: {e}"))?;
    let port = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| "无法获取本地服务端口".to_string())?
        .port();

    let root = product_root.to_path_buf();
    let dist2 = dist.clone();
    let serve_thread = std::thread::spawn(move || {
        for request in server.incoming_requests() {
            let root = root.clone();
            let dist = dist2.clone();
            std::thread::spawn(move || {
                handle_request(request, &root, &dist);
            });
        }
    });

    let pw = (width_in * 96.0 * scale).round() as u32;
    let ph = (height_in * 96.0 * scale).round() as u32;
    let url = format!("http://127.0.0.1:{port}/?render=1&slide={slide_no}");

    let args: Vec<String> = vec![
        "--headless=new".to_string(),
        "--disable-gpu".to_string(),
        "--hide-scrollbars".to_string(),
        "--force-device-scale-factor=1".to_string(),
        format!("--window-size={pw},{ph}"),
        "--virtual-time-budget=8000".to_string(),
        "--no-first-run".to_string(),
        "--no-default-browser-check".to_string(),
        "--disable-extensions".to_string(),
        format!("--user-data-dir={}", profile.to_string_lossy()),
        format!("--screenshot={}", out_path.to_string_lossy()),
        url.clone(),
    ];
    let status = Command::new(&browser)
        .args(&args)
        .status()
        .map_err(|e| format!("启动浏览器失败: {e}"))?;

    let _ = serve_thread;
    drop(serve_thread);
    if !status.success() {
        return Err(format!("浏览器截图失败（退出码 {:?}）", status.code()));
    }
    if !out_path.exists() {
        return Err("截图未生成".to_string());
    }
    Ok(())
}

/// 处理单个 HTTP 请求
fn handle_request(request: tiny_http::Request, product_root: &Path, dist: &Path) {
    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or("/").to_string();

    if path == "/api/overview" {
        match crate::product::overview_json(product_root) {
            Ok(json) => {
                respond_json(request, 200, &json);
            }
            Err(e) => {
                respond_text(
                    request,
                    500,
                    &format!("{{\"error\":\"{}\"}}", e.message.replace('"', "'")),
                    "application/json; charset=utf-8",
                );
            }
        }
        return;
    }

    if let Some(rel) = path.strip_prefix("/api/media/") {
        let rel = rel.replace("../", "");
        let file = product_root.join(&rel);
        match std::fs::read(&file) {
            Ok(bytes) => {
                let ct = content_type(&file);
                let resp =
                    tiny_http::Response::from_data(bytes).with_header(hm("Content-Type", ct));
                let _ = request.respond(resp);
            }
            Err(_) => {
                let _ = request
                    .respond(tiny_http::Response::from_string("not found").with_status_code(404));
            }
        }
        return;
    }

    // 静态文件
    let rel = path.trim_start_matches('/');
    let file = dist.join(if rel.is_empty() { INDEX_HTML } else { rel });
    let file = if file.exists() && !file.is_dir() {
        file
    } else {
        dist.join(INDEX_HTML)
    };
    match std::fs::read(&file) {
        Ok(bytes) => {
            let ct = content_type(&file);
            let resp = tiny_http::Response::from_data(bytes).with_header(hm("Content-Type", ct));
            let _ = request.respond(resp);
        }
        Err(_) => {
            let _ = request
                .respond(tiny_http::Response::from_string("not found").with_status_code(404));
        }
    }
}

fn hm(name: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(name.as_bytes().to_vec(), value.as_bytes().to_vec())
        .expect("合法 header")
}

fn respond_json(request: tiny_http::Request, status: u16, json: &serde_json::Value) {
    respond_text(
        request,
        status,
        &json.to_string(),
        "application/json; charset=utf-8",
    );
}

fn respond_text(request: tiny_http::Request, status: u16, body: &str, ct: &str) {
    let resp = tiny_http::Response::from_string(body.to_string())
        .with_status_code(status)
        .with_header(hm("Content-Type", ct));
    let _ = request.respond(resp);
}
