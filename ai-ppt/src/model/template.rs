//! 模板预设
//!
//! 定义了 6 种内置主题模板：现代简洁、深色科技、商务专业、创意多彩、自然清新、极简黑白。
//! 每种模板包含完整的颜色方案和字体配置。

use crate::model::Theme;

/// 模板预设定义
pub struct TemplatePreset {
    /// 模板名称（用于 JSON 中 template 字段引用）
    pub name: &'static str,
    /// 中文显示名称
    pub display: &'static str,
    /// 英文描述
    pub desc: &'static str,
    /// 默认背景色
    pub bg: Option<&'static str>,
    /// 主题颜色对 [(键, 值), ...]
    pub theme_colors: &'static [(&'static str, &'static str)],
    /// 主要字体（标题字体）
    pub major_font: &'static str,
    /// 次要字体（正文字体）
    pub minor_font: &'static str,
}

impl TemplatePreset {
    /// 将预设转换为 Theme 对象
    pub fn to_theme(&self) -> Theme {
        Theme {
            colors: Some(
                self.theme_colors
                    .iter()
                    .map(|&(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            ),
            major_font: Some(self.major_font.to_string()),
            minor_font: Some(self.minor_font.to_string()),
        }
    }

    /// 根据名称查找模板预设
    pub fn from_name(name: &str) -> Option<&'static TemplatePreset> {
        ALL.iter().copied().find(|t| t.name == name)
    }

    /// 获取所有模板预设
    pub fn all() -> &'static [&'static TemplatePreset] {
        ALL
    }
}

macro_rules! preset {
    ($name:expr, $display:expr, $desc:expr, $bg:expr, $major:expr, $minor:expr, [$(($k:expr, $v:expr)),*]) => {
        TemplatePreset {
            name: $name,
            display: $display,
            desc: $desc,
            bg: $bg,
            theme_colors: &[$(($k, $v)),*],
            major_font: $major,
            minor_font: $minor,
        }
    };
}

pub static MODERN: TemplatePreset = preset!(
    "modern",
    "现代简洁",
    "Clean blue & white",
    Some("#FFFFFF"),
    "Segoe UI",
    "Microsoft YaHei",
    [
        ("dk1", "000000"),
        ("lt1", "FFFFFF"),
        ("dk2", "1E3A5F"),
        ("lt2", "F0F4FA"),
        ("accent1", "0078D4"),
        ("accent2", "00BCF2"),
        ("accent3", "0099FF"),
        ("accent4", "FFB900"),
        ("accent5", "7FBA00"),
        ("accent6", "E74856"),
        ("hlink", "0078D4"),
        ("folHlink", "9B59B6")
    ]
);

pub static DARK: TemplatePreset = preset!(
    "dark",
    "深色科技",
    "Dark theme with vibrant accents",
    Some("#1E1E1E"),
    "Helvetica Neue",
    "Noto Sans SC",
    [
        ("dk1", "FFFFFF"),
        ("lt1", "1E1E1E"),
        ("dk2", "CCCCCC"),
        ("lt2", "2D2D2D"),
        ("accent1", "E53935"),
        ("accent2", "FFB300"),
        ("accent3", "00BCD4"),
        ("accent4", "8E24AA"),
        ("accent5", "43A047"),
        ("accent6", "FF7043"),
        ("hlink", "42A5F5"),
        ("folHlink", "AB47BC")
    ]
);

pub static CORPORATE: TemplatePreset = preset!(
    "corporate",
    "商务专业",
    "Professional navy blue",
    Some("#FFFFFF"),
    "Calibri",
    "SimHei",
    [
        ("dk1", "000000"),
        ("lt1", "FFFFFF"),
        ("dk2", "1A3C5E"),
        ("lt2", "E8EDF2"),
        ("accent1", "1A5276"),
        ("accent2", "2E86C1"),
        ("accent3", "85C1E9"),
        ("accent4", "F39C12"),
        ("accent5", "27AE60"),
        ("accent6", "E74C3C"),
        ("hlink", "2980B9"),
        ("folHlink", "8E44AD")
    ]
);

pub static CREATIVE: TemplatePreset = preset!(
    "creative",
    "创意多彩",
    "Bright & colorful palette",
    Some("#FFF8F8"),
    "Montserrat",
    "STKaiti",
    [
        ("dk1", "2D3436"),
        ("lt1", "FFFFFF"),
        ("dk2", "636E72"),
        ("lt2", "FFF5F5"),
        ("accent1", "FF6B6B"),
        ("accent2", "4ECDC4"),
        ("accent3", "FFE66D"),
        ("accent4", "292F36"),
        ("accent5", "FF8A5C"),
        ("accent6", "A29BFE"),
        ("hlink", "4ECDC4"),
        ("folHlink", "FF6B6B")
    ]
);

pub static NATURE: TemplatePreset = preset!(
    "nature",
    "自然清新",
    "Green & earthy tones",
    Some("#F1F8E9"),
    "Georgia",
    "STSong",
    [
        ("dk1", "1B4332"),
        ("lt1", "FFFFFF"),
        ("dk2", "2D6A4F"),
        ("lt2", "E8F5E9"),
        ("accent1", "4CAF50"),
        ("accent2", "8BC34A"),
        ("accent3", "66BB6A"),
        ("accent4", "FFCA28"),
        ("accent5", "26A69A"),
        ("accent6", "EF5350"),
        ("hlink", "2E7D32"),
        ("folHlink", "6A1B9A")
    ]
);

pub static MINIMAL: TemplatePreset = preset!(
    "minimal",
    "极简黑白",
    "Clean monochrome minimal",
    Some("#FFFFFF"),
    "Helvetica Light",
    "Noto Sans SC Light",
    [
        ("dk1", "212121"),
        ("lt1", "FFFFFF"),
        ("dk2", "424242"),
        ("lt2", "FAFAFA"),
        ("accent1", "607D8B"),
        ("accent2", "B0BEC5"),
        ("accent3", "78909C"),
        ("accent4", "90A4AE"),
        ("accent5", "546E7A"),
        ("accent6", "CFD8DC"),
        ("hlink", "37474F"),
        ("folHlink", "546E7A")
    ]
);

pub static ALL: &[&TemplatePreset] = &[&MODERN, &DARK, &CORPORATE, &CREATIVE, &NATURE, &MINIMAL];
