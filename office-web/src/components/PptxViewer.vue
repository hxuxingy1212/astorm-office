<script setup lang="ts">
//! pptx 高保真预览：复用 ai-ppt/web 的元素渲染器（只读），悬浮显示 CLI 路径
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { inchToPx, setThemeColors } from '@/renderers/pptx/convert'
import { textContentPlain } from '@/renderers/pptx/text'
import ElementView from '@/renderers/pptx/elements/ElementView.vue'
import { provideMediaResolver } from '@/renderers/pptx/media'
import type { Element, Slide, PptxDoc } from '@/renderers/pptx/presentation'
import { usePathHover, type SelectInfo } from '@/core/hover'
import PathCard from '@/components/PathCard.vue'

const props = withDefaults(
  defineProps<{
    presentation: PptxDoc
    /** 媒体解析：xl/media/xxx → 可加载 URL（默认 /api/media/ 前缀） */
    resolveMedia?: (src: string) => string
    slideIndex?: number
    /** 显示缩略图导航（默认开） */
    showThumbnails?: boolean
  }>(),
  { slideIndex: 0, showThumbnails: true },
)

const emit = defineEmits<{
  (e: 'select', path: string, info?: SelectInfo): void
  (e: 'hover', path: string | null): void
  (e: 'slide-change', index: number): void
}>()

provideMediaResolver(
  props.resolveMedia ?? ((src: string) => (src.startsWith('http') || src.startsWith('data:') ? src : `/api/media/${src}`)),
)

const hover = usePathHover()
const active = ref(props.slideIndex)
watch(
  () => props.slideIndex,
  (v) => (active.value = v),
)

// 主题色注入（方案色 accent1/dk1/lt1 → 实际 hex）
setThemeColors((props.presentation.theme as { colors?: Record<string, string> } | undefined)?.colors)
watch(
  () => props.presentation.theme,
  (t) => setThemeColors((t as { colors?: Record<string, string> } | undefined)?.colors),
  { immediate: true },
)

const slide = computed<Slide | undefined>(() => props.presentation.slides[active.value])
/** 用户缩放倍率（相对"适配宽度"的倍率） */
const zoom = ref(1)

const slideW = computed(() => inchToPx(props.presentation.width || 13.333))
const slideH = computed(() => inchToPx(props.presentation.height || 7.5))

/** 舞台自适应：按容器宽高取"适配"倍率（1 为原始尺寸），避免内容溢出被裁切 */
const wrapRef = ref<HTMLElement | null>(null)
const wrapW = ref(0)
const wrapH = ref(0)
let ro: ResizeObserver | null = null

const fitScale = computed(() => {
  const availW = Math.max(120, wrapW.value - 48)
  const availH = Math.max(120, wrapH.value - 48)
  if (!wrapW.value || !wrapH.value) return 1
  return Math.min(1, availW / slideW.value, availH / slideH.value)
})
const scale = computed(() => Math.max(0.05, fitScale.value * zoom.value))

const stageBoxStyle = computed(() => ({
  width: `${Math.round(slideW.value * scale.value)}px`,
  height: `${Math.round(slideH.value * scale.value)}px`,
  position: 'relative' as const,
}))
const stageInnerStyle = computed(() => ({
  position: 'absolute' as const,
  left: '0',
  top: '0',
  width: `${slideW.value}px`,
  height: `${slideH.value}px`,
  transform: `scale(${scale.value})`,
  transformOrigin: 'top left',
}))

