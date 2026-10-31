<script setup lang="ts">
//! docx 高保真预览：产物分片 → 物理分页 → 逐页渲染（纸张/页边距/样式/块级元素），悬浮显示 CLI 路径
//!
//! 产物里的 `parts` 是**编辑分片**（分节符 / 一级标题切块），不是页：一个分片常横跨多页。
//! 所以这里把每个分片再切成物理页——分页符/段前分页是硬分页，内容超出页高是软分页；
//! 高度靠离屏度量（同一份块渲染）拿到，页数/页码因此与真实文档近似一致。
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { DocxBlock, DocxDocument, DocxSection } from '@/types'
import { ptToPx } from '@/core/units'
import { usePathHover, type PathCardInfo, type SelectInfo } from '@/core/hover'
import PathCard from '@/components/PathCard.vue'
import DocxBlockView from '@/components/DocxBlock.vue'
import { fontStack, isWesternLang, normalLineRatio, ordinals, resetLineRatioCache } from '@/renderers/docx/blocks'
import { paginateBlocks, type PaginateBlock } from '@/renderers/docx/paginate'
import { LATEX_CSS } from '@/renderers/docx/latex'
import { useZoomGesture } from '@/core/zoomGesture'
import { useOvTheme } from '@/core/theme'

const props = withDefaults(
  defineProps<{
    document: DocxDocument
    /** 媒体解析：把 word/media/xxx 解析成可加载 URL */
    resolveMedia?: (src: string) => string
    /** 页码格式：{page} 占位（默认读取 section.footer_format） */
    pageFormat?: string
    /** 初始分片下标（会被换算成该分片的第一页） */
    partIndex?: number
    /** 受控物理页下标:变化即跳页(宿主自渲染换页 UI 时用;宿主主动设页不发 page-change,
     *  组件内部翻页才发)。与 partIndex 并存时以后变化者为准。 */
    pageIndex?: number
    /** 内置换页器(侧栏底部 ▲▼+页码,默认显示):false 隐藏,翻页由宿主经
     *  pageIndex(写)+page-change(读)+pages-change(总页数)接管。 */
    showPager?: boolean
    /** 铺满容器:容器有确定高度时传 true——高度改为 100% 跟随容器、内容内部滚动;
     *  默认 false(自撑高 72~78vh 兜底)。 */
    fill?: boolean
    /** 受控缩放:相对「适配可视区基准」的用户倍率。提供即受控——隐藏内置缩放控件、
     *  缩放只跟此值走;不传(undefined)保持自主模式(内置滑杆可用)。 */
    zoom?: number
    /** 拾取模式:仅 true 时启用交互层——元素 hover 高亮/路径卡/选中事件;
     *  false(默认)为纯只读观感(无高亮、无卡片、不发 select)。 */
    pickMode?: boolean
    /** 侧边目录开合:提供即受控——显隐只跟此值走,内置按钮改为汇报 outline-change 由宿主回写;
     *  不传(undefined)保持自主模式(内置按钮直接开合,默认展开)。 */
    outline?: boolean
    /** 主题:light / dark / auto(默认 auto,跟随系统 prefers-color-scheme 实时切换)。
     *  只暗化 chrome(目录侧栏/翻页器);纸张保持白底,宿主要连纸张一起暗在外层加 ov-dark-pages 类。 */
    theme?: 'light' | 'dark' | 'auto'
  }>(),
  { partIndex: 0 },
)

const dark = useOvTheme(() => props.theme)

const emit = defineEmits<{
  (e: 'select', path: string, info?: SelectInfo): void
  (e: 'hover', path: string | null): void
  /** 翻页(物理页下标，非分片下标) */
  (e: 'page-change', index: number): void
  /** 缩放汇报(scale 变化即发):scale=绝对倍率(展示用百分比),fitScale=适配基准(宿主换算) */
  (e: 'zoom-change', v: { scale: number; fitScale: number }): void
  /** 目录开合汇报(内置按钮按下即发;受控模式宿主应回写 outline prop) */
  (e: 'outline-change', open: boolean): void
  /** 总物理页数汇报(分页重算后变化即发;宿主自渲染换页 UI 需要总数) */
  (e: 'pages-change', total: number): void
}>()

const hover = usePathHover()

const PAGE_SIZES: Record<string, [number, number]> = {
  A4: [8.27, 11.69],
  A3: [11.69, 16.54],
  A5: [5.83, 8.27],
  Letter: [8.5, 11],
  Legal: [8.5, 14],
}

/** 纸张几何（自然 px）：宽高按纸张/方向，四边距按磅换算；多栏节带栏宽表 */
interface Geometry {
  paperW: number
  paperH: number
  padTop: number
  padRight: number
  padBottom: number
  padLeft: number
  /** 多栏排版：各栏宽（自然 px）+ 栏间距 */
  cols: { widths: number[]; gap: number } | null
}

/** 文档级节属性：无分节符的文档只有末尾分片带 body sectPr，但它作用于全文 */
const docSection = computed<DocxSection | null>(
  () => props.document.parts[props.document.parts.length - 1]?.section ?? null,
)
function sectionOf(partIndex: number): DocxSection | null {
  return props.document.parts[partIndex]?.section ?? docSection.value
}

function geometryOf(partIndex: number): Geometry {
  const section = sectionOf(partIndex)
  const p = (section?.page ?? props.document.page) as
    | (DocxSection['page'] & { width?: number | null; height?: number | null })
    | undefined
  const size = p?.size ?? 'A4'
  const landscape = (p?.orientation ?? '').toLowerCase() === 'landscape'
  let pw: number
  let ph: number
  if (size === 'custom' && typeof p?.width === 'number' && typeof p?.height === 'number') {
    // custom 尺寸以「磅」记录（与 CLI 模型一致）
    pw = ptToPx(p.width) / 96
    ph = ptToPx(p.height) / 96
  } else {
    const [w, h] = PAGE_SIZES[size] ?? PAGE_SIZES.A4
    pw = w
    ph = h
  }
  if (landscape) [pw, ph] = [ph, pw]
  // 页边距单位是磅（pt），与 json2docx 模型一致
  const m = (p?.margins ?? {}) as Record<string, number | undefined>
  const pt = (v: number | undefined, dft: number) => (typeof v === 'number' ? v : dft)
  const geom: Geometry = {
    paperW: Math.round(pw * 96),
    paperH: Math.round(ph * 96),
    padTop: Math.round(ptToPx(pt(m.top, 72))),
    padRight: Math.round(ptToPx(pt(m.right, 90))),
    padBottom: Math.round(ptToPx(pt(m.bottom, 72))),
    padLeft: Math.round(ptToPx(pt(m.left, 90))),
    cols: null,
  }
  // 多栏：显式 w:col 宽度优先，缺省等宽（Word 默认栏间距 0.5in = 36pt）
  const count = typeof p?.columns === 'number' ? p.columns : 0
  if (count > 1) {
    const contentW = geom.paperW - geom.padLeft - geom.padRight
    const space = ptToPx(typeof p?.column_space === 'number' ? p.column_space : 36)
    const explicit =
      Array.isArray(p?.column_widths) && p.column_widths.length === count
        ? (p.column_widths as number[]).map((w) => ptToPx(w))
        : null
    if (explicit) {
      const sum = explicit.reduce((a, b) => a + b, 0) + space * (count - 1)
      const k = sum > 0 ? contentW / sum : 1
      geom.cols = { widths: explicit.map((w) => Math.round(w * k)), gap: Math.round(space * k) }
    } else {
      const w = Math.max(40, (contentW - space * (count - 1)) / count)
      geom.cols = { widths: Array.from({ length: count }, () => Math.round(w)), gap: Math.round(space) }
    }
  }
  return geom
}

const geometry = computed<Geometry[]>(() =>
  props.document.parts.map((_, pi) => geometryOf(pi)),
)
/** 同名块序号表（分片内）：路径与展示编号共用 */
const ordinalTable = computed<number[][]>(() =>
  props.document.parts.map((p) => ordinals(p.blocks ?? [])),
)

