<script setup lang="ts">
//! docx 单块渲染：物理分页视图与离屏度量共用同一份渲染，保证「量到的高度 = 页里排的高度」
//!
//! 路径（`/part[n]/heading[2]/…`）与元素摘要由宿主（DocxViewer）提供：本组件只负责
//! 把块画出来，并在元素上派发 pick 事件（enter/move/leave/tap），pickMode=false 时完全不参与交互。
import { computed } from 'vue'
import type { DocxBlock, DocxDocument, DocxRun } from '@/types'
import {
  blockStyle,
  figureStyle,
  fontMetricsOf,
  headingStyle,
  runStyle,
  shapeStyle,
} from '@/renderers/docx/blocks'
import { layoutParagraph, spansOf, type StyledChar } from '@/renderers/docx/linebreak'
import { textWidth } from '@/renderers/docx/textmetrics'
import { ptToPx } from '@/core/units'
import DocxRuns from '@/components/DocxRuns.vue'
import { latexToHtml } from '@/renderers/docx/latex'

const props = withDefaults(
  defineProps<{
    block: DocxBlock
    /** 所属文档（解析命名样式） */
    doc?: DocxDocument
    /** 同名块在本分片内的序号（展示编号 + 路径） */
    ordinal?: number
    /** 拾取模式：false 为纯只读观感（不派发任何 pick 事件） */
    pickMode?: boolean
    /** 元素路径：无 tail = 块自身，有 tail = 块内子元素（如 `table[1]/row[2]`） */
    pathOf?: (tail?: string) => string
    /** 媒体解析：把 word/media/xxx 解析成可加载 URL */
    resolveMedia?: (src: string) => string
    /** 表格跨页续排：本片从第几行开始（>0 时片首重复表头行，与 Word 的 w:tblHeader 一致） */
    rowFrom?: number
    /** 表格跨页续排：本片渲染到第几行（不含；缺省到表尾） */
    rowTo?: number
    /** 表格行内切：本片首块（rowFrom 行）跳过几行文本（续排片） */
    skipLines?: number
    /** 表格行内切：本片末块（rowTo-1 行）只画前几行文本 */
    keepLines?: number
    /** 表格逐行几何（行内切逐格裁剪用）：行高与各格行数 */
    rowGeom?: { lines: number; lineH: number; cells: number[] }[] | null
    /** 正文流版心宽（px）：段落预断行用；缺省则交给浏览器断行 */
    breakWidth?: number
  }>(),
  { ordinal: 1 },
)

type PickAction = 'enter' | 'move' | 'leave' | 'tap'
const emit = defineEmits<{
  (e: 'pick', action: PickAction, block: DocxBlock, path: string, ev: MouseEvent): void
}>()

const headingLevel = computed(() =>
  Math.min(6, Math.max(1, Number((props.block as Record<string, unknown>).level ?? 1))),
)
const anyB = computed(() => props.block as unknown as Record<string, unknown>)
const runs = computed(() => (anyB.value.runs as DocxRun[] | undefined) ?? [])
const items = computed(() => (anyB.value.items as Record<string, unknown>[] | undefined) ?? [])
const rows = computed(() => (anyB.value.rows as Record<string, unknown>[] | undefined) ?? [])

/** 表格可见行（含跨页续排片）：续排片片首重复表头行，末尾止于 rowTo */
const visibleRows = computed<{ row: Record<string, unknown>; ri: number }[]>(() => {
  const all = rows.value
  const from = Math.max(0, props.rowFrom ?? 0)
  const to = Math.min(all.length, props.rowTo ?? all.length)
  const head = anyB.value.header_row === true ? 1 : 0
  const out: { row: Record<string, unknown>; ri: number }[] = []
  if (from > head) {
    for (let k = 0; k < head && k < all.length; k += 1) out.push({ row: all[k], ri: k })
  }
  for (let k = from; k < to; k += 1) out.push({ row: all[k], ri: k })
  return out
})

