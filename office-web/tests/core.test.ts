//! 核心纯逻辑单测（离线可跑：tsc 编译后用 node 执行）
declare const process: { exit(code: number): void }

import { formatNumber, splitSections, groupInt } from '../src/renderers/xlsx/numberFormat'
import { latexToHtml } from '../src/renderers/docx/latex'
import { colLetter, cellRef, xlsxCellPath, docxPath, typeOrdinal, pdfElementPath } from '../src/core/path'
import { pdfElementType, pdfElementBBox, pdfElementContent } from '../src/renderers/pdf/types'
import { colCharsToPx, rowPtToPx, cssColor } from '../src/core/units'
import { packSheet, packedCellAt, adapterFor, isPackedSheet, packedMB } from '../src/renderers/xlsx/packed'
import {
  packSheetFromStream,
  matchObjectEnd,
  RowScanner,
  parseRowAt,
  readStringAt,
  readNumberAt,
  skipValueAt,
  newParsedCell,
  type RowSink,
} from '../src/renderers/xlsx/stream'
import type { XlsxSheet } from '../src/types'
import {
  buildRowLayout,
  rowTop,
  visibleRows,
  renderedRowCount,
  canvasScale,
  refToPos,
} from '../src/renderers/xlsx/virtual'
import { paginateBlocks } from '../src/renderers/docx/paginate'
import { wheelZoomAt, pinchZoomAt, scrollAnchor } from '../src/core/zoomMath'
import {
  layoutParagraph,
  spansOf,
  type StyledChar,
  type MeasureText,
} from '../src/renderers/docx/linebreak'

let failed = 0
function eq<T>(actual: T, expected: T, label: string) {
  const a = JSON.stringify(actual)
  const b = JSON.stringify(expected)
  if (a !== b) {
    failed += 1
    console.error(`✗ ${label}\n  actual:   ${a}\n  expected: ${b}`)
  } else {
    console.log(`✓ ${label}`)
  }
}

// 数字格式（与 ai-excel Rust 引擎逐条对齐）
eq(formatNumber(0.131, '0.0%').text, '13.1%', 'percent')
eq(formatNumber(27269358, '#,##0').text, '27,269,358', 'thousands')
eq(formatNumber(1234.5, '#,##0.00').text, '1,234.50', 'two decimals')
eq(formatNumber(-50, '0.00;(0.00)').text, '(50.00)', 'negative section')
eq(formatNumber(50, '0.00;(0.00)').text, '50.00', 'positive section')
const acct = '_(* #,##0.00_);_(* \\(#,##0.00\\);_(* "-"??_);_(@_)'
eq(formatNumber(123, acct).text, ' 123.00 ', 'accounting positive')
eq(formatNumber(-50, acct).text, ' (50.00)', 'accounting negative')
eq(formatNumber(0, acct).text, ' -   ', 'accounting zero')
eq(formatNumber(12.5, '"$"#,##0.00').text, '$12.50', 'quoted literal currency')
eq(formatNumber(-5, '[Red]-0.00').text, '-5.00', 'color modifier text')
eq(formatNumber(-5, '[Red]-0.00').color, 'FF0000', 'color modifier color')
eq(formatNumber(-7.5, '#,##0.00').text, '-7.50', 'single section negative')
eq(formatNumber(1, '0.##').text, '1', 'optional decimals trimmed')
eq(formatNumber(1.5, '0.##').text, '1.5', 'optional decimals kept')
eq(formatNumber(0, '??').text, '  ', 'question placeholder zero')
eq(splitSections('0.00;(0.00);"-"').length, 3, 'split sections')
eq(groupInt('-1234567.89'), '-1,234,567.89', 'groupInt')

// 路径
eq(colLetter(1), 'A', 'colLetter A')
eq(colLetter(27), 'AA', 'colLetter AA')
eq(cellRef(2, 3), 'B3', 'cellRef')
eq(xlsxCellPath(0, 2, 3), '/sheet[1]/cell[B3]', 'xlsx cell path')
eq(docxPath(0, ['paragraph[3]']), '/part[1]/paragraph[3]', 'docx path')
eq(typeOrdinal(['text', 'shape', 'text'], 2), 2, 'typeOrdinal')

// LaTeX
eq(latexToHtml('E=mc^2'), 'E=mc<sup>2</sup>', 'latex sup')
eq(latexToHtml('\\frac{a}{b}').includes('lx-frac'), true, 'latex frac')
eq(latexToHtml('\\alpha+\\beta').includes('α'), true, 'latex greek')
eq(latexToHtml('\\sqrt{x}').includes('lx-sqrt'), true, 'latex sqrt')

// 单位
eq(colCharsToPx(8.43), 64, 'colCharsToPx')
eq(rowPtToPx(15), 20, 'rowPtToPx')
eq(cssColor('ff0000'), '#FF0000', 'cssColor')

