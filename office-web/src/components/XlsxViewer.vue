<script setup lang="ts">
//! xlsx 高保真预览：列宽/行高/样式/数字格式/合并/网格线/行列标/图片/图表 + 悬浮路径卡片
import { computed, onBeforeUnmount, onMounted, onUnmounted, ref, watch } from 'vue'
import type { XlsxChart, XlsxSheet, XlsxWorkbook } from '@/types'
import type { PackedSheet } from '@/renderers/xlsx/packed'
import { colCharsToPx, rowPtToPx, cssColor } from '@/core/units'
import { cellRef, colLetter } from '@/core/path'
import { displayValue } from '@/renderers/xlsx/numberFormat'
import { resolveStyle, styleToCss } from '@/renderers/xlsx/styles'
import { usePathHover, type SelectInfo } from '@/core/hover'
import PathCard from '@/components/PathCard.vue'
import XlsxChartView from '@/renderers/xlsx/XlsxChartView.vue'
import { parseRange } from '@/renderers/xlsx/range'
import { buildRowLayout, canvasScale, rowTop, visibleRows } from '@/renderers/xlsx/virtual'
import { adapterFor, isPackedSheet } from '@/renderers/xlsx/packed'

const props = withDefaults(
  defineProps<{
    workbook: XlsxWorkbook | { sheets: (XlsxSheet | PackedSheet)[] }
    /** 媒体解析：把 xl/media/xxx 解析成可加载 URL */
    resolveMedia?: (src: string) => string
    /** 显示 A/B/C 与 1/2/3 行列标（默认开） */
    showHeadings?: boolean
    /** 显示网格线（默认跟随文件设置） */
    showGridlines?: boolean
    /** 初始工作表下标 */
    sheetIndex?: number
  }>(),
  { showHeadings: true, sheetIndex: 0 },
)

const emit = defineEmits<{
  (e: 'select', path: string, info?: SelectInfo): void
  (e: 'hover', path: string | null): void
  (e: 'sheet-change', index: number): void
}>()

const hover = usePathHover()
const active = ref(props.sheetIndex)

watch(
  () => props.sheetIndex,
  (v) => (active.value = v),
)

/** 当前工作表（JSON 或已打包） */
const rawSheet = computed<XlsxSheet | PackedSheet | undefined>(
  () => (props.workbook as { sheets: (XlsxSheet | PackedSheet)[] }).sheets[active.value],
)

/** 默认行高（px） */
const defaultRowPx = computed(() => rowPtToPx(rawSheet.value ? sheetMetaOf(rawSheet.value).default_row_height ?? 15 : 15))

function sheetMetaOf(sh: XlsxSheet | PackedSheet): XlsxSheet {
  return isPackedSheet(sh) ? sh.meta : sh
}

/** 取数适配器：JSON 原样 / 打包形态统一接口 */
const adapter = computed(() => (rawSheet.value ? adapterFor(rawSheet.value, defaultRowPx.value) : undefined))
/** 工作表元信息（合并/图表/图片/列宽/打印…） */
const sheet = computed<XlsxSheet | undefined>(() => (rawSheet.value ? sheetMetaOf(rawSheet.value) : undefined))
/** 组件外部读取当前工作表（保持原有 `sheet` 语义） */

const cellAt = (r: number, c: number) => adapter.value?.cell(r, c)

/** 标签色（兼容打包形态） */
function sheetTabColor(s: XlsxSheet | PackedSheet): string {
  const color = sheetMetaOf(s).tab_color
  return color ? cssColor(color) : ''
}

function colToIndex(letters: string): number {
  let n = 0
  for (const ch of letters) n = n * 26 + (ch.charCodeAt(0) - 64)
  return n
}

/** 合并区锚点 → (colspan, rowspan)，被吞并的单元格 */
const mergeInfo = computed(() => {
  const anchors = new Map<string, [number, number]>()
  const covered = new Set<string>()
  for (const m of sheet.value?.merges ?? []) {
    const r = parseRange(m)
    if (!r) continue
    const { c1, r1, c2, r2 } = r
    if (c2 > c1 || r2 > r1) {
      anchors.set(`${r1}:${c1}`, [c2 - c1 + 1, r2 - r1 + 1])
      for (let rr = r1; rr <= r2; rr += 1)
        for (let cc = c1; cc <= c2; cc += 1) if (!(rr === r1 && cc === c1)) covered.add(`${rr}:${cc}`)
    }
  }
  return { anchors, covered }
})