/** 行内切：该格本片要画几行文本、要跳过头几行（裁到行边界，Word 的一行跨页） */
function cellClip(ri: number, ci: number): { lines: number; skip: number } | null {
  const geom = props.rowGeom?.[ri]
  if (!geom) return null
  const cellLines = geom.cells[ci] ?? geom.lines
  const first = props.rowFrom ?? 0
  const last = (props.rowTo ?? rows.value.length) - 1
  let skip = 0
  if (ri === first && props.skipLines) skip = Math.min(props.skipLines, cellLines)
  let lines = Math.max(0, cellLines - skip)
  if (ri === last && props.keepLines !== undefined) lines = Math.min(lines, props.keepLines)
  if (skip === 0 && lines === cellLines) return null
  return { lines, skip }
}

/** 行内切时的裁剪盒样式：外层定高（行数 × 行高）+ 溢出裁掉，内层负外边距跳过前几行 */
function cellClipStyle(ri: number, ci: number): Record<string, string> | undefined {
  const clip = cellClip(ri, ci)
  const geom = props.rowGeom?.[ri]
  if (!clip || !geom) return undefined
  return {
    height: `${Math.round(clip.lines * geom.lineH * 100) / 100}px`,
    marginTop: clip.skip > 0 ? `${(-Math.round(clip.skip * geom.lineH * 100) / 100)}px` : '0px',
  }
}
const mediaSrc = computed(() => {
  const src = String(anyB.value.src ?? '')
  return props.resolveMedia ? props.resolveMedia(src) : src
})

/** 预断行后的一行：spans = 版心内文本；hang 行的 hangSpans = 悬挂到版心外的那个标点 */
interface BrokenLine {
  spans: { text: string; style?: Record<string, string> }[]
  hangSpans: { text: string; style?: Record<string, string> }[]
  hang: boolean
  last: boolean
  /** 本行可用宽（首行缩进后的首行更窄） */
  avail: number
  /** 首行缩进量（px） */
  indent: number
}

/**
 * 段落预断行（Word 兼容）：贪心排字 + 中文禁则 + 行末标点悬挂。
 *
 * 浏览器断行没有「标点悬挂」——行末放不下的句读会被挤到下一行，而 Word 让它探出版心一个字
 * （参考 PDF 实测 307 个正文行里 16 行这样越界，越界的全是「，。、」）。单段看只差一个字，
 * 长文逐段累积就是整页页差（实测说明文档 18 页 vs 参考 17 页）。
 *
 * 只在**纯文本段落**上启用：带图片/公式的 run 无法用文本重排。表格单元格不进来——单元格里
 * 的段落由表格模板直接渲染，且行内切分已经按实测行数逐格裁剪，再预断行反而不一致。
 */
function breakBlock(b: DocxBlock, width: number): BrokenLine[] | null {
  if (b.type !== 'paragraph' || width < 60) return null
  const st = blockStyle(props.doc, b)
  const any = b as unknown as Record<string, unknown>
  const rs = (any.runs as DocxRun[] | undefined) ?? []
  const text = rs.length
    ? rs.map((r) => String((r as unknown as Record<string, unknown>).text ?? '')).join('')
    : String(any.text ?? '')
  // 太短不值得重排（单行段落交给浏览器）；太长（异常块）不做，避免卡住渲染
  if (text.length < 12 || text.length > 4000) return null
  // 带媒体/公式的 run 不能用纯文本重排
  if (
    rs.some((r) => {
      const rr = r as unknown as Record<string, unknown>
      return rr.src || rr.latex || rr.formula
    })
  ) {
    return null
  }
  const chars: StyledChar[] = []
  if (rs.length) {
    rs.forEach((r) => {
      const fm = fontMetricsOf(st, r)
      const style = runStyle(r)
      for (const ch of String((r as unknown as Record<string, unknown>).text ?? '')) {
        chars.push({ ch, font: fm.font, size: fm.size, style })
      }
    })
  } else {
    const fm = fontMetricsOf(st)
    for (const ch of text) chars.push({ ch, font: fm.font, size: fm.size })
  }
  const inset =
    (parseFloat(st['margin-left'] ?? '') || 0) + (parseFloat(st['padding-left'] ?? '') || 0)
  const avail = Math.round(width - inset)
  const indent = Math.round((parseFloat(st['text-indent'] ?? '') || 0) * 100) / 100
  const size = fontMetricsOf(st).size || 16
  const lines = layoutParagraph(chars, {
    firstAvail: avail - indent,
    avail,
    // 悬挂上限取「一个字」：全角标点 1em，给 1.2 倍容差
    hangMax: size * 1.2,
    measure: textWidth,
  })
  // 单行且不悬挂的段落交给浏览器（没有重排的必要）；单行但**末字悬挂**的必须自己排——
  // 浏览器会把探出版心的那个标点折到下一行，正是要修的差别（LO 里这类行都只占一行）。
  if (lines.length < 2 && !lines.some((l) => l.hang)) return null
  return lines.map((ln, i) => {
    // 悬挂单元是单个全角标点（NO_START 全是全角），末字符即悬挂字符
    const hang = ln.hang && ln.to - ln.from > 1
    const mainTo = hang ? ln.to - 1 : ln.to
    return {
      hang,
      last: i === lines.length - 1,
      avail,
      indent: i === 0 ? indent : 0,
      spans: spansOf(chars, ln.from, mainTo),
      hangSpans: hang ? spansOf(chars, mainTo, ln.to) : [],
    }
  })
}

