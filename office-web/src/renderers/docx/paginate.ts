//! docx 物理分页（纯逻辑，可离线单测）
//!
//! 产物模型里的 `parts` 是**编辑分片**（分节符 / 一级标题切块），不是页面：一个分片
//! 往往横跨多页。本模块把分片内的块序列切成物理页：
//!   · 硬分页——`page_break` 块（页在此结束，块本身不渲染）与块的 `page_break_before`；
//!   · 软分页——块自身底端超出页面可用高度时，从该块起另起一页；
//!   · 段内分页——段落带 `lineTops`（逐行顶端）时可在行边界切开，与 Word / LibreOffice
//!     一致：孤行控制下首尾各留 ≥2 行；
//!   · 表内分页——表格带 `rowBoxes`（逐行盒）时按行切开，续排片重复表头行（Word 的
//!     `w:tblHeader` 行为）；表头行之外至少要有一行数据行，否则整表挪走。
//! 高度由调用方离屏度量给出（浏览器排版 ≠ Word 排版，只求页数与分页位置近似）。

/** 孤行控制：段落跨页时首尾各至少保留的行数（Word 默认「孤行控制」开） */
const WIDOW_LINES = 2

/** 单个块的分页信息（与块流一一对应） */
export interface PaginateBlock {
  /** 块顶端/底端相对流起点的自然 px；不参与排版的块（分页符/未度量）为 null */
  top: number | null
  bottom: number | null
  /** 块前强制分页（page_break_before / 分节符 / 纸张变化） */
  breakBefore?: boolean
  /** 该块自身是分页符：当前页在此结束 */
  breakHere?: boolean
  /** 本块所属分片的页面可用高度（分片纸张/页边距可以不同；缺省用调用参数） */
  pageH?: number
  /** 与下段同页（w:keepNext，标题默认）：本块不能留在页底，换页时一起带去下一页 */
  keepWithNext?: boolean
  /** 块内逐行顶端相对块顶端的偏移（升序、首项为 0）；给出即可在行边界拆分 */
  lineTops?: number[] | null
  /** 表格逐行盒 [top, bottom]（相对块顶端，升序）；给出即可在行边界拆分 */
  rowBoxes?: Array<[number, number]> | null
  /** 表格逐行「内容盒」[top, bottom]（不含单元格上下内边距，与 rowLines 配套）；
   *  给出即可在**行内**按文本行拆分（Word 允许一行跨页：行内文本接着排） */
  rowContent?: Array<[number, number]> | null
  /** 表格逐行的文本行数（最高单元格；与 rowContent 配套） */
  rowLines?: number[] | null
  /** 表格表头行数（拆分后每页续排都要重复这几行） */
  headerRows?: number
}

/** 物理页在块流中的区间与内容范围（top/bottom 相对流起点） */
export interface DocxPageRange {
  /** 块区间 [from, to)（流下标） */
  from: number
  to: number
  top: number
  bottom: number
  /** 首页首块从第几行开始（跨页续段：前面的行在上一页） */
  leadSkip?: number
  /** 末页末块在本页显示到第几行（不含；跨页续段：后面的行在下一页） */
  trailKeep?: number
  /** 首页首块是表格时从第几行开始（前面的行在上一页；续排片重复表头） */
  leadRows?: number
  /** 末页末块是表格时显示到第几行（不含；后面的行在下一页） */
  trailRows?: number
  /** 拆在行内：首页末块表格的第 trailRows-1 行只画前几行文本 */
  trailLines?: number
  /** 拆在行内：首页末块表格的第 leadRows 行已画过几行文本（续排跳过） */
  leadLines?: number
}

/** 表格逐行盒（拆不动返回 null） */
function rowsOf(b: PaginateBlock): Array<[number, number]> | null {
  const rows = b.rowBoxes
  if (!rows || rows.length < 2 || b.top === null || b.bottom === null) return null
  return rows
}

