<script setup lang="ts">
// 左侧缩略图列表（支持右键菜单：添加/复制/删除）
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useSlidesStore } from '@/stores/slides'
import { useEditorStore } from '@/stores/editor'
import { useSnapshotStore } from '@/stores/snapshot'
import { useCommentsStore } from '@/stores/comments'
import { uploadPptx } from '@/api'
import SlideView from './canvas/SlideView.vue'

const slides = useSlidesStore()
const editor = useEditorStore()
const snapshot = useSnapshotStore()
const comments = useCommentsStore()

// === 上传 ===
const fileInput = ref<HTMLInputElement | null>(null)
const uploading = ref(false)
const errorMsg = ref('')

async function onFileChange(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  uploading.value = true
  errorMsg.value = ''
  try {
    const overview = await uploadPptx(file)
    slides.loadOverview(overview)
    editor.clearSelection()
    snapshot.flush()
    comments.stop()
  } catch (err) {
    errorMsg.value = `上传失败: ${(err as Error).message}`
  } finally {
    uploading.value = false
    input.value = ''
  }
}

/** 缩略图尺寸：固定宽 140px，按比例缩放 */
const thumbW = 140
const thumbH = computed(() => (thumbW * slides.canvasHeightPx) / slides.canvasWidthPx)

/** 缩略缩放：容器用逻辑尺寸 + transform scale（与主画布一致），外层裁剪到 thumbW×thumbH */
const thumbStyle = computed(() => ({
  width: `${slides.canvasWidthPx}px`,
  height: `${slides.canvasHeightPx}px`,
  transform: `scale(${thumbW / slides.canvasWidthPx})`,
}))

const currentSlideNo = computed(() => slides.slideIndex + 1)

// === 右键菜单 ===
const ctxMenu = ref<{ x: number; y: number; index: number } | null>(null)

function openCtx(e: MouseEvent, index: number) {
  e.preventDefault()
  slides.setSlideIndex(index)
  // 限制菜单不超出视口
  const menuW = 132
  const menuH = 100
  ctxMenu.value = {
    x: Math.min(e.clientX, window.innerWidth - menuW - 4),
    y: Math.min(e.clientY, window.innerHeight - menuH - 4),
    index,
  }
}

function closeCtx() {
  ctxMenu.value = null
}

function ctxAdd(index: number) {
  slides.setSlideIndex(index)
  slides.addSlide(true)
  closeCtx()
}
function ctxCopy(index: number) {
  slides.setSlideIndex(index)
  slides.addSlide(false)
  closeCtx()
}
function ctxDelete(index: number) {
  slides.setSlideIndex(index)
  slides.deleteSlide()
  closeCtx()
}

function onDocMouseDown(e: MouseEvent) {
  const t = e.target as Node
  const menu = document.querySelector('.thumb-ctx-menu')
  if (ctxMenu.value && menu && !menu.contains(t)) closeCtx()
  if (ctxMenu.value && !menu) closeCtx()
}

function onDocKey(e: KeyboardEvent) {
  if (e.key === 'Escape') closeCtx()
}

onMounted(() => {
  document.addEventListener('mousedown', onDocMouseDown)
  document.addEventListener('keydown', onDocKey)
})
onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onDocMouseDown)
  document.removeEventListener('keydown', onDocKey)
})
</script>

<template>
  <aside class="thumbnails">
    <div class="thumb-header">
      <input ref="fileInput" type="file" accept=".pptx" hidden @change="onFileChange" />
      <button class="thumb-open" :disabled="uploading" @click="fileInput?.click()">
        <svg viewBox="0 0 20 20" width="14" height="14"><path d="M3 10a7 7 0 0 1 12-5m-2-2 2 2-2 2M17 10a7 7 0 0 1-12 5m2 2-2-2 2-2" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
        {{ uploading ? '打开中…' : '打开' }}
      </button>
      <button class="thumb-add" :disabled="!slides.loaded" @click="slides.addSlide(true)">
        <svg viewBox="0 0 18 18" width="14" height="14"><path d="M9 2v14M2 9h14" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>
        添加
      </button>
    </div>
    <div v-if="errorMsg" class="thumb-error">{{ errorMsg }}</div>
    <div class="thumb-list">
      <div
        v-for="(slide, i) in slides.slides"
        :key="i"
        class="thumb-item"
        :class="{ active: i === slides.slideIndex }"
        @click="slides.setSlideIndex(i)"
        @contextmenu="openCtx($event, i)"
      >
        <div class="thumb-number">{{ i + 1 }}</div>
        <div class="thumb-frame" :style="{ width: `${thumbW}px`, height: `${thumbH}px` }">
          <div class="thumb-scale" :style="thumbStyle">
            <SlideView :slide="slide" :width-px="slides.canvasWidthPx" :height-px="slides.canvasHeightPx" />
          </div>
        </div>
      </div>
    </div>
    <div class="thumb-footer">
      <div class="thumb-info">共 {{ slides.slides.length }} 页 · 第 {{ currentSlideNo }} 页</div>
    </div>
  </aside>

  <teleport to="body">
    <div
      v-if="ctxMenu"
      class="thumb-ctx-menu"
      :style="{ left: `${ctxMenu.x}px`, top: `${ctxMenu.y}px` }"
      @mousedown.stop
    >
      <div class="ctx-item" @click="ctxAdd(ctxMenu.index)">添加幻灯片</div>
      <div class="ctx-item" @click="ctxCopy(ctxMenu.index)">复制此页</div>
      <div class="ctx-item danger" :class="{ disabled: slides.slides.length <= 1 }" @click="ctxDelete(ctxMenu.index)">
        <span v-if="slides.slides.length <= 1" class="ctx-tip">至少保留一页</span>
        删除此页
      </div>
    </div>
  </teleport>
