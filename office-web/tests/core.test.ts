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

