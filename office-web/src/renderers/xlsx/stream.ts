//! 流式打包：边读边解析工作表 JSON，直接写入列式数组
//!
//! 动机：`JSON.parse(整个 sheet.json)` 会在解析期产生「每单元格一个临时对象」的峰值
//! （600 万单元格 ≈ 800MB 临时对象 + 数百 MB 文本），是打包之后的下一个内存瓶颈。
//!
//! 做法：按「行对象」为粒度流式扫描——在字符流上做括号匹配（识别字符串与转义），
//! 每当一个完整的行对象出现，就单独 `JSON.parse` 该行（一个行对象只有几十字节，
//! 临时对象数量被限制在单行规模），随即写入列式数组并丢弃。峰值内存 ≈ 分块文本 + 列式数组。
//!
//! 头部（rows 之前的字段）与尾部（rows 之后的字段：merges/charts/images/columns/print 等）
//! 分别累积后各解析一次（体积都很小）。

import type { XlsxSheet } from '@/types'
import { ensureRowSorted, KIND, sheetMeta, type CellExtras, type PackResult, type PackedSheet } from './packed'
import { refToPos } from './virtual'

interface RawCell {
  reference?: string
  value?: unknown
  number_format?: string
  formula?: string
  cell_type?: string
  style?: unknown
  link?: string
  link_tooltip?: string
  comment?: string
}

interface RawRow {
  index?: number
  height?: number | null
  cells?: RawCell[]
}

/** 可增长的列式写入器（未知总规模，按块扩容） */
class PackedWriter {
  private cap = 4096
  private n = 0
  rowNos: number[] = []
  rowStart: number[] = [0]
  cols = new Uint16Array(this.cap)
  nums = new Float64Array(this.cap)
  strs = new Int32Array(this.cap).fill(-1)
  bools = new Uint8Array(this.cap)
  fmts = new Int32Array(this.cap).fill(-1)
  kinds = new Uint8Array(this.cap)
  heights: number[] = []
  strings: string[] = []
  formats: string[] = []
  extras = new Map<number, CellExtras>()
  private strIndex = new Map<string, number>()
  private fmtIndex = new Map<string, number>()
  maxCol = 0

  private grow(): void {
    const cap = this.cap * 2
    const cols = new Uint16Array(cap)
    cols.set(this.cols)
    const nums = new Float64Array(cap)
    nums.set(this.nums)
    const strs = new Int32Array(cap).fill(-1)
    strs.set(this.strs)
    const bools = new Uint8Array(cap)
    bools.set(this.bools)
    const fmts = new Int32Array(cap).fill(-1)
    fmts.set(this.fmts)
    const kinds = new Uint8Array(cap)
    kinds.set(this.kinds)
    this.cols = cols
    this.nums = nums
    this.strs = strs
    this.bools = bools
    this.fmts = fmts
    this.kinds = kinds
    this.cap = cap
  }

  /** 字段级扫描路径：把复用 cell 对象写入数组 */
  pushParsedCell(cell: ParsedCell): void {
    if (this.n >= this.cap) this.grow()
    const k = this.n
    this.cols[k] = cell.col
    if (cell.col > this.maxCol) this.maxCol = cell.col
    this.kinds[k] = cell.kind
    if (cell.kind === KIND.number) this.nums[k] = cell.num
    else if (cell.kind === KIND.bool) this.bools[k] = cell.bool ? 1 : 0
    else if (cell.kind === KIND.string && cell.str !== null) {
      let id = this.strIndex.get(cell.str)
      if (id === undefined) {
        id = this.strings.length
        this.strings.push(cell.str)
        this.strIndex.set(cell.str, id)
      }
      this.strs[k] = id
    }
    if (cell.fmt) {
      let id = this.fmtIndex.get(cell.fmt)
      if (id === undefined) {
        id = this.formats.length
        this.formats.push(cell.fmt)
        this.fmtIndex.set(cell.fmt, id)
      }
      this.fmts[k] = id
    }
    if (cell.hasExtra) {
      this.extras.set(k, {
        formula: cell.formula ?? undefined,
        cell_type: cell.cellType ?? undefined,
        style: cell.style,
        link: cell.link ?? undefined,
        link_tooltip: cell.linkTooltip ?? undefined,
        comment: cell.comment ?? undefined,
      })
    }
    this.n += 1
  }

