<script setup lang="ts">
// 线条属性面板
import { computed } from 'vue'
import type { LineElement } from '@/types/presentation'
import { withHash } from '@/utils/convert'

const props = defineProps<{ el: LineElement }>()
const emit = defineEmits<{ (e: 'update', props: Partial<LineElement>): void }>()

const el = computed(() => props.el)

function set(prop: string, value: unknown) {
  emit('update', { [prop]: value } as Partial<LineElement>)
}
</script>

<template>
  <div class="panel">
    <h4>线条</h4>
    <div class="row">
      <label>颜色</label>
      <input
        type="color"
        :value="withHash(el.color ?? '#4472C4')"
        @input="(e) => set('color', (e.target as HTMLInputElement).value.slice(1))"
      />
    </div>
    <div class="row">
      <label>宽度</label>
      <input
        type="number"
        :value="el.width ?? 1"
        min="0.5"
        max="30"
        step="0.5"
        @input="(e) => set('width', Number((e.target as HTMLInputElement).value))"
      />
    </div>
    <div class="row">
      <label>虚线</label>
      <select :value="el.dash ?? 'solid'" @change="(e) => set('dash', (e.target as HTMLSelectElement).value)">
        <option value="solid">实线</option>
        <option value="dashed">虚线</option>
        <option value="dotted">点线</option>
      </select>
    </div>
    <div class="row">
      <label>起点</label>
      <select
        :value="el.arrow_start ?? 'none'"
        @change="(e) => set('arrow_start', (e.target as HTMLSelectElement).value)"
      >
        <option value="none">无</option>
        <option value="arrow">箭头</option>
        <option value="dot">圆点</option>
      </select>
    </div>
    <div class="row">
      <label>终点</label>
      <select
        :value="el.arrow_end ?? 'none'"
        @change="(e) => set('arrow_end', (e.target as HTMLSelectElement).value)"
      >
        <option value="none">无</option>
        <option value="arrow">箭头</option>
        <option value="dot">圆点</option>
      </select>
    </div>
    <div class="row">
      <label>平滑</label>
      <input type="checkbox" :checked="!!el.smooth" @input="(e) => set('smooth', (e.target as HTMLInputElement).checked)" />
    </div>
  </div>
</template>
