<script setup lang="ts">
// 文本属性面板
import { computed } from 'vue'
import type { TextElement } from '@/types/presentation'
import { withHash } from '@/utils/convert'
import { textContentPlain } from '@/utils/text'

const props = defineProps<{ el: TextElement }>()
const emit = defineEmits<{ (e: 'update', props: Partial<TextElement>): void }>()

const plain = computed(() => textContentPlain(props.el.text))

function set(prop: string, value: unknown) {
  emit('update', { [prop]: value } as Partial<TextElement>)
}
</script>

<template>
  <div class="panel">
    <h4>文本</h4>
    <label>内容</label>
    <textarea
      :value="plain"
      rows="3"
      @input="(e) => set('text', (e.target as HTMLTextAreaElement).value)"
    />
    <div class="row">
      <label>字号</label>
      <input
        type="number"
        :value="el.font_size ?? 18"
        min="8"
        max="200"
        @input="(e) => set('font_size', Number((e.target as HTMLInputElement).value))"
      />
    </div>
    <div class="row">
      <label>颜色</label>
      <input
        type="color"
        :value="withHash(el.color ?? '#000000')"
        @input="(e) => set('color', (e.target as HTMLInputElement).value.slice(1))"
      />
    </div>
    <div class="row">
      <label>字体</label>
      <select :value="el.font_family ?? ''" @change="(e) => set('font_family', (e.target as HTMLSelectElement).value || undefined)">
        <option value="">默认</option>
        <option value="Microsoft YaHei">微软雅黑</option>
        <option value="SimSun">宋体</option>
        <option value="Arial">Arial</option>
        <option value="Calibri">Calibri</option>
        <option value="Noto Sans SC">Noto Sans SC</option>
      </select>
    </div>
    <div class="row">
      <label>对齐</label>
      <select :value="el.align ?? 'left'" @change="(e) => set('align', (e.target as HTMLSelectElement).value)">
        <option value="left">左对齐</option>
        <option value="center">居中</option>
        <option value="right">右对齐</option>
        <option value="justify">两端</option>
      </select>
    </div>
    <div class="row">
      <label>样式</label>
      <span class="btn-group">
        <button :class="{ active: el.bold }" @click="set('bold', !el.bold)">B</button>
        <button :class="{ active: el.italic }" @click="set('italic', !el.italic)">I</button>
        <button :class="{ active: el.underline }" @click="set('underline', !el.underline)">U</button>
      </span>
    </div>
    <div class="row">
      <label>垂直</label>
      <select
        :value="el.vert_align ?? 'top'"
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
  </div>
</template>
