<script setup lang="ts">
//! office-viewer 演示：四格式切换 + 悬浮路径卡片 + select 事件回显
import { computed, nextTick, onMounted, ref, shallowRef } from 'vue'
import { OfficeViewer, loadProduct } from '@/index'
import type { DocxDocument, XlsxWorkbook } from '@/types'
import type { PptxDoc } from '@/renderers/pptx/presentation'
import type { PdfDoc } from '@/renderers/pdf/types'
import type { PackedSheet } from '@/renderers/xlsx/packed'
import {
  buildPackedSynthetic,
  buildStreamedSynthetic,
  buildSyntheticWorkbook,
  measureParseThroughput,
} from './perf'

type Kind = 'pptx' | 'docx' | 'xlsx' | 'xlsx-charts' | 'pdf' | 'perf'

const kind = ref<Kind>('pptx')
const loading = ref(false)
const error = ref('')
type AnyWorkbook = XlsxWorkbook | { sheets: PackedSheet[] }
const data = shallowRef<DocxDocument | AnyWorkbook | PptxDoc | PdfDoc | null>(null)
const hoveredPath = ref<string | null>(null)
const selectedPaths = ref<string[]>([])

/** 样例产物目录（demo/public/samples/<kind>）；BASE_URL 兼容 Pages 子路径部署 */
const ASSET_BASE = import.meta.env.BASE_URL
const SAMPLES: Record<Kind, { dir: string; label: string }> = {
  pptx: { dir: 'pptx', label: 'PPTX · 12 套设计模板（creative）' },
  docx: { dir: 'docx', label: 'DOCX · 学术论文（含公式/表格/题注）' },
  xlsx: { dir: 'xlsx', label: 'XLSX · 经营看板（样式/数字格式/合并/图表）' },
  'xlsx-charts': { dir: 'xlsx-charts', label: 'XLSX · 多表 + 图片 + 形状（真实语料）' },
  pdf: { dir: 'pdf', label: 'PDF · 产品说明页（文本/图形/渐变/图片）' },
  perf: { dir: '', label: '⚡ 性能压测（虚拟滚动 · 百万行）' },
}

/* ---- 性能压测：客户端合成工作簿，测量生成/渲染/滚动指标 ---- */
const perfRows = ref(200_000)
const perfCols = ref(6)
const perfDense = ref(true)
const perfMode = ref<'packed' | 'json' | 'stream'>('stream')
const perfMetrics = ref<{
  buildMs: number
  cells: number
  heapMB: number | null
  packedMB: number | null
  textMB?: number
  mountMs: number | null
  domRows: number
  domNodes: number
  scrollMs: number | null
} | null>(null)

/** 解析吞吐基准结果 */
const bench = ref<{
  textMB: number
  jsonMBps: number
  streamMBps: number
  scannerMBps: number
  batchMBps: number
  jsonParseMs: number
  streamMs: number
  scannerMs: number
  batchMs: number
} | null>(null)
const benchRunning = ref(false)

async function runBench() {
  benchRunning.value = true
  await nextFrame()
  try {
    const r = await measureParseThroughput({ rows: 130_000, cols: 6 })
    bench.value = {
      textMB: r.textMB,
      jsonMBps: r.jsonMBps,
      streamMBps: r.streamMBps,
      scannerMBps: r.scannerMBps,
      batchMBps: r.batchMBps,
      jsonParseMs: r.jsonParseMs,
      streamMs: r.streamMs,
      scannerMs: r.scannerMs,
      batchMs: r.batchMs,
    }
  } finally {
    benchRunning.value = false
  }
}

/** 等一帧：后台标签页 rAF 会被节流，故与定时器赛跑 */
function nextFrame(ms = 120): Promise<void> {
  return Promise.race([
    new Promise<void>((r) => requestAnimationFrame(() => r())),
    new Promise<void>((r) => setTimeout(r, ms)),
  ])
}

