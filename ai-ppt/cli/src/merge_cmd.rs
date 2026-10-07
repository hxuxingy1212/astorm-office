//! merge：用 JSON 数据替换产物中的 `{{key}}` 占位符

use office_core::CliError;
use std::path::Path;

/// 对产物目录执行模板填充；返回替换次数
pub fn run(product_root: &Path, data: &serde_json::Value) -> Result<usize, CliError> {
    let mut slides = crate::product::load_slides(product_root)?;
    let n = json2pptx::merge_slides(&mut slides, data);
    crate::product::save_slides(product_root, &slides)?;
    Ok(n)
}