/** 物理页：若干「带」自顶向下堆叠。带=一段连续正文流，或一块分栏区（连续分节不再强制分页，
 *  分栏区整带插进当前页；放不下才另起页）。 */
interface RenderSegment {
  partIndex: number
  from: number
  to: number
}
interface RenderColumn {
  partIndex: number
  width: number
  /** 与前一栏的间距（首栏 0） */
  gap: number
  segments: RenderSegment[]
}
interface RenderBandFlow {
  kind: 'flow'
  segments: RenderSegment[]
  height: number
  /** 本段首块从第几行开始渲染（跨页续段；前面的行在上一页） */
  leadSkip?: number
  /** 本段末块渲染到第几行（不含；跨页续段的后半在下页） */
  trailKeep?: number
  /** 本段首块是表格时从第几行开始渲染（续排片会重复表头） */
  leadRows?: number
  /** 本段末块是表格时渲染到第几行（不含） */
  trailRows?: number
  /** 表格行内切：首块的第 leadRows 行已画过几行文本（续排跳过） */
  leadLines?: number
  /** 表格行内切：末块的第 trailRows-1 行只画前几行文本 */
  trailLines?: number
}
interface RenderBandCols {
  kind: 'cols'
  columns: RenderColumn[]
  height: number
}
type RenderBand = RenderBandFlow | RenderBandCols
interface RenderPage {
  bands: RenderBand[]
  /** 已用内容高（自然 px，各带高度之和） */
  used: number
}

const pages = ref<RenderPage[]>([])
const active = ref(0)

/* ------------------------------ 离屏度量 + 分页 ------------------------------ */

const measureRef = ref<HTMLElement | null>(null)

/** 量每个分片：逐块自身范围（相对内容区顶端）与内容总高；不可布局（隐藏/未挂载）时返回 null
 *  用 offsetTop/offsetHeight（布局量）而非 getBoundingClientRect：不受祖先 transform 缩放影响 */
interface MeasuredPart {
  tops: (number | null)[]
  bottoms: (number | null)[]
  contentH: number
  /** 可跨页段落：逐行顶端（相对块顶端，首项 0）；不可切为 null */
  lineTops: (number[] | null)[]
  /** 可跨页表格：逐行盒 [top, bottom]（相对块顶端，含上下内边距）；不可切为 null */
  rowBoxes: (Array<[number, number]> | null)[]
  /** 表格逐行「内容盒」（不含内边距，与 rowLines 配套；行内切用） */
  rowContent: (Array<[number, number]> | null)[]
  /** 表格逐行文本行数（最高单元格） */
  rowLines: (number[] | null)[]
  /** 表格行几何（渲染行内切用：每行行高与各单元格行数） */
  rowGeom: (TableRowGeom[] | null)[]
}

/** 表格单行几何：行内文本行数、行高，以及各单元格的行数（渲染时逐格裁剪） */
interface TableRowGeom {
  lines: number
  lineH: number
  cells: number[]
}

/** 元素内文本的行盒顶端（升序、原始坐标）。
 *  注意：内容含 <span>/多块时 `getClientRects()` 会额外给出「整块矩形」（包住该块所有行盒），
 *  不滤掉就会把 2 行算成 3 行——只保留不被**更高**的矩形包含的行盒。
 *  判据必须是「更高且包含」：同一行的多个 run（比如「·成本与延迟：」+「…（见 4.3 节）。」）
 *  顶端/底端完全相同，互相都满足朴素包含判据，会一起被滤掉、整段行数算成 0；高度相等即同行。 */
function lineBoxTops(el: HTMLElement): number[] {
  let rects: DOMRect[]
  try {
    const range = document.createRange()
    range.selectNodeContents(el)
    rects = Array.from(range.getClientRects()).filter((r) => r.height > 0.5)
  } catch {
    return []
  }
  const kept = rects.filter(
    (r) =>
      !rects.some(
        (o) =>
          o !== r &&
          o.height > r.height + 0.5 &&
          o.top <= r.top + 0.5 &&
          o.bottom >= r.bottom - 0.5,
      ),
  )
  const tops: number[] = []
  for (const r of kept) {
    const top = Math.round(r.top * 100) / 100
    if (!tops.length || Math.abs(top - tops[tops.length - 1]) > 1) tops.push(top)
  }
  return tops
}

/** 段落逐行顶端（相对首行、首项 0；跨页切行用）；行数太少不值得切 */
function lineTopsOf(el: HTMLElement): number[] | null {
  if (el.childElementCount === 1 && el.firstElementChild?.tagName === 'BR') return null
  const tops = lineBoxTops(el)
  if (tops.length < 4) return null
  // 首行归零：首个 rect 顶端含首行半行距，行距（相邻差）不受影响
  return tops.map((t) => Math.round((t - tops[0]) * 100) / 100)
}

/** 元素内文本的行数（表格单元格用） */
function linesInEl(el: HTMLElement): number {
  return lineBoxTops(el).length
}

/** 表格行几何：逐行盒 + 逐行内容盒（不含内边距）+ 逐行行数 + 各格行数。
 *  行内切分要用「行的文本行网格」——取行内行数最多的那个单元格的内容盒为准。 */
function tableGeomOf(el: HTMLElement): {
  rowBoxes: Array<[number, number]>
  rowContent: Array<[number, number]>
  rowLines: number[]
  rows: TableRowGeom[]
} | null {
  const trs = Array.from(el.querySelectorAll<HTMLElement>('tr')).filter(
    (tr) => tr.closest('table') === el,
  )
  if (trs.length < 2) return null
  const base = el.getBoundingClientRect().top
  const round = (v: number) => Math.round(v * 100) / 100
  const rowBoxes: Array<[number, number]> = []
  const rowContent: Array<[number, number]> = []
  const rowLines: number[] = []
  const rows: TableRowGeom[] = []
  for (const tr of trs) {
    const r = tr.getBoundingClientRect()
    const tds = Array.from(tr.children).filter((c) => c.tagName === 'TD' || c.tagName === 'TH') as HTMLElement[]
    if (!tds.length) return null
    const cells = tds.map((td) => Math.max(1, linesInEl(td)))
    const lines = Math.max(...cells)
    // 行网格 = 行数最多的那个单元格的内容盒（上下内边距扣掉）
    const grid = tds.find((_, k) => cells[k] === lines) ?? tds[0]
    const cs = getComputedStyle(grid)
    const gb = grid.getBoundingClientRect()
    const gridTop = gb.top + (parseFloat(cs.paddingTop) || 0)
    const gridBottom = gb.bottom - (parseFloat(cs.paddingBottom) || 0)
    rowBoxes.push([round(r.top - base), round(r.bottom - base)])
    rowContent.push([round(gridTop - base), round(gridBottom - base)])
    rowLines.push(lines)
    rows.push({ lines, lineH: round((gridBottom - gridTop) / lines), cells })
  }
  return { rowBoxes, rowContent, rowLines, rows }
}