async function runPerf() {
  loading.value = true
  error.value = ''
  try {
    const sample =
      perfMode.value === 'stream'
        ? await buildStreamedSynthetic({ rows: perfRows.value, cols: perfCols.value })
        : perfMode.value === 'packed'
          ? buildPackedSynthetic({ rows: perfRows.value, cols: perfCols.value })
          : buildSyntheticWorkbook({
              rows: perfRows.value,
              cols: perfCols.value,
              dense: perfDense.value,
            })
    perfMetrics.value = {
      buildMs: sample.buildMs,
      cells: sample.cells,
      heapMB: sample.heapMB,
      packedMB: sample.packedMB,
      textMB: sample.textMB,
      mountMs: null,
      domRows: 0,
      domNodes: 0,
      scrollMs: null,
    }
    const t0 = performance.now()
    data.value = sample.workbook
    kind.value = 'perf'
    selectedPaths.value = []
    await nextTick()
    await nextFrame()
    const t1 = performance.now()
    const domRows = document.querySelectorAll('.ov-sheet-canvas tbody tr').length
    perfMetrics.value = {
      ...perfMetrics.value,
      mountMs: Math.round(t1 - t0),
      domRows,
      domNodes: document.querySelectorAll('.ov-sheet-canvas *').length,
    }
  } catch (e) {
    error.value = `压测失败: ${(e as Error).message}`
  } finally {
    loading.value = false
  }
}

/** 滚动到 90% 处并测量该次渲染耗时 */
async function probeScroll() {
  const wrap = document.querySelector('.ov-sheet-wrap') as HTMLElement | null
  if (!wrap || !perfMetrics.value) return
  const t0 = performance.now()
  wrap.scrollTop = Math.round((wrap.scrollHeight - wrap.clientHeight) * 0.9)
  wrap.dispatchEvent(new Event('scroll'))
  await nextFrame()
  await nextFrame()
  const t1 = performance.now()
  perfMetrics.value = {
    ...perfMetrics.value,
    scrollMs: Math.round(t1 - t0),
    domRows: document.querySelectorAll('.ov-sheet-canvas tbody tr').length,
    domNodes: document.querySelectorAll('.ov-sheet-canvas *').length,
  }
}

/** 请求序号：快速切换时丢弃过期响应，避免旧请求覆盖当前选择 */
let loadSeq = 0

async function load(k: Kind) {
  const productKind = (k === 'xlsx-charts' || k === 'perf' ? 'xlsx' : k) as
    | 'docx'
    | 'xlsx'
    | 'pptx'
    | 'pdf'
  const seq = ++loadSeq
  loading.value = true
  error.value = ''
  try {
    const meta = SAMPLES[k]
    if (k === 'perf') {
      await runPerf()
      return
    }
    // 产物目录的 slides/parts/sheets 是路径数组，loader 负责按路径组装
    const loaded = (await loadProduct(productKind, `${ASSET_BASE}samples/${meta.dir}`)) as
      | DocxDocument
      | XlsxWorkbook
      | PptxDoc
      | PdfDoc
    if (seq !== loadSeq) return // 已有更新的选择，丢弃本次结果
    data.value = loaded
    kind.value = k
    selectedPaths.value = []
  } catch (e) {
    error.value = `加载样例失败: ${(e as Error).message}`
    data.value = null
  } finally {
    loading.value = false
  }
}

/** 媒体解析：产物内相对路径 → 样例目录 URL */
const resolveMedia = computed(() => {
  const dir = SAMPLES[kind.value].dir
  return (src: string) => (src.startsWith('http') || src.startsWith('data:') ? src : `${ASSET_BASE}samples/${dir}/${src}`)
})

onMounted(() => load('pptx'))
</script>

