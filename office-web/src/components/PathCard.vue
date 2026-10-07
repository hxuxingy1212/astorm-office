<script setup lang="ts">
//! 悬浮路径卡片：元素名 + CLI 路径 + 内容
import type { PathCardState } from '@/core/hover'

defineProps<{ state: PathCardState }>()
</script>

<template>
  <div
    v-if="state.visible"
    class="ov-pathcard"
    :style="{ left: `${state.x}px`, top: `${state.y}px` }"
    @mouseenter="$emit('keep')"
    @mouseleave="$emit('release')"
  >
    <div class="pc-head">
      <span class="pc-type">{{ state.type }}</span>
      <span v-if="state.name" class="pc-name">{{ state.name }}</span>
    </div>
    <code class="pc-path">{{ state.path }}</code>
    <div v-if="state.text" class="pc-text">{{ state.text }}</div>
  </div>
</template>

<style>
/* macOS 弹出层：毛玻璃 + 圆角 + 指向箭头 */
.ov-pathcard {
  position: fixed;
  z-index: 9999;
  width: 320px;
  max-height: 260px;
  overflow: auto;
  padding: 10px 12px 11px;
  border-radius: var(--ov-radius-popover, 12px);
  background: var(--ov-bg-popover);
  backdrop-filter: saturate(180%) blur(24px);
  -webkit-backdrop-filter: saturate(180%) blur(24px);
  color: var(--ov-label);
  font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'PingFang SC', 'Helvetica Neue',
    sans-serif;
  font-size: 12px;
  line-height: 1.45;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.2), 0 0 0 0.5px rgba(0, 0, 0, 0.08);
  pointer-events: auto;
}
.ov-pathcard::before {
  /* 指向箭头的近似（左上小三角） */
  content: '';
  position: absolute;
  left: 14px;
  top: -6px;
  width: 12px;
  height: 12px;
  background: var(--ov-bg-popover-arrow);
  transform: rotate(45deg);
  border-radius: 2px 0 0 0;
  box-shadow: -0.5px -0.5px 0 rgba(0, 0, 0, 0.06);
}
.ov-pathcard .pc-head {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 6px;
}
.ov-pathcard .pc-type {
  padding: 1px 7px;
  border-radius: 999px;
  background: var(--ov-accent-soft);
  color: var(--ov-accent-text);
  font-weight: 590;
  font-size: 11px;
}
.ov-pathcard .pc-name {
  color: var(--ov-label-2);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ov-pathcard .pc-path {
  display: block;
  padding: 3px 7px;
  border-radius: 5px;
  background: var(--ov-bg-segment);
  font-family: ui-monospace, 'SF Mono', SFMono-Regular, Menlo, monospace;
  font-size: 11.5px;
  color: var(--ov-text-strong);
  word-break: break-all;
}
.ov-pathcard .pc-text {
  margin-top: 6px;
  color: var(--ov-label-2);
  max-height: 96px;
  overflow: hidden;
}
</style>
