<script setup lang="ts">
// 画布容器：缩放 + 元素选中/拖拽/缩放/旋转 + 操作层
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { useEditorStore } from '@/stores/editor'
import { useSlidesStore, parsePath, getElementByPath } from '@/stores/slides'
import { useSnapshotStore } from '@/stores/snapshot'
import { useCommentsStore } from '@/stores/comments'
import { inchToPx, pxToInch, round2 } from '@/utils/convert'
import { elementRotation, createTextElement } from '@/utils/elements'
import { elementCliPath } from '@/utils/path'
import type { Element } from '@/types/presentation'
import SlideView from './SlideView.vue'
import OperateLayer from './OperateLayer.vue'

const editor = useEditorStore()
const slides = useSlidesStore()
const snapshot = useSnapshotStore()
const comments = useCommentsStore()

const viewportRef = ref<HTMLDivElement | null>(null)

const logicW = computed(() => slides.canvasWidthPx)
const logicH = computed(() => slides.canvasHeightPx)

/** 拖拽插入选区的显示样式（逻辑像素） */
const drawRectStyle = computed(() => {
  if (!drawRect.value) return {}
  return {
    left: `${drawRect.value.x}px`,
    top: `${drawRect.value.y}px`,
    width: `${drawRect.value.w}px`,
    height: `${drawRect.value.h}px`,
  }
})

/** 批注目标元素（仅批注模式） */
const hoverPath = ref('')

/** 当前高亮路径：悬停优先，其次已点击固定的目标 */
const activeCommentPath = computed(() => hoverPath.value || comments.targetPath)

const commentTargetEl = computed<Element | null>(() => {
  if (!comments.active || !activeCommentPath.value) return null
  return getElementByPath(slides.currentSlide?.elements ?? [], parsePath(activeCommentPath.value))
})

/** 批注目标元素的高亮框样式 */
const commentTargetStyle = computed(() => {
  const el = commentTargetEl.value
  if (!el) return {}
  return {
    left: `${inchToPx(el.position.x)}px`,
    top: `${inchToPx(el.position.y)}px`,
    width: `${inchToPx(el.position.w)}px`,
    height: `${inchToPx(el.position.h)}px`,
  }
})

/** 批注目标的 CLI 路径（/slide[N]/type[K]） */
const commentCliPath = computed(() =>
  elementCliPath(slides.slideIndex, activeCommentPath.value, slides.slides),
)

/** 悬停命中元素：读取 data-el-path 更新高亮（仅选择模式） */
function onViewportMouseMove(e: MouseEvent) {
  if (!comments.active) {
    if (hoverPath.value) hoverPath.value = ''
    return
  }
  const target = e.target as HTMLElement | null
  const host = target?.closest?.('[data-el-path]') as HTMLElement | null
  const next = host?.getAttribute('data-el-path') ?? ''
  if (next !== hoverPath.value) hoverPath.value = next
}

watch(
  () => comments.active,
  (v) => {
    if (!v) hoverPath.value = ''
  },
)

/** 元素屏幕坐标 → 逻辑坐标（自动处理缩放） */
function eventToLogic(e: MouseEvent): { x: number; y: number } {
  const rect = viewportRef.value!.getBoundingClientRect()
  const scale = rect.width / logicW.value
  return {
    x: (e.clientX - rect.left) / scale,
    y: (e.clientY - rect.top) / scale,
  }
}

// === 选中 ===

function onElementMouseDown(e: MouseEvent, path: string) {
  if (e.button !== 0) return
  if (comments.active) {
    comments.setTarget(slides.slideIndex, path)
    return
  }
  if (editor.drawMode) {
    startDrawArea(e)
    return
  }
  const additive = e.shiftKey || e.ctrlKey || e.metaKey
  if (!additive && !editor.activeElementIds.includes(path)) {
    editor.select(path)
  } else if (additive) {
    editor.select(path, true)
  } else {
    editor.handleElementId = path
  }
  startDrag(e)
}

// === 拖拽绘制插入（文本框等）===

const drawRect = ref<{ x: number; y: number; w: number; h: number } | null>(null)
let drawing = false
let drawStart = { x: 0, y: 0 }

