<script setup lang="ts">
// 可折叠设置分组：标题 + 展开/收起
import { ref, watch } from 'vue'

const props = defineProps<{
  title: string
  defaultOpen?: boolean
}>()

const open = ref(props.defaultOpen !== false)

watch(
  () => props.defaultOpen,
  (v) => {
    if (v !== undefined) open.value = v
  },
)
</script>

<template>
  <div class="section-group" :class="{ open }">
    <button class="section-head" @click="open = !open">
      <span class="section-caret" :class="{ expanded: open }">
        <svg viewBox="0 0 12 12" width="12" height="12"><path d="M3 4.5 6 8l3-3.5Z" fill="currentColor" /></svg>
      </span>
      <span class="section-title">{{ title }}</span>
    </button>
    <div v-show="open" class="section-body">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.section-group {
  border: 1px solid #e7e7e7;
  margin-bottom: 10px;
  overflow: hidden;
  background: #fff;
}
.section-head {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  padding: 10px 12px;
  border: none;
  background: #fafbfc;
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
  color: #333;
  text-align: left;
}
.section-head:hover {
  background: #f2f4f7;
}
.section-caret {
  display: inline-flex;
  transition: transform 0.18s ease;
  color: #8a8f99;
}
.section-caret.expanded {
  transform: rotate(90deg);
}
.section-title {
  flex: 1;
}
.section-body {
  padding: 10px 12px 12px;
}
</style>
