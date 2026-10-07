<script setup lang="ts">
// 图标元素渲染：内置 SVG 图标库
import { computed } from 'vue'
import type { IconElement } from '@/types/presentation'
import { inchToPx, withHash } from '@/utils/convert'
import { iconDef } from '@/configs/icons'

const props = defineProps<{ el: IconElement }>()

const pos = computed(() => props.el.position)

const def = computed(() => iconDef(props.el.icon))

const wrapStyle = computed(() => ({
  position: 'absolute' as const,
  left: `${inchToPx(pos.value.x)}px`,
  top: `${inchToPx(pos.value.y)}px`,
  width: `${inchToPx(pos.value.w)}px`,
  height: `${inchToPx(pos.value.h)}px`,
  transform: props.el.rotation ? `rotate(${props.el.rotation}deg)` : 'none',
}))

const color = computed(() => withHash(props.el.color ?? '#4472C4'))
</script>

<template>
  <div class="el el-icon" :style="wrapStyle">
    <svg viewBox="0 0 24 24" width="100%" height="100%" preserveAspectRatio="xMidYMid meet">
      <template v-if="def">
        <path v-for="(d, i) in def.paths" :key="i" :d="d" :fill="color" />
      </template>
      <text v-else x="12" y="12" text-anchor="middle" dominant-baseline="middle" font-size="8" fill="#999">
        ?
      </text>
    </svg>
  </div>
</template>

<style scoped>
.el-icon {
  display: flex;
  align-items: center;
  justify-content: center;
}
</style>