function measureParts(): Array<MeasuredPart[] | null> {
  const host = measureRef.value
  return props.document.parts.map((part, pi) => {
    const widthCount = Math.max(1, geometry.value[pi]?.cols?.widths.length ?? 1)
    const list: MeasuredPart[] = []
    for (let ci = 0; ci < widthCount; ci += 1) {
      const el = host?.querySelector<HTMLElement>(`.ov-measure-page[data-mc="${pi}:${ci}"]`)
      if (!el || !el.offsetWidth) return null
      const nodes = Array.from(el.querySelectorAll<HTMLElement>('[data-mb]')).filter(
        (node) => !node.classList.contains('ov-pagebreak-mark'),
      )
      if (!nodes.length) {
        list.push({
          tops: [],
          bottoms: [],
          contentH: 0,
          lineTops: [],
          rowBoxes: [],
          rowContent: [],
          rowLines: [],
          rowGeom: [],
        })
        continue
      }
      // 同一栏内统一基准：只需相对量（差值与页高比较），基准取最小值避免首块外边距干扰
      const base = Math.min(...nodes.map((node) => node.offsetTop))
      const tops: (number | null)[] = []
      const bottoms: (number | null)[] = []
      const lineTops: (number[] | null)[] = []
      const rowBoxes: (Array<[number, number]> | null)[] = []
      const rowContent: (Array<[number, number]> | null)[] = []
      const rowLines: (number[] | null)[] = []
      const rowGeom: (TableRowGeom[] | null)[] = []
      let contentH = 0
      nodes.forEach((node) => {
        const bi = Number(node.dataset.mb)
        tops[bi] = node.offsetTop - base
        bottoms[bi] = node.offsetTop - base + node.offsetHeight
        contentH = Math.max(contentH, node.offsetTop - base + node.offsetHeight)
        // 只有段落可整段跨页（图片/形状/标题整块挪走）
        const type = (part.blocks ?? [])[bi]?.type
        if (type === 'paragraph' || type === 'quote') {
          lineTops[bi] = node.classList.contains('ov-para') ? lineTopsOf(node) : null
        } else if (type === 'table') {
          const geom = tableGeomOf(node)
          rowBoxes[bi] = geom?.rowBoxes ?? null
          rowContent[bi] = geom?.rowContent ?? null
          rowLines[bi] = geom?.rowLines ?? null
          rowGeom[bi] = geom?.rows ?? null
        }
      })
      list.push({ tops, bottoms, contentH, lineTops, rowBoxes, rowContent, rowLines, rowGeom })
    }
    return list
  })
}

/** 多栏分片照 Word 串栏排：块按「当前栏宽」量高，装不下就进下一栏，栏满换页。
 *  返回值按「带」给出：一个带 = 一页里的那块分栏区（页高由栏内最高者决定）。 */
function layoutColumnBands(
  partIndex: number,
  blocks: Record<string, unknown>[],
  measured: MeasuredPart[],
): { columns: RenderColumn[]; height: number }[] {
  const geom = geometry.value[partIndex] ?? geometryOf(0)
  const cols = geom.cols
  if (!cols) return []
  const availH = Math.max(80, geom.paperH - geom.padTop - geom.padBottom)
  const bands: { columns: RenderColumn[]; height: number }[] = []
  let bi = 0
  while (bi < blocks.length) {
    const columns: RenderColumn[] = []
    let usedMax = 0
    let pageEnded = false
    for (let ci = 0; ci < cols.widths.length && bi < blocks.length; ci += 1) {
      const m = measured[ci] ?? measured[0]
      const from = bi
      const firstTop = (m?.tops[bi] ?? 0) as number
      let lastBottom = firstTop
      while (bi < blocks.length) {
        const b = blocks[bi]
        if (b.type === 'page_break') {
          // 分栏节里的分页符 = 整页结束（分页符本身不渲染）
          bi += 1
          pageEnded = true
          break
        }
        if (b.type === 'column_break') {
          // 分栏符：本栏到此为止，下一块进下一栏
          bi += 1
          break
        }
        if (b.page_break_before && bi > from) break // 段前分页：换栏
        const top = m?.tops[bi]
        const bottom = m?.bottoms[bi]
        if (top === null || top === undefined || bottom === null || bottom === undefined) {
          bi += 1
          continue
        }
        if (bi > from && bottom - firstTop > availH) break // 本栏装不下 → 下一栏
        lastBottom = bottom
        bi += 1
      }
      const used = Math.max(0, lastBottom - firstTop)
      usedMax = Math.max(usedMax, used)
      if (bi > from) {
        columns.push({
          partIndex,
          width: cols.widths[ci],
          gap: ci > 0 ? cols.gap : 0,
          segments: [{ partIndex, from, to: bi }],
        })
      }
      if (pageEnded) break
    }
    if (!columns.length) break // 防御：整页排不出内容
    bands.push({ columns, height: usedMax })
  }
  return bands
}

/** 纸张/页边距是否同一套版式（不同则分片之间必须分页） */
function sameGeometry(a: Geometry | undefined, b: Geometry | undefined): boolean {
  if (!a || !b) return false
  return (
    a.paperW === b.paperW &&
    a.paperH === b.paperH &&
    a.padTop === b.padTop &&
    a.padRight === b.padRight &&
    a.padBottom === b.padBottom &&
    a.padLeft === b.padLeft
  )
}

/** 流区间 → 分片段（一页可跨分片） */
function toSegments(
  map: { partIndex: number; bi: number }[],
  from: number,
  to: number,
): RenderSegment[] {
  const out: RenderSegment[] = []
  for (let i = from; i < to; i += 1) {
    const entry = map[i]
    if (!entry) continue
    const last = out[out.length - 1]
    if (last && last.partIndex === entry.partIndex && entry.bi === last.to) last.to = entry.bi + 1
    else out.push({ partIndex: entry.partIndex, from: entry.bi, to: entry.bi + 1 })
  }
  return out
}

/** 分片前是否有硬分页：上一分片自带的节不是 continuous（缺省 nextPage），或纸张/页边距变版式 */
function hardBreakBefore(partIndex: number): boolean {
  if (partIndex <= 0) return false
  const prevSection = props.document.parts[partIndex - 1]?.section ?? null
  if (prevSection) {
    const type = (prevSection.section_type ?? 'nextPage').toLowerCase()
    if (type !== 'continuous') return true
  }
  // 无自带节属性 = 无分节符（一级标题切块）→ 续排；版式变了才断页
  return !sameGeometry(geometry.value[partIndex - 1], geometry.value[partIndex])
}

/** 重算物理分页：硬分页（分页符/段前分页/分节符）+ 软分页（超出页高的块另起一页）；
 *  连续分节之间续排，分栏区作为一个「带」插进当前页（放不下才另起页） */
