<script setup lang="ts">
// 顶栏（Gamma/Canva 风格）：三线设置 / 标题编辑 / 添加幻灯片 / 主题下拉 / 工具条
import { ref, watch, onMounted, onBeforeUnmount } from 'vue'
import { useSlidesStore } from '@/stores/slides'
import { useEditorStore } from '@/stores/editor'
import { useSnapshotStore } from '@/stores/snapshot'
import { uploadPptx, saveAllSlides, repackAndDownload } from '@/api'
import type { Theme } from '@/types/presentation'
import { useCommentsStore } from '@/stores/comments'

const slides = useSlidesStore()
const editor = useEditorStore()
const snapshot = useSnapshotStore()
const comments = useCommentsStore()

const fileInput = ref<HTMLInputElement | null>(null)
const uploading = ref(false)
const saving = ref(false)
const errorMsg = ref('')

// 标题编辑
const title = ref('')
const titleEditing = ref(false)
function saveTitle() {
  titleEditing.value = false
  slides.setDocTitle(title.value.trim())
}

// 三线设置菜单
const menuOpen = ref(false)

// 主题下拉
const themeOpen = ref(false)

const menuWrap = ref<HTMLElement | null>(null)
const themeWrap = ref<HTMLElement | null>(null)

/** 点击下拉容器外部时关闭对应菜单 */
function onDocMouseDown(e: MouseEvent) {
  const t = e.target as Node
  if (menuWrap.value && !menuWrap.value.contains(t)) menuOpen.value = false
  if (themeWrap.value && !themeWrap.value.contains(t)) themeOpen.value = false
}
onMounted(() => document.addEventListener('mousedown', onDocMouseDown))
onBeforeUnmount(() => document.removeEventListener('mousedown', onDocMouseDown))

const PRESET_THEMES: { name: string; theme: Theme }[] = [
  { name: '默认蓝', theme: { colors: { accent1: '4472C4', dk1: '000000', lt1: 'FFFFFF' }, major_font: 'Calibri', minor_font: 'Calibri' } },
  { name: '石墨黑', theme: { colors: { accent1: '111827', dk1: '000000', lt1: 'FFFFFF' }, major_font: 'Calibri', minor_font: 'Calibri' } },
  { name: '活力橙', theme: { colors: { accent1: 'ED7D31', dk1: '000000', lt1: 'FFFFFF' }, major_font: 'Calibri', minor_font: 'Calibri' } },
  { name: '清新绿', theme: { colors: { accent1: '2DD4BF', dk1: '1F2937', lt1: 'FFFFFF' }, major_font: 'Calibri', minor_font: 'Calibri' } },
  { name: '典雅紫', theme: { colors: { accent1: '7C3AED', dk1: '000000', lt1: 'FFFFFF' }, major_font: 'Calibri', minor_font: 'Calibri' } },
  { name: '玫瑰粉', theme: { colors: { accent1: 'E11D48', dk1: '000000', lt1: 'FFFFFF' }, major_font: 'Calibri', minor_font: 'Calibri' } },
]

const currentThemeName = ref('默认蓝')

async function onFileChange(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  uploading.value = true
  errorMsg.value = ''
  try {
    const overview = await uploadPptx(file)
    slides.loadOverview(overview)
    title.value = overview.meta?.title ?? overview.name
    editor.clearSelection()
    snapshot.flush()
  } catch (err) {
    errorMsg.value = `上传失败: ${(err as Error).message}`
  } finally {
    uploading.value = false
    input.value = ''
  }
}

async function save() {
  if (!slides.loaded) return
  saving.value = true
  errorMsg.value = ''
  try {
    await saveAllSlides(slides.slides)
  } catch (err) {
    errorMsg.value = `保存失败: ${(err as Error).message}`
  } finally {
    saving.value = false
  }
}

async function download() {
  if (!slides.loaded) return
  try {
    await saveAllSlides(slides.slides)
    await repackAndDownload(slides.name)
  } catch (err) {
    errorMsg.value = `导出失败: ${(err as Error).message}`
  }
}

