//! 列式打包：把「每单元格一个 JS 对象」的工作表压成 TypedArray + 字符串池
//!
//! 动机：产物 JSON 模型约 140B/单元格（100 万行 × 6 列 ≈ 800MB），是百万行场景的真正瓶颈。
//! 列式形态每单元格 ≈ 21B（列号 2B + 数值 8B + 字符串池下标 4B + 格式池下标 4B + 类型 1B + 行前缀摊销），
//! 字符串与数字格式去重后只存一份；公式/样式/批注等低频字段走稀疏 side-map（多数表为空）。
//!
//! 打包后调用方可以清空 `sheet.rows` 丢弃对象模型——渲染器只通过 `SheetAdapter` 取数，
//! 不再依赖原始行数组。`SheetAdapter` 同时支持「JSON 原样」与「已打包」两种形态。

import type { XlsxCell, XlsxSheet } from '@/types'
import { refToPos } from './virtual'

/** 单元格类型码 */
export const KIND = { empty: 0, number: 1, string: 2, bool: 3 } as const

/** 低频字段（公式/样式/链接/批注等）：只对有值的单元格记录，稀疏存储 */
export interface CellExtras {
  formula?: string
  cell_type?: string
  style?: unknown
  link?: string
  link_tooltip?: string
  comment?: string
}

export interface PackedSheet {
  name: string
  /** 行号（1-based，升序） */
  rowNos: Int32Array
  /** 第 i 行单元格在数组中的起点，长度 = rowNos.length + 1 */
  rowStart: Uint32Array
  /** 列号（1-based） */
  cols: Uint16Array
  /** 数值（KIND.number 时有意义） */
  nums: Float64Array
  /** 字符串池下标（-1 表示非字符串） */
  strs: Int32Array
  /** 布尔值（KIND.bool 时有意义） */
  bools: Uint8Array
  /** 数字格式池下标（-1 表示无） */
  fmts: Int32Array
  /** 类型码 */
  kinds: Uint8Array
  strings: string[]
  formats: string[]
  /** 单元格下标 → 低频字段（公式/样式/链接/批注），无则不占条目 */
  extras: Map<number, CellExtras>
  /** 行高（px，0 表示默认） */
  heights: Float32Array
  defaultRowPx: number
  maxCol: number
  cellCount: number
  /** 工作表元信息（不含 rows：合并/图表/图片/列宽/打印/网格线等） */
  meta: XlsxSheet
}

export interface PackResult {
  packed: PackedSheet
  packMs: number
  /** 估算字节数（TypedArray + 字符串池 + 稀疏 extras） */
  bytes: number
}


/** 类型码对应的默认 cell_type：与之相同则无需进 extras（省 6M 条 Map 条目） */
function defaultTypeOf(kind: number): string {
  return kind === KIND.number ? 'number' : kind === KIND.string ? 'string' : kind === KIND.bool ? 'boolean' : ''
}

/** 去掉 rows 的工作表元信息（保留合并/图表/图片/列宽/打印等小字段） */
export function sheetMeta(sheet: XlsxSheet): XlsxSheet {
  return { ...sheet, rows: [] }
}

