<script setup lang="ts">
// 顶部插入工具栏：文本框 / 形状 / 图片 / 线条 / 图表 / 表格 / 图标 / 公式
import { ref } from 'vue'
import { computed, nextTick, onBeforeUnmount, onMounted } from 'vue'
import { useSlidesStore } from '@/stores/slides'
import { useEditorStore } from '@/stores/editor'
import { useSnapshotStore } from '@/stores/snapshot'
import {
  createShapeElement,
  createLineElement,
  createImageElement,
  createTableElement,
  createBarChartElement,
  createIconElement,
  createFormulaElement,
} from '@/utils/elements'
import { uploadMedia } from '@/api'
import { SHAPE_PATHS } from '@/configs/shapes'
import { ICON_LIST } from '@/configs/icons'
import type { Element } from '@/types/presentation'
import { inchToPx, pxToInch } from '@/utils/convert'
import SlideView from './canvas/SlideView.vue'

const slides = useSlidesStore()
const editor = useEditorStore()
const snapshot = useSnapshotStore()

const COMMON_SHAPES = [
  'rect', 'roundRect', 'ellipse', 'triangle', 'diamond', 'pentagon', 'hexagon',
  'rightArrow', 'leftArrow', 'chevron', 'star5', 'donut', 'flowChartProcess',
  'flowChartDecision', 'flowChartTerminator', 'lightningBolt', 'heart', 'cloud',
]

/** 在画布中心附近插入元素 */
function insert(el: Element) {
  if (!slides.currentSlide) return
  const cx = pxToInch(slides.canvasWidthPx / 2 - inchToPx(el.position.w) / 2)
  const cy = pxToInch(slides.canvasHeightPx / 2 - inchToPx(el.position.h) / 2)
  el.position = { ...el.position, x: Math.round(cx * 10) / 10, y: Math.round(cy * 10) / 10 }
  slides.addElement('', el)
  const lastPath = `${slides.currentSlide.elements.length - 1}`
  editor.select(lastPath)
  snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
}

// === 各插入动作 ===

// === 弹出面板状态 ===
const showShapePicker = ref(false)
const showChartPicker = ref(false)
const showIconPicker = ref(false)
const showTextPicker = ref(false)

// 下拉容器 ref（用于点击外部关闭）
const textWrap = ref<HTMLElement | null>(null)
const shapeWrap = ref<HTMLElement | null>(null)
const chartWrap = ref<HTMLElement | null>(null)
const iconWrap = ref<HTMLElement | null>(null)

/** 点击任意下拉容器外部时关闭对应弹层 */
function onDocMouseDown(e: MouseEvent) {
  const t = e.target as Node
  if (textWrap.value && !textWrap.value.contains(t)) showTextPicker.value = false
  if (shapeWrap.value && !shapeWrap.value.contains(t)) showShapePicker.value = false
  if (chartWrap.value && !chartWrap.value.contains(t)) showChartPicker.value = false
  if (iconWrap.value && !iconWrap.value.contains(t)) showIconPicker.value = false
}
onMounted(() => document.addEventListener('mousedown', onDocMouseDown))
onBeforeUnmount(() => document.removeEventListener('mousedown', onDocMouseDown))

/** 文本框插入：开启拖拽绘制模式（横向/纵向） */
function addText(vert = 'horz') {
  editor.startDraw('text', vert)
  showTextPicker.value = false
}
function addLine() {
  insert(createLineElement(1, 1))
}
function addShape(shapeType: string) {
  insert(createShapeElement(shapeType, 1, 1))
  showShapePicker.value = false
}
function addTable() {
  insert(createTableElement(1, 1))
}
function addChart() {
  insert(createBarChartElement(1, 1))
  showChartPicker.value = false
}
function addIcon(icon: string) {
  insert(createIconElement(icon, 1, 1))
  showIconPicker.value = false
}
function addFormula() {
  insert(createFormulaElement('E = mc^2', 1, 1))
}

const imageInput = ref<HTMLInputElement | null>(null)
const imageError = ref('')
async function onImageChange(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  try {
    const src = await uploadMedia(file)
    insert(createImageElement(src, 1, 1))
    imageError.value = ''
  } catch (err) {
    imageError.value = `图片上传失败: ${(err as Error).message}`
  } finally {
    input.value = ''
  }
}

