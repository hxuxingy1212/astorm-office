<script setup lang="ts">
//! docx 高保真预览：按分片渲染文档页（纸张/页边距/样式/块级元素），悬浮显示 CLI 路径
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { DocxBlock, DocxDocument, DocxPart, DocxRun, DocxSection, DocxStyle } from '@/types'
import { ptToPx, cssColor } from '@/core/units'
import { usePathHover, type PathCardInfo } from '@/core/hover'
import PathCard from '@/components/PathCard.vue'
import { latexToHtml, LATEX_CSS } from '@/renderers/docx/latex'

const props = withDefaults(
  defineProps<{
    document: DocxDocument
    /** 媒体解析：把 word/media/xxx 解析成可加载 URL */
    resolveMedia?: (src: string) => string
    /** 页码格式：{page} 占位（默认读取 section.footer_format） */
    pageFormat?: string
    partIndex?: number
  }>(),
  { partIndex: 0 },
)

const emit = defineEmits<{
  (e: 'select', path: string): void
  (e: 'hover', path: string | null): void
  (e: 'page-change', index: number): void
}>()

const hover = usePathHover()
const active = ref(props.partIndex)
watch(
  () => props.partIndex,
  (v) => (active.value = v),
)

const part = computed<DocxPart | undefined>(() => props.document.parts[active.value])

const PAGE_SIZES: Record<string, [number, number]> = {
  A4: [8.27, 11.69],
  A3: [11.69, 16.54],
  A5: [5.83, 8.27],
  Letter: [8.5, 11],
  Legal: [8.5, 14],
}

/** 缩放：适配可视区（默认整体缩小到全部可见），滑杆在适配基础上放大倍率 */
const wrapRef = ref<HTMLElement | null>(null)
const zoom = ref(1)
const wrapW = ref(0)
const wrapH = ref(0)
const naturalH = ref(0)
let roWrap: ResizeObserver | null = null
let roPage: ResizeObserver | null = null

const pageNaturalW = ref(794)

const pageBox = computed(() => {
  const p = (part.value?.section?.page ?? props.document.page) as
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
    width: `${Math.round(pw * 96)}px`,
    minHeight: `${Math.round(ph * 96)}px`,
    padding: `${Math.round(ptToPx(pt(m.top, 72)))}px ${Math.round(ptToPx(pt(m.right, 90)))}px ${Math.round(
      ptToPx(pt(m.bottom, 72)),
    )}px ${Math.round(ptToPx(pt(m.left, 90)))}px`,
    paperW: Math.round(pw * 96),
    paperH: Math.round(ph * 96),
  }
})

/** 页面自然尺寸（宽 = 纸张宽；高 = 纸张高与内容高的较大者） */
const naturalW = computed(() => pageBox.value.paperW)
const naturalTotalH = computed(() => Math.max(pageBox.value.paperH, naturalH.value))

/** 适配倍率：使整页（含全部内容）落在可视区内 */
const fitScale = computed(() => {
  const availW = Math.max(80, wrapW.value - 32)
  const availH = Math.max(80, wrapH.value - 32)
  if (!wrapW.value || !wrapH.value || !naturalH.value) return 1
  const fitW = Math.min(1, availW / naturalW.value)
  const fitH = availH / naturalTotalH.value
  // 内容极端超高（整本文档被并成一个分片）时按高度适配会缩到不可读：
  // 高度适配低于宽度适配的 1/3 时退化为按宽适配，超出部分纵向滚动
  return Math.min(fitW, Math.max(fitH, fitW * 0.3))
})
const scale = computed(() => Math.max(0.05, fitScale.value * zoom.value))

const pageStyle = computed(() => {
  const box = pageBox.value
  return {
    ...box,
    width: `${naturalW.value}px`,
    boxSizing: 'border-box' as const,
    transform: `scale(${scale.value})`,
    transformOrigin: 'top left',
    margin: '0',
    maxHeight: 'none',
    overflow: 'visible',
  }
})
const pageBoxStyle = computed(() => ({
  width: `${Math.round(naturalW.value * scale.value)}px`,
  height: `${Math.round(naturalTotalH.value * scale.value)}px`,
  position: 'relative' as const,
  margin: 'auto' as const,
  flex: 'none' as const,
}))

