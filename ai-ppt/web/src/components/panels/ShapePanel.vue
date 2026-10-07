<script setup lang="ts">
// 形状属性面板
import { computed } from 'vue'
import type { ShapeElement } from '@/types/presentation'
import { withHash, solidFillColor } from '@/utils/convert'
import { textContentPlain } from '@/utils/text'

const props = defineProps<{ el: ShapeElement }>()
const emit = defineEmits<{ (e: 'update', props: Partial<ShapeElement>): void }>()

const plainText = computed(() => (props.el.text ? textContentPlain(props.el.text) : ''))

function set(prop: string, value: unknown) {
  emit('update', { [prop]: value } as Partial<ShapeElement>)
}

/** 常用形状列表（下拉选择） */
const COMMON_SHAPES = [
  'rect', 'roundRect', 'ellipse', 'triangle', 'diamond', 'pentagon', 'hexagon', 'octagon',
  'parallelogram', 'trapezoid', 'rightArrow', 'leftArrow', 'upArrow', 'downArrow', 'chevron',
  'star5', 'donut', 'blockArc', 'flowChartProcess', 'flowChartDecision', 'flowChartTerminator',
  'flowChartInputOutput', 'lightningBolt', 'heart', 'cloud',
]
</script>

<template>
  <div class="panel">
    <h4>形状</h4>
    <div class="row">
      <label>类型</label>
      <select :value="el.shape_type" @change="(e) => set('shape_type', (e.target as HTMLSelectElement).value)">
        <option v-for="s in COMMON_SHAPES" :key="s" :value="s">{{ s }}</option>
      </select>
    </div>
    <div class="row">
      <label>填充</label>
      <input
        type="color"
        :value="withHash(solidFillColor(el.fill) ?? '#4472C4')"
        @input="(e) => set('fill', (e.target as HTMLInputElement).value.slice(1))"
      />
      <button class="small" @click="set('no_fill', true)">无填充</button>
      <button class="small" @click="set('no_fill', false)">填充</button>
    </div>
    <div class="row">
      <label>边框色</label>
      <input
        type="color"
        :value="withHash(el.line?.color ?? '#000000')"
        @input="(e) => set('line', { color: (e.target as HTMLInputElement).value.slice(1), width: el.line?.width ?? 1 })"
      />
    </div>
    <div class="row">
      <label>边框宽</label>
      <input
        type="number"
        :value="el.line?.width ?? 0"
        min="0"
        max="20"
        step="0.5"
        @input="(e) => set('line', { color: el.line?.color ?? '000000', width: Number((e.target as HTMLInputElement).value) })"
      />
    </div>
    <template v-if="el.text !== undefined">
      <label>内部文字</label>
      <textarea
        :value="plainText"
        rows="2"
        @input="(e) => set('text', (e.target as HTMLTextAreaElement).value)"
      />
      <div class="row">
        <label>字号</label>
        <input
          type="number"
          :value="el.font_size ?? 18"
          min="8"
          @input="(e) => set('font_size', Number((e.target as HTMLInputElement).value))"
        />
      </div>
      <div class="row">
        <label>文字色</label>
        <input
          type="color"
          :value="withHash(el.color ?? '#000000')"
          @input="(e) => set('color', (e.target as HTMLInputElement).value.slice(1))"
        />
      </div>
      <div class="row">
        <label>水平</label>
        <select :value="el.align ?? 'left'" @change="(e) => set('align', (e.target as HTMLSelectElement).value)">
          <option value="left">左对齐</option>
          <option value="center">居中</option>
          <option value="right">右对齐</option>
          <option value="justify">两端</option>
        </select>
      </div>
      <div class="row">
        <label>垂直</label>
        <select
          :value="el.vert_align ?? 'middle'"
          @change="(e) => set('vert_align', (e.target as HTMLSelectElement).value)"
        >
          <option value="top">顶部</option>
          <option value="middle">居中</option>
          <option value="bottom">底部</option>
        </select>
      </div>
      <div class="row">
        <label>行高</label>
        <input
          type="number"
          :value="el.line_spacing ?? 1.2"
          min="0.8"
          max="3"
          step="0.1"
          @input="(e) => set('line_spacing', Number((e.target as HTMLInputElement).value))"
        />
      </div>
    </template>
  </div>
</template>