  /** 行尾：写入行号/行高并做行内有序化 */
  endParsedRow(rowNo: number, heightPx: number): void {
    this.rowNos.push(rowNo)
    this.heights.push(heightPx)
    const s0 = this.rowStart[this.rowStart.length - 1]
    ensureRowSorted(
      {
        cols: this.cols,
        nums: this.nums,
        strs: this.strs,
        bools: this.bools,
        fmts: this.fmts,
        kinds: this.kinds,
        extras: this.extras,
      },
      s0,
      this.n,
    )
    this.rowStart.push(this.n)
  }

  pushRow(row: RawRow, rowNo: number, heightPx: number): void {
    this.rowNos.push(rowNo)
    this.heights.push(heightPx)
    const cells = row.cells ?? []
    let pos = 1
    for (const c of cells) {
      let col = pos
      if (c.reference) {
        const p = refToPos(c.reference)
        if (p) col = p.col
      }
      pos = col + 1
      if (this.n >= this.cap) this.grow()
      const k = this.n
      this.cols[k] = col
      if (col > this.maxCol) this.maxCol = col
      const v = c.value
      if (typeof v === 'number') {
        this.kinds[k] = KIND.number
        this.nums[k] = v
      } else if (typeof v === 'boolean') {
        this.kinds[k] = KIND.bool
        this.bools[k] = v ? 1 : 0
      } else if (typeof v === 'string') {
        this.kinds[k] = KIND.string
        let id = this.strIndex.get(v)
        if (id === undefined) {
          id = this.strings.length
          this.strings.push(v)
          this.strIndex.set(v, id)
        }
        this.strs[k] = id
      } else {
        this.kinds[k] = KIND.empty
      }
      if (c.number_format) {
        let id = this.fmtIndex.get(c.number_format)
        if (id === undefined) {
          id = this.formats.length
          this.formats.push(c.number_format)
          this.fmtIndex.set(c.number_format, id)
        }
        this.fmts[k] = id
      }
      const kindNow = this.kinds[k]
      const defaultType =
        kindNow === KIND.number ? 'number' : kindNow === KIND.string ? 'string' : kindNow === KIND.bool ? 'boolean' : ''
      const extraType = c.cell_type && c.cell_type !== defaultType ? c.cell_type : undefined
      if (c.formula || c.style || c.link || c.link_tooltip || c.comment || extraType) {
        this.extras.set(k, {
          formula: c.formula,
          cell_type: extraType,
          style: c.style,
          link: c.link,
          link_tooltip: c.link_tooltip,
          comment: c.comment,
        })
      }
      this.n += 1
    }
    const s0 = this.rowStart[this.rowStart.length - 1]
    ensureRowSorted(
      {
        cols: this.cols,
        nums: this.nums,
        strs: this.strs,
        bools: this.bools,
        fmts: this.fmts,
        kinds: this.kinds,
        extras: this.extras,
      },
      s0,
      this.n,
    )
    this.rowStart.push(this.n)
  }

  finish(name: string, defaultRowPx: number, meta: XlsxSheet): PackResult {
    const n = this.n
    const packed: PackedSheet = {
      name,
      rowNos: Int32Array.from(this.rowNos),
      rowStart: Uint32Array.from(this.rowStart),
      cols: this.cols.subarray(0, n),
      nums: this.nums.subarray(0, n),
      strs: this.strs.subarray(0, n),
      bools: this.bools.subarray(0, n),
      fmts: this.fmts.subarray(0, n),
      kinds: this.kinds.subarray(0, n),
      strings: this.strings,
      formats: this.formats,
      extras: this.extras,
      heights: Float32Array.from(this.heights),
      defaultRowPx,
      maxCol: this.maxCol,
      cellCount: n,
      meta,
    }
    let extraBytes = 0
    this.extras.forEach((e) => {
      extraBytes += 80
      for (const v of [e.formula, e.cell_type, e.link, e.link_tooltip, e.comment]) {
        if (typeof v === 'string') extraBytes += v.length * 2 + 24
      }
    })
    const bytes =
      packed.rowNos.byteLength +
      packed.rowStart.byteLength +
      packed.cols.byteLength +
      packed.nums.byteLength +
      packed.strs.byteLength +
      packed.bools.byteLength +
      packed.fmts.byteLength +
      packed.kinds.byteLength +
      packed.heights.byteLength +
      this.strings.reduce((a, s) => a + s.length * 2 + 40, 0) +
      this.formats.reduce((a, s) => a + s.length * 2 + 40, 0) +
      extraBytes
    return { packed, packMs: 0, bytes }
  }
}


