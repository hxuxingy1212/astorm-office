//! 不可直接预览的图片滤镜（JPX / JBIG2 / CCITT）的像素解码。
//!
//! 字节保真链路（unpack → repack）不经过这里——它始终原样回写原码流；
//! 本模块只服务于"看得见"：unpack 时解码出 RGB 像素生成预览 PNG
//! （`Element::Image` 的 `preview` 字段），供 `render` 与 web 预览使用。
//!
//! 解码器选型（全部纯 Rust、静态链接、无 C 依赖，对包体积影响 ~1MB）：
//! - CCITT：`hayro-ccitt`（Apache-2.0 OR MIT）——G3 1D/2D 与 G4
//! - JBIG2：`hayro-jbig2`（Apache-2.0 OR MIT）——嵌入式流 + Globals
//! - JPX：`openjpeg2-pure-rs`（BSD-2-Clause，OpenJPEG 的纯 Rust 翻译）

use serde_json::Value;

/// 解码出的像素（RGB8，行主序）
pub struct PreviewPixels {
    pub width: u32,
    pub height: u32,
    /// 长度 = width * height * 3
    pub rgb: Vec<u8>,
}

/// 按首个终止型滤镜解码为像素；失败返回 None（调用方回退占位呈现）。
///
/// - `filter`/`parms`：外层可解滤镜已剥离后的终止滤镜链与参数
/// - `globals`：JBIG2 的 JBIG2Globals 码流（共享符号/模板上下文）
/// - `dict_width/height`：图片字典尺寸（CCITT Rows 缺省 / 兜底用）
/// - `decode`：/Decode 数组（`[1 0]` 表示位反转；CMYK 的 Adobe 反相）
pub fn decode_preview(
    filter: &[String],
    parms: &[Value],
    data: &[u8],
    globals: Option<&[u8]>,
    dict_width: i64,
    dict_height: i64,
    decode: Option<&[f64]>,
) -> Option<PreviewPixels> {
    let f = filter.first()?.to_lowercase();
    match f.as_str() {
        "ccittfaxdecode" => decode_ccitt(parms, data, dict_width, dict_height, decode),
        "jbig2decode" => decode_jbig2(data, globals, decode),
        "jpxdecode" => decode_jpx(data, decode),
        _ => None,
    }
}

/// 是否有可解码的终止型滤镜（决定要不要尝试生成预览）
pub fn has_decoder(filter: &[String]) -> bool {
    filter.first().is_some_and(|f| {
        matches!(
            f.to_lowercase().as_str(),
            "ccittfaxdecode" | "jbig2decode" | "jpxdecode"
        )
    })
}

fn parm_dict(parms: &[Value]) -> Option<&serde_json::Map<String, Value>> {
    parms
        .iter()
        .find(|v| v.is_object())
        .and_then(|v| v.as_object())
}

fn parm_int(p: Option<&serde_json::Map<String, Value>>, key: &str, default: i64) -> i64 {
    p.and_then(|m| m.get(key))
        .and_then(|v| v.as_i64())
        .unwrap_or(default)
}