function undo() {
  const snap = snapshot.undo()
  if (snap) {
    slides.slides = snap.slides
    slides.slideIndex = snap.slideIndex
    editor.clearSelection()
  }
}

function redo() {
  const snap = snapshot.redo()
  if (snap) {
    slides.slides = snap.slides
    slides.slideIndex = snap.slideIndex
    editor.clearSelection()
  }
}

function applyTheme(name: string, theme: Theme) {
  slides.applyTheme(theme)
  currentThemeName.value = name
  themeOpen.value = false
}

/** 切换批注模式（清空编辑器选择，避免显示拖拽手柄） */
function toggleCommentMode() {
  editor.clearSelection()
  if (comments.active) comments.stop()
  else comments.start()
}

// 加载/上传后同步标题
watch(
  () => slides.loaded,
  (v) => {
    if (v) title.value = slides.meta?.title ?? slides.name
  },
  { immediate: true },
)
</script>

<template>
  <header class="app-header">
    <!-- 三线设置 -->
    <div class="header-left">
      <div class="menu-wrap" ref="menuWrap">
        <button class="icon-btn hov" title="菜单" @click="menuOpen = !menuOpen">
          <svg viewBox="0 0 20 20" width="18" height="18"><path d="M3 5h14M3 10h14M3 15h14" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
        </button>
        <transition name="fade">
          <div v-if="menuOpen" class="dropdown menu-dropdown">
            <input ref="fileInput" type="file" accept=".pptx" hidden @change="onFileChange" />
            <button class="menu-item" @click="fileInput?.click()">上传 PPTX…</button>
            <button class="menu-item" :disabled="!slides.loaded || saving" @click="save">保存</button>
            <button class="menu-item" :disabled="!slides.loaded" @click="download">下载 PPTX</button>
            <div class="menu-sep" />
            <button class="menu-item" @click="slides.addSlide(true)">新增空白页</button>
            <button class="menu-item" @click="slides.addSlide(false)">复制当前页</button>
            <button class="menu-item" :disabled="!slides.loaded || slides.slides.length <= 1" @click="slides.deleteSlide()">删除当前页</button>
          </div>
        </transition>
      </div>

      <!-- 标题（左对齐，紧跟设置右侧） -->
      <input
        v-if="titleEditing"
        v-model="title"
        class="title-input editing"
        @blur="saveTitle"
        @keydown.enter="saveTitle"
      />
      <button v-else class="title-input" @click="titleEditing = true" :title="'点击编辑标题'">
        {{ title || (slides.name || '未命名演示文稿') }}
      </button>

      <!-- 主题选择（移到左侧） -->
      <div class="theme-wrap" ref="themeWrap">
        <button class="btn" :disabled="!slides.loaded" @click="themeOpen = !themeOpen">
          <span class="theme-dot" :style="{ background: currentThemeName === '默认蓝' ? '#4472C4' : '#111827' }" />
          {{ currentThemeName }}
        </button>
        <transition name="fade">
          <div v-if="themeOpen" class="dropdown theme-dropdown">
            <div class="dropdown-title">主题</div>
            <button
              v-for="t in PRESET_THEMES"
              :key="t.name"
              class="menu-item theme-pick"
              :class="{ active: currentThemeName === t.name }"
              @click="applyTheme(t.name, t.theme)"
            >
              <span class="theme-dot" :style="{ background: t.theme.colors?.accent1 ?? '#4472C4' }" />
              {{ t.name }}
            </button>
          </div>
        </transition>
      </div>
    </div>

    <!-- 工具条（右侧） -->
    <div class="header-right">
      <span class="sep" />

      <button class="icon-btn" :class="{ active: comments.active }" :disabled="!slides.loaded" title="批注 (选择元素加评论)" @click="toggleCommentMode">
        <svg viewBox="0 0 20 20" width="17" height="17"><path d="M4 3h12a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H9l-4 3v-3H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/></svg>
      </button>

      <button class="icon-btn" :disabled="!snapshot.canUndo" title="撤销 (Ctrl+Z)" @click="undo">
        <svg viewBox="0 0 20 20" width="17" height="17" fill="none"><path d="M9 5 4 10l5 5M4 10h7a5 5 0 0 1 0 10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </button>
      <button class="icon-btn" :disabled="!snapshot.canRedo" title="重做 (Ctrl+Shift+Z)" @click="redo">
        <svg viewBox="0 0 20 20" width="17" height="17" fill="none"><path d="M11 5l5 5-5 5M16 10H9a5 5 0 0 0 0 10" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>
      </button>
      <button class="icon-btn" :disabled="!slides.loaded" title="缩小" @click="editor.zoomOut()">
        <svg viewBox="0 0 18 18" width="15" height="15"><path d="M5 9h8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
      </button>
      <span class="zoom-val">{{ editor.canvasPercentage }}%</span>
      <button class="icon-btn" :disabled="!slides.loaded" title="放大" @click="editor.zoomIn()">
        <svg viewBox="0 0 18 18" width="15" height="15"><path d="M9 5v8M5 9h8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
      </button>
    </div>

    <div v-if="errorMsg" class="error-msg">{{ errorMsg }}</div>
  </header>
