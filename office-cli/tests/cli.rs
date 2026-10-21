//! 集成测试：用**桩 CLI**（shell 脚本）替代四个真实二进制，验证
//! `astorm-office` 的分发、参数转义、退出码透传、批处理与控制流。
//!
//! 为什么要桩：本测试只关心「统一入口」的逻辑（选择哪个二进制、传哪些参数、
//! 失败怎么处置），真实文档能力已在各自 CLI 的测试里覆盖。桩还让「子进程
//! 退出码 3」「子进程不存在」这类边界可复现。

#![cfg(unix)]

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_astorm-office")
}

/// 独立的临时目录（每个测试一个，避免并行互踩）
fn tmpdir(tag: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!(
        "astorm-office-cli-{}-{}-{}",
        std::process::id(),
        tag,
        n
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// 桩 CLI 集合：四个脚本，把调用追加进 $STUB_LOG，按 $STUB_EXIT 退出
struct Stubs {
    dir: PathBuf,
    log: PathBuf,
}

impl Stubs {
    fn new(tag: &str) -> Self {
        let dir = tmpdir(tag);
        let log = dir.join("calls.log");
        for name in ["json2docx", "json2xlsx", "json2pptx", "json2pdf"] {
            let p = dir.join(name);
            let mut f = std::fs::File::create(&p).unwrap();
            // 逐参数换行写入，便于断言「参数原样转发」
            writeln!(
                f,
                "#!/bin/sh\nprintf '%s\\n' \"=== {name}\" >> \"$STUB_LOG\"\nfor a in \"$@\"; do printf 'ARG %s\\n' \"$a\" >> \"$STUB_LOG\"; done\necho \"OUT {name} $*\"\necho \"ERR {name}\" >&2\nexit ${{STUB_EXIT:-0}}"
            )
            .unwrap();
            drop(f);
            let mut perm = std::fs::metadata(&p).unwrap().permissions();
            perm.set_mode(0o755);
            std::fs::set_permissions(&p, perm).unwrap();
        }
        Stubs { dir, log }
    }

    /// 用桩环境跑一次 astorm-office
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        self.run_with(args, "0")
    }

    fn run_with(&self, args: &[&str], exit: &str) -> (i32, String, String) {
        let out = Command::new(bin())
            .args(args)
            .env("ASTORM_JSON2DOCX", self.dir.join("json2docx"))
            .env("ASTORM_JSON2XLSX", self.dir.join("json2xlsx"))
            .env("ASTORM_JSON2PPTX", self.dir.join("json2pptx"))
            .env("ASTORM_JSON2PDF", self.dir.join("json2pdf"))
            .env("STUB_LOG", &self.log)
            .env("STUB_EXIT", exit)
            .output()
            .expect("spawn astorm-office");
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
        )
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }
}

/// 断言桩调用日志里出现「某二进制被以下参数调用」
fn assert_called(log: &str, bin: &str, args: &[&str]) {
    let mut expect = format!("=== {bin}\n");
    for a in args {
        expect.push_str(&format!("ARG {a}\n"));
    }
    assert!(
        log.contains(&expect),
        "期望调用:\n{expect}\n实际日志:\n{log}"
    );
}

