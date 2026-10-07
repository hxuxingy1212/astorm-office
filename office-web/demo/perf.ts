//! 性能压测用的合成工作簿生成器（在浏览器内存里直接构造，避免提交巨型 JSON）
import type { XlsxCell, XlsxSheet, XlsxWorkbook } from '@/types'
import type { PackedSheet } from '@/renderers/xlsx/packed'

export interface PerfOptions {
  rows: number
  cols: number
  /** 每行都写值（true）或仅稀疏写入第一列（false） */
  dense?: boolean
  /** 单元格写入 reference 字符串（默认 false：按列序推断，省 30%+ 内存） */
  refs?: boolean
}

export interface PerfSample {
  workbook: XlsxWorkbook | { sheets: PackedSheet[] }
  /** 打包形态的估算体积（MB），JSON 形态为 null */
  packedMB: number | null
  /** 流式模式下生成的 JSON 文本量（MB） */
  textMB?: number
  /** 生成耗时（ms） */
  buildMs: number
  /** 单元格总数 */
  cells: number
  /** 生成后的堆内存占用（MB，Chrome 可用；其他环境为 null） */
  heapMB: number | null
}

const HEAD = ['月份', '收入', '成本', '毛利', '增长率', '客单价']
const FILL: XlsxCell['cell_type'][] = ['string', 'number', 'number', 'number', 'number', 'number']

/** 直接构造列式打包工作表：不产生 6M 个 JS 对象，内存占用大幅下降 */
export function buildPackedSynthetic(opts: PerfOptions): PerfSample {
  const t0 = performance.now()
  const { rows, cols } = opts
  const n = rows * cols
  const rowNos = new Int32Array(rows)
  const rowStart = new Uint32Array(rows + 1)
  const cellCols = new Uint16Array(n)
  const nums = new Float64Array(n)
  const strs = new Int32Array(n).fill(-1)
  const fmts = new Int32Array(n).fill(-1)
  const kinds = new Uint8Array(n)
  const strings: string[] = []
  for (let r = 0; r < rows; r += 1) {
    rowNos[r] = r + 1
    rowStart[r] = r * cols
    for (let c = 0; c < cols; c += 1) {
      const i = r * cols + c
      cellCols[i] = c + 1
      if (r === 0) {
        kinds[i] = 2
        strs[i] = strings.length
        strings.push(HEAD[c % HEAD.length])
      } else if (c === 0) {
        kinds[i] = 2
        strs[i] = strings.length
        strings.push(`行 ${r + 1}`)
      } else {
        kinds[i] = 1
        nums[i] = Math.round(Math.sin(r * 0.01 + c) * 10000) / 100 + r
        fmts[i] = 0
      }
    }
  }
  rowStart[rows] = n
  const packed: PackedSheet = {
    name: `百万行压测(列式 ${rows.toLocaleString()}×${cols})`,
    rowNos,
    rowStart,
    cols: cellCols,
    nums,
    strs,
    bools: new Uint8Array(n),
    fmts,
    kinds,
    strings,
    formats: ['#,##0.00'],
    extras: new Map(),
    heights: new Float32Array(rows),
    defaultRowPx: 20,
    maxCol: cols,
    cellCount: n,
    meta: {
      name: `百万行压测(列式 ${rows.toLocaleString()}×${cols})`,
      rows: [],
      default_row_height: 15,
      default_col_width: 12,
      headings: false,
    },
  }
  const bytes =
    rowNos.byteLength + rowStart.byteLength + cellCols.byteLength + nums.byteLength +
    strs.byteLength + fmts.byteLength + kinds.byteLength +
    strings.reduce((a, v) => a + v.length * 2 + 40, 0)
  const heap = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory
  return {
    workbook: { sheets: [packed] },
    packedMB: Math.round((bytes / 1048576) * 10) / 10,
    buildMs: Math.round(performance.now() - t0),
    cells: n,
    heapMB: heap ? Math.round((heap.usedJSHeapSize / 1048576) * 10) / 10 : null,
  }
}

/**
 * 合成 JSON 文本流并流式解析：文本不整体驻留内存（分块生成 → 分块解析），
 * 峰值内存 ≈ 分块文本 + 列式数组，用于对比「JSON.parse 全量」的峰值。
 */
