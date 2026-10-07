//! 行虚拟化布局计算（纯函数，便于单测）
//!
//! 目标：百万行量级的工作表只渲染视口内的行，滚动位置由「前缀高度」精确映射。
//! 大多数行使用默认行高，少数显式行高按稀疏增量记录，避免为每行存一个偏移量。

export interface RowLayout {
  /** 默认行高（px） */
  defaultH: number
  /** 行号 → 显式行高（px），仅记录非默认行 */
  explicit: Map<number, number>
  /** 按行号升序的累计增量（显式行高相对默认值的差） */
  deltas: { row: number; cum: number }[]
  /** 总高（px） */
  total: number
  /** 行数 */
  count: number
}

/**
 * 构建行布局。
 * @param rowIds 升序去重的行号（1-based）
 * @param heights 行号 → 行高（缺省用 defaultH）
 */
export function buildRowLayout(rowIds: number[], heights: Map<number, number>, defaultH: number): RowLayout {
  const explicit = new Map<number, number>()
  const deltas: { row: number; cum: number }[] = []
  let cum = 0
  rowIds.forEach((rowNo, pos) => {
    const h = heights.get(rowNo)
    if (h !== undefined && Math.abs(h - defaultH) > 1e-6) {
      explicit.set(pos, h) // 键为 0-based 行序，与 rowTop/rowHeight 一致
      cum += h - defaultH
      deltas.push({ row: pos, cum })
    }
  })
  const total = rowIds.length * defaultH + cum
  return { defaultH, explicit, deltas, total, count: rowIds.length }
}

/** pos（0-based 行序）之前的累计高度（px）；deltas 亦以 0-based 行序为键 */
export function rowTop(layout: RowLayout, pos: number): number {
  if (pos <= 0) return 0
  // 二分：只用「pos 之前」的增量（rowTop(pos) = Σ_{i<pos} h(i)）
  let lo = 0
  let hi = layout.deltas.length - 1
  let acc = 0
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (layout.deltas[mid].row < pos) {
      acc = layout.deltas[mid].cum
      lo = mid + 1
    } else {
      hi = mid - 1
    }
  }
  return pos * layout.defaultH + acc
}

/** 某行（0-based 行序）的高度 */
export function rowHeight(layout: RowLayout, pos: number): number {
  const h = layout.explicit.get(pos)
  return h === undefined ? layout.defaultH : h
}

/** 已知滚动位置时，落在视口内的行序区间 [start, end]（含端点，0-based） */
export function visibleRows(
  layout: RowLayout,
  scrollTop: number,
  viewportH: number,
  overscan = 6,
): [number, number] {
  if (layout.count === 0) return [0, -1]
  const top = Math.max(0, scrollTop)
  const bottom = top + Math.max(1, viewportH)
  // 二分定位起始行：最后一个 rowTop(pos) <= top
  let lo = 0
  let hi = layout.count - 1
  let start = 0
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    if (rowTop(layout, mid) <= top) {
      start = mid
      lo = mid + 1
    } else {
      hi = mid - 1
    }
  }
  let end = start
  while (end + 1 < layout.count && rowTop(layout, end + 1) < bottom) end += 1
  return [Math.max(0, start - overscan), Math.min(layout.count - 1, end + overscan)]
}

/**
 * 画布高度上限：浏览器对可滚动区域的有效上限远小于元素高度上限
 * （Chrome 实测 scrollHeight 被钳制在 2^24-1 = 16,777,215 px），
 * 超过则按比例压缩画布高度，滚动位置同步换算，保证百万行都能滚到。
 */
export const MAX_CANVAS_H = 16_000_000

/** 超高表：画布高度的压缩比例（1 = 不压缩）；压缩后需按同一比例换算滚动位置 */
export function canvasScale(totalH: number): number {
  return totalH > MAX_CANVAS_H ? MAX_CANVAS_H / totalH : 1
}

/** 单元格引用 → (col, row)：手写解析（正则版在百万单元格场景明显偏慢） */
export function refToPos(ref: string): { col: number; row: number } | null {
  let i = 0
  const n = ref.length
  if (i < n && ref.charCodeAt(i) === 36) i += 1 // $
  let col = 0
  let letters = 0
  while (i < n) {
    const c = ref.charCodeAt(i)
    let v = 0
    if (c >= 65 && c <= 90) v = c - 64
    else if (c >= 97 && c <= 122) v = c - 96
    else break
    col = col * 26 + v
    letters += 1
    i += 1
  }
  if (letters === 0) return null
  if (i < n && ref.charCodeAt(i) === 36) i += 1 // $
  let row = 0
  let digits = 0
  while (i < n) {
    const c = ref.charCodeAt(i)
    if (c < 48 || c > 57) return null
    row = row * 10 + (c - 48)
    digits += 1
    i += 1
  }
  if (digits === 0) return null
  return { col, row }
}

/** 统计用：给定布局与视口，说明当前只渲染了多少行 */
export function renderedRowCount(layout: RowLayout, scrollTop: number, viewportH: number, overscan = 6): number {
  const [a, b] = visibleRows(layout, scrollTop, viewportH, overscan)
  return Math.max(0, b - a + 1)
}