/** 本段（顶层块）的预断行结果 */
const brokenLines = computed<BrokenLine[] | null>(() =>
  breakBlock(props.block, props.breakWidth ?? 0),
)

/** 列表项内层块的预断行：可用宽 = 版心宽 − `.ov-list` 的 padding-left（26px，见样式表）。
 *  列表项的文字在 LO/Word 里同样按正文断行（含标点悬挂），不预断行会多出一行。 */
const LIST_INDENT = 26
function breakItem(ib: DocxBlock): BrokenLine[] | null {
  const width = props.breakWidth ?? 0
  return breakBlock(ib, width - LIST_INDENT)
}
/** 是否两端对齐（Word 的 justify）：只有 justify 段落的非末行才拉伸 */
const justifyPara = computed(() => blockStyle(props.doc, props.block)['text-align'] === 'justify')
/** 行盒样式：清零 text-indent（首行缩进改挂首行的 margin，否则每行都会缩进），
 *  非末行用 text-align-last 硬拉伸到版心（每个行盒只有一行，末行规则要显式指定） */
function lineStyle(ln: BrokenLine): Record<string, string> {
  const out: Record<string, string> = { textIndent: '0' }
  if (ln.indent) out.marginLeft = `${ln.indent}px`
  if (justifyPara.value && !ln.last) {
    out.textAlign = 'justify'
    out.textAlignLast = 'justify'
  }
  return out
}
/** 悬挂行的版心主段：两端对齐的段落把主段拉伸到正好版心宽（标点因此从版心右缘起悬挂）。
 *  首行有缩进时主段还要减掉缩进量（缩进由 lineStyle 的 margin-left 补上），否则整行右移。 */
function hangMainStyle(ln: BrokenLine): Record<string, string> | undefined {
  if (!justifyPara.value || ln.last) return undefined
  return {
    display: 'inline-block',
    width: `${Math.round((ln.avail - (ln.indent || 0)) * 100) / 100}px`,
    textAlign: 'justify',
    textAlignLast: 'justify',
  }
}

/** 元素路径：tail 为空即块自身 */
function path(tail?: string): string {
  return props.pathOf?.(tail) ?? ''
}