export async function buildStreamedSynthetic(opts: PerfOptions): Promise<PerfSample & { parseMs: number; textMB: number }> {
  const t0 = performance.now()
  const { rows, cols } = opts
  const chunkRows = 2000
  let produced = 0
  let textBytes = 0
  const encoder = new TextEncoder()
  const stream = new ReadableStream<Uint8Array>({
    pull(controller) {
      if (produced >= rows) {
        controller.close()
        return
      }
      const end = Math.min(produced + chunkRows, rows)
      let buf = produced === 0 ? '{"name":"流式压测","merges":[],"rows":[' : ''
      for (let r = produced; r < end; r += 1) {
        const cells: string[] = []
        for (let c = 0; c < cols; c += 1) {
          const ref = `${colName(c + 1)}${r + 1}`
          if (r === 0) cells.push(`{"reference":"${ref}","value":"${HEAD[c % HEAD.length]}","cell_type":"string"}`)
          else if (c === 0) cells.push(`{"reference":"${ref}","value":"行 ${r + 1}","cell_type":"string"}`)
          else {
            const v = Math.round(Math.sin(r * 0.01 + c) * 10000) / 100 + r
            cells.push(`{"reference":"${ref}","value":${v},"cell_type":"number","number_format":"#,##0.00"}`)
          }
        }
        buf += (r > produced || r > 0 ? ',' : '') + `{"index":${r + 1},"cells":[${cells.join(',')}]}`
      }
      if (end >= rows) buf += '],"columns":[{"width":12}],"default_row_height":15}'
      produced = end
      const bytes = encoder.encode(buf)
      textBytes += bytes.byteLength
      controller.enqueue(bytes)
    },
  })
  const { packSheetFromStream } = await import('@/renderers/xlsx/stream')
  const res = await packSheetFromStream(stream, { defaultRowPx: 20, name: '流式压测' })
  const heap = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory
  return {
    workbook: { sheets: [res.packed] },
    packedMB: Math.round((res.bytes / 1048576) * 10) / 10,
    buildMs: Math.round(performance.now() - t0),
    parseMs: res.parseMs,
    textMB: Math.round((textBytes / 1048576) * 10) / 10,
    cells: res.packed.cellCount,
    heapMB: heap ? Math.round((heap.usedJSHeapSize / 1048576) * 10) / 10 : null,
  }
}

/** 构造合成工作表：行数可达百万级，用于验证虚拟滚动 */
export function buildSyntheticWorkbook(opts: PerfOptions): PerfSample {
  const t0 = performance.now()
  const { rows, cols, dense = true, refs = false } = opts
  const out: XlsxSheet['rows'] = new Array(rows)
  let cells = 0
  for (let r = 1; r <= rows; r += 1) {
    const list: XlsxCell[] = []
    const limit = dense ? cols : 1
    for (let c = 1; c <= limit; c += 1) {
      const head = r === 1
      const value: string | number = head
        ? HEAD[(c - 1) % HEAD.length]
        : c === 1
          ? `行 ${r}`
          : Math.round(Math.sin(r * 0.01 + c) * 10000) / 100 + r
      list.push({
        ...(refs ? { reference: `${colName(c)}${r}` } : {}),
        value,
        cell_type: head ? 'string' : FILL[(c - 1) % FILL.length],
        number_format: !head && c > 1 ? '#,##0.00' : undefined,
      })
      cells += 1
    }
    out[r - 1] = { index: r, cells: list as XlsxCell[] }
  }
  const sheet: XlsxSheet = {
    name: `百万行压测(${rows.toLocaleString()}×${cols})`,
    rows: out,
    default_row_height: 15,
    default_col_width: 12,
    headings: false,
  }
  const heap = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory
  return {
    workbook: { sheets: [sheet] },
    packedMB: null,
    buildMs: Math.round(performance.now() - t0),
    cells,
    heapMB: heap ? Math.round((heap.usedJSHeapSize / 1048576) * 10) / 10 : null,
  }
}

/**
 * 解析吞吐微基准：预生成固定体积 JSON 文本，分别测「流式按行解析」与「JSON.parse 全量」。
 * 注意：两者交替运行会互相干扰（JSON.parse 产生大量垃圾拖慢后续测量），
 * 因此先跑 3 次流式取中位数，再跑 3 次 JSON.parse 取中位数。
 */