/** 逐行几何：[top, bottom]（相对块顶端；末行底端用块底端兜底） */
function lineBoxes(b: PaginateBlock): Array<[number, number]> | null {
  const tops = b.lineTops
  if (!tops || tops.length < WIDOW_LINES * 2 || b.top === null || b.bottom === null) return null
  const h = b.bottom - b.top
  const out: Array<[number, number]> = []
  for (let k = 0; k < tops.length; k += 1) {
    const top = tops[k]
    const bottom = k + 1 < tops.length ? tops[k + 1] : h
    if (bottom > top) out.push([top, bottom])
  }
  return out.length >= WIDOW_LINES * 2 ? out : null
}

/** 从第 start 行起、**可用高度 avail**（相对该行顶端）内能放几行（末行底端必须装下） */
function linesFitting(boxes: Array<[number, number]>, start: number, avail: number): number {
  const limit = avail + boxes[start][0]
  let n = 0
  for (let k = start; k < boxes.length; k += 1) {
    if (boxes[k][1] > limit) break
    n += 1
  }
  return n
}

/**
 * 把块流切成物理页。
 *
 * @param blocks   逐块分页信息（长度 = 块数，坐标同一坐标系）
 * @param pageH    页面可用高度（纸张高 − 上下页边距）缺省值；块自带 pageH 时以块为准
 * @param contentH 度量到的内容总高（末块底端缺失时兜底）
 * @param startUsed 首页已占用高（连续分节的正文接在页内已有内容之后续排；后续页为 0）
 */