fn parm_bool(p: Option<&serde_json::Map<String, Value>>, key: &str, default: bool) -> bool {
    p.and_then(|m| m.get(key))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// /Decode 数组是否要求位反转（`[1 0]`）
fn decode_inverts(decode: Option<&[f64]>) -> bool {
    matches!(decode, Some(d) if d.len() >= 2 && d[0] == 1.0 && d[1] == 0.0)
}

fn gray_to_rgb(bits: &[u8]) -> Vec<u8> {
    bits.iter().flat_map(|&v| [v, v, v]).collect()
}

// === CCITT（G3 1D/2D、G4）===

fn decode_ccitt(
    parms: &[Value],
    data: &[u8],
    dict_width: i64,
    dict_height: i64,
    decode: Option<&[f64]>,
) -> Option<PreviewPixels> {
    let p = parm_dict(parms);
    let k = parm_int(p, "K", 0);
    let columns_default = if dict_width > 0 { dict_width } else { 1728 };
    let columns = parm_int(p, "Columns", columns_default).clamp(1, 1 << 16) as u32;
    let rows_parm = parm_int(p, "Rows", 0);
    let rows = if rows_parm > 0 {
        rows_parm as u32
    } else {
        dict_height.clamp(1, 1 << 17) as u32
    };
    let black_is1 = parm_bool(p, "BlackIs1", false);
    let settings = hayro_ccitt::DecodeSettings {
        columns,
        rows,
        end_of_block: parm_bool(p, "EndOfBlock", true),
        end_of_line: parm_bool(p, "EndOfLine", false),
        rows_are_byte_aligned: parm_bool(p, "EncodedByteAlign", false),
        encoding: if k < 0 {
            hayro_ccitt::EncodingMode::Group4
        } else if k == 0 {
            hayro_ccitt::EncodingMode::Group3_1D
        } else {
            hayro_ccitt::EncodingMode::Group3_2D { k: k as u32 }
        },
        // hayro 的 invert_black 反转码流位义：BlackIs1 即"1=黑"
        invert_black: black_is1,
    };

    struct Collector {
        gray: Vec<u8>,
        cur: Vec<u8>,
    }
    impl hayro_ccitt::Decoder for Collector {
        fn push_pixels(&mut self, white: bool, count: u32) {
            let v: u8 = if white { 255 } else { 0 };
            for _ in 0..count {
                self.cur.push(v);
            }
        }
        fn next_line(&mut self) {
            self.gray.append(&mut self.cur);
        }
    }

    let mut col = Collector {
        gray: Vec::new(),
        cur: Vec::new(),
    };
    let mut ctx = hayro_ccitt::DecoderContext::new(settings);
    hayro_ccitt::decode(data, &mut col, &mut ctx).ok()?;

    // 行截断/补齐到 columns*rows（受损流可能提前结束）
    let expected = columns as usize * rows as usize;
    let mut gray = col.gray;
    gray.truncate(expected);
    gray.resize(expected, 255);
    if decode_inverts(decode) {
        for v in gray.iter_mut() {
            *v = 255 - *v;
        }
    }
    let rgb = gray_to_rgb(&gray);
    Some(PreviewPixels {
        width: columns,
        height: rows,
        rgb,
    })
}

// === JBIG2（嵌入式流 + Globals）===

fn decode_jbig2(
    data: &[u8],
    globals: Option<&[u8]>,
    decode: Option<&[f64]>,
) -> Option<PreviewPixels> {
    let img = hayro_jbig2::Image::new_embedded(data, globals).ok()?;
    let (width, height) = (img.width(), img.height());
    if width == 0 || height == 0 {
        return None;
    }

    struct Collector {
        gray: Vec<u8>,
        cur: Vec<u8>,
    }
    impl hayro_jbig2::Decoder for Collector {
        fn push_pixel(&mut self, black: bool) {
            self.cur.push(if black { 0 } else { 255 });
        }
        fn push_pixel_chunk(&mut self, black: bool, chunk_count: u32) {
            let v: u8 = if black { 0 } else { 255 };
            for _ in 0..chunk_count * 8 {
                self.cur.push(v);
            }
        }
        fn next_line(&mut self) {
            self.gray.append(&mut self.cur);
        }
    }

    let mut col = Collector {
        gray: Vec::new(),
        cur: Vec::new(),
    };
    img.decode(&mut col).ok()?;
    let expected = width as usize * height as usize;
    col.gray.truncate(expected);
    col.gray.resize(expected, 255);
    let mut gray = col.gray;
    if decode_inverts(decode) {
        for v in gray.iter_mut() {
            *v = 255 - *v;
        }
    }
    Some(PreviewPixels {
        width,
        height,
        rgb: gray_to_rgb(&gray),
    })
}

// === JPX（JPEG2000，raw codestream 或 JP2 容器）===

fn decode_jpx(data: &[u8], decode: Option<&[f64]>) -> Option<PreviewPixels> {
    use openjp2::{Decoder, Format};

    // PDF 里 JPXDecode 可能是裸 J2K 码流（ff4f ff51）或 JP2 容器（签名盒）
    let format = if data.starts_with(&[0x00, 0x00, 0x00, 0x0C, 0x6A, 0x50, 0x20, 0x20]) {
        Format::Jp2
    } else {
        Format::J2k
    };
    let mut dec = Decoder::new(format).ok()?;
    let img = dec.decode_slice(data).ok()?;
    let (width, height) = (img.width, img.height);
    if width == 0 || height == 0 {
        return None;
    }
    let comps = &img.components;
    if comps.is_empty() {
        return None;
    }

    /// 平面 → u8（按 precision 归一化，含子采样最近邻上采样）
    fn to_u8_plane(c: &openjp2::ImageComponent, w: u32, h: u32) -> Vec<u8> {
        let max = (1i64 << c.precision.saturating_sub(if c.signed { 1 } else { 0 })) - 1;
        let offset = if c.signed {
            1i64 << (c.precision - 1)
        } else {
            0
        };
        let mut out = vec![0u8; w as usize * h as usize];
        for y in 0..h {
            let sy = (y * c.height / h.max(1)).min(c.height.saturating_sub(1)) as usize;
            for x in 0..w {
                let sx = (x * c.width / w.max(1)).min(c.width.saturating_sub(1)) as usize;
                let v = c.data[sy * c.width as usize + sx] as i64 - offset;
                let v = v.clamp(0, max);
                out[y as usize * w as usize + x as usize] =
                    ((v * 255 + max / 2) / max).clamp(0, 255) as u8;
            }
        }
        out
    }

    let planes: Vec<Vec<u8>> = comps
        .iter()
        .map(|c| to_u8_plane(c, width, height))
        .collect();
    // Adobe CMYK JPX 的 /Decode [1 0 1 0 1 0 1 0]：分量先反相
    let invert_cmyk = matches!(decode, Some(d) if d.len() >= 8 && d[0] == 1.0);
    let rgb: Vec<u8> = match planes.len() {
        1 => gray_to_rgb(&planes[0]),
        4 => {
            let n = width as usize * height as usize;
            let mut rgb = Vec::with_capacity(n * 3);
            for (((&c, &m), &y), &k) in planes[0]
                .iter()
                .zip(planes[1].iter())
                .zip(planes[2].iter())
                .zip(planes[3].iter())
            {
                let (mut c, mut m, mut y) = (c as u16, m as u16, y as u16);
                let k = k as u16;
                if invert_cmyk {
                    c = 255 - c;
                    m = 255 - m;
                    y = 255 - y;
                }
                rgb.push(255 - (c.saturating_add(k)).min(255) as u8);
                rgb.push(255 - (m.saturating_add(k)).min(255) as u8);
                rgb.push(255 - (y.saturating_add(k)).min(255) as u8);
            }
            let _ = n;
            rgb
        }
        _ => {
            let p0 = &planes[0];
            let p1 = planes.get(1).unwrap_or(&planes[0]);
            let p2 = planes.get(2).unwrap_or(&planes[0]);
            let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
            for i in 0..width as usize * height as usize {
                rgb.push(p0[i]);
                rgb.push(p1[i]);
                rgb.push(p2[i]);
            }
            if img.color_space == openjp2::ColorSpace::Sycc {
                openjp2::convert::ycbcr_to_rgb8_in_place(
                    &mut rgb,
                    width,
                    height,
                    openjp2::convert::YCbCrTransform::Bt601OpenSlideRounding,
                );
            }
            rgb
        }
    };
    Some(PreviewPixels { width, height, rgb })
}

/// 解码并编码为 PNG（unpack 侧生成预览文件的统一入口）
pub fn decode_preview_png(
    filter: &[String],
    parms: &[Value],
    data: &[u8],
    globals: Option<&[u8]>,
    dict_width: i64,
    dict_height: i64,
    decode: Option<&[f64]>,
) -> Option<Vec<u8>> {
    let px = decode_preview(
        filter,
        parms,
        data,
        globals,
        dict_width,
        dict_height,
        decode,
    )?;
    super::decode_to_png_bytes(
        &px.rgb,
        px.width as i64,
        px.height as i64,
        Some("DeviceRGB"),
        8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_filter_is_none() {
        assert!(decode_preview(&["dctdecode".into()], &[], b"", None, 1, 1, None).is_none());
        assert!(!has_decoder(&["dctdecode".into()]));
        assert!(has_decoder(&["jbig2decode".into()]));
    }

    #[test]
    fn invalid_data_is_none() {
        assert!(
            decode_preview(&["jbig2decode".into()], &[], b"garbage", None, 10, 10, None).is_none()
        );
        assert!(
            decode_preview(&["jpxdecode".into()], &[], b"garbage", None, 10, 10, None).is_none()
        );
        assert!(decode_preview(&["ccittfaxdecode".into()], &[], b"", None, 10, 10, None).is_none());
    }

    #[test]
    fn gray_to_rgb_expands() {
        assert_eq!(gray_to_rgb(&[0, 255]), vec![0, 0, 0, 255, 255, 255]);
    }
}
