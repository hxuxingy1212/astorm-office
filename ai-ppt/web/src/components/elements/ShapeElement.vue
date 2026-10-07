<script setup lang="ts">
// 形状元素渲染：SVG path + 填充/线条/阴影/旋转 + 内部文字
import { computed } from 'vue'
import type { ShapeElement } from '@/types/presentation'
import { shapePath } from '@/configs/shapes'
import { inchToPx, ptToPx, withHash, colorWithAlpha, shadowToCss, solidFillColor, gradientFill } from '@/utils/convert'
import { textContentHtml } from '@/utils/text'

const props = defineProps<{ el: ShapeElement }>()

const pos = computed(() => props.el.position)
const path = computed(() => shapePath(props.el.shape_type))

/** 圆角矩形 adjust（adj 0~50000 对应圆角半径 0~50%） */
const roundRectPath = computed(() => {
  const adj = props.el.adjust?.adj
  const r = adj !== undefined ? (adj / 50000) * 200 : 20
  return `M ${r} 0 L ${200 - r} 0 Q 200 0 200 ${r} L 200 ${200 - r} Q 200 200 ${200 - r} 200 L ${r} 200 Q 0 200 0 ${200 - r} L 0 ${r} Q 0 0 ${r} 0 Z`
})

const displayPath = computed(() => {
  if (props.el.shape_type === 'roundRect') return roundRectPath.value
  return path.value
})

const fillStyle = computed(() => {
  const el = props.el
  if (el.no_fill) return 'none'
  const color = solidFillColor(el.fill)
  if (color) return colorWithAlpha(color, el.fill_alpha)
  return 'none'
})

const fillGradient = computed(() => gradientFill(props.el.fill))

const strokeColor = computed(() => (props.el.line ? withHash(props.el.line.color) : 'none'))
const strokeWidth = computed(() => (props.el.line ? props.el.line.width : 0))
const rotateTransform = computed(() => (props.el.rotation ? `rotate(${props.el.rotation}deg)` : 'none'))

const wrapStyle = computed(() => {
  const el = props.el
  return {
    position: 'absolute' as const,
    left: `${inchToPx(pos.value.x)}px`,
    top: `${inchToPx(pos.value.y)}px`,
    width: `${inchToPx(pos.value.w)}px`,
    height: `${inchToPx(pos.value.h)}px`,
    transform: rotateTransform.value,
    filter: shadowToCss(el.shadow) !== 'none' ? undefined : undefined,
    boxShadow: shadowToCss(el.shadow),
  }
})

// 内部文字
const textStyle = computed(() => {
  const el = props.el
  const parts: string[] = []
  const vAlign =
    el.vert_align === 'top' ? 'flex-start' : el.vert_align === 'bottom' ? 'flex-end' : 'center'
  if (el.font_size) parts.push(`font-size:${ptToPx(el.font_size)}px`)
  if (el.color) parts.push(`color:${withHash(el.color)}`)
  if (el.align) parts.push(`text-align:${el.align}`)
  // 多段落垂直堆叠；align-items:stretch 让段落占满宽度，按 text-align 对齐
  parts.push(`display:flex;flex-direction:column;align-items:stretch;justify-content:${vAlign}`)
  return parts.join(';')
})
</script>

<template>
  <div class="el el-shape" :style="wrapStyle">
    <svg
      :width="inchToPx(pos.w)"
      :height="inchToPx(pos.h)"
      viewBox="0 0 200 200"
      preserveAspectRatio="none"
      overflow="visible"
      style="display: block"
    >
      <defs v-if="fillGradient">
        <linearGradient
          :id="`grad-${el.name ?? 'shape'}`"
          x1="0%"
          y1="0%"
          x2="100%"
          y2="100%"
        >
          <stop
            v-for="(s, i) in fillGradient.stops"
            :key="i"
            :offset="`${s.position * 100}%`"
            :stop-color="withHash(s.color)"
          />
        </linearGradient>
      </defs>
      <!-- fallback：未覆盖的形状显示为矩形 + 居中类型名 -->
      <path
        v-if="displayPath"
        vector-effect="non-scaling-stroke"
        :d="displayPath"
        :fill="fillGradient ? `url(#grad-${el.name ?? 'shape'})` : fillStyle"
        :stroke="strokeColor"
        :stroke-width="strokeWidth"
      />
      <template v-else>
        <rect
          x="5"
          y="5"
          width="190"
          height="190"
          fill="none"
          stroke="#999"
          stroke-width="1"
          stroke-dasharray="4 3"
        />
        <text
          x="100"
          y="100"
          text-anchor="middle"
          dominant-baseline="middle"
          font-size="10"
          fill="#888"
        >
          {{ el.shape_type }}
        </text>
      </template>
    </svg>
    <div
      v-if="el.text"
      class="el-shape-text"
      :style="textStyle"
      v-html="textContentHtml(el.text, el.line_spacing)"
    />
  </div>
</template>

<style scoped>
.el-shape {
  overflow: hidden;
}
.el-shape-text {
  position: absolute;
  inset: 0;
  padding: 2px 4px;
}
.el-shape-text :deep(p) {
  margin: 0;
  line-height: 1.2;
}
</style>
