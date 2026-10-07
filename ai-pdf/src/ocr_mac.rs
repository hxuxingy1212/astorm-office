//! macOS 系统 OCR（Apple Vision）——`OcrEngine` 的本机原生实现。
//!
//! 与 tesseract/HTTP 兜底同构（PNG 字节进 → 行出），但走系统自带 Vision：
//! 无需安装任何东西、中英混排质量显著高于 tesseract。绑定层用 objc2 系统
//! 框架绑定（纯 Rust 生成代码，链接系统框架、零 C 编译单元），仅 macOS
//! 目标编译；声明版本 ≥ macOS 12探测（VNRecognizeTextRequest 支持语言查询）。
//!
//! bbox：Vision `boundingBox` 是归一化坐标（原点在图像左下，y 向上），
//! 此处转成 trait 约定的"像素空间、y 向下自页顶"：
//! x0 = origin.x·W；y_top = (1 - origin.y - height)·H；y_bot = (1 - origin.y)·H。

#![cfg(target_os = "macos")]

use crate::ocr::{OcrEngine, OcrLine};
use anyhow::{anyhow, Result};
use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::AnyObject;
use objc2::AnyThread;
use objc2_foundation::{NSArray, NSData, NSDictionary, NSProcessInfo, NSString};
use objc2_vision::{
    VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
    VNRequestTextRecognitionLevel,
};

pub struct MacOcr;

impl MacOcr {
    /// 探测：macOS ≥ 12 且能查询支持语言（一次轻量调用，不发识别请求）
    pub fn available() -> bool {
        let ver = NSProcessInfo::processInfo().operatingSystemVersion();
        if ver.majorVersion < 12 {
            return false;
        }
        Self::supported_languages().is_some()
    }

    /// Vision 文本识别当前支持的语言（BCP-47 或 ISO 639-1 码）
    pub fn supported_languages() -> Option<Vec<String>> {
        let langs = autoreleasepool(|_| {
            let req = VNRecognizeTextRequest::new();
            // 这是最轻的可用性探针：只查询元数据
            let langs = unsafe { req.supportedRecognitionLanguagesAndReturnError() };
            langs.ok()
        })?;
        Some(langs.iter().map(|s| s.to_string()).collect())
    }

    /// 输入语言码（tesseract 码 / ISO / BCP-47）→ Vision 候选语言列表
    fn candidates(language: &str) -> Vec<String> {
        let mapped = map_language(language);
        let mut out = vec![mapped];
        // 兜底加入英文：Vision 首选语言失败时减少整页空白
        let en = "en-US".to_string();
        if !out.contains(&en) {
            out.push(en);
        }
        out
    }
}

/// tesseract 语言码 / ISO 639-1 / BCP-47 → Vision 语言标识（未识别的当 BCP-47 直传）
fn map_language(code: &str) -> String {
    match code.trim().to_lowercase().as_str() {
        "eng" | "en" | "en-us" => "en-US".to_string(),
        "chi_sim" | "zh" | "zh-cn" | "zho" | "cmn" => "zh-Hans".to_string(),
        "chi_tra" | "zh-tw" | "zh-hant" => "zh-Hant".to_string(),
        "jpn" | "ja" | "ja-jp" => "ja-JP".to_string(),
        "kor" | "ko" | "ko-kr" => "ko-KR".to_string(),
        "fra" | "fre" | "fr" | "fr-fr" => "fr-FR".to_string(),
        "deu" | "de" | "de-de" => "de-DE".to_string(),
        "spa" | "es" | "es-es" => "es-ES".to_string(),
        "ita" | "it" | "it-it" => "it-IT".to_string(),
        "por" | "pt" | "pt-br" => "pt-BR".to_string(),
        "rus" | "ru" | "ru-ru" => "ru-RU".to_string(),
        "ara" | "ar" => "ar-SA".to_string(),
        "tha" | "th" => "th-TH".to_string(),
        "ukr" | "uk" => "uk-UA".to_string(),
        other => other.to_string(),
    }
}

