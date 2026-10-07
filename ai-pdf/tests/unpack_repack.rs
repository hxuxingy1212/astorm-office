//! 双向解析端到端测试：PDF → 产物目录 → PDF round-trip，
//! 以及手写产物目录 JSON（含中文，走 CID 字体嵌入）→ PDF → 提取回读。

use json2pdf::{repack, text, unpack};
use lopdf::Document;
use serde_json::{json, Value};

/// 与 tests/integration.rs 同构的最小合法 PDF 构造。
fn build_pdf(pages_content: &[&str]) -> Vec<u8> {
    let mut objs: Vec<(u32, String)> = Vec::new();
    let n = pages_content.len();
    objs.push((1, "<< /Type /Catalog /Pages 2 0 R >>".into()));
    let kids: Vec<String> = (0..n)
        .map(|i| format!("{} 0 R", 3 + i as u32 * 2))
        .collect();
    objs.push((
        2,
        format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")),
    ));
    let font_num = 3 + n as u32 * 2;
    for (i, content) in pages_content.iter().enumerate() {
        let page_num = 3 + i as u32 * 2;
        let content_num = page_num + 1;
        objs.push((
            page_num,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                 /Resources << /Font << /F1 {font_num} 0 R >> >> /Contents {content_num} 0 R >>"
            ),
        ));
        objs.push((
            content_num,
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        ));
    }
    objs.push((
        font_num,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
    ));
    let mut out = String::from("%PDF-1.7\n");
    let mut offsets: Vec<(u32, u64)> = Vec::new();
    for (num, body) in &objs {
        offsets.push((*num, out.len() as u64));
        out.push_str(&format!("{num} 0 obj\n{body}\nendobj\n"));
    }
    let max = objs.iter().map(|(n, _)| *n).max().unwrap_or(0);
    let xref_start = out.len();
    out.push_str(&format!("xref\n0 {}\n", max + 1));
    out.push_str("0000000000 65535 f \n");
    for num in 1..=max {
        let off = offsets
            .iter()
            .find(|(n, _)| n == &num)
            .map(|(_, o)| *o)
            .unwrap_or(0);
        out.push_str(&format!("{off:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_start}\n%%EOF",
        max + 1
    ));
    out.into_bytes()
}

#[test]
fn unpack_produces_artifact_dir() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("in.pdf");
    std::fs::write(
        &pdf,
        build_pdf(&[
            "BT /F1 14 Tf 0.13 0.27 0.43 rg 72 720 Td (Hello Roundtrip) Tj ET",
            "BT /F1 12 Tf 72 680 Td (Second) Tj ET",
        ]),
    )
    .unwrap();

    let out = dir.path().join("artifact");
    let r = unpack::unpack(&pdf, &out).unwrap();
    assert_eq!(r.pages, 2);
    assert!(out.join("document.json").exists());
    assert!(out.join("pages/page-001.json").exists());
    assert!(out.join("pages/page-002.json").exists());

    let doc_json: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("document.json")).unwrap()).unwrap();
    assert_eq!(doc_json["pages"].as_array().unwrap().len(), 2);

    let page1: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("pages/page-001.json")).unwrap())
            .unwrap();
    let elements = page1["elements"].as_array().unwrap();
    let texts: Vec<&str> = elements
        .iter()
        .filter(|e| e["type"] == "text")
        .map(|e| e["text"].as_str().unwrap())
        .collect();
    assert!(
        texts.iter().any(|t| t.contains("Hello Roundtrip")),
        "texts: {texts:?}"
    );
    // 第二个文本在第 2 页
    let page2: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("pages/page-002.json")).unwrap())
            .unwrap();
    let texts2: Vec<&str> = page2["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "text")
        .map(|e| e["text"].as_str().unwrap())
        .collect();
    assert!(
        texts2.iter().any(|t| t.contains("Second")),
        "texts2: {texts2:?}"
    );
    // 颜色被采集
    let colored = elements
        .iter()
        .find(|e| e["type"] == "text" && e["text"].as_str().unwrap().contains("Hello"))
        .unwrap();
    assert_eq!(colored["color"], "#21446d"); // 0.13/0.27/0.43 as f32×255 向零取整
    assert_eq!(colored["font"], "Helvetica");
    assert_eq!(colored["size"], 14.0);
}