/** 打包：XlsxSheet → PackedSheet */
export function packSheet(sheet: XlsxSheet, defaultRowPx: number): PackResult {
  const t0 = typeof performance !== 'undefined' ? performance.now() : 0
  const rows = sheet.rows ?? []
  const n0 = rows.length
  const rowNos = new Int32Array(n0)
  const rowStart = new Uint32Array(n0 + 1)
  const heights = new Float32Array(n0)

  let n = 0
  let maxCol = 0
  let ri = 0
  for (const row of rows) {
    rowNos[ri] = row.index ?? ri + 1
    heights[ri] = row.height ? (row.height * 96) / 72 : 0
    const cells = row.cells ?? []
    n += cells.length
    for (const c of cells) {
      if (c.reference) {
        const p = refToPos(c.reference)
        if (p && p.col > maxCol) maxCol = p.col
      }
    }
    rowStart[ri + 1] = n
    ri += 1
  }
  if (maxCol === 0 && n > 0) {
    let mx = 0
    for (const row of rows) mx = Math.max(mx, row.cells?.length ?? 0)
    maxCol = mx
  }
  for (const m of sheet.merges ?? []) {
    const mm = /^([A-Za-z]+)\d+(?::([A-Za-z]+)\d+)?$/.exec(m)
    if (mm) {
      let col = 0
      for (const ch of (mm[2] ?? mm[1]).toUpperCase()) col = col * 26 + (ch.charCodeAt(0) - 64)
      if (col > maxCol) maxCol = col
    }
  }

  const cols = new Uint16Array(n)
  const nums = new Float64Array(n)
  const strs = new Int32Array(n).fill(-1)
  const bools = new Uint8Array(n)
  const fmts = new Int32Array(n).fill(-1)
  const kinds = new Uint8Array(n)
  const strings: string[] = []
  const formats: string[] = []
  const extras = new Map<number, CellExtras>()
  const strIndex = new Map<string, number>()
  const fmtIndex = new Map<string, number>()

  let k = 0
  ri = 0
  for (const row of rows) {
    const cells = row.cells ?? []
    let pos = 1
    for (const c of cells) {
      let col = pos
      if (c.reference) {
        const p = refToPos(c.reference)
        if (p) col = p.col
      }
      pos = col + 1
      cols[k] = col
      const v = c.value
      if (typeof v === 'number') {
        kinds[k] = KIND.number
        nums[k] = v
      } else if (typeof v === 'boolean') {
        kinds[k] = KIND.bool
        bools[k] = v ? 1 : 0
      } else if (typeof v === 'string') {
        kinds[k] = KIND.string
        let id = strIndex.get(v)
        if (id === undefined) {
          id = strings.length
          strings.push(v)
          strIndex.set(v, id)
        }
        strs[k] = id
      } else {
        kinds[k] = KIND.empty
      }
      if (c.number_format) {
        let id = fmtIndex.get(c.number_format)
        if (id === undefined) {
          id = formats.length
          formats.push(c.number_format)
          fmtIndex.set(c.number_format, id)
        }
        fmts[k] = id
      }
      const extraType =
        c.cell_type && c.cell_type !== defaultTypeOf(kinds[k]) ? c.cell_type : undefined
      if (c.formula || c.style || c.link || c.link_tooltip || c.comment || extraType) {
        extras.set(k, {
          formula: c.formula ?? undefined,
          cell_type: extraType ?? undefined,
          style: c.style ?? undefined,
          link: c.link ?? undefined,
          link_tooltip: c.link_tooltip ?? undefined,
          comment: c.comment ?? undefined,
        })
      }
      k += 1
    }
    ensureRowSorted({ cols, nums, strs, bools, fmts, kinds, extras }, rowStart[ri], k)
    rowStart[ri + 1] = k
    ri += 1
  }

  let extraBytes = 0
  extras.forEach((e) => {
    extraBytes += 80
    for (const v of [e.formula, e.cell_type, e.link, e.link_tooltip, e.comment]) {
      if (typeof v === 'string') extraBytes += v.length * 2 + 24
    }
  })
  const bytes =
    rowNos.byteLength +
    rowStart.byteLength +
    cols.byteLength +
    nums.byteLength +
    strs.byteLength +
    bools.byteLength +
    fmts.byteLength +
    kinds.byteLength +
    heights.byteLength +
    strings.reduce((a, s) => a + s.length * 2 + 40, 0) +
    formats.reduce((a, s) => a + s.length * 2 + 40, 0) +
    extraBytes

  const t1 = typeof performance !== 'undefined' ? performance.now() : 0
  return {
    packed: {
      name: sheet.name,
      rowNos,
      rowStart,
      cols,
      nums,
      strs,
      bools,
      fmts,
      kinds,
      strings,
      formats,
      extras,
      heights,
      defaultRowPx,
      maxCol,
      cellCount: n,
      meta: sheetMeta(sheet),
    },
    packMs: Math.round((t1 - t0) * 10) / 10,
    bytes,
  }
}


