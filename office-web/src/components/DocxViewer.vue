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
import { fontStack, normalLineRatio, ordinals, resetLineRatioCache } from '@/renderers/docx/blocks'
import { paginateBlocks, type PaginateBlock } from '@/renderers/docx/paginate'
import { LATEX_CSS } from '@/renderers/docx/latex'

const props = withDefaults(
  defineProps<{
    document: DocxDocument
    /** 媒体解析：把 word/media/xxx 解析成可加载 URL */
    resolveMedia?: (src: string) => string
    /** 页码格式：{page} 占位（默认读取 section.footer_format） */
    pageFormat?: string
    /** 初始分片下标（会被换算成该分片的第一页） */
    partIndex?: number
    /** 受控缩放:相对「适配可视区基准」的用户倍率。提供即受控——隐藏内置缩放控件、
     *  缩放只跟此值走;不传(undefined)保持自主模式(内置滑杆可用)。 */
    zoom?: number
    /** 拾取模式:仅 true 时启用交互层——元素 hover 高亮/路径卡/选中事件;
     *  false(默认)为纯只读观感(无高亮、无卡片、不发 select)。 */
    pickMode?: boolean
  }>(),
  { partIndex: 0 },
)

const emit = defineEmits<{
  (e: 'select', path: string, info?: SelectInfo): void
  (e: 'hover', path: string | null): void
  /** 翻页(物理页下标，非分片下标) */
  (e: 'page-change', index: number): void
  /** 缩放汇报(scale 变化即发):scale=绝对倍率(展示用百分比),fitScale=适配基准(宿主换算) */
  (e: 'zoom-change', v: { scale: number; fitScale: number }): void
}>()

const hover = usePathHover()

const PAGE_SIZES: Record<string, [number, number]> = {
  A4: [8.27, 11.69],
  A3: [11.69, 16.54],
  A5: [5.83, 8.27],
  Letter: [8.5, 11],
  Legal: [8.5, 14],
}

/** 纸张几何（自然 px）：宽高按纸张/方向，四边距按磅换算 */
interface Geometry {
  paperW: number
  paperH: number
  padTop: number
  padRight: number
  padBottom: number
  padLeft: number
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
  return {
    paperW: Math.round(pw * 96),
    paperH: Math.round(ph * 96),
    padTop: Math.round(ptToPx(pt(m.top, 72))),
    padRight: Math.round(ptToPx(pt(m.right, 90))),
    padBottom: Math.round(ptToPx(pt(m.bottom, 72))),
    padLeft: Math.round(ptToPx(pt(m.left, 90))),
  }
}

const geometry = computed<Geometry[]>(() =>
  props.document.parts.map((_, pi) => geometryOf(pi)),
)
/** 同名块序号表（分片内）：路径与展示编号共用 */
const ordinalTable = computed<number[][]>(() =>
  props.document.parts.map((p) => ordinals(p.blocks ?? [])),
)

/** 物理页：流坐标里的内容范围 + 渲染用的分片段（一页可跨分片） */
interface RenderSegment {
  partIndex: number
  from: number
  to: number
}
interface RenderPage {
  segments: RenderSegment[]
  top: number
  bottom: number
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
}