</template>

<style scoped>
.thumbnails {
  width: 164px;
  background: #fafafa;
  border-right: 1px solid #e5e5e5;
  display: flex;
  flex-direction: column;
  box-sizing: border-box;
  flex-shrink: 0;
}
.thumb-list {
  flex: 1;
  overflow-y: auto;
  padding: 10px 6px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}
.thumb-footer {
  flex-shrink: 0;
  border-top: 1px solid #e5e5e5;
  padding: 8px 10px;
  background: #f4f4f4;
  display: flex;
  justify-content: center;
}
.thumb-item {
  position: relative;
  cursor: pointer;
  border: 2px solid transparent;
  padding: 2px;
}
.thumb-item.active {
  border-color: #4472c4;
}
.thumb-number {
  position: absolute;
  top: 4px;
  left: 6px;
  font-size: 11px;
  color: #888;
  z-index: 5;
  background: rgba(255, 255, 255, 0.7);
  padding: 0 4px;
}
.thumb-frame {
  overflow: hidden;
  background: #fff;
  position: relative;
  flex-shrink: 0;
}
.thumb-scale {
  position: absolute;
  top: 0;
  left: 0;
  transform-origin: top left;
}
.thumb-header {
  flex-shrink: 0;
  padding: 10px;
  border-bottom: 1px solid #e5e5e5;
  background: #fafafa;
  display: flex;
  gap: 6px;
}
.thumb-open {
  flex: 1;
  height: 34px;
  border: 1px solid #e0e3e8;
  background: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  color: #333;
  cursor: pointer;
  font-size: 13px;
  border-radius: 4px;
}
.thumb-open:hover:not(:disabled) {
  border-color: #4472c4;
  color: #4472c4;
}
.thumb-open:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
.thumb-add {
  flex: 1;
  height: 34px;
  border: 1px dashed #bbb;
  background: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  color: #888;
  cursor: pointer;
  font-size: 13px;
  border-radius: 4px;
}
.thumb-add:hover {
  border-color: #4472c4;
  color: #4472c4;
}
.thumb-add:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.thumb-error {
  flex-shrink: 0;
  padding: 4px 10px;
  font-size: 11px;
  color: #c0392b;
  background: #fdecea;
  border-bottom: 1px solid #f3c2c4;
}
.thumb-info {
  font-size: 11px;
  color: #999;
}
</style>

<style>
/* 右键菜单（teleport 到 body，需全局样式） */
.thumb-ctx-menu {
  position: fixed;
  z-index: 1000;
  background: #fff;
  border: 1px solid #e3e5e9;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.14);
  padding: 5px;
  border-radius: 6px;
  min-width: 130px;
  user-select: none;
}
.thumb-ctx-menu .ctx-item {
  padding: 7px 12px;
  font-size: 13px;
  color: #333;
  cursor: pointer;
  border-radius: 4px;
  position: relative;
}
.thumb-ctx-menu .ctx-item:hover {
  background: #eef3fb;
}
.thumb-ctx-menu .ctx-item.danger {
  color: #e5484d;
}
.thumb-ctx-menu .ctx-item.danger:hover {
  background: #fdecea;
}
.thumb-ctx-menu .ctx-item.disabled {
  color: #bbb;
  cursor: not-allowed;
  pointer-events: none;
}
.thumb-ctx-menu .ctx-tip {
  position: absolute;
  right: 12px;
  top: 9px;
  font-size: 10px;
  color: #bbb;
}
</style>
