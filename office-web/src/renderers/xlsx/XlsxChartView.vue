<script setup lang="ts">
//! xlsx 图表：从 data_range 读数值，SVG 绘制常见类型（column/bar/line/area/pie/ring/doughnut/scatter）
import { computed } from 'vue'
import type { XlsxChart, XlsxSheet } from '@/types'
import { parseCell, parseRange } from '@/renderers/xlsx/range'
import type { PathCardInfo } from '@/core/hover'

const props = defineProps<{
  chart: XlsxChart
  sheet: XlsxSheet
  chartIndex: number
  sheetIndex: number
}>()

const emit = defineEmits<{
  (e: 'hover-cell', info: PathCardInfo, ev: MouseEvent): void
  (e: 'leave'): void
  (e: 'move', ev: MouseEvent): void
}>()

const W = 420
const H = 240
const PAD = 34

/** (row,col) → 数值 */
function numAt(row: number, col: number): number | null {
  for (const r of props.sheet.rows) {
    if ((r.index ?? 0) !== row) continue
    let next = 1
    for (const cell of r.cells) {
      let c = next
      if (cell.reference) {
        const p = parseCell(cell.reference)
        if (p) c = p.col
      }
      next = c + 1
      if (c === col) return typeof cell.value === 'number' ? cell.value : null
    }
  }
  return null
}

function textAt(row: number, col: number): string {
  for (const r of props.sheet.rows) {
    if ((r.index ?? 0) !== row) continue
    let next = 1
    for (const cell of r.cells) {
      let c = next
      if (cell.reference) {
        const p = parseCell(cell.reference)
        if (p) c = p.col
      }
      next = c + 1
      if (c === col) return cell.value === null || cell.value === undefined ? '' : String(cell.value)
    }
  }
  return ''
}

const range = computed(() => (props.chart.data_range ? parseRange(props.chart.data_range) : null))

/** 系列（按列）：每列一组数值 */
const series = computed(() => {
  const r = range.value
  if (!r) return [] as { name: string; values: number[]; color?: string }[]
  const out: { name: string; values: number[]; color?: string }[] = []
  for (let c = r.c1; c <= r.c2; c += 1) {
    const values: number[] = []
    for (let rr = r.r1; rr <= r.r2; rr += 1) values.push(numAt(rr, c) ?? 0)
    const name = textAt(r.r1 - 1 >= 1 ? r.r1 - 1 : r.r1, c) || `系列${c - r.c1 + 1}`
    out.push({ name, values, color: props.chart.colors?.[c - r.c1] })
  }
  return out
})

/** 类别（行标签）：优先 categories 范围，否则取数据区左侧一列 */
const categories = computed(() => {
  const r = range.value
  if (!r) return [] as string[]
  const catRange = props.chart.categories ? parseRange(props.chart.categories) : null
  const out: string[] = []
  for (let rr = r.r1; rr <= r.r2; rr += 1) {
    const t = catRange ? textAt(rr, catRange.c1) : r.c1 > 1 ? textAt(rr, r.c1 - 1) : String(rr)
    out.push(t || String(rr))
  }
  return out
})

const kind = computed(() => {
  const t = (props.chart.type || 'column').toLowerCase()
  if (['pie', 'doughnut', 'ring', 'donut'].includes(t)) return 'pie'
  if (['line', 'area', 'scatter'].includes(t)) return t
  if (['bar', 'column', 'stackedColumn', 'stackedcolumn', 'stackedBar', 'stackedbar'].includes(t))
    return 'column'
  return 'column'
})

const PALETTE = ['#4472C4', '#ED7D31', '#A5A5A5', '#FFC000', '#5B9BD5', '#70AD47', '#264478']

const maxValue = computed(() => {
  let m = 0
  for (const s of series.value) for (const v of s.values) m = Math.max(m, v)
  return m > 0 ? m : 1
})


