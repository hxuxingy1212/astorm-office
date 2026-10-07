//! 生成烟雾测试用的 fixture PDF（与 tests/integration.rs 同构）。
//! 用法：cargo run -p json2pdf --example mkfixture -- <out.pdf>

fn build_pdf(pages_content: &[&str]) -> Vec<u8> {
    let mut objs: Vec<(u32, String)> = Vec::new();
    let n = pages_content.len();
    objs.push((1, "<< /Type /Catalog /Pages 2 0 R >>".into()));
    let kids: Vec<String> = (0..n)
        .map(|i| format!("{} 0 R", 3 + i as u32 * 2))
        .collect();
    objs.push((
        2,
        format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")),
    ));
    let font_num = 3 + n as u32 * 2;
    for (i, content) in pages_content.iter().enumerate() {
        let page_num = 3 + i as u32 * 2;
        let content_num = page_num + 1;
        objs.push((
            page_num,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                 /Resources << /Font << /F1 {font_num} 0 R >> >> /Contents {content_num} 0 R >>"
            ),
        ));
        objs.push((
            content_num,
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            ),
        ));
    }
    objs.push((
        font_num,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
    ));

    let mut out = String::from("%PDF-1.7\n");
    let mut offsets: Vec<(u32, u64)> = Vec::new();
    for (num, body) in &objs {
        offsets.push((*num, out.len() as u64));
        out.push_str(&format!("{num} 0 obj\n{body}\nendobj\n"));
    }
    let max = objs.iter().map(|(n, _)| *n).max().unwrap_or(0);
    let xref_start = out.len();
    out.push_str(&format!("xref\n0 {}\n", max + 1));
    out.push_str("0000000000 65535 f \n");
    for num in 1..=max {
        let off = offsets
            .iter()
            .find(|(n, _)| n == &num)
            .map(|(_, o)| *o)
            .unwrap_or(0);
        out.push_str(&format!("{off:010} 00000 n \n"));
    }
    out.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_start}\n%%EOF",
        max + 1
    ));
    out.into_bytes()
}

fn main() {
    let out = std::env::args().nth(1).expect("usage: mkfixture <out.pdf>");
    let pdf = build_pdf(&[
        // 封面
        "BT /F1 24 Tf 200 700 Td (2026 AI Data Report) Tj ET",
        // 目录
        "BT /F1 14 Tf 72 720 Td (Contents) Tj ET",
        "BT /F1 12 Tf 72 680 Td [(Introduction) -900 (..........) -100 (1)] TJ 0 -20 Td [(Methods) -1400 (..........) -100 (5)] TJ 0 -20 Td [(Results) -1350 (..........) -100) (9)] TJ ET",
        // 正文：两行
        "BT /F1 12 Tf 72 720 Td (Quarterly revenue grew 32% YoY.) Tj 0 -24 Td (See table below.) Tj ET",
        // 表格页：3 行 × 3 列（同一页）
        "BT /F1 12 Tf 72 660 Td (Region) Tj ET BT /F1 12 Tf 250 660 Td (Revenue) Tj ET BT /F1 12 Tf 400 660 Td (Growth) Tj ET \
         BT /F1 12 Tf 72 636 Td (North) Tj ET BT /F1 12 Tf 250 636 Td (1,204) Tj ET BT /F1 12 Tf 400 636 Td (28%) Tj ET \
         BT /F1 12 Tf 72 612 Td (South) Tj ET BT /F1 12 Tf 250 612 Td (986) Tj ET BT /F1 12 Tf 400 612 Td (41%) Tj ET",
        // 空白页
        "",
    ]);
    std::fs::write(&out, pdf).expect("write fixture");
    println!("wrote {out}");
}