/// PNG 头 IHDR 宽高（渲染管线产出的标准 PNG 可保证）
fn png_dims(png: &[u8]) -> Option<(f64, f64)> {
    if png.len() >= 24 && &png[12..16] == b"IHDR" {
        let w = u32::from_be_bytes(png[16..20].try_into().ok()?) as f64;
        let h = u32::from_be_bytes(png[20..24].try_into().ok()?) as f64;
        if w > 0.0 && h > 0.0 {
            return Some((w, h));
        }
    }
    None
}

impl OcrEngine for MacOcr {
    fn name(&self) -> &'static str {
        "vision"
    }

    fn recognize(&self, png: &[u8], language: &str) -> Result<Vec<OcrLine>> {
        let (w, h) = png_dims(png)
            .ok_or_else(|| anyhow!("PNG 头无法解析，Vision 无法拿到归一化换算基准"))?;
        let supported =
            Self::supported_languages().ok_or_else(|| anyhow!("Vision 支持语言查询失败"))?;
        // 候选 ∩ 支持；全不在支持列表则不设语言（交系统自动判断）
        let cands: Vec<Retained<NSString>> = Self::candidates(language)
            .into_iter()
            .filter(|c| supported.iter().any(|s| s.eq_ignore_ascii_case(c)))
            .map(|c| NSString::from_str(&c))
            .collect();

        autoreleasepool(|_| -> Result<Vec<OcrLine>> {
            let data = NSData::with_bytes(png);
            let handler = VNImageRequestHandler::initWithData_options(
                VNImageRequestHandler::alloc(),
                &data,
                &NSDictionary::<VNImageOption, AnyObject>::new(),
            );
            let req = VNRecognizeTextRequest::new();
            if !cands.is_empty() {
                req.setRecognitionLanguages(&NSArray::from_retained_slice(&cands));
            }
            req.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            req.setUsesLanguageCorrection(false);

            let base: Retained<VNRequest> = req.clone().into_super().into_super();
            handler.performRequests_error(&NSArray::from_retained_slice(std::slice::from_ref(
                &base,
            )))?;

            let mut out = Vec::new();
            if let Some(obs) = req.results() {
                for o in obs.iter() {
                    if let Some(t) = o.topCandidates(1).iter().next() {
                        let text = t.string().to_string();
                        if text.trim().is_empty() {
                            continue;
                        }
                        let bb = unsafe { o.boundingBox() };
                        let x0 = bb.origin.x * w;
                        let y_top = (1.0 - bb.origin.y - bb.size.height) * h;
                        let x1 = (bb.origin.x + bb.size.width) * w;
                        let y_bot = (1.0 - bb.origin.y) * h;
                        let confidence = t.confidence() as f64;
                        if confidence <= 0.1 {
                            continue; // 与 uniOCR/服务路径同阈值
                        }
                        out.push(OcrLine {
                            text,
                            bbox: (x0, y_top, x1, y_bot),
                            confidence,
                        });
                    }
                }
            }
            Ok(out)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_mapping() {
        assert_eq!(map_language("eng"), "en-US");
        assert_eq!(map_language("chi_sim"), "zh-Hans");
        assert_eq!(map_language("zh"), "zh-Hans");
        assert_eq!(map_language("zh-Hant"), "zh-Hant");
        assert_eq!(map_language("ja"), "ja-JP");
        assert_eq!(map_language("kbd"), "kbd"); // 未知码直传
    }

    #[test]
    fn png_header_dims() {
        // 合成 8 字节签名 + len + "IHDR" + 尺寸
        let mut png = vec![0x89u8; 8];
        png.extend_from_slice(&[0, 0, 0, 13]);
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(png_dims(&png), Some((640.0, 480.0)));
        assert_eq!(png_dims(b"not a png"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn vision_probe() {
        assert!(MacOcr::available());
        let supported = MacOcr::supported_languages().unwrap();
        assert!(
            supported.iter().any(|l| l.starts_with("en-")),
            "en-US 应在支持列表：{supported:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn vision_recognize_synthetic_image() {
        // 用 png crate 在无外部资源依赖下合成一张带文字…不行，纯 Rust 画不了字。
        // 改用真实端到端：fixtures 里的扫描图（见 tests/integration.rs 的 pdf 用例）。
        // 这里只验证引擎小路径：空 PNG 报错清晰。
        let e = MacOcr;
        let r = e.recognize(b"", "eng");
        assert!(r.is_err());
    }
}