/** 最大列号由适配器给出（打包形态已预计算） */
const maxCol = computed(() => adapter.value?.maxCol ?? 0)

/** 行号集合：工作表行 + 合并区覆盖的行 */
const rowIds = computed(() => {
  const base = adapter.value ? Array.from(adapter.value.rowNos) : []
  const set = new Set<number>(base)
  for (const m of sheet.value?.merges ?? []) {
    const r = parseRange(m)
    if (r) for (let rr = r.r1; rr <= r.r2; rr += 1) set.add(rr)
  }
  const arr = [...set].sort((a, b) => a - b)
  return arr
})

const defaultColChars = computed(
  () => sheet.value?.default_col_width ?? sheet.value?.base_col_width ?? 8.43,
)

const colWidthPx = (c: number): number => {
  const col = sheet.value?.columns?.[c - 1]
  if (col?.hidden) return 0
  return colCharsToPx(col?.width ?? defaultColChars.value)
}

/** 默认行高（px）与显式行高表（行号 → px） */
const rowHeights = computed(() => {
  const m = new Map<number, number>()
  const a = adapter.value
  if (!a) return m
  for (const r of a.rowNos) {
    const h = a.heightOfRow(r)
    if (h !== undefined) m.set(r, h)
  }
  return m
})

/** 行布局（前缀高度）：百万行下只存稀疏增量 */
const layout = computed(() => buildRowLayout(rowIds.value, rowHeights.value, defaultRowPx.value))

/** 按行号取行高（px）：显式行高优先，否则默认行高 */
function rowHeightPx(r: number): number {
  return rowHeights.value.get(r) ?? defaultRowPx.value
}

/* ---- 虚拟滚动：只渲染视口内的行 ---- */
const wrapRef = ref<HTMLElement | null>(null)
const scrollTop = ref(0)
const viewportH = ref(600)
const wrapW = ref(0)
/** 用户缩放（50%~200%，1 = 自然尺寸） */
const zoom = ref(1)
let rafId = 0

function onScroll() {
  if (rafId) return
  rafId = requestAnimationFrame(() => {
    rafId = 0
    const el = wrapRef.value
    if (!el) return
    scrollTop.value = el.scrollTop
    viewportH.value = el.clientHeight
  })
}

/** 画布压缩比（>1 行高上限时 < 1）；滚动位置 → 逻辑像素的换算系数 */
const scaleY = computed(() => canvasScale(layout.value.total))
/** 压缩画布的滚动位置换算回逻辑像素 */
const logicalTop = computed(() => scrollTop.value / scaleY.value)
const logicalViewportH = computed(() => viewportH.value / scaleY.value)

const range = computed(() => visibleRows(layout.value, logicalTop.value, logicalViewportH.value))
/** 视口内的行号（含 overscan） */
const visibleRowIds = computed(() => {
  const [a, b] = range.value
  if (b < a) return [] as number[]
  return rowIds.value.slice(a, b + 1)
})
/** 渲染起点在整表中的像素偏移 */
const offsetTop = computed(() => rowTop(layout.value, Math.max(0, range.value[0])))
/** 画布高度（压缩后仍在上限内） */
const canvasH = computed(() => layout.value.total * scaleY.value)
/** 画布总高：数据区与浮动对象（图表/图片锚点延伸）取较大者，保证都能滚动查看 */
const canvasTotal = computed(() => Math.max(canvasH.value, floatsExtent.value * scaleY.value))

/** 缩放占位盒：撑出滚动范围；内容按逻辑尺寸渲染后整体 transform 缩放 */
const zoomerStyle = computed(() => ({
  width: `${Math.round(canvasWidth.value * zoom.value)}px`,
  height: `${Math.round(canvasTotal.value * zoom.value)}px`,
}))
const canvasStyle = computed(() => ({
  width: `${Math.round(canvasWidth.value)}px`,
  transform: `scale(${zoom.value})`,
  transformOrigin: 'top left',
}))

