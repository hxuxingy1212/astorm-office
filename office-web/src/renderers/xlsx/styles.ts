//! xlsx 单元格样式 → CSS（与 ai-excel/cli/src/render.rs 的 css() 对齐并扩展：分侧边框、垂直对齐、换行）
import type { XlsxStyle, XlsxWorkbook } from '@/types'
import { cssColor, ptToPx } from '@/core/units'

export function resolveStyle(wb: XlsxWorkbook | undefined, style: unknown): XlsxStyle | undefined {
  if (!style) return undefined
  if (typeof style === 'string') return wb?.styles?.[style]
  if (typeof style === 'object') return style as XlsxStyle
  return undefined
}

const BORDER_CSS: Record<string, string> = {
  thin: '1px solid',
  medium: '2px solid',
  thick: '3px solid',
  dashed: '1px dashed',
  dotted: '1px dotted',
  double: '3px double',
  hair: '1px solid',
  mediumDashed: '2px dashed',
  dashDot: '1px dashed',
  mediumDashDot: '2px dashed',
  dashDotDot: '1px dotted',
  slantDashDot: '1px dashed',
}

function borderStyle(kind?: string | null, color?: string | null): string | undefined {
  if (!kind || kind === 'none') return undefined
  const c = cssColor(color) || '#999'
  const base = BORDER_CSS[kind] ?? '1px solid'
  return `${base} ${c}`
}

export function styleToCss(s?: XlsxStyle): Record<string, string> {
  const out: Record<string, string> = {}
  if (!s) return out
  const f = s.font
  if (f) {
    if (f.name) out['font-family'] = `${f.name}, Calibri, Arial, sans-serif`
    if (f.size) out['font-size'] = `${ptToPx(f.size)}px`
    if (f.bold) out['font-weight'] = 'bold'
    if (f.italic) out['font-style'] = 'italic'
    if (f.strike) out['text-decoration'] = 'line-through'
    if (f.underline) out['text-decoration'] = out['text-decoration'] ? `${out['text-decoration']} underline` : 'underline'
    if (f.color) out['color'] = cssColor(f.color)
  }
  if (s.fill && s.fill !== 'GRAY125') out['background-color'] = cssColor(s.fill)
  if (s.fill === 'GRAY125') out['background-image'] = 'repeating-linear-gradient(45deg,#bfbfbf 0 2px,#fff 2px 4px)'
  const b = s.border
  if (b) {
    const all = borderStyle(b.all, b.color)
    if (all) out['border'] = all
    const sides: [string, string | null | undefined][] = [
      ['top', b.top],
      ['bottom', b.bottom],
      ['left', b.left],
      ['right', b.right],
    ]
    for (const [side, kind] of sides) {
      const v = borderStyle(kind, b.color)
      if (v) out[`border-${side}`] = v
    }
  }
  const a = s.alignment
  if (a) {
    if (a.horizontal) out['text-align'] = a.horizontal === 'justify' ? 'justify' : a.horizontal
    if (a.vertical) {
      out['vertical-align'] = a.vertical === 'center' ? 'middle' : a.vertical
    }
    if (a.wrap_text) out['white-space'] = 'pre-wrap'
  }
  if (s.locked === false) out['color'] = out['color'] ?? 'inherit'
  return out
}
