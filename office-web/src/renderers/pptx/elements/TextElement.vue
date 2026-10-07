<script setup lang="ts">
// 文本元素渲染（编辑态：双击编辑）
import { computed, ref, watch } from 'vue'
import type { TextElement } from '.././presentation'
import { inchToPx, ptToPx, withHash, colorWithAlpha, lineStyle, shadowToCss, solidFillColor } from '.././convert'
import { textContentHtml, textContentPlain } from '.././text'
import { elementRotation } from '.././elements'

const props = defineProps<{ el: TextElement; editable?: boolean }>()

const pos = computed(() => props.el.position)
const rotation = computed(() => elementRotation(props.el) ?? 0)
const style = computed(() => {
  const el = props.el
  const fillColor = solidFillColor(el.fill)
  return {
    position: 'absolute' as const,
    left: `${inchToPx(pos.value.x)}px`,
    top: `${inchToPx(pos.value.y)}px`,
    width: `${inchToPx(pos.value.w)}px`,
    height: `${inchToPx(pos.value.h)}px`,
    transform: rotation.value ? `rotate(${rotation.value}deg)` : 'none',
    background: colorWithAlpha(fillColor, el.fill_alpha),
    border: lineStyle(el.line),
    boxShadow: shadowToCss(el.shadow),
  }
})

// 默认文本样式（元素级）
const contentStyle = computed(() => {
  const el = props.el
  const parts: string[] = []
  if (el.font_size) parts.push(`font-size:${ptToPx(el.font_size)}px`)
  if (el.bold) parts.push('font-weight:bold')
  if (el.italic) parts.push('font-style:italic')
  if (el.underline) parts.push('text-decoration:underline')
  if (el.color) parts.push(`color:${withHash(el.color)}`)
  if (el.font_family) parts.push(`font-family:${el.font_family}`)
  if (el.align) parts.push(`text-align:${el.align}`)
  if (el.vert && el.vert !== 'horz') {
    parts.push('writing-mode:vertical-rl;text-orientation:mixed')
  } else {
    const vAlign =
      el.vert_align === 'middle' ? 'center' : el.vert_align === 'bottom' ? 'flex-end' : 'flex-start'
    parts.push('display:flex;flex-direction:column;justify-content:' + vAlign)
  }
  return parts.join(';')
})

// 编辑态：双击进入文本编辑
const editing = ref(false)
const editText = ref('')
const emit = defineEmits<{ (e: 'save-text', text: string): void }>()

function startEdit() {
  if (props.editable === false) return
  editing.value = true
  editText.value = textContentPlain(props.el.text)
}

function commitEdit() {
  editing.value = false
  emit('save-text', editText.value)
}

watch(editing, (v) => {
  if (v) {
    setTimeout(() => {
      const el = document.querySelector<HTMLTextAreaElement>('.el-text-editor')
      el?.focus()
    }, 0)
  }
})
</script>

<template>
  <div class="el el-text" :style="style" @dblclick.stop="startEdit">
    <div
      v-if="!editing"
      class="el-text-content"
      :style="contentStyle"
      v-html="textContentHtml(el.text, el.line_spacing)"
    />
    <textarea
      v-else
      v-model="editText"
      class="el-text-editor"
      @blur="commitEdit"
      @keydown.esc.stop="commitEdit"
    />
  </div>
</template>

<style scoped>
.el-text {
  overflow: hidden;
}
.el-text-content {
  width: 100%;
  height: 100%;
  font-size: 18px;
}
.el-text-content :deep(p) {
  margin: 0;
  padding: 0;
}
.el-text-editor {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  border: 1px dashed #4472c4;
  resize: none;
  font: inherit;
  box-sizing: border-box;
  padding: 2px;
  background: rgba(255, 255, 255, 0.95);
}
</style>
