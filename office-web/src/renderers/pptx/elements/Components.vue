<script setup lang="ts">
// 高级组件渲染：图表用 ECharts（SVG renderer），其余 div/SVG 模拟。
// echarts 是可选 peerDep：按需动态加载，未安装时图表渲染为占位块而不是让整个库 import 失败。
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type * as EChartsNS from 'echarts/core'
import type { Element } from '.././presentation'
import { inchToPx, ptToPx, withHash } from '.././convert'

let echartsMod: typeof EChartsNS | null | undefined // undefined=未尝试, null=不可用

async function ensureEcharts(): Promise<typeof EChartsNS | null> {
  if (echartsMod !== undefined) return echartsMod
  try {
    const [core, charts, comps, renderers] = await Promise.all([
      import('echarts/core'),
      import('echarts/charts'),
      import('echarts/components'),
      import('echarts/renderers'),
    ])
    core.use([
      charts.BarChart,
      charts.LineChart,
      charts.PieChart,
      comps.LegendComponent,
      comps.TooltipComponent,
      renderers.SVGRenderer,
    ])
    echartsMod = core
  } catch {
    echartsMod = null // 未安装 echarts：图表降级为占位
  }
  return echartsMod
}

const props = defineProps<{ el: Element }>()

const pos = computed(() => props.el.position)
const wPx = computed(() => inchToPx(pos.value.w))
const hPx = computed(() => inchToPx(pos.value.h))

const wrapStyle = computed(() => ({
  position: 'absolute' as const,
  left: `${inchToPx(pos.value.x)}px`,
  top: `${inchToPx(pos.value.y)}px`,
  width: `${wPx.value}px`,
  height: `${hPx.value}px`,
}))

// === ECharts 图表 ===

const chartRef = ref<HTMLDivElement | null>(null)
const chartUnavailable = ref(false)
const isChart = computed(() => ['barChart', 'lineChart', 'pieChart', 'ringChart'].includes(props.el.type))
let chart: EChartsNS.ECharts | null = null

const DEFAULT_COLORS = ['#4472C4', '#ED7D31', '#A5A5A5', '#FFC000', '#5B9BD5', '#70AD47', '#FF0000', '#7030A0']

function chartOption(el: Element) {
  const base = { legend: { show: true, bottom: 0 }, tooltip: { trigger: 'axis' } }
  switch (el.type) {
    case 'barChart': {
      const labels = el.labels ?? el.data.map((_, i) => `${i + 1}`)
      return {
        ...base,
        xAxis: { type: 'category', data: labels },
        yAxis: { type: 'value', max: el.max },
        series: [{
          type: 'bar',
          data: el.data,
          itemStyle: { color: (p: { dataIndex: number }) => withHash(el.colors?.[p.dataIndex] ?? '#4472C4') },
          label: { show: el.show_values !== false, position: 'top', fontSize: 10 },
        }],
      }
    }
    case 'lineChart': {
      const labels = el.labels ?? el.data.map((_, i) => `${i + 1}`)
      return {
        ...base,
        xAxis: { type: 'category', data: labels },
        yAxis: { type: 'value', max: el.max },
        series: [{
          type: 'line',
          data: el.data,
          smooth: !!el.smooth,
          symbolSize: 6,
          lineStyle: { color: withHash(el.colors?.[0] ?? '#4472C4') },
          itemStyle: { color: withHash(el.colors?.[0] ?? '#4472C4') },
          label: { show: el.show_values !== false, position: 'top', fontSize: 10 },
        }],
      }
    }
    case 'pieChart': {
      const labels = el.labels ?? el.data.map((_, i) => `${i + 1}`)
      return {
        ...base,
        tooltip: { trigger: 'item' },
        series: [{
          type: 'pie',
          radius: ['0%', '70%'],
          data: el.data.map((v, i) => ({
            value: v,
            name: labels[i] ?? `${i + 1}`,
            itemStyle: { color: withHash(el.colors?.[i] ?? DEFAULT_COLORS[i % 8]) },
          })),
          label: { show: el.show_values !== false, formatter: '{d}%', fontSize: 10 },
        }],
      }
    }
    case 'ringChart': {
      const labels = el.labels ?? el.data.map((_, i) => `${i + 1}`)
      return {
        ...base,
        tooltip: { trigger: 'item' },
        series: [{
          type: 'pie',
          radius: ['55%', '75%'],
          data: el.data.map((v, i) => ({
            value: v,
            name: labels[i] ?? `${i + 1}`,
            itemStyle: { color: withHash(el.colors?.[i] ?? DEFAULT_COLORS[i % 8]) },
          })),
          label: { show: el.show_values !== false, formatter: '{d}%', fontSize: 10 },
        }],
      }
    }
    default:
      return {}
  }
}

async function renderChart(el: Element) {
  if (!isChart.value) return
  const ec = await ensureEcharts()
  if (!ec) {
    chartUnavailable.value = true
    return
  }
  chartUnavailable.value = false
  if (!chartRef.value) return
  if (!chart) {
    chart = ec.init(chartRef.value, undefined, { renderer: 'svg' })
  }
  chart.setOption(chartOption(el), true)
}