let roWrap: ResizeObserver | null = null
onMounted(() => {
  const el = wrapRef.value
  if (!el) return
  viewportH.value = el.clientHeight
  wrapW.value = el.clientWidth
  el.addEventListener('scroll', onScroll, { passive: true })
  roWrap = new ResizeObserver(() => {
    viewportH.value = el.clientHeight
    wrapW.value = el.clientWidth
  })
  roWrap.observe(el)
})
onBeforeUnmount(() => {
  wrapRef.value?.removeEventListener('scroll', onScroll)
  roWrap?.disconnect()
  roWrap = null
  if (rafId) cancelAnimationFrame(rafId)
})

watch(active, () => {
  scrollTop.value = 0
  if (wrapRef.value) wrapRef.value.scrollTop = 0
})

const gridlines = computed(() =>
  props.showGridlines !== undefined ? props.showGridlines : sheet.value?.gridlines !== false,
)

const tableStyle = computed(() => ({
  borderCollapse: 'collapse' as const,
  tableLayout: 'fixed' as const,
  fontSize: '13px',
}))

function cellStyle(r: number, c: number): Record<string, string> {
  const cell = cellAt(r, c)
  const s = resolveStyle(props.workbook as XlsxWorkbook, cell?.style)
  const css = styleToCss(s)
  const explicitAlign = s?.alignment?.horizontal
  if (!explicitAlign) {
    if (typeof cell?.value === 'number') css['text-align'] = 'right'
    else if (typeof cell?.value === 'boolean') css['text-align'] = 'center'
  }
  if (!gridlines.value) css['border'] = 'none'
  return css
}

function cellText(r: number, c: number): string {
  const cell = cellAt(r, c)
  if (!cell) return ''
  // Excel 语义：有缓存值显示计算结果（与 LibreOffice/Excel 一致），无值才回退公式文本
  const f = displayValue(cell.value, cell.number_format)
  if (f.text) return f.text
  if (cell.formula) return `=${cell.formula}`
  return ''
}

function cellColor(r: number, c: number): string | undefined {
  const cell = cellAt(r, c)
  if (!cell || typeof cell.value !== 'number') return undefined
  return displayValue(cell.value, cell.number_format).color
}

/** 单元格有效水平对齐（显式样式优先；文本左/数字右/布尔中，同 Excel general 语义） */
function cellAlign(r: number, c: number): string {
  const cell = cellAt(r, c)
  const s = resolveStyle(props.workbook as XlsxWorkbook, cell?.style)
  const explicit = s?.alignment?.horizontal
  if (explicit) return explicit
  if (cell && typeof cell.value === 'number') return 'right'
  if (cell && typeof cell.value === 'boolean') return 'center'
  return 'left'
}

/** 单元格是否为空（无值无公式；合并覆盖格与空白格视作空，可被溢入） */
function isCellEmpty(r: number, c: number): boolean {
  const cell = cellAt(r, c)
  if (!cell) return true
  const hasValue = cell.value !== undefined && cell.value !== null && cell.value !== ''
  return !hasValue && !cell.formula
}

/** 从 c 沿 dir 方向的连续空格总宽（px） */
function emptyExtent(r: number, c: number, dir: 1 | -1): number {
  let w = 0
  let cc = c + dir
  while (cc >= 1 && cc <= maxCol.value) {
    if (!isCellEmpty(r, cc)) break
    w += colWidthPx(cc)
    cc += dir
  }
  return w
}

/** Excel 文本溢出语义：字符串过长时溢入相邻空单元格，在首个非空格边界裁剪（无省略号）；
 *  数字/公式格不溢出（过窄即裁剪，对应 Excel 的 ### 行为）。返回文本 span 的定位样式。 */
