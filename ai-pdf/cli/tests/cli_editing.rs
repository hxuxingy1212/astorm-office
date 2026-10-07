//! CLI 契约测试：help 声明的属性面必须被实现接受（防文档-实现漂移），
//! dump→batch 回放保真且幂等，view/edit/validate 的退出码与建议语义。

use std::path::{Path, PathBuf};
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_json2pdf"))
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
    let dir =
        std::env::temp_dir()
            .join("json2pdf_cli")
            .join(format!("{}_{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 手工构造最小产物目录（标准 14 字体，零嵌入，repack 快）
fn make_product(dir: &Path) -> PathBuf {
    let product = dir.join("doc");
    std::fs::create_dir_all(product.join("pages")).unwrap();
    std::fs::write(
        product.join("document.json"),
        r##"{"version":2,"page_size":{"width":612.0,"height":792.0},"pages":["pages/page-001.json","pages/page-002.json"]}"##,
    )
    .unwrap();
    std::fs::write(
        product.join("pages/page-001.json"),
        r##"{"elements":[
            {"type":"text","text":"Hello","x":72,"y":720,"size":24,"font":"Helvetica","color":"#000000"},
            {"type":"rect","x":50,"y":600,"w":200,"h":40,"fill":"#3366cc"}
        ]}"##,
    )
    .unwrap();
    std::fs::write(
        product.join("pages/page-002.json"),
        r##"{"elements":[{"type":"text","text":"Page two","x":72,"y":720,"size":18,"font":"Helvetica","color":"#222222"}]}"##,
    )
    .unwrap();
    product
}

/// 每个元素类型的探测路径（与 capabilities::ELEMENTS 对应）
const TOPICS: &[(&str, &str)] = &[
    ("page", "/page[1]"),
    ("text", "/page[1]/text[1]"),
    ("rect", "/page[1]/rect[1]"),
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
        "/page[1]/text[1]",
        "set",
        "--prop",
        "colour=#ff0000",
    ]);
    assert_eq!(r.code, Some(1));
    let v: serde_json::Value = serde_json::from_str(&r.stderr).unwrap();
    let sug = v["error"]["suggestion"].as_str().unwrap_or("");
    assert!(sug.contains("color"), "结构化建议缺少纠错: {v}");

    // 未知 add 类型 → 合法类型建议
    let r = run(&[
        "edit",
        ps,
        "/page[1]",
        "add",
        "--type",
        "imagee",
        "--prop",
        "src=x.png",
    ]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("image"), "{}", r.stderr);

    // help 未知元素 → 建议 + 退出码 1
    let r = run(&["help", "rectt"]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("rect"), "{}", r.stderr);
}

