//! 旧版 Word（.doc, MS-DOC）→ 新版 .docx：基于 rwml（纯 Rust）
//!
//! rwml 的 DocModel 覆盖段落/run 格式/标题/列表/表格/图片/批注/页眉页脚，
//! 直接用其 write_docx 产出 .docx，再走现有 unpack 流水线。
//! 保真度详见 docs/legacy-formats.md。

use crate::error::Error;
use std::path::Path;

/// 文件是否为 OLE2/CFB 容器（.doc 的共同特征）
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

/// .doc → .docx（内存 bytes，调用方写入目标路径）
pub fn doc_to_docx_bytes(path: &Path) -> Result<Vec<u8>, Error> {
    let bytes = std::fs::read(path)?;
    let doc = rwml::Document::open(&bytes).map_err(|e| {
        Error::InvalidInput(format!(
            "检测到旧版 Word 文件（.doc），自动转换失败: {e}\n\
                 建议: 先用 LibreOffice 将其另存为新格式后重试，\n\
                 例如: soffice --headless --convert-to docx <文件>"
        ))
    })?;
    let model = doc.model();
    let out = rwml::write_docx(&model);
    Ok(out)
}