/** 单元格样式：对齐 + 列宽（模型 widths 是磅）+ 单元格边距（磅） */
function cellStyle(cell: Record<string, unknown>, ci: number): Record<string, string | undefined> {
  const widths = anyB.value.widths as number[] | undefined
  const n = widths?.length ?? 0
  const margin = cell.margin as number | undefined
  const pad = margin != null ? `${Math.round(ptToPx(margin))}px` : undefined
  // 单元格自身的字号/行距取首个内层段落：否则继承页面基准行高（1.74×字号），行会被撑爆
  const inner = (cell.blocks as DocxBlock[] | undefined)?.[0]
  return {
    ...(inner ? innerBlockStyle(inner) : {}),
    textAlign: (cell.align as string) ?? undefined,
    // 单元格垂直对齐（Word 的 w:vAlign：标签列常用居中）
    verticalAlign: (cell.valign as string) ?? undefined,
    width: widths?.[ci]
      ? `${Math.round(ptToPx(widths[ci]))}px`
      : n
        ? `${Math.round(100 / n)}%`
        : undefined,
    // 单元格纵向内边距：参考 PDF 实测「行高 × 行数 + 1.6pt」，即单侧约 0.8pt≈1px；
    // 给 2px 会让每行高 1.6px，跨几张表就够把下一页的表挤下去
    padding: pad ? `1px ${pad}` : undefined,
  }
}

/** 表格整体：列宽合计定宽（磅）、fixed 布局、缩进；border.style=none 表示无框线 */
function tableStyle(): Record<string, string | undefined> {
  const widths = anyB.value.widths as number[] | undefined
  const total = widths?.length ? widths.reduce((a, b) => a + b, 0) : 0
  const indent = anyB.value.indent as number | undefined
  return {
    tableLayout: anyB.value.layout === 'fixed' ? 'fixed' : 'auto',
    width: total > 0 ? `${Math.round(ptToPx(total))}px` : '100%',
    marginLeft: typeof indent === 'number' ? `${Math.round(ptToPx(indent))}px` : undefined,
  }
}

/** 列表项：把首个子块的字号/行距落到 li（否则继承页面基准行高，行高被撑大） */
function listItemStyle(item: Record<string, unknown>): Record<string, string> {
  const first = (item.blocks as DocxBlock[] | undefined)?.[0]
  return first ? innerBlockStyle(first) : {}
}

/** 无框线表格（模型 border.style = none / nil） */
const plainBorder = computed(() => {
  const border = anyB.value.border as { style?: string } | undefined
  return !border || !border.style || border.style === 'none' || border.style === 'nil'
})

/** 内层块（列表项/单元格里的段落）：行距、字号这些排版属性必须落到内层，否则表格会虚高 */
function innerBlockStyle(b: DocxBlock): Record<string, string> {
  const st = blockStyle(props.doc, b)
  return {
    'font-size': st['font-size'] ?? '',
    'font-family': st['font-family'] ?? '',
    'line-height': st['line-height'] ?? '',
    color: st['color'] ?? '',
    'font-weight': st['font-weight'] ?? '',
    'font-style': st['font-style'] ?? '',
    'text-align': st['text-align'] ?? '',
  }
}

function pick(action: PickAction, block: DocxBlock, tail: string | undefined, ev?: MouseEvent) {
  if (!props.pickMode) return
  emit('pick', action, block, path(tail), ev as MouseEvent)
}
function enter(block: DocxBlock, ev: MouseEvent, tail?: string) {
  pick('enter', block, tail, ev)
}
function move(ev: MouseEvent) {
  pick('move', props.block, undefined, ev)
}
function leave() {
  pick('leave', props.block, undefined)
}
function tap(block: DocxBlock, ev: MouseEvent, tail?: string) {
  pick('tap', block, tail, ev)
}
</script>

