<script setup lang="ts">
// 元素分发器：按 type 渲染对应组件
import { computed } from 'vue'
import type { Element } from '@/types/presentation'
import { isComponent } from '@/utils/elements'
import TextElement from './TextElement.vue'
import ShapeElement from './ShapeElement.vue'
import LineElement from './LineElement.vue'
import ImageElement from './ImageElement.vue'
import IconElement from './IconElement.vue'
import FormulaElement from './FormulaElement.vue'
import TableElement from './TableElement.vue'
import GroupElement from './GroupElement.vue'
import Components from './Components.vue'

const props = defineProps<{
  el: Element
  path: string
  /** 交互模式：edit 可选中/拖拽；view 只读（缩略图/放映） */
  mode?: 'edit' | 'view'
}>()

const interactive = computed(() => props.mode !== 'view')

/** 点击元素（选中），叶子元素与组件可交互；group 容器穿透 */
function onMouseDown(e: MouseEvent) {
  if (!interactive.value) return
  if (props.el.type === 'group') return
  e.stopPropagation()
  emit('element-mousedown', e, props.path)
}

const emit = defineEmits<{
  (e: 'element-mousedown', ev: MouseEvent, path: string): void
  (e: 'save-text', path: string, text: string): void
}>()
</script>

<template>
  <TextElement
    v-if="el.type === 'text'"
    :el="el"
    :editable="interactive"
    :class="interactive ? 'interactive' : ''"
    :data-el-path="path"
    @mousedown="onMouseDown"
    @save-text="(t: string) => emit('save-text', path, t)"
  />
  <ShapeElement
    v-else-if="el.type === 'shape'"
    :el="el"
    :class="interactive ? 'interactive' : ''"
    :data-el-path="path"
    @mousedown="onMouseDown"
  />
  <LineElement
    v-else-if="el.type === 'line'"
    :el="el"
    :class="interactive ? 'interactive' : ''"
    :data-el-path="path"
    @mousedown="onMouseDown"
  />
  <ImageElement
    v-else-if="el.type === 'image'"
    :el="el"
    :class="interactive ? 'interactive' : ''"
    :data-el-path="path"
    @mousedown="onMouseDown"
  />
  <IconElement
    v-else-if="el.type === 'icon'"
    :el="el"
    :class="interactive ? 'interactive' : ''"
    :data-el-path="path"
    @mousedown="onMouseDown"
  />
  <FormulaElement
    v-else-if="el.type === 'formula'"
    :el="el"
    :class="interactive ? 'interactive' : ''"
    :data-el-path="path"
    @mousedown="onMouseDown"
  />
  <TableElement
    v-else-if="el.type === 'table'"
    :el="el"
    :class="interactive ? 'interactive' : ''"
    :data-el-path="path"
    @mousedown="onMouseDown"
  />
  <GroupElement
    v-else-if="el.type === 'group'"
    :el="el"
    :path="path"
    @element-mousedown="(e: MouseEvent, p: string) => emit('element-mousedown', e, p)"
    @save-text="(p: string, t: string) => emit('save-text', p, t)"
  />
  <Components
    v-else-if="isComponent(el)"
    :el="el"
    :class="interactive ? 'interactive' : ''"
    :data-el-path="path"
    @mousedown="onMouseDown"
  />
</template>

<style>
/* 叶子元素在编辑/选择模式可命中事件（group 容器内也生效） */
.el.interactive {
  pointer-events: auto;
  cursor: pointer;
}
.el-group {
  pointer-events: none;
}
</style>