function measureParts(): Array<MeasuredPart | null> {
  const els = Array.from(
    measureRef.value?.querySelectorAll<HTMLElement>('.ov-measure-page') ?? [],
  )
  return props.document.parts.map((_, pi) => {
    const el = els[pi]
    if (!el || !el.offsetWidth) return null
    const nodes = Array.from(el.querySelectorAll<HTMLElement>('[data-mb]')).filter(
      (node) => !node.classList.contains('ov-pagebreak-mark'),
    )
    if (!nodes.length) return { tops: [], bottoms: [], contentH: 0 }
    // 分片内统一基准：只需相对量（差值与页高比较），基准取最小值避免首块外边距干扰
    const base = Math.min(...nodes.map((node) => node.offsetTop))
    const tops: (number | null)[] = []
    const bottoms: (number | null)[] = []
    let contentH = 0
    nodes.forEach((node) => {
      const bi = Number(node.dataset.mb)
      tops[bi] = node.offsetTop - base
      bottoms[bi] = node.offsetTop - base + node.offsetHeight
      contentH = Math.max(contentH, node.offsetTop - base + node.offsetHeight)
    })
    return { tops, bottoms, contentH }
  })
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

/**
 * 全文档块流：产物分片是**编辑分片**（分节符 / 一级标题切块），不是页——
 * 分片之间默认内容续排，只有分节符（前一分片带节属性）或纸张变化处才强制分页。
 * 各分片块坐标平移到流坐标（前一分片内容末尾 = 本分片起点）后统一分页。
 */
function buildFlow(measured: Array<MeasuredPart | null>): {
  flow: PaginateBlock[]
  map: { partIndex: number; bi: number }[]
  totalH: number
  fallbackPageH: number
} {
  const flow: PaginateBlock[] = []
  const map: { partIndex: number; bi: number }[] = []
  let offset = 0
  let fallbackPageH = Number.POSITIVE_INFINITY
  props.document.parts.forEach((part, pi) => {
    const geom = geometry.value[pi] ?? geometryOf(0)
    const m = measured[pi]
    const pageH = m
      ? Math.max(80, geom.paperH - geom.padTop - geom.padBottom)
      : Number.POSITIVE_INFINITY
    if (Number.isFinite(pageH)) fallbackPageH = Math.min(fallbackPageH, pageH)
    // 分节符：前一分片**自带**节属性（分节符挂在它身上）→ 新节另起一页；
    // 无分节符的文档只有末尾分片带 body sectPr（作用于全文），不能据此在分片间断页
    const sectioned =
      pi > 0 &&
      ((props.document.parts[pi - 1]?.section ?? null) !== null ||
        !sameGeometry(geometry.value[pi - 1], geom))
    const blocks = (part.blocks ?? []) as Record<string, unknown>[]
    blocks.forEach((b, bi) => {
      const isBreak = b.type === 'page_break'
      const top = isBreak ? null : (m?.tops[bi] ?? null)
      const bottom = isBreak ? null : (m?.bottoms[bi] ?? null)
      flow.push({
        top: top === null ? null : offset + top,
        bottom: bottom === null ? null : offset + bottom,
        breakBefore: !!b.page_break_before || (bi === 0 && sectioned),
        breakHere: isBreak,
        pageH,
      })
      map.push({ partIndex: pi, bi })
    })
    offset += m?.contentH ?? 0
  })
  return { flow, map, totalH: offset, fallbackPageH }
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

/** 重算物理分页：硬分页（分页符/段前分页/分节）+ 软分页（超出页高的块另起一页） */
async function rebuild(): Promise<void> {
  await nextTick()
  const measured = measureParts()
  const { flow, map, totalH, fallbackPageH } = buildFlow(measured)
  const out: RenderPage[] = paginateBlocks(flow, fallbackPageH, totalH).map((r) => ({
    segments: toSegments(map, r.from, r.to),
    top: r.top,
    bottom: r.bottom,
  }))
  // 保持阅读位置：原页起始块仍在则跟随，否则按下标夹取
  const anchor = pages.value[active.value]?.segments[0]
  const hit = anchor
    ? out.findIndex(
        (p) => p.segments[0]?.partIndex === anchor.partIndex && p.segments[0]?.from === anchor.from,
      )
    : -1
  pages.value = out
  active.value = hit >= 0 ? hit : Math.min(active.value, Math.max(0, out.length - 1))
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
const curPartIndex = computed(() => curPage.value?.segments[0]?.partIndex ?? 0)
const curGeom = computed<Geometry>(() => geometry.value[curPartIndex.value] ?? geometryOf(0))

/** 页自然尺寸：纸张高与内容高的较大者（超高块不裁切，页面长高） */
const naturalW = computed(() => curGeom.value.paperW)
const naturalH = computed(() => {
  const g = curGeom.value
  const p = curPage.value
  if (!p) return g.paperH
  return Math.max(g.paperH, g.padTop + Math.max(0, p.bottom - p.top) + g.padBottom)
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

/** 度量页几何：与渲染页同宽同边距（无缩放、无高度约束——量的是自然高度） */
function measureStyle(partIndex: number): Record<string, string> {
  const g = geometry.value[partIndex] ?? geometryOf(0)
  return {
    width: `${g.paperW}px`,
    minHeight: `${g.paperH}px`,
    padding: `${g.padTop}px ${g.padRight}px ${g.padBottom}px ${g.padLeft}px`,
  }
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
  out['line-height'] = String(ls * normalLineRatio(family))
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

/** 当前页按分片段取值（含块在所属分片中的绝对下标与序号，路径与分片内序号保持一致） */
const pageBlocks = computed(() => {
  const page = curPage.value
  if (!page) return []
  const out: { partIndex: number; block: DocxBlock; bi: number; ord: number }[] = []
  page.segments.forEach((seg) => {
    const blocks = props.document.parts[seg.partIndex]?.blocks ?? []
    const ords = ordinalTable.value[seg.partIndex] ?? []
    for (let bi = seg.from; bi < seg.to; bi += 1) {
      const block = blocks[bi]
      if (!block) continue
      out.push({ partIndex: seg.partIndex, block, bi, ord: ords[bi] ?? 1 })
    }
  })
  return out
})

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

/** 目录点击：定位到该标题所在的物理页，再滚动到块 */
async function scrollToBlock(entry: { partIndex: number; bi: number }) {
  const inPage = (p: RenderPage) =>
    p.segments.some((s) => s.partIndex === entry.partIndex && entry.bi >= s.from && entry.bi < s.to)
  const hit = pages.value.findIndex(inPage)
  const target =
    hit >= 0
      ? hit
      : pages.value.findIndex((p) => p.segments.some((s) => s.partIndex === entry.partIndex))
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
  const hit = pages.value.findIndex((p) => p.segments.some((s) => s.partIndex === partIndex))
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
// 页面基准排版（字号/行距）变化后块高度变，需重算分页
watch(baseStyle, scheduleRebuild)
watch(active, () => {
  // 换页后实测高度先清零，等新页的 RO 回调补上（避免沿用上一页的高值撑大占位盒）
  pageRenderedH.value = 0
})

defineExpose({ hover })
</script>

<template>
  <div class="ov-docx">
    <!-- 左侧：目录（按分片分组）+ 上下翻页 -->
    <aside class="ov-docx-side">
      <div class="ov-side-title">目录</div>
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

      <div class="ov-pager">
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

            <DocxBlockView
              v-for="item in pageBlocks"
              :key="`${item.partIndex}-${item.bi}`"
              :data-bi="item.bi"
              :block="item.block"
              :doc="document"
              :ordinal="item.ord"
              :pick-mode="pickMode"
              :path-of="pathOfFor(item.partIndex, item.bi)"
              :resolve-media="resolveMedia"
              @pick="onPick"
            />

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
      <div
        v-for="(p, pi) in document.parts"
        :key="pi"
        class="ov-page ov-measure-page"
        :style="[baseStyle, measureStyle(pi)]"
      >
        <DocxBlockView
          v-for="(b, bi) in p.blocks ?? []"
          :key="bi"
          :data-mb="bi"
          :block="b"
          :doc="document"
          :ordinal="(ordinalTable[pi] ?? [])[bi] ?? 1"
          :resolve-media="resolveMedia"
        />
      </div>
    </div>
  </div>
</template>

<style>
.ov-docx {
  display: flex;
  align-items: stretch;
  gap: 14px;
  font-family: 'Songti SC', 'SimSun', 'Times New Roman', serif;
  color: var(--ov-text-strong);
}
/* 左侧：目录 + 翻页（毛玻璃侧栏） */
.ov-docx-side {
  flex: none;
  width: 208px;
  display: flex;
  flex-direction: column;
  height: min(76vh, 900px);
  border-radius: 12px;
  background: var(--ov-bg-stage-blur);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.1), 0 1px 3px rgba(0, 0, 0, 0.06);
  overflow: hidden;
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
  flex: none;
  display: flex;
  height: min(76vh, 900px);
  padding: 16px;
  overflow: auto;
  border-radius: 12px;
  background: var(--ov-bg-stage);
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.08), 0 1px 3px rgba(0, 0, 0, 0.06);
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
  color: rgba(0, 0, 0, 0.4);
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
  border: 0.5px solid rgba(0, 0, 0, 0.28);
  padding: 5px 9px;
  vertical-align: top;
}
.ov-tr-head td {
  background: rgba(120, 120, 128, 0.1);
  font-weight: 600;
}
.ov-figure {
  margin: 10px 0;
}
.ov-figure img {
  border-radius: 4px;
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.12);
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
  background: rgba(120, 120, 128, 0.1);
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
  color: rgba(0, 0, 0, 0.72);
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