// ---------------------------------------------------------------- 字段级扫描 ---
// ---- 零分配键分类：避免每个键 slice 出字符串（6M 键 → 6M 次小字符串分配） ----
const KEY = {
  reference: 1,
  value: 2,
  numberFormat: 3,
  formula: 4,
  cellType: 5,
  link: 6,
  linkTooltip: 7,
  comment: 8,
  style: 9,
  index: 10,
  height: 11,
  cells: 12,
  unknown: 0,
} as const

/** 读取键（不含引号）并按 (长度, 首/次/末字符) 分类；返回分类码与结束下标 */
export function keyCodeAt(s: string, i: number): { code: number; end: number } {
  let j = i + 1
  let esc = false
  while (j < s.length) {
    const c = s.charCodeAt(j)
    if (esc) esc = false
    else if (c === 92) esc = true
    else if (c === 34) break
    j += 1
  }
  const end = j + 1
  const len = j - i - 1
  const c1 = s.charCodeAt(i + 1)
  const c2 = len > 1 ? s.charCodeAt(i + 2) : 0
  const cl = len > 2 ? s.charCodeAt(j - 1) : 0
  // 已知键集合内 (len, c1, c2, cl) 唯一
  if (len === 9 && c1 === 114) return { code: KEY.reference, end }
  if (len === 5 && c1 === 118) return { code: KEY.value, end }
  if (len === 13 && c1 === 110) return { code: KEY.numberFormat, end }
  if (len === 7 && c1 === 102) return { code: KEY.formula, end }
  if (len === 9 && c1 === 99) return { code: KEY.cellType, end }
  if (len === 4 && c1 === 108) return { code: KEY.link, end }
  if (len === 12 && c1 === 108 && cl === 112) return { code: KEY.linkTooltip, end }
  if (len === 7 && c1 === 99) return { code: KEY.comment, end }
  if (len === 5 && c1 === 115) return { code: KEY.style, end }
  if (len === 5 && c1 === 105) return { code: KEY.index, end }
  if (len === 6 && c1 === 104) return { code: KEY.height, end }
  if (len === 5 && c1 === 99) return { code: KEY.cells, end }
  void c2
  return { code: KEY.unknown, end }
}

//
// 目标：单遍扫描行文本，直接产出 `ParsedCell`，避免「括号匹配 + 每行 JSON.parse」两遍开销。
// 基准显示逐行 JSON.parse 路径约为 V8 原生 JSON.parse 的 0.37×，本扫描器用于把差距抹平。

/** 复用的单元格记录（避免每格分配对象） */
export interface ParsedCell {
  col: number
  kind: number
  num: number
  str: string | null
  bool: boolean
  fmt: string | null
  formula: string | null
  cellType: string | null
  style: unknown
  link: string | null
  linkTooltip: string | null
  comment: string | null
  hasExtra: boolean
}

export function newParsedCell(): ParsedCell {
  return {
    col: 0,
    kind: KIND.empty,
    num: 0,
    str: null,
    bool: false,
    fmt: null,
    formula: null,
    cellType: null,
    style: undefined,
    link: null,
    linkTooltip: null,
    comment: null,
    hasExtra: false,
  }
}

/** 行级回调（复用同一对象，实现方需立即消费） */
export interface RowSink {
  cell: ParsedCell
  /** 一行开始（index 可为 null，表示未给出） */
  beginRow(index: number | null, heightPx: number): void
  endRow(): void
  /** 每个单元格解析完成后回调（可选） */
  endRowCell?(): void
}

export function skipWsAt(s: string, i: number): number {
  while (i < s.length) {
    const c = s.charCodeAt(i)
    if (c === 32 || c === 10 || c === 13 || c === 9) i += 1
    else break
  }
  return i
}