/** 缩略图：按侧栏宽度等比缩放真实渲染（与 ai-ppt/web 同法） */
const THUMB_W = 146
const thumbBoxStyle = computed(() => ({
  width: `${THUMB_W}px`,
  height: `${Math.round((THUMB_W * slideH.value) / slideW.value)}px`,
}))
const thumbSlideStyle = computed(() => ({
  position: 'absolute' as const,
  left: '0',
  top: '0',
  width: `${slideW.value}px`,
  height: `${slideH.value}px`,
  transform: `scale(${THUMB_W / slideW.value})`,
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

function backgroundCss(bg: unknown): string {
  if (!bg) return '#fff'
  if (typeof bg === 'string') return bg.startsWith('#') ? bg : `#${bg}`
  // 兼容两种形态：嵌套 { gradient: { stops, angle } } 与扁平 { type: 'gradient', stops, angle }
  // （后者来自 LibreOffice 引擎的转换产物）；position 兼容 0-1 与 0-100。
  const b = bg as {
    color?: string
    type?: string
    angle?: number
    stops?: { color: string; position: number }[]
    gradient?: { stops?: { color: string; position: number }[]; angle?: number }
    image?: { src?: string }
  }
  if (b.color) return b.color.startsWith('#') ? b.color : `#${b.color}`
  const grad = b.gradient?.stops?.length ? b.gradient : b.stops?.length ? { stops: b.stops, angle: b.angle } : undefined
  if (grad?.stops?.length) {
    const stops = grad.stops
      .map((s) => {
        const c = s.color.startsWith('#') ? s.color : `#${s.color}`
        const pct = s.position <= 1 ? s.position * 100 : s.position
        return `${c} ${Math.round(pct)}%`
      })
      .join(', ')
    const angle = grad.angle ?? 90
    return `linear-gradient(${angle}deg, ${stops})`
  }
  if (b.image?.src) return `url("${props.resolveMedia ? props.resolveMedia(b.image.src) : b.image.src}") center/cover`
  return '#fff'
}

/** 数字索引路径（"2.1"）→ CLI 路径（/slide[1]/text[2]） */
function cliPath(numPath: string): string {
  const segs = numPath.split('.').filter((s) => s !== '').map(Number)
  const containers: string[][] = []
  let current: Element[] = slide.value?.elements ?? []
  for (const idx of segs) {
    containers.push(current.map((e) => e.type))
    const el = current[idx]
    if (!el) break
    current = el.type === 'group' ? (el as unknown as { children: Element[] }).children : []
  }
  const nodes: string[] = [`slide[${active.value + 1}]`]
  segs.forEach((idx, depth) => {
    const types = containers[depth] ?? []
    const type = types[idx]
    if (!type) return
    let k = 0
    for (let j = 0; j <= idx; j += 1) if (types[j] === type) k += 1
    nodes.push(`${type}[${k}]`)
  })
  return `/${nodes.join('/')}`
}

function elementAt(numPath: string): Element | undefined {
  const segs = numPath.split('.').filter((s) => s !== '').map(Number)
  let list: Element[] = slide.value?.elements ?? []
  let el: Element | undefined
  for (const idx of segs) {
    el = list[idx]
    if (!el) return undefined
    list = el.type === 'group' ? (el as unknown as { children: Element[] }).children : []
  }
  return el
}

/** 元素内容摘要：string 直取；Paragraph[] 拼出纯文本（runs 串联、段落换行），卡片内容行用 */
function elementContent(el: Element | undefined): string | undefined {
  const t = (el as { text?: unknown } | undefined)?.text
  if (typeof t === 'string') return t || undefined
  if (!Array.isArray(t)) return undefined
  const lines = t.map((p: any) =>
    typeof p === 'string'
      ? p
      : Array.isArray(p?.runs)
        ? (p.runs as { text?: string }[]).map((r) => r.text ?? '').join('')
        : (p?.text ?? ''),
  )
  const s = lines.filter(Boolean).join('\n')
  return s ? s.slice(0, 160) : undefined
}

function onHover(ev: MouseEvent, numPath: string) {
  const el = elementAt(numPath)
  const path = cliPath(numPath)
  hover.enter(
    {
      path,
      type: el?.type ?? 'element',
      name: (el as { name?: string } | undefined)?.name ?? undefined,
      text: elementContent(el),
    },
    ev,
  )
  emit('hover', path)
}

function onLeave() {
  hover.leave()
  emit('hover', null)
}

function onSelect(numPath: string) {
  const el = elementAt(numPath)
  const path = cliPath(numPath)
  emit('select', path, {
    path,
    type: el?.type ?? 'element',
    name: (el as { name?: string } | undefined)?.name ?? undefined,
    text: elementContent(el),
  })
}

/** 缩略图文字：取该页第一个文本元素的前 12 字 */
function thumbLabel(s: Slide): string {
  const first = s.elements.find((e) => e.type === 'text') as { text?: unknown } | undefined
  if (!first?.text) return '—'
  const plain = textContentPlain(first.text as Parameters<typeof textContentPlain>[0])
  return plain.replace(/\s+/g, ' ').slice(0, 14) || '—'
}

function go(delta: number) {
  const next = Math.min(Math.max(active.value + delta, 0), props.presentation.slides.length - 1)
  if (next !== active.value) {
    active.value = next
    emit('slide-change', next)
  }
}

function onKey(ev: KeyboardEvent) {
  if (ev.key === 'ArrowRight' || ev.key === 'PageDown' || ev.key === ' ') go(1)
  else if (ev.key === 'ArrowLeft' || ev.key === 'PageUp') go(-1)
}

defineExpose({ hover, go })
</script>

<template>
  <div class="ov-pptx" tabindex="0" @keydown="onKey">
    <!-- 左侧：缩略图大纲 -->
    <aside v-if="showThumbnails && presentation.slides.length > 1" class="ov-pptx-side">
      <div class="ov-side-title">幻灯片</div>
      <button
        v-for="(s, i) in presentation.slides"
        :key="i"
        class="ov-thumb"
        :class="{ active: i === active }"
        @click="
          active = i;
          emit('slide-change', i)
        "
      >
        <span class="ov-thumb-canvas" :style="thumbBoxStyle">
          <!-- 缩略图用与舞台相同的渲染器真实渲染后缩放（与 ai-ppt/web 一致） -->
          <span class="ov-thumb-slide" :style="thumbSlideStyle">
            <span class="ov-thumb-bg" :style="{ background: backgroundCss(s.background) }"></span>
            <ElementView
              v-for="(el, ei) in s.elements"
              :key="ei"
              :el="el"
              :path="String(ei)"
              mode="view"
            />
          </span>
        </span>
        <span class="ov-thumb-meta">
          <span class="ov-thumb-no">{{ i + 1 }}</span>
          <span class="ov-thumb-text">{{ thumbLabel(s) }}</span>
        </span>
      </button>
    </aside>

    <div class="ov-pptx-main">
    <div ref="wrapRef" class="ov-stage-wrap">
      <!-- 外层占位 = 缩放后尺寸（保证滚动条正确），内层按逻辑尺寸渲染后整体缩放 -->
      <div class="ov-stage" :style="stageBoxStyle">
        <div class="ov-stage-inner" :style="stageInnerStyle">
          <span class="ov-stage-bg" :style="{ background: backgroundCss(slide?.background) }"></span>
          <ElementView
            v-for="(el, i) in slide?.elements ?? []"
            :key="i"
            :el="el"
            :path="String(i)"
            mode="view"
            @element-hover="onHover"
            @element-move="hover.move($event)"
            @element-leave="onLeave"
            @element-mousedown="(_e: MouseEvent, p: string) => onSelect(p)"
          />
        </div>
      </div>
    </div>

    <div class="ov-pptx-bar">
      <button class="ov-nav" :disabled="active === 0" @click="go(-1)">‹</button>
      <span class="ov-page-no">{{ active + 1 }} / {{ presentation.slides.length }}</span>
      <button class="ov-nav" :disabled="active >= presentation.slides.length - 1" @click="go(1)">›</button>
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
.ov-pptx {
  display: flex;
  align-items: stretch;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', 'Helvetica Neue',
    sans-serif;
  outline: none;
  border-radius: 12px;
  overflow: hidden;
  background: var(--ov-page-bg);
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.08), 0 1px 3px rgba(0, 0, 0, 0.06);
}
/* 左侧缩略图大纲（毛玻璃侧栏） */
.ov-pptx-side {
  flex: none;
  width: 178px;
  max-height: 72vh;
  overflow-y: auto;
  padding: 8px 8px 12px;
  background: var(--ov-bg-toolbar);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  border-right: 0.5px solid var(--ov-separator);
}
.ov-side-title {
  padding: 4px 6px 6px;
  font-size: 11px;
  font-weight: 590;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  color: var(--ov-label-2);
}
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
  background: var(--ov-bg-fill);
}
.ov-thumb-canvas {
  position: relative;
  display: block;
  border-radius: 5px;
  background: var(--ov-page-bg);
  overflow: hidden;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.14), 0 0 0 0.5px rgba(0, 0, 0, 0.08);
}
.ov-thumb-slide,
.ov-thumb-bg {
  position: absolute;
  inset: 0;
  display: block;
}
.ov-thumb-bg {
  z-index: 0;
}
.ov-thumb.active .ov-thumb-canvas {
  box-shadow: 0 0 0 2px var(--ov-accent), 0 2px 8px rgba(0, 122, 255, 0.28);
}
.ov-stage {
  position: relative;
  flex: none; /* 关键：不参与 flex 压缩，放大后由容器滚动而不是被裁 */
  margin: auto; /* 水平 + 垂直居中；内容超出容器时仍可滚动到左上起点 */
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.16), 0 0 0 0.5px rgba(0, 0, 0, 0.08);
  border-radius: 3px;
  overflow: hidden;
}
.ov-stage-inner {
  overflow: hidden;
}
.ov-stage-bg {
  position: absolute;
  inset: 0;
  z-index: 0;
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
  color: var(--ov-label-2);
  font-variant-numeric: tabular-nums;
}
.ov-thumb-text {
  flex: 1;
  font-size: 10px;
  color: var(--ov-label-2);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.ov-pptx-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}