<template>
  <div class="demo">
    <div class="demo-window">
      <header class="demo-titlebar">
        <span class="demo-lights">
          <i class="light close"></i><i class="light min"></i><i class="light max"></i>
        </span>
        <span class="demo-title">office-viewer — 四格式高保真预览</span>
        <span class="demo-subtitle">悬浮任意元素查看 CLI 路径</span>
      </header>

      <nav class="demo-tabs ov-segmented">
        <button
          v-for="(meta, k) in SAMPLES"
          :key="k"
          :class="{ active: kind === k }"
          @click="load(k as Kind)"
        >
          {{ meta.label }}
        </button>
      </nav>

    <div class="demo-body">
      <section class="demo-viewer">
        <p v-if="loading" class="demo-status">加载中…</p>
        <p v-else-if="error" class="demo-error">{{ error }}</p>
        <OfficeViewer
          v-else-if="data"
          :key="kind"
          :kind="kind === 'xlsx-charts' || kind === 'perf' ? 'xlsx' : kind"
          :data="data"
          :resolve-media="resolveMedia"
          @hover="(p) => (hoveredPath = p)"
          @select="(p) => selectedPaths.unshift(p)"
        />
      </section>

      <aside class="demo-side">
        <template v-if="kind === 'perf'">
          <h3>压测参数</h3>
          <div class="perf-ctl">
            <label>行数 <input v-model.number="perfRows" type="number" min="1000" step="100000" /></label>
            <div class="perf-presets">
              <button class="ov-btn" @click="perfRows = 100_000">10 万</button>
              <button class="ov-btn" @click="perfRows = 500_000">50 万</button>
              <button class="ov-btn" @click="perfRows = 1_000_000">100 万</button>
            </div>
            <label>列数 <input v-model.number="perfCols" type="number" min="1" max="50" /></label>
            <label>
              数据形态
              <select v-model="perfMode" class="perf-select">
                <option value="stream">流式解析（峰值最低）</option>
                <option value="packed">列式打包（直接构造）</option>
                <option value="json">JSON 对象模型（对照）</option>
              </select>
            </label>
            <label v-if="perfMode === 'json'" class="perf-check"><input v-model="perfDense" type="checkbox" /> 每行写满单元格</label>
            <button class="ov-btn" :disabled="loading" @click="runPerf">生成并渲染</button>
            <button class="ov-btn" :disabled="!perfMetrics" @click="probeScroll">滚动到 90% 并测速</button>
            <button class="ov-btn" :disabled="benchRunning" @click="runBench">
              {{ benchRunning ? '基准测试中…' : '解析吞吐基准' }}
            </button>
          </div>
          <h3>指标</h3>
          <dl v-if="perfMetrics" class="perf-metrics">
            <dt>合成耗时</dt><dd>{{ perfMetrics.buildMs }} ms</dd>
            <dt>单元格总数</dt><dd>{{ perfMetrics.cells.toLocaleString() }}</dd>
            <dt>JS 堆占用</dt><dd>{{ perfMetrics.heapMB !== null ? perfMetrics.heapMB + ' MB' : '—' }}</dd>
            <dt>列式体积估算</dt><dd>{{ perfMetrics.packedMB !== null ? perfMetrics.packedMB + ' MB' : '（JSON 模型）' }}</dd>
            <dt>生成文本量</dt><dd>{{ perfMetrics.textMB !== undefined ? perfMetrics.textMB + ' MB' : '—' }}</dd>
            <dt>首屏渲染</dt><dd>{{ perfMetrics.mountMs !== null ? perfMetrics.mountMs + ' ms' : '—' }}</dd>
            <dt>DOM 行数</dt><dd>{{ perfMetrics.domRows }}</dd>
            <dt>DOM 节点数</dt><dd>{{ perfMetrics.domNodes.toLocaleString() }}</dd>
            <dt>滚动一次渲染</dt><dd>{{ perfMetrics.scrollMs !== null ? perfMetrics.scrollMs + ' ms' : '—' }}</dd>
          </dl>
          <dl v-if="bench" class="perf-metrics">
            <dt>基准文本量</dt><dd>{{ bench.textMB }} MB</dd>
            <dt>JSON.parse（原生）</dt><dd>{{ bench.jsonMBps }} MB/s（{{ bench.jsonParseMs }} ms）</dd>
            <dt>流式 · 逐行 parse</dt><dd>{{ bench.streamMBps }} MB/s（{{ bench.streamMs }} ms）</dd>
            <dt>流式 · 字段扫描</dt><dd>{{ bench.scannerMBps }} MB/s（{{ bench.scannerMs }} ms）</dd>
            <dt>流式 · 批解析</dt><dd>{{ bench.batchMBps }} MB/s（{{ bench.batchMs }} ms）</dd>
            <dt>批解析/原生 比值</dt><dd>{{ (bench.batchMBps / bench.jsonMBps).toFixed(2) }}×</dd>
          </dl>
          <p class="perf-hint">
            虚拟滚动：DOM 行数只与视口有关（约 30~60 行），与总行数无关。<br />
            内存提示：数据模型本身占大头（每单元格约 140B，100 万 × 6 列 ≈ 800MB），
            渲染侧已与行数解耦。
          </p>
        </template>

        <h3>悬浮路径</h3>
        <code class="demo-path">{{ hoveredPath ?? '（把鼠标移到文档元素上）' }}</code>
        <h3>点击选中（最近 8 条）</h3>
        <ol class="demo-selected">
          <li v-for="(p, i) in selectedPaths.slice(0, 8)" :key="i"><code>{{ p }}</code></li>
        </ol>
        <h3>CLI 用法示例</h3>
        <pre class="demo-cli">json2pptx edit deck/ '{{ hoveredPath ?? '/slide[1]/text[1]' }}' get