function startDrawArea(e: MouseEvent) {
  const start = eventToLogic(e)
  drawing = true
  drawStart = start
  drawRect.value = { x: start.x, y: start.y, w: 0, h: 0 }
  document.addEventListener('mousemove', onDrawMove)
  document.addEventListener('mouseup', onDrawEnd)
}

function onDrawMove(e: MouseEvent) {
  if (!drawing) return
  const cur = eventToLogic(e)
  drawRect.value = {
    x: Math.min(drawStart.x, cur.x),
    y: Math.min(drawStart.y, cur.y),
    w: Math.abs(cur.x - drawStart.x),
    h: Math.abs(cur.y - drawStart.y),
  }
}

function onDrawEnd() {
  document.removeEventListener('mousemove', onDrawMove)
  document.removeEventListener('mouseup', onDrawEnd)
  drawing = false
  const rect = drawRect.value
  const mode = editor.drawMode
  drawRect.value = null
  editor.stopDraw()
  if (!mode || !slides.currentSlide) return
  if (!rect || rect.w < 0.2 || rect.h < 0.2) return
  const x = round2(rect.x)
  const y = round2(rect.y)
  const w = round2(rect.w)
  const h = round2(rect.h)
  let el: Element | null = null
  if (mode.type === 'text') el = createTextElement(x, y, w, h, mode.vert)
  if (!el) return
  slides.addElement('', el)
  const lastPath = `${slides.currentSlide.elements.length - 1}`
  editor.select(lastPath)
  snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
}

// === 拖拽 ===

let dragState: {
  startX: number
  startY: number
  origPos: { x: number; y: number }
  moved: boolean
} | null = null

function startDrag(e: MouseEvent) {
  const path = editor.handleElementId
  const el = getElementByPath(slides.currentSlide?.elements ?? [], parsePath(path))
  if (!el) return
  const start = eventToLogic(e)
  dragState = {
    startX: start.x,
    startY: start.y,
    origPos: { ...el.position },
    moved: false,
  }
  editor.isScaling = true
  document.addEventListener('mousemove', onDragMove)
  document.addEventListener('mouseup', onDragEnd)
}

function onDragMove(e: MouseEvent) {
  if (!dragState) return
  const el = getElementByPath(slides.currentSlide?.elements ?? [], parsePath(editor.handleElementId))
  if (!el) return
  const cur = eventToLogic(e)
  let dx = cur.x - dragState.startX
  let dy = cur.y - dragState.startY
  // Shift 锁定水平/垂直
  if (e.shiftKey) {
    if (Math.abs(dx) > Math.abs(dy)) dy = 0
    else dx = 0
  }
  if (Math.abs(dx) > 0.5 || Math.abs(dy) > 0.5) dragState.moved = true
  const pos = {
    x: round2(dragState.origPos.x + pxToInch(dx)),
    y: round2(dragState.origPos.y + pxToInch(dy)),
  }
  // 越界限制
  pos.x = Math.max(-el.position.w, Math.min(pos.x, pxToInch(logicW.value)))
  pos.y = Math.max(-el.position.h, Math.min(pos.y, pxToInch(logicH.value)))
  slides.updateElement(editor.handleElementId, { position: { ...el.position, ...pos } })
}

function onDragEnd() {
  document.removeEventListener('mousemove', onDragMove)
  document.removeEventListener('mouseup', onDragEnd)
  dragState = null
  editor.isScaling = false
  snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
}

// === 缩放 ===

let scaleState: {
  dir: string
  startX: number
  startY: number
  orig: { x: number; y: number; w: number; h: number }
  el: Element
  fixedRatio: boolean
} | null = null

function onScaleStart(dir: string, e: MouseEvent) {
  const el = getElementByPath(slides.currentSlide?.elements ?? [], parsePath(editor.handleElementId))
  if (!el) return
  const start = eventToLogic(e)
  scaleState = {
    dir,
    startX: start.x,
    startY: start.y,
    orig: { ...el.position },
    el,
    fixedRatio: e.shiftKey || e.ctrlKey || e.metaKey,
  }
  editor.isScaling = true
  document.addEventListener('mousemove', onScaleMove)
  document.addEventListener('mouseup', onScaleEnd)
}