function measure() {
  if (pageRef.value) {
    pageNaturalW.value = pageRef.value.offsetWidth || pageNaturalW.value
    naturalH.value = pageRef.value.scrollHeight
  }
}

onMounted(() => {
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
  if (pageRef.value) {
    roPage = new ResizeObserver(measure)
    roPage.observe(pageRef.value)
  }
  measure()
})
onBeforeUnmount(() => {
  roWrap?.disconnect()
  roPage?.disconnect()
  roWrap = null
  roPage = null
})
watch(active, () => nextTick(measure))
watch(pageBox, () => nextTick(measure))

/** 命名样式：递归解析 based_on */
function styleOf(name?: string | null, depth = 0): DocxStyle | undefined {
  if (!name || depth > 4) return undefined
  const s = props.document.styles?.[name]
  if (!s) return undefined
  const base = styleOf(s.based_on ?? undefined, depth + 1)
  return base ? { ...base, ...stripUndefined(s) } : s
}

function stripUndefined<T extends object>(o: T): T {
  const out: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(o)) if (v !== undefined && v !== null) out[k] = v
  return out as T
}

/** 块级样式（段落/标题共用） */
function blockStyle(b: DocxBlock): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  const named = styleOf(typeof anyB.style === 'string' ? (anyB.style as string) : undefined)
  const out: Record<string, string> = {}
  const fontFamily = (anyB.font_family as string) ?? named?.font
  if (fontFamily) out['font-family'] = `${fontFamily}, SimSun, serif`
  const size = (anyB.font_size as number) ?? named?.size
  if (size) out['font-size'] = `${ptToPx(size)}px`
  const bold = (anyB.bold as boolean) ?? named?.bold
  if (bold) out['font-weight'] = 'bold'
  const italic = (anyB.italic as boolean) ?? named?.italic
  if (italic) out['font-style'] = 'italic'
  const color = (anyB.color as string) ?? named?.color
  if (color) out['color'] = cssColor(color)
  const align = (anyB.align as string) ?? named?.align
  if (align) out['text-align'] = align === 'both' ? 'justify' : align
  const ls = (anyB.line_spacing as number) ?? named?.line_spacing
  if (ls) out['line-height'] = String(ls)
  const sb = (anyB.space_before as number) ?? named?.space_before
  if (sb) out['margin-top'] = `${ptToPx(sb)}px`
  const sa = (anyB.space_after as number) ?? named?.space_after
  if (sa) out['margin-bottom'] = `${ptToPx(sa)}px`
  const fi = (anyB.first_line_indent as number) ?? named?.first_line_indent
  if (fi) out['text-indent'] = `${ptToPx(fi)}px`
  const indent = (anyB.indent as number) ?? named?.indent
  if (indent) out['margin-left'] = `${ptToPx(indent)}px`
  const hanging = anyB.hanging_indent as number
  if (hanging) out['padding-left'] = `${ptToPx(hanging)}px`
  const shading = anyB.shading as string
  if (shading) out['background-color'] = cssColor(shading)
  if (anyB.page_break_before) out['break-before'] = 'page'
  return out
}

/** 图片块对齐样式 */
function figureStyle(b: DocxBlock): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  return { textAlign: (anyB.align as string) ?? 'center' }
}

/** 形状/文本框样式 */
function shapeStyle(b: DocxBlock): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  const out: Record<string, string> = { ...blockStyle(b) }
  if (anyB.fill) out['background'] = cssColor(anyB.fill as string)
  if (anyB.line) out['border'] = `1px solid ${cssColor(anyB.line as string)}`
  return out
}

function runStyle(r: DocxRun): Record<string, string> {
  const out: Record<string, string> = {}
  if (r.bold) out['font-weight'] = 'bold'
  if (r.italic) out['font-style'] = 'italic'
  const deco: string[] = []
  if (r.underline) deco.push('underline')
  if (r.strike) deco.push('line-through')
  if (deco.length) out['text-decoration'] = deco.join(' ')
  if (r.color) out['color'] = cssColor(r.color)
  if (r.font_size) out['font-size'] = `${ptToPx(r.font_size)}px`
  if (r.font_family) out['font-family'] = `${r.font_family}, SimSun, serif`
  if (r.highlight) out['background-color'] = cssColor(r.highlight)
  if (r.code) out['font-family'] = 'ui-monospace, Menlo, monospace'
  if (r.vert_align === 'superscript') out['vertical-align'] = 'super'
  if (r.vert_align === 'subscript') out['vertical-align'] = 'sub'
  return out
}