json2docx edit doc/ '/part[1]/paragraph[3]' set --prop text=新文本
json2xlsx edit book/ '/sheet[1]/cell[B3]' set --prop value=1234
{{ hoveredPath ? `# 路径来自悬浮元素：${hoveredPath}` : '' }}</pre>
      </aside>
      </div>
    </div>
  </div>
</template>

<style>
/* ---- macOS 窗口外壳 ---- */
body {
  margin: 0;
  background: linear-gradient(180deg, #dcdce0 0%, #cfd2d8 100%);
  color: rgba(0, 0, 0, 0.85);
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', 'Helvetica Neue',
    sans-serif;
  -webkit-font-smoothing: antialiased;
  min-height: 100vh;
}
.demo {
  max-width: 1480px;
  margin: 0 auto;
  padding: 28px 24px 56px;
}
.demo-window {
  border-radius: 12px;
  overflow: hidden;
  background: #ececec;
  box-shadow: 0 24px 60px rgba(0, 0, 0, 0.28), 0 0 0 0.5px rgba(0, 0, 0, 0.18);
}
/* 标题栏：毛玻璃 + 交通灯 */
.demo-titlebar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 9px 14px;
  background: rgba(246, 246, 246, 0.78);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  border-bottom: 0.5px solid rgba(0, 0, 0, 0.12);
}
.demo-lights {
  display: inline-flex;
  gap: 8px;
  align-items: center;
}
.light {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  display: inline-block;
  box-shadow: inset 0 0 0 0.5px rgba(0, 0, 0, 0.14);
}
.light.close {
  background: #ff5f57;
}
.light.min {
  background: #febc2e;
}
.light.max {
  background: #28c840;
}
.demo-title {
  font-size: 13px;
  font-weight: 590;
  color: rgba(0, 0, 0, 0.82);
}
.demo-subtitle {
  font-size: 12px;
  color: rgba(0, 0, 0, 0.42);
}
.demo-tabs {
  display: flex;
  gap: 2px;
  margin: 10px 12px 12px;
  padding: 2px;
  border-radius: 7px;
  background: rgba(120, 120, 128, 0.12);
  width: fit-content;
  max-width: calc(100% - 24px);
  overflow-x: auto;
}
.demo-tabs button {
  appearance: none;
  border: 0;
  padding: 5px 13px;
  border-radius: 5px;
  background: transparent;
  color: rgba(0, 0, 0, 0.85);
  font: inherit;
  font-size: 12px;
  white-space: nowrap;
  cursor: default;
}
.demo-tabs button:hover {
  background: rgba(0, 0, 0, 0.04);
}
.demo-tabs button.active {
  background: #fff;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.12), 0 0 0 0.5px rgba(0, 0, 0, 0.06);
  font-weight: 500;
  color: #0a6cd8;
}
.demo-body {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 300px;
  gap: 16px;
  align-items: start;
  padding: 0 12px 12px;
}
.demo-viewer {
  min-width: 0;
}
/* 侧栏：毛玻璃 inspector */
.demo-side {
  position: sticky;
  top: 16px;
  padding: 14px 16px;
  border-radius: 12px;
  background: rgba(246, 246, 246, 0.72);
  backdrop-filter: saturate(180%) blur(20px);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.1), 0 1px 3px rgba(0, 0, 0, 0.06);
}
.demo-side h3 {
  margin: 14px 0 6px;
  font-size: 11px;
  font-weight: 590;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  color: rgba(0, 0, 0, 0.42);
}
.demo-side h3:first-child {
  margin-top: 0;
}
.demo-path {
  display: block;
  min-height: 20px;
  padding: 6px 9px;
  border-radius: 6px;
  background: rgba(120, 120, 128, 0.14);
  color: #1d1d1f;
  font-family: ui-monospace, 'SF Mono', SFMono-Regular, Menlo, monospace;
  font-size: 11.5px;
  word-break: break-all;
}
.demo-selected {
  margin: 0;
  padding-left: 18px;
  font-size: 11.5px;
  color: rgba(0, 0, 0, 0.7);
}
.demo-selected code {
  font-family: ui-monospace, 'SF Mono', Menlo, monospace;
}
.demo-cli {
  margin: 0;
  padding: 9px 10px;
  border-radius: 8px;
  background: #fff;
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.08);
  font-family: ui-monospace, 'SF Mono', Menlo, monospace;
  font-size: 10.5px;
  line-height: 1.5;
  color: rgba(0, 0, 0, 0.72);
  white-space: pre-wrap;
  word-break: break-all;
  overflow: hidden;
}
.perf-ctl {
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12px;
}
.perf-ctl label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.perf-ctl input[type='number'] {
  width: 108px;
  height: 22px;
  padding: 0 6px;
  border: 1px solid rgba(0, 0, 0, 0.12);
  border-radius: 5px;
  font: inherit;
}
.perf-ctl input[type='checkbox'] {
  margin-right: 4px;
}
.perf-check {
  justify-content: flex-start !important;
}
.perf-presets {
  display: flex;
  gap: 6px;
}
.perf-select {
  height: 22px;
  padding: 0 4px;
  border: 1px solid rgba(0, 0, 0, 0.12);
  border-radius: 5px;
  font: inherit;
  background: #fff;
}
.perf-presets .ov-btn {
  flex: 1;
  font-size: 11px;
}
.perf-metrics {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 2px 10px;
  margin: 0;
  font-size: 12px;
}
.perf-metrics dt {
  color: rgba(0, 0, 0, 0.42);
}
.perf-metrics dd {
  margin: 0;
  font-variant-numeric: tabular-nums;
  text-align: right;
}
.perf-hint {
  margin: 8px 0 0;
  font-size: 11px;
  color: rgba(0, 0, 0, 0.42);
  line-height: 1.5;
}
.demo-status,
.demo-error {
  padding: 14px;
  font-size: 13px;
  color: rgba(0, 0, 0, 0.5);
}
.demo-error {
  color: #c62828;
}
</style>
