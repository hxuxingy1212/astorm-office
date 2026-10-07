<script setup lang="ts">
// 公式属性面板：LaTeX 内容 + 颜色 + 字号
import type { FormulaElement } from '@/types/presentation'
import { withHash } from '@/utils/convert'

const props = defineProps<{ el: FormulaElement }>()
const emit = defineEmits<{ (e: 'update', props: Partial<FormulaElement>): void }>()

function set(prop: string, value: unknown) {
  emit('update', { [prop]: value } as Partial<FormulaElement>)
}
</script>

<template>
  <div class="panel">
    <h4>公式</h4>
    <label>LaTeX</label>
    <textarea
      :value="props.el.latex"
      rows="2"
      @input="(e) => set('latex', (e.target as HTMLTextAreaElement).value)"
    />
    <div class="row">
      <label>颜色</label>
      <input
        type="color"
        :value="withHash(props.el.color ?? '#000000')"
        @input="(e) => set('color', (e.target as HTMLInputElement).value.slice(1))"
      />
    </div>
    <div class="row">
      <label>字号</label>
      <input
        type="number"
        :value="props.el.font_size ?? 24"
        min="8"
        max="200"
        @input="(e) => set('font_size', Number((e.target as HTMLInputElement).value))"
      />
    </div>
  </div>
</template>