watch(
  () => props.el,
  (el) => {
    void renderChart(el)
  },
  { immediate: true, deep: true },
)
// immediate 回调发生在挂载前（chartRef 还没绑上），挂载后补一次首渲
onMounted(() => {
  void renderChart(props.el)
})

onBeforeUnmount(() => {
  chart?.dispose()
  chart = null
})

// === 非图表组件 ===

const progress = computed(() => {
  const el = props.el as { type: 'progressBar' | 'progressRing'; value?: number; color?: string; track_color?: string }
  return Math.min(100, Math.max(0, el.value ?? 50))
})

const ringColor = computed(() => withHash((props.el as { color?: string }).color ?? '#4472C4'))
const ringTrack = computed(() => withHash((props.el as { track_color?: string }).track_color ?? '#E0E0E0'))

const rating = computed(() => (props.el as { rating?: number; max?: number }).rating ?? 0)
const ratingMax = computed(() => (props.el as { rating?: number; max?: number }).max ?? 5)
const ratingColor = computed(() => withHash((props.el as { color?: string }).color ?? '#F1C40F'))
const ratingEmpty = computed(() => withHash((props.el as { empty_color?: string }).empty_color ?? '#CCCCCC'))

const kpi = computed(() => props.el as { value?: string; label?: string; delta?: string; bg?: string; accent?: string; text_color?: string })
const kpiBg = computed(() => withHash(kpi.value.bg ?? '#2c3e50'))
const kpiAccent = computed(() => withHash(kpi.value.accent ?? '#3498db'))
const kpiText = computed(() => withHash(kpi.value.text_color ?? '#FFFFFF'))

const flowSteps = computed(() => (props.el as { steps?: string[]; colors?: string[]; text_color?: string }).steps ?? [])
const flowColors = computed(() => (props.el as { colors?: string[] }).colors ?? ['#3498db', '#2ecc71', '#e74c3c', '#f39c12', '#9b59b6'])
const flowText = computed(() => withHash((props.el as { text_color?: string }).text_color ?? '#FFFFFF'))

const timeline = computed(() => props.el as { items?: Record<string, unknown>[]; line_color?: string; dot_color?: string; label_color?: string })
const tlLine = computed(() => withHash(timeline.value.line_color ?? '#3498db'))
const tlDot = computed(() => withHash(timeline.value.dot_color ?? '#3498db'))
const tlLabel = computed(() => withHash(timeline.value.label_color ?? '#000000'))

function timelineLabel(item: Record<string, unknown>): string {
  const title = item.title ?? item.date ?? item.name ?? ''
  return String(title)
}
</script>