function xAt(i: number, n: number): number {
  const inner = W - PAD * 2
  return PAD + (n <= 1 ? inner / 2 : (inner * i) / (n - 1))
}
function yAt(v: number): number {
  const inner = H - PAD * 2
  return PAD + inner - (v / maxValue.value) * inner
}

const gridLines = computed(() => {
  const out: { y: number; label: string }[] = []
  for (let i = 0; i <= 4; i += 1) {
    const v = (maxValue.value * i) / 4
    out.push({ y: yAt(v), label: String(Math.round(v * 100) / 100) })
  }
  return out
})

/** 饼/环：取第一系列 */
const pieSlices = computed(() => {
  const s = series.value[0]
  if (!s) return [] as { value: number; label: string; color: string; d: string }[]
  const total = s.values.reduce((a, b) => a + Math.max(0, b), 0) || 1
  let angle = -Math.PI / 2
  return s.values.map((v, i) => {
    const frac = Math.max(0, v) / total
    const a0 = angle
    const a1 = angle + frac * Math.PI * 2
    angle = a1
    const cx = W / 2
    const cy = H / 2 + 6
    const rOut = 88
    const rIn = kind.value === 'pie' ? 0 : 50
    const p = (a: number, r: number) => `${cx + r * Math.cos(a)},${cy + r * Math.sin(a)}`
    const large = frac > 0.5 ? 1 : 0
    const d =
      rIn > 0
        ? `M${p(a0, rOut)}A${rOut},${rOut} 0 ${large} 1 ${p(a1, rOut)}L${p(a1, rIn)}A${rIn},${rIn} 0 ${large} 0 ${p(a0, rIn)}Z`
        : `M${cx},${cy}L${p(a0, rOut)}A${rOut},${rOut} 0 ${large} 1 ${p(a1, rOut)}Z`
    return { value: v, label: categories.value[i] ?? String(i + 1), color: PALETTE[i % PALETTE.length], d }
  })
})

function onChartEnter(ev: MouseEvent) {
  emit(
    'hover-cell',
    {
      path: `/sheet[${props.sheetIndex}]/chart[${props.chartIndex}]`,
      type: 'chart',
      name: props.chart.title || props.chart.type,
    },
    ev,
  )
}

function seriesColor(i: number, s: { color?: string }): string {
  if (s.color) return s.color.startsWith('#') ? s.color : `#${s.color}`
  return PALETTE[i % PALETTE.length]
}

const barCount = computed(() => Math.max(1, series.value.length))
</script>

