//! 动画数据模型

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 元素动画效果
///
/// 支持进入、退出、强调和路径动画四大类。
/// 触发方式支持：onClick（点击）、afterPrevious（之后）、withPrevious（同时）。
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Animation {
    /// 动画类型：appear, fadeIn, fadeOut, flyIn, flyOut, zoomIn, zoomOut,
    ///           wipeIn, wipeOut, bounceIn, floatIn, floatOut, swivel,
    ///           dissolveIn, dissolveOut, splitIn, pulse, spin,
    ///           growShrink, colorChange, transparency, teeter, blink, motionPath
    pub r#type: String,
    /// 动画持续时间（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
    /// 动画延迟（毫秒）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<u64>,
    /// 触发方式：onClick, afterPrevious, withPrevious
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    /// 方向：top, bottom, left, right 等
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// 动画执行顺序
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
    /// 缩放比例（百分比）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scale: Option<u32>,
    /// 旋转角度（度）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub degrees: Option<f64>,
    /// 颜色变化目标色
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// 透明度变化目标值
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    /// 路径动画的关键点
    #[serde(skip_serializing_if = "Option::is_none")]
    pub points: Option<Vec<HashMap<String, f64>>>,
    /// 动画移动距离（英寸）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<f64>,
    /// 重复次数（数字字符串或 "indefinite"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat: Option<String>,
    /// 重启方式（如 "whenNotActive"）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    /// 是否自动反向播放
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_reverse: Option<bool>,
}