#[test]
fn formats_lists_aliases_and_resolved_paths() {
    let s = Stubs::new("formats");
    let (code, out, _err) = s.run(&["formats", "--json"]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let rows = v["formats"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    let word = rows.iter().find(|r| r["format"] == "word").unwrap();
    assert!(word["aliases"].as_str().unwrap().contains("docs"));
    assert_eq!(word["bin"], "json2docx");
    assert_eq!(word["env"], "ASTORM_JSON2DOCX");
    assert!(word["ok"].as_bool().unwrap());
    assert_eq!(word["source"], "env");
    assert!(word["path"].as_str().unwrap().ends_with("json2docx"));
}

#[test]
fn passthrough_forwards_args_verbatim_and_propagates_exit_code() {
    let s = Stubs::new("passthrough");
    let (code, out, err) = s.run(&["word", "unpack", "thesis.docx", "-o", "prod/", "--json"]);
    assert_eq!(code, 0);
    // stdout/stderr 原样透传（子命令的 --json 属于子命令）
    assert!(out.contains("OUT json2docx"), "{out}");
    assert!(err.contains("ERR json2docx"), "{err}");
    assert_called(
        &s.calls(),
        "json2docx",
        &["unpack", "thesis.docx", "-o", "prod/", "--json"],
    );

    // 退出码透传（3 = 产物有校验问题，调度层不得吞掉或改写）
    let (code, _out, _err) = s.run_with(&["pdf", "validate", "a.pdf"], "3");
    assert_eq!(code, 3);
}

#[test]
fn alias_token_picks_the_right_binary() {
    let s = Stubs::new("aliases");
    for (token, b) in [
        ("ppt", "json2pptx"),
        ("pptx", "json2pptx"),
        ("word", "json2docx"),
        ("docs", "json2docx"),
        ("excel", "json2xlsx"),
        ("sheet", "json2xlsx"),
        ("pdf", "json2pdf"),
    ] {
        let (code, _o, _e) = s.run(&[token, "templates"]);
        assert_eq!(code, 0, "格式 {token}");
        assert!(s.calls().contains(&format!("=== {b}")), "{token} → {b}");
    }
}

#[test]
fn format_can_be_inferred_from_arguments() {
    let s = Stubs::new("infer");
    // 输入文件扩展名
    let (code, _o, _e) = s.run(&["unpack", "book.xlsx", "-o", "prod"]);
    assert_eq!(code, 0);
    assert_called(
        &s.calls(),
        "json2xlsx",
        &["unpack", "book.xlsx", "-o", "prod"],
    );

    // 只有 -o 的输出扩展名
    std::fs::remove_file(&s.log).unwrap();
    let (code, _o, _e) = s.run(&["repack", "prod/", "-o", "out.pptx"]);
    assert_eq!(code, 0);
    assert_called(
        &s.calls(),
        "json2pptx",
        &["repack", "prod/", "-o", "out.pptx"],
    );

    // 产物目录以格式命名（本仓库 samples/ 的布局）
    std::fs::remove_file(&s.log).unwrap();
    let (code, _o, _e) = s.run(&["view", "samples/docx", "/", "text"]);
    assert_eq!(code, 0);
    assert_called(
        &s.calls(),
        "json2docx",
        &["view", "samples/docx", "/", "text"],
    );

    // 推断不出来时给出用法建议
    let (code, _o, err) = s.run(&["formats"]); // 已被本 CLI 接管
    assert_eq!(code, 0, "{err}");
    let (code, _o, err) = s.run(&["merge", "prod/"]);
    assert_eq!(code, 1);
    assert!(err.contains("无法从参数推断格式"), "{err}");
    assert!(err.contains("建议"), "{err}");
}

#[test]
fn explicit_format_flag_and_conflict() {
    let s = Stubs::new("flag");
    let (code, _o, _e) = s.run(&["-f", "ppt", "view", "deck.pptx", "/", "text"]);
    assert_eq!(code, 0);
    assert_called(&s.calls(), "json2pptx", &["view", "deck.pptx", "/", "text"]);

    // 位置格式与 --format 冲突：报错而不是猜
    let (code, _o, err) = s.run(&["-f", "pdf", "word", "view", "a.docx", "/", "text"]);
    assert_eq!(code, 1);
    assert!(err.contains("格式冲突"), "{err}");
}

#[test]
fn unknown_binary_reports_actionable_error() {
    let out = Command::new(bin())
        .args(["word", "templates"])
        .env("ASTORM_JSON2DOCX", "/nonexistent/json2docx")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("ASTORM_JSON2DOCX"), "{err}");
    assert!(err.contains("不存在"), "{err}");
}

#[test]
fn batch_runs_text_script_across_formats() {
    let s = Stubs::new("batch_text");
    let script = s.dir.join("steps.txt");
    std::fs::write(
        &script,
        "# 注释行\nword unpack a.docx -o prod/\n\nexcel import d.csv -o b.xlsx\npdf qa a.pdf\n",
    )
    .unwrap();
    let (code, out, err) = s.run(&["batch", script.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    // 默认模式：子命令输出直通 stdout，汇总走 stderr
    assert!(out.contains("OUT json2docx"), "{out}");
    assert!(out.contains("OUT json2xlsx"), "{out}");
    assert!(out.contains("OUT json2pdf"), "{out}");
    assert!(err.contains("批处理"), "{err}");
    assert!(err.contains("成功 3"), "{err}");

    let log = s.calls();
    assert_called(&log, "json2docx", &["unpack", "a.docx", "-o", "prod/"]);
    assert_called(&log, "json2xlsx", &["import", "d.csv", "-o", "b.xlsx"]);
    assert_called(&log, "json2pdf", &["qa", "a.pdf"]);
}

#[test]
fn batch_json_script_and_json_report() {
    let s = Stubs::new("batch_json");
    // 内联 JSON：一步显式格式、一步靠推断、一步字符串形式
    let commands = r#"{"version":1,"commands":[
        {"format":"word","args":["view","prod/","/","text"]},
        {"args":["extract","text","r.pdf","--format","markdown"]},
        "ppt view deck.pptx / layout"
    ]}"#;
    let (code, out, err) = s.run(&["batch", "--commands", commands, "--json"]);
    assert_eq!(code, 0, "{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["total"], 3);
    assert_eq!(v["succeeded"], 3);
    assert_eq!(v["failed"], 0);
    let steps = v["steps"].as_array().unwrap();
    assert_eq!(steps[0]["format"], "word");
    assert_eq!(steps[1]["format"], "pdf");
    assert_eq!(steps[2]["format"], "ppt");
    assert!(steps[0]["stdout"]
        .as_str()
        .unwrap()
        .contains("OUT json2docx"));
    // 子进程 stderr 被**捕获进报告**（不直通到 stderr）
    assert!(steps[0]["stderr"]
        .as_str()
        .unwrap()
        .contains("ERR json2docx"));
    assert!(!err.contains("ERR "), "stderr 不应直通: {err}");
    assert_eq!(steps[0]["source"], "env");
    assert_called(
        &s.calls(),
        "json2pdf",
        &["extract", "text", "r.pdf", "--format", "markdown"],
    );
}

#[test]
fn batch_stops_on_failure_unless_forced() {
    let s = Stubs::new("batch_fail");
    let script = s.dir.join("steps.txt");
    std::fs::write(&script, "word a.docx\nword b.docx\nexcel c.xlsx\n").unwrap();

    // 全部成功：退出码 0
    let (code, _o, _err) = s.run_with(&["batch", script.to_str().unwrap()], "0");
    assert_eq!(code, 0);

    // STUB_EXIT 对所有桩生效 → 第 1 步即失败，后续不执行
    std::fs::remove_file(&s.log).unwrap();
    let (code, _o, err) = s.run_with(&["batch", script.to_str().unwrap()], "1");
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("停在第 1 步"), "{err}");
    let log = s.calls();
    assert!(log.contains("=== json2docx"), "{log}");
    assert!(
        !log.contains("=== json2xlsx"),
        "失败即停，不应执行第 3 步：{log}"
    );

    // --force：继续跑完，报告里失败计数为真
    std::fs::remove_file(&s.log).unwrap();
    let (code, out, _err) = s.run_with(
        &["batch", script.to_str().unwrap(), "--force", "--json"],
        "1",
    );
    assert_eq!(code, 1);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["failed"], 3);
    assert!(v["stopped_at"].is_null(), "{v}");
    let log = s.calls();
    assert!(
        log.contains("=== json2xlsx"),
        "force 应继续执行第 3 步：{log}"
    );
}