</template>

<style scoped>
.app-header {
  height: 52px;
  background: #fff;
  border-bottom: 1px solid #e5e5e5;
  display: flex;
  align-items: center;
  padding: 0 12px;
  gap: 10px;
  position: relative;
}
.header-left {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}
.header-right {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  gap: 8px;
}
.icon-btn {
  width: 34px;
  height: 34px;
  border: 1px solid transparent;
  background: transparent;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #555;
}
.icon-btn.hov:hover,
.icon-btn:hover:not(:disabled) {
  background: #f0f2f5;
}
.icon-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.icon-btn.active {
  color: #4472c4;
  background: #eef3fb;
}
.btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 12px;
  border: 1px solid #d9dce1;
  background: #fff;
  cursor: pointer;
  font-size: 13px;
  color: #333;
}
.btn:hover:not(:disabled) {
  border-color: #c2c7cf;
  background: #f6f8fa;
}
.btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.title-input {
  width: 160px;
  flex-shrink: 0;
  box-sizing: border-box;
  border: 1px solid transparent;
  background: transparent;
  font-size: 15px;
  font-weight: 600;
  color: #1f2329;
  text-align: center;
  padding: 6px 10px;
  cursor: text;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.title-input:hover {
  background: #f6f8fa;
}
.title-input.editing {
  border-color: #4472c4;
  background: #fff;
  outline: none;
  cursor: text;
}
.sep {
  width: 1px;
  height: 20px;
  background: #e3e5e9;
  margin: 0 2px;
}
.zoom-val {
  font-size: 12px;
  color: #666;
  min-width: 36px;
  text-align: center;
}
.menu-wrap,
.theme-wrap {
  position: relative;
}
.dropdown {
  position: absolute;
  top: 42px;
  background: #fff;
  border: 1px solid #e3e5e9;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.12);
  padding: 6px;
  z-index: 100;
  min-width: 180px;
}
.menu-dropdown {
  left: 0;
}
.theme-dropdown {
  left: 0;
  min-width: 170px;
}
.dropdown-title {
  font-size: 12px;
  color: #888;
  padding: 6px 10px;
}
.menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  padding: 8px 10px;
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 13px;
  color: #333;
  text-align: left;
}
.menu-item:hover:not(:disabled) {
  background: #f0f2f5;
}
.menu-item:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.menu-item.active {
  background: #eef3fb;
  color: #4472c4;
}
.menu-sep {
  height: 1px;
  background: #eef0f2;
  margin: 4px 6px;
}
.theme-dot {
  width: 14px;
  height: 14px;
  flex-shrink: 0;
  border: 1px solid rgba(0, 0, 0, 0.1);
}
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.15s ease;
}
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}
.error-msg {
  position: absolute;
  bottom: -30px;
  left: 16px;
  background: #fdecea;
  color: #c0392b;
  font-size: 12px;
  padding: 4px 10px;
  z-index: 100;
}
</style>
