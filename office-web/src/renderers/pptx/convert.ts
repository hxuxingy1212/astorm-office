//! 单位换算与颜色工具
//!
//! 产物 JSON 使用英寸 / 点（pt），屏幕渲染使用 px（1in = 96px）。

/** 英寸 → 像素（1in = 96px） */
export const INCH_TO_PX = 96

export function inchToPx(inches: number): number {
  return inches * INCH_TO_PX
}

export function pxToInch(px: number): number {
  return px / INCH_TO_PX
}

/** 点 → 像素（浏览器渲染 1pt = 4/3 px） */
export function ptToPx(pt: number): number {
  return (pt * 4) / 3
}

/** 像素 → 点 */
export function pxToPt(px: number): number {
  return (px * 3) / 4
}

/** 颜色：去掉 # 前缀（模型约定 6 位 hex 省略 #） */
export function stripHash(color: string): string {
  return color.replace(/^#/, '')
}

/** 主题色表（由 Viewer 在渲染前注入：presentation.theme.colors） */
let THEME_COLORS: Record<string, string> = {}
/** 方案色别名（OOXML 的 tx1/bg1 等与主题表 dk1/lt1 的对应） */
const SCHEME_ALIAS: Record<string, string> = {
  tx1: 'dk1',
  tx2: 'dk2',
  bg1: 'lt1',
  bg2: 'lt2',
  dk1: 'dk1',
  dk2: 'dk2',
  lt1: 'lt1',
  lt2: 'lt2',
}

/** 注入主题色（PptxViewer 调用；切换文档时重新注入） */
export function setThemeColors(colors?: Record<string, string> | null): void {
  THEME_COLORS = colors ? { ...colors } : {}
}

/** 方案色（accent1/dk1/lt1…）→ hex；非方案色返回 undefined */
export function schemeColor(name: string): string | undefined {
  const key = SCHEME_ALIAS[name] ?? name
  return THEME_COLORS[key]
}

/** 颜色：确保有 # 前缀（渲染时使用；方案色走主题表） */
export function withHash(color?: string): string {
  if (!color) return '#000000'
  if (color.startsWith('#')) return color
  if (/^[0-9a-fA-F]{6}$/.test(color)) return `#${color}`
  const themed = schemeColor(color)
  if (themed) return `#${themed.replace(/^#/, '')}`
  return '#4472C4'
}

/** 颜色：转 CSS rgba（支持 alpha 0~1）；无颜色返回 transparent（不能回退成黑色） */
export function colorWithAlpha(color?: string, alpha?: number): string {
  if (!color) return 'transparent'
  if (alpha === undefined || alpha === null) return withHash(color)
  return `${withHash(color)}${Math.round(alpha * 255)
    .toString(16)
    .padStart(2, '0')}`
}

/** 颜色：转 CSS 边框线样式 */
export function lineStyle(line?: { color: string; width: number }): string {
  if (!line) return 'none'
  return `${line.width}px solid ${withHash(line.color)}`
}

/**
 * 纯色填充颜色。
 * fill 有两种形态：字符串（模型 untagged 序列化，如 "4472C4"）或旧格式对象 { Solid: string }。
 */
export function solidFillColor(fill: unknown): string | undefined {
  if (typeof fill === 'string') return fill
  if (fill && typeof fill === 'object' && 'Solid' in fill) {
    return (fill as { Solid: string }).Solid
  }
  return undefined
}

/** 渐变填充（无则返回 null） */
export function gradientFill(fill: unknown): {
  stops: { color: string; position: number }[]
  angle?: number | null
} | null {
  if (fill && typeof fill === 'object' && 'Gradient' in fill) {
    return (fill as { Gradient: { stops: { color: string; position: number }[]; angle?: number | null } })
      .Gradient
  }
  return null
}

/** 阴影 → CSS box-shadow */
export function shadowToCss(shadow?: {
  blur: number
  distance: number
  angle: number
  opacity: number
  color: string
}): string {
  if (!shadow) return 'none'
  const rad = (shadow.angle * Math.PI) / 180
  const x = Math.round(Math.cos(rad) * shadow.distance)
  const y = Math.round(Math.sin(rad) * shadow.distance)
  return `${x}px ${y}px ${shadow.blur}px ${Math.round(shadow.opacity * 100)}% ${withHash(shadow.color)}`
}

/** 数值保留 2 位小数（写回 JSON 前规范化） */
export function round2(n: number): number {
  return Math.round(n * 100) / 100
}