<template>
  <!-- 标题 -->
  <component
    :is="`h${headingLevel}`"
    v-if="block.type === 'heading'"
    class="ov-heading"
    :style="headingStyle(doc, block) as any"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
    @click="tap(block, $event)"
  >
    <template v-if="anyB.numbering">{{ ordinal }}. </template>{{ anyB.text }}
  </component>

  <!-- 段落 -->
  <p
    v-else-if="block.type === 'paragraph'"
    class="ov-para"
    :style="blockStyle(doc, block)"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
    @click="tap(block, $event)"
  >
    <!-- 预断行：逐行渲染，行末标点悬挂出版心（Word 的 overflowPunct） -->
    <template v-if="brokenLines">
      <span v-for="(ln, li) in brokenLines" :key="li" class="ov-line" :style="lineStyle(ln)">
        <template v-if="ln.hang && hangMainStyle(ln)">
          <span class="ov-jmain" :style="hangMainStyle(ln)">
            <span v-for="(s, si) in ln.spans" :key="si" :style="s.style">{{ s.text }}</span>
          </span>
        </template>
        <template v-else>
          <span v-for="(s, si) in ln.spans" :key="si" :style="s.style">{{ s.text }}</span>
        </template>
        <span v-for="(s, si) in ln.hangSpans" :key="`h${si}`" :style="s.style">{{ s.text }}</span>
      </span>
    </template>
    <DocxRuns v-else-if="runs.length" :runs="runs" :resolve-media="resolveMedia" />
    <template v-else-if="anyB.text">{{ anyB.text }}</template>
    <!-- 空段落：Word 里段标记占一行（封面留白/空行全靠它），HTML 空块会塌成 0 高，这里补一个换行 -->
    <br v-else />
  </p>

  <!-- 列表 -->
  <component
    :is="anyB.ordered ? 'ol' : 'ul'"
    v-else-if="block.type === 'list'"
    class="ov-list"
    :style="blockStyle(doc, block)"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    <li v-for="(item, ii) in items" :key="ii" :style="listItemStyle(item)">
      <span
        v-for="(ib, iib) in (item.blocks as DocxBlock[] | undefined) ?? []"
        :key="iib"
        :style="innerBlockStyle(ib)"
        @mouseenter="enter(ib, $event, `item[${ii + 1}]/${ib.type}[1]`)"
        @mousemove="move($event)"
        @mouseleave="leave()"
      >
        <!-- 列表项内的段落同样按 Word 断行（标点悬挂） -->
        <template v-if="breakItem(ib)">
          <span
            v-for="(ln, li) in breakItem(ib) ?? []"
            :key="li"
            class="ov-line"
            :style="lineStyle(ln)"
          >
            <template v-if="ln.hang && hangMainStyle(ln)">
              <span class="ov-jmain" :style="hangMainStyle(ln)">
                <span v-for="(s, si) in ln.spans" :key="si" :style="s.style">{{ s.text }}</span>
              </span>
            </template>
            <template v-else>
              <span v-for="(s, si) in ln.spans" :key="si" :style="s.style">{{ s.text }}</span>
            </template>
            <span v-for="(s, si) in ln.hangSpans" :key="`h${si}`" :style="s.style">{{
              s.text
            }}</span>
          </span>
        </template>
        <DocxRuns
          v-else-if="(ib as Record<string, unknown>).runs"
          :runs="((ib as Record<string, unknown>).runs as DocxRun[])"
          :resolve-media="resolveMedia"
          :float="false"
        />
        <template v-else>{{ (ib as Record<string, unknown>).text ?? '' }}</template>
      </span>
    </li>
  </component>

  <!-- 表格 -->
  <table
    v-else-if="block.type === 'table'"
    class="ov-table"
    :class="{ 'ov-table-plain': plainBorder }"
    :style="tableStyle()"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    <tbody>
      <tr
        v-for="entry in visibleRows"
        :key="entry.ri"
        :class="{ 'ov-tr-head': anyB.header_row && entry.ri === 0 }"
        @mouseenter="enter(entry.row as DocxBlock, $event, `row[${entry.ri + 1}]`)"
        @mousemove="move($event)"
        @mouseleave="leave()"
      >
        <td
          v-for="(cell, ci) in (entry.row.cells as Record<string, unknown>[] | undefined) ?? []"
          :key="ci"
          :colspan="(cell.colspan as number) ?? 1"
          :style="cellStyle(cell, ci)"
          @mouseenter.stop="enter(cell as DocxBlock, $event, `row[${entry.ri + 1}]/cell[${ci + 1}]`)"
          @mousemove="move($event)"
          @mouseleave="leave()"
        >
          <!-- 行内切（Word 的一行跨页）：逐格裁到行边界；不切时 style 为空，等于直接放内容 -->
          <div class="ov-cellclip" :style="cellClipStyle(entry.ri, ci)">
            <span
              v-for="(cb, cbi) in (cell.blocks as DocxBlock[] | undefined) ?? []"
              :key="cbi"
              :style="innerBlockStyle(cb)"
              @mouseenter.stop="
                enter(cb, $event, `row[${entry.ri + 1}]/cell[${ci + 1}]/${cb.type}[1]`)
              "
              @mousemove="move($event)"
              @mouseleave="leave()"
            >
              <DocxRuns
                v-if="(cb as Record<string, unknown>).runs"
                :runs="((cb as Record<string, unknown>).runs as DocxRun[])"
                :resolve-media="resolveMedia"
                :float="false"
              />
              <template v-else>{{ (cb as Record<string, unknown>).text ?? '' }}</template>
            </span>
          </div>
        </td>
      </tr>
    </tbody>
  </table>

  <!-- 图片 -->
  <figure
    v-else-if="block.type === 'image'"
    class="ov-figure"
    :style="figureStyle(block)"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    <img
      :src="mediaSrc"
      :style="{
        width: anyB.width ? `${Math.round((anyB.width as number) * 96)}px` : undefined,
        maxWidth: '100%',
      }"
      :alt="(anyB.alt as string) ?? ''"
    />
    <figcaption v-if="anyB.caption" class="ov-caption">{{ anyB.caption }}</figcaption>
  </figure>

  <!-- 公式 -->
  <div
    v-else-if="block.type === 'formula'"
    class="ov-formula"
    :class="{ display: anyB.display !== false }"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    <span v-html="latexToHtml((anyB.latex as string) ?? '')"></span>
    <span v-if="anyB.number" class="ov-eqno">（{{ anyB.number }}）</span>
  </div>

  <!-- 代码 -->
  <pre
    v-else-if="block.type === 'code'"
    class="ov-code"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
    >{{ anyB.text }}</pre
  >

  <!-- 引用 -->
  <blockquote
    v-else-if="block.type === 'quote'"
    class="ov-quote"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    {{ anyB.text }}
  </blockquote>

  <!-- 题注 -->
  <div
    v-else-if="block.type === 'caption'"
    class="ov-caption-block"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    {{ anyB.text }}
  </div>

  <!-- 目录 -->
  <div
    v-else-if="block.type === 'toc'"
    class="ov-toc"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    <div class="ov-toc-title">{{ anyB.title || '目录' }}</div>
    <div
      v-for="(e, ei) in (anyB.entries as Record<string, unknown>[] | undefined) ?? []"
      :key="ei"
      class="ov-toc-item"
      :style="{ paddingLeft: `${(Number(e.level ?? 1) - 1) * 16}px` }"
    >
      {{ e.text }}
    </div>
  </div>

  <!-- 参考文献 -->
  <div
    v-else-if="block.type === 'bibliography'"
    class="ov-bib"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    <div class="ov-bib-title">{{ anyB.title || '参考文献' }}</div>
    <div
      v-for="(e, ei) in (anyB.entries as Record<string, unknown>[] | undefined) ?? []"
      :key="ei"
      class="ov-bib-item"
    >
      [{{ ei + 1 }}] {{ ((e.authors as string[] | undefined) ?? []).join(', ') }}. {{ e.title }}.
      <em v-if="e.container">{{ e.container }}</em
      ><template v-if="e.year">, {{ e.year }}</template>.
    </div>
  </div>

  <!-- 形状 / 文本框 -->
  <div
    v-else-if="block.type === 'shape' || block.type === 'textbox'"
    class="ov-shape"
    :style="shapeStyle(doc, block)"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    {{ anyB.text }}
  </div>

  <!-- 分页符/分栏符：边界由分页器消费，这里只占位（不渲染可见内容、不占高度） -->
  <span
    v-else-if="block.type === 'page_break' || block.type === 'column_break'"
    class="ov-pagebreak-mark"
  />

  <!-- 未知块：原样提示（保证不丢信息） -->
  <div v-else class="ov-unknown">［{{ block.type }}］</div>
</template>

<style>
.ov-pagebreak-mark {
  display: none;
}
</style>
