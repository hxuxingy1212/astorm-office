//! 单位换算：与各 CLI 渲染端保持一致（英寸/磅/EMU → CSS px，字符宽 → px）

/** 96dpi 下 1 英寸 = 96px */
export const inchToPx = (inch: number): number => inch * 96
/** 磅 → px（96/72） */
export const ptToPx = (pt: number): number => (pt * 96) / 72
/** EMU → px（914400 EMU = 1 英寸） */
export const emuToPx = (emu: number): number => (emu / 914400) * 96
/** 列宽（字符）→ px：Excel 近似 7px/字符 + 5px 内边距 */
export const colCharsToPx = (chars: number): number => Math.round(chars * 7) + 5
/** 行高（磅）→ px */
export const rowPtToPx = (pt: number): number => Math.round(ptToPx(pt))

/** 颜色归一化：去 #、8 位 ARGB 取后 6 位 */
export function normalizeColor(c?: string | null): string {
  if (!c) return ''
  const s = String(c).trim().replace(/^#/, '').toUpperCase()
  return s.length === 8 ? s.slice(2) : s
}

/** 带 # 的 CSS 颜色 */
export function cssColor(c?: string | null): string {
  const n = normalizeColor(c)
  return n ? `#${n}` : ''
}

/** 数字裁剪：整数不带小数点 */
export function trimNum(n: number): string {
  return Number.isInteger(n) ? String(n) : String(n)
}