/* 舞台：Keynote 式浅灰画布（内容水平+垂直居中） */
.ov-stage-wrap {
  flex: none;
  display: flex; /* 配合 .ov-stage 的 margin:auto 实现双向居中，且溢出时仍可滚到起点 */
  overflow: auto;
  height: min(72vh, 720px);
  padding: 24px;
  background: var(--ov-bg-stage);
}
/* 工具条 */
.ov-pptx-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 12px;
  background: var(--ov-bg-toolbar);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  border-top: 0.5px solid var(--ov-separator);
  font-size: 12px;
  color: var(--ov-label-2);
}
.ov-nav {
  appearance: none;
  width: 26px;
  height: 24px;
  border: 1px solid var(--ov-separator);
  border-radius: 6px;
  background: var(--ov-bg-control);
  color: var(--ov-label);
  box-shadow: 0 0.5px 1px rgba(0, 0, 0, 0.08);
  cursor: default;
  font-size: 14px;
  line-height: 1;
}
.ov-nav:hover:not(:disabled) {
  background: var(--ov-bg-control-hover);
}
.ov-nav:disabled {
  color: var(--ov-label-3);
  box-shadow: none;
}
.ov-page-no {
  font-variant-numeric: tabular-nums;
  color: var(--ov-label-2);
  min-width: 46px;
  text-align: center;
}
.ov-spacer {
  flex: 1;
}
</style>
