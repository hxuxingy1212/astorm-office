use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::is_false;
use super::style::StyleDef;

/// A cell's `style` field: either a name referencing `workbook.styles` or an
/// inline style definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum CellStyleRef {
    Named(String),
    Inline(StyleDef),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Cell {
    /// A1 reference, e.g. "B2". Inferred from position when omitted.
    #[serde(rename = "ref", default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// One of string|number|boolean|date|error|richtext. Inferred when omitted.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub cell_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<CellStyleRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_tooltip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<Run>,
}

impl Cell {
    /// Infer the cell type from its value when not explicitly set.
    pub fn infer_type(&self) -> Option<String> {
        if self.cell_type.is_some() {
            return self.cell_type.clone();
        }
        if self.formula.is_some() {
            return None;
        }
        match &self.value {
            Some(Value::String(_)) => Some("string".into()),
            Some(Value::Number(_)) => Some("number".into()),
            Some(Value::Bool(_)) => Some("boolean".into()),
            Some(Value::Null) | None => None,
            Some(Value::Array(_)) | Some(Value::Object(_)) => Some("string".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Run {
    pub text: String,
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub strike: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<String>,
    /// "superscript" or "subscript".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vert_align: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
}
