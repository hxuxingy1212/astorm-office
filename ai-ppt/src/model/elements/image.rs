//! 图片元素模型

use crate::model::elements::{Line, Position};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// 图片裁剪参数（EMU 单位）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Crop {
    /// 左侧裁剪距离
    #[serde(default)]
    pub left: i64,
    /// 顶部裁剪距离
    #[serde(default)]
    pub top: i64,
    /// 右侧裁剪距离
    #[serde(default)]
    pub right: i64,
    /// 底部裁剪距离
    #[serde(default)]
    pub bottom: i64,
}

/// 媒体引用（视频 / 音频）
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MediaRef {
    /// 类型：video / audio
    pub kind: String,
    /// 媒体文件路径或 URL
    pub src: String,
}

/// 图片元素
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ImageElement {
    /// 稳定 ID（来自/写入 p:cNvPr/@id），用于 `@id=N` 寻址；生成时缺省自动分配
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u32>,
    /// 图片源路径（本地路径或 URL）
    pub src: String,
    /// 位置和尺寸
    pub position: Position,
    /// 元素名称
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 旋转角度（度）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    /// 裁剪参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
    /// 超链接 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hyperlink: Option<String>,
    /// 图片边框
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<Line>,
    /// 亮度调整（-1.0 ~ 1.0）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness: Option<f64>,
    /// 对比度调整（-1.0 ~ 1.0）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrast: Option<f64>,
    /// 填充模式：stretch（默认）/ contain / cover / tile
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_mode: Option<String>,
    /// 悬浮提示文字
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tooltip: Option<String>,
    /// 关联的媒体（视频/音频，src 为媒体文件，图片本身作为封面）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<MediaRef>,
    /// 动画列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animations: Option<Vec<super::animation::Animation>>,
}
