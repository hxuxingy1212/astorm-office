<script setup lang="ts">
// 高级组件属性面板（图表数据/进度值/KPI 等）
import { computed } from 'vue'
import type { Element } from '@/types/presentation'

const props = defineProps<{ el: Element }>()
const emit = defineEmits<{ (e: 'update', props: Partial<Element>): void }>()

function set(prop: string, value: unknown) {
  emit('update', { [prop]: value } as Partial<Element>)
}

const dataText = computed(() => {
  const el = props.el as { data?: number[] }
  return (el.data ?? []).join(', ')
})
const labelsText = computed(() => {
  const el = props.el as { labels?: string[] }
  return (el.labels ?? []).join(', ')
})
const colorsText = computed(() => {
  const el = props.el as { colors?: string[] }
  return (el.colors ?? []).join(', ')
})

function parseNumbers(s: string): number[] {
  return s
    .split(',')
    .map((v) => Number(v.trim()))
    .filter((v) => !Number.isNaN(v))
}
function parseStrings(s: string): string[] {
  return s
    .split(',')
    .map((v) => v.trim())
    .filter((v) => v.length > 0)
}
</script>

<template>
  <div class="panel">
    <h4>组件</h4>

    <!-- 进度类 -->
    <template v-if="el.type === 'progressBar' || el.type === 'progressRing'">
      <div class="row">
        <label>进度值</label>
        <input
          type="number"
          :value="el.value ?? 50"
          min="0"
          max="100"
          @input="(e) => set('value', Number((e.target as HTMLInputElement).value))"
        />
      </div>
      <div class="row">
        <label>主色</label>
        <input
          type="color"
          :value="`#${el.color ?? '4472C4'}`"
          @input="(e) => set('color', (e.target as HTMLInputElement).value.slice(1))"
        />
      </div>
      <div class="row">
        <label>轨道色</label>
        <input
          type="color"
          :value="`#${el.track_color ?? 'E0E0E0'}`"
          @input="(e) => set('track_color', (e.target as HTMLInputElement).value.slice(1))"
        />
      </div>
    </template>

    <!-- 图表类 -->
    <template v-else-if="['barChart', 'lineChart', 'pieChart', 'ringChart'].includes(el.type)">
      <label>数据（逗号分隔）</label>
      <input
        :value="dataText"
        @input="(e) => set('data', parseNumbers((e.target as HTMLInputElement).value))"
      />
      <label>标签（逗号分隔）</label>
      <input
        :value="labelsText"
        @input="(e) => set('labels', parseStrings((e.target as HTMLInputElement).value))"
      />
      <label>颜色（逗号分隔 hex）</label>
      <input
        :value="colorsText"
        @input="(e) => set('colors', parseStrings((e.target as HTMLInputElement).value))"
      />
    </template>

    <!-- KPI 卡片 -->
    <template v-else-if="el.type === 'kpiCard'">
      <label>主数值</label>
      <input :value="el.value" @input="(e) => set('value', (e.target as HTMLInputElement).value)" />
      <label>标签</label>
      <input
        :value="el.label ?? ''"
        @input="(e) => set('label', (e.target as HTMLInputElement).value || undefined)"
      />
      <label>变化</label>
      <input
        :value="el.delta ?? ''"
        @input="(e) => set('delta', (e.target as HTMLInputElement).value || undefined)"
      />
    </template>

    <!-- 星级评分 -->
    <template v-else-if="el.type === 'ratingStars'">
      <div class="row">
        <label>评分</label>
        <input
          type="number"
          :value="el.rating"
          min="0"
          :max="el.max ?? 5"
          step="0.5"
          @input="(e) => set('rating', Number((e.target as HTMLInputElement).value))"
        />
      </div>
      <div class="row">
        <label>总数</label>
        <input
          type="number"
          :value="el.max ?? 5"
          min="1"
          max="10"
          @input="(e) => set('max', Number((e.target as HTMLInputElement).value))"
        />
      </div>
    </template>

    <!-- 流程步骤 -->
    <template v-else-if="el.type === 'processFlow'">
      <label>步骤（逗号分隔）</label>
      <input
        :value="(el.steps ?? []).join(', ')"
        @input="(e) => set('steps', parseStrings((e.target as HTMLInputElement).value))"
      />
    </template>

    <p v-else class="hint">该组件暂不支持属性编辑（可移动/缩放）</p>
  </div>
</template>
