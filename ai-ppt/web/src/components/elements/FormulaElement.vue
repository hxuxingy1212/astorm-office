<script setup lang="ts">
// 公式元素渲染：以 $...$ 包裹的 LaTeX 文本（轻量近似渲染，非完整 LaTeX 引擎）
import { computed } from 'vue'
import type { FormulaElement } from '@/types/presentation'
import { inchToPx, ptToPx, withHash } from '@/utils/convert'

const props = defineProps<{ el: FormulaElement }>()

const pos = computed(() => props.el.position)

const wrapStyle = computed(() => ({
  position: 'absolute' as const,
  left: `${inchToPx(pos.value.x)}px`,
  top: `${inchToPx(pos.value.y)}px`,
  width: `${inchToPx(pos.value.w)}px`,
  height: `${inchToPx(pos.value.h)}px`,
  transform: props.el.rotation ? `rotate(${props.el.rotation}deg)` : 'none',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  fontFamily: 'Cambria Math, "Times New Roman", serif',
  fontStyle: 'italic',
  fontSize: `${ptToPx(props.el.font_size ?? 24)}px`,
  color: withHash(props.el.color ?? '#000000'),
  whiteSpace: 'pre-wrap',
}))

/** 去掉外层 $...$，简单转义上下标符号 */
const displayText = computed(() => {
  let s = props.el.latex
  if (s.startsWith('$') && s.endsWith('$')) s = s.slice(1, -1)
  return s
})
</script>

<template>
  <div class="el el-formula" :style="wrapStyle">{{ displayText }}</div>
</template>
