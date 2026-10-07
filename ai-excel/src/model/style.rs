use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::is_false;

/// A reusable cell style definition. Cells reference it by name (via
/// [`super::CellStyleRef::Named`]) or embed it inline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct StyleDef {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<Font>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<Border>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<Alignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked: Option<bool>,
}

impl StyleDef {
    pub fn is_empty(&self) -> bool {
        self.font.is_none()
            && self.fill.is_none()
            && self.border.is_none()
            && self.alignment.is_none()
            && self.number_format.is_none()
            && self.locked.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Font {
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub strike: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub underline: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
}

impl Font {
    pub fn is_empty(&self) -> bool {
        !self.bold
            && !self.italic
            && !self.strike
            && self.underline.is_none()
            && self.name.is_none()
            && self.size.is_none()
            && self.color.is_none()
    }
}

/// Border sides. `all` is a shorthand expanded into the four sides during
/// normalization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Border {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bottom: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagonal: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub diagonal_up: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub diagonal_down: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagonal_color: Option<String>,
}

impl Border {
    pub fn is_empty(&self) -> bool {
        self.all.is_none()
            && self.top.is_none()
            && self.bottom.is_none()
            && self.left.is_none()
            && self.right.is_none()
            && self.color.is_none()
            && self.diagonal.is_none()
            && !self.diagonal_up
            && !self.diagonal_down
            && self.diagonal_color.is_none()
    }

    /// Expand the `all` shorthand and drop it.
    pub fn expand(&mut self) {
        if let Some(style) = self.all.take() {
            if self.top.is_none() {
                self.top = Some(style.clone());
            }
            if self.bottom.is_none() {
                self.bottom = Some(style.clone());
            }
            if self.left.is_none() {
                self.left = Some(style.clone());
            }
            if self.right.is_none() {
                self.right = Some(style);
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, JsonSchema)]
pub struct Alignment {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub horizontal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vertical: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub wrap_text: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_rotation: Option<i32>,
}

impl Alignment {
    pub fn is_empty(&self) -> bool {
        self.horizontal.is_none()
            && self.vertical.is_none()
            && !self.wrap_text
            && self.indent.is_none()
            && self.text_rotation.is_none()
    }
}
