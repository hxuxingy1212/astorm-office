<script setup lang="ts">
// 右侧属性面板分发器
import { computed } from 'vue'
import { useEditorStore } from '@/stores/editor'
import { useSlidesStore, getElementByPath, parsePath } from '@/stores/slides'
import { useSnapshotStore } from '@/stores/snapshot'
import type { Element } from '@/types/presentation'
import { typeLabel } from '@/utils/elements'
import TextPanel from './TextPanel.vue'
import ShapePanel from './ShapePanel.vue'
import LinePanel from './LinePanel.vue'
import ComponentPanel from './ComponentPanel.vue'
import PositionPanel from './PositionPanel.vue'
import SlidePanel from './SlidePanel.vue'
import ImagePanel from './ImagePanel.vue'
import IconPanel from './IconPanel.vue'
import FormulaPanel from './FormulaPanel.vue'

const editor = useEditorStore()
const slides = useSlidesStore()
const snapshot = useSnapshotStore()

const el = computed<Element | null>(() => {
  const id = editor.handleElementId
  if (!id) return null
  return getElementByPath(slides.currentSlide?.elements ?? [], parsePath(id))
})

const showSlidePanel = computed(() => editor.activeElementIds.length === 0)

function update(props: Partial<Element>) {
  const id = editor.handleElementId
  if (!id) return
  slides.updateElement(id, props)
  snapshot.addHistorySnapshot(slides.slides, slides.slideIndex)
}
</script>

<template>
  <aside class="style-panel">
    <SlidePanel v-if="showSlidePanel" />
    <template v-else-if="el">
      <div class="panel-title">
        {{ typeLabel(el.type) }}
        <button class="small" @click="editor.clearSelection">取消</button>
      </div>
      <TextPanel v-if="el.type === 'text'" :el="el" @update="update" />
      <ShapePanel v-else-if="el.type === 'shape'" :el="el" @update="update" />
      <LinePanel v-else-if="el.type === 'line'" :el="el" @update="update" />
      <ImagePanel v-else-if="el.type === 'image'" :el="el" @update="update" />
      <IconPanel v-else-if="el.type === 'icon'" :el="el" @update="update" />
      <FormulaPanel v-else-if="el.type === 'formula'" :el="el" @update="update" />
      <ComponentPanel v-else-if="el.type !== 'table' && el.type !== 'group'" :el="el" @update="update" />
      <p v-else class="hint">该元素类型暂不支持属性编辑（可移动/缩放/删除）</p>
      <PositionPanel :el="el" @update="update" />
    </template>
    <div v-else class="empty-hint">未选中元素</div>
  </aside>
</template>

<style scoped>
.style-panel {
  width: 220px;
  background: #fff;
  border-left: 1px solid #e5e5e5;
  overflow-y: auto;
  padding: 12px;
  box-sizing: border-box;
}
.panel-title {
  font-weight: bold;
  margin-bottom: 8px;
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.empty-hint {
  color: #999;
  font-size: 13px;
  text-align: center;
  margin-top: 40px;
}
.hint {
  color: #999;
  font-size: 12px;
}
</style>
