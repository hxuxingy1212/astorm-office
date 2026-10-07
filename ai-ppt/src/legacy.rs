//! 旧版 PowerPoint（.ppt, MS-PPT）→ 新版 .pptx：基于 office_oxide（纯 Rust）
//!
//! office_oxide 对旧格式只读，可通过 save_as 输出 OOXML；之后走现有 unpack 流水线。
//! 保真度详见 docs/legacy-formats.md（.ppt 为纯 Rust 路线中保真最弱的一档）。

use crate::error::Error;
use std::path::Path;

/// 文件是否为 OLE2/CFB 容器（.ppt 的共同特征）
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

/// .ppt → .pptx
pub fn ppt_to_pptx(path: &Path, out: &Path) -> Result<(), Error> {
    let doc = office_oxide::Document::open(path.to_str().unwrap_or_default()).map_err(|e| {
        Error::InvalidInput(format!(
            "检测到旧版 PowerPoint 文件（.ppt），自动转换失败: {e}\n\
                 建议: 先用 LibreOffice 将其另存为新格式后重试，\n\
                 例如: soffice --headless --convert-to pptx <文件>"
        ))
    })?;
    doc.save_as(out.to_str().unwrap_or_default())
        .map_err(|e| Error::InvalidInput(format!("旧版 .ppt 转 .pptx 失败: {e}")))?;
    Ok(())
}
