//! 统一 CLI 契约测试（docs/cli-conventions.md）：--json 结果、退出码 0/1/3、
//! validate/dump/extract/raw 命令、raw-set 不覆写输入。

mod common;

use std::path::PathBuf;
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_json2pptx"))
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
    let dir = std::env::temp_dir().join("json2pptx_contract").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 生成一份最小演示文稿（一张幻灯片 + 一个越界/正常文本可调）
fn make_pptx(dir: &std::path::Path, name: &str, x: f64) -> PathBuf {
    let pres = json2pptx::model::Presentation {
        width: 13.333,
        height: 7.5,
        meta: Some(json2pptx::model::Meta {
            company: None,
            application: None,
            last_modified_by: None,
            title: Some("契约测试".to_string()),
            author: Some("json2pptx".to_string()),
        }),
        template: None,
        theme: None,
        table_styles: None,
        slides: vec![json2pptx::model::Slide {
            background: None,
            transition: None,
            notes: None,
            elements: vec![json2pptx::model::elements::Element::Text(
                json2pptx::model::elements::TextElement {
                    text: json2pptx::model::elements::TextContent::Simple("标题".to_string()),
                    position: json2pptx::model::elements::Position {
                        x,
                        y: 1.0,
                        w: 8.0,
                        h: 1.0,
                    },
                    name: Some("Title".to_string()),
                    ..Default::default()
                },
            )],
        }],
    };
    let pptx = dir.join(name);
    json2pptx::generate(&pres, pptx.to_str().unwrap()).expect("generate 失败");
    pptx
}

#[test]
fn json_mode_and_status_stream() {
    let dir = temp_dir("flags");
    let pptx = make_pptx(&dir, "deck.pptx", 1.0);
    let out_dir = dir.join("u");

    // 默认：unpack 状态行在 stderr，stdout 为空
    let r = run(&[
        "unpack",
        pptx.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert!(r.stdout.trim().is_empty(), "默认模式 stdout 应为空");
    assert!(r.stderr.contains("已解包"), "{}", r.stderr);

    // --json：repack 结果 JSON 在 stdout
    let out_pptx = dir.join("re.pptx");
    let r = run(&[
        "--json",
        "repack",
        out_dir.to_str().unwrap(),
        "-o",
        out_pptx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("stdout 应为 JSON");
    assert_eq!(v["ok"], serde_json::Value::Bool(true));
    assert_eq!(v["slides"], 1);
}

#[test]
fn validate_exit_codes_and_dump() {
    let dir = temp_dir("validate");
    let ok_pptx = make_pptx(&dir, "ok.pptx", 1.0);
    let warn_pptx = make_pptx(&dir, "warn.pptx", 100.0); // 越界 → warning 级，不影响退出码

    let r = run(&["validate", ok_pptx.to_str().unwrap()]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["count"], 0);

    // warning 级问题：报告但不改变退出码
    let r = run(&["validate", warn_pptx.to_str().unwrap()]);
    assert_eq!(r.code, Some(0), "warning 不应导致退出 3: {}", r.stdout);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert!(v["count"].as_u64().unwrap() >= 1);

    // error 级问题（图片缺 src）→ 退出码 3
    let product = dir.join("prod");
    let r = run(&[
        "unpack",
        ok_pptx.to_str().unwrap(),
        "-o",
        product.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0));
    let slide_path = product.join("ppt/slides/slide1.json");
    let mut slide: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&slide_path).unwrap()).unwrap();
    slide["elements"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "type": "image",
            "src": "",
            "position": { "x": 1.0, "y": 1.0, "w": 1.0, "h": 1.0 }
        }));
    std::fs::write(&slide_path, serde_json::to_string_pretty(&slide).unwrap()).unwrap();
    let r = run(&["validate", product.to_str().unwrap()]);
    assert_eq!(r.code, Some(3), "error 级问题应退出 3: {}", r.stdout);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["errors"], 1);

    // dump：可回放指令
    let r = run(&["dump", ok_pptx.to_str().unwrap()]);
    assert_eq!(r.code, Some(0));
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["version"], 1);
    let cmds = v["commands"].as_array().unwrap();
    assert!(cmds.iter().any(|c| c["op"] == "add"));
}

#[test]
fn extract_and_raw() {
    let dir = temp_dir("extract");
    let pptx = make_pptx(&dir, "deck.pptx", 1.0);

    // extract：产物含 template.json + TEMPLATE.md
    let r = run(&[
        "--json",
        "extract",
        pptx.to_str().unwrap(),
        "-o",
        dir.join("tpl").to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert!(dir.join("tpl/template.json").is_file());
    assert!(dir.join("tpl/TEMPLATE.md").is_file());

    // raw：读取字面部件（二进制安全）
    let r = run(&["raw", pptx.to_str().unwrap(), "ppt/presentation.xml"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert!(r.stdout.contains("p:presentation"));

    // raw-set：缺 -o 报错；-o 与输入相同报错；正常路径输入不被改写
    let original = std::fs::read(&pptx).unwrap();
    let part = dir.join("part.xml");
    std::fs::write(&part, "<custom/>").unwrap();

    let r = run(&[
        "raw-set",
        pptx.to_str().unwrap(),
        "customXml/item1.xml",
        "--file",
        part.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(1));

    let r = run(&[
        "raw-set",
        pptx.to_str().unwrap(),
        "customXml/item1.xml",
        "--file",
        part.to_str().unwrap(),
        "-o",
        pptx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(1));

    let out_pptx = dir.join("out.pptx");
    let r = run(&[
        "raw-set",
        pptx.to_str().unwrap(),
        "customXml/item1.xml",
        "--file",
        part.to_str().unwrap(),
        "-o",
        out_pptx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert_eq!(
        std::fs::read(&pptx).unwrap(),
        original,
        "输入文件不得被修改"
    );
    let r = run(&["raw", out_pptx.to_str().unwrap(), "customXml/item1.xml"]);
    assert_eq!(r.stdout, "<custom/>");
}
