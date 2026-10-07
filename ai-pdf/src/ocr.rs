//! OCR 引擎抽象与实现链（按优先级）：
//! 1. **HTTP OCR 服务**（liteparse 规范，显式配置才启用）：`--ocr-server-url` 或
//!    `UNIOCR_URL` 环境变量。POST /ocr（multipart: file + language）→
//!    `{results: [{text, bbox, confidence}]}`。用于远端/高精度服务部署。
//!    （注：crates.io 上的 uni-ocr 是封装本机原生引擎的库，与此无关——
//!    本机的原生 OCR 不走 HTTP，直接见 `ocr_mac`。）
//! 2. **本机原生**：macOS 走系统 Vision（`ocr_mac`，无需安装）；Windows/Linux
//!    暂未接原生引擎。
//! 3. **Tesseract 兜底**（信创/离线环境）：本机 `tesseract` 二进制（免安装
//!    即查 PATH），TSV 输出解析出词级 bbox。
//!
//! 触发策略：仅对 `complexity::needs_ocr` 判定为扫描/无文本/乱码的页送 OCR
//! （selective OCR，借鉴 liteparse）。

use anyhow::{anyhow, Result};
use serde_json::Value;

/// OCR 引擎 trait：输入页面位图（PNG 字节），输出带 bbox 的文本行（pt 坐标、
/// 页面视口空间——y 向下自顶部起，调用方负责换算）。
pub trait OcrEngine {
    /// 引擎名（写入 --json 输出，便于核对实际用了哪级引擎）
    fn name(&self) -> &'static str;
    fn recognize(&self, png: &[u8], language: &str) -> Result<Vec<OcrLine>>;
}

/// OCR 识别出的一行/一词
#[derive(Debug, Clone)]
pub struct OcrLine {
    pub text: String,
    /// 视口空间 bbox（x0, y_top, x1, y_bottom），单位 pt
    pub bbox: (f64, f64, f64, f64),
    pub confidence: f64,
}

impl OcrEngine for Box<dyn OcrEngine> {
    fn name(&self) -> &'static str {
        self.as_ref().name()
    }
    fn recognize(&self, png: &[u8], language: &str) -> Result<Vec<OcrLine>> {
        self.as_ref().recognize(png, language)
    }
}

// ── HTTP OCR 服务（liteparse 规范客户端）─────────────────────────────────

pub struct HttpOcr {
    pub base_url: String,
}

impl HttpOcr {
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("UNIOCR_URL").ok()?;
        Some(Self {
            base_url: url.trim_end_matches('/').to_string(),
        })
    }

    pub fn from_url(url: &str) -> Self {
        Self {
            base_url: url.trim_end_matches('/').to_string(),
        }
    }
}

impl OcrEngine for HttpOcr {
    fn name(&self) -> &'static str {
        "http"
    }
    fn recognize(&self, png: &[u8], language: &str) -> Result<Vec<OcrLine>> {
        let boundary = format!("json2pdf-{}", std::process::id());
        let mut body: Vec<u8> = Vec::new();
        let file_part = format!(
            "--{b}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"page.png\"\r\nContent-Type: image/png\r\n\r\n",
            b = boundary
        );
        body.extend_from_slice(file_part.as_bytes());
        body.extend_from_slice(png);
        body.extend_from_slice(
            format!("\r\n--{b}\r\nContent-Disposition: form-data; name=\"language\"\r\n\r\n{language}\r\n--{b}--\r\n", b = boundary).as_bytes(),
        );

        let url = if self.base_url.ends_with("/ocr") {
            self.base_url.clone()
        } else {
            format!("{}/ocr", self.base_url)
        };
        let resp = http_post(&url, &body, &boundary)?;
        // 期望 {"results":[{"text":..., "bbox":[x1,y1,x2,y2], "confidence":0.9}, ...]}
        let v: Value = serde_json::from_str(&resp).map_err(|e| {
            anyhow!(
                "uniOCR 响应不是 JSON: {e}（前 200 字: {}）",
                &resp[..resp.len().min(200)]
            )
        })?;
        let mut out = Vec::new();
        if let Some(results) = v.get("results").and_then(|r| r.as_array()) {
            for r in results {
                let text = r
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if text.is_empty() {
                    continue;
                }
                let bbox = r
                    .get("bbox")
                    .and_then(|b| b.as_array())
                    .and_then(|a| {
                        let n: Vec<f64> = a.iter().filter_map(|x| x.as_f64()).collect();
                        if n.len() >= 4 {
                            Some((n[0], n[1], n[2], n[3]))
                        } else {
                            None
                        }
                    })
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                let confidence = r.get("confidence").and_then(|c| c.as_f64()).unwrap_or(0.9);
                if confidence <= 0.1 {
                    continue; // 低置信度丢弃（liteparse 同阈值）
                }
                out.push(OcrLine {
                    text,
                    bbox,
                    confidence,
                });
            }
        }
        Ok(out)
    }
}

