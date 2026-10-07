//! 范围解析与数值提取（xlsx 图表/合并用）

export interface RangeParts {
  c1: number
  r1: number
  c2: number
  r2: number
}

/** 列字母 → 列号（1-based） */
export function colToIndex(letters: string): number {
  let n = 0
  for (const ch of letters.toUpperCase()) n = n * 26 + (ch.charCodeAt(0) - 64)
  return n
}

/** `A1:C10` / `Sheet1!A1:C10` → 范围（失败返回 null） */
export function parseRange(ref: string): RangeParts | null {
  const clean = ref.includes('!') ? ref.slice(ref.lastIndexOf('!') + 1) : ref
  const m = /^\$?([A-Za-z]+)\$?(\d+)(?::\$?([A-Za-z]+)\$?(\d+))?$/.exec(clean.trim())
  if (!m) return null
  const c1 = colToIndex(m[1])
  const r1 = Number(m[2])
  const c2 = m[3] ? colToIndex(m[3]) : c1
  const r2 = m[4] ? Number(m[4]) : r1
  return { c1, r1, c2, r2 }
}

/** 单格引用 → (col, row) */
export function parseCell(ref: string): { col: number; row: number } | null {
  const m = /^\$?([A-Za-z]+)\$?(\d+)$/.exec(ref.trim())
  if (!m) return null
  return { col: colToIndex(m[1]), row: Number(m[2]) }
}