const HEADING_SCALE: Record<number, number> = { 1: 22, 2: 18, 3: 15.5, 4: 14, 5: 13, 6: 12.5 }

function headingStyle(b: DocxBlock): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  const level = Number(anyB.level ?? 1)
  const named = styleOf(`Heading${level}`) ?? styleOf('Heading')
  const out = blockStyle(b)
  if (!out['font-size']) out['font-size'] = `${ptToPx(HEADING_SCALE[level] ?? 14)}px`
  if (!out['font-weight']) out['font-weight'] = 'bold'
  if (!out['margin-top']) out['margin-top'] = level === 1 ? '18px' : '12px'
  if (!out['margin-bottom']) out['margin-bottom'] = '8px'
  if (!out['color'] && named?.color) out['color'] = cssColor(named.color)
  return out
}

/** 块路径：容器链（类型[序号]） */
function blockPath(chain: string[]): string {
  return `/part[${active.value + 1}]${chain.map((s) => `/${s}`).join('')}`
}

/** 计算同类型序号（容器内） */
function ordinals(blocks: DocxBlock[]): number[] {
  const seen: Record<string, number> = {}
  return blocks.map((b) => {
    seen[b.type] = (seen[b.type] ?? 0) + 1
    return seen[b.type]
  })
}

function blockInfo(b: DocxBlock, path: string): PathCardInfo {
  const anyB = b as unknown as Record<string, unknown>
  const text =
    (anyB.text as string) ??
    (Array.isArray(anyB.runs)
      ? (anyB.runs as { text?: string }[])
          .map((r) => r.text ?? '')
          .join('')
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

function enterBlock(b: DocxBlock, path: string, ev: MouseEvent) {
  hover.enter(blockInfo(b, path), ev)
  emit('hover', path)
}
function leave() {
  hover.leave()
  emit('hover', null)
}
function clickBlock(path: string) {
  emit('select', path)
}

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

/** 目录点击：必要时切换分片，然后滚动到对应标题 */
async function scrollToBlock(entry: { partIndex: number; bi: number }) {
  if (entry.partIndex !== active.value) {
    active.value = entry.partIndex
    emit('page-change', entry.partIndex)
    await nextTick()
  }
  const root = pageRef.value
  if (!root) return
  const el = root.querySelector(`[data-bi="${entry.bi}"]`)
  if (el instanceof HTMLElement) el.scrollIntoView({ behavior: 'smooth', block: 'start' })
}

/** 上下翻页（分片） */
function go(delta: number) {
  const next = Math.min(Math.max(active.value + delta, 0), props.document.parts.length - 1)
  if (next !== active.value) {
    active.value = next
    emit('page-change', next)
  }
}

function onPageKey(ev: KeyboardEvent) {
  if (ev.key === 'ArrowDown' || ev.key === 'PageDown') {
    active.value = Math.min(active.value + 1, props.document.parts.length - 1)
  } else if (ev.key === 'ArrowUp' || ev.key === 'PageUp') {
    active.value = Math.max(active.value - 1, 0)
  }
}

const headerText = computed(() => part.value?.section?.header ?? '')
const footerText = computed(() => {
  const s = part.value?.section
  const fmt = props.pageFormat ?? s?.footer_format
  if (fmt) return fmt.replace('{page}', String(active.value + 1))
  return s?.footer_page_number ? String(active.value + 1) : ''
})

const LATEX_CSS_INJECTED = ref(false)
if (typeof document !== 'undefined' && !document.getElementById('ov-latex-css')) {
  const style = document.createElement('style')
  style.id = 'ov-latex-css'
  style.textContent = LATEX_CSS
  document.head.appendChild(style)
  LATEX_CSS_INJECTED.value = true
}

defineExpose({ hover })
</script>

<template>
  <div class="ov-docx">
    <!-- 左侧：目录（当前分片标题）+ 上下翻页 -->
    <aside class="ov-docx-side">
      <div class="ov-side-title">目录</div>
      <nav class="ov-outline">
        <template v-for="h in outline" :key="`${h.partIndex}-${h.bi}`">
          <div v-if="h.showGroup" class="ov-outline-part">{{ h.group }}</div>
          <button
            class="ov-outline-item"
            :class="{ current: h.partIndex === active }"
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
        <button class="ov-btn" :disabled="active <= 0" title="上一分片" @click="go(-1)">▲</button>
        <span class="ov-page-no">{{ active + 1 }} / {{ document.parts.length }}</span>
        <button
          class="ov-btn"
          :disabled="active >= document.parts.length - 1"
          title="下一分片"
          @click="go(1)"
        >
          ▼
        </button>
      </div>
    </aside>

    <div class="ov-docx-main">
    <div ref="wrapRef" class="ov-docx-canvas">
      <div class="ov-page-box" :style="pageBoxStyle">
        <div ref="pageRef" class="ov-page" :style="pageStyle" @keydown="onPageKey" tabindex="0">
      <div v-if="headerText" class="ov-page-header">{{ headerText }}</div>

      <template v-for="(b, bi) in part?.blocks ?? []" :key="bi">
        <!-- 标题 -->
        <component
          :is="`h${Math.min(6, Math.max(1, Number((b as any).level ?? 1)))}`"
          v-if="b.type === 'heading'"
          class="ov-heading"
          :style="headingStyle(b) as any"
          @mouseenter="enterBlock(b, blockPath([`heading[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
          @click="clickBlock(blockPath([`heading[${ordinals(part!.blocks)[bi]}]`]))"
        >
          <template v-if="(b as any).numbering">{{ ordinals(part!.blocks)[bi] }}. </template
          >{{ (b as any).text }}
        </component>

        <!-- 段落 -->
        <p
          v-else-if="b.type === 'paragraph'"
          class="ov-para"
          :style="blockStyle(b)"
          @mouseenter="enterBlock(b, blockPath([`paragraph[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
          @click="clickBlock(blockPath([`paragraph[${ordinals(part!.blocks)[bi]}]`]))"
        >
          <template v-if="(b as any).runs?.length">
            <span v-for="(r, ri) in (b as any).runs as DocxRun[]" :key="ri" :style="runStyle(r)">{{
              r.text
            }}</span>
          </template>
          <template v-else>{{ (b as any).text ?? '' }}</template>
        </p>

        <!-- 列表 -->
        <component
          :is="(b as any).ordered ? 'ol' : 'ul'"
          v-else-if="b.type === 'list'"
          class="ov-list"
          :style="blockStyle(b)"
          @mouseenter="enterBlock(b, blockPath([`list[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
        >
          <li v-for="(item, ii) in (b as any).items ?? []" :key="ii">
            <template v-for="ib in item.blocks ?? []" :key="String(ib.type)">
              <span
                @mouseenter="
                  enterBlock(
                    ib,
                    blockPath([
                      `list[${ordinals(part!.blocks)[bi]}]`,
                      `item[${ii + 1}]`,
                      `${ib.type}[1]`,
                    ]),
                    $event,
                  )
                "
                @mousemove="hover.move($event)"
                @mouseleave="leave"
                >{{ (ib as any).text ?? '' }}</span
              >
            </template>
          </li>
        </component>

        <!-- 表格 -->
        <table
          v-else-if="b.type === 'table'"
          class="ov-table"
          @mouseenter="enterBlock(b, blockPath([`table[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
        >
          <tbody>
            <tr
              v-for="(row, ri) in (b as any).rows ?? []"
              :key="ri"
              :class="{ 'ov-tr-head': (b as any).header_row && ri === 0 }"
              @mouseenter="
                enterBlock(
                  row,
                  blockPath([
                    `table[${ordinals(part!.blocks)[bi]}]`,
                    `row[${ri + 1}]`,
                  ]),
                  $event,
                )
              "
              @mousemove="hover.move($event)"
              @mouseleave="leave"
            >
              <td
                v-for="(cell, ci) in row.cells ?? []"
                :key="ci"
                :colspan="cell.colspan ?? 1"
                :style="{
                  textAlign: cell.align ?? undefined,
                  width: (b as any).column_widths?.[ci]
                    ? `${Math.round((b as any).column_widths[ci] * 96)}px`
                    : undefined,
                }"
                @mouseenter.stop="
                  enterBlock(
                    cell,
                    blockPath([
                      `table[${ordinals(part!.blocks)[bi]}]`,
                      `row[${ri + 1}]`,
                      `cell[${ci + 1}]`,
                    ]),
                    $event,
                  )
                "
                @mousemove="hover.move($event)"
                @mouseleave="leave"
              >
                <template v-for="cb in cell.blocks ?? []" :key="String(cb.type)">
                  <span
                    @mouseenter.stop="
                      enterBlock(
                        cb,
                        blockPath([
                          `table[${ordinals(part!.blocks)[bi]}]`,
                          `row[${ri + 1}]`,
                          `cell[${ci + 1}]`,
                          `${cb.type}[1]`,
                        ]),
                        $event,
                      )
                    "
                    @mousemove="hover.move($event)"
                    @mouseleave="leave"
                    >{{ (cb as any).text ?? '' }}</span
                  >
                </template>
              </td>
            </tr>
          </tbody>
        </table>

        <!-- 图片 -->
        <figure
          v-else-if="b.type === 'image'"
          class="ov-figure"
          :style="figureStyle(b)"
          @mouseenter="enterBlock(b, blockPath([`image[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
        >
          <img
            :src="resolveMedia ? resolveMedia((b as any).src) : (b as any).src"
            :style="{
              width: (b as any).width ? `${Math.round((b as any).width * 96)}px` : undefined,
              maxWidth: '100%',
            }"
            :alt="(b as any).alt ?? ''"
          />
          <figcaption v-if="(b as any).caption" class="ov-caption">
            {{ (b as any).caption }}
          </figcaption>
        </figure>

        <!-- 公式 -->
        <div
          v-else-if="b.type === 'formula'"
          class="ov-formula"
          :class="{ display: (b as any).display !== false }"
          @mouseenter="enterBlock(b, blockPath([`formula[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
        >
          <span v-html="latexToHtml((b as any).latex ?? '')"></span>
          <span v-if="(b as any).number" class="ov-eqno">（{{ (b as any).number }}）</span>
        </div>

        <!-- 代码 -->
        <pre
          v-else-if="b.type === 'code'"
          class="ov-code"
          @mouseenter="enterBlock(b, blockPath([`code[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
          >{{ (b as any).text }}</pre
        >

        <!-- 引用 -->
        <blockquote
          v-else-if="b.type === 'quote'"
          class="ov-quote"
          @mouseenter="enterBlock(b, blockPath([`quote[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
        >
          {{ (b as any).text }}
        </blockquote>

        <!-- 题注 -->
        <div
          v-else-if="b.type === 'caption'"
          class="ov-caption-block"
          @mouseenter="enterBlock(b, blockPath([`caption[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
        >
          {{ (b as any).text }}
        </div>

        <!-- 目录 -->
        <div
          v-else-if="b.type === 'toc'"
          class="ov-toc"
          @mouseenter="enterBlock(b, blockPath([`toc[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
        >
          <div class="ov-toc-title">{{ (b as any).title || '目录' }}</div>
          <div
            v-for="(e, ei) in (b as any).entries ?? []"
            :key="ei"
            class="ov-toc-item"
            :style="{ paddingLeft: `${(e.level - 1) * 16}px` }"
          >
            {{ e.text }}
          </div>
        </div>

        <!-- 参考文献 -->
        <div
          v-else-if="b.type === 'bibliography'"
          class="ov-bib"
          @mouseenter="enterBlock(b, blockPath([`bibliography[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
        >
          <div class="ov-bib-title">{{ (b as any).title || '参考文献' }}</div>
          <div v-for="(e, ei) in (b as any).entries ?? []" :key="ei" class="ov-bib-item">
            [{{ ei + 1 }}] {{ (e.authors ?? []).join(', ') }}. {{ e.title }}.
            <em v-if="e.container">{{ e.container }}</em
            ><template v-if="e.year">, {{ e.year }}</template>.
          </div>
        </div>

        <!-- 形状 / 文本框 -->
        <div
          v-else-if="b.type === 'shape' || b.type === 'textbox'"
          class="ov-shape"
          :style="shapeStyle(b)"
          @mouseenter="enterBlock(b, blockPath([`${b.type}[${ordinals(part!.blocks)[bi]}]`]), $event)"
          @mousemove="hover.move($event)"
          @mouseleave="leave"
        >
          {{ (b as any).text }}
        </div>

        <!-- 分页符 -->
        <hr v-else-if="b.type === 'page_break'" class="ov-pagebreak" />

        <!-- 未知块：原样提示（保证不丢信息） -->
        <div v-else class="ov-unknown">［{{ b.type }}］</div>
      </template>

          <div v-if="footerText" class="ov-page-footer">{{ footerText }}</div>
        </div>
      </div>
    </div>

    <div class="ov-docx-bar">
      <span>缩放</span>
      <input v-model.number="zoom" type="range" min="0.5" max="3" step="0.05" />
      <span class="ov-zoom-val">{{ Math.round(scale * 100) }}%</span>
    </div>
    </div>

    <PathCard :state="hover.state.value" />
  </div>
