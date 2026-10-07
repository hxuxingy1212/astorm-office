<script setup lang="ts">
// 应用主布局
import { computed, onMounted } from 'vue'
import { useSlidesStore } from '@/stores/slides'
import { fetchOverview } from '@/api'
import Thumbnails from '@/components/Thumbnails.vue'
import CanvasView from '@/components/canvas/CanvasView.vue'
import SlideView from '@/components/canvas/SlideView.vue'
import ViewerToolbar from '@/components/ViewerToolbar.vue'
import AnnotationCard from '@/components/AnnotationCard.vue'
import { useCommentsStore } from '@/stores/comments'
import { inchToPx } from '@/utils/convert'

const slides = useSlidesStore()
const comments = useCommentsStore()

/** 渲染模式（无头浏览器截屏用）：?render=1&slide=N，隐藏所有编辑 UI，仅显示指定页 */
const params = new URLSearchParams(window.location.search)
const renderMode = computed(() => params.get('render') === '1')
const renderSlide = computed(() => Number(params.get('slide') ?? 1))

/** 渲染模式下目标页（0-based） */
const renderIndex = computed(() => Math.max(0, Math.min(renderSlide.value - 1, slides.slides.length - 1)))

/** 渲染页的像素尺寸（原生 96dpi），配合截屏窗口大小 */
const renderW = computed(() => inchToPx(slides.width))
const renderH = computed(() => inchToPx(slides.height))

/** 页面刷新时恢复后端会话 */
onMounted(async () => {
  try {
    const overview = await fetchOverview()
    if (overview && overview.slides && overview.slides.length > 0) {
      slides.loadOverview(overview)
    }
  } catch {
    // 无会话：等待用户上传
  }
})
</script>

<template>
  <div class="app">
    <!-- 渲染模式：仅显示单个幻灯片，干净页面 -->
    <template v-if="renderMode">
      <div class="render-root" v-if="slides.loaded">
        <SlideView
          :slide="slides.slides[renderIndex] ?? slides.slides[0]"
          :width-px="renderW"
          :height-px="renderH"
          mode="view"
        />
      </div>
      <div v-else class="render-empty">加载中…</div>
    </template>

    <template v-else>
      <div class="app-body">
        <Thumbnails />
        <div class="canvas-area">
          <div v-if="!slides.loaded" class="empty-state">
            <div class="empty-icon">📊</div>
            <p>上传 PPTX 文件开始预览与编辑</p>
            <p class="sub">点击左侧「打开」选择 PPTX，文件由 json2pptx 解包为中间产物</p>
          </div>
          <CanvasView v-else />
        </div>
      </div>
      <ViewerToolbar v-if="slides.loaded" />
      <AnnotationCard v-if="comments.active && comments.targetPath" />
    </template>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}
html,
body,
#app {
  height: 100%;
  font-family: 'PingFang SC', 'Microsoft YaHei', 'Helvetica Neue', Arial, sans-serif;
  font-size: 14px;
  color: #333;
}
</style>

<style scoped>
.app {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.app-body {
  flex: 1;
  display: flex;
  overflow: hidden;
}
.canvas-area {
  flex: 1;
  position: relative;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.empty-state {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: #888;
}
.empty-icon {
  font-size: 56px;
  margin-bottom: 16px;
}
.empty-state .sub {
  font-size: 12px;
  margin-top: 8px;
  color: #aaa;
}
.render-root {
  width: 100%;
  height: 100%;
  background: #fff;
  overflow: hidden;
  display: flex;
  align-items: flex-start;
  justify-content: flex-start;
}
.render-root :deep(.slide-view) {
  box-shadow: none;
}
.render-empty {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #888;
}
</style>

<style>
/* 面板通用样式（非 scoped，供各面板共用） */
.panel h4 {
  font-size: 13px;
  color: #666;
  margin-bottom: 8px;
  border-bottom: 1px solid #f0f0f0;
  padding-bottom: 6px;
}
.panel label {
  display: block;
  font-size: 12px;
  color: #888;
  margin: 8px 0 3px;
}
.panel .row {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 6px;
  margin: 8px 0;
}
.panel .row label {
  margin: 0;
  min-width: 44px;
}
.panel input[type='text'],
.panel input[type='number'],
.panel textarea,
.panel select {
  width: 100%;
  padding: 4px 6px;
  border: 1px solid #d8d8d8;
  font-size: 13px;
  background: #fff;
  outline: none;
}
.panel input[type='color'] {
  width: 34px;
  height: 26px;
  border: 1px solid #d8d8d8;
  padding: 0;
  background: #fff;
  outline: none;
}
.panel input[type='text']:focus,
.panel input[type='number']:focus,
.panel textarea:focus,
.panel select:focus,
.panel input[type='color']:focus,
.panel input[type='text']:focus-visible,
.panel input[type='number']:focus-visible,
.panel textarea:focus-visible,
.panel select:focus-visible,
.panel input[type='color']:focus-visible {
  outline: none;
  border-color: #4472c4;
}
.panel textarea {
  resize: vertical;
}
.panel .btn-group {
  display: flex;
  gap: 4px;
}
.panel .btn-group button {
  width: 30px;
  height: 28px;
  border: 1px solid #d8d8d8;
  background: #fff;
  cursor: pointer;
  font-weight: bold;
}
.panel .btn-group button.active {
  background: #4472c4;
  color: #fff;
  border-color: #4472c4;
}
.panel button.small {
  padding: 3px 8px;
  font-size: 12px;
  border: 1px solid #d8d8d8;
  background: #fff;
  cursor: pointer;
}
.panel button.primary {
  padding: 4px 12px;
  font-size: 12px;
  background: #4472c4;
  color: #fff;
  border: none;
  cursor: pointer;
}
.notes-editor {
  margin-top: 6px;
}
</style>