// ---- 行虚拟化 ----
const ids = Array.from({ length: 1_000_000 }, (_, i) => i + 1)
const heights = new Map<number, number>([
  [1, 30],
  [500_000, 40],
])
const layout = buildRowLayout(ids, heights, 20)
eq(layout.total, 1_000_000 * 20 + 10 + 20, '百万行总高（含显式行高增量）')
eq(rowTop(layout, 0), 0, 'rowTop(0)')
eq(rowTop(layout, 1), 30, 'rowTop(1) 受首行显式高度影响')
eq(rowTop(layout, 2), 50, 'rowTop(2)')
eq(rowTop(layout, 499_999), 499_999 * 20 + 10, 'rowTop 命中稀疏增量前')
eq(rowTop(layout, 500_000), 500_000 * 20 + 10 + 20, 'rowTop 命中稀疏增量后')
const [v0, v1] = visibleRows(layout, 2_000_000, 800)
eq(v1 - v0 + 1 <= 800 / 20 + 13, true, '视口内渲染行数受控（虚拟化）')
eq(renderedRowCount(layout, 0, 800) <= 60, true, '顶部渲染行数 <= 60')
eq(renderedRowCount(layout, 19_999_000, 800) <= 60, true, '底部渲染行数 <= 60')
eq(visibleRows(buildRowLayout([], new Map(), 20), 0, 800)[1], -1, '空表返回空区间')
eq(canvasScale(10_000_000), 1, '未超限不压缩画布')
eq(canvasScale(60_000_000) < 1, true, '超限时压缩画布高度')
eq(canvasScale(20_000_000) < 1, true, '百万行 × 20px 也需压缩（浏览器滚动高度上限）')
eq(refToPos('AB12'), { col: 28, row: 12 }, 'refToPos 解析')

// ---- 列式打包 ----
const demoSheet: XlsxSheet = {
  name: 'S',
  rows: [
    { index: 1, cells: [{ reference: 'A1', value: '标题', cell_type: 'string' }, { reference: 'B1', value: 1.5, number_format: '#,##0.00' }] },
    { index: 2, cells: [{ reference: 'A2', value: true }, { reference: 'C2', value: '标题' }] },
    { index: 4, cells: [{ reference: 'B4', value: -3, formula: 'A1-1', comment: '备注', style: 'bold' }] },
  ],
  merges: ['A1:C1'],
  default_row_height: 15,
}
const packedRes = packSheet(demoSheet, 20)
const pk = packedRes.packed
eq(isPackedSheet(pk), true, 'isPackedSheet 识别')
eq(pk.cellCount, 5, '打包单元格数')
eq(pk.maxCol, 3, '打包 maxCol（含合并区）')
eq(pk.strings.length, 1, '字符串池去重（重复的 "标题" 只存一份）')
eq(pk.strings[0], '标题', '字符串池内容')
eq(Array.from(pk.cols.slice(0, 2)), [1, 2], '行内升序（原 JSON 为 B1 在前，已按列号排好）')
eq(packedCellAt(pk, 1, 1)!.value, '标题', '乱序行仍可按列取到单元格')
eq(pk.formats.length, 1, '数字格式池去重')
eq(packedCellAt(pk, 1, 1)!.value, '标题', '打包读字符串')
eq(packedCellAt(pk, 1, 2)!.value, 1.5, '打包读数值')
eq(packedCellAt(pk, 1, 2)!.number_format, '#,##0.00', '打包读数字格式')
eq(packedCellAt(pk, 2, 1)!.value, true, '打包读布尔')
eq(packedCellAt(pk, 3, 1), null, '空行返回 null')
const formulaCell = packedCellAt(pk, 4, 2)!
eq(formulaCell.formula, 'A1-1', '低频字段（公式）经 extras 保留')
eq(formulaCell.comment, '备注', '低频字段（批注）保留')
eq(formulaCell.style, 'bold', '低频字段（样式）保留')
eq(pk.extras.size, 1, '与 kind 一致的 cell_type 不占 extras（仅公式单元格一条）')
eq(packedCellAt(pk, 1, 1)!.cell_type, null, '默认类型时 cell_type 为空（由 kind 推导）')
{
  const withType = packSheet(
    { name: 'T', rows: [{ index: 1, cells: [{ reference: 'A1', value: 45000, cell_type: 'date' }] }] },
    20,
  ).packed
  eq(packedCellAt(withType, 1, 1)!.cell_type, 'date', '非默认类型（date）保留在 extras')
}
const adp = adapterFor(pk, 20)
eq(Array.from(adp.rowNos), [1, 2, 4], '适配器行号（稀疏行保留）')
eq(adp.cell(1, 1)!.value, '标题', '适配器取数（打包）')
const jsonAdp = adapterFor(demoSheet, 20)
eq(jsonAdp.cell(4, 2)!.formula, 'A1-1', '适配器取数（JSON 原样）')
// 体积：列式形态每单元格约 21B（百万行×6 列 ≈ 126MB，而 JSON 模型约 800MB）
eq(packedMB(6_000_000 * 21) < 130, true, '600 万单元格列式估算 < 130MB')