let lastPageTotal = -1
async function rebuild(): Promise<void> {
  await nextTick()
  const measured = measureParts()
  // 逐行/表格行几何留一份给渲染用（跨页段落裁切、跨页表格逐格按行裁剪）
  measuredLineTops.value = measured.map((list) => list?.[0]?.lineTops ?? null)
  measuredRowGeom.value = measured.map((list) => list?.[0]?.rowGeom ?? null)
  const out: RenderPage[] = []
  /** 当前页装配：带自顶向下堆叠，used = 已用高 */
  let bands: RenderBand[] = []
  let used = 0
  const closePage = () => {
    if (bands.length) out.push({ bands, used })
    bands = []
    used = 0
  }

  // 正文流：连续分节之间续排（坐标累加），跨分片合成一段「流」
  let flow: PaginateBlock[] = []
  let map: { partIndex: number; bi: number }[] = []
  let offset = 0
  let flowPageH = Number.POSITIVE_INFINITY
  let flowHardStart = false
  const flushFlow = () => {
    if (!flow.length) return
    if (flowHardStart) closePage() // 新节另起一页（节类型非 continuous）
    // 首页带上当前页已占用的高：接在分栏带等已有内容之后续排
    paginateBlocks(flow, flowPageH, offset, bands.length ? used : 0).forEach((r, i) => {
      const band: RenderBandFlow = {
        kind: 'flow',
        segments: toSegments(map, r.from, r.to),
        height: Math.max(0, r.bottom - r.top),
        ...(r.leadSkip ? { leadSkip: r.leadSkip } : {}),
        ...(r.trailKeep ? { trailKeep: r.trailKeep } : {}),
        ...(r.leadRows ? { leadRows: r.leadRows } : {}),
        ...(r.trailRows ? { trailRows: r.trailRows } : {}),
        ...(r.leadLines ? { leadLines: r.leadLines } : {}),
        ...(r.trailLines ? { trailLines: r.trailLines } : {}),
      }
      if (i === 0 && (!bands.length || used + band.height <= flowPageH + 0.5)) {
        bands.push(band)
        used += band.height
      } else {
        // 后续页：各段自成一页（首页放不下也走这里）
        closePage()
        bands.push(band)
        used = band.height
      }
    })
    flow = []
    map = []
    offset = 0
    flowPageH = Number.POSITIVE_INFINITY
    flowHardStart = false
  }

  props.document.parts.forEach((part, pi) => {
    const geom = geometry.value[pi] ?? geometryOf(0)
    const blocks = (part.blocks ?? []) as Record<string, unknown>[]
    const colsDef = geom.cols
    if (colsDef) {
      // 分栏区：整带插进当前页；一页装不下（或分栏节的换页符）则续到新页
      flushFlow()
      const pageH = Math.max(80, geom.paperH - geom.padTop - geom.padBottom)
      layoutColumnBands(pi, blocks, measured[pi] ?? []).forEach((cb) => {
        if (bands.length && used + cb.height > pageH + 0.5) closePage()
        bands.push({ kind: 'cols', columns: cb.columns, height: cb.height })
        used += cb.height
      })
      return
    }
    const m = measured[pi]?.[0] ?? null
    const pageH = m
      ? Math.max(80, geom.paperH - geom.padTop - geom.padBottom)
      : Number.POSITIVE_INFINITY
    const hard = hardBreakBefore(pi)
    if (hard && flow.length) flushFlow() // 硬分节的起点：先把前面那段流收口成页
    if (!flow.length) flowHardStart = hard // 本段流的起点是否硬分页
    flowPageH = Math.min(flowPageH, pageH)
    blocks.forEach((b, bi) => {
      const isBreak = b.type === 'page_break'
      const invisible = isBreak || b.type === 'column_break' // 分栏符在单栏节里无意义
      const top = invisible ? null : (m?.tops[bi] ?? null)
      const bottom = invisible ? null : (m?.bottoms[bi] ?? null)
      flow.push({
        top: top === null ? null : offset + top,
        bottom: bottom === null ? null : offset + bottom,
        breakBefore: !!b.page_break_before,
        breakHere: isBreak,
        pageH,
        // 与下段同页：标题默认（Word 的内建 Heading 样式即 keepNext）+ 段落显式设置
        keepWithNext: b.keep_next === true || (b.keep_next === undefined && b.type === 'heading'),
        // 段落可在行边界跨页（孤行控制）；表格按行/行内跨页（续排片重复表头）；图片等整块挪走
        lineTops: m?.lineTops[bi] ?? null,
        rowBoxes: m?.rowBoxes[bi] ?? null,
        rowContent: m?.rowContent[bi] ?? null,
        rowLines: m?.rowLines[bi] ?? null,
        headerRows: b.type === 'table' && b.header_row === true ? 1 : 0,
      })
      map.push({ partIndex: pi, bi })
    })
    offset += m?.contentH ?? 0
  })
  flushFlow()
  closePage()

  // 保持阅读位置：原页起始块仍在则跟随，否则按下标夹取
  const anchor = leadSegment(pages.value[active.value])
  const hit = anchor
    ? out.findIndex((p) => {
        const lead = leadSegment(p)
        return lead?.partIndex === anchor.partIndex && lead?.from === anchor.from
      })
    : -1
  pages.value = out
  active.value = hit >= 0 ? hit : Math.min(active.value, Math.max(0, out.length - 1))
  // 总页数汇报（宿主自渲染换页 UI 用）：只在变化时发，避免字体/尺寸抖动连发
  if (out.length !== lastPageTotal) {
    lastPageTotal = out.length
    emit('pages-change', out.length)
  }
}

/** 页的首个块位置（页首带是分栏带时取第一栏） */
function leadSegment(page: RenderPage | undefined): RenderSegment | undefined {
  const band = page?.bands[0]
  if (!band) return undefined
  return band.kind === 'cols' ? band.columns[0]?.segments[0] : band.segments[0]
}

let rebuildQueued = false
function scheduleRebuild(): void {
  if (rebuildQueued) return
  rebuildQueued = true
  requestAnimationFrame(() => {
    rebuildQueued = false
    void rebuild()
  })
}

/* ---------------------------------- 缩放 ---------------------------------- */

const wrapRef = ref<HTMLElement | null>(null)
const selfZoom = ref(1)
const zoom = computed(() => props.zoom ?? selfZoom.value)
const wrapW = ref(0)
const wrapH = ref(0)
let roWrap: ResizeObserver | null = null
let roMeasure: ResizeObserver | null = null
let roPage: ResizeObserver | null = null

const curPage = computed(() => pages.value[active.value])
/** 当前页首块所属分片（页可跨分片：页眉页脚/纸张以首块的分片为准） */
const curPartIndex = computed(() => leadSegment(curPage.value)?.partIndex ?? 0)
const curGeom = computed<Geometry>(() => geometry.value[curPartIndex.value] ?? geometryOf(0))

/** 页自然尺寸：纸张高与内容高的较大者（超高块不裁切，页面长高） */
const naturalW = computed(() => curGeom.value.paperW)
const naturalH = computed(() => {
  const g = curGeom.value
  const p = curPage.value
  if (!p) return g.paperH
  return Math.max(g.paperH, g.padTop + p.used + g.padBottom)
})

/** 渲染页实测高度（页是 min-height，段首外边距等会让它比估算略高）：占位盒跟随，避免裁切 */
const pageRenderedH = ref(0)
const boxH = computed(() => Math.max(naturalH.value, pageRenderedH.value))

/** 适配倍率：整页落在可视区内 */
const fitScale = computed(() => {
  const availW = Math.max(80, wrapW.value - 32)
  const availH = Math.max(80, wrapH.value - 32)
  if (!wrapW.value || !wrapH.value) return 1
  return Math.min(1, availW / naturalW.value, availH / boxH.value)
})
const scale = computed(() => Math.max(0.05, fitScale.value * zoom.value))
watch(scale, (s) => emit('zoom-change', { scale: s, fitScale: fitScale.value }), { immediate: true })

/** 滚轮/捏合缩放（Ctrl/⌘+滚轮、触控板捏合、触屏双指；普通滚轮保持滚动） */
useZoomGesture({
  el: wrapRef,
  min: 0.5,
  max: 3,
  getZoom: () => zoom.value,
  setZoom: (z) => {
    if (props.zoom === undefined) selfZoom.value = z
    else emit('zoom-change', { scale: Math.max(0.05, fitScale.value * z), fitScale: fitScale.value })
  },
})

const pageStyle = computed(() => {
  const g = curGeom.value
  return {
    width: `${naturalW.value}px`,
    minHeight: `${naturalH.value}px`,
    boxSizing: 'border-box' as const,
    padding: `${g.padTop}px ${g.padRight}px ${g.padBottom}px ${g.padLeft}px`,
    transform: `scale(${scale.value})`,
    transformOrigin: 'top left',
    margin: '0',
    maxHeight: 'none',
    overflow: 'visible',
  }
})
const pageBoxStyle = computed(() => ({
  width: `${Math.round(naturalW.value * scale.value)}px`,
  height: `${Math.round(boxH.value * scale.value)}px`,
  position: 'relative' as const,
  margin: 'auto' as const,
  flex: 'none' as const,
}))

/** 度量页几何：与渲染页同宽同边距（无缩放、无高度约束——量的是自然高度）。
 *  多栏分片按栏宽量（ci 为栏下标；单栏分片 ci=0 用整幅内容宽）。 */
function measureStyle(partIndex: number, ci: number): Record<string, string> {
  const g = geometry.value[partIndex] ?? geometryOf(0)
  const cols = g.cols
  if (cols) {
    return {
      width: `${cols.widths[ci] ?? cols.widths[0]}px`,
      minHeight: '0px',
    }
  }
  return {
    width: `${g.paperW}px`,
    minHeight: `${g.paperH}px`,
    padding: `${g.padTop}px ${g.padRight}px ${g.padBottom}px ${g.padLeft}px`,
  }
}

/** 该分片需要几份度量页（多栏 = 栏数） */
function measureWidths(partIndex: number): number[] {
  const cols = geometry.value[partIndex]?.cols
  const n = Math.max(1, cols?.widths.length ?? 1)
  return Array.from({ length: n }, (_, i) => i)
}

