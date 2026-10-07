//! 能力契约测试：`help` 声明的属性面必须被实现接受（防文档-实现漂移），
//! batch 的 dump→回放必须保真，@id 寻址在多步编辑间稳定。

use std::path::{Path, PathBuf};
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_json2pptx"))
}

#[derive(Debug)]
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
    let dir = std::env::temp_dir().join("json2pptx_caps").join(format!(
        "{}_{}",
        name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 用 JSON 生成最小演示文稿并 unpack 到产物目录
fn make_product(dir: &Path) -> PathBuf {
    let pres = dir.join("pres.json");
    std::fs::write(
        &pres,
        r#"{"version":1,"slides":[
            {"elements":[
                {"type":"text","text":"标题 {{title}}","x":1,"y":1,"w":8,"h":1,"name":"Title"},
                {"type":"shape","shape_type":"rect","x":1,"y":3,"w":4,"h":2,"text":"box","name":"Box1"}
            ]},
            {"elements":[{"type":"text","text":"第二页","x":1,"y":1,"w":8,"h":1}]}
        ]}"#,
    )
    .unwrap();
    // JSON 生成：用 generate 端（repack 需要产物目录；这里走 build 一个空模板再 unpack 不方便，
    // 直接用 web-server 的生成入口也不方便 —— 改用「unpack 模板 pptx」不可控。
    // 实际做法：先写 presentation.json + slides 到产物目录，走 edit/view 通道。
    let product = dir.join("deck");
    std::fs::create_dir_all(product.join("ppt/slides")).unwrap();
    std::fs::write(
        product.join("presentation.json"),
        r#"{"version":1,"width":13.333,"height":7.5,"slides":["ppt/slides/slide1.json","ppt/slides/slide2.json"]}"#,
    )
    .unwrap();
    let slide1 = dir.join("slide1.json");
    let slide2 = dir.join("slide2.json");
    std::fs::write(
        &slide1,
        r#"{"elements":[{"type":"text","text":"标题 {{title}}","position":{"x":1,"y":1,"w":8,"h":1},"name":"Title","id":2},{"type":"shape","shape_type":"rect","position":{"x":1,"y":3,"w":4,"h":2},"text":"box","name":"Box1","id":3}]}"#,
    )
    .unwrap();
    std::fs::write(
        &slide2,
        r#"{"elements":[{"type":"text","text":"第二页","position":{"x":1,"y":1,"w":8,"h":1},"id":2}]}"#,
    )
    .unwrap();
    std::fs::rename(&slide1, product.join("ppt/slides/slide1.json")).unwrap();
    std::fs::rename(&slide2, product.join("ppt/slides/slide2.json")).unwrap();
    product
}

/// 每个元素类型的探测路径（与 capabilities::ELEMENTS 对应）
const TOPICS: &[(&str, &str)] = &[
    ("slide", "/slide[1]"),
    ("text", "/slide[1]/text[1]"),
    ("shape", "/slide[1]/shape[1]"),
];

/// help --json 声明的属性 → set 必须成功（仅验证声明的适用元素）
#[test]
fn every_documented_prop_is_accepted_by_set() {
    let dir = temp_dir("props");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    for (topic, path) in TOPICS {
        let r = run(&["--json", "help", topic]);
        assert_eq!(r.code, Some(0), "help {topic}: {}", r.stderr);
        let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
        for p in v["props"].as_array().unwrap() {
            let name = p["name"].as_str().unwrap();
            let sample = p["example"]
                .as_str()
                .unwrap()
                .split_once('=')
                .map(|(_, s)| s)
                .unwrap();
            let prop = format!("{name}={sample}");
            let r = run(&["edit", ps, path, "set", "--prop", &prop]);
            assert_eq!(
                r.code,
                Some(0),
                "help {topic} 声明 {prop} 但 set 失败: {}",
                r.stderr
            );
        }
    }
}

#[test]
fn unknown_prop_and_type_suggest_fixes() {
    let dir = temp_dir("suggest");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // 未知属性 → 纠错建议（--json 下在 stderr 的 error.suggestion）
    let r = run(&[
        "--json",
        "edit",
        ps,
        "/slide[1]/text[1]",
        "set",
        "--prop",
        "colour=FF0000",
    ]);
    assert_eq!(r.code, Some(1));
    let v: serde_json::Value = serde_json::from_str(&r.stderr).unwrap();
    let sug = v["error"]["suggestion"].as_str().unwrap_or("");
    assert!(sug.contains("color"), "结构化建议缺少纠错: {v}");

    // 未知 add 类型 → 合法类型建议
    let r = run(&[
        "edit",
        ps,
        "/slide[1]",
        "add",
        "--type",
        "shapee",
        "--prop",
        "x=1",
    ]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("shape"), "{}", r.stderr);

    // help 未知元素 → 建议
    let r = run(&["help", "shapee"]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("shape"), "{}", r.stderr);
}