// ---- 括号匹配 / 流式行扫描 ----
eq(matchObjectEnd('{"a":{"b":1}}', 0), 12, 'matchObjectEnd 嵌套对象')
eq(matchObjectEnd('{"a":"}"}', 0), 8, 'matchObjectEnd 字符串内的括号不计入嵌套')
eq(matchObjectEnd('[{"a":1},{"b":2}]', 1), 7, 'matchObjectEnd 数组内对象定位')
eq(matchObjectEnd('{"a":1', 0), -1, 'matchObjectEnd 未闭合')
{
  const scanner = new RowScanner()
  const rows: string[] = []
  scanner.onRow = (j) => rows.push(j)
  // 故意在关键字/行中间切断
  scanner.feed('{"name":"S","columns":[{"width":10}],"rows":[{"index":1,"cells":')
  scanner.feed('[{"reference":"A1","value":"x"}]},{"index":2,"cells":[{"value":2}]}],"charts":[]}')
  scanner.done()
  eq(rows.length, 2, '流式切分出 2 行')
  eq(JSON.parse(rows[0]).index, 1, '切分行内容正确')
  eq(JSON.parse(rows[1]).cells[0].value, 2, '第二行内容正确')
  const parts = JSON.parse(`{${scanner.headText.replace(/^\{\s*/, '').replace(/,\s*$/, '')}}`)
  eq(parts.name, 'S', '头部字段可解析')
  eq(scanner.tailText.trim().startsWith(','), true, '尾部文本保留')
}

// ---- 字段级扫描器：转义 / 数字 / 嵌套 / 乱序键 ----
{
  eq(readStringAt('"abc"', 0).value, 'abc', 'readStringAt 普通串')
  eq(readStringAt('"a\\tb\\u4e2d"', 0).value, 'a\tb中', 'readStringAt 转义与 \\u')
  eq(readNumberAt('-1.25e3', 0).value, -1250, 'readNumberAt 指数')
  eq(skipValueAt('{"a":[1,{"b":"}"}]}', 0), 19, 'skipValueAt 嵌套（含字符串内括号）')

  const rowText = '{"index":7,"height":30,"cells":'
    + '[{"reference":"B7","value":"x\\ny","number_format":"0.00"},'
    + '{"reference":"A7","comment":"注","value":-3.5e2},'
    + '{"value":true},{"value":null}]}'
  const seen: string[] = []
  const heights: number[] = []
  const indexes: (number | null)[] = []
  const cell = newParsedCell()
  const sink: RowSink = {
    cell,
    beginRow(index, heightPx) {
      indexes.push(index)
      heights.push(heightPx)
    },
    endRow() {
      seen.push('end')
    },
    endRowCell() {
      seen.push(`${cell.col}|${cell.kind}|${cell.num}|${cell.str ?? ''}|${cell.fmt ?? ''}|${cell.comment ?? ''}`)
    },
  }
  const end = parseRowAt(rowText, 0, sink)
  eq(end, rowText.length, 'parseRowAt 覆盖整行')
  eq(indexes, [7], '行号解析')
  eq(heights, [40], '行高 30pt → 40px')
  eq(seen[0], '2|2|0|x\ny|0.00|', '单元格1：列/类型/字符串/格式')
  eq(seen[1], '1|1|-350|||注', '单元格2：负数指数 + 批注')
  // 无 reference 的单元格由调用方按位置补列号（解析器留 0）
  eq(seen[2], '0|3|0|||', '单元格3：布尔（列号由消费方按位置补）')
  eq(seen[3], '0|0|0|||', '单元格4：null（列号由消费方按位置补）')
  eq(seen[4], 'end', '行结束回调')
}