/** 读取 JSON 字符串（s[i] === '"'），返回 { value, end } */
export function readStringAt(s: string, i: number): { value: string; end: number } {
  let j = i + 1
  let simple = true
  while (j < s.length) {
    const c = s.charCodeAt(j)
    if (c === 92) {
      simple = false
      j += 2
      continue
    }
    if (c === 34) break
    j += 1
  }
  const raw = s.slice(i + 1, j)
  if (simple) return { value: raw, end: j + 1 }
  // 慢路径：处理转义（含 \uXXXX 与代理对）
  let out = ''
  for (let k = 0; k < raw.length; k += 1) {
    if (raw.charCodeAt(k) !== 92) {
      out += raw[k]
      continue
    }
    const e = raw[k + 1]
    k += 1
    switch (e) {
      case '"': out += '"'; break
      case '\\': out += '\\'; break
      case '/': out += '/'; break
      case 'b': out += '\b'; break
      case 'f': out += '\f'; break
      case 'n': out += '\n'; break
      case 'r': out += '\r'; break
      case 't': out += '\t'; break
      case 'u': {
        const hex = raw.slice(k + 1, k + 5)
        out += String.fromCharCode(parseInt(hex, 16))
        k += 4
        break
      }
      default: out += e ?? ''
    }
  }
  return { value: out, end: j + 1 }
}

/** 读取数字（含指数） */
export function readNumberAt(s: string, i: number): { value: number; end: number } {
  let j = i
  while (j < s.length) {
    const c = s.charCodeAt(j)
    if ((c >= 48 && c <= 57) || c === 43 || c === 45 || c === 46 || c === 101 || c === 69) j += 1
    else break
  }
  return { value: Number(s.slice(i, j)), end: j }
}

/** 跳过任意 JSON 值（对象/数组/字符串/数字/字面量），返回结束下标 */
export function skipValueAt(s: string, i: number): number {
  i = skipWsAt(s, i)
  const c = s[i]
  if (c === '"') return readStringAt(s, i).end
  if (c === '{' || c === '[') {
    let depth = 0
    let inStr = false
    let esc = false
    for (let j = i; j < s.length; j += 1) {
      const ch = s[j]
      if (inStr) {
        if (esc) esc = false
        else if (ch === '\\') esc = true
        else if (ch === '"') inStr = false
        continue
      }
      if (ch === '"') inStr = true
      else if (ch === '{' || ch === '[') depth += 1
      else if (ch === '}' || ch === ']') {
        depth -= 1
        if (depth === 0) return j + 1
      }
    }
    return s.length
  }
  // 数字/true/false/null
  let j = i
  while (j < s.length && !',}]'.includes(s[j])) j += 1
  return j
}

/** 解析一行（`{...}`），通过 sink 产出；返回行结束下标（失败返回 -1） */
export function parseRowAt(s: string, start: number, sink: RowSink): number {
  let i = skipWsAt(s, start)
  if (s[i] !== '{') return -1
  i += 1
  let index: number | null = null
  let heightPx = 0
  let began = false
  const cell = sink.cell
  for (;;) {
    i = skipWsAt(s, i)
    if (s[i] === '}') {
      if (!began) sink.beginRow(index, heightPx)
      sink.endRow()
      return i + 1
    }
    if (s[i] !== '"') return -1
    const key = keyCodeAt(s, i)
    i = skipWsAt(s, key.end)
    if (s[i] !== ':') return -1
    i = skipWsAt(s, i + 1)
    if (key.code === KEY.index) {
      const n = readNumberAt(s, i)
      index = n.value
      i = n.end
    } else if (key.code === KEY.height) {
      if (s[i] === 'n') {
        i += 4 // null
      } else {
        const n = readNumberAt(s, i)
        heightPx = (n.value * 96) / 72
        i = n.end
      }
    } else if (key.code === KEY.cells) {
      if (!began) {
        sink.beginRow(index, heightPx)
        began = true
      }
      if (s[i] !== '[') return -1
      i += 1
      for (;;) {
        i = skipWsAt(s, i)
        if (s[i] === ']') {
          i += 1
          break
        }
        if (s[i] === ',') {
          i += 1
          continue
        }
        if (s[i] !== '{') return -1
        i = parseCellAt(s, i, cell)
        if (i < 0) return -1
        sink.endRowCell?.()
      }
    } else {
      i = skipValueAt(s, i)
    }
    i = skipWsAt(s, i)
    if (s[i] === ',') {
      i += 1
      continue
    }
    if (s[i] === '}') {
      if (!began) sink.beginRow(index, heightPx)
      sink.endRow()
      return i + 1
    }
    if (i >= s.length) return -1
    i += 1
  }
}