export function paginateBlocks(
  blocks: readonly PaginateBlock[],
  pageH: number,
  contentH: number,
  startUsed = 0,
): DocxPageRange[] {
  const pages: DocxPageRange[] = []
  /** 本页首块的起始行（跨页续段；0 = 块首行） */
  let pageLead = 0
  /** 本页首块是表格时的起始行 / 行内已画行数（跨页续排） */
  let pageLeadRows = 0
  let pageLeadLines = 0

  /** 表格逐行内容盒（与行盒等长才算可用） */
  const rowContentOf = (b: PaginateBlock | undefined): Array<[number, number]> | null => {
    const rows = b?.rowBoxes
    const content = b?.rowContent
    if (!rows || !content || content.length !== rows.length || !b?.rowLines) return null
    return content
  }
  /** 表格某行内第 k 行文本的顶端（相对块顶端；k 取行数时给内容底端） */
  const rowLineTop = (
    b: PaginateBlock | undefined,
    box: [number, number],
    idx: number,
    k: number,
  ): number => {
    const content = rowContentOf(b)?.[idx]
    const lines = b?.rowLines?.[idx] ?? 1
    if (!content) return box[0] + (k * (box[1] - box[0])) / Math.max(1, lines)
    return content[0] + (k * (content[1] - content[0])) / Math.max(1, lines)
  }
  /** 表格行的单侧上下内边距 */
  const rowPad = (b: PaginateBlock | undefined, box: [number, number], idx: number): number => {
    const content = rowContentOf(b)?.[idx]
    if (!content) return 0
    return Math.max(0, box[1] - box[0] - (content[1] - content[0])) / 2
  }

  /** 收口一页：顶端取首块，底端取末块自身（缺省用内容末尾） */
  const close = (
    from: number,
    to: number,
    trailKeep?: number,
    trailRows?: number,
    trailLines?: number,
  ) => {
    const last = blocks[to - 1]
    const boxes = last ? lineBoxes(last) : null
    const keeping = trailKeep !== undefined && boxes && trailKeep < boxes.length ? trailKeep : undefined
    const rows = last ? rowsOf(last) : null
    // 记下末块可见到哪一行；行内切时即使切在最后一行也要记（底端要按行数算）
    const rowKeeping =
      trailRows === undefined || !rows
        ? undefined
        : trailRows < rows.length
          ? trailRows
          : trailLines !== undefined && trailRows > 0
            ? trailRows
            : undefined
    let bottom: number
    if (keeping !== undefined && last?.top != null) {
      bottom = last.top + boxes![keeping][0] // 下一行顶端 = 本页可见内容的底端
    } else if (rowKeeping !== undefined && last?.top != null && trailLines !== undefined) {
      // 行内切：末行只画前 trailLines 行文本 → 底端 = 该行内容第 trailLines 行的底端 + 下内边距
      const idx = rowKeeping - 1
      bottom =
        last.top + rowLineTop(last, rows![idx], idx, trailLines) + rowPad(last, rows![idx], idx)
    } else if (rowKeeping !== undefined && last?.top != null) {
      bottom = last.top + rows![rowKeeping - 1][1] // 末行底端 = 本页可见内容的底端
    } else {
      bottom = last?.bottom ?? contentH
    }
    // 顶端用本页实际起点（跨页续段时 = 续行顶端，而非块顶端）
    const range: DocxPageRange = { from, to, top: pageTop, bottom }
    if (pageLead > 0) range.leadSkip = pageLead
    if (pageLeadRows > 0) range.leadRows = pageLeadRows
    if (pageLeadLines > 0) range.leadLines = pageLeadLines
    if (keeping !== undefined) range.trailKeep = keeping
    if (rowKeeping !== undefined) range.trailRows = rowKeeping
    if (rowKeeping !== undefined && trailLines !== undefined) range.trailLines = trailLines
    pages.push(range)
    pageLead = 0
    pageLeadRows = 0
    pageLeadLines = 0
    pageTop = 0
  }

  let from = -1
  let startTop = 0
  let pageTop = 0
  /** 首页已占用高：只影响第一页的容量判断，一旦翻页即归零 */
  let occupied = startUsed

  const open = (index: number, top: number, lead = 0, leadRows = 0, leadLines = 0) => {
    from = index
    startTop = top
    pageTop = top
    pageLead = lead
    pageLeadRows = leadRows
    pageLeadLines = leadLines
  }

  /** 表格放不下又挪不走时的兜底：先放几行（页面长高），避免死循环 */
  const forceRows = (rows: Array<[number, number]>, hdr: number, start: number) =>
    Math.min(rows.length, Math.max(hdr + 1, start + 1))

  for (let i = 0; i < blocks.length; i += 1) {
    const b = blocks[i]
    if (b.breakHere) {
      if (from >= 0) {
        close(from, i)
        from = -1
        occupied = 0
      }
      continue
    }
    if (b.breakBefore && from >= 0) {
      close(from, i)
      from = -1
      occupied = 0
    }
    // 未度量的块：留在本页，不做溢出判断
    if (b.top === null || b.bottom === null) {
      if (from < 0) open(i, 0)
      continue
    }
    if (from < 0) open(i, b.top)
    const limit = b.pageH ?? pageH

    // 溢出：优先在本块内按行切开（表格先按行拆，段落再按行拆）
    if (b.bottom - startTop + occupied > limit) {
      // 表格跨页：按行拆分（能拆到行内就拆行内）；续排片重复表头行（Word 的 w:tblHeader 行为）
      const rows = rowsOf(b)
      if (rows) {
        const hdr = Math.max(0, Math.min(b.headerRows ?? 0, rows.length - 1))
        const headH = hdr > 0 ? rows[hdr - 1][1] : 0
        const start = from === i ? pageLeadRows : 0
        const drawn = from === i ? pageLeadLines : 0
        // 续排片的绘制起点：表头（重复）在上面，故自然坐标里要上抬一个表头高；
        // 首片从表顶起算（含单元格上内边距），续排片从行内第 drawn 行的顶端起算
        const pieceTop =
          b.top +
          (start === 0 && drawn === 0 ? rows[0][0] : rowLineTop(b, rows[start], start, drawn)) -
          (start > 0 ? headH : 0)
        const limitY = pieceTop + Math.max(0, limit - (pieceTop - startTop + occupied))
        // 1) 整行能放下的都放下
        let end = start
        while (end < rows.length && b.top + rows[end][1] <= limitY) end += 1
        // 2) 末尾那行放不下整行：试试行内切（Word 允许一行跨页接着排）
        let lines = 0
        if (end < rows.length && b.rowLines) {
          const row = rows[end]
          const already = start === end ? drawn : 0
          const avail = limitY - (b.top + rowLineTop(b, row, end, already))
          const lineH = Math.max(1, (rowLineTop(b, row, end, already + 1) - rowLineTop(b, row, end, already)))
          const fit = Math.floor(avail / lineH)
          const cut = already + Math.max(0, fit)
          if (cut < (b.rowLines[end] ?? 1)) lines = cut
        }
        // 至少要有内容行（表头之外那一行数据行），否则整表挪走，别让表头单独占一页
        const covered = end + (lines > 0 ? 1 : 0)
        const ok = lines > 0 ? covered > hdr : covered >= Math.max(start + 1, hdr + 1)
        if (ok) {
          const trailRows = lines > 0 ? end + 1 : end
          close(from, i + 1, undefined, trailRows, lines > 0 ? lines : undefined)
          if (end >= rows.length) {
            from = -1 // 整表放完：本页到此为止，余下照常续排
            occupied = 0
            continue
          }
          // 行内切：续排仍从该行开始（leadLines 记已画的行数）
          open(i, b.top + rowLineTop(b, rows[end], end, lines) - headH, 0, end, lines)
          occupied = 0
          i -= 1 // 余下各行在新页重扫（可能还要再切）
          continue
        }
        // 连「表头 + 一行数据行」都放不下：已在页首（或续排片放不下表头）就强行放，页面长高
        if (from === i) {
          const keep = forceRows(rows, hdr, start)
          const keepRow = Math.min(keep, rows.length) - 1
          const keepLines = start === keepRow ? (b.rowLines?.[keepRow] ?? 1) : undefined
          close(from, i + 1, undefined, keep, keepLines)
          if (keep >= rows.length) {
            from = -1
            occupied = 0
            continue
          }
          open(i, b.top + rowLineTop(b, rows[keep], keep, 0) - headH, 0, keep, 0)
          occupied = 0
          i -= 1
          continue
        }
        // 整表挪到下一页；页底不留「挂着下段」的块（keepNext，连链一起挪）
        let brk = i
        while (brk > from + 1 && blocks[brk - 1]?.keepWithNext) brk -= 1
        if (brk === i && blocks[i - 1]?.keepWithNext) continue // 留在一起，页面撑高
        close(from, brk)
        open(brk, blocks[brk]?.top ?? b.top)
        occupied = 0
        i = brk - 1 // 被挪走的块在新页重扫（可能还要再切/再挪）
        continue
      }
      const boxes = lineBoxes(b)
      if (boxes) {
        // 本页首块是续段时从 leadSkip 行起算
        const start = from === i ? pageLead : 0
        const avail = limit - (b.top + boxes[start][0] - startTop + occupied)
        const fit = linesFitting(boxes, start, avail)
        if (fit >= WIDOW_LINES && boxes.length - start - fit >= WIDOW_LINES) {
          close(from, i + 1, start + fit)
          open(i, b.top + boxes[start + fit][0], start + fit)
          occupied = 0
          i -= 1 // 同一块的余下部分在新页重扫（可能还要再切）
          continue
        }
      }
      // 单块超页且切不动：留在本页，页面随之长高（不裁切）
      if (from === i) continue
      // 整块挪到下一页；页底不留「挂着下段」的块（keepNext，连链一起挪）
      let brk = i
      while (brk > from + 1 && blocks[brk - 1]?.keepWithNext) brk -= 1
      // keepNext 链已顶到页首、再挪就要把同页伙伴拆开：留在一起（页面撑高）
      if (brk === i && blocks[i - 1]?.keepWithNext) continue
      close(from, brk)
      open(brk, blocks[brk]?.top ?? b.top)
      occupied = 0
      i = brk - 1 // 被挪走的块在新页重扫（可能还要再切/再挪）
    }
  }
  if (from >= 0) close(from, blocks.length)
  if (!pages.length) pages.push({ from: 0, to: 0, top: 0, bottom: 0 })
  return pages
}