// ---- 流式打包与 JSON.parse 打包结果一致 ----
async function streamEquality() {
  const sheet: XlsxSheet = {
    name: '流式',
    columns: [{ width: 12 }],
    merges: ['A1:B1'],
    rows: [
      { index: 1, cells: [{ reference: 'A1', value: '标题' }, { reference: 'B1', value: 1.25, number_format: '0.00' }] },
      { index: 3, height: 30, cells: [{ reference: 'B3', value: -2.5, formula: 'A1-B1', comment: '注' }, { reference: 'A3', value: false }] },
    ],
  }
  const json = JSON.stringify(sheet)
  const bytes = new TextEncoder().encode(json)
  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      // 按 16B 分块，模拟网络分片（考验跨块切分）
      for (let i = 0; i < bytes.length; i += 16) controller.enqueue(bytes.slice(i, i + 16))
      controller.close()
    },
  })
  const res = await packSheetFromStream(stream, { defaultRowPx: 20 })
  const packed = res.packed
  const reference = packSheet(sheet, 20).packed
  eq(Array.from(packed.rowNos), Array.from(reference.rowNos), '流式：行号一致')
  eq(Array.from(packed.rowStart), Array.from(reference.rowStart), '流式：行前缀一致')
  eq(Array.from(packed.cols), Array.from(reference.cols), '流式：列号一致')
  eq(Array.from(packed.kinds), Array.from(reference.kinds), '流式：类型码一致')
  eq(Array.from(packed.nums), Array.from(reference.nums), '流式：数值一致')
  eq(Array.from(packed.strs), Array.from(reference.strs), '流式：字符串下标一致')
  eq(Array.from(packed.fmts), Array.from(reference.fmts), '流式：格式下标一致')
  eq(Array.from(packed.heights), Array.from(reference.heights), '流式：行高一致')
  eq(packed.strings, reference.strings, '流式：字符串池一致')
  eq(packed.formats, reference.formats, '流式：格式池一致')
  eq(packed.maxCol, reference.maxCol, '流式：maxCol 一致')
  eq(packed.extras.size, reference.extras.size, '流式：低频字段条目一致')
  eq(packedCellAt(packed, 3, 2)!.formula, 'A1-B1', '流式：公式保留')
  eq(packedCellAt(packed, 3, 1)!.value, false, '流式：布尔保留')
  eq(packed.meta.merges, ['A1:B1'], '流式：合并区保留')
  // 无 reference 的单元格：流式与 JSON 路径都要按位置补列号
  {
    const noRef: XlsxSheet = {
      name: 'N',
      rows: [{ index: 1, cells: [{ value: 'a' }, { value: 'b' }, { value: 'c' }] }],
    }
    const bytes2 = new TextEncoder().encode(JSON.stringify(noRef))
    const st2 = new ReadableStream<Uint8Array>({
      start(c) {
        c.enqueue(bytes2)
        c.close()
      },
    })
    const packedNoRef = (await packSheetFromStream(st2, { defaultRowPx: 20 })).packed
    const refNoRef = packSheet(noRef, 20).packed
    eq(Array.from(packedNoRef.cols), Array.from(refNoRef.cols), '流式：无引用单元格列号按位置一致')
    eq(Array.from(packedNoRef.cols), [1, 2, 3], '按位置补列号为 1,2,3')
    eq(packedNoRef.maxCol, 3, '无引用时 maxCol 由位置推断')
  }
  eq((packed.meta.columns ?? [])[0].width, 12, '流式：列宽保留')
  eq(packed.name, '流式', '流式：表名保留')
}
// ---- pdf：路径与几何辅助 ----
{
  const types = ['text', 'rect', 'text']
  eq(pdfElementPath(0, [2], [types]), '/page[1]/text[2]', 'pdf 路径：按类型计数')
  eq(pdfElementPath(1, [0, 0], [['group'], ['text']]), '/page[2]/group[1]/text[1]', 'pdf 路径：group 下钻')
  const el = { type: 'text', text: 'hi', x: 72, y: 720, size: 24 }
  eq(pdfElementType(el), 'text', 'pdf 类型名')
  const bb = pdfElementBBox(el, 612, 792)
  eq(bb[0] < 72 && bb[3] <= 792 && bb[1] < 720, true, 'pdf 文本包围盒在基线上方')
  eq(pdfElementContent(el), 'hi', 'pdf 内容摘要')
  eq(pdfElementContent({ type: 'image', src: 'media/x.png' }), 'media/x.png', 'pdf 图片摘要')
}