// === 预览（全屏演示） ===
const showPreview = ref(false)
const previewIndex = ref(0)
const previewMaskRef = ref<HTMLElement | null>(null)
const viewportSize = ref({ w: window.innerWidth, h: window.innerHeight })

/** 幻灯片按 contain 缩放填满屏幕 */
const previewScale = computed(() => {
  const lw = slides.canvasWidthPx
  const lh = slides.canvasHeightPx
  const vw = viewportSize.value.w
  const vh = viewportSize.value.h
  const scale = Math.min(vw / lw, vh / lh)
  return Math.max(0.01, scale)
})

function resizeHandler() {
  viewportSize.value = { w: window.innerWidth, h: window.innerHeight }
}

async function openPreview() {
  previewIndex.value = slides.slideIndex
  showPreview.value = true
  viewportSize.value = { w: window.innerWidth, h: window.innerHeight }
  // 等待预览层渲染后再请求浏览器真全屏（否则 ref 尚未挂载，requestFullscreen 被跳过）
  await nextTick()
  previewMaskRef.value?.requestFullscreen?.().catch(() => {})
}

function closePreview() {
  showPreview.value = false
  if (document.fullscreenElement) document.exitFullscreen?.().catch(() => {})
}

function nextSlide() {
  if (previewIndex.value < slides.slides.length - 1) previewIndex.value += 1
}
function prevSlide() {
  if (previewIndex.value > 0) previewIndex.value -= 1
}

/** 键盘：Esc 退出 / Enter 下一张 / 左右箭头翻页 */
function onPreviewKey(e: KeyboardEvent) {
  if (!showPreview.value) return
  const key = e.key.toLowerCase()
  if (key === 'escape') {
    closePreview()
  } else if (key === 'enter' || key === 'arrowright' || key === ' ') {
    e.preventDefault()
    nextSlide()
  } else if (key === 'arrowleft') {
    e.preventDefault()
    prevSlide()
  }
}

/** 滚轮：向下/向右下一张，向上/向左上一张 */
function onPreviewWheel(e: WheelEvent) {
  if (!showPreview.value) return
  e.preventDefault()
  if (e.deltaY > 0) nextSlide()
  else prevSlide()
}

/** 点击进度区提示（点击右侧 vs 左侧翻页） */
function onPreviewClick(e: MouseEvent) {
  if (!showPreview.value) return
  const x = e.clientX
  if (x < window.innerWidth * 0.35) prevSlide()
  else if (x > window.innerWidth * 0.65) nextSlide()
}

window.addEventListener('keydown', onPreviewKey, true)
window.addEventListener('resize', resizeHandler)

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onPreviewKey, true)
  window.removeEventListener('resize', resizeHandler)
})
</script>

