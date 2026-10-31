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
    /** 缩略图侧栏开合(pptx/pdf):提供即受控——显隐只跟此值走,内置按钮汇报 thumbnails-change;
     *  不传保持自主模式(内置按钮直接开合,默认展开)。 */
    showThumbnails?: boolean
    /** 内置换页器(docx/pptx/pdf,默认显示):false 隐藏,翻页由宿主经 index/pageIndex(写)+
     *  page-change(读)+pages-change(docx 总页数)接管。 */
    showPager?: boolean
    /** 受控物理页(docx 专用;index 对 pptx/pdf/xlsx 即页/表下标且本就可外部驱动):
     *  变化即跳页,宿主主动设页不发 page-change。 */
    pageIndex?: number
    /** 铺满容器:容器有确定高度时传 true——viewer 高度 100% 跟随容器、内容内部滚动;
     *  默认 false(自撑高 72~78vh 兜底)。 */
    fill?: boolean
    /** 主题:light / dark / auto(默认 auto,跟随系统 prefers-color-scheme 实时切换),透传四格式 viewer。
     *  只暗化 chrome;页面(纸张/幻灯片/表格画布)保持白底,宿主要连页面一起暗在外层加 ov-dark-pages 类。 */
    theme?: 'light' | 'dark' | 'auto'
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
  /** 缩略图侧栏开合汇报(pptx/pdf;受控模式宿主应回写 showThumbnails prop) */
  (e: 'thumbnails-change', open: boolean): void
  /** 总物理页数汇报(docx;宿主自渲染换页 UI 需要) */
  (e: 'pages-change', total: number): void
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
      :show-thumbnails="showThumbnails"
      :show-pager="showPager"
      :fill="fill"
      :theme="theme"
      @select="(p, info) => emit('select', p, info)"
      @hover="(p) => emit('hover', p)"
      @slide-change="(i) => emit('page-change', i)"
      @zoom-change="(v) => emit('zoom-change', v)"
      @thumbnails-change="(o) => emit('thumbnails-change', o)"
    />
    <DocxViewer
      v-else-if="kind === 'docx'"
      :document="asDocx"
      :resolve-media="resolveMedia"
      :part-index="index"
      :page-index="pageIndex"
      :zoom="zoom"
      :pick-mode="pickMode"
      :outline="outline"
      :show-pager="showPager"
      :fill="fill"
      :theme="theme"
      @select="(p, info) => emit('select', p, info)"
      @hover="(p) => emit('hover', p)"
      @page-change="(i) => emit('page-change', i)"
      @zoom-change="(v) => emit('zoom-change', v)"
      @outline-change="(o) => emit('outline-change', o)"
      @pages-change="(n) => emit('pages-change', n)"
    />
    <PdfViewer
      v-else-if="kind === 'pdf'"
      :doc="asPdf"
      :resolve-media="resolveMedia"
      :page-index="index"
      :zoom="zoom"
      :pick-mode="pickMode"
      :show-thumbnails="showThumbnails"
      :show-pager="showPager"
      :fill="fill"
      :theme="theme"
      @select="(p, info) => emit('select', p, info)"
      @hover="(p) => emit('hover', p)"
      @page-change="(i) => emit('page-change', i)"
      @zoom-change="(v) => emit('zoom-change', v)"
      @thumbnails-change="(o) => emit('thumbnails-change', o)"
    />
    <XlsxViewer
      v-else
      :workbook="asXlsx"
      :resolve-media="resolveMedia"
      :sheet-index="index"
      :zoom="zoom"
      :pick-mode="pickMode"
      :fill="fill"
      :theme="theme"
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