/** 解析单个单元格（`{...}`）；结果写入复用的 cell 对象 */
function parseCellAt(s: string, start: number, cell: ParsedCell): number {
  cell.col = 0
  cell.kind = KIND.empty
  cell.num = 0
  cell.str = null
  cell.bool = false
  cell.fmt = null
  cell.formula = null
  cell.cellType = null
  cell.style = undefined
  cell.link = null
  cell.linkTooltip = null
  cell.comment = null
  cell.hasExtra = false
  let i = start + 1
  for (;;) {
    i = skipWsAt(s, i)
    if (s[i] === '}') return i + 1
    if (s[i] !== '"') return -1
    const key = keyCodeAt(s, i)
    i = skipWsAt(s, key.end)
    if (s[i] !== ':') return -1
    i = skipWsAt(s, i + 1)
    switch (key.code) {
      case KEY.reference: {
        const v = readStringAt(s, i)
        const p = refToPos(v.value)
        if (p) cell.col = p.col
        i = v.end
        break
      }
      case KEY.value: {
        const c = s[i]
        if (c === '"') {
          const v = readStringAt(s, i)
          cell.kind = KIND.string
          cell.str = v.value
          i = v.end
        } else if (c === 't') {
          cell.kind = KIND.bool
          cell.bool = true
          i += 4
        } else if (c === 'f') {
          cell.kind = KIND.bool
          cell.bool = false
          i += 5
        } else if (c === 'n') {
          cell.kind = KIND.empty
          i += 4
        } else if (c === '{' || c === '[') {
          // 富文本等复杂值：原样保留为 JSON 文本
          const end = skipValueAt(s, i)
          cell.kind = KIND.string
          cell.str = s.slice(i, end)
          cell.cellType = 'json'
          cell.hasExtra = true
          i = end
        } else {
          const n = readNumberAt(s, i)
          cell.kind = KIND.number
          cell.num = n.value
          i = n.end
        }
        break
      }
      case KEY.numberFormat: {
        const v = readStringAt(s, i)
        cell.fmt = v.value
        i = v.end
        break
      }
      case KEY.formula: {
        if (s[i] === 'n') {
          i += 4
        } else {
          const v = readStringAt(s, i)
          cell.formula = v.value
          cell.hasExtra = true
          i = v.end
        }
        break
      }
      case KEY.cellType: {
        const v = readStringAt(s, i)
        cell.cellType = v.value
        cell.hasExtra = true
        i = v.end
      }
      case KEY.link: {
        const v = readStringAt(s, i)
        cell.link = v.value
        cell.hasExtra = true
        i = v.end
        break
      }
      case KEY.linkTooltip: {
        const v = readStringAt(s, i)
        cell.linkTooltip = v.value
        cell.hasExtra = true
        i = v.end
        break
      }
      case KEY.comment: {
        const v = readStringAt(s, i)
        cell.comment = v.value
        cell.hasExtra = true
        i = v.end
        break
      }
      case KEY.style: {
        const end = skipValueAt(s, i)
        const rawText = s.slice(i, end)
        try {
          cell.style = JSON.parse(rawText)
        } catch {
          cell.style = rawText
        }
        cell.hasExtra = true
        i = end
        break
      }
      default:
        i = skipValueAt(s, i)
    }
    i = skipWsAt(s, i)
    if (s[i] === ',') {
      i += 1
      continue
    }
    if (s[i] === '}') return i + 1
    return -1
  }
}

/** 扫描器：在字符缓冲上做括号匹配，逐行吐出 JSON 文本 */
export class RowScanner {
  private buf = ''
  /** 'seek-rows' 之前累积头部文本；'in-rows' 逐行切分；'tail' 累积尾部 */
  private phase: 'seek-rows' | 'in-rows' | 'tail' = 'seek-rows'
  headText = ''
  tailText = ''
  /** 已切分出的行 JSON 文本回调（兼容路径） */
  onRow: (rowJson: string) => void = () => {}
  /** 行边界回调：直接在缓冲内解析，避免每行一次字符串切片 */
  onRowText: (text: string, start: number, end: number) => number = (text, start, end) => {
    this.onRow(text.slice(start, end))
    return end
  }

