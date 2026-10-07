<script setup lang="ts">
//! PDF 高保真预览：渲染 json2pdf unpack 产物（pages/*.json），悬浮显示 CLI 路径
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { ptToPx } from '@/core/units'
import { usePathHover } from '@/core/hover'
import PathCard from '@/components/PathCard.vue'
import PdfElementView from '@/renderers/pdf/PdfElementView.vue'
import { provideMediaResolver } from '@/renderers/pptx/media'
import {
  pdfElementContent,
  pdfElementType,
  pdfViewport,
  prepareElements,
  type PdfDoc,
  type PdfElement,
} from '@/renderers/pdf/types'

const props = withDefaults(
  defineProps<{
    /** loadPdf 产物（document.json + pages 内联） */
    doc: PdfDoc
    /** 媒体解析：media/xxx → 可加载 URL */
    resolveMedia?: (src: string) => string
    pageIndex?: number
    /** 显示缩略图导航（默认开） */
    showThumbnails?: boolean
  }>(),
  { pageIndex: 0, showThumbnails: true },
)

const emit = defineEmits<{
  (e: 'select', path: string): void
  (e: 'hover', path: string | null): void
  (e: 'page-change', index: number): void
}>()

provideMediaResolver(
  props.resolveMedia ?? ((src: string) => (src.startsWith('http') || src.startsWith('data:') ? src : `/api/media/${src}`)),
)

const hover = usePathHover()
const active = ref(props.pageIndex)
watch(
  () => props.pageIndex,
  (v) => (active.value = v),
)

const page = computed(() => props.doc.pages[active.value])
/** 可视区（打印态：crop 优先，其次 mediabox，缺省整页——与 json2pdf render 一致） */
const view = computed(() =>
  pdfViewport(page.value ?? {}, props.doc.page_size),
)
const pageW = computed(() => ptToPx(view.value.vw))
const pageH = computed(() => ptToPx(view.value.vh))
/** 逻辑尺寸（pt，crop 后）供渲染层换算 */
const pageWpt = computed(() => view.value.vw)
const pageHpt = computed(() => view.value.vh)
const vx0 = computed(() => view.value.vx0)
const vy0 = computed(() => view.value.vy0)

/** 用户缩放倍率（相对"适配宽度"的倍率） */
const zoom = ref(1)

const wrapRef = ref<HTMLElement | null>(null)
const wrapW = ref(0)
const wrapH = ref(0)
let ro: ResizeObserver | null = null

const fitScale = computed(() => {
  const availW = Math.max(120, wrapW.value - 48)
  const availH = Math.max(120, wrapH.value - 48)
  if (!wrapW.value || !wrapH.value) return 1
  return Math.min(1, availW / pageW.value, availH / pageH.value)
})
const scale = computed(() => Math.max(0.05, fitScale.value * zoom.value))

const stageBoxStyle = computed(() => ({
  width: `${Math.round(pageW.value * scale.value)}px`,
  height: `${Math.round(pageH.value * scale.value)}px`,
  position: 'relative' as const,
}))
const stageInnerStyle = computed(() => ({
  position: 'absolute' as const,
  left: '0',
  top: '0',
  width: `${pageW.value}px`,
  height: `${pageH.value}px`,
  transform: `scale(${scale.value})`,
  transformOrigin: 'top left',
}))

/** 缩略图：按侧栏宽度等比缩放真实渲染 */
const THUMB_W = 146
const thumbBoxStyle = computed(() => ({
  width: `${THUMB_W}px`,
  height: `${Math.round((THUMB_W * pageH.value) / pageW.value)}px`,
}))
const thumbSlideStyle = computed(() => ({
  position: 'absolute' as const,
  left: '0',
  top: '0',
  width: `${pageW.value}px`,
  height: `${pageH.value}px`,
  transform: `scale(${THUMB_W / pageW.value})`,
  transformOrigin: 'top left',
  pointerEvents: 'none' as const,
}))

onMounted(() => {
  if (!wrapRef.value) return
  const measure = () => {
    const el = wrapRef.value
    if (!el) return
    wrapW.value = el.clientWidth
    wrapH.value = el.clientHeight
  }
  measure()
  ro = new ResizeObserver(measure)
  ro.observe(wrapRef.value)
})
onBeforeUnmount(() => {
  ro?.disconnect()
  ro = null
})

/** 页面元素的 CLI 路径（/page[N]/type[K]，1 起按类型计数；group 内递归） */
function childPath(i: number, els: PdfElement[]): string {
  const t = pdfElementType(els[i])
  let k = 0
  for (let j = 0; j <= i; j += 1) if (pdfElementType(els[j]) === t) k += 1
  return `/${t}[${k}]`
}

