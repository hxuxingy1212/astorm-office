<script setup lang="ts">
// 元素操作层：选中框 + 8 缩放手柄 + 旋转手柄
import { computed } from 'vue'
import type { Element } from '@/types/presentation'
import { inchToPx } from '@/utils/convert'
import { isComponent, elementRotation } from '@/utils/elements'

const props = defineProps<{ el: Element }>()

const emit = defineEmits<{
  (e: 'scale-start', dir: string, ev: MouseEvent): void
  (e: 'rotate-start', ev: MouseEvent): void
}>()

const pos = computed(() => props.el.position)

/** 8 个手柄位置（% 定位在边框上） */
const HANDLES = [
  { dir: 'nw', left: '0%', top: '0%', cursor: 'nwse-resize' },
  { dir: 'n', left: '50%', top: '0%', cursor: 'ns-resize' },
  { dir: 'ne', left: '100%', top: '0%', cursor: 'nesw-resize' },
  { dir: 'e', left: '100%', top: '50%', cursor: 'ew-resize' },
  { dir: 'se', left: '100%', top: '100%', cursor: 'nwse-resize' },
  { dir: 's', left: '50%', top: '100%', cursor: 'ns-resize' },
  { dir: 'sw', left: '0%', top: '100%', cursor: 'nesw-resize' },
  { dir: 'w', left: '0%', top: '50%', cursor: 'ew-resize' },
]

const boxStyle = computed(() => ({
  position: 'absolute' as const,
  left: `${inchToPx(pos.value.x)}px`,
  top: `${inchToPx(pos.value.y)}px`,
  width: `${inchToPx(pos.value.w)}px`,
  height: `${inchToPx(pos.value.h)}px`,
  transform: elementRotation(props.el) ? `rotate(${elementRotation(props.el)}deg)` : 'none',
}))
</script>

<template>
  <div class="operate-box" :style="boxStyle">
    <!-- 边框 -->
    <div class="operate-border" />
    <!-- 旋转手柄（顶部） -->
    <div class="operate-rotate" @mousedown.stop="(e) => emit('rotate-start', e)">
      <svg width="14" height="14" viewBox="0 0 14 14">
        <circle cx="7" cy="7" r="6" fill="#4472C4" />
        <path d="M 7 3 A 4 4 0 1 1 3.5 4.5" fill="none" stroke="#fff" stroke-width="1.5" />
      </svg>
    </div>
    <!-- 8 个缩放手柄 -->
    <div
      v-for="h in HANDLES"
      :key="h.dir"
      class="operate-handle"
      :class="`handle-${h.dir}`"
      :style="{ cursor: h.cursor }"
      @mousedown.stop="(e) => emit('scale-start', h.dir, e)"
    />
    <!-- 组件标签 -->
    <div v-if="isComponent(el)" class="operate-tag">{{ el.type }}</div>
  </div>
</template>

<style scoped>
.operate-box {
  pointer-events: none;
}
.operate-border {
  position: absolute;
  inset: 0;
  border: 1px solid #4472c4;
}
.operate-handle {
  position: absolute;
  width: 9px;
  height: 9px;
  background: #fff;
  border: 1px solid #4472c4;
  transform: translate(-50%, -50%);
  pointer-events: auto;
  z-index: 10;
}
.operate-rotate {
  position: absolute;
  top: -22px;
  left: 50%;
  transform: translateX(-50%);
  pointer-events: auto;
  cursor: grab;
  z-index: 10;
}
.operate-tag {
  position: absolute;
  top: -18px;
  left: 0;
  background: rgba(68, 114, 196, 0.9);
  color: #fff;
  font-size: 10px;
  padding: 1px 5px;
  white-space: nowrap;
}
</style>