  /** 消费一个文本分块；可能产出多行 */
  feed(chunk: string): void {
    this.buf += chunk
    if (this.phase === 'seek-rows') {
      const i = this.buf.indexOf('"rows"')
      if (i < 0) {
        // 保留少量尾部以防关键字被切断
        if (this.buf.length > 16) {
          this.headText += this.buf.slice(0, -16)
          this.buf = this.buf.slice(-16)
        }
        return
      }
      this.headText += this.buf.slice(0, i)
      const open = this.buf.indexOf('[', i + 6)
      if (open < 0) {
        this.buf = this.buf.slice(i)
        return
      }
      this.buf = this.buf.slice(open + 1)
      this.phase = 'in-rows'
    }
    if (this.phase === 'in-rows') this.scanRows()
    if (this.phase === 'tail') {
      this.tailText += this.buf
      this.buf = ''
    }
  }

  private scanRows(): void {
    let i = 0
    const s = this.buf
    while (i < s.length) {
      // 跳过空白与逗号
      const c = s[i]
      if (c === ' ' || c === '\n' || c === '\r' || c === '\t' || c === ',') {
        i += 1
        continue
      }
      if (c === ']') {
        // rows 数组结束 → 余下为尾部
        this.tailText = s.slice(i + 1)
        this.buf = ''
        this.phase = 'tail'
        return
      }
      if (c !== '{') {
        // 容错：未知内容丢弃
        i += 1
        continue
      }
      const end = matchObjectEnd(s, i)
      if (end < 0) break // 行未完整，等下一个分块
      const next = this.onRowText(s, i, end + 1)
      i = next > i ? next : end + 1
    }
    this.buf = s.slice(i)
  }

  /** 结束：返回头部与尾部文本（供解析小字段与元信息） */
  done(): void {
    if (this.phase === 'seek-rows') {
      this.headText += this.buf
      this.buf = ''
    } else if (this.phase === 'in-rows') {
      this.scanRows()
      this.tailText += this.buf
      this.buf = ''
    }
  }
}

/** 从 s[start]（'{'）找到匹配的 '}'，处理字符串与转义；未找到返回 -1 */
export function matchObjectEnd(s: string, start: number): number {
  let depth = 0
  let inStr = false
  let esc = false
  for (let i = start; i < s.length; i += 1) {
    const c = s[i]
    if (inStr) {
      if (esc) esc = false
      else if (c === '\\') esc = true
      else if (c === '"') inStr = false
      continue
    }
    if (c === '"') inStr = true
    else if (c === '{' || c === '[') depth += 1
    else if (c === '}' || c === ']') {
      depth -= 1
      if (depth === 0) return i
    }
  }
  return -1
}