/** 页面基准排版：文档默认样式（字号/行距/字体）——块级样式在其上覆盖 */
const baseStyle = computed<Record<string, string>>(() => {
  const d = props.document.defaults
  if (!d) return {}
  const out: Record<string, string> = {}
  const family = d.font_family ?? d.east_asia_font
  if (family) out['font-family'] = fontStack(family)
  const size = typeof d.font_size === 'number' && d.font_size > 0 ? d.font_size : undefined
  if (size) out['font-size'] = `${ptToPx(size)}px`
  const ls = typeof d.line_spacing === 'number' && d.line_spacing > 0 ? d.line_spacing : 1
  out['line-height'] = String(ls * normalLineRatio(family, isWesternLang(d.lang_ea)))
  return out
})

/* ------------------------------- 元素 / 路径 ------------------------------- */

/** 块路径：/part[n]/容器[序号]… */
function pathOfFor(partIndex: number, bi: number): (tail?: string) => string {
  const type = String((props.document.parts[partIndex]?.blocks?.[bi] as DocxBlock | undefined)?.type ?? 'block')
  const ord = ordinalTable.value[partIndex]?.[bi] ?? 1
  const head = `${type}[${ord}]`
  return (tail?: string) => `/part[${partIndex + 1}]/${tail ? `${head}/${tail}` : head}`
}

function blockInfo(b: DocxBlock, path: string): PathCardInfo {
  const anyB = b as unknown as Record<string, unknown>
  const text =
    (anyB.text as string) ??
    (Array.isArray(anyB.runs)
      ? (anyB.runs as { text?: string }[]).map((r) => r.text ?? '').join('')
      : undefined) ??
    (Array.isArray(anyB.rows) ? `表格 ${(anyB.rows as unknown[]).length} 行` : undefined) ??
    (Array.isArray(anyB.items) ? `列表 ${(anyB.items as unknown[]).length} 项` : undefined)
  return {
    path,
    type: b.type,
    name: (anyB.name as string) ?? undefined,
    text: typeof text === 'string' && text ? text.slice(0, 160) : undefined,
  }
}

interface PageItem {
  partIndex: number
  block: DocxBlock
  bi: number
  ord: number
  /** 跨页续段：首块跳过的行数 / 末块显示到第几行（渲染时裁切） */
  leadSkip?: number
  trailKeep?: number
  /** 该块逐行顶端（相对块顶端，px）——裁切高度由它算 */
  lineTops?: number[] | null
  /** 跨页续表：首块从第几行开始（续排片重复表头）/ 末块显示到第几行（不含） */
  rowFrom?: number
  rowTo?: number
  /** 表格行内切：首块跳过几行文本（续排）/ 末块只画前几行文本 */
  skipLines?: number
  keepLines?: number
  /** 表格逐行几何（行内切逐格裁剪用） */
  rowGeom?: TableRowGeom[] | null
}

/** 片段 → 渲染项（含块在所属分片中的绝对下标与序号，路径与分片内序号保持一致）；
 *  首/末项带上跨页续段信息（leadSkip/trailKeep、表格的 rowFrom/rowTo/行内行数） */
function itemsOf(
  segments: RenderSegment[],
  marks?: {
    leadSkip?: number
    trailKeep?: number
    leadRows?: number
    trailRows?: number
    leadLines?: number
    trailLines?: number
  },
): PageItem[] {
  const out: PageItem[] = []
  segments.forEach((seg) => {
    const blocks = props.document.parts[seg.partIndex]?.blocks ?? []
    const ords = ordinalTable.value[seg.partIndex] ?? []
    for (let bi = seg.from; bi < seg.to; bi += 1) {
      const block = blocks[bi]
      if (!block) continue
      out.push({
        partIndex: seg.partIndex,
        block,
        bi,
        ord: ords[bi] ?? 1,
        lineTops: measuredLineTops.value[seg.partIndex]?.[bi] ?? null,
        rowGeom: measuredRowGeom.value[seg.partIndex]?.[bi] ?? null,
      })
    }
  })
  if (out.length) {
    if (marks?.leadSkip) out[0].leadSkip = marks.leadSkip
    if (marks?.trailKeep) out[out.length - 1].trailKeep = marks.trailKeep
    if (marks?.leadRows) out[0].rowFrom = marks.leadRows
    if (marks?.trailRows) out[out.length - 1].rowTo = marks.trailRows
    if (marks?.leadLines) out[0].skipLines = marks.leadLines
    if (marks?.trailLines) out[out.length - 1].keepLines = marks.trailLines
  }
  return out
}

/** 最近一次度量的逐行几何（渲染裁切复用；度量页与渲染页同宽同字体，行距一致） */
const measuredLineTops = ref<Array<Array<number[] | null> | null>>([])
/** 最近一次度量的表格行几何（行内切逐格裁剪用） */
const measuredRowGeom = ref<Array<Array<TableRowGeom[] | null> | null>>([])

/** 正文流版心宽：页宽 − 左右页边距（分栏带的每栏另有栏宽） */
function flowWidthOf(pi: number): number {
  const g = geometry.value[pi]
  return g ? g.paperW - g.padLeft - g.padRight : 0
}

/** 度量页的版心宽（段落预断行用）：多栏量的是栏宽，单栏量的是内容宽。
 *  必须与渲染页传的值一致——预断行依赖宽度，量出来的行数/行高要能对上排出来的 */
function measureBreakWidth(partIndex: number, ci: number): number {
  const g = geometry.value[partIndex]
  if (!g) return 0
  if (g.cols) return g.cols.widths[ci] ?? g.cols.widths[0] ?? 0
  return g.paperW - g.padLeft - g.padRight
}

/** 当前页渲染视图：带列表（正文带=块序列，分栏带=各栏块序列） */
interface BandView {
  kind: 'flow' | 'cols'
  items: PageItem[]
  /** 正文流版心宽（px）：段落预断行用 */
  width: number
  columns: { width: number; gap: number; items: PageItem[] }[]
}
const pageBands = computed<BandView[]>(() => {
  const page = curPage.value
  if (!page) return []
  return page.bands.map((band) =>
    band.kind === 'cols'
      ? {
          kind: 'cols' as const,
          items: [],
          width: 0,
          columns: band.columns.map((c) => ({
            width: c.width,
            gap: c.gap,
            items: itemsOf(c.segments),
          })),
        }
      : {
          kind: 'flow' as const,
          items: itemsOf(band.segments, {
            leadSkip: band.leadSkip,
            trailKeep: band.trailKeep,
            leadRows: band.leadRows,
            trailRows: band.trailRows,
            leadLines: band.leadLines,
            trailLines: band.trailLines,
          }),
          width: flowWidthOf(band.segments[0]?.partIndex ?? 0),
          columns: [],
        },
  )
})

/** 跨页段落裁切：外层盒高 = 显示到第 trailKeep 行的顶端；内层负外边距 = 跳过前 leadSkip 行 */
function clipBoxStyle(item: PageItem): Record<string, string> {
  const tops = item.lineTops
  if (!tops || !item.trailKeep) return {}
  const top = tops[item.trailKeep]
  return top === undefined ? {} : { height: `${top}px` }
}
function clipInnerStyle(item: PageItem): Record<string, string> {
  const tops = item.lineTops
  if (!tops || !item.leadSkip) return {}
  const top = tops[item.leadSkip]
  return top === undefined ? {} : { marginTop: `-${top}px` }
}
/** 是否需要裁切包装（跨页续段） */
function needsClip(item: PageItem): boolean {
  return !!item.lineTops && !!(item.leadSkip || item.trailKeep)
}

function onPick(
  action: 'enter' | 'move' | 'leave' | 'tap',
  block: DocxBlock,
  path: string,
  ev: MouseEvent,
): void {
  if (action === 'enter') {
    hover.enter(blockInfo(block, path), ev)
    emit('hover', path)
  } else if (action === 'move') {
    hover.move(ev)
  } else if (action === 'leave') {
    hover.leave()
    emit('hover', null)
  } else {
    emit('select', path, blockInfo(block, path))
  }
}

