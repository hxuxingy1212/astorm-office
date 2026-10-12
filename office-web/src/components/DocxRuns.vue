<script setup lang="ts">
//! run 序列渲染：文字 span（runStyle）+ 内嵌图片（inline 随文流；锚浮图按 x/y 绝对定位到所在块）
//! 段落、标题、列表项、表格单元格共用一份，避免「表里只画 text、图片整段丢」的偏科。
import type { DocxRun, DocxRunImage } from '@/types'
import { ptToPx } from '@/core/units'
import { runStyle } from '@/renderers/docx/blocks'

const props = withDefaults(
  defineProps<{
    runs: readonly DocxRun[]
    /** 媒体解析：把 word/media/xxx 解析成可加载 URL */
    resolveMedia?: (src: string) => string
    /** 允许锚浮图绝对定位（段落/标题为 true；单元格内退化为随文流，避免撑破表格） */
    float?: boolean
  }>(),
  { float: true },
)

function imgSrc(img: DocxRunImage): string {
  const src = String(img.src ?? '')
  return props.resolveMedia ? props.resolveMedia(src) : src
}

/** 图片样式：有尺寸按尺寸；锚浮（wrap≠inline 或带 x/y）且允许时，相对块左上角绝对定位 */
function imgStyle(img: DocxRunImage): Record<string, string | undefined> {
  const w =
    typeof img.width === 'number' && img.width > 0 ? `${Math.round(ptToPx(img.width))}px` : undefined
  const h =
    typeof img.height === 'number' && img.height > 0
      ? `${Math.round(ptToPx(img.height))}px`
      : undefined
  const anchored =
    (typeof img.wrap === 'string' && img.wrap !== 'inline') ||
    typeof img.x === 'number' ||
    typeof img.y === 'number'
  if (props.float && anchored) {
    return {
      position: 'absolute',
      left: `${Math.round(ptToPx(img.x ?? 0))}px`,
      top: `${Math.round(ptToPx(img.y ?? 0))}px`,
      width: w,
      height: h,
      maxWidth: 'none',
    }
  }
  return { width: w, height: h, verticalAlign: 'middle', maxWidth: '100%' }
}
</script>

<template>
  <template v-for="(r, ri) in runs" :key="ri">
    <img v-if="r.image" class="ov-run-img" :src="imgSrc(r.image)" :style="imgStyle(r.image)" alt="" />
    <span v-if="r.text" :style="runStyle(r)">{{ r.text }}</span>
  </template>
</template>

<style>
.ov-run-img {
  display: inline-block;
}
</style>