/** 把头部/尾部文本解析成对象（两者都很小） */
function parseParts(headText: string, tailText: string): Record<string, unknown> {
  let head: Record<string, unknown> = {}
  const h = headText.trim().replace(/^\{\s*/, '').replace(/,\s*$/, '')
  if (h.trim()) {
    try {
      head = JSON.parse(`{${h}}`) as Record<string, unknown>
    } catch {
      head = {}
    }
  }
  let tail: Record<string, unknown> = {}
  const t = tailText.trim().replace(/^\s*,\s*/, '')
  const tt = t.startsWith('}') ? '' : t.replace(/,\s*$/, '')
  if (tt.trim()) {
    try {
      tail = JSON.parse(`{${tt}`) as Record<string, unknown>
    } catch {
      tail = {}
    }
  }
  return { ...head, ...tail }
}

/** 把 ReadableStream（UTF-8 文本）流式解析为 PackedSheet */
export async function packSheetFromStream(
  stream: ReadableStream<Uint8Array>,
  opts: { defaultRowPx?: number; name?: string; mode?: 'auto' | 'batch' | 'scanner' | 'json' } = {},
): Promise<PackResult & { parseMs: number; chunks: number; mode: 'batch' | 'scanner' | 'json' }> {
  const t0 = typeof performance !== 'undefined' ? performance.now() : 0
  const defaultRowPx = opts.defaultRowPx ?? 20
  const writer = new PackedWriter()
  const scanner = new RowScanner()
  let seq = 0
  let pendingIndex: number | null = null
  let pendingHeight = 0
  let cellCol = 1
  const sink: RowSink = {
    cell: newParsedCell(),
    beginRow(index, heightPx) {
      seq += 1
      pendingIndex = index
      pendingHeight = heightPx
      cellCol = 0
    },
    endRow() {
      writer.endParsedRow(pendingIndex ?? seq, pendingHeight)
    },
  }
  sink.endRowCell = () => {
    const c = sink.cell
    cellCol += 1
    if (c.col === 0) c.col = cellCol
    else cellCol = c.col
    writer.pushParsedCell(c)
  }
  // 行解析策略（隔离测量：三条路径吞吐相近，约 37~44 MB/s；瓶颈是逐行扫描/切片等固定开销，
  // 而非解析器选型）：
  // - batch  ：把约 1MB 的行文本拼成数组交给原生 JSON.parse（V8 解析器最快；
  //            瞬时对象限制在 MB 级，不产生整表对象图）—— 默认；
  // - scanner：字段级单遍扫描（零键分配，约 native 的 1/3，无任何临时对象）；
  // - json   ：逐行 JSON.parse（每行一次调用 + 一次切片，最慢，保留供对照）。
  const mode: 'batch' | 'scanner' | 'json' =
    opts.mode === 'scanner' ? 'scanner' : opts.mode === 'json' ? 'json' : 'batch'
  let batchStart = -1
  let batchText = ''
  const flushBatch = () => {
    if (batchStart < 0) return
    // batchText 每行以 ',' 结尾，拼成数组时要去掉末尾逗号（否则 JSON.parse 报错）
    const payload = `[${batchText.endsWith(',') ? batchText.slice(0, -1) : batchText}]`
    batchStart = -1
    batchText = ''
    try {
      const arr = JSON.parse(payload) as RawRow[]
      for (const row of arr) {
        seq += 1
        writer.pushRow(row, row.index ?? seq, row.height ? (row.height * 96) / 72 : 0)
      }
    } catch {
      // 极端情况（超长行导致切片异常）：回落到逐行解析
      for (const piece of payload.slice(1, -1).split('},{')) {
        try {
          const row = JSON.parse(piece.startsWith('{') ? piece : `{${piece}`) as RawRow
          seq += 1
          writer.pushRow(row, row.index ?? seq, row.height ? (row.height * 96) / 72 : 0)
        } catch {
          /* 跳过无法解析的片段 */
        }
      }
    }
  }
  const BATCH_BYTES = 1 << 20
  scanner.onRowText = (text, start, end) => {
    if (mode === 'batch') {
      if (batchStart < 0) batchStart = start
      batchText += text.slice(start, end) + ','
      if (batchText.length >= BATCH_BYTES) flushBatch()
      return end
    }
    if (mode === 'json') {
      const row = JSON.parse(text.slice(start, end)) as RawRow
      seq += 1
      writer.pushRow(row, row.index ?? seq, row.height ? (row.height * 96) / 72 : 0)
      return end
    }
    const next = parseRowAt(text, start, sink)
    if (next < 0 || next > end) {
      const row = JSON.parse(text.slice(start, end)) as RawRow
      seq += 1
      writer.pushRow(row, row.index ?? seq, row.height ? (row.height * 96) / 72 : 0)
    }
    return next < 0 ? end : next
  }
  scanner.onRow = (json) => {
    const row = JSON.parse(json) as RawRow
    seq += 1
    writer.pushRow(row, row.index ?? seq, row.height ? (row.height * 96) / 72 : 0)
  }
  const reader = stream.getReader()
  const dec = new TextDecoder()
  let chunks = 0
  for (;;) {
    const { done, value } = await reader.read()
    if (done) break
    chunks += 1
    scanner.feed(dec.decode(value, { stream: true }))
  }
  scanner.feed(dec.decode())
  scanner.done()
  if (mode === 'batch') flushBatch()

  const parts = parseParts(scanner.headText, scanner.tailText)
  const name = (parts.name as string) ?? opts.name ?? 'Sheet'
  const meta = sheetMeta({
    name,
    rows: [],
    ...(parts as Partial<XlsxSheet>),
  })
  const res = writer.finish(name, defaultRowPx, meta)
  const t1 = typeof performance !== 'undefined' ? performance.now() : 0
  return {
    ...res,
    packMs: Math.round((t1 - t0) * 10) / 10,
    parseMs: Math.round((t1 - t0) * 10) / 10,
    chunks,
    mode,
  }
}
