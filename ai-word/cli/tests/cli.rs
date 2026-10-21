//! CLI 集成测试：7 子命令端到端

use json2docx::model::blocks::*;
use json2docx::model::{Document, Part};
use std::path::PathBuf;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_json2docx")
}

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("json2docx-test-{}-{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn make_docx(path: &std::path::Path) {
    let mut doc = Document::default();
    let mut part = Part::default();
    part.blocks.push(Block::Heading {
        level: 1,
        text: "第一章".to_string(),
        numbering: None,
        align: None,
        runs: None,
    });
    part.blocks
        .push(Block::Paragraph(Paragraph::text("原始正文")));
    doc.parts = vec![part];
    json2docx::generate(&doc, path.to_str().unwrap()).unwrap();
}

#[test]
fn templates_lists_presets() {
    let out = Command::new(bin()).arg("templates").output().unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("academic-paper"));
    assert!(s.contains("report"));
}

#[test]
fn unpack_view_edit_repack_flow() {
    let dir = tmpdir("flow");
    std::fs::create_dir_all(&dir).unwrap();
    let docx = dir.join("in.docx");
    make_docx(&docx);

    // unpack
    let prod = dir.join("prod");
    let out = Command::new(bin())
        .args([
            "unpack",
            docx.to_str().unwrap(),
            "-o",
            prod.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(prod.join("document.json").exists());

    // view layout
    let out = Command::new(bin())
        .args(["view", prod.to_str().unwrap(), "/part[1]", "layout"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("原始正文"));

    // edit set (product dir, in place)
    let out = Command::new(bin())
        .args([
            "edit",
            prod.to_str().unwrap(),
            "/part[1]/paragraph[1]",
            "set",
            "--prop",
            "text=修改后正文",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // repack
    let out_docx = dir.join("out.docx");
    let out = Command::new(bin())
        .args([
            "repack",
            prod.to_str().unwrap(),
            "-o",
            out_docx.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed = json2docx::parse(out_docx.to_str().unwrap()).unwrap();
    let text = parsed
        .all_blocks()
        .map(|b| b.plain_text())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("修改后正文"));
}

#[test]
fn edit_docx_without_output_fails() {
    let dir = tmpdir("guard");
    std::fs::create_dir_all(&dir).unwrap();
    let docx = dir.join("in.docx");
    make_docx(&docx);
    let out = Command::new(bin())
        .args([
            "edit",
            docx.to_str().unwrap(),
            "/part[1]/paragraph[1]",
            "set",
            "--prop",
            "text=x",
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn extract_template_command() {
    let dir = tmpdir("extract");
    std::fs::create_dir_all(&dir).unwrap();
    let docx = dir.join("in.docx");
    make_docx(&docx);
    let out = dir.join("tpl");
    let res = Command::new(bin())
        .args([
            "extract",
            docx.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        res.status.success(),
        "{}",
        String::from_utf8_lossy(&res.stderr)
    );
    assert!(out.join("template.json").exists());
    assert!(out.join("TEMPLATE.md").exists());
    let md = std::fs::read_to_string(out.join("TEMPLATE.md")).unwrap();
    assert!(md.contains("段落样式"));
}

#[test]
fn chart_and_textbox_flow() {
    let dir = tmpdir("feat");
    std::fs::create_dir_all(&dir).unwrap();
    let doc_json = dir.join("document.json");
    std::fs::write(
        &doc_json,
        r#"{
          "watermark": "DRAFT",
          "parts": [
            { "blocks": [
              { "type": "heading", "level": 1, "text": "报告" },
              { "type": "chart", "chart_type": "column", "title": "季度",
                "categories": ["Q1","Q2"], "series": [ { "name":"营收", "values":[10,20] } ] },
              { "type": "textbox", "text": "提示文字", "fill": "FFF2CC" }
            ] }
          ]
        }"#,
    )
    .unwrap();

    let out_docx = dir.join("out.docx");
    let out = Command::new(bin())
        .args([
            "repack",
            dir.to_str().unwrap(),
            "-o",
            out_docx.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let parsed = json2docx::parse(out_docx.to_str().unwrap()).unwrap();
    let types: Vec<&str> = parsed.all_blocks().map(|b| b.type_name()).collect();
    assert!(types.contains(&"chart"));
    assert!(types.contains(&"textbox"));
    assert!(types.contains(&"heading"));
}

#[test]
fn render_html_output() {
    let dir = tmpdir("render");
    std::fs::create_dir_all(&dir).unwrap();
    let docx = dir.join("in.docx");
    make_docx(&docx);
    let html = dir.join("out.html");
    let out = Command::new(bin())
        .args([
            "render",
            docx.to_str().unwrap(),
            "/part[1]",
            "-o",
            html.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = std::fs::read_to_string(&html).unwrap();
    assert!(s.contains("原始正文"));
    assert!(s.contains("<h1>"));
}

/// 写一个最小产物目录（document.json + 分片），用于验证 markdown 视图
fn write_product(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("word/parts")).unwrap();
    std::fs::write(
        dir.join("document.json"),
        r#"{"parts": ["word/parts/part1.json", "word/parts/part2.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("word/parts/part1.json"),
        r#"{"blocks": [
            {"type": "heading", "level": 1, "text": "研究报告"},
            {"type": "heading", "level": 2, "text": "背景"},
            {"type": "paragraph", "text": "第一段正文。"},
            {"type": "list", "ordered": false, "items": [
                {"level": 0, "blocks": [{"type": "paragraph", "text": "要点一"}]},
                {"level": 1, "blocks": [{"type": "paragraph", "text": "子要点"}]}
            ]},
            {"type": "table", "header_row": true, "caption": "表 1 数据", "rows": [
                {"cells": [{"blocks": [{"type": "paragraph", "text": "项目"}]},
                           {"blocks": [{"type": "paragraph", "text": "数值"}]}]},
                {"cells": [{"blocks": [{"type": "paragraph", "text": "A"}]},
                           {"blocks": [{"type": "paragraph", "text": "1"}]}]}
            ]},
            {"type": "code", "lang": "python", "text": "print(1)"},
            {"type": "formula", "latex": "E=mc^2", "display": true},
            {"type": "page_break"}
        ]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("word/parts/part2.json"),
        r#"{"blocks": [{"type": "heading", "level": 1, "text": "结论"},
                        {"type": "paragraph", "text": "收尾。"}]}"#,
    )
    .unwrap();
}

#[test]
fn view_text_emits_structured_markdown() {
    let dir = tmpdir("md");
    std::fs::create_dir_all(&dir).unwrap();
    let prod = dir.join("prod");
    write_product(&prod);

    // 默认：stdout 直出 Markdown（不是 JSON），且不带任何注释标记
    let out = Command::new(bin())
        .args(["view", prod.to_str().unwrap(), "/part[1]", "text"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let md = String::from_utf8_lossy(&out.stdout);
    assert!(md.starts_with("# 研究报告\n\n## 背景"), "{md}");
    assert!(md.contains("- 要点一\n  - 子要点"), "{md}");
    assert!(
        md.contains("| 项目 | 数值 |\n| --- | --- |\n| A | 1 |"),
        "{md}"
    );
    assert!(md.contains("```python\nprint(1)\n```"), "{md}");
    assert!(md.contains("$$\nE=mc^2\n$$"), "{md}");
    assert!(md.contains("---"), "分页输出为分隔线：{md}");
    assert!(!md.contains("<!--"), "纯 Markdown 不带注释标记：{md}");

    // --json：结构化信封（markdown + 媒体 + 体检信息）
    let out = Command::new(bin())
        .args(["view", prod.to_str().unwrap(), "/part[1]", "text", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["part"], 1);
    assert!(v["markdown"].as_str().unwrap().starts_with("# 研究报告"));
    assert!(v["media"].is_array() && v["issues"].is_array() && v["warnings"].is_array());

    // 整篇：分片内容按空行拼接，无 `<!-- part N -->` 锚点
    let out = Command::new(bin())
        .args(["view", prod.to_str().unwrap(), "/", "text", "--json"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["parts"], 2);
    let md = v["markdown"].as_str().unwrap();
    assert!(md.starts_with("# 研究报告"), "{md}");
    assert!(md.contains("\n\n# 结论"), "{md}");
    assert!(md.trim_end().ends_with("收尾。"), "{md}");
    assert!(!md.contains("<!--"), "{md}");

    // 整篇不支持 layout（layout 是分片级结构树）
    let out = Command::new(bin())
        .args(["view", prod.to_str().unwrap(), "/", "layout"])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("不支持"));
}

#[test]
fn view_text_exports_attachments_and_rewrites_links() {
    let dir = tmpdir("attach");
    std::fs::create_dir_all(dir.join("word/parts")).unwrap();
    std::fs::create_dir_all(dir.join("word/embeddings")).unwrap();
    let payload = b"PK\x03\x04-embedded-payload";
    std::fs::write(dir.join("word/embeddings/测试数据.xlsx"), payload).unwrap();
    std::fs::write(
        dir.join("document.json"),
        r#"{"parts": ["word/parts/part1.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("word/parts/part1.json"),
        r#"{"blocks": [
            {"type": "heading", "level": 1, "text": "附件章节"},
            {"type": "attachment", "src": "word/embeddings/测试数据.xlsx",
             "name": "测试数据.xlsx", "prog_id": "Excel.Sheet.12", "label": "随附数据"},
            {"type": "attachment", "src": "https://example.com/远程.pdf", "name": "远程.pdf"}
        ]}"#,
    )
    .unwrap();

    // 默认：链接保持模型里的原始引用（包内相对路径 / URL），不落任何文件
    let out = Command::new(bin())
        .args(["view", dir.to_str().unwrap(), "/part[1]", "text"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let md = String::from_utf8_lossy(&out.stdout);
    assert!(
        md.contains("[测试数据.xlsx](word/embeddings/测试数据.xlsx)"),
        "{md}"
    );
    assert!(
        md.contains("[远程.pdf](https://example.com/远程.pdf)"),
        "{md}"
    );

    // --attachments：载荷导出到指定目录，链接指向新路径；URL 原样保留
    let att = dir.join("attach-out");
    let out = Command::new(bin())
        .args([
            "view",
            dir.to_str().unwrap(),
            "/part[1]",
            "text",
            "--attachments",
            att.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let md = String::from_utf8_lossy(&out.stdout);
    let exported = att.join("测试数据.xlsx");
    assert!(exported.is_file(), "附件应已导出");
    assert_eq!(std::fs::read(&exported).unwrap(), payload, "载荷字节应一致");
    assert!(
        md.contains(&format!("[测试数据.xlsx]({})", exported.display())),
        "{md}"
    );
    assert!(
        md.contains("[远程.pdf](https://example.com/远程.pdf)"),
        "URL 原样保留：{md}"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("附件已导出"), "{err}");

    // --json 信封带附件清单（kind=file / url）
    let out = Command::new(bin())
        .args([
            "view",
            dir.to_str().unwrap(),
            "/part[1]",
            "text",
            "--attachments",
            att.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let list = v["attachments"].as_array().unwrap();
    assert_eq!(list.len(), 2, "{v}");
    assert_eq!(list[0]["kind"], "file");
    assert_eq!(list[1]["kind"], "url");

    // 载荷缺失：保留原引用并标注 missing（不静默改链接）
    std::fs::remove_file(dir.join("word/embeddings/测试数据.xlsx")).unwrap();
    let out = Command::new(bin())
        .args([
            "view",
            dir.to_str().unwrap(),
            "/part[1]",
            "text",
            "--attachments",
            att.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["attachments"][0]["kind"], "missing");
    assert!(v["markdown"]
        .as_str()
        .unwrap()
        .contains("(word/embeddings/测试数据.xlsx)"));

    // --attachments 只对 text 有意义
    let out = Command::new(bin())
        .args([
            "view",
            dir.to_str().unwrap(),
            "/part[1]",
            "layout",
            "--attachments",
            att.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("只对 text"));
}

#[test]
fn view_text_exports_media_and_rewrites_image_links() {
    let dir = tmpdir("media");
    std::fs::create_dir_all(dir.join("word/parts")).unwrap();
    std::fs::create_dir_all(dir.join("word/media")).unwrap();
    let png = b"\x89PNG\r\n\x1a\n-fake-png-bytes";
    std::fs::write(dir.join("word/media/image1.png"), png).unwrap();
    std::fs::write(
        dir.join("document.json"),
        r#"{"parts": ["word/parts/part1.json"]}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("word/parts/part1.json"),
        r#"{"blocks": [
            {"type": "image", "src": "word/media/image1.png", "alt": "示例图", "caption": "图 1  示例"},
            {"type": "image", "src": "https://example.com/remote.png", "alt": "远程图"}
        ]}"#,
    )
    .unwrap();

    // 默认：图片链接保持模型里的原始引用（包内相对路径 / URL）
    let out = Command::new(bin())
        .args(["view", dir.to_str().unwrap(), "/part[1]", "text"])
        .output()
        .unwrap();
    let md = String::from_utf8_lossy(&out.stdout);
    assert!(md.contains("![示例图](word/media/image1.png)"), "{md}");
    assert!(
        md.contains("![远程图](https://example.com/remote.png)"),
        "{md}"
    );

    // --media：图片导出到指定目录，链接指向新路径；URL 原样保留；media[] 同步
    let media = dir.join("media-out");
    let out = Command::new(bin())
        .args([
            "view",
            dir.to_str().unwrap(),
            "/part[1]",
            "text",
            "--media",
            media.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let exported = media.join("image1.png");
    assert!(exported.is_file(), "图片应已导出");
    assert_eq!(std::fs::read(&exported).unwrap(), png, "图片字节应一致");
    let md = v["markdown"].as_str().unwrap();
    assert!(
        md.contains(&format!("![示例图]({})", exported.display())),
        "{md}"
    );
    assert!(
        md.contains("![远程图](https://example.com/remote.png)"),
        "URL 保留：{md}"
    );
    // media[] 与 media_exported 都指向导出路径
    assert_eq!(v["media"][0]["src"], exported.display().to_string(), "{v}");
    let list = v["media_exported"].as_array().unwrap();
    assert_eq!(list.len(), 2, "同一图片只导出一份：{v}");
    assert_eq!(list[0]["kind"], "file");
    assert_eq!(list[1]["kind"], "url");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("已导出"), "{err}");

    // 只对 text 模式有意义
    let out = Command::new(bin())
        .args([
            "view",
            dir.to_str().unwrap(),
            "/part[1]",
            "layout",
            "--media",
            media.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("只对 text"));
}

#[test]
fn view_text_revision_modes_and_annotations_from_docx() {
    let dir = tmpdir("revision-text");
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("revisions.docx");
    let doc: Document = serde_json::from_value(serde_json::json!({
        "parts":[{"blocks":[{"type":"heading", "level":1,"text":"旧题新题", "runs":[
            {"text":"旧题", "revision":"del"}, {"text":"新题", "revision":"ins"}
        ]}, {"type":"paragraph","runs":[
            {"text":"废止条款", "revision":"del", "revision_author":"法务"},
            {"text":"生效条款", "revision":"ins", "revision_author":"法务"},
            {"text":"来源", "hyperlink":"https://example.com/source", "footnote":"说明口径", "endnote":"参考材料", "comment":"待核对", "comment_author":"审阅人"}
        ]}]}]
    })).unwrap();
    json2docx::generate(&doc, input.to_str().unwrap()).unwrap();
    // The heading generator currently emits run formatting without revision wrappers.
    // Use actual OOXML tracked changes here to exercise the reader independently.
    let xml = office_core::opc::read_part(&input, "word/document.xml")
        .unwrap()
        .unwrap();
    let xml = String::from_utf8(xml).unwrap();
    let start = xml
        .find("<w:p><w:pPr><w:pStyle w:val=\"Heading1\"")
        .unwrap();
    let end = start + xml[start..].find("</w:p>").unwrap() + "</w:p>".len();
    let heading = r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:del w:id="90" w:author="法务"><w:r><w:delText>旧题</w:delText></w:r></w:del><w:ins w:id="91" w:author="法务"><w:r><w:t>新题</w:t></w:r></w:ins></w:p>"#;
    let patched = format!("{}{}{}", &xml[..start], heading, &xml[end..]);
    let fixture = dir.join("fixture.docx");
    office_core::opc::write_part(&input, &fixture, "word/document.xml", patched.as_bytes())
        .unwrap();
    let input = fixture;
    for (mode, expected, excluded) in [
        ("final", "生效条款", Some("废止条款")),
        ("original", "废止条款", Some("生效条款")),
        ("markup", "<del>废止条款</del>", None),
    ] {
        let out = Command::new(bin())
            .args([
                "view",
                input.to_str().unwrap(),
                "/",
                "text",
                "--revisions",
                mode,
            ])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let md = String::from_utf8_lossy(&out.stdout);
        match mode {
            "final" => assert!(md.contains("# 新题") && !md.contains("旧题"), "{md}"),
            "original" => assert!(md.contains("# 旧题") && !md.contains("新题"), "{md}"),
            _ => assert!(
                md.contains("<del>旧题</del>") && md.contains("<ins>新题</ins>"),
                "{md}"
            ),
        }
        assert!(md.contains(expected), "{mode}: {md}");
        if let Some(excluded) = excluded {
            assert!(!md.contains(excluded), "{mode}: {md}");
        }
        for annotation in [
            "[来源](https://example.com/source)",
            "说明口径",
            "参考材料",
            "待核对",
            "审阅人",
        ] {
            assert!(md.contains(annotation), "missing {annotation}: {md}");
        }
    }
    let out = Command::new(bin())
        .args(["view", input.to_str().unwrap(), "/", "text"])
        .output()
        .unwrap();
    let md = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && md.contains("生效条款") && !md.contains("废止条款"),
        "{md}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn resource_export_manifest_and_strict_mode_preserve_each_failure() {
    let dir = tmpdir("resource-report");
    std::fs::create_dir_all(dir.join("word/embeddings")).unwrap();
    std::fs::write(dir.join("word/embeddings/good.pdf"), b"abc").unwrap();
    let good = serde_json::json!({"type":"attachment","name":"report #1.pdf","src":"word/embeddings/good.pdf"});
    let missing = serde_json::json!({"type":"attachment","name":"missing.pdf","src":"word/embeddings/missing.pdf"});
    let doc = serde_json::json!({"parts":[{"blocks":[
        good.clone(), missing.clone(), missing,
        {"type":"attachment","name":format!("{}.pdf", "a".repeat(300)),"src":"word/embeddings/copy-source.pdf"},
        {"type":"table","rows":[{"cells":[{"blocks":[good.clone()]}]}]},
        {"type":"list","ordered":false,"items":[{"blocks":[good]}]}
    ]}]});
    // Separate source file gives the intentionally overlong target its own export record.
    std::fs::write(dir.join("word/embeddings/copy-source.pdf"), b"abc").unwrap();
    std::fs::write(dir.join("document.json"), serde_json::to_vec(&doc).unwrap()).unwrap();
    let mut identity = None;
    for strict in [false, true] {
        let output = dir.join(if strict { "strict" } else { "tolerant" });
        let mut command = Command::new(bin());
        command.args([
            "view",
            dir.to_str().unwrap(),
            "/",
            "text",
            "--attachments",
            output.to_str().unwrap(),
            "--json",
        ]);
        if strict {
            command.arg("--strict");
        }
        let result = command.output().unwrap();
        assert_eq!(result.status.code(), Some(if strict { 1 } else { 0 }));
        let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        let entries = report["attachments"].as_array().unwrap();
        assert_eq!(entries.len(), 3, "{report}");
        let good = entries
            .iter()
            .find(|entry| entry["kind"] == "file")
            .unwrap();
        assert_eq!(good["size"], 3);
        assert_eq!(good["resource_type"], "application/pdf");
        assert_eq!(
            good["sha256"],
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(good["relative_link"], "report%20%231.pdf");
        let locations = good["source_locations"].as_array().unwrap();
        assert!(
            locations.contains(&serde_json::json!(
                "/part[1]/block[5]/row[1]/cell[1]/block[1]"
            )),
            "{good}"
        );
        assert!(
            locations.contains(&serde_json::json!("/part[1]/block[6]/item[1]/block[1]")),
            "{good}"
        );
        if let Some(identity) = &identity {
            assert_eq!(&good["resource_id"], identity);
        } else {
            identity = Some(good["resource_id"].clone());
        }
        assert!(entries
            .iter()
            .any(|entry| entry["error"]["code"] == "payload_missing"
                && entry["source_locations"].as_array().unwrap().len() == 2));
        assert!(entries
            .iter()
            .any(|entry| entry["error"]["code"] == "copy_failed" && entry["kind"] == "error"));
        assert_eq!(report["export_summary"]["failed"], 2);
        if strict {
            assert_eq!(report["ok"], false);
            assert_eq!(report["error"]["code"], "resource_export_failed");
        }
    }
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, b"existing").unwrap();
    let result = Command::new(bin())
        .args([
            "view",
            dir.to_str().unwrap(),
            "/",
            "text",
            "--attachments",
            blocked.to_str().unwrap(),
            "--strict",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(report["attachments"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["error"]["code"] == "create_dir_failed"));
    assert_eq!(std::fs::read(blocked).unwrap(), b"existing");
    std::fs::remove_dir_all(dir).unwrap();
}
