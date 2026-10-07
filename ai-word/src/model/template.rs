//! 文档模板预设

use crate::model::document::Document;
use crate::model::style::{Margins, Style};
use std::collections::BTreeMap;

/// 模板预设
#[derive(Debug, Clone)]
pub struct TemplatePreset {
    pub name: &'static str,
    pub display: &'static str,
    pub desc: &'static str,
    page_size: &'static str,
    major_font: &'static str,
    minor_font: &'static str,
    east_asia_font: &'static str,
    heading_font: &'static str,
    body_size: f64,
    #[allow(dead_code)]
    heading1_size: f64,
    line_spacing: f64,
    first_line_indent: f64,
    heading_color: &'static str,
    /// (styleId, 字号, 粗体, 对齐)
    headings: &'static [(&'static str, &'static str, f64, bool, &'static str)],
}

const fn heading_defs() -> &'static [(&'static str, &'static str, f64, bool, &'static str)] {
    &[
        ("Heading1", "Heading 1", 18.0, true, "left"),
        ("Heading2", "Heading 2", 15.0, true, "left"),
        ("Heading3", "Heading 3", 13.0, true, "left"),
        ("Heading4", "Heading 4", 12.0, true, "left"),
    ]
}

const ACADEMIC: TemplatePreset = TemplatePreset {
    name: "academic-paper",
    display: "Academic Paper (学术论文)",
    desc: "Times New Roman/宋体，1.5 倍行距，首行缩进，标题分级",
    page_size: "A4",
    major_font: "Times New Roman",
    minor_font: "Times New Roman",
    east_asia_font: "宋体",
    heading_font: "黑体",
    body_size: 12.0,
    heading1_size: 18.0,
    line_spacing: 1.5,
    first_line_indent: 24.0,
    heading_color: "1F3864",
    headings: heading_defs(),
};

const REPORT: TemplatePreset = TemplatePreset {
    name: "report",
    display: "Business Report (报告)",
    desc: "Calibri，1.15 倍行距，无首行缩进，标题左对齐",
    page_size: "A4",
    major_font: "Calibri",
    minor_font: "Calibri",
    east_asia_font: "微软雅黑",
    heading_font: "Calibri",
    body_size: 11.0,
    heading1_size: 20.0,
    line_spacing: 1.15,
    first_line_indent: 0.0,
    heading_color: "1F4E79",
    headings: heading_defs(),
};

const LETTER: TemplatePreset = TemplatePreset {
    name: "letter",
    display: "Letter (信函)",
    desc: "Garamond，单倍行距，无首行缩进",
    page_size: "Letter",
    major_font: "Garamond",
    minor_font: "Garamond",
    east_asia_font: "宋体",
    heading_font: "Garamond",
    body_size: 11.0,
    heading1_size: 16.0,
    line_spacing: 1.0,
    first_line_indent: 0.0,
    heading_color: "000000",
    headings: heading_defs(),
};

const RESUME: TemplatePreset = TemplatePreset {
    name: "resume",
    display: "Resume (简历)",
    desc: "Calibri，紧凑行距，标题醒目",
    page_size: "Letter",
    major_font: "Calibri",
    minor_font: "Calibri",
    east_asia_font: "微软雅黑",
    heading_font: "Calibri",
    body_size: 10.5,
    heading1_size: 16.0,
    line_spacing: 1.05,
    first_line_indent: 0.0,
    heading_color: "2E74B5",
    headings: heading_defs(),
};

const MEMO: TemplatePreset = TemplatePreset {
    name: "memo",
    display: "Memo (备忘录)",
    desc: "Calibri，简洁，标题小号",
    page_size: "Letter",
    major_font: "Calibri",
    minor_font: "Calibri",
    east_asia_font: "宋体",
    heading_font: "Calibri",
    body_size: 11.0,
    heading1_size: 14.0,
    line_spacing: 1.15,
    first_line_indent: 0.0,
    heading_color: "222222",
    headings: heading_defs(),
};

const ALL: &[TemplatePreset] = &[ACADEMIC, REPORT, LETTER, RESUME, MEMO];

impl TemplatePreset {
    /// 全部模板
    pub fn all() -> &'static [TemplatePreset] {
        ALL
    }

    /// 按名称查找
    pub fn from_name(name: &str) -> Option<&'static TemplatePreset> {
        ALL.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// 将模板合并进文档（用户显式字段优先）
    pub fn apply(&self, doc: &mut Document) {
        // 页面
        if doc.page.size == "A4" && self.page_size != "A4" {
            doc.page.size = self.page_size.to_string();
        }
        if doc.page.margins == Margins::default() {
            doc.page.margins = Margins::default();
        }
        // 主题
        if doc.theme.major_font.is_none() {
            doc.theme.major_font = Some(self.major_font.to_string());
        }
        if doc.theme.minor_font.is_none() {
            doc.theme.minor_font = Some(self.minor_font.to_string());
        }
        if doc.theme.east_asia_font.is_none() {
            doc.theme.east_asia_font = Some(self.east_asia_font.to_string());
        }
        if doc.theme.heading_font.is_none() {
            doc.theme.heading_font = Some(self.heading_font.to_string());
        }
        if doc.theme.colors.is_none() {
            let mut colors = BTreeMap::new();
            colors.insert("heading".to_string(), self.heading_color.to_string());
            colors.insert("link".to_string(), "0563C1".to_string());
            doc.theme.colors = Some(colors);
        }
        // 样式
        doc.styles.entry("Normal".to_string()).or_insert(Style {
            font_size: Some(self.body_size),
            line_spacing: Some(self.line_spacing),
            first_line_indent: if self.first_line_indent > 0.0 {
                Some(self.first_line_indent)
            } else {
                None
            },
            font_family: Some(self.minor_font.to_string()),
            east_asia_font: Some(self.east_asia_font.to_string()),
            ..Default::default()
        });
        for (id, _name, size, bold, align) in self.headings {
            doc.styles.entry(id.to_string()).or_insert(Style {
                font_size: Some(*size),
                bold: Some(*bold),
                color: Some(self.heading_color.to_string()),
                align: Some((*align).to_string()),
                font_family: Some(self.heading_font.to_string()),
                east_asia_font: Some(self.heading_font.to_string()),
                ..Default::default()
            });
        }
        doc.styles.entry("Caption".to_string()).or_insert(Style {
            font_size: Some(10.0),
            italic: Some(true),
            align: Some("center".to_string()),
            ..Default::default()
        });
        doc.styles.entry("Quote".to_string()).or_insert(Style {
            font_size: Some(self.body_size),
            italic: Some(true),
            align: Some("left".to_string()),
            ..Default::default()
        });
        doc.styles.entry("Code".to_string()).or_insert(Style {
            font_family: Some("Consolas".to_string()),
            font_size: Some(10.0),
            ..Default::default()
        });
        doc.styles.entry("Reference".to_string()).or_insert(Style {
            font_size: Some(self.body_size),
            line_spacing: Some(self.line_spacing),
            ..Default::default()
        });
    }
}