export async function measureParseThroughput(opts: { rows?: number; cols?: number } = {}): Promise<{
  textMB: number
  cells: number
  jsonParseMs: number
  jsonMBps: number
  streamMs: number
  streamMBps: number
  scannerMs: number
  scannerMBps: number
  batchMs: number
  batchMBps: number
}> {
  const rows = opts.rows ?? 40_000
  const cols = opts.cols ?? 6
  const enc = new TextEncoder()
  const chunks: Uint8Array[] = []
  let textBytes = 0
  const chunkRows = 2000
  for (let r0 = 0; r0 < rows; r0 += chunkRows) {
    const end = Math.min(r0 + chunkRows, rows)
    let buf = r0 === 0 ? '{"name":"吞吐测试","rows":[' : ''
    for (let r = r0; r < end; r += 1) {
      const cells: string[] = []
      for (let c = 0; c < cols; c += 1) {
        const ref = `${colName(c + 1)}${r + 1}`
        if (r === 0) cells.push(`{"reference":"${ref}","value":"${HEAD[c % HEAD.length]}","cell_type":"string"}`)
        else if (c === 0) cells.push(`{"reference":"${ref}","value":"行 ${r + 1}","cell_type":"string"}`)
        else {
          const v = Math.round(Math.sin(r * 0.01 + c) * 10000) / 100 + r
          cells.push(`{"reference":"${ref}","value":${v},"cell_type":"number","number_format":"#,##0.00"}`)
        }
      }
      buf += (r > 0 ? ',' : '') + `{"index":${r + 1},"cells":[${cells.join(',')}]}`
    }
    if (end >= rows) buf += ']}'
    const b = enc.encode(buf)
    textBytes += b.byteLength
    chunks.push(b)
  }
  const textMB = Math.round((textBytes / 1048576) * 10) / 10
  const cells = rows * cols
  const { packSheetFromStream } = await import('@/renderers/xlsx/stream')

  const streamOnce = async (mode: 'json' | 'scanner' | 'batch'): Promise<number> => {
    let idx = 0
    const stream = new ReadableStream<Uint8Array>({
      pull(c) {
        if (idx >= chunks.length) {
          c.close()
          return
        }
        c.enqueue(chunks[idx])
        idx += 1
      },
    })
    const t = performance.now()
    await packSheetFromStream(stream, { defaultRowPx: 20, mode })
    return performance.now() - t
  }
  const median = (xs: number[]): number => xs.slice().sort((a, b) => a - b)[Math.floor(xs.length / 2)]

  // 先跑流式（避免 JSON.parse 的垃圾影响）
  const streamTimes: number[] = []
  for (let i = 0; i < 3; i += 1) {
    streamTimes.push(await streamOnce('json'))
    await new Promise((r) => setTimeout(r, 0))
  }
  const scannerTimes: number[] = []
  for (let i = 0; i < 3; i += 1) {
    scannerTimes.push(await streamOnce('scanner'))
    await new Promise((r) => setTimeout(r, 0))
  }
  const batchTimes: number[] = []
  for (let i = 0; i < 3; i += 1) {
    batchTimes.push(await streamOnce('batch'))
    await new Promise((r) => setTimeout(r, 0))
  }

  const bigText = chunks.map((c) => new TextDecoder().decode(c)).join('')
  const jsonTimes: number[] = []
  for (let i = 0; i < 3; i += 1) {
    const t = performance.now()
    const parsed = JSON.parse(bigText) as { rows: unknown[] }
    jsonTimes.push(performance.now() - t)
    ;(parsed as { rows: unknown[] }).rows = []
    await new Promise((r) => setTimeout(r, 0))
  }

  const streamMs = Math.round(median(streamTimes))
  const scannerMs = Math.round(median(scannerTimes))
  const batchMs = Math.round(median(batchTimes))
  const jsonParseMs = Math.round(median(jsonTimes))
  return {
    textMB,
    cells,
    jsonParseMs,
    jsonMBps: Math.round((textMB / (jsonParseMs / 1000)) * 10) / 10,
    streamMs,
    streamMBps: Math.round((textMB / (streamMs / 1000)) * 10) / 10,
    scannerMs,
    scannerMBps: Math.round((textMB / (scannerMs / 1000)) * 10) / 10,
    batchMs,
    batchMBps: Math.round((textMB / (batchMs / 1000)) * 10) / 10,
  }
}

/** 1-based 列号 → 列字母 */
export function colName(n: number): string {
  let s = ''
  let x = n
  while (x > 0) {
    const rem = (x - 1) % 26
    s = String.fromCharCode(65 + rem) + s
    x = Math.floor((x - 1) / 26)
  }
  return s
}
