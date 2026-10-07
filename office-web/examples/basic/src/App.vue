<script setup lang="ts">
//! 外部应用接入示例：只 import 包的公开 API（组件 + loader + 样式），
//! 与 `npm i @astorm/office-viewer` 后的写法完全一致。
import { computed, onMounted, ref, shallowRef } from 'vue'
import { OfficeViewer, loadProduct } from '@astorm/office-viewer'
import '@astorm/office-viewer/style.css'

type Kind = 'pptx' | 'docx' | 'xlsx' | 'pdf'

const kind = ref<Kind>('pptx')
const loading = ref(false)
const error = ref('')
const data = shallowRef<unknown>(null)
const hoveredPath = ref<string | null>(null)
const selectedPaths = ref<string[]>([])

const TABS: Record<Kind, string> = {
  pptx: 'PPTX 幻灯片',
  docx: 'DOCX 文档',
  xlsx: 'XLSX 表格',
  pdf: 'PDF 文档',
}

let loadSeq = 0
async function load(k: Kind) {
  const seq = ++loadSeq
  loading.value = true
  error.value = ''
  try {
    const loaded = await loadProduct(k, `/samples/${k}`)
    if (seq !== loadSeq) return
    data.value = loaded
    kind.value = k
    selectedPaths.value = []
  } catch (e) {
    error.value = `加载失败: ${(e as Error).message}`
  } finally {
    loading.value = false
  }
}

/** 媒体解析：包内相对路径 → 样例目录 URL（应用方按自己的资源托管方式实现） */
const resolveMedia = computed(() => {
  return (src: string) =>
    src.startsWith('http') || src.startsWith('data:') ? src : `/samples/${kind.value}/${src}`
})

onMounted(() => load('pptx'))
</script>

<template>
  <div class="app">
    <header class="app-bar">
      <strong>@astorm/office-viewer</strong>
      <span>外部应用接入示例 — 只依赖公开 API</span>
      <nav>
        <button
          v-for="(label, k) in TABS"
          :key="k"
          :class="{ active: kind === k }"
          @click="load(k)"
        >
          {{ label }}
        </button>
      </nav>
    </header>

    <p v-if="loading" class="status">加载中…</p>
    <p v-else-if="error" class="status error">{{ error }}</p>
    <OfficeViewer
      v-else-if="data"
      :key="kind"
      :kind="kind"
      :data="data as any"
      :resolve-media="resolveMedia"
      @hover="(p) => (hoveredPath = p)"
      @select="(p) => selectedPaths.unshift(p)"
    />

    <footer class="app-foot">
      <span>悬浮路径：<code>{{ hoveredPath ?? '—' }}</code></span>
      <span v-if="selectedPaths.length">最近点击：<code>{{ selectedPaths[0] }}</code></span>
    </footer>
  </div>
</template>

<style>
body {
  margin: 0;
  font-family: -apple-system, 'PingFang SC', sans-serif;
  background: #f5f5f7;
}
.app {
  display: flex;
  flex-direction: column;
  height: 100vh;
}
.app-bar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 8px 14px;
  border-bottom: 1px solid rgba(0, 0, 0, 0.08);
  background: #fff;
  font-size: 13px;
}
.app-bar span {
  color: rgba(0, 0, 0, 0.45);
}
.app-bar nav {
  margin-left: auto;
  display: flex;
  gap: 4px;
}
.app-bar button {
  border: 0;
  padding: 5px 12px;
  border-radius: 6px;
  background: transparent;
  font: inherit;
  cursor: pointer;
}
.app-bar button.active {
  background: #007aff;
  color: #fff;
}
.app > .ov-root {
  flex: 1;
  min-height: 0;
}
.status {
  padding: 20px;
  font-size: 13px;
  color: rgba(0, 0, 0, 0.5);
}
.status.error {
  color: #c62828;
}
.app-foot {
  display: flex;
  gap: 18px;
  padding: 6px 14px;
  border-top: 1px solid rgba(0, 0, 0, 0.08);
  background: #fff;
  font-size: 12px;
  color: rgba(0, 0, 0, 0.6);
}
.app-foot code {
  font-family: ui-monospace, Menlo, monospace;
}
</style>