function onHover(ev: MouseEvent, seg: string) {
  const path = `/page[${active.value + 1}]${seg}`
  // 沿路径找元素（供卡片摘要）
  const parts = seg
    .split('/')
    .filter((s) => s !== '')
    .map((s) => {
      const m = s.match(/^(\w+)\[(\d+)\]$/)
      return m ? { type: m[1], idx: Number(m[2]) } : null
    })
    .filter((x): x is { type: string; idx: number } => x !== null)
  let list: PdfElement[] = page.value?.elements ?? []
  let el: PdfElement | undefined
  for (const p of parts) {
    let k = 0
    el = undefined
    for (const e of list) {
      if (pdfElementType(e) === p.type) {
        k += 1
        if (k === p.idx) {
          el = e
          break
        }
      }
    }
    if (!el) break
    list = el.children ?? []
  }
  hover.enter(
    {
      path,
      type: el ? pdfElementType(el) : 'element',
      name: el?.src ?? undefined,
      text: el ? pdfElementContent(el) : undefined,
    },
    ev,
  )
  emit('hover', path)
}

function onLeave() {
  hover.leave()
  emit('hover', null)
}

function onSelect(seg: string) {
  emit('select', `/page[${active.value + 1}]${seg}`)
}

function thumbItems(p: PdfPageT) {
  return prepareElements(p.elements)
}
function thumbView(p: PdfPageT) {
  return pdfViewport(p, props.doc.page_size)
}

/** 缩略图文字：取该页第一个文本元素 */
function thumbLabel(p: PdfPageT): string {
  const first = p.elements.find((e) => e.type === 'text')
  const t = first?.text ?? ''
  return t.replace(/\s+/g, ' ').slice(0, 14) || '—'
}
type PdfPageT = PdfDoc['pages'][number]

function go(delta: number) {
  const next = Math.min(Math.max(active.value + delta, 0), props.doc.pages.length - 1)
  if (next !== active.value) {
    active.value = next
    emit('page-change', next)
  }
}

function onKey(ev: KeyboardEvent) {
  if (ev.key === 'ArrowRight' || ev.key === 'PageDown' || ev.key === ' ') go(1)
  else if (ev.key === 'ArrowLeft' || ev.key === 'PageUp') go(-1)
}

defineExpose({ hover, go })
</script>

<template>
  <div class="ov-pdf" tabindex="0" @keydown="onKey">
    <!-- 左侧：页面缩略图 -->
    <aside v-if="showThumbnails && doc.pages.length > 1" class="ov-pdf-side">
      <div class="ov-side-title">页面</div>
      <button
        v-for="(p, i) in doc.pages"
        :key="i"
        class="ov-thumb"
        :class="{ active: i === active }"
        @click="
          active = i;
          emit('page-change', i)
        "
      >
        <span class="ov-thumb-canvas" :style="thumbBoxStyle">
          <span class="ov-thumb-slide" :style="thumbSlideStyle">
            <template v-for="item in thumbItems(p)" :key="item.idx">
              <PdfElementView
                :el="item.kind === 'el' ? item.el : item.text"
                :page-w="thumbView(p).vw"
                :page-h="thumbView(p).vh"
                :vx0="thumbView(p).vx0"
                :vy0="thumbView(p).vy0"
                :pair-fill="item.kind === 'pair' ? item.fill : null"
                :hidden="item.kind === 'hidden'"
                :path="childPath(item.idx, p.elements)"
              />
            </template>
          </span>
        </span>
        <span class="ov-thumb-meta">
          <span class="ov-thumb-no">{{ i + 1 }}</span>
          <span class="ov-thumb-text">{{ thumbLabel(p) }}</span>
        </span>
      </button>
    </aside>

    <div class="ov-pdf-main">
      <div ref="wrapRef" class="ov-stage-wrap">
        <div class="ov-stage" :style="stageBoxStyle">
          <div class="ov-stage-inner" :style="stageInnerStyle">
            <template v-for="item in prepareElements(page?.elements ?? [])" :key="item.idx">
              <PdfElementView
                :el="item.kind === 'el' ? item.el : item.text"
                :page-w="pageWpt"
                :page-h="pageHpt"
                :vx0="vx0"
                :vy0="vy0"
                :pair-fill="item.kind === 'pair' ? item.fill : null"
                :hidden="item.kind === 'hidden'"
                :path="childPath(item.idx, page?.elements ?? [])"
                @element-hover="onHover"
                @element-move="hover.move($event)"
                @element-leave="onLeave"
                @element-mousedown="(_e: MouseEvent, p: string) => onSelect(p)"
              />
            </template>
          </div>
        </div>
      </div>

      <div class="ov-pdf-bar">
        <button class="ov-nav" :disabled="active === 0" @click="go(-1)">‹</button>
        <span class="ov-page-no">{{ active + 1 }} / {{ doc.pages.length }}</span>
        <button class="ov-nav" :disabled="active >= doc.pages.length - 1" @click="go(1)">›</button>
        <span class="ov-spacer"></span>
        <label class="ov-zoom">
          缩放
          <input v-model.number="zoom" type="range" min="0.5" max="2.2" step="0.05" />
          <span>{{ Math.round(scale * 100) }}%</span>
        </label>
      </div>
    </div>

    <PathCard :state="hover.state.value" />
  </div>
