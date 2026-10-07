"use strict";
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
Object.defineProperty(exports, "__esModule", { value: true });
exports.packSheetFromStream = exports.matchObjectEnd = exports.RowScanner = exports.parseRowAt = exports.skipValueAt = exports.readNumberAt = exports.readStringAt = exports.skipWsAt = exports.newParsedCell = void 0;
const packed_1 = require("./packed");
const virtual_1 = require("./virtual");
/** 可增长的列式写入器（未知总规模，按块扩容） */
class PackedWriter {
    constructor() {
        this.cap = 4096;
        this.n = 0;
        this.rowNos = [];
        this.rowStart = [0];
        this.cols = new Uint16Array(this.cap);
        this.nums = new Float64Array(this.cap);
        this.strs = new Int32Array(this.cap).fill(-1);
        this.bools = new Uint8Array(this.cap);
        this.fmts = new Int32Array(this.cap).fill(-1);
        this.kinds = new Uint8Array(this.cap);
        this.heights = [];
        this.strings = [];
        this.formats = [];
        this.extras = new Map();
        this.strIndex = new Map();
        this.fmtIndex = new Map();
        this.maxCol = 0;
    }
    grow() {
        const cap = this.cap * 2;
        const cols = new Uint16Array(cap);
        cols.set(this.cols);
        const nums = new Float64Array(cap);
        nums.set(this.nums);
        const strs = new Int32Array(cap).fill(-1);
        strs.set(this.strs);
        const bools = new Uint8Array(cap);
        bools.set(this.bools);
        const fmts = new Int32Array(cap).fill(-1);
        fmts.set(this.fmts);
        const kinds = new Uint8Array(cap);
        kinds.set(this.kinds);
        this.cols = cols;
        this.nums = nums;
        this.strs = strs;
        this.bools = bools;
        this.fmts = fmts;
        this.kinds = kinds;
        this.cap = cap;
    }
    /** 字段级扫描路径：把复用 cell 对象写入数组 */
    pushParsedCell(cell) {
        if (this.n >= this.cap)
            this.grow();
        const k = this.n;
        this.cols[k] = cell.col;
        if (cell.col > this.maxCol)
            this.maxCol = cell.col;
        this.kinds[k] = cell.kind;
        if (cell.kind === packed_1.KIND.number)
            this.nums[k] = cell.num;
        else if (cell.kind === packed_1.KIND.bool)
            this.bools[k] = cell.bool ? 1 : 0;
        else if (cell.kind === packed_1.KIND.string && cell.str !== null) {
            let id = this.strIndex.get(cell.str);
            if (id === undefined) {
                id = this.strings.length;
                this.strings.push(cell.str);
                this.strIndex.set(cell.str, id);
            }
            this.strs[k] = id;
        }
        if (cell.fmt) {
            let id = this.fmtIndex.get(cell.fmt);
            if (id === undefined) {
                id = this.formats.length;
                this.formats.push(cell.fmt);
                this.fmtIndex.set(cell.fmt, id);
            }
            this.fmts[k] = id;
        }
        if (cell.hasExtra) {
            this.extras.set(k, {
                formula: cell.formula ?? undefined,
                cell_type: cell.cellType ?? undefined,
                style: cell.style,
                link: cell.link ?? undefined,
                link_tooltip: cell.linkTooltip ?? undefined,
                comment: cell.comment ?? undefined,
            });
        }
        this.n += 1;
    }
    /** 行尾：写入行号/行高并做行内有序化 */
    endParsedRow(rowNo, heightPx) {
        this.rowNos.push(rowNo);
        this.heights.push(heightPx);
        const s0 = this.rowStart[this.rowStart.length - 1];
        (0, packed_1.ensureRowSorted)({
            cols: this.cols,
            nums: this.nums,
            strs: this.strs,
            bools: this.bools,
            fmts: this.fmts,
            kinds: this.kinds,
            extras: this.extras,
        }, s0, this.n);
        this.rowStart.push(this.n);
    }
    pushRow(row, rowNo, heightPx) {
        this.rowNos.push(rowNo);
        this.heights.push(heightPx);
        const cells = row.cells ?? [];
        let pos = 1;
        for (const c of cells) {
            let col = pos;
            if (c.reference) {
                const p = (0, virtual_1.refToPos)(c.reference);
                if (p)
                    col = p.col;
            }
            pos = col + 1;
            if (this.n >= this.cap)
                this.grow();
            const k = this.n;
            this.cols[k] = col;
            if (col > this.maxCol)
                this.maxCol = col;
            const v = c.value;
            if (typeof v === 'number') {
                this.kinds[k] = packed_1.KIND.number;
                this.nums[k] = v;
            }
            else if (typeof v === 'boolean') {
                this.kinds[k] = packed_1.KIND.bool;
                this.bools[k] = v ? 1 : 0;
            }
            else if (typeof v === 'string') {
                this.kinds[k] = packed_1.KIND.string;
                let id = this.strIndex.get(v);
                if (id === undefined) {
                    id = this.strings.length;
                    this.strings.push(v);
                    this.strIndex.set(v, id);
                }
                this.strs[k] = id;
            }
            else {
                this.kinds[k] = packed_1.KIND.empty;
            }
            if (c.number_format) {
                let id = this.fmtIndex.get(c.number_format);
                if (id === undefined) {
                    id = this.formats.length;
                    this.formats.push(c.number_format);
                    this.fmtIndex.set(c.number_format, id);
                }
                this.fmts[k] = id;
            }
            const kindNow = this.kinds[k];
            const defaultType = kindNow === packed_1.KIND.number ? 'number' : kindNow === packed_1.KIND.string ? 'string' : kindNow === packed_1.KIND.bool ? 'boolean' : '';
            const extraType = c.cell_type && c.cell_type !== defaultType ? c.cell_type : undefined;
            if (c.formula || c.style || c.link || c.link_tooltip || c.comment || extraType) {
                this.extras.set(k, {
                    formula: c.formula,
                    cell_type: extraType,
                    style: c.style,
                    link: c.link,
                    link_tooltip: c.link_tooltip,
                    comment: c.comment,
                });
            }
            this.n += 1;
        }
        const s0 = this.rowStart[this.rowStart.length - 1];
        (0, packed_1.ensureRowSorted)({
            cols: this.cols,
            nums: this.nums,
            strs: this.strs,
            bools: this.bools,
            fmts: this.fmts,
            kinds: this.kinds,
            extras: this.extras,
        }, s0, this.n);
        this.rowStart.push(this.n);
    }
    finish(name, defaultRowPx, meta) {
        const n = this.n;
        const packed = {
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
        };
        let extraBytes = 0;
        this.extras.forEach((e) => {
            extraBytes += 80;
            for (const v of [e.formula, e.cell_type, e.link, e.link_tooltip, e.comment]) {
                if (typeof v === 'string')
                    extraBytes += v.length * 2 + 24;
            }
        });
        const bytes = packed.rowNos.byteLength +
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
            extraBytes;
        return { packed, packMs: 0, bytes };
    }
}
function newParsedCell() {
    return {
        col: 0,
        kind: packed_1.KIND.empty,
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
    };
}
exports.newParsedCell = newParsedCell;
function skipWsAt(s, i) {
    while (i < s.length) {
        const c = s.charCodeAt(i);
        if (c === 32 || c === 10 || c === 13 || c === 9)
            i += 1;
        else
            break;
    }
    return i;
}
exports.skipWsAt = skipWsAt;
/** 读取 JSON 字符串（s[i] === '"'），返回 { value, end } */
function readStringAt(s, i) {
    let j = i + 1;
    let simple = true;
    while (j < s.length) {
        const c = s.charCodeAt(j);
        if (c === 92) {
            simple = false;
            j += 2;
            continue;
        }
        if (c === 34)
            break;
        j += 1;
    }
    const raw = s.slice(i + 1, j);
    if (simple)
        return { value: raw, end: j + 1 };
    // 慢路径：处理转义（含 \uXXXX 与代理对）
    let out = '';
    for (let k = 0; k < raw.length; k += 1) {
        if (raw.charCodeAt(k) !== 92) {
            out += raw[k];
            continue;
        }
        const e = raw[k + 1];
        k += 1;
        switch (e) {
            case '"':
                out += '"';
                break;
            case '\\':
                out += '\\';
                break;
            case '/':
                out += '/';
                break;
            case 'b':
                out += '\b';
                break;
            case 'f':
                out += '\f';
                break;
            case 'n':
                out += '\n';
                break;
            case 'r':
                out += '\r';
                break;
            case 't':
                out += '\t';
                break;
            case 'u': {
                const hex = raw.slice(k + 1, k + 5);
                out += String.fromCharCode(parseInt(hex, 16));
                k += 4;
                break;
            }
            default: out += e ?? '';
        }
    }
    return { value: out, end: j + 1 };
}
exports.readStringAt = readStringAt;
/** 读取数字（含指数） */
function readNumberAt(s, i) {
    let j = i;
    while (j < s.length) {
        const c = s.charCodeAt(j);
        if ((c >= 48 && c <= 57) || c === 43 || c === 45 || c === 46 || c === 101 || c === 69)
            j += 1;
        else
            break;
    }
    return { value: Number(s.slice(i, j)), end: j };
}
exports.readNumberAt = readNumberAt;
/** 跳过任意 JSON 值（对象/数组/字符串/数字/字面量），返回结束下标 */
function skipValueAt(s, i) {
    i = skipWsAt(s, i);
    const c = s[i];
    if (c === '"')
        return readStringAt(s, i).end;
    if (c === '{' || c === '[') {
        let depth = 0;
        let inStr = false;
        let esc = false;
        for (let j = i; j < s.length; j += 1) {
            const ch = s[j];
            if (inStr) {
                if (esc)
                    esc = false;
                else if (ch === '\\')
                    esc = true;
                else if (ch === '"')
                    inStr = false;
                continue;
            }
            if (ch === '"')
                inStr = true;
            else if (ch === '{' || ch === '[')
                depth += 1;
            else if (ch === '}' || ch === ']') {
                depth -= 1;
                if (depth === 0)
                    return j + 1;
            }
        }
        return s.length;
    }
    // 数字/true/false/null
    let j = i;
    while (j < s.length && !',}]'.includes(s[j]))
        j += 1;
    return j;
}
exports.skipValueAt = skipValueAt;
/** 解析一行（`{...}`），通过 sink 产出；返回行结束下标（失败返回 -1） */
function parseRowAt(s, start, sink) {
    let i = skipWsAt(s, start);
    if (s[i] !== '{')
        return -1;
    i += 1;
    let index = null;
    let heightPx = 0;
    let began = false;
    const cell = sink.cell;
    for (;;) {
        i = skipWsAt(s, i);
        if (s[i] === '}')
            return i + 1;
        if (s[i] !== '"')
            return -1;
        const key = readStringAt(s, i);
        i = skipWsAt(s, key.end);
        if (s[i] !== ':')
            return -1;
        i = skipWsAt(s, i + 1);
        if (key.value === 'index') {
            const n = readNumberAt(s, i);
            index = n.value;
            i = n.end;
        }
        else if (key.value === 'height') {
            if (s[i] === 'n') {
                i += 4; // null
            }
            else {
                const n = readNumberAt(s, i);
                heightPx = (n.value * 96) / 72;
                i = n.end;
            }
        }
        else if (key.value === 'cells') {
            if (!began) {
                sink.beginRow(index, heightPx);
                began = true;
            }
            if (s[i] !== '[')
                return -1;
            i += 1;
            for (;;) {
                i = skipWsAt(s, i);
                if (s[i] === ']') {
                    i += 1;
                    break;
                }
                if (s[i] === ',') {
                    i += 1;
                    continue;
                }
                if (s[i] !== '{')
                    return -1;
                i = parseCellAt(s, i, cell);
                if (i < 0)
                    return -1;
                sink.endRowCell?.();
            }
        }
        else {
            i = skipValueAt(s, i);
        }
        i = skipWsAt(s, i);
        if (s[i] === ',') {
            i += 1;
            continue;
        }
        if (s[i] === '}') {
            if (!began)
                sink.beginRow(index, heightPx);
            return i + 1;
        }
        if (i >= s.length)
            return -1;
        i += 1;
    }
}
exports.parseRowAt = parseRowAt;
/** 解析单个单元格（`{...}`）；结果写入复用的 cell 对象 */
function parseCellAt(s, start, cell) {
    cell.col = 0;
    cell.kind = packed_1.KIND.empty;
    cell.num = 0;
    cell.str = null;
    cell.bool = false;
    cell.fmt = null;
    cell.formula = null;
    cell.cellType = null;
    cell.style = undefined;
    cell.link = null;
    cell.linkTooltip = null;
    cell.comment = null;
    cell.hasExtra = false;
    let i = start + 1;
    for (;;) {
        i = skipWsAt(s, i);
        if (s[i] === '}')
            return i + 1;
        if (s[i] !== '"')
            return -1;
        const key = readStringAt(s, i);
        i = skipWsAt(s, key.end);
        if (s[i] !== ':')
            return -1;
        i = skipWsAt(s, i + 1);
        switch (key.value) {
            case 'reference': {
                const v = readStringAt(s, i);
                const p = (0, virtual_1.refToPos)(v.value);
                if (p)
                    cell.col = p.col;
                i = v.end;
                break;
            }
            case 'value': {
                const c = s[i];
                if (c === '"') {
                    const v = readStringAt(s, i);
                    cell.kind = packed_1.KIND.string;
                    cell.str = v.value;
                    i = v.end;
                }
                else if (c === 't') {
                    cell.kind = packed_1.KIND.bool;
                    cell.bool = true;
                    i += 4;
                }
                else if (c === 'f') {
                    cell.kind = packed_1.KIND.bool;
                    cell.bool = false;
                    i += 5;
                }
                else if (c === 'n') {
                    cell.kind = packed_1.KIND.empty;
                    i += 4;
                }
                else if (c === '{' || c === '[') {
                    // 富文本等复杂值：原样保留为 JSON 文本
                    const end = skipValueAt(s, i);
                    cell.kind = packed_1.KIND.string;
                    cell.str = s.slice(i, end);
                    cell.cellType = 'json';
                    cell.hasExtra = true;
                    i = end;
                }
                else {
                    const n = readNumberAt(s, i);
                    cell.kind = packed_1.KIND.number;
                    cell.num = n.value;
                    i = n.end;
                }
                break;
            }
            case 'number_format': {
                const v = readStringAt(s, i);
                cell.fmt = v.value;
                i = v.end;
                break;
            }
            case 'formula': {
                if (s[i] === 'n') {
                    i += 4;
                }
                else {
                    const v = readStringAt(s, i);
                    cell.formula = v.value;
                    cell.hasExtra = true;
                    i = v.end;
                }
                break;
            }
            case 'cell_type': {
                const v = readStringAt(s, i);
                cell.cellType = v.value;
                cell.hasExtra = true;
                i = v.end;
            }
            case 'link': {
                const v = readStringAt(s, i);
                cell.link = v.value;
                cell.hasExtra = true;
                i = v.end;
                break;
            }
            case 'link_tooltip': {
                const v = readStringAt(s, i);
                cell.linkTooltip = v.value;
                cell.hasExtra = true;
                i = v.end;
                break;
            }
            case 'comment': {
                const v = readStringAt(s, i);
                cell.comment = v.value;
                cell.hasExtra = true;
                i = v.end;
                break;
            }
            case 'style': {
                const end = skipValueAt(s, i);
                const rawText = s.slice(i, end);
                try {
                    cell.style = JSON.parse(rawText);
                }
                catch {
                    cell.style = rawText;
                }
                cell.hasExtra = true;
                i = end;
                break;
            }
            default:
                i = skipValueAt(s, i);
        }
        i = skipWsAt(s, i);
        if (s[i] === ',') {
            i += 1;
            continue;
        }
        if (s[i] === '}')
            return i + 1;
        return -1;
    }
}
/** 扫描器：在字符缓冲上做括号匹配，逐行吐出 JSON 文本 */
class RowScanner {
    constructor() {
        this.buf = '';
        /** 'seek-rows' 之前累积头部文本；'in-rows' 逐行切分；'tail' 累积尾部 */
        this.phase = 'seek-rows';
        this.headText = '';
        this.tailText = '';
        /** 已切分出的行 JSON 文本回调（兼容路径） */
        this.onRow = () => { };
        /** 行边界回调：直接在缓冲内解析，避免每行一次字符串切片 */
        this.onRowText = (text, start, end) => {
            this.onRow(text.slice(start, end));
            return end;
        };
    }
    /** 消费一个文本分块；可能产出多行 */
    feed(chunk) {
        this.buf += chunk;
        if (this.phase === 'seek-rows') {
            const i = this.buf.indexOf('"rows"');
            if (i < 0) {
                // 保留少量尾部以防关键字被切断
                if (this.buf.length > 16) {
                    this.headText += this.buf.slice(0, -16);
                    this.buf = this.buf.slice(-16);
                }
                return;
            }
            this.headText += this.buf.slice(0, i);
            const open = this.buf.indexOf('[', i + 6);
            if (open < 0) {
                this.buf = this.buf.slice(i);
                return;
            }
            this.buf = this.buf.slice(open + 1);
            this.phase = 'in-rows';
        }
        if (this.phase === 'in-rows')
            this.scanRows();
        if (this.phase === 'tail') {
            this.tailText += this.buf;
            this.buf = '';
        }
    }
    scanRows() {
        let i = 0;
        const s = this.buf;
        while (i < s.length) {
            // 跳过空白与逗号
            const c = s[i];
            if (c === ' ' || c === '\n' || c === '\r' || c === '\t' || c === ',') {
                i += 1;
                continue;
            }
            if (c === ']') {
                // rows 数组结束 → 余下为尾部
                this.tailText = s.slice(i + 1);
                this.buf = '';
                this.phase = 'tail';
                return;
            }
            if (c !== '{') {
                // 容错：未知内容丢弃
                i += 1;
                continue;
            }
            const end = matchObjectEnd(s, i);
            if (end < 0)
                break; // 行未完整，等下一个分块
            const next = this.onRowText(s, i, end + 1);
            i = next > i ? next : end + 1;
        }
        this.buf = s.slice(i);
    }
    /** 结束：返回头部与尾部文本（供解析小字段与元信息） */
    done() {
        if (this.phase === 'seek-rows') {
            this.headText += this.buf;
            this.buf = '';
        }
        else if (this.phase === 'in-rows') {
            this.scanRows();
            this.tailText += this.buf;
            this.buf = '';
        }
    }
}
exports.RowScanner = RowScanner;
/** 从 s[start]（'{'）找到匹配的 '}'，处理字符串与转义；未找到返回 -1 */
function matchObjectEnd(s, start) {
    let depth = 0;
    let inStr = false;
    let esc = false;
    for (let i = start; i < s.length; i += 1) {
        const c = s[i];
        if (inStr) {
            if (esc)
                esc = false;
            else if (c === '\\')
                esc = true;
            else if (c === '"')
                inStr = false;
            continue;
        }
        if (c === '"')
            inStr = true;
        else if (c === '{' || c === '[')
            depth += 1;
        else if (c === '}' || c === ']') {
            depth -= 1;
            if (depth === 0)
                return i;
        }
    }
    return -1;
}
exports.matchObjectEnd = matchObjectEnd;
/** 把头部/尾部文本解析成对象（两者都很小） */
function parseParts(headText, tailText) {
    let head = {};
    const h = headText.trim().replace(/^\{\s*/, '').replace(/,\s*$/, '');
    if (h.trim()) {
        try {
            head = JSON.parse(`{${h}}`);
        }
        catch {
            head = {};
        }
    }
    let tail = {};
    const t = tailText.trim().replace(/^\s*,\s*/, '');
    const tt = t.startsWith('}') ? '' : t.replace(/,\s*$/, '');
    if (tt.trim()) {
        try {
            tail = JSON.parse(`{${tt}`);
        }
        catch {
            tail = {};
        }
    }
    return { ...head, ...tail };
}
/** 把 ReadableStream（UTF-8 文本）流式解析为 PackedSheet */
async function packSheetFromStream(stream, opts = {}) {
    const t0 = typeof performance !== 'undefined' ? performance.now() : 0;
    const defaultRowPx = opts.defaultRowPx ?? 20;
    const writer = new PackedWriter();
    const scanner = new RowScanner();
    let seq = 0;
    let pendingIndex = null;
    let pendingHeight = 0;
    let cellCol = 1;
    const sink = {
        cell: newParsedCell(),
        beginRow(index, heightPx) {
            seq += 1;
            pendingIndex = index;
            pendingHeight = heightPx;
            cellCol = 0;
        },
        endRow() {
            writer.endParsedRow(pendingIndex ?? seq, pendingHeight);
        },
    };
    sink.endRowCell = () => {
        const c = sink.cell;
        cellCol += 1;
        if (c.col === 0)
            c.col = cellCol;
        else
            cellCol = c.col;
        writer.pushParsedCell(c);
    };
    // 字段级扫描（单遍）；遇到无法解析的行再回落到 JSON.parse
    scanner.onRowText = (text, start, end) => {
        const next = parseRowAt(text, start, sink);
        if (next < 0 || next > end) {
            const row = JSON.parse(text.slice(start, end));
            seq += 1;
            writer.pushRow(row, row.index ?? seq, row.height ? (row.height * 96) / 72 : 0);
        }
        return next < 0 ? end : next;
    };
    scanner.onRow = (json) => {
        const row = JSON.parse(json);
        seq += 1;
        writer.pushRow(row, row.index ?? seq, row.height ? (row.height * 96) / 72 : 0);
    };
    const reader = stream.getReader();
    const dec = new TextDecoder();
    let chunks = 0;
    for (;;) {
        const { done, value } = await reader.read();
        if (done)
            break;
        chunks += 1;
        scanner.feed(dec.decode(value, { stream: true }));
    }
    scanner.feed(dec.decode());
    scanner.done();
    const parts = parseParts(scanner.headText, scanner.tailText);
    const name = parts.name ?? opts.name ?? 'Sheet';
    const meta = (0, packed_1.sheetMeta)({
        name,
        rows: [],
        ...parts,
    });
    const res = writer.finish(name, defaultRowPx, meta);
    const t1 = typeof performance !== 'undefined' ? performance.now() : 0;
    return { ...res, packMs: Math.round((t1 - t0) * 10) / 10, parseMs: Math.round((t1 - t0) * 10) / 10, chunks };
}
exports.packSheetFromStream = packSheetFromStream;
