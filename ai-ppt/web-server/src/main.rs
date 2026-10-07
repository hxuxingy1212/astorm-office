//! json2pptx web 后端
//!
//! 提供 HTTP 接口：上传 PPTX → unpack 产物 → 前端预览/编辑 → 写回产物 → repack 下载。
//! 会话为单用户本地模式（每次上传替换当前会话），数据存放在 `data/<session_id>/`。

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path as AxumPath, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};

/// 当前打开的演示会话
#[derive(Clone)]
struct Session {
    name: String,
    root: PathBuf,
}

type SharedState = Arc<Mutex<Option<Session>>>;

/// API 错误
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad(msg: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    fn internal(msg: impl Into<String>) -> Self {
        ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::internal(format!("IO 错误: {e}"))
    }
}

impl From<json2pptx::Error> for ApiError {
    fn from(e: json2pptx::Error) -> Self {
        ApiError::internal(format!("json2pptx 错误: {e}"))
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(e: serde_json::Error) -> Self {
        ApiError::internal(format!("JSON 错误: {e}"))
    }
}

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// 读取产物根下的 JSON 文件并解析为 serde_json::Value
fn read_json(root: &Path, rel: &str) -> Result<serde_json::Value, ApiError> {
    let text = std::fs::read_to_string(root.join(rel))?;
    serde_json::from_str(&text).map_err(|e| ApiError::internal(format!("JSON 解析失败: {e}")))
}

/// 构造 overview：演示元数据 + 每页幻灯片 JSON + 媒体清单
fn build_overview(session: &Session) -> Result<serde_json::Value, ApiError> {
    let root = &session.root;
    let pres: serde_json::Value = read_json(root, "presentation.json")?;

    let slide_paths: Vec<String> = pres["slides"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut slides = Vec::new();
    for (i, rel) in slide_paths.iter().enumerate() {
        slides.push(serde_json::json!({
            "index": i + 1,
            "data": read_json(root, rel)?,
        }));
    }

    // 媒体清单
    let mut media: Vec<String> = Vec::new();
    let media_dir = root.join("ppt").join("media");
    if let Ok(entries) = std::fs::read_dir(&media_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str().map(String::from) {
                media.push(format!("ppt/media/{name}"));
            }
        }
    }
    media.sort();

    Ok(serde_json::json!({
        "name": session.name,
        "width": pres["width"],
        "height": pres["height"],
        "meta": pres["meta"],
        "theme": pres["theme"],
        "slides": slides,
        "media": media,
    }))
}

/// 上传 PPTX：保存 → unpack → 建立会话 → 返回 overview
async fn upload(
    State(state): State<SharedState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut file_name = String::from("input.pptx");
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad(format!("multipart 读取失败: {e}")))?
    {
        if field.name() == Some("file") {
            file_name = field
                .file_name()
                .map(String::from)
                .unwrap_or_else(|| file_name.clone());
            file_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::bad(format!("文件读取失败: {e}")))?
                    .to_vec(),
            );
        }
    }
    let bytes = file_bytes.ok_or_else(|| ApiError::bad("缺少 file 字段"))?;

    // 新会话目录
    let session_id = format!(
        "{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let dir = data_dir().join(&session_id);
    std::fs::create_dir_all(&dir)?;
    let input_path = dir.join("input.pptx");
    std::fs::write(&input_path, &bytes)?;

    // unpack 到 out/
    let out_dir = dir.join("out");
    let result = json2pptx::unpack(input_path.to_str().unwrap(), out_dir.to_str().unwrap())?;
    if result.slides == 0 {
        return Err(ApiError::bad("PPTX 中没有幻灯片"));
    }

    let session = Session {
        name: file_name,
        root: out_dir,
    };
    let overview = build_overview(&session)?;
    *state.lock().unwrap() = Some(session);
    Ok(Json(overview))
}

/// 当前会话 overview（含全部幻灯片 JSON）
async fn overview(State(state): State<SharedState>) -> Result<Json<serde_json::Value>, ApiError> {
    let guard = state.lock().unwrap();
    let session = guard
        .as_ref()
        .ok_or_else(|| ApiError::bad("尚未上传 PPTX"))?;
    Ok(Json(build_overview(session)?))
}

/// 媒体文件（产物内相对路径，如 ppt/media/image1.png）
async fn media(
    State(state): State<SharedState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, ApiError> {
    let guard = state.lock().unwrap();
    let session = guard
        .as_ref()
        .ok_or_else(|| ApiError::bad("尚未上传 PPTX"))?;
    let clean = path.replace("..", "");
    let file_path = session.root.join(&clean);
    let bytes = std::fs::read(&file_path)?;
    let content_type = match file_path.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "application/octet-stream",
    };
    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, content_type)],
        bytes,
    )
        .into_response())
}