<template>
  <div class="insert-toolbar">
    <div class="wrap" ref="textWrap">
      <button class="tool-btn labeled" title="文本框" @click="showTextPicker = !showTextPicker">
        <span class="tb-icon">T</span>
        <span class="tb-label">文本</span>
      </button>
      <div v-if="showTextPicker" class="popover text-popover">
        <div class="pop-title">文本方向</div>
        <div class="text-dir-item" @click="addText('horz')">横向</div>
        <div class="text-dir-item" @click="addText('vert')">纵向</div>
      </div>
    </div>

    <div class="wrap" ref="shapeWrap">
      <button class="tool-btn labeled" title="形状" @click="showShapePicker = !showShapePicker">
        <svg viewBox="0 0 200 200" class="tb-svg"><rect x="30" y="30" width="140" height="140" rx="10" fill="#4472C4" /></svg>
        <span class="tb-label">形状</span>
      </button>
      <div v-if="showShapePicker" class="popover">
        <div class="pop-title">形状</div>
        <div class="shape-grid">
          <div v-for="s in COMMON_SHAPES" :key="s" class="shape-item" :title="s" @click="addShape(s)">
            <svg viewBox="0 0 200 200" width="24" height="24">
              <path :d="`${SHAPE_PATHS[s] ?? ''}`" fill="#4472C4" stroke="#2b4d8f" stroke-width="4" />
            </svg>
          </div>
        </div>
      </div>
    </div>

    <div class="wrap">
      <button class="tool-btn labeled" title="图片" @click="imageInput?.click()">
        <svg viewBox="0 0 24 24" class="tb-svg"><path d="M5 3h14a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2zm0 14 4-5 3 3 4-6 5 8V5H5v12z" fill="#4472C4"/></svg>
        <span class="tb-label">图片</span>
      </button>
      <input ref="imageInput" type="file" accept="image/*" hidden @change="onImageChange" />
    </div>

    <button class="tool-btn labeled" title="线条" @click="addLine">
      <svg viewBox="0 0 24 24" class="tb-svg"><path d="M3 17 L21 7" stroke="#4472C4" stroke-width="3" stroke-linecap="round"/></svg>
      <span class="tb-label">线条</span>
    </button>

    <div class="wrap" ref="chartWrap">
      <button class="tool-btn labeled" title="图表" @click="showChartPicker = !showChartPicker">
        <svg viewBox="0 0 24 24" class="tb-svg"><path d="M4 20V6h3v14H4zm6 0V10h3v10h-3zm6 0V3h3v17h-3z" fill="#4472C4"/></svg>
        <span class="tb-label">图表</span>
      </button>
      <div v-if="showChartPicker" class="popover">
        <div class="pop-title">图表类型</div>
        <div class="chart-list">
          <div class="chart-item" @click="addChart()">柱状图</div>
        </div>
      </div>
    </div>

    <button class="tool-btn labeled" title="表格" @click="addTable">
      <svg viewBox="0 0 24 24" class="tb-svg"><path d="M4 4h16a1 1 0 0 1 1 1v14a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V5a1 1 0 0 1 1-1zm1 5v9h14V9H5zm0-3v2h14V6H5z" fill="#4472C4"/></svg>
      <span class="tb-label">表格</span>
    </button>

    <div class="wrap" ref="iconWrap">
      <button class="tool-btn labeled" title="图标" @click="showIconPicker = !showIconPicker">
        <svg viewBox="0 0 24 24" class="tb-svg"><path d="M12 2l2.4 4.9 5.4.8-3.9 3.8.9 5.4L12 14.6 7.2 16.9l.9-5.4L4.2 7.7l5.4-.8L12 2z" fill="#4472C4"/></svg>
        <span class="tb-label">图标</span>
      </button>
      <div v-if="showIconPicker" class="popover">
        <div class="pop-title">图标</div>
        <div class="icon-grid">
          <div
            v-for="ic in ICON_LIST"
            :key="ic.name"
            class="icon-cell"
            :title="ic.label"
            @click="addIcon(ic.name)"
          >
            <svg viewBox="0 0 24 24" width="22" height="22">
              <path v-for="(d, i) in ic.paths" :key="i" :d="d" fill="#4472C4" />
            </svg>
          </div>
        </div>
      </div>
    </div>

    <button class="tool-btn labeled" title="公式" @click="addFormula">
      <span class="tb-formula">ƒ∑</span>
      <span class="tb-label">公式</span>
    </button>

    <span class="sep" />
    <div class="ctrl-group">
      <button class="tool-btn labeled preview" title="预览 (F5)" @click="openPreview">
        <span class="tb-preview-icon">▶</span>
        <span class="tb-label">预览</span>
      </button>
    </div>

    <div v-if="imageError" class="toolbar-error">{{ imageError }}</div>
  </div>

  <!-- 全屏演示 -->
  <teleport to="body">
    <div
      v-if="showPreview"
      ref="previewMaskRef"
      class="preview-stage"
      @wheel="onPreviewWheel"
      @click="onPreviewClick"
      @contextmenu="closePreview"
    >
      <div
        class="preview-core"
        :style="{
          width: `${slides.canvasWidthPx}px`,
          height: `${slides.canvasHeightPx}px`,
          transform: `scale(${previewScale})`,
        }"
      >
        <SlideView
          v-if="slides.slides[previewIndex]"
          :slide="slides.slides[previewIndex]"
          :width-px="slides.canvasWidthPx"
          :height-px="slides.canvasHeightPx"
          mode="view"
        />
      </div>
      <div class="preview-hint">{{ previewIndex + 1 }} / {{ slides.slides.length }}</div>
      <div class="preview-guide">←→ 翻页 · Enter 下一张 · Esc 退出</div>
    </div>
  </teleport>