</template>

<style>
.ov-pdf {
  display: flex;
  align-items: stretch;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', 'Helvetica Neue',
    sans-serif;
  outline: none;
  border-radius: 12px;
  overflow: hidden;
  background: #fff;
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.08), 0 1px 3px rgba(0, 0, 0, 0.06);
}
.ov-pdf-side {
  flex: none;
  width: 178px;
  max-height: 72vh;
  overflow-y: auto;
  padding: 8px 8px 12px;
  background: rgba(246, 246, 246, 0.82);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  border-right: 0.5px solid rgba(0, 0, 0, 0.1);
}
.ov-pdf-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
.ov-pdf-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 12px;
  background: rgba(246, 246, 246, 0.82);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  border-top: 0.5px solid rgba(0, 0, 0, 0.1);
  font-size: 12px;
  color: rgba(0, 0, 0, 0.5);
}
.ov-side-title {
  padding: 4px 6px 6px;
  font-size: 11px;
  font-weight: 590;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  color: rgba(0, 0, 0, 0.42);
}
/* 缩略图与舞台（与 PptxViewer 同法；此处冗余声明保证组件单独可用） */
.ov-thumb {
  display: block;
  width: 100%;
  margin-bottom: 8px;
  padding: 4px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  cursor: default;
  font: inherit;
  text-align: left;
}
.ov-thumb:hover {
  background: rgba(0, 0, 0, 0.04);
}
.ov-thumb-canvas {
  position: relative;
  display: block;
  border-radius: 5px;
  background: #fff;
  overflow: hidden;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.14), 0 0 0 0.5px rgba(0, 0, 0, 0.08);
}
.ov-thumb-slide {
  position: absolute;
  inset: 0;
  display: block;
}
.ov-thumb.active .ov-thumb-canvas {
  box-shadow: 0 0 0 2px #007aff, 0 2px 8px rgba(0, 122, 255, 0.28);
}
.ov-thumb-meta {
  display: flex;
  align-items: baseline;
  gap: 5px;
  margin-top: 4px;
  padding: 0 2px;
}
.ov-thumb-no {
  flex: none;
  font-size: 10px;
  font-weight: 590;
  color: rgba(0, 0, 0, 0.55);
  font-variant-numeric: tabular-nums;
}
.ov-thumb-text {
  flex: 1;
  font-size: 10px;
  color: rgba(0, 0, 0, 0.42);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ov-stage {
  position: relative;
  flex: none;
  margin: auto;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.16), 0 0 0 0.5px rgba(0, 0, 0, 0.08);
  border-radius: 3px;
  overflow: hidden;
  background: #fff;
}
.ov-stage-inner {
  overflow: hidden;
}
.ov-stage-wrap {
  flex: none;
  display: flex;
  overflow: auto;
  height: min(72vh, 720px);
  padding: 24px;
  background: linear-gradient(180deg, #f3f3f5 0%, #ececf0 100%);
}
.ov-nav {
  appearance: none;
  width: 26px;
  height: 24px;
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 6px;
  background: #fff;
  color: rgba(0, 0, 0, 0.8);
  box-shadow: 0 0.5px 1px rgba(0, 0, 0, 0.08);
  cursor: default;
  font-size: 14px;
  line-height: 1;
}
.ov-nav:hover:not(:disabled) {
  background: #f7f7f7;
}
.ov-nav:disabled {
  color: rgba(0, 0, 0, 0.26);
  box-shadow: none;
}
.ov-page-no {
  font-variant-numeric: tabular-nums;
  color: rgba(0, 0, 0, 0.65);
  min-width: 46px;
  text-align: center;
}
.ov-spacer {
  flex: 1;
}
</style>