/** 并行数组视图（行内有序化用） */
interface CellArrays {
  cols: Uint16Array
  nums: Float64Array
  strs: Int32Array
  bools: Uint8Array
  fmts: Int32Array
  kinds: Uint8Array
  extras?: Map<number, CellExtras>
}

/**
 * 保证行内单元格按列号升序（真实文件里同一行的单元格顺序不保证升序，
 * 而行内定位用二分，必须有序）。仅在不满足时做一次小范围排序。
 */
export function ensureRowSorted(a: CellArrays, s0: number, s1: number): void {
  let sorted = true
  for (let i = s0 + 1; i < s1; i += 1) {
    if (a.cols[i] < a.cols[i - 1]) {
      sorted = false
      break
    }
  }
  if (sorted) return
  const order: number[] = []
  for (let i = s0; i < s1; i += 1) order.push(i)
  order.sort((x, y) => a.cols[x] - a.cols[y])
  const snapshot = order.map((i) => ({
    col: a.cols[i],
    num: a.nums[i],
    str: a.strs[i],
    bool: a.bools[i],
    fmt: a.fmts[i],
    kind: a.kinds[i],
    extra: a.extras?.get(i),
  }))
  snapshot.forEach((v, k) => {
    const i = s0 + k
    a.cols[i] = v.col
    a.nums[i] = v.num
    a.strs[i] = v.str
    a.bools[i] = v.bool
    a.fmts[i] = v.fmt
    a.kinds[i] = v.kind
    if (a.extras) {
      if (v.extra) a.extras.set(i, v.extra)
      else if (i !== s0 + k || !v.extra) a.extras.delete(i)
    }
  })
  // extras 需要按新下标重排
  if (a.extras) {
    const moved = new Map<number, CellExtras>()
    snapshot.forEach((v, k) => {
      if (v.extra) moved.set(s0 + k, v.extra)
    })
    for (let i = s0; i < s1; i += 1) a.extras.delete(i)
    moved.forEach((v, i) => a.extras!.set(i, v))
  }
}

/** 行内二分：单元格下标（-1 = 不存在） */
function posInRow(p: PackedSheet, rowPos: number, col: number): number {
  let lo = p.rowStart[rowPos]
  let hi = p.rowStart[rowPos + 1] - 1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    const c = p.cols[mid]
    if (c === col) return mid
    if (c < col) lo = mid + 1
    else hi = mid - 1
  }
  return -1
}

/** 行号 → 行序（二分；-1 = 不存在） */
export function packedRowPos(p: PackedSheet, rowNo: number): number {
  let lo = 0
  let hi = p.rowNos.length - 1
  while (lo <= hi) {
    const mid = (lo + hi) >> 1
    const v = p.rowNos[mid]
    if (v === rowNo) return mid
    if (v < rowNo) lo = mid + 1
    else hi = mid - 1
  }
  return -1
}

/** 打包形态下按单元格下标取单元格（含低频字段拼装） */
export function packedCellAtPos(p: PackedSheet, pos: number): XlsxCell | null {
  if (pos < 0) return null
  const kind = p.kinds[pos]
  let value: string | number | boolean | null = null
  if (kind === KIND.number) value = p.nums[pos]
  else if (kind === KIND.string) value = p.strings[p.strs[pos]]
  else if (kind === KIND.bool) value = p.bools[pos] === 1
  const number_format = p.fmts[pos] >= 0 ? p.formats[p.fmts[pos]] : null
  const extra = p.extras.get(pos)
  return {
    value,
    number_format,
    formula: extra?.formula ?? null,
    cell_type: extra?.cell_type ?? null,
    style: (extra?.style as XlsxCell['style']) ?? null,
    link: extra?.link ?? null,
    link_tooltip: extra?.link_tooltip ?? null,
    comment: extra?.comment ?? null,
  }
}