/// multipart POST（无 openssl 依赖：ureq + 手拼 multipart）
fn http_post(url: &str, body: &[u8], boundary: &str) -> Result<String> {
    let resp = ureq::post(url)
        .set(
            "Content-Type",
            &format!("multipart/form-data; boundary={boundary}"),
        )
        .timeout(std::time::Duration::from_secs(120))
        .send(body)?;
    let text = resp.into_string()?;
    Ok(text)
}

// ── Tesseract 兜底（信创环境免安装即可用）─────────────────────────────────

pub struct TesseractOcr {
    pub tessdata_path: Option<String>,
}

impl TesseractOcr {
    pub fn available() -> bool {
        which_tesseract().is_some()
    }
}

fn which_tesseract() -> Option<String> {
    let ok = std::process::Command::new("tesseract")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    ok.then(|| "tesseract".to_string())
}

impl OcrEngine for TesseractOcr {
    fn name(&self) -> &'static str {
        "tesseract"
    }
    fn recognize(&self, png: &[u8], language: &str) -> Result<Vec<OcrLine>> {
        let tesseract = which_tesseract()
            .ok_or_else(|| anyhow!("tesseract 不可用（信创兜底未安装）；安装 tesseract-ocr 或设置 UNIOCR_URL 用平台 OCR"))?;
        let dir = std::env::temp_dir().join(format!("json2pdf-ocr-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let img = dir.join("page.png");
        std::fs::write(&img, png)?;
        let mut cmd = std::process::Command::new(&tesseract);
        cmd.arg(&img).arg("stdout").arg("-l").arg(language);
        cmd.arg("tsv"); // TSV：词级 bbox
        if let Some(tp) = &self.tessdata_path {
            cmd.arg(format!("--tessdata-dir={tp}"));
        }
        let out = cmd.output()?;
        let _ = std::fs::remove_dir_all(&dir);
        if !out.status.success() {
            return Err(anyhow!(
                "tesseract 失败: {}",
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            ));
        }
        parse_tsv(&String::from_utf8_lossy(&out.stdout))
    }
}

/// 解析 `tesseract ... tsv` 输出：level=5 的行是词
fn parse_tsv(tsv: &str) -> Result<Vec<OcrLine>> {
    let mut out = Vec::new();
    for line in tsv.lines().skip(1) {
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() < 12 {
            continue;
        }
        // level conf left top width height ... text
        let level: i32 = cols[0].parse().unwrap_or(0);
        if level != 5 {
            continue; // 5 = word
        }
        let conf: f64 = cols[10].parse().unwrap_or(-1.0);
        let text = cols[11].trim().to_string();
        if text.is_empty() || conf < 10.0 {
            continue;
        }
        let left: f64 = cols[6].parse().unwrap_or(0.0);
        let top: f64 = cols[7].parse().unwrap_or(0.0);
        let w: f64 = cols[8].parse().unwrap_or(0.0);
        let h: f64 = cols[9].parse().unwrap_or(0.0);
        out.push(OcrLine {
            text,
            bbox: (left, top, left + w, top + h),
            confidence: (conf / 100.0).clamp(0.0, 1.0),
        });
    }
    Ok(out)
}

// ── 引擎选择与调用 ────────────────────────────────────────────────────────

/// 按优先级构造 OCR 引擎：显式 HTTP 服务（--ocr-server-url / UNIOCR_URL）→
/// 本机原生（macOS Vision）→ tesseract 兜底（任何引擎选择永不失败，
/// 不可用时留到逐页报错并给安装建议）。
pub fn engine(server_url: Option<&str>, tessdata: Option<&str>) -> Result<Box<dyn OcrEngine>> {
    if let Some(u) = server_url {
        return Ok(Box::new(HttpOcr::from_url(u)));
    }
    if let Some(e) = HttpOcr::from_env() {
        return Ok(Box::new(e));
    }
    #[cfg(target_os = "macos")]
    if crate::ocr_mac::MacOcr::available() {
        return Ok(Box::new(crate::ocr_mac::MacOcr));
    }
    Ok(Box::new(TesseractOcr {
        tessdata_path: tessdata.map(String::from),
    }))
}

/// 把 OCR 行合并进 PageText（bbox → PDF 用户空间 y 向上：y_base = page_h - y_top）。
/// 输出行作为独立 Line 追加（OCR 文本与 native 文本来源不同，不参与行合并）。
pub fn merge_into_lines(
    ocr: &[OcrLine],
    page_height: f64,
    dpi: f64,
    lines: &mut Vec<crate::text::Line>,
) -> usize {
    // 像素 → pt：tesseract/uniOCR 的 bbox 是位图像素（按 dpi 渲染）
    let scale = 72.0 / dpi;
    let mut n = 0;
    for l in ocr {
        let (x0, y_top, x1, _) = l.bbox;
        if x1 - x0 <= 0.0 {
            continue;
        }
        let x = x0 * scale;
        let y_base = page_height - y_top * scale;
        let size = ((l.bbox.3 - l.bbox.1) * scale).clamp(6.0, 48.0);
        lines.push(crate::text::Line {
            y: y_base,
            items: vec![crate::text::TextItem {
                x,
                end_x: x1 * scale,
                size,
                text: l.text.clone(),
            }],
        });
        n += 1;
    }
    n
}

