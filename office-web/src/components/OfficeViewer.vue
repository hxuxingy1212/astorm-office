<script setup lang="ts">
//! 统一入口：按 kind 分发到四格式 Viewer；也可直接用各专用组件
import { computed } from 'vue'
import type { SelectInfo } from '@/core/hover'
import type { DocxDocument, XlsxSheet, XlsxWorkbook } from '@/types'
import type { PackedSheet } from '@/renderers/xlsx/packed'
import type { PptxDoc } from '@/renderers/pptx/presentation'
import type { PdfDoc } from '@/renderers/pdf/types'
import PptxViewer from '@/components/PptxViewer.vue'
import DocxViewer from '@/components/DocxViewer.vue'
import XlsxViewer from '@/components/XlsxViewer.vue'
import PdfViewer from '@/components/PdfViewer.vue'

const props = withDefaults(
  defineProps<{
    /** 格式：docx / xlsx / pptx / pdf */
    kind: 'docx' | 'xlsx' | 'pptx' | 'pdf'
    /** 产物 JSON（unpack 后的 document.json / workbook.json / presentation.json） */
    data:
      | DocxDocument
      | XlsxWorkbook
      | { sheets: (XlsxSheet | PackedSheet)[] }
      | PptxDoc
      | PdfDoc
    /** 媒体解析：把包内相对路径（word/media/x、xl/media/x、media/x）解析成可加载 URL */
    resolveMedia?: (src: string) => string
    /** 各格式通用：初始页/分片/工作表下标 */
    index?: number
    /** 受控缩放:提供即受控——四 viewer 隐藏内置缩放控件,缩放跟随此值(相对适配基准的倍率;
     *  XLSX 为相对自然尺寸)。不传保持自主模式(内置缩放滑杆)。 */
    zoom?: number
    /** 拾取模式:仅 true 时启用交互层(元素 hover 高亮/路径卡/选中事件);false(默认)纯只读。 */
    pickMode?: boolean
    /** 侧边目录开合(docx):提供即受控——目录显隐只跟此值走,内置按钮汇报 outline-change;
     *  不传保持自主模式(内置按钮直接开合)。 */
    outline?: boolean
  }>(),
  { index: 0 },
)

const emit = defineEmits<{
  (e: 'select', path: string, info?: SelectInfo): void
  (e: 'hover', path: string | null): void
  (e: 'page-change', index: number): void
  /** 缩放汇报(受控模式):scale=绝对倍率(展示用百分比),fitScale=适配基准(宿主换算) */
  (e: 'zoom-change', v: { scale: number; fitScale: number }): void
  /** 目录开合汇报(docx;受控模式宿主应回写 outline prop) */
  (e: 'outline-change', open: boolean): void
}>()

const asPptx = computed(() => props.data as PptxDoc)
const asDocx = computed(() => props.data as DocxDocument)
const asXlsx = computed(() => props.data as XlsxWorkbook | { sheets: (XlsxSheet | PackedSheet)[] })
const asPdf = computed(() => props.data as PdfDoc)
</script>

<template>
  <div class="ov-root">
    <PptxViewer
      v-if="kind === 'pptx'"
      :presentation="asPptx"
      :resolve-media="resolveMedia"
      :slide-index="index"
      :zoom="zoom"
      :pick-mode="pickMode"
      @select="(p, info) => emit('select', p, info)"
      @hover="(p) => emit('hover', p)"
      @slide-change="(i) => emit('page-change', i)"
      @zoom-change="(v) => emit('zoom-change', v)"
    />
    <DocxViewer
      v-else-if="kind === 'docx'"
      :document="asDocx"
      :resolve-media="resolveMedia"
      :part-index="index"
      :zoom="zoom"
      :pick-mode="pickMode"
      :outline="outline"
      @select="(p, info) => emit('select', p, info)"
      @hover="(p) => emit('hover', p)"
      @page-change="(i) => emit('page-change', i)"
      @zoom-change="(v) => emit('zoom-change', v)"
      @outline-change="(o) => emit('outline-change', o)"
    />
    <PdfViewer
      v-else-if="kind === 'pdf'"
      :doc="asPdf"
      :resolve-media="resolveMedia"
      :page-index="index"
      :zoom="zoom"
      :pick-mode="pickMode"
      @select="(p, info) => emit('select', p, info)"
      @hover="(p) => emit('hover', p)"
      @page-change="(i) => emit('page-change', i)"
      @zoom-change="(v) => emit('zoom-change', v)"
    />
    <XlsxViewer
      v-else
      :workbook="asXlsx"
      :resolve-media="resolveMedia"
      :sheet-index="index"
      :zoom="zoom"
      :pick-mode="pickMode"
      @select="(p, info) => emit('select', p, info)"
      @hover="(p) => emit('hover', p)"
      @sheet-change="(i) => emit('page-change', i)"
      @zoom-change="(v) => emit('zoom-change', v)"
    />
  </div>
</template>

<style>
.ov-root {
  color: var(--ov-text-strong);
  /* 透传宿主高度：页签/Modal 给了高度就撑满（docx 目录与内容同高 relies on it）；
     没给高度时各 viewer 自带 min-height 兜底 */
  height: 100%;
}
</style>