#[test]
fn batch_dry_run_executes_nothing() {
    let s = Stubs::new("batch_dry");
    let script = s.dir.join("steps.txt");
    std::fs::write(&script, "word unpack a.docx\npdf qa a.pdf\n").unwrap();
    let (code, out, err) = s.run(&["batch", script.to_str().unwrap(), "--dry-run", "--json"]);
    assert_eq!(code, 0);
    assert!(err.is_empty() || !err.contains("ERR"), "{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["dry_run"], true);
    assert_eq!(v["total"], 2);
    assert!(v["steps"][0]["dry_run"].as_bool().unwrap());
    assert!(v["steps"][0]["exit_code"].is_null());
    assert!(s.calls().is_empty(), "空跑不应调用任何 CLI：{}", s.calls());
}

#[test]
fn batch_helpful_errors() {
    let s = Stubs::new("batch_err");
    // 缺少 args
    let (code, _o, err) = s.run(&["batch", "--commands", r#"[{"format":"word"}]"#]);
    assert_eq!(code, 1);
    assert!(err.contains("args"), "{err}");

    // 未知格式
    let (code, _o, err) = s.run(&["batch", "--commands", r#"[{"format":"nope","args":["x"]}]"#]);
    assert_eq!(code, 1);
    assert!(err.contains("未知格式"), "{err}");

    // 脚本文件不存在
    let (code, _o, err) = s.run(&["batch", "/nonexistent/steps.txt"]);
    assert_eq!(code, 1);
    assert!(err.contains("脚本文件不存在"), "{err}");

    // 空脚本
    let p = s.dir.join("empty.txt");
    std::fs::write(&p, "# 只有注释\n").unwrap();
    let (code, _o, err) = s.run(&["batch", p.to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(err.contains("没有可执行的命令"), "{err}");
}

#[test]
fn batch_reads_script_from_stdin() {
    let s = Stubs::new("batch_stdin");
    let mut child = Command::new(bin())
        .args(["batch", "-", "--json"])
        .env("ASTORM_JSON2PPTX", s.dir.join("json2pptx"))
        .env("STUB_LOG", &s.log)
        .env("STUB_EXIT", "0")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"ppt view deck.pptx / text\n")
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_called(&s.calls(), "json2pptx", &["view", "deck.pptx", "/", "text"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["succeeded"], 1);
}

#[test]
fn help_mentions_the_four_spoken_formats() {
    let out = Command::new(bin()).arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let out = String::from_utf8_lossy(&out.stdout).to_string();
    for t in ["ppt", "word", "docs", "pdf", "excel", "batch", "formats"] {
        assert!(out.contains(t), "--help 应提到 {t}：{out}");
    }
    // 无参数：clap 的 arg_required_else_help
    let out = Command::new(bin()).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(!out.stdout.is_empty() || !out.stderr.is_empty());
}

/// 桩目录里的日志文件路径在断言里要用到（保持 Stubs 结构可用性）
#[allow(dead_code)]
fn _unused(p: &Path) -> bool {
    p.exists()
}
