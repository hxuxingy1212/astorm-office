//! CLI CSV 导入端到端测试。

#[test]
fn import_csv_e2e() {
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let csv = dir.join(format!("json2xlsx_imp_{pid}.csv"));
    let out = dir.join(format!("json2xlsx_imp_{pid}.xlsx"));
    std::fs::write(&csv, "Name,Amount\n\"A, B\",12.5\nC,3\n").unwrap();
    let bin = env!("CARGO_BIN_EXE_json2xlsx");
    let status = std::process::Command::new(bin)
        .args([
            "import",
            csv.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--header",
        ])
        .status()
        .expect("spawn cli");
    assert!(status.success());
    let wb = json2xlsx::parse(&out).expect("parse imported xlsx");
    assert_eq!(wb.sheets.len(), 1);
    let cells = &wb.sheets[0].rows[1].cells;
    assert_eq!(cells[0].value, Some(serde_json::json!("A, B")));
    assert_eq!(cells[1].value, Some(serde_json::json!(12.5)));
    let _ = std::fs::remove_file(&csv);
    let _ = std::fs::remove_file(&out);
}
