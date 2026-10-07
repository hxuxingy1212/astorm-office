<script setup lang="ts">
//! 统一入口：按 kind 分发到四格式 Viewer；也可直接用各专用组件
import { computed } from 'vue'
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
  }>(),
  { index: 0 },
)

const emit = defineEmits<{
  (e: 'select', path: string): void
  (e: 'hover', path: string | null): void
  (e: 'page-change', index: number): void
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
      @select="(p) => emit('select', p)"
      @hover="(p) => emit('hover', p)"
      @slide-change="(i) => emit('page-change', i)"
    />
    <DocxViewer
      v-else-if="kind === 'docx'"
      :document="asDocx"
      :resolve-media="resolveMedia"
      :part-index="index"
      @select="(p) => emit('select', p)"
      @hover="(p) => emit('hover', p)"
      @page-change="(i) => emit('page-change', i)"
    />
    <PdfViewer
      v-else-if="kind === 'pdf'"
      :doc="asPdf"
      :resolve-media="resolveMedia"
      :page-index="index"
      @select="(p) => emit('select', p)"
      @hover="(p) => emit('hover', p)"
      @page-change="(i) => emit('page-change', i)"
    />
    <XlsxViewer
      v-else
      :workbook="asXlsx"
      :resolve-media="resolveMedia"
      :sheet-index="index"
      @select="(p) => emit('select', p)"
      @hover="(p) => emit('hover', p)"
      @sheet-change="(i) => emit('page-change', i)"
    />
  </div>
</template>

<style>
.ov-root {
  color: #1f2328;
}
</style>
