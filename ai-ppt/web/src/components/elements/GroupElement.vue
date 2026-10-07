<script setup lang="ts">
// 分组元素渲染：容器 + 子元素递归
import { computed } from 'vue'
import type { GroupElement } from '@/types/presentation'
import { inchToPx } from '@/utils/convert'
import ElementView from './ElementView.vue'

const props = defineProps<{ el: GroupElement; path: string }>()

const emit = defineEmits<{
  (e: 'element-mousedown', ev: MouseEvent, path: string): void
  (e: 'save-text', path: string, text: string): void
}>()

const pos = computed(() => props.el.position)
const wrapStyle = computed(() => ({
  position: 'absolute' as const,
  left: `${inchToPx(pos.value.x)}px`,
  top: `${inchToPx(pos.value.y)}px`,
  width: `${inchToPx(pos.value.w)}px`,
  height: `${inchToPx(pos.value.h)}px`,
  transform: props.el.rotation ? `rotate(${props.el.rotation}deg)` : 'none',
  pointerEvents: 'none' as const,
}))
</script>

<template>
  <div class="el el-group" :style="wrapStyle">
    <ElementView
      v-for="(child, i) in el.children"
      :key="i"
      :el="child"
      :path="`${path}.${i}`"
      @element-mousedown="(e: MouseEvent, p: string) => emit('element-mousedown', e, p)"
      @save-text="(p: string, t: string) => emit('save-text', p, t)"
    />
  </div>
</template>
