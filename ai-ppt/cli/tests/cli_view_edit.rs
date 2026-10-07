//! view / edit CLI 集成测试
//!
//! 无状态语义：view/edit/render 输入可以是 PPTX 文件（一次性解包到临时目录）
//! 或 unpack 产物目录（直接读写）。edit 对 .pptx 的写操作必须 -o 输出新文件。

mod common;

use std::path::PathBuf;
use std::process::Command;

fn cli() -> Command {
    // CARGO_BIN_EXE_* 由 cargo 注入，指向本包刚构建的二进制（无需预 build）
    Command::new(env!("CARGO_BIN_EXE_json2pptx"))
}

/// 运行 CLI 命令，返回 (exit_success, stdout, stderr)
fn run(args: &[&str]) -> (bool, String, String) {
    let out = cli().args(args).output().expect("CLI 执行失败");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("json2pptx_cli_test").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 构造含图片+表格+嵌套 group 的测试 PPTX
fn build_test_pptx() -> PathBuf {
    let img = common::create_dummy_image();
    let pptx = common::temp_pptx_path();
    let pres = json2pptx::model::Presentation {
        table_styles: None,
        width: 13.333,
        height: 7.5,
        meta: Some(json2pptx::model::Meta {
            company: None,
            application: None,
            last_modified_by: None,
            title: Some("CLI 测试".to_string()),
            author: Some("json2pptx".to_string()),
        }),
        template: None,
        theme: None,
        slides: vec![json2pptx::model::Slide {
            background: Some(serde_json::json!({ "type": "image", "src": img })),
            transition: None,
            elements: vec![
                json2pptx::model::elements::Element::Text(
                    json2pptx::model::elements::TextElement {
                        placeholder: None,
                        autofit: None,
                        effects: None,
                        vert: None,
                        text: json2pptx::model::elements::TextContent::Simple(
                            "标题文本".to_string(),
                        ),
                        position: json2pptx::model::elements::Position {
                            x: 1.0,
                            y: 1.0,
                            w: 8.0,
                            h: 1.0,
                        },
                        name: Some("Title".to_string()),
                        ..Default::default()
                    },
                ),
                json2pptx::model::elements::Element::Image(
                    json2pptx::model::elements::ImageElement {
                        brightness: None,
                        contrast: None,
                        fill_mode: None,
                        tooltip: None,
                        media: None,
                        src: img.to_string_lossy().to_string(),
                        position: json2pptx::model::elements::Position {
                            x: 1.0,
                            y: 3.0,
                            w: 2.0,
                            h: 2.0,
                        },
                        ..Default::default()
                    },
                ),
                json2pptx::model::elements::Element::Group(
                    json2pptx::model::elements::GroupElement {
                        position: json2pptx::model::elements::Position {
                            x: 4.0,
                            y: 3.0,
                            w: 5.0,
                            h: 3.0,
                        },
                        name: Some("组一".to_string()),
                        children: vec![
                            json2pptx::model::elements::Element::Text(
                                json2pptx::model::elements::TextElement {
                                    placeholder: None,
                                    autofit: None,
                                    effects: None,
                                    vert: None,
                                    text: json2pptx::model::elements::TextContent::Simple(
                                        "组内文本".to_string(),
                                    ),
                                    position: json2pptx::model::elements::Position {
                                        x: 0.0,
                                        y: 0.0,
                                        w: 3.0,
                                        h: 1.0,
                                    },
                                    ..Default::default()
                                },
                            ),
                            json2pptx::model::elements::Element::Shape(
                                json2pptx::model::elements::ShapeElement {
                                    placeholder: None,
                                    autofit: None,
                                    effects: None,
                                    shape_type: "rect".to_string(),
                                    position: json2pptx::model::elements::Position {
                                        x: 0.0,
                                        y: 1.0,
                                        w: 2.0,
                                        h: 1.0,
                                    },
                                    fill: Some(json2pptx::model::elements::Fill::Solid(
                                        "FF0000".to_string(),
                                    )),
                                    ..Default::default()
                                },
                            ),
                        ],
                        ..Default::default()
                    },
                ),
            ],
            notes: None,
        }],
    };
    json2pptx::generate(&pres, pptx.to_str().unwrap()).unwrap();
    pptx
}

#[test]
fn test_view_on_pptx_and_product() {
    let pptx = build_test_pptx();
    let p = pptx.to_str().unwrap();
    let product = temp_dir("view_prod");
    let (ok, _out, err) = run(&["unpack", p, "-o", product.to_str().unwrap()]);
    assert!(ok, "unpack 失败: {err}");
    let product = product.to_str().unwrap().to_string();

    // 产物目录输入：view text
    let (ok, out, _err) = run(&["view", &product, "/slide[1]", "text"]);
    assert!(ok, "view text 失败: {out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["slide"], 1);
    let texts = v["texts"].as_array().unwrap();
    assert!(texts.iter().any(|t| t["text"] == "标题文本"));
    let group = texts.iter().find(|t| t.get("children").is_some()).unwrap();
    assert_eq!(group["children"][0]["text"], "组内文本");
    let media = v["media"].as_array().unwrap();
    assert!(media.iter().any(|m| m["name"] == "background"));
    // 图片媒体解析为产物内的绝对路径（URL 保留原样）
    assert!(media.iter().any(|m| {
        let src = m["src"].as_str().unwrap();
        !src.starts_with("http") && !src.is_empty()
    }));
    assert!(media.iter().any(|m| m["src"]
        .as_str()
        .unwrap()
        .replace('\\', "/")
        .contains("ppt/media/")));

    // PPTX 文件输入（一次性解包到临时目录）：结果一致
    let (ok, out, _err) = run(&["view", p, "/slide[1]", "text"]);
    assert!(ok, "view pptx 失败: {out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let texts = v["texts"].as_array().unwrap();
    assert!(texts.iter().any(|t| t["text"] == "标题文本"));
    assert!(texts.iter().find(|t| t.get("children").is_some()).is_some());

    // view layout：层级与位置
    let (ok, out, _err) = run(&["view", &product, "/slide[1]", "layout"]);
    assert!(ok, "view layout 失败: {out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let layout = v["layout"].as_array().unwrap();
    let group = layout.iter().find(|e| e["type"] == "group").unwrap();
    assert_eq!(group["name"], "组一");
    assert_eq!(group["position"]["x"], 4.0);
    let children = group["children"].as_array().unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0]["type"], "text");
    assert_eq!(children[1]["type"], "shape");
}

#[test]
fn test_edit_roundtrip_via_product() {
    let pptx = build_test_pptx();
    let p = pptx.to_str().unwrap();
    let product = temp_dir("edit_prod");
    let (ok, _out, err) = run(&["unpack", p, "-o", product.to_str().unwrap()]);
    assert!(ok, "unpack 失败: {err}");
    let product = product.to_str().unwrap().to_string();

    // get
    let (ok, out, _err) = run(&["edit", &product, "/slide[1]/text[1]", "get"]);
    assert!(ok, "get 失败: {out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["text"], "标题文本");

    // 嵌套路径 get
    let (ok, out, _err) = run(&["edit", &product, "/slide[1]/group[1]/text[1]", "get"]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["text"], "组内文本");

    // set 元素文字
    let (ok, out, _err) = run(&[
        "edit",
        &product,
        "/slide[1]/text[1]",
        "set",
        "--prop",
        "text=修改后的标题",
    ]);
    assert!(ok, "set 失败: {out}");

    // add 到 slide 级
    let (ok, _out, _err) = run(&[
        "edit",
        &product,
        "/slide[1]",
        "add",
        "--type",
        "text",
        "--prop",
        "text=新增段落",
        "--prop",
        "x=1",
        "--prop",
        "y=7",
        "--prop",
        "w=6",
        "--prop",
        "h=1",
    ]);
    assert!(ok, "add 失败");

    // add 到 group 内
    let (ok, _out, _err) = run(&[
        "edit",
        &product,
        "/slide[1]/group[1]",
        "add",
        "--type",
        "shape",
        "--prop",
        "shape_type=ellipse",
        "--prop",
        "fill=00FF00",
    ]);
    assert!(ok, "add 到 group 失败");

    // remove 组内形状
    let (ok, out, _err) = run(&["edit", &product, "/slide[1]/group[1]/shape[1]", "remove"]);
    assert!(ok, "remove 失败: {out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["removed"], "shape");

    // 修改已在产物中持久化（view 重新读取）
    let (ok, out, _err) = run(&["view", &product, "/slide[1]", "text"]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let texts = v["texts"].as_array().unwrap();
    assert!(texts.iter().any(|t| t["text"] == "修改后的标题"));
    assert!(texts.iter().any(|t| t["text"] == "新增段落"));

    // repack 回环：编辑产物 → PPTX → parse
    let out_pptx = temp_dir("edit_out").join("edited.pptx");
    let (ok, _out, _err) = run(&["repack", &product, "-o", out_pptx.to_str().unwrap()]);
    assert!(ok, "repack 失败");
    let parsed = json2pptx::parse(out_pptx.to_str().unwrap()).unwrap();
    let el_types: Vec<&str> = parsed.slides[0]
        .elements
        .iter()
        .map(|e| e.type_name())
        .collect();
    assert_eq!(el_types, vec!["text", "image", "group", "text"]);
    let group = match &parsed.slides[0].elements[2] {
        json2pptx::model::elements::Element::Group(g) => g,
        _ => panic!("预期 group"),
    };
    let child_types: Vec<&str> = group.children.iter().map(|c| c.type_name()).collect();
    assert_eq!(child_types, vec!["text", "shape"]);
    match &group.children[1] {
        json2pptx::model::elements::Element::Shape(s) => {
            assert_eq!(s.shape_type, "ellipse");
        }
        _ => panic!("预期 shape"),
    }
}

#[test]
fn test_edit_oneshot_on_pptx() {
    let pptx = build_test_pptx();
    let p = pptx.to_str().unwrap();

    // 对 .pptx 的写操作必须 -o
    let (ok, _out, err) = run(&["edit", p, "/slide[1]/text[1]", "set", "--prop", "text=X"]);
    assert!(!ok, "缺 -o 应报错");
    assert!(err.contains("-o") || err.contains("pptx"));

    // 一次性修改：unpack → set → repack，输出新文件，原文件不变
    let out_pptx = temp_dir("oneshot_out").join("edited.pptx");
    let (ok, out, _err) = run(&[
        "edit",
        p,
        "/slide[1]/text[1]",
        "set",
        "--prop",
        "text=一次性修改",
        "-o",
        out_pptx.to_str().unwrap(),
    ]);
    assert!(ok, "一次性 edit 失败: {out}");

    // 输出文件包含修改后的文本
    let (ok, out, _err) = run(&["view", out_pptx.to_str().unwrap(), "/slide[1]", "text"]);
    assert!(ok, "view 输出失败: {out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let texts = v["texts"].as_array().unwrap();
    assert!(texts.iter().any(|t| t["text"] == "一次性修改"));

    // 原文件未被修改
    let (ok, out, _err) = run(&["view", p, "/slide[1]", "text"]);
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let texts = v["texts"].as_array().unwrap();
    assert!(texts.iter().any(|t| t["text"] == "标题文本"));
    assert!(!texts.iter().any(|t| t["text"] == "一次性修改"));
}

#[test]
fn test_invalid_paths() {
    let pptx = build_test_pptx();
    let p = pptx.to_str().unwrap();
    let product = temp_dir("invalid_prod");
    let (ok, _out, err) = run(&["unpack", p, "-o", product.to_str().unwrap()]);
    assert!(ok, "unpack 失败: {err}");
    let product = product.to_str().unwrap().to_string();

    // slide 越界（错误信息输出到 stderr）
    let (ok, _out, err) = run(&["view", &product, "/slide[9]", "text"]);
    assert!(!ok);
    assert!(err.contains("越界") || err.contains("slide"));

    // 元素不存在
    let (ok, _out, _err) = run(&["edit", &product, "/slide[1]/text[9]", "get"]);
    assert!(!ok);

    // 路径格式错误
    let (ok, _out, _err) = run(&["view", &product, "slide[1]", "text"]);
    assert!(!ok);

    // remove 需要元素路径
    let (ok, _out, _err) = run(&["edit", &product, "/slide[1]", "remove"]);
    assert!(!ok);
}

/// 新元素（line/图表组件）与 notes 的 CLI 端到端
#[test]
fn test_edit_line_chart_notes() {
    let pptx = build_test_pptx();
    let p = pptx.to_str().unwrap();
    let product = temp_dir("line_prod");
    let (ok, _out, err) = run(&["unpack", p, "-o", product.to_str().unwrap()]);
    assert!(ok, "unpack 失败: {err}");
    let product = product.to_str().unwrap().to_string();

    // add line 到 slide 级
    let (ok, out, _err) = run(&[
        "edit",
        &product,
        "/slide[1]",
        "add",
        "--type",
        "line",
        "--prop",
        "points=0,0.5,5,0.5",
        "--prop",
        "color=E74C3C",
        "--prop",
        "width=2",
        "--prop",
        "dash=dashed",
        "--prop",
        "arrow_end=arrow",
    ]);
    assert!(ok, "add line 失败: {out} {_err}");

    // set line 属性
    let (ok, out, _err) = run(&[
        "edit",
        &product,
        "/slide[1]/line[1]",
        "set",
        "--prop",
        "color=4472C4",
        "--prop",
        "smooth=true",
    ]);
    assert!(ok, "set line 失败: {out}");

    // add lineChart 组件
    let (ok, out, _err) = run(&[
        "edit",
        &product,
        "/slide[1]",
        "add",
        "--type",
        "lineChart",
        "--prop",
        "data=10,20,15",
        "--prop",
        "labels=Q1,Q2,Q3",
    ]);
    assert!(ok, "add lineChart 失败: {out}");

    // set 幻灯片备注
    let (ok, out, _err) = run(&[
        "edit",
        &product,
        "/slide[1]",
        "set",
        "--prop",
        "notes=CLI 端到端备注",
    ]);
    assert!(ok, "set notes 失败: {out}");

    // view layout：line 与 lineChart 类型可见
    let (ok, out, _err) = run(&["view", &product, "/slide[1]", "layout"]);
    assert!(ok, "view layout 失败: {out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let layout = v["layout"].as_array().unwrap();
    let line = layout.iter().find(|e| e["type"] == "line").unwrap();
    assert!(line["position"].is_object());
    let chart = layout.iter().find(|e| e["type"] == "line_chart").unwrap();
    assert_eq!(chart["data"], serde_json::json!([10.0, 20.0, 15.0]));

    // view text：notes 输出
    let (ok, out, _err) = run(&["view", &product, "/slide[1]", "text"]);
    assert!(ok, "view text 失败: {out}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["notes"], "CLI 端到端备注");

    // repack → parse 回环：line 元素与 notes 保留
    let out_pptx = temp_dir("line_out").join("edited2.pptx");
    let (ok, _out, _err) = run(&["repack", &product, "-o", out_pptx.to_str().unwrap()]);
    assert!(ok, "repack 失败");
    let parsed = json2pptx::parse(out_pptx.to_str().unwrap()).unwrap();
    let slide = &parsed.slides[0];
    let el_types: Vec<&str> = slide.elements.iter().map(|e| e.type_name()).collect();
    assert!(el_types.contains(&"line"), "回环后应含 line: {el_types:?}");
    // lineChart 组件展开为形状 Group（形状模拟不回环，与现有组件行为一致）
    assert!(
        el_types.contains(&"group"),
        "回环后应含展开的 group: {el_types:?}"
    );
    let line = slide
        .elements
        .iter()
        .find(|e| e.type_name() == "line")
        .unwrap();
    match line {
        json2pptx::model::elements::Element::Line(l) => {
            assert_eq!(l.color.as_deref(), Some("4472C4"));
            assert_eq!(l.smooth, Some(true));
            assert_eq!(l.arrow_end.as_deref(), Some("arrow"));
        }
        _ => panic!("预期 line"),
    }
    assert_eq!(slide.notes.as_deref(), Some("CLI 端到端备注"));
}
#[test]
fn test_query_validate_dump_commands() {
    let pptx = build_test_pptx();
    let p = pptx.to_str().unwrap();

    // query：按类型
    let (ok, out, err) = run(&["query", p, "text"]);
    assert!(ok, "query failed: {err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["count"].as_u64().unwrap() >= 1, "query count: {out}");

    // query：:contains + slide 限定
    let (ok, out, err) = run(&["query", p, "slide[1] text:contains(\"标题\")"]);
    assert!(ok, "query contains failed: {err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["count"].as_u64().unwrap() >= 1, "contains count: {out}");

    // validate
    let (ok, out, err) = run(&["validate", p]);
    assert!(ok, "validate failed: {err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["issues"].is_array());

    // dump
    let (ok, out, err) = run(&["dump", p]);
    assert!(ok, "dump failed: {err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v["commands"].as_array().unwrap().len() >= 2, "dump: {out}");
}

#[test]
fn test_name_path_addressing() {
    let pptx = build_test_pptx();
    let p = pptx.to_str().unwrap();
    // 先 query 拿到某个元素名称
    let (ok, out, _) = run(&["query", p, "text"]);
    assert!(ok, "query failed");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let name = v["matches"][0]["name"].as_str().unwrap().to_string();
    // 用 @name 定位 get
    let path = format!("/slide[1]/text[@name={name}]");
    let (ok, out, err) = run(&["edit", p, &path, "get"]);
    assert!(ok, "@name get failed: {err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["name"].as_str().unwrap(), name);
}
