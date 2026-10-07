<script setup lang="ts">
// 线条元素渲染：SVG path + 箭头 marker + 虚线
import { computed } from 'vue'
import type { LineElement } from '@/types/presentation'
import { inchToPx, withHash } from '@/utils/convert'

const props = defineProps<{ el: LineElement }>()

const pos = computed(() => props.el.position)

/** 顶点 → 屏幕 px（相对 position 原点） */
const pointsPx = computed(() =>
  props.el.points.map(([x, y]) => [inchToPx(x), inchToPx(y)] as [number, number]),
)

/** SVG path：直线/折线；smooth 用二次贝塞尔（控制点 = 中点） */
const pathD = computed(() => {
  const pts = pointsPx.value
  if (pts.length === 0) return ''
  const [x0, y0] = pts[0]
  let d = `M ${x0} ${y0}`
  if (props.el.smooth) {
    for (let i = 1; i < pts.length; i += 1) {
      const [x1, y1] = pts[i - 1]
      const [x2, y2] = pts[i]
      d += ` Q ${(x1 + x2) / 2} ${(y1 + y2) / 2} ${x2} ${y2}`
    }
  } else {
    for (let i = 1; i < pts.length; i += 1) {
      d += ` L ${pts[i][0]} ${pts[i][1]}`
    }
  }
  return d
})

const strokeColor = computed(() => withHash(props.el.color ?? '4472C4'))
const strokeWidth = computed(() => props.el.width ?? 1)
const strokeDash = computed(() => {
  switch (props.el.dash) {
    case 'dashed':
      return '6 4'
    case 'dotted':
      return '2 4'
    default:
      return 'none'
  }
})

/** 端点 marker：arrow → 实心箭头；dot → 圆点 */
const startMarker = computed(() => markerId(props.el.arrow_start, 'start'))
const endMarker = computed(() => markerId(props.el.arrow_end, 'end'))

function markerId(end: string | undefined, kind: string): string {
  if (!end || end === 'none') return ''
  return `url(#line-marker-${kind}-${end})`
}

const rotateTransform = computed(() => (props.el.rotation ? `rotate(${props.el.rotation}deg)` : 'none'))

const wrapStyle = computed(() => ({
  position: 'absolute' as const,
  left: `${inchToPx(pos.value.x)}px`,
  top: `${inchToPx(pos.value.y)}px`,
  width: `${inchToPx(pos.value.w)}px`,
  height: `${inchToPx(pos.value.h)}px`,
  transform: rotateTransform.value,
  pointerEvents: 'none' as const,
}))
</script>

<template>
  <div class="el el-line" :style="wrapStyle">
    <svg :width="inchToPx(pos.w)" :height="inchToPx(pos.h)" style="display: block">
      <defs>
        <marker
          v-if="el.arrow_start && el.arrow_start !== 'none'"
          :id="`line-marker-start-${el.arrow_start}`"
          :markerWidth="8"
          :markerHeight="8"
          refX="4"
          refY="4"
          orient="auto"
        >
          <path
            v-if="el.arrow_start === 'arrow'"
            d="M 0 0 L 8 4 L 0 8 Z"
            :fill="strokeColor"
          />
          <circle v-else cx="4" cy="4" r="3.5" :fill="strokeColor" />
        </marker>
        <marker
          v-if="el.arrow_end && el.arrow_end !== 'none'"
          :id="`line-marker-end-${el.arrow_end}`"
          :markerWidth="8"
          :markerHeight="8"
          refX="4"
          refY="4"
          orient="auto"
        >
          <path v-if="el.arrow_end === 'arrow'" d="M 0 0 L 8 4 L 0 8 Z" :fill="strokeColor" />
          <circle v-else cx="4" cy="4" r="3.5" :fill="strokeColor" />
        </marker>
      </defs>
      <path
        :d="pathD"
        fill="none"
        :stroke="strokeColor"
        :stroke-width="strokeWidth"
        :stroke-dasharray="strokeDash"
        :marker-start="startMarker || undefined"
        :marker-end="endMarker || undefined"
      />
    </svg>
  </div>
</template>
