//! Image loading and format detection.

use std::io::Read;
use std::path::Path;

use crate::error::{Error, Result};

/// Detect an image extension from magic bytes. Falls back to the file
/// extension, then to "png".
pub fn detect_extension(bytes: &[u8], src: &str) -> String {
    if bytes.len() >= 8 && bytes[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A] {
        return "png".into();
    }
    if bytes.len() >= 3 && bytes[..3] == [0xFF, 0xD8, 0xFF] {
        return "jpg".into();
    }
    if bytes.len() >= 6 && (&bytes[..6] == b"GIF87a" || &bytes[..6] == b"GIF89a") {
        return "gif".into();
    }
    if bytes.len() >= 2 && bytes[..2] == [0x42, 0x4D] {
        return "bmp".into();
    }
    let ext = Path::new(src)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("jpg") | Some("jpeg") => "jpg".into(),
        Some("gif") => "gif".into(),
        Some("bmp") => "bmp".into(),
        _ => "png".into(),
    }
}

/// Load image bytes from a local path or an HTTP(S) URL.
pub fn load_image_bytes(src: &str) -> Result<Vec<u8>> {
    if src.starts_with("http://") || src.starts_with("https://") {
        let resp = ureq::get(src)
            .call()
            .map_err(|e| Error::InvalidInput(format!("无法下载图片 {src}: {e}")))?;
        let mut bytes = Vec::new();
        resp.into_reader()
            .read_to_end(&mut bytes)
            .map_err(|e| Error::InvalidInput(format!("读取远程图片失败 {src}: {e}")))?;
        return Ok(bytes);
    }
    std::fs::read(src).map_err(|e| Error::InvalidInput(format!("无法读取图片 {src}: {e}")))
}
