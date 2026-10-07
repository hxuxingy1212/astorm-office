<script setup lang="ts">
// 图标属性面板：选择图标 + 颜色
import { computed } from 'vue'
import type { IconElement } from '@/types/presentation'
import { withHash } from '@/utils/convert'
import { ICON_LIST, iconDef } from '@/configs/icons'

const props = defineProps<{ el: IconElement }>()
const emit = defineEmits<{ (e: 'update', props: Partial<IconElement>): void }>()

function set(prop: string, value: unknown) {
  emit('update', { [prop]: value } as Partial<IconElement>)
}

const label = computed(() => iconDef(props.el.icon)?.label ?? '自定义')
</script>

<template>
  <div class="panel">
    <h4>图标</h4>
    <label>图标（{{ label }}）</label>
    <div class="icon-grid">
      <div
        v-for="ic in ICON_LIST"
        :key="ic.name"
        class="icon-cell"
        :class="{ active: el.icon === ic.name }"
        :title="ic.label"
        @click="set('icon', ic.name)"
      >
        <svg viewBox="0 0 24 24" width="18" height="18">
          <path v-for="(d, i) in ic.paths" :key="i" :d="d" fill="currentColor" />
        </svg>
      </div>
    </div>
    <div class="row">
      <label>颜色</label>
      <input
        type="color"
        :value="withHash(el.color ?? '#4472C4')"
        @input="(e) => set('color', (e.target as HTMLInputElement).value.slice(1))"
      />
    </div>
  </div>
</template>

<style scoped>
.icon-grid {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 4px;
  margin: 8px 0;
  max-height: 220px;
  overflow-y: auto;
}
.icon-cell {
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px;
  border: 1px solid transparent;
  cursor: pointer;
  color: #555;
}
.icon-cell:hover {
  background: #eef3fb;
}
.icon-cell.active {
  border-color: #4472c4;
  color: #4472c4;
}
</style>