// ---- docx：物理分页（硬分页符 + 按度量高度的软分页） ----
{
  /** 造块：top/bottom 为块自身范围（相对分片内容区顶端，自然 px）；分页符传 null */
  const b = (
    top: number | null,
    h = 40,
    extra: { breakBefore?: boolean; breakHere?: boolean } = {},
  ) => ({ top, bottom: top === null ? null : top + h, ...extra })
  const brk = { top: null, bottom: null, breakHere: true }
  const span = (r: { from: number; to: number }) => [r.from, r.to]
  const range = (r: { top: number; bottom: number }) => [r.top, r.bottom]

  // 硬分页：分页符把 5 个块切成 3 段
  {
    const pages = paginateBlocks(
      [b(0, 100), b(100, 100), brk, b(200, 50), brk, b(250, 20)],
      1000,
      270,
    )
    eq(pages.map(span), [[0, 2], [3, 4], [5, 6]], '硬分页：分页符切三段')
    eq(pages.map(range), [[0, 200], [200, 250], [250, 270]], '硬分页：内容范围随分段')
  }

  // 软分页：页高 100、块高 40 —— 第 3 块（80→120）放不下时另起一页
  {
    const pages = paginateBlocks([b(0), b(40), b(80), b(120), b(160)], 100, 200)
    eq(pages.map(span), [[0, 2], [2, 4], [4, 5]], '软分页：超出页高另起一页')
    eq(pages[1].top, 80, '软分页：续页顶端 = 该块顶端')
    eq(pages[0].bottom, 80, '软分页：底端 = 末块自身底端')
  }

  // 段前分页（page_break_before）与首块不制造空页
  {
    const pages = paginateBlocks([b(0, 40, { breakBefore: true }), b(40), b(80, 20, { breakBefore: true })], 1000, 100)
    eq(pages.map(span), [[0, 2], [2, 3]], '段前分页：切页且首块不产生空页')
  }

  // 单块超过页高：独占一页（不裁切，页面随后按内容长高）
  {
    const pages = paginateBlocks([b(0), b(40), b(80, 620)], 100, 700)
    eq(pages.map(span), [[0, 2], [2, 3]], '超高块：独占一页')
    eq(pages[1].bottom, 700, '超高块：底端取块自身底端')
  }

  // 未度量（pageH = Infinity / 高度全空）：只做硬分页
  {
    const pages = paginateBlocks([b(null), b(null), brk, b(null)], Number.POSITIVE_INFINITY, 0)
    eq(pages.map(span), [[0, 2], [3, 4]], '未度量：仅按分页符切')
  }

  // 尾部分页符不留空页；全分页符的分片仍给出一页可渲染
  {
    eq(paginateBlocks([b(0), brk], 1000, 100).map(span), [[0, 1]], '尾部分页符不留空页')
    const empty = paginateBlocks([brk], 1000, 0)
    eq(empty.length, 1, '全分页符：仍给出一页')
    eq(span(empty[0]), [0, 0], '全分页符：空页区间')
  }

  // 首页已占用高（连续分节：正文接在分栏带等内容之后续排）
  {
    // 两块共 90px、页高 100：无占用时整段一页；首页已占 45 时第二块放不下 → 另起一页
    const flow = [b(0, 60), b(60, 30)]
    eq(paginateBlocks(flow, 100, 90).map(span), [[0, 2]], '无占用：整段一页')
    const pages = paginateBlocks(flow, 100, 90, 45)
    eq(pages.map(span), [[0, 1], [1, 2]], '首页已占用：按剩余高度分页')
    eq(pages[1].top, 60, '首页已占用：续页顶端 = 该块顶端')
  }

  // 段前分页后必须恢复整页容量（占用高只作用于首页）
  {
    // 页高 200、首页已占 160：段前分页换页后，新页应能装下 190px（块 1+2）
    const pages = paginateBlocks(
      [b(0, 40), b(40, 40, { breakBefore: true }), b(80, 150), b(230, 20)],
      200,
      250,
      160,
    )
    eq(pages.map(span), [[0, 1], [1, 3], [3, 4]], '段前分页：换页后按整页容量续排')
    eq(pages[1].top, 40, '段前分页：新页顶端 = 该块顶端')
  }

  // 与下段同页（keepNext）：页底的标题要一起带到下一页
  {
    const kn = (
      top: number,
      h: number,
      keepWithNext = false,
    ): { top: number; bottom: number; keepWithNext?: boolean } => ({
      top,
      bottom: top + h,
      ...(keepWithNext ? { keepWithNext: true } : {}),
    })
    // 页高 100：a(0,40) + heading(40,20,keepNext) + body(60,90) —— body 溢出时标题一起下移
    const pages = paginateBlocks([kn(0, 40), kn(40, 20, true), kn(60, 90)], 100, 150)
    eq(pages.map(span), [[0, 1], [1, 3]], 'keepNext：页底标题随下段换页')
    eq(pages[1].top, 40, 'keepNext：新页顶端 = 标题顶端')
    // keepNext 链顶到页首：两块必须同页，页面随之撑高（不拆散）
    const allKeep = paginateBlocks([kn(0, 40, true), kn(40, 90, true)], 100, 130)
    eq(allKeep.map(span), [[0, 2]], 'keepNext：链顶到页首时页面撑高')
  }

  // 段内分页（孤行控制）：段落可在行边界切开，首尾各留 ≥2 行
  {
    /** 造可切段落：每行 10px、共 n 行 */
    const para = (top: number, n: number) => ({
      top,
      bottom: top + n * 10,
      lineTops: Array.from({ length: n }, (_, k) => k * 10),
    })
    // 页高 100：块 0 占 0..40，段落 8 行（40..120）→ 本页放 6 行，余 2 行续页
    const pages = paginateBlocks([b(0, 40), para(40, 8)], 100, 120)
    eq(pages.map(span), [[0, 2], [1, 2]], '段内分页：段落切成两页（续页仍标该块）')
    eq(pages.length, 2, '段内分页：共两页')
    eq(pages[0].bottom, 100, '段内分页：首页底端 = 第 6 行底端')
    eq(pages[0].trailKeep, 6, '段内分页：首页显示前 6 行')
    eq(pages[1].top, 100, '段内分页：续页顶端 = 第 7 行顶端')
    eq(pages[1].leadSkip, 6, '段内分页：续页从第 7 行开始')
    eq(pages[1].bottom, 120, '段内分页：续页底端 = 段落底端')

    // 孤行控制：只剩 1 行放得下 → 不切，整段挪到下一页
    const widow = paginateBlocks([b(0, 85), para(85, 4)], 100, 125)
    eq(widow.map(span), [[0, 1], [1, 2]], '孤行控制：本页剩 1 行时整段下移')
    eq(widow[1].leadSkip, undefined, '孤行控制：续页从块首行开始')

    // 续段仍超页：一段跨三页
    const long = paginateBlocks([para(0, 25)], 100, 250)
    eq(long.map(span), [[0, 1], [0, 1], [0, 1]], '长段：跨三页')
    eq(long.map((r) => r.trailKeep), [10, 20, undefined], '长段：逐页切到第 10/20 行')
    eq(long[1].leadSkip, 10, '长段：第二页从第 11 行开始')
    eq(long[1].top, 100, '长段：第二页顶端 = 第 11 行顶端')
  }

  // 表格跨页（Word 行为）：按行切开、续排片重复表头行
  {
    /** 造可切表格：每行 20px；headerRows 为表头行数 */
    const table = (top: number, rows: number, headerRows = 0) => ({
      top,
      bottom: top + rows * 20,
      rowBoxes: Array.from({ length: rows }, (_, k): [number, number] => [k * 20, k * 20 + 20]),
      headerRows,
    })
    // 无表头：8 行 160px、页高 100 → 首页 5 行，余 3 行续页
    const plain = paginateBlocks([table(0, 8)], 100, 160)
    eq(plain.map(span), [[0, 1], [0, 1]], '表格分页：按行切成两页（续页仍标该块）')
    eq(plain[0].bottom, 100, '表格分页：首页底端 = 第 5 行底端')
    eq(plain[0].trailRows, 5, '表格分页：首页放到第 5 行')
    eq(plain[1].leadRows, 5, '表格分页：续页从第 6 行开始')
    eq(plain[1].top, 100, '表格分页：续页顶端 = 第 6 行顶端')
    eq(plain[1].bottom, 160, '表格分页：续页底端 = 表尾')

    // 带 1 行表头：续排片（表头 20 + 3 行 60 = 80）→ 续页顶端上抬一个表头高
    const head = paginateBlocks([table(0, 8, 1)], 100, 160)
    eq(head.map(span), [[0, 1], [0, 1]], '表格分页：带表头切成两页')
    eq(head[0].trailRows, 5, '表格分页：表头连 4 行数据行留在首页')
    eq(head[1].leadRows, 5, '表格分页：续页从第 6 行（数据行）续排')
    eq(head[1].top, 80, '表格分页：续页顶端上抬表头高（重复表头占位）')
    eq(head[1].bottom, 160, '表格分页：续页底端仍为表尾')

    // 前方有内容：剩余高度放得下表头 + 至少 1 行数据行 → 就地在页底续排
    const after = paginateBlocks([b(0, 40), table(40, 5, 1)], 100, 140)
    eq(after.map(span), [[0, 2], [1, 2]], '表格分页：页底放得下就就地在页底续排')
    eq(after[0].trailRows, 3, '表格分页：首页放表头 + 2 行')
    eq(after[1].top, 80, '表格分页：续页顶端 = 表头 + 剩余行')

    // 连表头都放不下（剩余 15px）→ 整表挪到下一页
    const moved = paginateBlocks([b(0, 85), table(85, 5, 1)], 100, 185)
    eq(moved.map(span), [[0, 1], [1, 2]], '表格分页：放不下表头时整表下移')
    eq(moved[0].trailRows, undefined, '表格分页：整表下移时不切行')
    eq(moved[1].bottom, 185, '表格分页：整表落在下一页')

    // 单行比页还高（页首）：强行放行、页面长高，不死循环
    const tall = { top: 0, bottom: 360, rowBoxes: [[0, 120], [120, 240], [240, 360]] as [number, number][], headerRows: 1 }
    const grown = paginateBlocks([tall, b(360, 20)], 100, 380)
    eq(grown.length, 3, '表格分页：超页高的行强行放行 → 三页收口')
    eq(grown[0].bottom, 240, '表格分页：页面按放下的行长高')
    eq(grown[0].trailRows, 2, '表格分页：一次放下表头 + 1 行')
    eq(grown[2].bottom, 380, '表格分页：末页收在流末尾')
  }

  // 表格行内切分（Word 允许一行跨页：行内文本按行接着排）
  {
    /** 造可切到行内的表格：行盒/内容盒相对块顶端（与度量一致），top/bottom 为自然坐标 */
    const rowsTable = (
      absTop: number,
      spec: Array<{ lines: number; lineH?: number; pad?: number }>,
      headerRows = 0,
    ) => {
      const rowBoxes: [number, number][] = []
      const rowContent: [number, number][] = []
      const rowLines: number[] = []
      let y = 0
      for (const r of spec) {
        const pad = r.pad ?? 0
        const lineH = r.lineH ?? 20
        const h = r.lines * lineH + pad * 2
        rowBoxes.push([y, y + h])
        rowContent.push([y + pad, y + h - pad])
        rowLines.push(r.lines)
        y += h
      }
      return { top: absTop, bottom: absTop + y, rowBoxes, rowContent, rowLines, headerRows }
    }

    // 表头 1 行 + 一行 3 行文本：页底剩 60px（表头 20 + 2 行 40）→ 行内切在第 2 行
    const cut = paginateBlocks([b(0, 40), rowsTable(40, [{ lines: 1 }, { lines: 3 }], 1)], 100, 120)
    eq(cut.map(span), [[0, 2], [1, 2]], '行内切：一行跨页（续页仍标该块）')
    eq(cut[0].trailRows, 2, '行内切：首页画到第 2 行（末行只画前 2 行文本）')
    eq(cut[0].trailLines, 2, '行内切：末行画 2 行文本')
    eq(cut[0].bottom, 100, '行内切：底端 = 第 2 行文本底端（不含下内边距）')
    eq(cut[1].leadRows, 1, '行内切：续页还是这一行')
    eq(cut[1].leadLines, 2, '行内切：续页跳过前 2 行文本')
    eq(cut[1].top, 80, '行内切：续页顶端 = 表头 + 剩余 1 行')
    eq(cut[1].bottom, 120, '行内切：续页底端 = 表尾')

    // 一行跨三页：表头 1 行 + 一行 12 行文本，页高 100（表头 20 + 每次 4 行 80）
    const long = paginateBlocks([rowsTable(0, [{ lines: 1 }, { lines: 12 }], 1)], 100, 260)
    eq(long.length, 3, '行内切：一行跨三页')
    eq(long.map((r) => r.trailLines), [4, 8, undefined], '行内切：逐页切到第 4/8 行文本')
    eq(long.map((r) => r.leadLines), [undefined, 4, 8], '行内切：续页跳过 4/8 行文本')
    eq(long.map((r) => r.top), [0, 80, 160], '行内切：每页顶端递进（含重复表头占位）')
    eq(long[2].bottom, 260, '行内切：末页收在表尾')

    // 连表头 + 1 行文本都放不下 → 整表下移（行内切也要守这条）
    const none = paginateBlocks([b(0, 85), rowsTable(85, [{ lines: 1 }, { lines: 2 }], 1)], 100, 145)
    eq(none.map(span), [[0, 1], [1, 2]], '行内切：放不下表头 + 1 行时整表下移')
  }
}