#[test]
fn stable_id_addressing_survives_edits() {
    let dir = temp_dir("stability");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // add 返回稳定 id
    let r = run(&[
        "--json",
        "edit",
        ps,
        "/slide[1]",
        "add",
        "--type",
        "text",
        "--prop",
        "text=新增",
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let id = v["id"].as_u64().expect("add 应回显稳定 id");

    // 删除前面的元素后，@id 寻址仍指向同一元素（索引会漂移，id 不漂移）
    let r = run(&["edit", ps, "/slide[1]/text[1]", "remove"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let r = run(&[
        "--json",
        "edit",
        ps,
        &format!("/slide[1]/text[@id={id}]"),
        "get",
    ]);
    assert_eq!(r.code, Some(0), "@id 寻址失败: {}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["text"], "新增", "@id 指向的元素应不变: {v}");
}

#[test]
fn batch_replays_dump_faithfully() {
    let dir = temp_dir("batch");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    for slide in ["1", "2"] {
        let r = run(&["view", ps, &format!("/slide[{slide}]"), "layout"]);
        assert_eq!(r.code, Some(0), "{}", r.stderr);
        let f = dir.join(format!("before{slide}.json"));
        std::fs::write(&f, &r.stdout).unwrap();
    }

    // dump → 回放（产物目录原地），视图不变
    let r = run(&["dump", ps]);
    assert_eq!(r.code, Some(0));
    let cmds = dir.join("cmds.json");
    std::fs::write(&cmds, &r.stdout).unwrap();
    let r = run(&[
        "batch",
        ps,
        "--input-file",
        cmds.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(r.code, Some(0), "回放失败: {}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["failed"], 0, "{v}");
    assert!(v["applied"].as_u64().unwrap() >= 3);

    for slide in ["1", "2"] {
        let r = run(&["view", ps, &format!("/slide[{slide}]"), "layout"]);
        let after = dir.join(format!("after{slide}.json"));
        std::fs::write(&after, &r.stdout).unwrap();
        let before: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join(format!("before{slide}.json"))).unwrap(),
        )
        .unwrap();
        let after: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&after).unwrap()).unwrap();
        assert_eq!(before, after, "slide{slide} 回放后视图应一致");
    }

    // 默认遇错即停；--force 继续；未知 op 带建议
    let bad = dir.join("bad.json");
    std::fs::write(
        &bad,
        r#"{"commands":[
            {"op":"set","path":"/slide[1]/text[1]","props":{"text":"ok"}},
            {"op":"sett","path":"/slide[1]","props":{"text":"x"}},
            {"op":"set","path":"/slide[1]/text[1]","props":{"text":"after"}}
        ]}"#,
    )
    .unwrap();
    let r = run(&["batch", ps, "--input-file", bad.to_str().unwrap(), "--json"]);
    assert_eq!(r.code, Some(1));
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["applied"], 1);
    assert_eq!(v["failed"], 1);
    let sug = v["steps"][1]["suggestion"].as_str().unwrap_or("");
    assert!(sug.contains("set"), "未知 op 应建议可用 op: {v}");

    let r = run(&[
        "batch",
        ps,
        "--input-file",
        bad.to_str().unwrap(),
        "--force",
        "--json",
    ]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["applied"], 2, "--force 应继续: {v}");
}

#[test]
fn merge_fills_placeholders() {
    let dir = temp_dir("merge");
    let product = make_product(&dir);
    let r = run(&[
        "--json",
        "merge",
        product.to_str().unwrap(),
        "--data",
        r#"{"title":"2026 战略"}"#,
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["replacements"], 1, "{v}");
    let r = run(&["view", product.to_str().unwrap(), "/slide[1]", "text"]);
    assert!(r.stdout.contains("2026 战略"), "{}", r.stdout);
}

#[test]
fn serve_smoke() {
    let dir = temp_dir("serve");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();
    let input = format!(
        "{}\n{}\n{}\n",
        r#"{"op":"edit","path":"/slide[1]/text[1]","action":"set","prop":["text=Serve"]}"#,
        r#"{"op":"edit","path":"/slide[1]/text[1]","action":"get"}"#,
        r#"{"op":"quit"}"#
    );
    let mut child = cli()
        .args(["serve", ps])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "serve 应逐行回复: {text:?}");
    let v: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(v["result"]["text"], "Serve", "{v}");
}

#[test]
fn mcp_smoke() {
    let input = format!(
        "{}\n{}\n{}\n",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"help","arguments":{"topic":"shape"}}}"#
    );
    let mut child = cli()
        .args(["mcp"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let lines: Vec<&str> = text.lines().collect();
    let v: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    let tools: Vec<String> = v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for want in ["batch", "help", "merge", "render", "schema", "serve"] {
        assert!(
            tools.contains(&want.to_string()) || want == "serve",
            "tools 缺 {want}: {tools:?}"
        );
    }
    let v: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(v["result"]["isError"], false, "{v}");
    assert!(v["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("shape"));
}