/** 打包形态下按行列取单元格 */
export function packedCellAt(p: PackedSheet, rowNo: number, col: number): XlsxCell | null {
  const rp = packedRowPos(p, rowNo)
  if (rp < 0) return null
  return packedCellAtPos(p, posInRow(p, rp, col))
}

export function isPackedSheet(s: unknown): s is PackedSheet {
  return !!s && typeof s === 'object' && 'rowStart' in (s as Record<string, unknown>)
}

/** 打包体积（MB） */
export function packedMB(bytes: number): number {
  return Math.round((bytes / 1048576) * 10) / 10
}

// ---------------------------------------------------------------- adapter ---

/** 渲染器统一取数接口：JSON 原样 / 已打包 两种形态共用 */
export interface SheetAdapter {
  name: string
  /** 工作表元信息（不含 rows） */
  meta: XlsxSheet
  maxCol: number
  /** 行号（1-based 升序） */
  rowNos: Int32Array | number[]
  rowCount: number
  /** 按行号取行高（px；缺省行高由布局兜底） */
  heightOfRow(rowNo: number): number | undefined
  /** 按行列取单元格（低频字段完整） */
  cell(rowNo: number, col: number): XlsxCell | undefined
}

/** JSON 形态适配器（不复制数据，行数组原样使用） */
export function jsonAdapter(sheet: XlsxSheet, _defaultRowPx: number): SheetAdapter {
  const rowNos: number[] = []
  const heights = new Map<number, number>()
  let maxCol = 0
  for (const row of sheet.rows ?? []) {
    const idx = row.index ?? 0
    if (idx > 0) {
      rowNos.push(idx)
      if (row.height) heights.set(idx, (row.height * 96) / 72)
    }
    let next = 1
    for (const c of row.cells ?? []) {
      let col = next
      if (c.reference) {
        const p = refToPos(c.reference)
        if (p) col = p.col
      }
      next = col + 1
      if (col > maxCol) maxCol = col
    }
  }
  for (const m of sheet.merges ?? []) {
    const mm = /^([A-Za-z]+)\d+(?::([A-Za-z]+)\d+)?$/.exec(m)
    if (mm) {
      let col = 0
      for (const ch of (mm[2] ?? mm[1]).toUpperCase()) col = col * 26 + (ch.charCodeAt(0) - 64)
      if (col > maxCol) maxCol = col
    }
  }
  rowNos.sort((a, b) => a - b)
  const byRow = new Map<number, NonNullable<XlsxSheet['rows'][number]>>()
  for (const row of sheet.rows ?? []) byRow.set(row.index ?? 0, row)
  const cellInRow = (rowNo: number, col: number) => {
    const row = byRow.get(rowNo)
    if (!row) return undefined
    let next = 1
    for (const c of row.cells ?? []) {
      let cc = next
      if (c.reference) {
        const p = refToPos(c.reference)
        if (p) cc = p.col
      }
      next = cc + 1
      if (cc === col) return c
    }
    return undefined
  }
  return {
    name: sheet.name,
    meta: sheet,
    maxCol,
    rowNos,
    rowCount: rowNos.length,
    heightOfRow: (r) => heights.get(r),
    cell: cellInRow,
  }
}

/** 打包形态适配器 */
export function packedAdapter(p: PackedSheet): SheetAdapter {
  const rowNos = p.rowNos
  const heights = p.heights
  return {
    name: p.name,
    meta: p.meta,
    maxCol: p.maxCol,
    rowNos,
    rowCount: rowNos.length,
    heightOfRow: (rowNo) => {
      const rp = packedRowPos(p, rowNo)
      if (rp < 0) return undefined
      const h = heights[rp]
      return h > 0 ? h : undefined
    },
    cell: (rowNo, col) => packedCellAt(p, rowNo, col) ?? undefined,
  }
}

/** 统一入口：按工作表形态选择适配器 */
export function adapterFor(
  sheet: XlsxSheet | PackedSheet,
  defaultRowPx: number,
): SheetAdapter {
  return isPackedSheet(sheet) ? packedAdapter(sheet) : jsonAdapter(sheet, defaultRowPx)
}