// Word 兼容断行（禁则 + 行末标点悬挂）：宽高都用合成量宽，不依赖 DOM
{
  // 汉字按 size 计宽（1em，与 buildUnits 的全角规则一致）；西文/空格按 8px/字（measure）
  const charsOf = (text: string, size = 10): StyledChar[] =>
    [...text].map((ch) => ({ ch, font: 't', size }))
  const measure: MeasureText = (t) => t.length * 8
  const widths = (text: string, opts: { avail: number; firstAvail?: number; hangMax?: number; size?: number }) =>
    layoutParagraph(charsOf(text, opts.size ?? 10), {
      firstAvail: opts.firstAvail ?? opts.avail,
      avail: opts.avail,
      hangMax: opts.hangMax ?? 12,
      measure,
    })
  const lens = (text: string, opts: { avail: number; firstAvail?: number; hangMax?: number; size?: number }) =>
    widths(text, opts).map((l) => l.to - l.from)
  const hangs = (text: string, opts: { avail: number; firstAvail?: number; hangMax?: number; size?: number }) =>
    widths(text, opts).map((l) => l.hang)

  eq(lens('一'.repeat(25), { avail: 100 }), [10, 10, 5], '断行：贪心排字（全角 1em）')
  eq(
    widths('一'.repeat(10) + '二三', { avail: 100 }).map((l) => [l.from, l.to]),
    [
      [0, 10],
      [10, 12],
    ],
    '断行：字符区间推进正确',
  )
  // 行末句读放不下 → 悬挂出版心（Word 的 overflowPunct）
  eq(lens('一'.repeat(10) + '，二', { avail: 100 }), [11, 1], '悬挂：句读留在上一行')
  eq(hangs('一'.repeat(10) + '，二', { avail: 100 }), [true, false], '悬挂：标记挂出版心')
  eq(lens('一'.repeat(10) + '。', { avail: 100 }), [11], '悬挂：末字悬挂也成立')
  // 标点比「一个字」还宽（hangMax 之外）→ 不敢挂，连前一个字一起下移（行首禁则）
  eq(lens('一'.repeat(10) + '，二', { avail: 100, hangMax: 6 }), [9, 3], '禁则：挂不下时前一字跟下移')
  eq(hangs('一'.repeat(10) + '，二', { avail: 100, hangMax: 6 }), [false, false], '禁则：不悬挂')
  // 行尾禁则：开括号不能落在行末
  eq(lens('一'.repeat(9) + '（二', { avail: 100 }), [9, 2], '禁则：开括号不下留行末')
  // 行尾空白丢弃（Word 断行吞掉断点处的空格），空格不占行尾
  eq(lens('一'.repeat(9) + ' ab', { avail: 100 }), [9, 2], '断行：行尾空格丢弃')
  eq(
    spansOf(charsOf('一'.repeat(9) + ' ab'), 0, 9).map((s) => s.text),
    ['一一一一一一一一一'],
    '断行：行尾空格不进本行文本',
  )
  // 首行缩进：首行更窄，其余行按版心
  eq(lens('一'.repeat(12), { avail: 100, firstAvail: 50 }), [5, 7], '断行：首行缩进')
  // 超宽单元（超长西文串）独占一行，保证推进不死循环
  eq(lens('abcdefghij', { avail: 40 }), [10], '断行：超宽西文串独占一行')
  // 强制换行符：每段独立断行，区间带 \n 偏移
  eq(
    widths('一二\n三四五六', { avail: 100 }).map((l) => [l.from, l.to]),
    [
      [0, 2],
      [3, 7],
    ],
    '断行：\\n 强制分段',
  )
  // 同样式字符并段（渲染用）
  const styled = charsOf('甲甲', 10).map((c) => ({ ...c, style: { 'font-weight': 'bold' } }))
  eq(spansOf(styled, 0, 2).length, 1, 'spansOf：同样式并段')
  eq(spansOf(charsOf('甲乙', 10), 0, 2).length, 1, 'spansOf：无样式同样并段')
  const mixed = charsOf('甲乙', 10)
  mixed[1] = { ...mixed[1], style: { color: 'red' } }
  eq(spansOf(mixed, 0, 2).length, 2, 'spansOf：样式不同则分段')
}

