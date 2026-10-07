<script setup lang="ts">
// 图片元素渲染
import { computed, ref } from 'vue'
import type { ImageElement } from '@/types/presentation'
import { inchToPx, lineStyle } from '@/utils/convert'

const props = defineProps<{ el: ImageElement }>()

const pos = computed(() => props.el.position)
const failed = ref(false)

/** 产物内媒体路径 → API 地址 */
const srcUrl = computed(() => {
  const src = props.el.src
  if (src.startsWith('http://') || src.startsWith('https://')) return src
  return `/api/media/${src}`
})

/** 裁剪（a:srcRect 单位 1/1000 百分比，0~100000，100000 = 100% → CSS clip） */
const clipStyle = computed(() => {
  const c = props.el.crop
  if (!c) return undefined
  const w = inchToPx(pos.value.w)
  const h = inchToPx(pos.value.h)
  return {
    clipPath: `inset(${(c.top / 100000) * h}px ${(c.right / 100000) * w}px ${(c.bottom / 100000) * h}px ${(c.left / 100000) * w}px)`,
  }
})

const wrapStyle = computed(() => ({
  position: 'absolute' as const,
  left: `${inchToPx(pos.value.x)}px`,
  top: `${inchToPx(pos.value.y)}px`,
  width: `${inchToPx(pos.value.w)}px`,
  height: `${inchToPx(pos.value.h)}px`,
  transform: props.el.rotation ? `rotate(${props.el.rotation}deg)` : 'none',
  border: lineStyle(props.el.line),
  overflow: 'hidden' as const,
}))

function onError() {
  failed.value = true
}
</script>

<template>
  <div class="el el-image" :style="wrapStyle">
    <img
      v-if="!failed"
      :src="srcUrl"
      :style="clipStyle"
      style="width: 100%; height: 100%; object-fit: fill; display: block"
      draggable="false"
      @error="onError"
    />
    <span v-else class="el-image-error">图片无效</span>
  </div>
</template>

<style scoped>
.el-image-error {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  font-size: 11px;
  color: #999;
  background: #f1f3f7;
}
</style>
