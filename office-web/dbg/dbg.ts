import { packSheetFromStream } from '../src/renderers/xlsx/stream'
import { packSheet, packedCellAt } from '../src/renderers/xlsx/packed'
import type { XlsxSheet } from '../src/types'

const sheet: XlsxSheet = {
  name: '流式',
  rows: [
    { index: 1, cells: [{ reference: 'A1', value: '标题' }, { reference: 'B1', value: 1.25, number_format: '0.00' }] },
    { index: 3, height: 30, cells: [{ reference: 'B3', value: -2.5, formula: 'A1-B1', comment: '注' }, { reference: 'A3', value: false }] },
  ],
}
const text = JSON.stringify(sheet)
const bytes = new TextEncoder().encode(text)
const stream = new ReadableStream<Uint8Array>({
  start(c) { for (let i = 0; i < bytes.length; i += 16) c.enqueue(bytes.slice(i, i + 16)); c.close() },
})
packSheetFromStream(stream, { defaultRowPx: 20 }).then((res) => {
  const p = res.packed
  console.log('rowNos', Array.from(p.rowNos), 'rowStart', Array.from(p.rowStart), 'cols', Array.from(p.cols), 'kinds', Array.from(p.kinds))
  console.log('strs', Array.from(p.strs), 'strings', p.strings, 'fmts', Array.from(p.fmts), 'extras', Array.from(p.extras.entries()))
  const ref = packSheet(sheet, 20)
  console.log('ref rowNos', Array.from(ref.packed.rowNos), 'cols', Array.from(ref.packed.cols))
})