// ---- 手势缩放纯函数（滚轮 / 捏合 / 滚动锚定） ----
{
  // 滚轮：上滚（ΔY<0）放大、下滚缩小，一格 ≈ ×1.25
  eq(wheelZoomAt(1, -100) > 1.2 && wheelZoomAt(1, -100) < 1.3, true, '滚轮上滚一格 ≈ 放大 1.25×')
  eq(wheelZoomAt(1, 100) < 0.81 && wheelZoomAt(1, 100) > 0.79, true, '滚轮下滚一格 ≈ 缩小 0.8×')
  eq(wheelZoomAt(2.9, -1000, 0, 0.5, 3), 3, '滚轮放大钳到 max')
  eq(wheelZoomAt(0.6, 1000, 0, 0.5, 3), 0.5, '滚轮缩小钳到 min')
  // deltaMode 归一（Firefox 行模式）：3 行 ≈ 48px，两模式结果一致
  eq(Math.abs(wheelZoomAt(1, 3, 1) - wheelZoomAt(1, 48, 0)) < 1e-12, true, 'deltaMode 行模式归一到像素')
  // 捏合：距离比 = 倍率比（绝对映射）；起点距离非法时不动
  eq(pinchZoomAt(1, 100, 200, 0.5, 3), 2, '捏合两指距离翻倍 → 倍率翻倍')
  eq(Math.abs(pinchZoomAt(0.8, 160, 100, 0.5, 3) - 0.5) < 1e-12, true, '捏合收拢钳到 min')
  eq(pinchZoomAt(1, 0, 200, 0.5, 3), 1, '捏合起点距离为 0 → 原值')
  // 滚动锚定：指针下的内容点缩放前后保持在指针下
  eq(scrollAnchor(100, 50, 2), 250, '锚定：滚动 + 指针偏移按比例放大后回减')
  eq(scrollAnchor(0, 300, 0.5), -150, '锚定：缩小时目标为负 → 容器钳回 0 即居中')
}

const streamDone = streamEquality()
streamDone
  .then(() => {
    if (failed > 0) {
      console.error(`\n${failed} 个断言失败`)
      process.exit(1)
    }
    console.log('\n全部断言通过')
  })
  .catch((e) => {
    console.error('流式测试异常:', e)
    process.exit(1)
  })