#[test]
fn repack_roundtrip_preserves_text() {
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("in.pdf");
    std::fs::write(
        &pdf,
        build_pdf(&["BT /F1 14 Tf 72 720 Td (Alpha beta) Tj 0 -24 Td (Gamma delta) Tj ET"]),
    )
    .unwrap();

    let artifact = dir.path().join("artifact");
    unpack::unpack(&pdf, &artifact).unwrap();
    let rebuilt = dir.path().join("rebuilt.pdf");
    let r = repack::repack(&artifact, &rebuilt).unwrap();
    assert_eq!(r.pages, 1);

    // 回读：文本应保持
    let doc = Document::load(&rebuilt).unwrap();
    let pts = text::extract_all(&doc).unwrap();
    let t = pts[0].text();
    assert!(t.contains("Alpha beta"), "got: {t:?}");
    assert!(t.contains("Gamma delta"), "got: {t:?}");
    // 再 unpack 一次，JSON 中文本一致
    let artifact2 = dir.path().join("artifact2");
    unpack::unpack(&rebuilt, &artifact2).unwrap();
    let page1: Value = serde_json::from_str(
        &std::fs::read_to_string(artifact2.join("pages/page-001.json")).unwrap(),
    )
    .unwrap();
    let texts: Vec<String> = page1["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "text")
        .map(|e| e["text"].as_str().unwrap().to_string())
        .collect();
    assert!(texts.iter().any(|t| t.contains("Alpha beta")), "{texts:?}");
    assert!(texts.iter().any(|t| t.contains("Gamma delta")), "{texts:?}");
}

#[test]
fn repack_rect_and_polyline() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("artifact");
    std::fs::create_dir_all(artifact.join("pages")).unwrap();
    std::fs::write(
        artifact.join("document.json"),
        json!({
            "version": 1,
            "page_size": {"width": 400.0, "height": 300.0},
            "pages": ["pages/page-001.json"]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        artifact.join("pages/page-001.json"),
        json!({
            "elements": [
                {"type": "rect", "x": 10.0, "y": 10.0, "w": 100.0, "h": 50.0,
                 "fill": "#ff0000", "stroke": "#000000", "line_width": 2.0},
                {"type": "polyline", "points": [[10,100],[200,100],[200,200]],
                 "stroke": "#00ff00", "line_width": 1.0},
                {"type": "text", "text": "Label", "x": 20.0, "y": 250.0, "size": 10.0}
            ]
        })
        .to_string(),
    )
    .unwrap();

    let out = dir.path().join("out.pdf");
    repack::repack(&artifact, &out).unwrap();
    // 生成产物可解析且有绘图
    let doc = Document::load(&out).unwrap();
    let pts = text::extract_all(&doc).unwrap();
    assert!(pts[0].has_drawings, "rect/polyline should be drawing ops");
    assert!(pts[0].text().contains("Label"));
}

#[test]
#[cfg(target_os = "macos")]
fn repack_chinese_text_via_cid_font() {
    // 手写产物目录：中文文本 → 系统字体嵌入 → 生成 PDF → 提取回读
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("artifact");
    std::fs::create_dir_all(artifact.join("pages")).unwrap();
    std::fs::write(
        artifact.join("document.json"),
        json!({
            "version": 1,
            "meta": {"title": "双向解析测试"},
            "page_size": {"width": 400.0, "height": 300.0},
            "pages": ["pages/page-001.json"]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(artifact.join("pages/page-001.json"), json!({
        "elements": [
            {"type": "text", "text": "季度营收报告", "x": 40.0, "y": 250.0, "size": 18.0,
             "font": "system:Arial Unicode", "color": "#23456e"},
            {"type": "text", "text": "中英混排 Revenue 32%", "x": 40.0, "y": 200.0, "size": 12.0,
             "font": "system:Arial Unicode", "color": "#000000"}
        ]
    }).to_string()).unwrap();

    let out = dir.path().join("cn.pdf");
    let r = repack::repack(&artifact, &out).unwrap();
    assert!(
        !r.fonts.is_empty(),
        "CID font should be embedded: {:?}",
        r.fonts
    );

    // 生成的 PDF 用我们自己的提取器回读（走 ToUnicode CMap 解码）
    let doc = Document::load(&out).unwrap();
    let pts = text::extract_all(&doc).unwrap();
    let t = pts[0].text();
    assert!(t.contains("季度营收报告"), "got: {t:?}");
    assert!(t.contains("中英混排 Revenue 32%"), "got: {t:?}");
    assert!(t.contains("32%"), "got: {t:?}");
    // 元数据 round-trip
    assert_eq!(r.pages, 1);
}

#[test]
fn repack_fontfile_stream_declares_flate_filter() {
    // 回归：字体流是 zlib 压缩写入的，必须声明 /Filter /FlateDecode。
    // 缺失时阅读器把压缩字节当字体数据（FT "unknown file format"），
    // 静默回退替代字体（评测语料 12% 样本受影响）。
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("artifact");
    std::fs::create_dir_all(artifact.join("pages")).unwrap();
    std::fs::write(
        artifact.join("document.json"),
        json!({
            "version": 1,
            "page_size": {"width": 300.0, "height": 200.0},
            "pages": ["pages/page-001.json"]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        artifact.join("pages/page-001.json"),
        json!({
            "elements": [
                {"type": "text", "text": "font filter regression", "x": 20.0, "y": 150.0,
                 "size": 14.0, "font": "system:Arial Unicode", "color": "#000000"}
            ]
        })
        .to_string(),
    )
    .unwrap();

    let out = dir.path().join("ff.pdf");
    repack::repack(&artifact, &out).unwrap();
    let doc = Document::load(&out).unwrap();
    let mut fontfiles = 0;
    for (_, obj) in doc.objects.iter() {
        if let lopdf::Object::Stream(s) = obj {
            let is_fontfile = s.dict.has(b"Length1")
                || s.dict.has(b"Subtype")
                    && s.dict
                        .get(b"Subtype")
                        .ok()
                        .and_then(|o| o.as_name_str().ok())
                        .map(|n| n == "OpenType" || n == "Type1C" || n == "CIDFontType0C")
                        .unwrap_or(false);
            let in_font = s.dict.get(b"Length1").is_ok();
            if is_fontfile && in_font {
                fontfiles += 1;
                let filter = s.dict.get(b"Filter").expect("FontFile 流必须声明 /Filter");
                assert_eq!(filter, &lopdf::Object::Name(b"FlateDecode".to_vec()));
                // 压缩数据以 zlib 魔数 0x78 起始，声明与数据一致
                assert_eq!(s.content.first(), Some(&0x78), "zlib 数据头");
            }
        }
    }
    assert!(fontfiles > 0, "应存在至少一个嵌入字体流");
}

#[test]
fn type3_bitmap_font_passthrough() {
    // 回归：Type3 字体（CharProcs 内 /Do 引用图片，如 sbix 位图字形）
    // 必须字体直通而非扁平化为页面图片——阅读器的字形缓存栅格化与
    // 直接画图存在采样相位差（sbix_bungee 5.01 / sbix_compyx 4.33）
    let dir = tempfile::tempdir().unwrap();
    let pdf = dir.path().join("t3.pdf");
    std::fs::write(&pdf, build_type3_pdf()).unwrap();

    let artifact = dir.path().join("artifact");
    unpack::unpack(&pdf, &artifact).unwrap();

    // 产物应包含 type3 字体资源（CharProcs + 图片 XObject）
    let mut found = None;
    for e in std::fs::read_dir(artifact.join("fonts")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map(|x| x == "json").unwrap_or(false) {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
            if v["format"] == "type3" {
                found = Some(v);
            }
        }
    }
    let v = found.expect("应提取 type3 字体资源");
    assert!(
        !v["type3"]["charprocs"].as_array().unwrap().is_empty(),
        "CharProcs 缺失"
    );
    assert!(
        !v["type3"]["xobjects"].as_object().unwrap().is_empty(),
        "字形图片未提取"
    );

    // 重建：应输出 /Subtype /Type3 且文本按单字节码写出
    let out = dir.path().join("t3-out.pdf");
    repack::repack(&artifact, &out).unwrap();
    let data = std::fs::read(&out).unwrap();
    let text = String::from_utf8_lossy(&data);
    assert!(
        text.contains("/Subtype/Type3") || text.contains("/Subtype /Type3"),
        "应重建 Type3 字体"
    );
    assert!(text.contains("/CharProcs"), "应带 CharProcs");
}

/// 构造一个最小 Type3 字体 PDF：一个字形 'A'（CharProc 内画 1x1 图片）。
fn build_type3_pdf() -> Vec<u8> {
    let mut objs: Vec<(u32, Vec<u8>)> = Vec::new();
    objs.push((1, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()));
    objs.push((2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()));
    let content = b"BT /F1 12 Tf 1 0 0 1 50 100 Tm <41> Tj ET".to_vec();
    objs.push((
        3,
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>".to_vec(),
    ));
    objs.push((
        4,
        [
            b"<< /Length ".to_vec(),
            content.len().to_string().into_bytes(),
            b" >>\nstream\n".to_vec(),
            content,
            b"\nendstream".to_vec(),
        ]
        .concat(),
    ));
    // 图片 XObject（1x1 红点）
    let img = b"q 1 0 0 1 0 0 cm /I1 Do Q".to_vec();
    let _ = img;
    let imgdata = vec![255u8, 0, 0];
    objs.push((6, [b"<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length 3 >>\nstream\n".to_vec(), imgdata, b"\nendstream".to_vec()].concat()));
    // CharProc：画图片
    let proc = b"1 0 0 1 0 0 cm /I1 Do".to_vec();
    objs.push((
        7,
        [
            b"<< /Length ".to_vec(),
            proc.len().to_string().into_bytes(),
            b" >>\nstream\n".to_vec(),
            proc,
            b"\nendstream".to_vec(),
        ]
        .concat(),
    ));
    objs.push((
        5,
        b"<< /Type /Font /Subtype /Type3 /Name /T3 /FontBBox [0 0 1000 1000] \
/FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << /A 7 0 R >> \
/Encoding << /Type /Encoding /Differences [65 /A] >> /FirstChar 65 /LastChar 65 /Widths [600] \
/Resources << /XObject << /I1 6 0 R >> >> >>"
            .to_vec(),
    ));
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offs = std::collections::BTreeMap::new();
    for (n, body) in &objs {
        offs.insert(*n, out.len());
        out.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for n in 1..=objs.len() as u32 {
        out.extend_from_slice(format!("{:010} 00000 n \n", offs[&n]).as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[test]
fn mirrored_text_roundtrip() {
    // 回归：镜像文本基（det<0，如转置 0 1 1 0）此前被当作普通 90° 旋转，
    // 镜像方向丢失（fpdf2 mirror_multi_cell diff 3.21）
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("artifact");
    std::fs::create_dir_all(artifact.join("pages")).unwrap();
    std::fs::write(
        artifact.join("document.json"),
        json!({
            "version": 1,
            "page_size": {"width": 400.0, "height": 300.0},
            "pages": ["pages/page-001.json"]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        artifact.join("pages/page-001.json"),
        json!({
            "elements": [
                {"type": "text", "text": "MIRRORED", "x": 100.0, "y": 150.0, "size": 12.0,
                 "font": "Helvetica", "color": "#000000", "rotation": 90.0, "mirror": true}
            ]
        })
        .to_string(),
    )
    .unwrap();

    let out = dir.path().join("mirror.pdf");
    repack::repack(&artifact, &out).unwrap();

    // 回读：镜像标志与角度、位置均应保留
    let artifact2 = dir.path().join("artifact2");
    unpack::unpack(&out, &artifact2).unwrap();
    let page: Value = serde_json::from_str(
        &std::fs::read_to_string(artifact2.join("pages/page-001.json")).unwrap(),
    )
    .unwrap();
    let texts: Vec<&Value> = page["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "text")
        .collect();
    assert!(!texts.is_empty(), "应提取到文本");
    let t = texts[0];
    assert_eq!(t["text"], "MIRRORED", "got: {t}");
    assert_eq!(t["mirror"], json!(true), "镜像标志丢失: {t}");
    assert!(
        (t["rotation"].as_f64().unwrap() - 90.0).abs() < 0.05,
        "got: {t}"
    );
    assert!((t["x"].as_f64().unwrap() - 100.0).abs() < 0.5, "got: {t}");
    assert!((t["y"].as_f64().unwrap() - 150.0).abs() < 0.5, "got: {t}");
}

#[test]
fn shading_luminosity_mask_roundtrip() {
    // 回归：ExtGState /SMask /S /Luminosity 的渐变掩膜此前被当作实心 alpha，
    // 整块变实心（fpdf2 gradient_opacity 27.3）。重建为 DeviceGray 表单
    // + /SMask，回读应保留 mask 渐变。
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("artifact");
    std::fs::create_dir_all(artifact.join("pages")).unwrap();
    std::fs::write(
        artifact.join("document.json"),
        json!({
            "version": 1,
            "page_size": {"width": 200.0, "height": 200.0},
            "pages": ["pages/page-001.json"]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(artifact.join("pages/page-001.json"), json!({
        "elements": [
            {"type": "shading", "kind": "axial", "coords": [20.0, 180.0, 180.0, 180.0],
             "stops": [{"offset": 0.0, "color": "#000000"}, {"offset": 1.0, "color": "#000000"}],
             "extend": [true, true], "bbox": [20.0, 20.0, 180.0, 180.0],
             "mask": {"kind": "axial", "coords": [20.0, 180.0, 180.0, 180.0],
                      "stops": [{"offset": 0.0, "color": "#ffffff"}, {"offset": 1.0, "color": "#000000"}],
                      "extend": [true, true], "bbox": [20.0, 20.0, 180.0, 180.0]}}
        ]
    }).to_string()).unwrap();

    let out = dir.path().join("mask.pdf");
    repack::repack(&artifact, &out).unwrap();
    let data = std::fs::read(&out).unwrap();
    let text = String::from_utf8_lossy(&data);
    assert!(text.contains("/Luminosity"), "应输出 /SMask /S /Luminosity");
    assert!(text.contains("/DeviceGray"), "掩膜表单应为 DeviceGray 组");

    // 回读：mask 渐变应保留
    let artifact2 = dir.path().join("artifact2");
    unpack::unpack(&out, &artifact2).unwrap();
    let page: Value = serde_json::from_str(
        &std::fs::read_to_string(artifact2.join("pages/page-001.json")).unwrap(),
    )
    .unwrap();
    let sh = page["elements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "shading")
        .expect("应有 shading 元素");
    assert!(sh.get("mask").is_some(), "mask 渐变丢失: {sh}");
}

#[test]
fn repack_image_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let artifact = dir.path().join("artifact");
    std::fs::create_dir_all(artifact.join("pages")).unwrap();
    std::fs::create_dir_all(artifact.join("media")).unwrap();

    // 2x2 RGB PNG（经 json2pdf 的像素→PNG 编码器生成）
    let pixels = vec![255u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0];
    let png_bytes =
        json2pdf::images::decode_to_png_bytes(&pixels, 2, 2, Some("DeviceRGB"), 8).unwrap();
    std::fs::write(artifact.join("media/test.png"), png_bytes).unwrap();

    std::fs::write(
        artifact.join("document.json"),
        json!({
            "version": 1,
            "page_size": {"width": 200.0, "height": 200.0},
            "pages": ["pages/page-001.json"]
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(artifact.join("pages/page-001.json"), json!({
        "elements": [
            {"type": "image", "src": "media/test.png", "x": 10.0, "y": 20.0, "w": 50.0, "h": 50.0}
        ]
    }).to_string()).unwrap();

    let out = dir.path().join("img.pdf");
    let r = repack::repack(&artifact, &out).unwrap();
    assert_eq!(r.images, 1, "warnings: {:?}", r.warnings);

    // 回 unpack：应得到相同放置的 image 元素
    let artifact2 = dir.path().join("artifact2");
    unpack::unpack(&out, &artifact2).unwrap();
    let page1: Value = serde_json::from_str(
        &std::fs::read_to_string(artifact2.join("pages/page-001.json")).unwrap(),
    )
    .unwrap();
    let images: Vec<&Value> = page1["elements"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["type"] == "image")
        .collect();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0]["x"], 10.0);
    assert_eq!(images[0]["y"], 20.0);
    assert_eq!(images[0]["w"], 50.0);
    assert_eq!(images[0]["h"], 50.0);
    // 媒体文件存在且为 PNG
    let src = images[0]["src"].as_str().unwrap();
    assert!(
        artifact2.join(src).exists(),
        "media file {src} should exist"
    );
}
