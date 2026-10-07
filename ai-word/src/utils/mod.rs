//! 通用工具模块：单位换算、XML 辅助、图片加载

pub mod b64;
pub mod constants;
pub mod units;
pub mod xml;

use crate::error::{Error, Result};

/// 加载图片字节：支持本地路径与 http(s) URL
pub fn load_image_bytes(src: &str) -> Result<Vec<u8>> {
    if src.starts_with("http://") || src.starts_with("https://") {
        let resp = ureq::get(src)
            .call()
            .map_err(|e| Error::ImageLoad(format!("下载 {src} 失败: {e}")))?;
        let mut buf = Vec::new();
        std::io::Read::read_to_end(&mut resp.into_reader(), &mut buf)
            .map_err(|e| Error::ImageLoad(format!("读取 {src} 失败: {e}")))?;
        Ok(buf)
    } else {
        std::fs::read(src).map_err(|e| Error::ImageLoad(format!("读取 {src} 失败: {e}")))
    }
}

/// 根据来源路径与字节魔数推断图片扩展名（返回 "png" / "jpeg" / "gif" 等）
pub fn detect_image_extension(src: &str, bytes: &[u8]) -> String {
    // 魔数优先
    if bytes.len() >= 4 {
        if bytes[0] == 0x89 && bytes[1] == b'P' && bytes[2] == b'N' && bytes[3] == b'G' {
            return "png".to_string();
        }
        if bytes[0] == 0xFF && bytes[1] == 0xD8 {
            return "jpeg".to_string();
        }
        if bytes.starts_with(b"GIF8") {
            return "gif".to_string();
        }
        if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            return "webp".to_string();
        }
        // EMF：record type 1 + 偏移 40 处签名 " EMF"
        if bytes[0] == 0x01
            && bytes[1] == 0x00
            && bytes[2] == 0x00
            && bytes[3] == 0x00
            && bytes.len() >= 44
            && &bytes[40..44] == b" EMF"
        {
            return "emf".to_string();
        }
        // WMF：标准(0xD7CD)或 Placeable(0x9AC6)
        if (bytes[0] == 0xD7 && bytes[1] == 0xCD) || (bytes[0] == 0x9A && bytes[1] == 0xC6) {
            return "wmf".to_string();
        }
        // TIFF
        if (bytes[0] == 0x49 && bytes[1] == 0x49 && bytes[2] == 0x2A)
            || (bytes[0] == 0x4D && bytes[1] == 0x4D && bytes[2] == 0x00 && bytes[3] == 0x2A)
        {
            return "tiff".to_string();
        }
        // SVG（XML 文本）
        if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") {
            return "svg".to_string();
        }
    }
    // 回退到扩展名
    let lower = src.to_ascii_lowercase();
    for ext in [
        "png", "jpeg", "jpg", "gif", "bmp", "webp", "emf", "wmf", "svg", "tif", "tiff",
    ] {
        if lower.ends_with(&format!(".{ext}")) {
            return if ext == "jpg" {
                "jpeg".to_string()
            } else {
                ext.to_string()
            };
        }
    }
    "png".to_string()
}

/// 读取图片像素尺寸（宽、高，像素）
pub fn image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    image::load_from_memory(bytes).ok().map(|img| {
        let (w, h) = (img.width(), img.height());
        (w, h)
    })
}
