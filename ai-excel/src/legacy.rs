//! 旧版 Excel（.xls, BIFF8）→ Workbook：基于 calamine（纯 Rust）
//!
//! parse 入口检测到 OLE2/CFB 魔数时自动走此转换（"转换成新的再处理"）。
//! 保真度：值/公式/合并单元格/多表可保留；字体/填充/边框等样式与图表会丢失
//! （calamine 对 BIFF8 样式提取有限），详见 docs/legacy-formats.md。

use crate::error::Error;
use crate::model::cell::Cell as ModelCell;
use crate::model::sheet::{Row, Sheet};
use crate::model::workbook::Workbook;
use calamine::Reader;
use std::path::Path;

/// 文件是否为 OLE2/CFB 容器（.doc/.xls/.ppt 与加密 OOXML 的共同特征）
pub fn is_cfb(path: &Path) -> bool {
    use std::io::Read;
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut magic = [0u8; 8];
    matches!(f.read_exact(&mut magic), Ok(()))
        && magic == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]
}

/// .xls → Workbook（内存模型，后续 unpack/repack/view/edit 全部复用现有流水线）
pub fn xls_to_workbook(path: &Path) -> Result<Workbook, Error> {
    let mut wb: calamine::Xls<std::io::BufReader<std::fs::File>> = calamine::open_workbook(path)
        .map_err(|e| {
            Error::InvalidInput(format!(
                "检测到旧版 Excel 文件（.xls），自动转换失败: {e}\n\
                 建议: 先用 LibreOffice 将其另存为新格式后重试，\n\
                 例如: soffice --headless --convert-to xlsx <文件>"
            ))
        })?;
    let mut out = Workbook::default();
    for name in wb.sheet_names() {
        let range = wb
            .worksheet_range(&name)
            .map_err(|e| Error::InvalidInput(format!("工作表 {name} 读取失败: {e}")))?;
        let formulas = wb
            .worksheet_formula(&name)
            .map_err(|e| Error::InvalidInput(format!("工作表 {name} 公式读取失败: {e}")))?;
        let merges: Vec<String> = wb
            .merge_cells_by_sheet_name(&name)
            .unwrap_or_default()
            .iter()
            .map(|dim: &calamine::Dimensions| {
                let (r1, c1) = dim.start;
                let (r2, c2) = dim.end;
                format!(
                    "{}{}:{}{}",
                    col_letter(c1 as usize + 1),
                    r1 + 1,
                    col_letter(c2 as usize + 1),
                    r2 + 1
                )
            })
            .collect();

        let mut rows: Vec<Row> = Vec::new();
        let (height, width) = (range.height(), range.width());
        for r in 0..height {
            let row_no = (r + 1) as u32;
            let mut cells: Vec<ModelCell> = Vec::new();
            for c in 0..width {
                let reference = format!("{}{}", col_letter(c + 1), row_no);
                let v = range.get_value((r as u32, c as u32));
                let formula = formulas
                    .get_value((r as u32, c as u32))
                    .map(|f| f.to_string());
                let mut model = ModelCell {
                    reference: Some(reference),
                    ..Default::default()
                };
                match v {
                    Some(calamine::Data::Empty) => {}
                    Some(calamine::Data::String(s)) => {
                        model.cell_type = Some("string".into());
                        model.value = Some(serde_json::Value::String(s.clone()));
                    }
                    Some(calamine::Data::Float(f)) => {
                        model.cell_type = Some("number".into());
                        model.value = Some(serde_json::json!(f));
                    }
                    Some(calamine::Data::Bool(b)) => {
                        model.cell_type = Some("boolean".into());
                        model.value = Some(serde_json::json!(b));
                    }
                    Some(other) => {
                        // DateTime/Duration/Iso/Error：统一转显示字符串
                        model.cell_type = Some("string".into());
                        model.value = Some(serde_json::Value::String(other.to_string()));
                    }
                    None => {}
                }
                if let Some(f) = formula {
                    // calamine 公式带前导 '='，模型约定不带
                    model.formula = Some(f.trim_start_matches('=').to_string());
                    model.cell_type = Some("formula".into());
                }
                if model.value.is_some() || model.formula.is_some() {
                    cells.push(model);
                }
            }
            if !cells.is_empty() {
                rows.push(Row {
                    index: Some(row_no),
                    cells,
                    ..Default::default()
                });
            }
        }

        out.sheets.push(Sheet {
            name: name.clone(),
            rows,
            merges,
            gridlines: None,
            freeze: None,
            ..Default::default()
        });
    }
    Ok(out)
}

fn col_letter(mut n: usize) -> String {
    let mut s = String::new();
    while n > 0 {
        let rem = ((n - 1) % 26) as u8;
        s.insert(0, (b'A' + rem) as char);
        n = (n - 1) / 26;
    }
    s
}

/// 旧格式自动识别与转换入口：OLE2/CFB 输入 → 内存 Workbook
pub fn maybe_legacy_workbook(path: &Path) -> Option<Result<Workbook, Error>> {
    if is_cfb(path) {
        // 先于 .xls 转换识别加密 OOXML（同为 CFB 容器）：给稳定的 encrypted 码而不是转换报错
        if office_core::opc::looks_encrypted_cfb(path) {
            return Some(Err(Error::Encrypted(
                "文档已加密（OOXML 密码保护），暂不支持".into(),
            )));
        }
        Some(xls_to_workbook(path))
    } else {
        None
    }
}