#[test]
fn edit_add_remove_and_paths() {
    let dir = temp_dir("edit");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // add 回显新元素路径
    let r = run(&[
        "--json",
        "edit",
        ps,
        "/page[1]",
        "add",
        "--type",
        "text",
        "--prop",
        "text=新增",
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["path"], "/page[1]/text[2]", "{v}");

    // get 走新路径
    let r = run(&["--json", "edit", ps, "/page[1]/text[2]", "get"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["text"], "新增", "{v}");

    // group 下钻
    let r = run(&["--json", "edit", ps, "/page[1]", "add", "--type", "group"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let r = run(&[
        "--json",
        "edit",
        ps,
        "/page[1]/group[1]",
        "add",
        "--type",
        "text",
        "--prop",
        "text=组内",
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["path"], "/page[1]/group[1]/text[1]", "{v}");

    // remove 后索引回落
    let r = run(&["edit", ps, "/page[1]/group[1]", "remove"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);

    // add page：/page 为文档容器
    let r = run(&["--json", "edit", ps, "/page", "add", "--type", "page"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["page"], 3, "{v}");
}

#[test]
fn edit_on_pdf_requires_output() {
    let dir = temp_dir("pdfinput");
    let product = make_product(&dir);
    // 先 repack 出一个真实 PDF（标准 14 字体，零嵌入，快）
    let pdf = dir.join("doc.pdf");
    let r = run(&[
        "repack",
        product.to_str().unwrap(),
        "-o",
        pdf.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);

    // 写操作无 -o → 报错
    let r = run(&[
        "edit",
        pdf.to_str().unwrap(),
        "/page[1]/text[1]",
        "set",
        "--prop",
        "text=x",
    ]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("-o"), "{}", r.stderr);

    // 只读 get 不需要 -o
    let r = run(&[
        "--json",
        "edit",
        pdf.to_str().unwrap(),
        "/page[1]/text[1]",
        "get",
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
}

#[test]
fn dump_batch_roundtrip_is_faithful_and_idempotent() {
    let dir = temp_dir("batch");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    let r = run(&["dump", ps]);
    assert_eq!(r.code, Some(0));
    let cmds = dir.join("cmds.json");
    std::fs::write(&cmds, &r.stdout).unwrap();

    // 回放到全新目录（bootstrap）→ dump 与原文档一致（保真闭环）
    let fresh = dir.join("fresh");
    let r = run(&[
        "batch",
        fresh.to_str().unwrap(),
        "--input-file",
        cmds.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(r.code, Some(0), "回放失败: {}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["failed"], 0, "{v}");
    let r = run(&["dump", fresh.to_str().unwrap()]);
    let d1: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let r = run(&["dump", ps]);
    let d2: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(d1, d2, "dump→batch→dump 应逐字节语义一致");

    // 二次回放到同一目录：按页面 id 幂等替换
    let r = run(&[
        "batch",
        fresh.to_str().unwrap(),
        "--input-file",
        cmds.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["applied"], 2, "{v}");
    let replaced: Vec<bool> = v["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["result"]["replaced"].as_bool().unwrap_or(false))
        .collect();
    assert_eq!(replaced, vec![true, true], "二次回放应全部幂等替换: {v}");

    // 未知 op 遇错即停；--force 继续
    let bad = dir.join("bad.json");
    std::fs::write(
        &bad,
        r#"{"commands":[
            {"op":"set","path":"/page[1]/text[1]","props":{"text":"ok"}},
            {"op":"sett","path":"/page[1]","props":{"text":"x"}},
            {"op":"set","path":"/page[1]/text[1]","props":{"text":"after"}}
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
fn validate_exit_codes() {
    let dir = temp_dir("validate");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    let r = run(&["validate", ps, "--json"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["errors"], 0, "{v}");

    // 非法 rotation → error 级 → 退出码 3
    let page1 = dir.join("p1.json");
    std::fs::write(
        &page1,
        r##"{"rotation":45,"elements":[{"type":"text","text":"x","x":10,"y":10,"font":"Helvetica","color":"#000000"}]}"##,
    )
    .unwrap();
    std::fs::copy(&page1, product.join("pages/page-001.json")).unwrap();
    let r = run(&["validate", ps, "--json"]);
    assert_eq!(r.code, Some(3), "error 级问题应退出码 3: {}", r.stdout);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["errors"], 1, "{v}");
}

#[test]
fn view_and_render_html() {
    let dir = temp_dir("view");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    let r = run(&["--json", "view", ps, "/page[1]", "text"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["texts"][0]["path"], "/page[1]/text[1]", "{v}");
    assert_eq!(v["texts"][0]["text"], "Hello");

    let r = run(&["--json", "view", ps, "/page[1]", "layout"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["layout"][0]["type"], "text", "{v}");

    // render html：自包含，不依赖 Chrome
    let html = dir.join("preview.html");
    let r = run(&["render", ps, "-o", html.to_str().unwrap(), "--json"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let content = std::fs::read_to_string(&html).unwrap();
    assert!(content.contains("Hello"), "HTML 应含页面文本");
    assert!(content.contains("@page"), "HTML 应含 @page 打印规则");
}

#[test]
fn serve_smoke() {
    let dir = temp_dir("serve");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();
    let input = format!(
        "{}\n{}\n{}\n",
        r#"{"op":"edit","path":"/page[1]/text[1]","action":"set","prop":["text=Serve"]}"#,
        r#"{"op":"edit","path":"/page[1]/text[1]","action":"get"}"#,
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
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"help","arguments":{"topic":"text"}}}"#
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
    for want in ["batch", "help", "edit", "view", "schema", "dump"] {
        assert!(
            tools.contains(&want.to_string()),
            "tools 缺 {want}: {tools:?}"
        );
    }
    let v: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(v["result"]["isError"], false, "{v}");
    assert!(v["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("text"));
}