/* --------------------------------- 导航 --------------------------------- */

const pageRef = ref<HTMLElement | null>(null)

/** 目录开合：受控模式跟随 props.outline（按钮只汇报），自主模式用内部值（按钮直接开合） */
const selfOutline = ref(true)
const outlineOpen = computed(() => props.outline ?? selfOutline.value)
function toggleOutline() {
  const next = !outlineOpen.value
  if (props.outline === undefined) selfOutline.value = next
  emit('outline-change', next)
}

/** 全文目录：跨分片的标题（level + text），带分片分组标签 */
const outline = computed(() => {
  const out: {
    partIndex: number
    bi: number
    level: number
    text: string
    group: string
    showGroup: boolean
  }[] = []
  let lastPart = -1
  props.document.parts.forEach((p, pi) => {
    ;(p.blocks ?? []).forEach((b, bi) => {
      if (b.type !== 'heading') return
      const anyB = b as unknown as Record<string, unknown>
      const group = p.name || `分片 ${pi + 1}`
      out.push({
        partIndex: pi,
        bi,
        level: Math.min(6, Math.max(1, Number(anyB.level ?? 1))),
        text: String(anyB.text ?? '').trim() || '(无标题文字)',
        group,
        showGroup: pi !== lastPart,
      })
      lastPart = pi
    })
  })
  return out
})

/** 页里的全部块区间（分栏带展开各栏） */
function pageSegments(page: RenderPage | undefined): RenderSegment[] {
  if (!page) return []
  return page.bands.flatMap((b) =>
    b.kind === 'cols' ? b.columns.flatMap((c) => c.segments) : b.segments,
  )
}

/** 目录点击：定位到该标题所在的物理页，再滚动到块 */
async function scrollToBlock(entry: { partIndex: number; bi: number }) {
  const inPage = (p: RenderPage) =>
    pageSegments(p).some(
      (s) => s.partIndex === entry.partIndex && entry.bi >= s.from && entry.bi < s.to,
    )
  const hit = pages.value.findIndex(inPage)
  const target =
    hit >= 0
      ? hit
      : pages.value.findIndex((p) =>
          pageSegments(p).some((s) => s.partIndex === entry.partIndex),
        )
  if (target >= 0 && target !== active.value) {
    active.value = target
    emit('page-change', target)
    await nextTick()
  }
  const root = pageRef.value
  if (!root) return
  const el = root.querySelector(`[data-bi="${entry.bi}"]`)
  if (el instanceof HTMLElement) el.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

/** 翻页（物理页，跨分片连续） */
function go(delta: number) {
  const next = Math.min(Math.max(active.value + delta, 0), pages.value.length - 1)
  if (next === active.value) return
  active.value = next
  emit('page-change', next)
}

/** 分片下标 → 该分片第一页 */
function jumpToPart(partIndex: number) {
  const hit = pages.value.findIndex((p) =>
    pageSegments(p).some((s) => s.partIndex === partIndex),
  )
  if (hit >= 0 && hit !== active.value) {
    active.value = hit
    emit('page-change', hit)
  }
}

function onPageKey(ev: KeyboardEvent) {
  if (ev.key === 'ArrowDown' || ev.key === 'PageDown') {
    go(1)
  } else if (ev.key === 'ArrowUp' || ev.key === 'PageUp') {
    go(-1)
  }
}

/* ------------------------------- 页眉 / 页脚 ------------------------------- */

const headerText = computed(() => sectionOf(curPartIndex.value)?.header ?? '')
const footerText = computed(() => {
  const s = sectionOf(curPartIndex.value)
  const fmt = props.pageFormat ?? s?.footer_format
  // 页码按物理页计（与真实文档一致），而非分片序号
  if (fmt) return fmt.replace('{page}', String(active.value + 1))
  return s?.footer_page_number ? String(active.value + 1) : ''
})

/* --------------------------------- 生命周期 --------------------------------- */

const LATEX_CSS_INJECTED = ref(false)
if (typeof document !== 'undefined' && !document.getElementById('ov-latex-css')) {
  const style = document.createElement('style')
  style.id = 'ov-latex-css'
  style.textContent = LATEX_CSS
  document.head.appendChild(style)
  LATEX_CSS_INJECTED.value = true
}

/** 度量页尺寸一变（图片解码、字体就位、从隐藏转可见）就重算分页 */
function observeMeasure(): void {
  roMeasure?.disconnect()
  if (!measureRef.value) return
  roMeasure = new ResizeObserver(scheduleRebuild)
  measureRef.value
    .querySelectorAll<HTMLElement>('.ov-measure-page')
    .forEach((el) => roMeasure?.observe(el))
}

/** 渲染页实测高度 → 占位盒跟随（页是 min-height，实际排版可能比估算高几像素） */
function observePage(): void {
  roPage?.disconnect()
  if (!pageRef.value) return
  roPage = new ResizeObserver(() => {
    pageRenderedH.value = pageRef.value?.offsetHeight ?? 0
  })
  roPage.observe(pageRef.value)
  pageRenderedH.value = pageRef.value.offsetHeight
}

onMounted(async () => {
  if (wrapRef.value) {
    const el = wrapRef.value
    const update = () => {
      wrapW.value = el.clientWidth
      wrapH.value = el.clientHeight
    }
    update()
    roWrap = new ResizeObserver(update)
    roWrap.observe(el)
  }
  await nextTick()
  observeMeasure()
  observePage()
  await rebuild()
  jumpToPart(props.partIndex)
  // 字体晚到会改变自然行高与块高度：清行高缓存后就位再校一次
  void document.fonts?.ready?.then(() => {
    resetLineRatioCache()
    scheduleRebuild()
  })
})

onBeforeUnmount(() => {
  roWrap?.disconnect()
  roMeasure?.disconnect()
  roPage?.disconnect()
  roWrap = null
  roMeasure = null
  roPage = null
})

watch(
  () => props.document,
  async () => {
    await nextTick()
    observeMeasure()
    observePage()
    await rebuild()
  },
)
watch(() => props.partIndex, (v) => jumpToPart(v))
// 受控物理页：宿主写 pageIndex 即跳页（设页本身不发 page-change，事件只汇报内部翻页）
watch(
  () => props.pageIndex,
  (v) => {
    if (typeof v !== 'number') return
    const next = Math.min(Math.max(v, 0), Math.max(0, pages.value.length - 1))
    if (next !== active.value) active.value = next
  },
)
/** 内置换页器显隐（默认显示；宿主接管翻页时关掉） */
const pagerVisible = computed(() => props.showPager !== false)
// 页面基准排版（字号/行距）变化后块高度变，需重算分页
watch(baseStyle, scheduleRebuild)
watch(active, () => {
  // 换页后实测高度先清零，等新页的 RO 回调补上（避免沿用上一页的高值撑大占位盒）
  pageRenderedH.value = 0
})

defineExpose({ hover })
</script>

<template>
  <div class="ov-docx" :class="{ 'ov-fill': fill, 'ov-dark': dark }">
    <!-- 左侧：目录（按分片分组）+ 上下翻页；收起后留一条细轨保留展开入口 -->
    <aside v-if="outlineOpen" class="ov-docx-side">
      <div class="ov-side-head">
        <span class="ov-side-title">目录</span>
        <button class="ov-side-toggle" title="收起目录" @click="toggleOutline">«</button>
      </div>
      <nav class="ov-outline">
        <template v-for="h in outline" :key="`${h.partIndex}-${h.bi}`">
          <div v-if="h.showGroup" class="ov-outline-part">{{ h.group }}</div>
          <button
            class="ov-outline-item"
            :class="{ current: h.partIndex === curPartIndex }"
            :style="{ paddingLeft: `${8 + (h.level - 1) * 12}px` }"
            :title="`${h.group} · ${h.text}`"
            @click="scrollToBlock(h)"
          >
            {{ h.text }}
          </button>
        </template>
        <div v-if="!outline.length" class="ov-outline-empty">（文档无标题）</div>
      </nav>

      <div v-if="pagerVisible" class="ov-pager">
        <button class="ov-btn" :disabled="active <= 0" title="上一页" @click="go(-1)">▲</button>
        <span class="ov-page-no">{{ active + 1 }} / {{ pages.length }}</span>
        <button
          class="ov-btn"
          :disabled="active >= pages.length - 1"
          title="下一页"
          @click="go(1)"
        >
          ▼
        </button>
      </div>
    </aside>
    <div v-else class="ov-docx-rail">
      <button class="ov-side-toggle" title="展开目录" @click="toggleOutline">»</button>
    </div>

    <div class="ov-docx-main">
      <div ref="wrapRef" class="ov-docx-canvas">
        <div class="ov-page-box" :style="pageBoxStyle">
          <div
            ref="pageRef"
            class="ov-page"
            :style="[baseStyle, pageStyle]"
            @keydown="onPageKey"
            tabindex="0"
          >
            <div v-if="headerText" class="ov-page-header">{{ headerText }}</div>

            <!-- 页内容：带自顶向下堆叠（正文带 / 分栏带；连续分节的分栏区就落在同一页里） -->
            <template v-for="(band, biBand) in pageBands" :key="biBand">
              <div v-if="band.kind === 'cols'" class="ov-cols">
                <div
                  v-for="(col, ci) in band.columns"
                  :key="ci"
                  class="ov-col"
                  :style="{
                    width: `${col.width}px`,
                    marginLeft: ci > 0 ? `${col.gap}px` : undefined,
                  }"
                >
                  <DocxBlockView
                    v-for="item in col.items"
                    :key="`${item.partIndex}-${item.bi}`"
                    :data-bi="item.bi"
                    :block="item.block"
                    :doc="document"
                    :ordinal="item.ord"
                    :pick-mode="pickMode"
                    :break-width="col.width"
                    :path-of="pathOfFor(item.partIndex, item.bi)"
                    :resolve-media="resolveMedia"
                    @pick="onPick"
                  />
                </div>
              </div>
              <template v-else>
                <template v-for="item in band.items" :key="`${item.partIndex}-${item.bi}`">
                  <!-- 跨页段落：按行裁切（首/末行留 2 行，与 Word 的孤行控制一致） -->
                  <div v-if="needsClip(item)" class="ov-clip" :style="clipBoxStyle(item)">
                    <div :style="clipInnerStyle(item)">
                      <DocxBlockView
                        :data-bi="item.bi"
                        :block="item.block"
                        :doc="document"
                        :ordinal="item.ord"
                        :pick-mode="pickMode"
                        :row-from="item.rowFrom"
                        :row-to="item.rowTo"
                        :skip-lines="item.skipLines"
                        :keep-lines="item.keepLines"
                        :row-geom="item.rowGeom"
                        :break-width="band.width"
                        :path-of="pathOfFor(item.partIndex, item.bi)"
                        :resolve-media="resolveMedia"
                        @pick="onPick"
                      />
                    </div>
                  </div>
                  <DocxBlockView
                    v-else
                    :data-bi="item.bi"
                    :block="item.block"
                    :doc="document"
                    :ordinal="item.ord"
                    :pick-mode="pickMode"
                    :row-from="item.rowFrom"
                    :row-to="item.rowTo"
                    :skip-lines="item.skipLines"
                    :keep-lines="item.keepLines"
                    :row-geom="item.rowGeom"
                    :break-width="band.width"
                    :path-of="pathOfFor(item.partIndex, item.bi)"
                    :resolve-media="resolveMedia"
                    @pick="onPick"
                  />
                </template>
              </template>
            </template>

            <div v-if="footerText" class="ov-page-footer">{{ footerText }}</div>
          </div>
        </div>
      </div>

      <div v-if="props.zoom === undefined" class="ov-docx-bar">
        <span>缩放</span>
        <input v-model.number="zoom" type="range" min="0.5" max="3" step="0.05" />
        <span class="ov-zoom-val">{{ Math.round(scale * 100) }}%</span>
      </div>
    </div>

    <PathCard :state="hover.state.value" />

    <!-- 离屏度量：与页面同一份块渲染，只量不画（0×0 固定层，不参与页面布局/滚动） -->
    <div ref="measureRef" class="ov-measure-host" aria-hidden="true">
      <template v-for="(p, pi) in document.parts" :key="pi">
        <div
          v-for="ci in measureWidths(pi)"
          :key="`${pi}-${ci}`"
          class="ov-page ov-measure-page"
          :data-mc="`${pi}:${ci}`"
          :style="[baseStyle, measureStyle(pi, ci)]"
        >
          <DocxBlockView
            v-for="(b, bi) in p.blocks ?? []"
            :key="bi"
            :data-mb="bi"
            :block="b"
            :doc="document"
            :ordinal="(ordinalTable[pi] ?? [])[bi] ?? 1"
            :break-width="measureBreakWidth(pi, ci)"
            :resolve-media="resolveMedia"
          />
        </div>
      </template>
    </div>
  </div>
