<script setup lang="ts">
// 批注浮层：显示所选元素路径（CLI 语法）+ 批注输入（在底部工具条上方居中）
import { computed } from 'vue'
import { useCommentsStore } from '@/stores/comments'
import { useSlidesStore } from '@/stores/slides'
import { elementCliPath } from '@/utils/path'

const comments = useCommentsStore()
const slides = useSlidesStore()

const hasPath = computed(() => !!comments.targetPath)

/** CLI 路径：/slide[N]/type[K] */
const cliPath = computed(() =>
  elementCliPath(comments.activeSlideIndex ?? 0, comments.targetPath, slides.slides),
)

function save() {
  comments.save()
}
function clear() {
  comments.text = ''
  comments.save()
}
</script>

<template>
  <div class="annotation-card" @mousedown.stop>
    <div class="ac-head">
      <span class="ac-title">批注 · 元素路径</span>
      <code v-if="hasPath" class="ac-path">{{ cliPath }}</code>
      <span v-else class="ac-empty">点击画布中的元素</span>
    </div>
    <textarea
      v-model="comments.text"
      :disabled="!hasPath"
      placeholder="在此输入批注…"
      class="ac-textarea"
    />
    <div class="ac-actions">
      <button class="ac-save" :disabled="!hasPath" @click="save">保存</button>
      <button class="ac-clear" :disabled="!hasPath" @click="clear">清除</button>
    </div>
  </div>
</template>

<style scoped>
.annotation-card {
  position: fixed;
  left: 50%;
  bottom: 78px;
  transform: translateX(-50%);
  z-index: 101;
  width: 340px;
  max-width: calc(100vw - 32px);
  background: #fff;
  border: 1px solid #e6e8ec;
  border-radius: 12px;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.16);
  padding: 12px 14px;
  user-select: none;
}
.ac-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-bottom: 8px;
}
.ac-title {
  font-size: 12px;
  color: #888;
  flex-shrink: 0;
}
.ac-path {
  display: inline-block;
  padding: 3px 8px;
  background: #f4f6fa;
  border: 1px solid #e2e6ee;
  border-radius: 4px;
  font-size: 11px;
  color: #4472c4;
  word-break: break-all;
}
.ac-empty {
  font-size: 12px;
  color: #999;
}
.ac-textarea {
  width: 100%;
  min-height: 90px;
  box-sizing: border-box;
  padding: 8px;
  border: 1px solid #d8d8d8;
  border-radius: 8px;
  font-size: 13px;
  resize: vertical;
  outline: none;
  font-family: inherit;
}
.ac-textarea:focus {
  border-color: #4472c4;
}
.ac-textarea:disabled {
  background: #f7f8fa;
  color: #999;
}
.ac-actions {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}
.ac-save {
  flex: 1;
  padding: 7px 0;
  border: none;
  border-radius: 6px;
  background: #4472c4;
  color: #fff;
  cursor: pointer;
  font-size: 13px;
}
.ac-save:disabled {
  background: #b9c6de;
  cursor: not-allowed;
}
.ac-clear {
  flex: 0 0 auto;
  padding: 7px 14px;
  border: 1px solid #e3e5e9;
  background: #fff;
  border-radius: 6px;
  color: #666;
  cursor: pointer;
  font-size: 13px;
}
.ac-clear:hover:not(:disabled) {
  border-color: #e5484d;
  color: #e5484d;
}
.ac-clear:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