function cellTextStyle(r: number, c: number): Record<string, string> | undefined {
  const color = cellColor(r, c)
  const base = color ? { color: `#${color}` } : undefined
  const cell = cellAt(r, c)
  if (!cell || cell.formula || typeof cell.value !== 'string' || cell.value === '') return base
  const align = cellAlign(r, c)
  const P = 5 // 对齐 td 水平内边距
  const ownW = colWidthPx(c)
  const leftExt = emptyExtent(r, c, -1)
  const rightExt = emptyExtent(r, c, 1)
  const style: Record<string, string> = { ...base }
  if (align === 'right' && leftExt > 0) {
    style.right = `${P}px`
    style.textAlign = 'right'
    style.maxWidth = `${ownW + leftExt - P * 2}px`
  } else if (align === 'center' && (leftExt > 0 || rightExt > 0)) {
    style.left = '50%'
    style.transform = 'translate(-50%, -50%)'
    style.maxWidth = `${leftExt + ownW + rightExt - P * 2}px`
  } else if (align !== 'right' && align !== 'center' && rightExt > 0) {
    style.left = `${P}px`
    style.maxWidth = `${ownW + rightExt - P * 2}px`
  } else {
    return base
  }
  style.position = 'absolute'
  style.top = '50%'
  if (!style.transform) style.transform = 'translateY(-50%)'
  style.width = 'max-content'
  style.whiteSpace = 'pre'
  style.overflow = 'hidden'
  return style
}

/** td 样式：有溢出 span 时放开裁剪，让 span 越界绘制 */
function tdStyle(r: number, c: number): Record<string, string> {
  const css = cellStyle(r, c)
  const cell = cellAt(r, c)
  if (cell && !cell.formula && typeof cell.value === 'string' && cell.value !== '') {
    return { ...css, overflow: 'visible' }
  }
  return css
}

function onCellEnter(r: number, c: number, ev: MouseEvent) {
  const cell = cellAt(r, c)
  const ref = cellRef(c, r)
  const path = `/sheet[${active.value + 1}]/cell[${ref}]`
  const f = cell ? displayValue(cell.value, cell.number_format) : undefined
  hover.enter(
    {
      path,
      type: 'cell',
      name: `${sheet.value?.name ?? ''}!${ref}`,
      text: f?.text || (cell?.comment ? `批注: ${cell.comment}` : undefined),
    },
    ev,
  )
  emit('hover', path)
}

function onCellLeave() {
  hover.leave()
  emit('hover', null)
}

function onCellClick(r: number, c: number) {
  const ref = cellRef(c, r)
  const path = `/sheet[${active.value + 1}]/cell[${ref}]`
  const cell = cellAt(r, c)
  const f = cell ? displayValue(cell.value, cell.number_format) : undefined
  emit('select', path, {
    path,
    type: 'cell',
    name: `${sheet.value?.name ?? ''}!${ref}`,
    text: f?.text || (cell?.comment ? `批注: ${cell.comment}` : undefined),
  })
}

/** 锚点列号的横向偏移（列配置外的列按默认列宽推进） */
function anchorLeft(c: number): number {
  let left = 0
  for (let k = 1; k < c; k += 1) left += colWidthPx(k)
  return left
}

/** 锚点行号的纵向偏移：表内行取实际行高累计；缺失/超出的行按默认行高外推 */
function anchorTop(r: number): number {
  const ids = rowIds.value
  if (!ids.length) return 0
  const last = ids[ids.length - 1]
  if (r > last) {
    return (
      rowTop(layout.value, ids.length - 1) + rowHeightPx(last) + (r - last) * defaultRowPx.value
    )
  }
  const pos = ids.indexOf(r)
  if (pos >= 0) return rowTop(layout.value, pos)
  let k = 0
  while (k < ids.length && ids[k] < r) k += 1
  if (k === 0) return rowTop(layout.value, 0) - (ids[0] - r) * defaultRowPx.value
  const base = ids[k - 1]
  return rowTop(layout.value, k - 1) + (r - base) * defaultRowPx.value
}

