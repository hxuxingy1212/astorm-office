<script setup lang="ts">
// 底部悬浮工具条：选择/退出选择 · 翻页 · 缩放 · 下载（参考图红色框样式）
import { computed, ref, watch } from 'vue'
import { useSlidesStore } from '@/stores/slides'
import { useEditorStore } from '@/stores/editor'
import { useCommentsStore } from '@/stores/comments'
import { saveAllSlides, repackAndDownload } from '@/api'

const slides = useSlidesStore()
const editor = useEditorStore()
const comments = useCommentsStore()

const total = computed(() => slides.slides.length)
const current = computed(() => slides.slideIndex + 1)

// 页码输入：与当前页联动
const pageInput = ref(current.value)
watch(current, (v) => (pageInput.value = v))

function applyPage() {
  let v = Number(pageInput.value)
  if (!Number.isFinite(v)) v = current.value
  v = Math.max(1, Math.min(total.value || 1, Math.round(v)))
  slides.setSlideIndex(v - 1)
}

function prev() {
  if (slides.slideIndex > 0) slides.setSlideIndex(slides.slideIndex - 1)
}
function next() {
  if (slides.slideIndex < total.value - 1) slides.setSlideIndex(slides.slideIndex + 1)
}

function toggleSelect() {
  if (comments.active) comments.stop()
  else comments.start()
}

async function download() {
  if (!slides.loaded) return
  try {
    await saveAllSlides(slides.slides)
    await repackAndDownload(slides.name)
  } catch (err) {
    alert(`导出失败: ${(err as Error).message}`)
  }
}
</script>

<template>
  <div class="viewer-toolbar">
    <!-- 选择 / 退出选择 -->
    <button class="vt-btn vt-select" :class="{ active: comments.active }" @click="toggleSelect">
      <svg viewBox="0 0 20 20" width="15" height="15"><path d="M5 2l12 8-5.2.7 3 6.3-2.2 1-3-6.3L5 15.6z" fill="currentColor"/></svg>
      <span>{{ comments.active ? '退出选择' : '选择' }}</span>
    </button>

    <div class="vt-sep" />

    <!-- 翻页 -->
    <button class="vt-icon" :disabled="slides.slideIndex <= 0" title="上一页" @click="prev">
      <svg viewBox="0 0 16 16" width="14" height="14"><path d="M10 3 5 8l5 5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
    </button>
    <input
      class="vt-page"
      type="number"
      v-model="pageInput"
      min="1"
      :max="total || 1"
      :disabled="!slides.loaded"
      @change="applyPage"
      @keydown.enter="applyPage"
    />
    <span class="vt-total">/ {{ total }}</span>
    <button class="vt-icon" :disabled="slides.slideIndex >= total - 1" title="下一页" @click="next">
      <svg viewBox="0 0 16 16" width="14" height="14"><path d="M6 3l5 5-5 5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>
    </button>

    <div class="vt-sep" />

    <!-- 缩放 -->
    <button class="vt-icon" :disabled="!slides.loaded" title="缩小" @click="editor.zoomOut()">
      <svg viewBox="0 0 16 16" width="12" height="12"><path d="M3 8h10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
    </button>
    <button class="vt-btn vt-zoom-val" title="缩放">
      <svg viewBox="0 0 20 20" width="14" height="14"><circle cx="9" cy="9" r="5.5" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M13 13l4 4" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
      {{ editor.canvasPercentage }}%
    </button>
    <button class="vt-icon" :disabled="!slides.loaded" title="放大" @click="editor.zoomIn()">
      <svg viewBox="0 0 16 16" width="12" height="12"><path d="M3 8h10M8 3v10" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
    </button>

    <div class="vt-sep" />

    <!-- 下载 -->
    <button class="vt-icon" :disabled="!slides.loaded" title="下载 PPTX" @click="download">
      <svg viewBox="0 0 20 20" width="16" height="16" fill="none"><path d="M10 3v9m0 0 3.5-3.5M10 12 6.5 8.5M4 15h12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
    </button>
  </div>
</template>

<style scoped>
.viewer-toolbar {
  position: fixed;
  left: 50%;
  bottom: 22px;
  transform: translateX(-50%);
  z-index: 100;
  display: flex;
  align-items: center;
  gap: 6px;
  background: #fff;
  border: 1px solid #e6e8ec;
  border-radius: 999px;
  box-shadow: 0 6px 22px rgba(0, 0, 0, 0.12);
  padding: 6px 12px;
  user-select: none;
}
.vt-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 32px;
  padding: 0 12px;
  border: none;
  background: transparent;
  border-radius: 999px;
  cursor: pointer;
  font-size: 13px;
  color: #333;
  white-space: nowrap;
}
.vt-btn:hover:not(:disabled) {
  background: #f2f4f7;
}
.vt-btn.vt-select.active {
  background: #4472c4;
  color: #fff;
}
.vt-zoom-val {
  padding: 0 4px;
  cursor: default;
}
.vt-zoom-val:hover {
  background: transparent;
}
.vt-icon {
  width: 30px;
  height: 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  border-radius: 50%;
  cursor: pointer;
  color: #444;
}
.vt-icon:hover:not(:disabled) {
  background: #f2f4f7;
}
.vt-icon:disabled {
  opacity: 0.35;
  cursor: not-allowed;
}
.vt-page {
  width: 44px;
  height: 28px;
  text-align: center;
  border: 1px solid #e0e3e8;
  border-radius: 6px;
  font-size: 13px;
  color: #222;
  outline: none;
  -moz-appearance: textfield;
}
.vt-page::-webkit-outer-spin-button,
.vt-page::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}
.vt-page:focus {
  border-color: #4472c4;
}
.vt-total {
  font-size: 13px;
  color: #888;
  margin-right: 4px;
}
.vt-sep {
  width: 1px;
  height: 20px;
  background: #e6e8ec;
  margin: 0 2px;
}
</style>