</template>

<style scoped>
.insert-toolbar {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  padding: 0 6px;
  width: 100%;
}
.tool-btn {
  height: 36px;
  padding: 0 10px;
  border: 1px solid transparent;
  background: transparent;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
}
.tool-btn:hover {
  background: #eef3fb;
  border-color: #cfe0f7;
}
.tool-btn:hover svg path {
  fill: #2b4d8f;
}
.tool-btn:hover svg rect {
  fill: #2b4d8f;
}
.tb-label {
  font-size: 13px;
  color: #333;
  white-space: nowrap;
}
.tb-icon {
  font-size: 14px;
  font-weight: bold;
  color: #4472c4;
}
.tb-formula {
  font-size: 14px;
  font-style: italic;
  font-family: 'Cambria Math', serif;
  color: #4472c4;
}
.tb-svg {
  width: 20px;
  height: 20px;
}
.wrap {
  position: relative;
}
.popover {
  position: absolute;
  top: 40px;
  left: 50%;
  transform: translateX(-50%);
  background: #fff;
  border: 1px solid #e0e0e0;
  box-shadow: 0 6px 18px rgba(0, 0, 0, 0.14);
  padding: 10px;
  z-index: 200;
  width: 260px;
  max-height: 280px;
  overflow-y: auto;
}
.pop-title {
  font-size: 12px;
  color: #888;
  margin-bottom: 8px;
}
.shape-grid {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 4px;
}
.shape-item {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px;
  cursor: pointer;
}
.shape-item:hover {
  background: #eef3fb;
}
.chart-list {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.chart-item {
  padding: 6px 10px;
  cursor: pointer;
  font-size: 13px;
}
.chart-item:hover {
  background: #eef3fb;
}
.text-popover {
  width: 110px;
  min-width: 110px;
}
.text-dir-item {
  padding: 7px 10px;
  cursor: pointer;
  font-size: 13px;
  border-radius: 4px;
}
.text-dir-item:hover {
  background: #eef3fb;
}
.icon-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 6px;
}
.icon-cell {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 6px;
  border: 1px solid transparent;
  cursor: pointer;
}
.icon-cell:hover {
  background: #eef3fb;
}
.toolbar-error {
  position: absolute;
  bottom: 40px;
  left: 50%;
  transform: translateX(-50%);
  background: #fdecea;
  color: #c0392b;
  font-size: 12px;
  padding: 4px 10px;
  white-space: nowrap;
  z-index: 300;
}
.sep {
  width: 1px;
  height: 22px;
  background: #e0e0e0;
  margin: 0 4px;
}
.ctrl-group {
  display: flex;
  align-items: center;
  gap: 3px;
}
.tool-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.zoom-val {
  font-size: 12px;
  color: #666;
  min-width: 36px;
  text-align: center;
}
.tool-btn.preview {
  background: transparent;
  border-color: transparent;
  color: inherit;
}
.tool-btn.preview:hover {
  background: #eef3fb;
}
.tool-btn.preview svg path {
  fill: #2b4d8f;
}
.tb-preview-icon {
  color: #333;
}

/* 全屏演示 */
.preview-stage {
  position: fixed;
  inset: 0;
  background: #000;
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
  overflow: hidden;
  cursor: default;
  user-select: none;
}
.preview-core {
  flex-shrink: 0;
  transform-origin: center center;
  background: #000;
}
.preview-core :deep(.slide-view) {
  width: auto;
  height: auto;
}
.preview-hint {
  position: absolute;
  right: 24px;
  bottom: 20px;
  font-size: 13px;
  color: rgba(255, 255, 255, 0.5);
  letter-spacing: 1px;
}
.preview-guide {
  position: absolute;
  left: 50%;
  bottom: 20px;
  transform: translateX(-50%);
  font-size: 12px;
  color: rgba(255, 255, 255, 0.3);
  letter-spacing: 0.5px;
}
</style>
