"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
const stream_1 = require("../Users/xuxin/workspace/astorm-office/office-web/src/renderers/xlsx/stream");
const packed_1 = require("../Users/xuxin/workspace/astorm-office/office-web/src/renderers/xlsx/packed");
const sheet = {
    name: '流式',
    rows: [
        { index: 1, cells: [{ reference: 'A1', value: '标题' }, { reference: 'B1', value: 1.25, number_format: '0.00' }] },
        { index: 3, height: 30, cells: [{ reference: 'B3', value: -2.5, formula: 'A1-B1', comment: '注' }, { reference: 'A3', value: false }] },
    ],
};
const text = JSON.stringify(sheet);
const bytes = new TextEncoder().encode(text);
const stream = new ReadableStream({
    start(c) { for (let i = 0; i < bytes.length; i += 16)
        c.enqueue(bytes.slice(i, i + 16)); c.close(); },
});
(0, stream_1.packSheetFromStream)(stream, { defaultRowPx: 20 }).then((res) => {
    const p = res.packed;
    console.log('rowNos', Array.from(p.rowNos), 'rowStart', Array.from(p.rowStart), 'cols', Array.from(p.cols), 'kinds', Array.from(p.kinds));
    console.log('strs', Array.from(p.strs), 'strings', p.strings, 'fmts', Array.from(p.fmts), 'extras', Array.from(p.extras.entries()));
    const ref = (0, packed_1.packSheet)(sheet, 20);
    console.log('ref rowNos', Array.from(ref.packed.rowNos), 'cols', Array.from(ref.packed.cols));
});
