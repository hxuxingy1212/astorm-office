//! 能力契约测试：`help` 声明的属性面必须被实现接受（防文档-实现漂移），
//! 且错误必须携带 suggestion；batch 的 dump→回放必须保真。

use std::path::{Path, PathBuf};
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_json2xlsx"))
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
    let dir = std::env::temp_dir().join("json2xlsx_caps").join(format!(
        "{}_{}",
        name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 造一个含数据的工作簿（1 张表；A 列文本，B 列数字）
fn make_xlsx(dir: &Path) -> PathBuf {
    let csv = dir.join("data.csv");
    std::fs::write(&csv, "name,age\nAlice,30\nBob,25\n").unwrap();
    let xlsx = dir.join("data.xlsx");
    let r = run(&[
        "import",
        csv.to_str().unwrap(),
        "-o",
        xlsx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    xlsx
}

fn make_product(dir: &Path) -> PathBuf {
    let xlsx = make_xlsx(dir);
    let product = dir.join("p");
    let r = run(&[
        "unpack",
        xlsx.to_str().unwrap(),
        "-o",
        product.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    product
}

/// 每个 help 主题 → 探测路径
const TOPICS: &[(&str, &str)] = &[
    ("sheet", "/sheet[1]"),
    ("cell", "/sheet[1]/cell[C3]"),
    ("row", "/sheet[1]/row[2]"),
    ("col", "/sheet[1]/col[D]"),
    ("range", "/sheet[1]/range[A1:C1]"),
];

#[test]
fn help_lists_documented_props_for_every_element() {
    // 概览：元素齐全
    let r = run(&["--json", "help"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let names: Vec<String> = v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["element"].as_str().unwrap().to_string())
        .collect();
    for (t, _) in TOPICS {
        assert!(names.contains(&t.to_string()), "help 概览缺少元素 {t}");
    }

    // 单元素：props 非空，且含 name/aliases/example/note
    for (t, path) in TOPICS {
        let r = run(&["--json", "help", t]);
        assert_eq!(r.code, Some(0), "help {t}: {}", r.stderr);
        let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
        assert_eq!(v["element"], *t);
        assert!(
            !v["path"].as_str().unwrap_or("").is_empty(),
            "help {t} 缺少示例路径"
        );
        let _ = path; // 探测路径见 every_documented_prop_is_accepted_by_set
        let props = v["props"].as_array().unwrap();
        assert!(!props.is_empty(), "help {t} 属性为空");
        for p in props {
            for key in ["name", "aliases", "example", "note"] {
                assert!(!p[key].is_null(), "help {t} 属性缺 {key}: {p}");
            }
        }
    }
}

#[test]
fn every_documented_prop_is_accepted_by_set() {
    let dir = temp_dir("props");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    for (topic, path) in TOPICS {
        let r = run(&["--json", "help", topic]);
        let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
        for p in v["props"].as_array().unwrap() {
            let name = p["name"].as_str().unwrap();
            // 概览里 example 形如 "--prop name=sample"，取出 sample
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
fn unknown_prop_suggests_nearest_and_valid_values() {
    let dir = temp_dir("suggest");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // 属性名拼错 → 建议最近匹配 + 合法值列表
    let r = run(&[
        "edit",
        ps,
        "/sheet[1]/cell[A1]",
        "set",
        "--prop",
        "bould=true",
    ]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("是否想用"), "缺少纠错建议: {}", r.stderr);
    assert!(r.stderr.contains("bold"), "{}", r.stderr);
    assert!(
        r.stderr.contains("可用值") || r.stderr.contains("可选值"),
        "缺少合法取值: {}",
        r.stderr
    );

    // --json 模式下建议进入结构化 error.suggestion
    let r = run(&[
        "--json",
        "edit",
        ps,
        "/sheet[1]/cell[A1]",
        "set",
        "--prop",
        "bould=true",
    ]);
    let v: serde_json::Value = serde_json::from_str(&r.stderr).unwrap();
    let sug = v["error"]["suggestion"].as_str().unwrap_or("");
    assert!(sug.contains("bold"), "结构化建议缺少纠错: {v}");

    // 未知 add 类型 → 建议合法类型
    let r = run(&[
        "edit",
        ps,
        "/sheet[1]",
        "add",
        "--type",
        "chartt",
        "--prop",
        "x=1",
    ]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("chart"), "{}", r.stderr);

    // help 未知元素 → 建议
    let r = run(&["help", "cel"]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("cell"), "{}", r.stderr);
}

#[test]
fn batch_replays_dump_and_stops_or_continues_as_flagged() {
    let dir = temp_dir("batch");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // dump → 回放：语义保真（忽略 raw/snapshot）
    let r = run(&["dump", ps]);
    assert_eq!(r.code, Some(0));
    let cmds = dir.join("cmds.json");
    std::fs::write(&cmds, &r.stdout).unwrap();

    let out = dir.join("replayed");
    let r = run(&[
        "batch",
        ps,
        "--input-file",
        cmds.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(r.code, Some(0), "回放失败: {}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["failed"], 0, "{v}");
    assert!(v["applied"].as_u64().unwrap() >= 2);

    let out_prod = dir.join("replayed_u");
    let r = run(&[
        "unpack",
        out.to_str().unwrap(),
        "-o",
        out_prod.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let a = norm(&read_json(&product.join("workbook.json")));
    let b = norm(&read_json(&out_prod.join("workbook.json")));
    assert_eq!(a, b, "回放后 workbook 语义应一致");
    let a = norm(&read_json(&product.join("xl/worksheets/sheet1.json")));
    let b = norm(&read_json(&out_prod.join("xl/worksheets/sheet1.json")));
    assert_eq!(a, b, "回放后工作表内容应一致");

    // 默认遇错即停；--force 跳过后继续，最终 exit 1
    let bad = dir.join("bad.json");
    std::fs::write(
        &bad,
        r#"{"commands":[
            {"op":"set","path":"/sheet[1]/cell[A1]","props":{"value":"ok"}},
            {"op":"set","path":"/sheet[1]/cell[A1]","props":{"nope":"1"}},
            {"op":"set","path":"/sheet[1]/cell[A2]","props":{"value":"after"}}
        ]}"#,
    )
    .unwrap();
    let out2 = dir.join("out2.xlsx");
    let r = run(&[
        "batch",
        ps,
        "--input-file",
        bad.to_str().unwrap(),
        "-o",
        out2.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(r.code, Some(1), "有用例失败应 exit 1");
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["applied"], 1);
    assert_eq!(v["failed"], 1);
    assert_eq!(v["steps"].as_array().unwrap().len(), 2, "默认应遇错即停");

    let r = run(&[
        "batch",
        ps,
        "--input-file",
        bad.to_str().unwrap(),
        "-o",
        dir.join("out3.xlsx").to_str().unwrap(),
        "--force",
        "--json",
    ]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["applied"], 2, "--force 应跳过错误继续: {v}");
    assert_eq!(v["failed"], 1);
    assert_eq!(v["steps"].as_array().unwrap().len(), 3);

    // 未知 op → 结构化建议
    let bad2 = dir.join("bad2.json");
    std::fs::write(
        &bad2,
        r#"{"commands":[{"op":"sett","path":"/sheet[1]","props":{"name":"x"}}]}"#,
    )
    .unwrap();
    let r = run(&[
        "--json",
        "batch",
        ps,
        "--input-file",
        bad2.to_str().unwrap(),
        "-o",
        dir.join("out4.xlsx").to_str().unwrap(),
    ]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let s = v["steps"][0]["suggestion"].as_str().unwrap_or("");
    assert!(s.contains("set"), "未知 op 应建议可用 op: {v}");
}

#[test]
fn merge_fills_placeholders_in_product_dir() {
    let dir = temp_dir("merge");
    let product = make_product(&dir);
    // 在 A1 写入占位符，再用 merge 数据填充
    let r = run(&[
        "edit",
        product.to_str().unwrap(),
        "/sheet[1]/cell[A1]",
        "set",
        "--prop",
        "value={{company}}",
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let r = run(&[
        "--json",
        "merge",
        product.to_str().unwrap(),
        "--data",
        r#"{"company":"ACME"}"#,
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["replacements"], 1, "{v}");
    let r = run(&["view", product.to_str().unwrap(), "/sheet[1]", "text"]);
    assert!(r.stdout.contains("ACME"), "{}", r.stdout);
}

#[test]
fn render_png_or_suggests_html_fallback() {
    let dir = temp_dir("render");
    let product = make_product(&dir);
    let png = dir.join("preview.png");
    let r = run(&[
        "render",
        product.to_str().unwrap(),
        "--format",
        "png",
        "-o",
        png.to_str().unwrap(),
    ]);
    if r.code == Some(0) {
        let bytes = std::fs::read(&png).expect("PNG 未生成");
        assert_eq!(&bytes[..4], b"\x89PNG", "输出不是 PNG");
    } else {
        // 无 Chrome 环境：必须给出可执行的回退建议
        assert!(
            r.stderr.contains("--format html"),
            "无头浏览器缺失时应建议 html 回退: {}",
            r.stderr
        );
    }
}

#[test]
fn serve_and_mcp_smoke() {
    let dir = temp_dir("serve");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // serve：一次 edit + get + quit
    let input = format!(
        "{}\n{}\n{}\n",
        r#"{"op":"edit","path":"/sheet[1]/cell[A1]","action":"set","prop":["value=Serve"]}"#,
        r#"{"op":"edit","path":"/sheet[1]/cell[A1]","action":"get"}"#,
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
    let text = out.stdout_text();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "serve 应逐行回复: {lines:?}");
    let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(v["ok"], true, "{v}");
    let v: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(v["result"]["value"], "Serve");

    // mcp：initialize + tools/list + tools/call(help)
    let input = format!(
        "{}\n{}\n{}\n",
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"help","arguments":{"topic":"cell"}}}"#
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
    let text = out.stdout_text();
    let lines: Vec<&str> = text.lines().collect();
    let v: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    let tools: Vec<String> = v["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    for want in ["batch", "help", "merge", "render", "schema"] {
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
        .contains("bold"));
}

// ---- 工具 ----

trait OutText {
    fn stdout_text(&self) -> String;
}
impl OutText for std::process::Output {
    fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).to_string()
    }
}

fn read_json(p: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// 忽略 raw/snapshot/raw_* 的递归归一化（这些是保真用的原始 XML 片段）
fn norm(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(m) => serde_json::Value::Object(
            m.iter()
                .filter(|(k, _)| {
                    k.as_str() != "raw" && k.as_str() != "snapshot" && !k.starts_with("raw_")
                })
                .map(|(k, v)| (k.clone(), norm(v)))
                .collect(),
        ),
        serde_json::Value::Array(a) => serde_json::Value::Array(a.iter().map(norm).collect()),
        other => other.clone(),
    }
}
