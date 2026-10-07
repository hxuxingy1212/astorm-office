<script setup lang="ts">
// 单页幻灯片渲染（供画布编辑与缩略图复用）
import { computed } from 'vue'
import type { Slide } from '@/types/presentation'
import { withHash } from '@/utils/convert'
import ElementView from '../elements/ElementView.vue'

const props = defineProps<{
  slide: Slide
  widthPx: number
  heightPx: number
  /** edit：可选中/拖拽；view：只读（缩略图） */
  mode?: 'edit' | 'view'
}>()

const emit = defineEmits<{
  (e: 'element-mousedown', ev: MouseEvent, path: string): void
  (e: 'save-text', path: string, text: string): void
}>()

/** 背景样式 */
const bgStyle = computed(() => {
  const bg = props.slide.background
  if (typeof bg === 'string') {
    return { background: withHash(bg) }
  }
  if (bg && typeof bg === 'object') {
    if (bg.type === 'gradient' && Array.isArray(bg.stops)) {
      const stops = bg.stops
        .map((s: { color: string; position: number }) => `${withHash(s.color)} ${s.position * 100}%`)
        .join(', ')
      return { background: `linear-gradient(${Number(bg.angle ?? 0) + 90}deg, ${stops})` }
    }
    if (bg.type === 'image' && bg.src) {
      const src = String(bg.src).startsWith('http')
        ? String(bg.src)
        : `/api/media/${String(bg.src)}`
      return { backgroundImage: `url(${src})`, backgroundSize: 'cover', backgroundPosition: 'center' }
    }
  }
  return {}
})
</script>

<template>
  <div
    class="slide-view"
    :style="{ width: `${widthPx}px`, height: `${heightPx}px`, ...bgStyle }"
  >
    <ElementView
      v-for="(el, i) in slide.elements"
      :key="i"
      :el="el"
      :path="`${i}`"
      :mode="props.mode ?? 'view'"
      @element-mousedown="(e: MouseEvent, p: string) => emit('element-mousedown', e, p)"
      @save-text="(p: string, t: string) => emit('save-text', p, t)"
    />
  </div>
</template>

<style scoped>
.slide-view {
  position: relative;
  overflow: hidden;
  flex-shrink: 0;
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.2);
  background: #ffffff;
}
</style>