</template>

<style>
.ov-docx {
  display: flex;
  align-items: stretch;
  gap: 0; /* 目录与内容拼成一块，无缝 */
  /* 自撑高兜底：与其余三个 viewer（72~78vh）一致给固定上限。此前用 height:100% +
     min-height 兜底——父级没给高度时 100% 失效、容器随 A4 整页内容长高，会把页面
     撑出窗口（demo 里 docx 溢出而 pdf 不会即此因）。铺满容器传 fill prop，或宿主
     :deep 覆盖 height:100%（特异性更高，桌面页签/Modal 即此法） */
  height: min(76vh, 900px);
  font-family: 'Songti SC', 'SimSun', 'Times New Roman', serif;
  /* 纸张正文墨色跟页面层（不跟 chrome 主题），否则暗 chrome 下白纸白字 */
  color: var(--ov-page-ink);
}
/* 铺满容器模式：高度 100% 跟随容器（需容器有确定高度），内容内部滚动 */
.ov-docx.ov-fill {
  height: 100%;
}
/* 左侧：目录 + 翻页（毛玻璃侧栏） */
.ov-docx-side {
  flex: none;
  width: 208px;
  display: flex;
  flex-direction: column;
  border-radius: 0;
  background: var(--ov-bg-stage-blur);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  /* 与内容区拼成一块：只留发丝分隔线，不再投影 */
  box-shadow: inset -0.5px 0 0 var(--ov-separator);
  overflow: hidden;
}
/* 目录标题行：标题 + 收起按钮 */
.ov-side-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-right: 10px;
}
.ov-side-toggle {
  appearance: none;
  flex: none;
  width: 22px;
  height: 20px;
  padding: 0;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--ov-label-3);
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 13px;
  line-height: 1;
  cursor: default;
}
.ov-side-toggle:hover {
  background: var(--ov-bg-fill);
  color: var(--ov-label);
}
/* 目录收起后的细轨：保留展开入口，底色与侧栏一致 */
.ov-docx-rail {
  flex: none;
  width: 26px;
  display: flex;
  justify-content: center;
  align-items: flex-start;
  padding-top: 10px;
  background: var(--ov-bg-stage-blur);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  box-shadow: inset -0.5px 0 0 var(--ov-separator);
}
.ov-side-title {
  padding: 10px 12px 6px;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 11px;
  font-weight: 590;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  color: var(--ov-label-2);
}
.ov-outline {
  flex: 1;
  overflow-y: auto;
  padding: 0 6px 6px;
}
.ov-outline-item {
  display: block;
  width: 100%;
  padding: 5px 8px;
  border: 0;
  border-radius: 5px;
  background: transparent;
  color: var(--ov-label);
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 12px;
  text-align: left;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: default;
}
.ov-outline-item:hover {
  background: var(--ov-bg-fill);
}
.ov-outline-item.current {
  color: var(--ov-text-strong);
  font-weight: 500;
}
.ov-outline-part {
  padding: 8px 8px 2px;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 10.5px;
  font-weight: 590;
  letter-spacing: 0.02em;
  color: var(--ov-label-3);
  text-transform: uppercase;
}
.ov-outline-item:active {
  background: var(--ov-accent-soft);
  color: var(--ov-accent-text);
}
.ov-outline-empty {
  padding: 6px 8px;
  font-size: 11.5px;
  color: var(--ov-label-3);
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
}
/* 上下翻页按钮 */
.ov-pager {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 8px 10px;
  border-top: 0.5px solid var(--ov-separator);
  background: var(--ov-bg-toolbar);
}
.ov-btn {
  appearance: none;
  min-width: 30px;
  height: 24px;
  padding: 0 8px;
  border: 1px solid var(--ov-separator);
  border-radius: 6px;
  background: var(--ov-bg-control);
  color: var(--ov-label);
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 11px;
  line-height: 1;
  box-shadow: 0 0.5px 1px rgba(0, 0, 0, 0.08);
  cursor: default;
}
.ov-btn:hover:not(:disabled) {
  background: var(--ov-bg-control-hover);
}
.ov-btn:disabled {
  color: var(--ov-label-3);
  box-shadow: none;
}
.ov-page-no {
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 11.5px;
  color: var(--ov-label-2);
  font-variant-numeric: tabular-nums;
}
/* 内容区：整体缩放画布 */
.ov-docx-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.ov-docx-canvas {
  flex: 1;
  min-height: 0;
  display: flex;
  padding: 16px;
  overflow: auto;
  border-radius: 0;
  background: var(--ov-bg-stage);
  box-shadow: var(--ov-hairline), 0 1px 3px rgba(0, 0, 0, 0.06);
}
/* 纸张（自然尺寸渲染，外层容器按缩放后尺寸占位） */
.ov-page {
  position: relative;
  box-sizing: border-box;
  background: var(--ov-page-bg);
  border-radius: 4px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.14), 0 0 0 0.5px rgba(0, 0, 0, 0.08);
  line-height: 1.75;
  font-size: 15px;
}
.ov-page-box {
  /* 占位盒不裁切：纸张按估算高度占位，实测更高时内容照常绘制（最多压到 16px 画布留白） */
  overflow: visible;
}
/* 离屏度量层：0×0 固定层，只参与排版不参与绘制
   （用 visibility 而非 display:none——后者量不到高度；0×0 + overflow 保证不撑开滚动区） */