/** 浮动对象（锚点单元格 + 英寸尺寸）的下边界，画布高度需容纳以免无法滚动查看 */
const floatsExtent = computed(() => {
  let bottom = layout.value.total
  const objs = [
    ...(sheet.value?.charts ?? []).map((c) => ({ a: c.anchor, s: c.size })),
    ...(sheet.value?.images ?? []).map((im) => ({ a: im.anchor, s: im.size })),
  ]
  for (const o of objs) {
    const m = /^([A-Z]+)(\d+)$/.exec((o.a ?? '').replace(/[^A-Za-z0-9]/g, ''))
    if (!m) continue
    const h = (o.s?.h ?? 2) * 96
    bottom = Math.max(bottom, anchorTop(Number(m[2])) + h)
  }
  return bottom
})

/** 数据区表格宽度（行头 + 各列宽），不随画布伸缩，保证锚点对齐 */
const tableW = computed(() => {
  let w = props.showHeadings ? 44 : 0
  for (let c = 1; c <= maxCol.value; c += 1) w += colWidthPx(c)
  return w
})

/** 浮动对象的右边界（含图表/图片宽度） */
const floatsRight = computed(() => {
  let right = 0
  const objs = [
    ...(sheet.value?.charts ?? []).map((c) => ({ a: c.anchor, s: c.size })),
    ...(sheet.value?.images ?? []).map((im) => ({ a: im.anchor, s: im.size })),
  ]
  for (const o of objs) {
    const m = /^([A-Z]+)(\d+)$/.exec((o.a ?? '').replace(/[^A-Za-z0-9]/g, ''))
    if (!m) continue
    right = Math.max(right, anchorLeft(colToIndex(m[1])) + (o.s?.w ?? 4.6) * 96)
  }
  return right
})

/** 画布总宽：容器宽 / 数据区 / 浮动对象右边界取较大者 */
const canvasWidth = computed(() => Math.max(Math.max(0, wrapW.value), tableW.value, floatsRight.value))

/** 图片锚点定位（近似：按 anchor 单元格定位到对应行列偏移） */
function imageBox(img: { anchor?: string | null; size?: { w: number; h: number } | null }) {
  const w = img.size ? `${Math.round(img.size.w * 96)}px` : '160px'
  const h = img.size ? `${Math.round(img.size.h * 96)}px` : 'auto'
  const cellRefStr = (img.anchor ?? '').replace(/[^A-Za-z0-9:]/g, '')
  const m = /^([A-Z]+)(\d+)(?::([A-Z]+)(\d+))?$/.exec(cellRefStr)
  if (!m) return { width: w, height: h }
  return {
    width: w,
    height: h,
    position: 'absolute' as const,
    left: `${anchorLeft(colToIndex(m[1]))}px`,
    top: `${anchorTop(Number(m[2])) * scaleY.value}px`,
  }
}

/** 图表锚点定位：anchor 单元格 + 英寸尺寸（Excel 浮动图表，悬浮于网格之上） */
function chartBox(ch: XlsxChart, i: number): Record<string, string> {
  const w = `${Math.round((ch.size?.w ?? 4.6) * 96)}px`
  const h = `${Math.round((ch.size?.h ?? 3) * 96)}px`
  const m = /^([A-Z]+)(\d+)$/.exec((ch.anchor ?? '').replace(/[^A-Za-z0-9]/g, ''))
  if (!m) {
    // 无锚点兜底：排到表格下方按序堆叠，避免压住数据区
    return {
      width: w,
      height: h,
      position: 'absolute',
      left: '16px',
      top: `${Math.round(canvasH.value) + 16 + i * 300}px`,
    }
  }
  return {
    width: w,
    height: h,
    position: 'absolute',
    left: `${anchorLeft(colToIndex(m[1]))}px`,
    top: `${anchorTop(Number(m[2])) * scaleY.value}px`,
  }
}

function onSheetKey(ev: KeyboardEvent) {
  if (ev.key === 'ArrowDown' || ev.key === 'PageDown') {
    active.value = Math.min(active.value + 1, props.workbook.sheets.length - 1)
  } else if (ev.key === 'ArrowUp' || ev.key === 'PageUp') {
    active.value = Math.max(active.value - 1, 0)
  } else return
  emit('sheet-change', active.value)
}

onMounted(() => window.addEventListener('keydown', onSheetKey))
onUnmounted(() => window.removeEventListener('keydown', onSheetKey))