<template>
  <div class="el el-component" :style="wrapStyle">
    <!-- 进度条 -->
    <template v-if="el.type === 'progressBar'">
      <div
        class="comp-progress-track"
        :style="{
          background: withHash(el.track_color ?? '#E0E0E0'),
          borderRadius: el.rounded === false ? '0' : '6px',
        }"
      >
        <div
          class="comp-progress-fill"
          :style="{
            width: `${progress}%`,
            background: withHash(el.color ?? '#4472C4'),
            borderRadius: el.rounded === false ? '0' : '6px',
          }"
        />
      </div>
      <div v-if="el.show_label !== false" class="comp-progress-label" :style="{ color: withHash(el.text_color ?? '#000') }">
        {{ el.label ?? `${Math.round(progress)}%` }}
      </div>
    </template>

    <!-- 环形进度 -->
    <svg v-else-if="el.type === 'progressRing'" :width="wPx" :height="hPx" style="display: block">
      <circle
        :cx="wPx / 2"
        :cy="hPx / 2"
        :r="Math.min(wPx, hPx) / 2 - (el.thickness ?? 8)"
        fill="none"
        :stroke="ringTrack"
        :stroke-width="el.thickness ?? 8"
      />
      <circle
        :cx="wPx / 2"
        :cy="hPx / 2"
        :r="Math.min(wPx, hPx) / 2 - (el.thickness ?? 8)"
        fill="none"
        :stroke="ringColor"
        :stroke-width="el.thickness ?? 8"
        :stroke-dasharray="`${(progress / 100) * Math.PI * 2 * (Math.min(wPx, hPx) / 2 - (el.thickness ?? 8))} 9999`"
        transform="rotate(-90)"
        :style="{ transformOrigin: 'center' }"
      />
      <text
        v-if="el.show_label !== false"
        :x="wPx / 2"
        :y="hPx / 2"
        text-anchor="middle"
        dominant-baseline="middle"
        :font-size="ptToPx(el.font_size ?? 18)"
        :fill="withHash(el.text_color ?? '#000')"
      >
        {{ Math.round(progress) }}%
      </text>
    </svg>

    <!-- 图表组件 -->
    <template v-else-if="isChart">
      <div v-if="chartUnavailable" class="comp-chart-missing">图表 · 未安装 echarts</div>
      <div v-else ref="chartRef" :style="{ width: '100%', height: '100%' }" />
    </template>

    <!-- KPI 卡片 -->
    <div v-else-if="el.type === 'kpiCard'" class="comp-kpi" :style="{ background: kpiBg }">
      <div class="comp-kpi-value" :style="{ color: kpiText }">{{ kpi.value }}</div>
      <div v-if="kpi.label" class="comp-kpi-label" :style="{ color: kpiText }">{{ kpi.label }}</div>
      <div
        v-if="kpi.delta"
        class="comp-kpi-delta"
        :style="{ color: kpi.delta.startsWith('+') ? '#2ecc71' : '#e74c3c' }"
      >
        {{ kpi.delta }}
      </div>
      <div class="comp-kpi-accent" :style="{ background: kpiAccent }" />
    </div>

    <!-- 星级评分 -->
    <div v-else-if="el.type === 'ratingStars'" class="comp-rating">
      <svg
        v-for="i in ratingMax"
        :key="i"
        :width="Math.min(hPx * 0.85, wPx / ratingMax)"
        :height="Math.min(hPx * 0.85, wPx / ratingMax)"
        viewBox="0 0 200 200"
      >
        <path
          d="M 100 15 L 125 75 L 190 80 L 140 125 L 155 190 L 100 155 L 45 190 L 60 125 L 10 80 L 75 75 Z"
          :fill="i <= rating ? ratingColor : ratingEmpty"
        />
      </svg>
    </div>

    <!-- 时间线 -->
    <div v-else-if="el.type === 'timeline'" class="comp-timeline">
      <div class="comp-timeline-line" :style="{ background: tlLine }" />
      <div
        v-for="(item, i) in (el.items ?? [])"
        :key="i"
        class="comp-timeline-item"
        :style="{ left: `${((i + 0.5) / Math.max(1, el.items?.length ?? 1)) * 100}%` }"
      >
        <div class="comp-timeline-dot" :style="{ background: tlDot }" />
        <div class="comp-timeline-text" :style="{ color: tlLabel }">{{ timelineLabel(item) }}</div>
      </div>
    </div>

    <!-- 流程步骤 -->
    <div v-else-if="el.type === 'processFlow'" class="comp-flow">
      <template v-for="(step, i) in flowSteps" :key="i">
        <div
          class="comp-flow-step"
          :style="{
            background: withHash(flowColors[i % flowColors.length] ?? '#3498db'),
            color: flowText,
          }"
        >
          {{ step }}
        </div>
        <div v-if="i < flowSteps.length - 1" class="comp-flow-arrow">▶</div>
      </template>
    </div>

    <!-- 未知组件占位 -->
    <div v-else class="comp-unknown">{{ el.type }}</div>
  </div>
</template>

<style scoped>
.el-component {
  overflow: hidden;
}
.comp-progress-track {
  position: absolute;
  top: 50%;
  left: 0;
  width: 85%;
  height: 55%;
  transform: translateY(-50%);
  overflow: hidden;
}
.comp-progress-fill {
  height: 100%;
}
.comp-progress-label {
  position: absolute;
  top: 50%;
  left: 88%;
  transform: translateY(-50%);
  font-size: 12px;
  white-space: nowrap;
}
.comp-kpi {
  width: 100%;
  height: 100%;
  padding: 8px 12px;
  box-sizing: border-box;
  position: relative;
}
.comp-kpi-value {
  font-size: 28px;
  font-weight: bold;
}
.comp-kpi-label {
  font-size: 12px;
  opacity: 0.85;
}
.comp-kpi-delta {
  position: absolute;
  top: 10px;
  right: 12px;
  font-size: 13px;
  font-weight: bold;
}
.comp-kpi-accent {
  position: absolute;
  bottom: 0;
  left: 0;
  width: 100%;
  height: 6px;
}
.comp-rating {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
}
.comp-timeline {
  position: relative;
  width: 100%;
  height: 100%;
}
.comp-timeline-line {
  position: absolute;
  top: 50%;
  left: 0;
  width: 100%;
  height: 3px;
  transform: translateY(-50%);
}
.comp-timeline-item {
  position: absolute;
  top: 50%;
  transform: translate(-50%, -50%);
  text-align: center;
}
.comp-timeline-dot {
  width: 12px;
  height: 12px;
  margin: 0 auto;
}
.comp-timeline-text {
  margin-top: 4px;
  font-size: 11px;
  white-space: nowrap;
}
.comp-flow {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  gap: 4px;
}
.comp-flow-step {
  flex: 1;
  height: 60%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: bold;
}
.comp-flow-arrow {
  font-size: 10px;
  color: #95a5a6;
}
.comp-unknown {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed #ccc;
  color: #999;
  font-size: 12px;
}
.comp-chart-missing {
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px dashed var(--ov-border, #ccc);
  color: var(--ov-text-muted, #999);
  font-size: 12px;
  box-sizing: border-box;
}
</style>
