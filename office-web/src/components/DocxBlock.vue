<script setup lang="ts">
//! docx 单块渲染：物理分页视图与离屏度量共用同一份渲染，保证「量到的高度 = 页里排的高度」
//!
//! 路径（`/part[n]/heading[2]/…`）与元素摘要由宿主（DocxViewer）提供：本组件只负责
//! 把块画出来，并在元素上派发 pick 事件（enter/move/leave/tap），pickMode=false 时完全不参与交互。
import { computed } from 'vue'
import type { DocxBlock, DocxDocument, DocxRun } from '@/types'
import { blockStyle, figureStyle, headingStyle, runStyle, shapeStyle } from '@/renderers/docx/blocks'
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
const mediaSrc = computed(() => {
  const src = String(anyB.value.src ?? '')
  return props.resolveMedia ? props.resolveMedia(src) : src
})

/** 元素路径：tail 为空即块自身 */
function path(tail?: string): string {
  return props.pathOf?.(tail) ?? ''
}

/** 单元格样式：对齐 + 列宽（模型里列宽是英寸） */
function cellStyle(cell: Record<string, unknown>, ci: number): Record<string, string | undefined> {
  const widths = anyB.value.column_widths as number[] | undefined
  return {
    textAlign: (cell.align as string) ?? undefined,
    width: widths?.[ci] ? `${Math.round(widths[ci] * 96)}px` : undefined,
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
    <template v-if="runs.length">
      <span v-for="(r, ri) in runs" :key="ri" :style="runStyle(r)">{{ r.text }}</span>
    </template>
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
    <li v-for="(item, ii) in items" :key="ii">
      <span
        v-for="(ib, iib) in (item.blocks as DocxBlock[] | undefined) ?? []"
        :key="iib"
        @mouseenter="enter(ib, $event, `item[${ii + 1}]/${ib.type}[1]`)"
        @mousemove="move($event)"
        @mouseleave="leave()"
        >{{ (ib as Record<string, unknown>).text ?? '' }}</span
      >
    </li>
  </component>

  <!-- 表格 -->
  <table
    v-else-if="block.type === 'table'"
    class="ov-table"
    :style="blockStyle(doc, block)"
    @mouseenter="enter(block, $event)"
    @mousemove="move($event)"
    @mouseleave="leave()"
  >
    <tbody>
      <tr
        v-for="(row, ri) in rows"
        :key="ri"
        :class="{ 'ov-tr-head': anyB.header_row && ri === 0 }"
        @mouseenter="enter(row as DocxBlock, $event, `row[${ri + 1}]`)"
        @mousemove="move($event)"
        @mouseleave="leave()"
      >
        <td
          v-for="(cell, ci) in (row.cells as Record<string, unknown>[] | undefined) ?? []"
          :key="ci"
          :colspan="(cell.colspan as number) ?? 1"
          :style="cellStyle(cell, ci)"
          @mouseenter.stop="enter(cell as DocxBlock, $event, `row[${ri + 1}]/cell[${ci + 1}]`)"
          @mousemove="move($event)"
          @mouseleave="leave()"
        >
          <span
            v-for="(cb, cbi) in (cell.blocks as DocxBlock[] | undefined) ?? []"
            :key="cbi"
            @mouseenter.stop="
              enter(cb, $event, `row[${ri + 1}]/cell[${ci + 1}]/${cb.type}[1]`)
            "
            @mousemove="move($event)"
            @mouseleave="leave()"
            >{{ (cb as Record<string, unknown>).text ?? '' }}</span
          >
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

  <!-- 分页符：页边界由分页器消费，这里只占位（不渲染可见内容、不占高度） -->
  <span v-else-if="block.type === 'page_break'" class="ov-pagebreak-mark" />

  <!-- 未知块：原样提示（保证不丢信息） -->
  <div v-else class="ov-unknown">［{{ block.type }}］</div>
</template>

<style>
.ov-pagebreak-mark {
  display: none;
}
</style>