/// 上传媒体（图片等）到当前会话的媒体目录，返回产物内相对路径（如 ppt/media/imageN.png）
async fn upload_media(
    State(state): State<SharedState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut name = String::from("image.png");
    let mut bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad(format!("multipart 读取失败: {e}")))?
    {
        if field.name() == Some("file") {
            if let Some(fname) = field.file_name().map(String::from) {
                name = fname;
            }
            bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::bad(format!("文件读取失败: {e}")))?
                    .to_vec(),
            );
        }
    }
    let file_bytes = bytes.ok_or_else(|| ApiError::bad("缺少 file 字段"))?;

    let guard = state.lock().unwrap();
    let session = guard
        .as_ref()
        .ok_or_else(|| ApiError::bad("尚未上传 PPTX"))?;

    // 生成唯一文件名（带原扩展名）
    let ext = std::path::Path::new(&name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .filter(|e| {
            matches!(
                e.as_str(),
                "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp"
            )
        })
        .unwrap_or_else(|| "png".to_string());

    let media_dir = session.root.join("ppt").join("media");
    std::fs::create_dir_all(&media_dir)?;

    // 取下一个空闲序号避免覆盖
    let mut idx = 1;
    let rel_path = loop {
        let candidate = format!("ppt/media/upload{}.{}", idx, ext);
        if !media_dir.join(format!("upload{}.{}", idx, ext)).exists() {
            break candidate;
        }
        idx += 1;
    };

    std::fs::write(session.root.join(&rel_path), &file_bytes)?;
    Ok(Json(json!({ "success": true, "src": rel_path })))
}

/// 写回单张幻灯片 JSON（body 为 slide JSON）
async fn save_slide(
    State(state): State<SharedState>,
    AxumPath(idx): AxumPath<usize>,
    body: String,
) -> Result<Json<serde_json::Value>, ApiError> {
    let guard = state.lock().unwrap();
    let session = guard
        .as_ref()
        .ok_or_else(|| ApiError::bad("尚未上传 PPTX"))?;
    let slide_json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| ApiError::bad(format!("slide JSON 解析失败: {e}")))?;
    let path = session.root.join(format!("ppt/slides/slide{idx}.json"));
    // 规范化写回（保留前端修改的全部内容）
    std::fs::write(&path, serde_json::to_string_pretty(&slide_json)?)?;
    Ok(Json(
        json!({ "success": true, "slide": idx, "saved": true }),
    ))
}

/// repack 产物为 PPTX 并返回文件流
async fn repack(State(state): State<SharedState>) -> Result<Response, ApiError> {
    let guard = state.lock().unwrap();
    let session = guard
        .as_ref()
        .ok_or_else(|| ApiError::bad("尚未上传 PPTX"))?;
    let out_pptx = session.root.join("repacked.pptx");
    json2pptx::repack(session.root.to_str().unwrap(), out_pptx.to_str().unwrap())?;
    let bytes = std::fs::read(&out_pptx)?;
    Ok((
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                    .to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", session.name),
            ),
        ],
        bytes,
    )
        .into_response())
}

#[tokio::main]
async fn main() {
    let state: SharedState = Arc::new(Mutex::new(None));
    std::fs::create_dir_all(data_dir()).expect("创建 data 目录失败");

    // 前端构建产物（web/dist），路径基于 crate 目录（与运行时 cwd 无关）
    let dist = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../web/dist")
        .canonicalize()
        .unwrap_or_else(|_| Path::new("../web/dist").to_path_buf());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/upload", post(upload))
        .route("/api/overview", get(overview))
        .route("/api/media", post(upload_media))
        .route("/api/media/*path", get(media))
        .route("/api/slide/:idx", put(save_slide))
        .route("/api/repack", post(repack))
        // release 模式服务前端构建产物（web/dist）
        .nest_service(
            "/",
            tower_http::services::ServeDir::new(&dist).fallback(
                tower_http::services::ServeFile::new(dist.join("index.html")),
            ),
        )
        // 跨域（web 直连后端时需要）
        .layer(cors)
        // PPTX 可能含大量媒体，放宽上传 body 上限（默认 2MB 太小）
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        // 请求日志（便于排查）
        .layer(axum::middleware::from_fn(log_requests))
        .with_state(state);

    let addr = "127.0.0.1:8080";
    println!("json2pptx web 服务已启动: http://{addr}  (上传 PPTX 即可预览编辑)");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("端口 8080 绑定失败");
    axum::serve(listener, app).await.expect("服务异常退出");
}

/// 请求日志中间件：打印方法、路径与状态码
async fn log_requests(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let resp = next.run(req).await;
    println!("{method} {path} -> {}", resp.status());
    resp
}