defineExpose({ hover })
</script>

<template>
  <div class="ov-xlsx">
    <div ref="wrapRef" class="ov-sheet-wrap">
      <!-- 缩放占位盒撑出滚动范围；画布按逻辑尺寸渲染后整体缩放（与 PPT 舞台同法） -->
      <div class="ov-sheet-zoomer" :style="zoomerStyle">
      <!-- 虚拟滚动：画布给出整表高度，仅渲染视口内的行 -->
      <div class="ov-sheet-canvas" :style="{ height: `${Math.round(canvasTotal)}px`, ...canvasStyle }">
        <table
          class="ov-sheet"
          :style="{
            ...tableStyle,
            position: 'absolute',
            left: '0',
            top: `${Math.round(offsetTop * scaleY)}px`,
            width: `${Math.round(tableW)}px`,
          }"
        >
          <colgroup>
            <col v-if="showHeadings" style="width: 44px" />
            <col v-for="c in maxCol" :key="c" :style="{ width: `${colWidthPx(c)}px` }" />
          </colgroup>
          <thead v-if="showHeadings">
            <tr>
              <th class="ov-corner"></th>
              <th v-for="c in maxCol" :key="c" class="ov-colhead">{{ colLetter(c) }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="r in visibleRowIds" :key="r" :style="{ height: `${rowHeightPx(r)}px` }">
              <th v-if="showHeadings" class="ov-rowhead">{{ r }}</th>
              <template v-for="c in maxCol" :key="c">
                <td
                  v-if="!mergeInfo.covered.has(`${r}:${c}`)"
                  :rowspan="mergeInfo.anchors.get(`${r}:${c}`)?.[1] ?? 1"
                  :colspan="mergeInfo.anchors.get(`${r}:${c}`)?.[0] ?? 1"
                  class="ov-cell"
                  :style="tdStyle(r, c)"
                  :title="cellAt(r, c)?.comment ?? undefined"
                  @mouseenter="onCellEnter(r, c, $event)"
                  @mousemove="hover.move($event)"
                  @mouseleave="onCellLeave"
                  @click="onCellClick(r, c)"
                >
                  <span :style="cellTextStyle(r, c)">{{ cellText(r, c) }}</span>
                  <a
                    v-if="cellAt(r, c)?.link"
                    class="ov-link"
                    :href="cellAt(r, c)!.link!"
                    target="_blank"
                    rel="noreferrer"
                    @click.stop
                    >↗</a
                  >
                </td>
              </template>
            </tr>
          </tbody>
        </table>

        <div v-if="(sheet?.images ?? []).length" class="ov-drawings">
          <img
            v-for="(img, i) in sheet?.images ?? []"
            :key="i"
            class="ov-drawing"
            :src="resolveMedia ? resolveMedia(img.src) : img.src"
            :style="imageBox(img)"
            :alt="img.src"
            @mouseenter="
              hover.enter(
                {
                  path: `/sheet[${active + 1}]/image[${i + 1}]`,
                  type: 'image',
                  name: img.src,
                },
                $event,
              )
            "
            @mousemove="hover.move($event)"
            @mouseleave="onCellLeave"
          />
        </div>

        <div v-if="(sheet?.charts ?? []).length" class="ov-charts">
          <XlsxChartView
            v-for="(ch, i) in sheet?.charts ?? []"
            :key="i"
            :chart="ch"
            :sheet="sheet!"
            :chart-index="i + 1"
            :sheet-index="active + 1"
            :style="chartBox(ch, i)"
            @hover-cell="(info, ev) => hover.enter(info, ev)"
            @leave="onCellLeave"
            @move="hover.move($event)"
          />
        </div>
      </div>
      </div>
    </div>

    <!-- 底部：工作表标签（左） + 缩放（最右，Numbers/Excel 式底栏） -->
    <div class="ov-sheet-tabs">
      <div class="ov-tabs-scroll">
        <button
          v-for="(s, i) in workbook.sheets as XlsxSheet[]"
          :key="s.name"
          class="ov-tab"
          :class="{ active: i === active }"
          :style="sheetTabColor(s) ? { boxShadow: `inset 0 3px 0 ${sheetTabColor(s)}` } : undefined"
          @click="
            active = i;
            emit('sheet-change', i)
          "
        >
          {{ s.name }}
        </button>
      </div>
      <label class="ov-zoom">
        缩放
        <input v-model.number="zoom" type="range" min="0.5" max="2" step="0.05" />
        <span>{{ Math.round(zoom * 100) }}%</span>
      </label>
    </div>

    <PathCard :state="hover.state.value" />
  </div>
</template>

<style>
.ov-xlsx {
  display: flex;
  flex-direction: column;
  /* 固定整体高度：切换工作表时不因内容多少而变化（底部标签栏位置稳定） */
  height: min(78vh, 860px);
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', 'Helvetica Neue',
    sans-serif;
  color: var(--ov-label);
  border-radius: 12px;
  overflow: hidden;
  background: var(--ov-page-bg);
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.08), 0 1px 3px rgba(0, 0, 0, 0.06);
}
/* 工作表标签：Numbers 底部标签栏风格（分段控件）；左侧标签滚动，右侧缩放固定 */
.ov-sheet-tabs {
  flex: none;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  background: var(--ov-bg-toolbar);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  border-top: 0.5px solid var(--ov-separator);
}
.ov-tabs-scroll {
  flex: 1;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 2px;
  overflow-x: auto;
}
.ov-sheet-tabs .ov-zoom {
  flex: none;
  font-size: 12px;
  color: var(--ov-label-2);
}
.ov-tab {
  appearance: none;
  border: 0;
  padding: 4px 11px;
  border-radius: 5px;
  background: transparent;
  color: var(--ov-label);
  font: inherit;
  font-size: 12px;
  white-space: nowrap;
  cursor: default;
}
.ov-tab:hover {
  background: var(--ov-bg-fill);
}
.ov-tab.active {
  background: var(--ov-bg-control);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12), 0 0 0 0.5px rgba(0, 0, 0, 0.06);
  font-weight: 500;
  color: var(--ov-accent-text);
}
.ov-sheet-wrap {
  position: relative;
  flex: 1; /* 撑满固定高度，内容超出时在内部滚动 */
  min-height: 0;
  overflow: auto;
  background: var(--ov-page-bg);
}
.ov-sheet-zoomer {
  position: relative;
}
.ov-sheet-canvas {
  position: relative;
}
.ov-sheet-canvas table {
  border-collapse: collapse;
  table-layout: fixed;
}
.ov-spacer td,
.ov-spacer th {
  padding: 0;
  border: 0;
  height: inherit;
}
/* 行列标：系统灰 + 发丝线 */
.ov-sheet th {
  background: var(--ov-bg-head);
  color: var(--ov-label-2);
  font-weight: 400;
  font-size: 11px;
  border: 0.5px solid rgba(0, 0, 0, 0.1);
  text-align: center;
  user-select: none;
}
.ov-corner {
  width: 44px;
}
.ov-colhead,
.ov-rowhead {
  position: sticky;
  top: 0;
  z-index: 2;
}
.ov-rowhead {
  left: 0;
  top: auto;
  width: 44px;
}
.ov-cell {
  position: relative; /* 文本溢出 span 的定位基准 */
  border: 0.5px solid rgba(0, 0, 0, 0.1);
  padding: 1px 5px;
  overflow: hidden;
  white-space: pre;
  cursor: default;
}
/* 悬浮：系统蓝细环（Numbers 观感） */
.ov-cell:hover {
  outline: 2px solid rgba(0, 122, 255, 0.6);
  outline-offset: -2px;
  border-radius: 2px;
}
.ov-link {
  margin-left: 4px;
  color: var(--ov-accent);
  text-decoration: none;
}
/* 浮动对象容器：定位在画布原点，子元素按锚点单元格绝对定位（Excel 浮动图表/图片） */
.ov-drawings {
  position: absolute;
  left: 0;
  top: 0;
}
.ov-drawing {
  border-radius: 4px;
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.12);
}
.ov-charts {
  position: absolute;
  left: 0;
  top: 0;
}
</style>
