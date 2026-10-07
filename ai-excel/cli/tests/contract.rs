//! 统一 CLI 契约测试（docs/cli-conventions.md）：
//! stdout 只放数据、状态走 stderr、--json 结构化、退出码 0/1/3、raw-set 不覆写输入。

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
    let dir = std::env::temp_dir().join("json2xlsx_contract").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn make_csv(dir: &Path) -> (PathBuf, PathBuf) {
    let csv = dir.join("data.csv");
    std::fs::write(&csv, "name,age\nAlice,30\nBob,25\n").unwrap();
    let xlsx = dir.join("data.xlsx");
    let r = run(&[
        "import",
        csv.to_str().unwrap(),
        "-o",
        xlsx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "import 失败: {}", r.stderr);
    (csv, xlsx)
}

#[test]
fn status_goes_to_stderr_and_json_mode_puts_result_on_stdout() {
    let dir = temp_dir("flags");
    let (_csv, xlsx) = make_csv(&dir);

    // 默认模式：import 不产生 stdout 输出，状态行在 stderr
    let r = run(&[
        "import",
        dir.join("data.csv").to_str().unwrap(),
        "-o",
        dir.join("again.xlsx").to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0));
    assert!(r.stdout.trim().is_empty(), "默认模式 stdout 应为空: {r:?}");
    assert!(
        r.stderr.contains("已导入"),
        "状态行应在 stderr: {}",
        r.stderr
    );

    // --json：unpack 向 stdout 输出结果 JSON
    let out_dir = dir.join("u");
    let r = run(&[
        "--json",
        "unpack",
        xlsx.to_str().unwrap(),
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("stdout 应为 JSON");
    assert_eq!(v["ok"], json_true());
    assert_eq!(v["sheets"], 1);
}

fn json_true() -> serde_json::Value {
    serde_json::Value::Bool(true)
}

#[test]
fn validate_dump_query_and_view_aliases() {
    let dir = temp_dir("commands");
    let (_csv, xlsx) = make_csv(&dir);

    // validate：无问题退出 0
    let r = run(&["validate", xlsx.to_str().unwrap()]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["count"], 0);

    // dump：可回放指令
    let r = run(&["dump", xlsx.to_str().unwrap()]);
    assert_eq!(r.code, Some(0));
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["version"], 1);
    assert!(v["commands"].as_array().unwrap().len() >= 2);

    // query：工作表列表
    let r = run(&["query", xlsx.to_str().unwrap(), "sheets"]);
    assert_eq!(r.code, Some(0));
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["sheets"].as_array().unwrap()[0]["name"], "Sheet1");

    // view 旧模式名兼容
    for mode in ["text", "layout", "values", "structure"] {
        let r = run(&["view", xlsx.to_str().unwrap(), "/sheet[1]", mode]);
        assert_eq!(r.code, Some(0), "view mode {mode}: {}", r.stderr);
    }

    // query：跨表文本查找
    let r = run(&["query", xlsx.to_str().unwrap(), r#"cell:contains("Alice")"#]);
    assert_eq!(r.code, Some(0));
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["matches"], 1);
}

#[test]
fn validate_reports_issues_with_exit_code_3() {
    let dir = temp_dir("issues");
    let (_csv, xlsx) = make_csv(&dir);
    let product = dir.join("p");
    let r = run(&[
        "unpack",
        xlsx.to_str().unwrap(),
        "-o",
        product.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0));

    // 直接修改产物 JSON：引用未定义的命名样式 → 校验问题
    let sheet_path = product.join("xl/worksheets/sheet1.json");
    let mut sheet: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&sheet_path).unwrap()).unwrap();
    sheet["rows"][1]["cells"][0]["style"] = serde_json::json!("nonexistent_style");
    std::fs::write(&sheet_path, serde_json::to_string_pretty(&sheet).unwrap()).unwrap();

    let r = run(&["validate", product.to_str().unwrap()]);
    assert_eq!(r.code, Some(3), "有校验问题应退出 3");
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert!(v["count"].as_u64().unwrap() >= 1);
}

#[test]
fn raw_set_requires_output_and_never_overwrites_input() {
    let dir = temp_dir("rawset");
    let (_csv, xlsx) = make_csv(&dir);
    let original = std::fs::read(&xlsx).unwrap();
    let part_content = dir.join("part.xml");
    std::fs::write(&part_content, "<customXml/>").unwrap();
    let out_xlsx = dir.join("out.xlsx");

    // 缺 -o → 报错
    let r = run(&[
        "raw-set",
        xlsx.to_str().unwrap(),
        "customXml/item1.xml",
        "--file",
        part_content.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("-o"), "{}", r.stderr);

    // -o 与输入相同 → 报错
    let r = run(&[
        "raw-set",
        xlsx.to_str().unwrap(),
        "customXml/item1.xml",
        "--file",
        part_content.to_str().unwrap(),
        "-o",
        xlsx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(1));

    // 正常路径：写新文件，输入不变
    let r = run(&[
        "raw-set",
        xlsx.to_str().unwrap(),
        "customXml/item1.xml",
        "--file",
        part_content.to_str().unwrap(),
        "-o",
        out_xlsx.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    assert_eq!(
        std::fs::read(&xlsx).unwrap(),
        original,
        "输入文件不得被修改"
    );
    // raw 读回验证
    let r = run(&["raw", out_xlsx.to_str().unwrap(), "customXml/item1.xml"]);
    assert_eq!(r.stdout, "<customXml/>");
}
