//! 统一 CLI 契约测试（docs/cli-conventions.md）：--json 结果、状态流分离、
//! validate/dump/raw/raw-set、退出码约定。

use json2docx::model::blocks::*;
use json2docx::model::{Document, Meta, Part};
use std::path::{Path, PathBuf};
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_json2docx"))
}

struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Run {
    let out = cli().args(args).output().expect("CLI 执行失败");
    Run {
        code: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("json2docx_contract").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_docx(dir: &Path, name: &str) -> PathBuf {
    let mut doc = Document {
        meta: Meta {
            title: Some("契约测试".to_string()),
            author: Some("json2docx".to_string()),
            subject: None,
            keywords: vec![],
        },
        ..Default::default()
    };
    let mut part = Part::default();
    part.blocks.push(Block::Heading {
        level: 1,
        text: "第一章".to_string(),
        numbering: None,
        align: None,
        runs: None,
    });
    part.blocks.push(Block::Paragraph(Paragraph {
        text: Some("正文内容。".to_string()),
        ..Default::default()
    }));
    doc.parts.push(part);
    let docx = dir.join(name);
    json2docx::generate(&doc, docx.to_str().unwrap()).expect("generate 失败");
    docx
}

#[test]
fn status_stream_and_json_mode() {
    let dir = temp_dir("flags");
    let docx = make_docx(&dir, "paper.docx");
    let product = dir.join("u");

    // 默认：状态行在 stderr，stdout 为空
    let r = run(&[
        "unpack",
        docx.to_str().unwrap(),
        "-o",
        product.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert!(r.stdout.trim().is_empty(), "默认模式 stdout 应为空");
    assert!(r.stderr.contains("已解包"), "{}", r.stderr);

    // --json：repack 结果 JSON 在 stdout
    let out_docx = dir.join("re.docx");
    let r = run(&[
        "--json",
        "repack",
        product.to_str().unwrap(),
        "-o",
        out_docx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("stdout 应为 JSON");
    assert_eq!(v["ok"], serde_json::Value::Bool(true));
    assert_eq!(v["issue_count"], 0);
}

#[test]
fn validate_dump_raw() {
    let dir = temp_dir("commands");
    let docx = make_docx(&dir, "paper.docx");

    // validate（.docx 直校验）：无问题退出 0
    let r = run(&["validate", docx.to_str().unwrap()]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["count"], 0);

    // dump：可回放指令（含 add heading / add paragraph）
    let r = run(&["dump", docx.to_str().unwrap()]);
    assert_eq!(r.code, Some(0));
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["version"], 1);
    let cmds = v["commands"].as_array().unwrap();
    assert!(cmds.iter().any(|c| c["type"] == "heading"));
    assert!(cmds.iter().any(|c| c["type"] == "paragraph"));

    // raw：读取字面部件
    let r = run(&["raw", docx.to_str().unwrap(), "word/document.xml"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert!(r.stdout.contains("w:document"));

    // raw-set：缺 -o 报错；-o 与输入相同报错；正常路径输入不被改写
    let original = std::fs::read(&docx).unwrap();
    let part = dir.join("part.xml");
    std::fs::write(&part, "<custom/>").unwrap();

    let r = run(&[
        "raw-set",
        docx.to_str().unwrap(),
        "customXml/item1.xml",
        "--file",
        part.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(1));

    let r = run(&[
        "raw-set",
        docx.to_str().unwrap(),
        "customXml/item1.xml",
        "--file",
        part.to_str().unwrap(),
        "-o",
        docx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(1));

    let out_docx = dir.join("out.docx");
    let r = run(&[
        "raw-set",
        docx.to_str().unwrap(),
        "customXml/item1.xml",
        "--file",
        part.to_str().unwrap(),
        "-o",
        out_docx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert_eq!(
        std::fs::read(&docx).unwrap(),
        original,
        "输入文件不得被修改"
    );
    let r = run(&["raw", out_docx.to_str().unwrap(), "customXml/item1.xml"]);
    assert_eq!(r.stdout, "<custom/>");
}
