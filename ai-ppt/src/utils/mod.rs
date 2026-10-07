//! 工具模块
//!
//! 提供图片加载、格式检测等通用功能。

pub mod constants;
pub mod xml;

use std::path::Path;

/// 加载图片字节数据
///
/// 支持本地文件路径和 HTTP(S) URL 两种方式。
pub fn load_image_bytes(src: &str) -> Option<Vec<u8>> {
    if src.starts_with("http://") || src.starts_with("https://") {
        match ureq::get(src).call() {
            Ok(response) => {
                let mut bytes: Vec<u8> = Vec::new();
                if std::io::Read::read_to_end(&mut response.into_reader(), &mut bytes).is_ok() {
                    Some(bytes)
                } else {
                    None
                }
            }
            Err(_) => None,
        }
    } else {
        std::fs::read(src).ok()
    }
}

/// 通过文件扩展名或文件头魔术字节检测图片格式
pub fn detect_image_extension(src: &str, bytes: &[u8]) -> String {
    // 优先使用文件扩展名
    if let Some(ext) = Path::new(src)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
    {
        if matches!(
            ext.as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "bmp"
        ) {
            return ext;
        }
    }
    // 通过魔术字节检测
    if bytes.len() >= 8 {
        if bytes[0..3] == [0x89, 0x50, 0x4E] {
            return "png".to_string();
        }
        if bytes[0..2] == [0xFF, 0xD8] {
            return "jpg".to_string();
        }
        if bytes[0..4] == [0x47, 0x49, 0x46] {
            return "gif".to_string();
        }
        if bytes[0..4] == [0x52, 0x49, 0x46, 0x46] {
            return "webp".to_string();
        }
    }
    "png".to_string()
}