.ov-measure-host {
  position: fixed;
  left: 0;
  top: 0;
  width: 0;
  height: 0;
  overflow: hidden;
  visibility: hidden;
  pointer-events: none;
}
.ov-measure-page {
  transform: none;
  box-shadow: none;
}
/* 底部缩放条 */
.ov-docx-bar {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 8px;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 12px;
  color: var(--ov-label-2);
}
.ov-docx-bar input[type='range'] {
  -webkit-appearance: none;
  appearance: none;
  width: 120px;
  height: 4px;
  border-radius: 999px;
  background: rgba(120, 120, 128, 0.24);
  outline: none;
}
.ov-docx-bar input[type='range']::-webkit-slider-thumb {
  -webkit-appearance: none;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--ov-page-bg);
  box-shadow: 0 0.5px 2px rgba(0, 0, 0, 0.3), 0 0 0 0.5px rgba(0, 0, 0, 0.1);
}
.ov-zoom-val {
  font-variant-numeric: tabular-nums;
  min-width: 36px;
  text-align: right;
}
.ov-page-header,
.ov-page-footer {
  position: absolute;
  left: 0;
  right: 0;
  text-align: center;
  color: var(--ov-page-ink-2);
  font-size: 12px;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
}
.ov-page-header {
  top: 24px;
}
.ov-page-footer {
  bottom: 24px;
}
.ov-heading {
  margin: 0;
  scroll-margin-top: 16px;
}
.ov-para {
  margin: 0 0 6px;
}
/* 段落/标题作为锚浮 run 图（照片等）的定位参照 */
.ov-heading,
.ov-para {
  position: relative;
}
/* 跨页段落裁切盒：只露出本页该显示的行（Word 的段内分页） */
.ov-clip {
  overflow: hidden;
  /* 行末悬挂的标点要探出版心一个字，而 overflow:hidden 会把版面外的绘制一起裁掉；
     clip-path 只裁边框盒以外的绘制，右侧留出的这一条缝让悬挂字照常可见 */
  clip-path: inset(0 -16px 0 0);
}

/* 预断行的段落：一行一个行盒（Word 兼容断行的落点）。行距/字体全部继承段落，
   所以逐行渲染与原来自动换行的排版一致。text-indent 必须清零——每个行盒都是块容器，
   不清零的话每行都会吃一次首行缩进（首行缩进改挂首行的 margin-left）。
   nowrap 是必须的：断行已经由 linebreak 定好，再让浏览器断一次，行末悬挂的标点会被
   挤到下一行（inline-block 与后随 span 之间有断行点），悬挂就白做了。 */
.ov-line {
  display: block;
  text-indent: 0;
  white-space: nowrap;
}
.ov-jmain {
  display: inline-block;
}

/* 表格行内切（一行跨页）：逐格裁到行边界 */
.ov-cellclip {
  overflow: hidden;
}
/* 多栏带：栏盒按节定义的栏宽/间距排布（分栏在页内，跨栏顺序与度量一致） */
.ov-cols {
  display: flex;
  align-items: flex-start;
}
.ov-col {
  flex: none;
}
.ov-list {
  margin: 0 0 8px;
  padding-left: 26px;
}
.ov-table {
  width: 100%;
  border-collapse: collapse;
  margin: 8px 0 12px;
  font-size: 14px;
}
.ov-table td {
  border: 0.5px solid var(--ov-page-line);
  /* 纵向内边距 0：Word 的表格默认单元格上下边距就是 0（w:tblCellMar），
     单元格带 w:tcMar 时由行内样式给。横向取 Word 默认 0.19cm ≈ 7px。 */
  padding: 0 7px;
  vertical-align: top;
}
.ov-tr-head td {
  background: var(--ov-page-fill);
  font-weight: 600;
}
/* 无框线表格（文档 border.style=none）：不画线也不留内边距 */
.ov-table-plain td {
  border: none;
  padding: 0;
}
.ov-figure {
  margin: 10px 0;
}
.ov-figure img {
  border-radius: 4px;
  box-shadow: 0 0 0 0.5px var(--ov-gridline);
}
.ov-caption,
.ov-caption-block {
  color: var(--ov-label-2);
  font-size: 13px;
  text-align: center;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
}
.ov-caption {
  margin-top: 5px;
}
.ov-caption-block {
  margin: 4px 0 10px;
}
.ov-formula {
  margin: 8px 0;
  text-align: center;
  font-family: 'Cambria Math', 'Latin Modern Math', serif;
  position: relative;
}
.ov-formula:not(.display) {
  display: inline-block;
}
.ov-eqno {
  position: absolute;
  right: 0;
  color: var(--ov-label-2);
}
.ov-code {
  background: var(--ov-page-fill);
  border-radius: 8px;
  padding: 10px 12px;
  font-family: ui-monospace, 'SF Mono', Menlo, monospace;
  font-size: 13px;
  overflow: auto;
}
.ov-quote {
  margin: 8px 0;
  padding: 6px 12px;
  border-left: 3px solid rgba(0, 122, 255, 0.5);
  color: var(--ov-label-2);
}
.ov-toc {
  margin: 8px 0 14px;
}
.ov-toc-title {
  font-weight: 600;
  margin-bottom: 4px;
}
.ov-toc-item {
  color: var(--ov-page-ink);
}
.ov-bib {
  margin: 10px 0;
  font-size: 14px;
}
.ov-bib-title {
  font-weight: 600;
  margin-bottom: 4px;
}
.ov-bib-item {
  margin-bottom: 3px;
  text-indent: -22px;
  padding-left: 22px;
}
.ov-shape {
  padding: 8px 10px;
  margin: 8px 0;
  border-radius: 6px;
}
.ov-unknown {
  color: var(--ov-label-3);
  font-size: 12px;
}
</style>