function onScaleMove(e: MouseEvent) {
  if (!scaleState) return
  const cur = eventToLogic(e)
  const dx = cur.x - scaleState.startX
  const dy = cur.y - scaleState.startY
  const { orig, dir } = scaleState
  let { x, y, w, h } = orig

  const ratio = orig.w > 0 && orig.h > 0 ? orig.h / orig.w : 1
  // 等比缩放：以 dx 为基准计算 dy
  let effDx = dx
  let effDy = dy
  if (scaleState.fixedRatio) {
    const d = Math.max(Math.abs(dx), Math.abs(dy)) * (dx * dy >= 0 ? 1 : -1)
    effDx = d
    effDy = d * ratio
    // 方向翻转修正
    if (dir.includes('n') !== dir.includes('s')) effDy = -Math.abs(effDy) * (dir.includes('n') ? -1 : 1)
    if (dir.includes('w') !== dir.includes('e')) effDx = -Math.abs(effDx) * (dir.includes('w') ? -1 : 1)
  }

  if (dir.includes('e')) w = Math.max(0.05, orig.w + effDx)
  if (dir.includes('s')) h = Math.max(0.05, orig.h + effDy)
  if (dir.includes('w')) {
    const nw = Math.max(0.05, orig.w - effDx)
    x = orig.x + (orig.w - nw)
    w = nw
  }
  if (dir.includes('n')) {
    const nh = Math.max(0.05, orig.h - effDy)
    y = orig.y + (orig.h - nh)
    h = nh
  }
  slides.updateElement(editor.handleElementId, {
    position: { x: round2(x), y: round2(y), w: round2(w), h: round2(h) },
  })
}

function onScaleEnd() {
  document.removeEventListener('mousemove', onScaleMove)
  document.removeEventListener('mouseup', onScaleEnd)
  scaleState = null
  editor.isScaling = false
  snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
}

// === 旋转 ===

function onRotateStart(e: MouseEvent) {
  const el = getElementByPath(slides.currentSlide?.elements ?? [], parsePath(editor.handleElementId))
  if (!el) return
  const start = eventToLogic(e)
  const center = {
    x: inchToPx(el.position.x) + inchToPx(el.position.w) / 2,
    y: inchToPx(el.position.y) + inchToPx(el.position.h) / 2,
  }
  const origRot = elementRotation(el) ?? 0
  const origAng = (Math.atan2(start.y - center.y, start.x - center.x) * 180) / Math.PI
  editor.isScaling = true

  const onMove = (ev: MouseEvent) => {
    const cur = eventToLogic(ev)
    const ang = (Math.atan2(cur.y - center.y, cur.x - center.x) * 180) / Math.PI
    const rot = round2(origRot + (ang - origAng))
    slides.updateElement(editor.handleElementId, { rotation: rot } as unknown as Partial<Element>)
  }
  const onEnd = () => {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onEnd)
    editor.isScaling = false
    snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
  }
  document.addEventListener('mousemove', onMove)
  document.addEventListener('mouseup', onEnd)
}

// === 文本保存 ===

function onSaveText(path: string, text: string) {
  const el = getElementByPath(slides.currentSlide?.elements ?? [], parsePath(path))
  if (!el) return
  if (el.type === 'text') {
    slides.updateElement(path, { text })
  } else if (el.type === 'shape') {
    slides.updateElement(path, { text })
  }
  snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
}

// === 空白点击 ===

function onCanvasMouseDown(e: MouseEvent) {
  if (comments.active) {
    comments.setTarget(slides.slideIndex, '')
    return
  }
  if (editor.drawMode) {
    startDrawArea(e)
    return
  }
  if (e.target === viewportRef.value) editor.clearSelection()
}

// === 键盘（删除） ===

