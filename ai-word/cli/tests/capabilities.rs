//! 能力契约测试：`help` 声明的属性面必须被实现接受（防文档-实现漂移）；
//! @paraId 寻址在编辑间稳定；batch 语义（默认停 / --force 继续 / 结构化建议）。

use std::path::{Path, PathBuf};
use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_json2docx"))
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
    let dir = std::env::temp_dir().join("json2docx_caps").join(format!(
        "{}_{}",
        name,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 造一个带 w14:paraId 的最小 docx
fn make_docx(dir: &Path) -> PathBuf {
    let docx = dir.join("in.docx");
    let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
 xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml">
<w:body>
<w:p w14:paraId="1A2B3C4D"><w:r><w:t>第一段 {{title}}</w:t></w:r></w:p>
<w:p w14:paraId="56789ABC"><w:r><w:t>第二段</w:t></w:r></w:p>
<w:p><w:r><w:t>第三段</w:t></w:r></w:p>
<w:sectPr/>
</w:body></w:document>"#;
    let ct = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;
    let docrels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#;
    {
        use std::io::Write;
        let f = std::fs::File::create(&docx).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        z.start_file("[Content_Types].xml", opts).unwrap();
        z.write_all(ct.as_bytes()).unwrap();
        z.start_file("_rels/.rels", opts).unwrap();
        z.write_all(rels.as_bytes()).unwrap();
        z.start_file("word/document.xml", opts).unwrap();
        z.write_all(xml.as_bytes()).unwrap();
        z.start_file("word/_rels/document.xml.rels", opts).unwrap();
        z.write_all(docrels.as_bytes()).unwrap();
        z.finish().unwrap();
    }
    docx
}

fn make_product(dir: &Path) -> PathBuf {
    let docx = make_docx(dir);
    let product = dir.join("doc");
    let r = run(&[
        "unpack",
        docx.to_str().unwrap(),
        "-o",
        product.to_str().unwrap(),
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    product
}

#[test]
fn help_lists_elements_and_props() {
    let r = run(&["--json", "help"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let names: Vec<String> = v["elements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["element"].as_str().unwrap().to_string())
        .collect();
    for want in ["part", "paragraph", "heading", "table", "image", "formula"] {
        assert!(names.contains(&want.to_string()), "help 概览缺少 {want}");
    }

    for topic in ["part", "paragraph"] {
        let r = run(&["--json", "help", topic]);
        assert_eq!(r.code, Some(0), "help {topic}: {}", r.stderr);
        let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
        assert!(!v["props"].as_array().unwrap().is_empty());
    }
}

#[test]
fn documented_props_are_accepted_and_unknown_suggest() {
    let dir = temp_dir("props");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // help paragraph 声明的全部属性都能 set（在真实段落上）
    let r = run(&["--json", "help", "paragraph"]);
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
        let r = run(&["edit", ps, "/part[1]/paragraph[3]", "set", "--prop", &prop]);
        assert_eq!(
            r.code,
            Some(0),
            "help paragraph 声明 {prop} 但 set 失败: {}",
            r.stderr
        );
    }

    // 未知属性 → 纠错建议（结构化）
    let r = run(&[
        "--json",
        "edit",
        ps,
        "/part[1]/paragraph[3]",
        "set",
        "--prop",
        "colr=333333",
    ]);
    assert_eq!(r.code, Some(1));
    let v: serde_json::Value = serde_json::from_str(&r.stderr).unwrap();
    let sug = v["error"]["suggestion"].as_str().unwrap_or("");
    assert!(sug.contains("color"), "结构化建议缺少纠错: {v}");

    // help 未知元素 → 建议
    let r = run(&["help", "paragraphs"]);
    assert_eq!(r.code, Some(0), "别名应可解析: {}", r.stderr);
    let r = run(&["help", "paragraphx"]);
    assert_eq!(r.code, Some(1));
    assert!(r.stderr.contains("paragraph"), "{}", r.stderr);
}

#[test]
fn para_id_addressing_survives_edits() {
    let dir = temp_dir("paraid");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // @paraId 定位（unpack 自原始 docx 的稳定 ID）
    let r = run(&[
        "--json",
        "edit",
        ps,
        "/part[1]/paragraph[@paraId=1A2B3C4D]",
        "get",
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["text"], "第一段 {{title}}", "{v}");

    // 删除前面的段落 → 索引漂移，@paraId 不漂移
    let r = run(&["edit", ps, "/part[1]/paragraph[1]", "remove"]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let r = run(&[
        "--json",
        "edit",
        ps,
        "/part[1]/paragraph[@paraId=56789ABC]",
        "get",
    ]);
    assert_eq!(r.code, Some(0), "删除后 @paraId 寻址失败: {}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["text"], "第二段");

    // repack 回写 w14:paraId
    let out = dir.join("out.docx");
    let r = run(&["repack", ps, "-o", out.to_str().unwrap()]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let f = std::fs::File::open(&out).unwrap();
    let mut z = zip::ZipArchive::new(f).unwrap();
    let xml = {
        use std::io::Read;
        let mut s = String::new();
        z.by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut s)
            .unwrap();
        s
    };
    assert!(
        xml.contains(r#"w14:paraId="56789ABC""#),
        "paraId 应回写: {xml}"
    );
}

#[test]
fn batch_semantics() {
    let dir = temp_dir("batch");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // 正常批次
    let bad = dir.join("cmds.json");
    std::fs::write(
        &bad,
        r#"{"commands":[
            {"op":"set","path":"/part[1]/paragraph[3]","props":{"text":"batch改"}},
            {"op":"add","type":"paragraph","path":"/part[1]","props":{"text":"追加"}},
            {"op":"set","path":"/part[1]/paragraph[3]","props":{"colr":"1"}},
            {"op":"set","path":"/part[1]/paragraph[3]","props":{"text":"不会执行"}}
        ]}"#,
    )
    .unwrap();
    let r = run(&["batch", ps, "--input-file", bad.to_str().unwrap(), "--json"]);
    assert_eq!(r.code, Some(1), "有失败应 exit 1: {}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["applied"], 2, "{v}");
    assert_eq!(v["failed"], 1);
    assert_eq!(v["steps"].as_array().unwrap().len(), 3, "默认遇错即停");
    let sug = v["steps"][2]["suggestion"].as_str().unwrap_or("");
    assert!(sug.contains("color"), "未知属性应带建议: {v}");

    // --force 继续
    let r = run(&[
        "batch",
        ps,
        "--input-file",
        bad.to_str().unwrap(),
        "--force",
        "--json",
    ]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["applied"], 3, "--force 应跳过继续: {v}");

    // 未知 op → 建议
    let bad2 = dir.join("bad2.json");
    std::fs::write(
        &bad2,
        r#"{"commands":[{"op":"setx","path":"/part[1]","props":{"x":"1"}}]}"#,
    )
    .unwrap();
    let r = run(&[
        "batch",
        ps,
        "--input-file",
        bad2.to_str().unwrap(),
        "--json",
    ]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let sug = v["steps"][0]["suggestion"].as_str().unwrap_or("");
    assert!(sug.contains("set"), "未知 op 应建议: {v}");
}

#[test]
fn merge_and_serve_smoke() {
    let dir = temp_dir("merge");
    let product = make_product(&dir);
    let ps = product.to_str().unwrap();

    // merge：{{title}} → 数据
    let r = run(&[
        "--json",
        "merge",
        ps,
        "--data",
        r#"{"title":"2026 战略报告"}"#,
    ]);
    assert_eq!(r.code, Some(0), "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["replaced"], 1, "{v}");
    let r = run(&["view", ps, "/part[1]", "text"]);
    assert!(r.stdout.contains("2026 战略报告"), "{}", r.stdout);

    // serve：edit + quit
    let input = format!(
        "{}\n{}\n",
        r#"{"op":"edit","path":"/part[1]/paragraph[1]","action":"get"}"#, r#"{"op":"quit"}"#
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
    assert!(!lines.is_empty(), "serve 应有回复: {text:?}");
    let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(v["ok"], true, "{v}");
}