</template>

<style>
.ov-docx {
  display: flex;
  align-items: stretch;
  gap: 14px;
  font-family: 'Songti SC', 'SimSun', 'Times New Roman', serif;
  color: #1d1d1f;
}
/* 左侧：目录 + 翻页（毛玻璃侧栏） */
.ov-docx-side {
  flex: none;
  width: 208px;
  display: flex;
  flex-direction: column;
  height: min(76vh, 900px);
  border-radius: 12px;
  background: rgba(246, 246, 246, 0.78);
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
  color: rgba(0, 0, 0, 0.42);
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
  color: rgba(0, 0, 0, 0.8);
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 12px;
  text-align: left;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: default;
}
.ov-outline-item:hover {
  background: rgba(0, 0, 0, 0.05);
}
.ov-outline-item.current {
  color: rgba(0, 0, 0, 0.92);
  font-weight: 500;
}
.ov-outline-part {
  padding: 8px 8px 2px;
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 10.5px;
  font-weight: 590;
  letter-spacing: 0.02em;
  color: rgba(0, 0, 0, 0.34);
  text-transform: uppercase;
}
.ov-outline-item:active {
  background: rgba(0, 122, 255, 0.14);
  color: #0a6cd8;
}
.ov-outline-empty {
  padding: 6px 8px;
  font-size: 11.5px;
  color: rgba(0, 0, 0, 0.3);
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', sans-serif;
}
/* 上下翻页按钮 */
.ov-pager {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 8px 10px;
  border-top: 0.5px solid rgba(0, 0, 0, 0.1);
  background: rgba(246, 246, 246, 0.9);
}
.ov-btn {
  appearance: none;
  min-width: 30px;
  height: 24px;
  padding: 0 8px;
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 6px;
  background: #fff;
  color: rgba(0, 0, 0, 0.8);
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', sans-serif;
  font-size: 11px;
  line-height: 1;
  box-shadow: 0 0.5px 1px rgba(0, 0, 0, 0.08);
  cursor: default;
}
.ov-btn:hover:not(:disabled) {
  background: #f7f7f7;
}
.ov-btn:disabled {
  color: rgba(0, 0, 0, 0.26);
  box-shadow: none;
}
.ov-page-no {
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', sans-serif;
  font-size: 11.5px;
  color: rgba(0, 0, 0, 0.55);
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
  background: linear-gradient(180deg, #f3f3f5 0%, #ececf0 100%);
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.08), 0 1px 3px rgba(0, 0, 0, 0.06);
}
/* 纸张（自然尺寸渲染，外层容器按缩放后尺寸占位） */
.ov-page {
  position: relative;
  box-sizing: border-box;
  background: #fff;
  border-radius: 4px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.14), 0 0 0 0.5px rgba(0, 0, 0, 0.08);
  line-height: 1.75;
  font-size: 15px;
}
.ov-page-box {
  overflow: hidden;
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
  color: rgba(0, 0, 0, 0.5);
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
  background: #fff;
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
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', sans-serif;
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
  color: rgba(0, 0, 0, 0.5);
  font-size: 13px;
  text-align: center;
  font-family: -apple-system, BlinkMacSystemFont, 'PingFang SC', sans-serif;
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
  color: rgba(0, 0, 0, 0.5);
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
  color: rgba(0, 0, 0, 0.6);
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
.ov-pagebreak {
  border: none;
  border-top: 1px dashed rgba(0, 0, 0, 0.2);
  margin: 18px 0;
}
.ov-unknown {
  color: rgba(0, 0, 0, 0.26);
  font-size: 12px;
}
</style>