function onKeyDown(e: KeyboardEvent) {
  const target = e.target as HTMLElement
  if (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT') return
  if (e.key === 'Delete' || e.key === 'Backspace') {
    if (editor.activeElementIds.length === 0) return
    // 从后往前删除避免索引错乱
    const paths = [...editor.activeElementIds].sort((a, b) => b.length - a.length)
    for (const p of paths) slides.deleteElement(p)
    editor.clearSelection()
    snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
  } else if (e.ctrlKey || e.metaKey) {
    if (e.key === 'z' && !e.shiftKey) {
      const snap = snapshot.undo()
      if (snap) {
        slides.slides = snap.slides
        slides.slideIndex = snap.slideIndex
        editor.clearSelection()
      }
    } else if (e.key === 'z' && e.shiftKey) {
      const snap = snapshot.redo()
      if (snap) {
        slides.slides = snap.slides
        slides.slideIndex = snap.slideIndex
        editor.clearSelection()
      }
    } else if (e.key === 'd') {
      // Ctrl+D 复制选中元素
      const el = getElementByPath(slides.currentSlide?.elements ?? [], parsePath(editor.handleElementId))
      if (el) {
        const copy = JSON.parse(JSON.stringify(el)) as Element
        copy.position = { ...copy.position, x: copy.position.x + 0.5, y: copy.position.y + 0.5 }
        slides.addElement('', copy)
        snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
      }
    }
  }
  e.preventDefault()
}

onBeforeUnmount(() => {
  document.removeEventListener('mousemove', onDragMove)
  document.removeEventListener('mouseup', onDragEnd)
  document.removeEventListener('mousemove', onScaleMove)
  document.removeEventListener('mouseup', onScaleEnd)
  document.removeEventListener('mousemove', onDrawMove)
  document.removeEventListener('mouseup', onDrawEnd)
})
</script>

<template>
  <div
    class="canvas-viewport"
    :class="{ drawing: editor.drawMode }"
    @mousedown="onCanvasMouseDown"
    @mousemove="onViewportMouseMove"
    @keydown="onKeyDown"
    tabindex="0"
  >
    <div
      ref="viewportRef"
      class="canvas-scale"
      :style="{
        width: `${logicW}px`,
        height: `${logicH}px`,
        transform: `scale(${editor.canvasScale})`,
      }"
    >
      <SlideView
        v-if="slides.currentSlide"
        :slide="slides.currentSlide"
        :width-px="logicW"
        :height-px="logicH"
        :mode="comments.active ? 'edit' : 'view'"
        @element-mousedown="onElementMouseDown"
        @save-text="onSaveText"
      />
      <!-- 拖拽插入时的选区框 -->
      <div v-if="drawRect" class="draw-selection" :style="drawRectStyle" />
      <!-- 批注目标高亮 -->
      <div v-if="commentTargetEl" class="comment-target-box" :style="commentTargetStyle">
        <span class="comment-target-label">{{ commentCliPath }}</span>
      </div>
      <!-- 操作层（仅编辑模式选中元素，批注模式下不显示拖拽手柄） -->
      <OperateLayer
        v-if="slides.currentSlide && editor.handleElementId && !comments.active"
        :el="getElementByPath(slides.currentSlide.elements, parsePath(editor.handleElementId))!"
        @scale-start="onScaleStart"
        @rotate-start="onRotateStart"
      />
    </div>
  </div>
</template>

<style scoped>
.canvas-viewport {
  flex: 1;
  overflow: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  background: #f0f0f0;
  outline: none;
  padding: 40px;
}
.canvas-scale {
  position: relative;
  transform-origin: center center;
  flex-shrink: 0;
}
.canvas-viewport.drawing {
  cursor: crosshair;
}
.draw-selection {
  position: absolute;
  border: 1.5px dashed #4472c4;
  background: rgba(68, 114, 196, 0.15);
  pointer-events: none;
  z-index: 10;
}
.comment-target-box {
  position: absolute;
  border: 2px solid #f5a623;
  background: rgba(245, 166, 35, 0.12);
  pointer-events: none;
  z-index: 10;
}
.comment-target-label {
  position: absolute;
  top: -26px;
  left: -2px;
  background: #f5a623;
  color: #fff;
  font-size: 11px;
  line-height: 1;
  padding: 5px 8px;
  border-radius: 3px 3px 3px 0;
  white-space: nowrap;
  pointer-events: none;
}
</style>