<template>
  <figure
    class="ov-chart"
    @mouseenter="onChartEnter"
    @mousemove="emit('move', $event)"
    @mouseleave="emit('leave')"
  >
    <figcaption v-if="chart.title">{{ chart.title }}</figcaption>
    <svg :viewBox="`0 0 ${W} ${H}`" role="img">
      <template v-if="kind === 'pie'">
        <path v-for="(sl, i) in pieSlices" :key="i" :d="sl.d" :fill="sl.color" stroke="#fff" stroke-width="1.5" />
      </template>
      <template v-else>
        <line
          v-for="(g, i) in gridLines"
          :key="`g${i}`"
          :x1="PAD"
          :x2="W - PAD"
          :y1="g.y"
          :y2="g.y"
          stroke="#e6e9ee"
          stroke-width="1"
        />
        <text
          v-for="(g, i) in gridLines"
          :key="`t${i}`"
          :x="PAD - 6"
          :y="g.y + 4"
          text-anchor="end"
          font-size="10"
          fill="#8b97a6"
        >
          {{ g.label }}
        </text>
        <!-- 柱状 -->
        <template v-if="kind === 'column'">
          <template v-for="(s, si) in series" :key="`ser${si}`">
            <rect
              v-for="(v, i) in s.values"
              :key="`b${si}-${i}`"
              :x="
                xAt(i, s.values.length) -
                ((barCount * (W - PAD * 2)) / Math.max(1, s.values.length) / (barCount + 1)) / 2 +
                (si * (W - PAD * 2)) / Math.max(1, s.values.length) / (barCount + 1)
              "
              :y="v >= 0 ? yAt(v) : yAt(0)"
              :width="Math.max(3, (W - PAD * 2) / Math.max(1, s.values.length) / (barCount + 1))"
              :height="Math.max(1, Math.abs(yAt(v) - yAt(0)))"
              :fill="seriesColor(si, s)"
              rx="2"
            />
          </template>
        </template>
        <!-- 折线 / 面积 -->
        <template v-else-if="kind === 'line' || kind === 'area'">
          <template v-for="(s, si) in series" :key="`ln${si}`">
            <polygon
              v-if="kind === 'area'"
              :points="`${PAD},${yAt(0)} ${s.values.map((v, i) => `${xAt(i, s.values.length)},${yAt(v)}`).join(' ')} ${W - PAD},${yAt(0)}`"
              :fill="seriesColor(si, s)"
              opacity="0.25"
            />
            <polyline
              :points="s.values.map((v, i) => `${xAt(i, s.values.length)},${yAt(v)}`).join(' ')"
              fill="none"
              :stroke="seriesColor(si, s)"
              stroke-width="2"
            />
            <circle
              v-for="(v, i) in s.values"
              :key="`p${si}-${i}`"
              :cx="xAt(i, s.values.length)"
              :cy="yAt(v)"
              r="2.6"
              :fill="seriesColor(si, s)"
            />
          </template>
        </template>
        <!-- 散点 -->
        <template v-else-if="kind === 'scatter'">
          <template v-for="(s, si) in series" :key="`scs${si}`">
            <circle
              v-for="(v, i) in s.values"
              :key="`sc${si}-${i}`"
              :cx="xAt(i, s.values.length)"
              :cy="yAt(v)"
              r="4"
              :fill="seriesColor(si, s)"
            />
          </template>
        </template>
        <!-- 类别标签（稀疏显示） -->
        <text
          v-for="(c, i) in categories"
          v-show="categories.length <= 12 || i % 2 === 0"
          :key="`c${i}`"
          :x="xAt(i, categories.length)"
          :y="H - PAD + 16"
          text-anchor="middle"
          font-size="10"
          fill="#5a6472"
        >
          {{ c.length > 6 ? `${c.slice(0, 6)}…` : c }}
        </text>
      </template>
      <!-- 图例 -->
      <template v-if="kind === 'pie'">
        <g v-for="(sl, i) in pieSlices.slice(0, 6)" :key="`lg${i}`">
          <rect :x="W - 120" :y="10 + i * 16" width="10" height="10" :fill="sl.color" rx="2" />
          <text :x="W - 104" :y="19 + i * 16" font-size="10" fill="#5a6472">
            {{ sl.label.length > 8 ? `${sl.label.slice(0, 8)}…` : sl.label }}
          </text>
        </g>
      </template>
      <template v-else-if="series.length > 1">
        <g v-for="(s, i) in series.slice(0, 5)" :key="`lg2${i}`">
          <rect :x="PAD + i * 92" :y="6" width="10" height="10" :fill="seriesColor(i, s)" rx="2" />
          <text :x="PAD + i * 92 + 14" :y="15" font-size="10" fill="#5a6472">
            {{ s.name.length > 8 ? `${s.name.slice(0, 8)}…` : s.name }}
          </text>
        </g>
      </template>
    </svg>
  </figure>
</template>

<style>
.ov-chart {
  margin: 0;
  padding: 10px 12px 6px;
  border-radius: 10px;
  background: #fff;
  box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.08), 0 1px 3px rgba(0, 0, 0, 0.06);
  /* 默认尺寸；被锚点定位时由外部 style 覆盖宽高 */
  width: 440px;
  height: 280px;
  display: flex;
  flex-direction: column;
}
.ov-chart svg {
  flex: 1;
  min-height: 0;
  width: 100%;
}
.ov-chart figcaption {
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', sans-serif;
  font-size: 12px;
  font-weight: 500;
  color: rgba(0, 0, 0, 0.7);
  margin-bottom: 4px;
}
</style>
