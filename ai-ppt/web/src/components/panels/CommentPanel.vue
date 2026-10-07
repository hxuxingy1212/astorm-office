<script setup lang="ts">
// 右侧批注面板：显示元素路径 + 批注输入
import { computed } from 'vue'
import { useCommentsStore } from '@/stores/comments'

const comments = useCommentsStore()

const hasPath = computed(() => !!comments.targetPath)

function save() {
  comments.save()
}

function remove() {
  comments.text = ''
  comments.save()
}

function exit() {
  comments.stop()
}
</script>

<template>
  <aside class="comment-panel">
    <div class="cp-head">
      <span class="cp-title">
        <svg viewBox="0 0 20 20" width="15" height="15"><path d="M4 3h12a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2H9l-4 3v-3H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" fill="#4472C4"/></svg>
        批注
      </span>
      <button class="cp-exit" title="退出批注" @click="exit">✕</button>
    </div>

    <div class="cp-section">
      <label>元素路径</label>
      <code v-if="hasPath" class="cp-path">{{ comments.targetPath }}</code>
      <p v-else class="cp-empty">请点击画布中的元素<br />（批注模式中）</p>
    </div>

    <div class="cp-section">
      <label>批注内容</label>
      <textarea
        v-model="comments.text"
        class="cp-textarea"
        :disabled="!hasPath"
        placeholder="在此输入批注…"
      />
    </div>

    <div class="cp-actions">
      <button class="cp-save" :disabled="!hasPath" @click="save">保存批注</button>
      <button class="cp-clear" :disabled="!hasPath" @click="remove">清除</button>
    </div>

    <p class="cp-hint">共 {{ Object.keys(comments.items).length }} 页标记了批注</p>
  </aside>
</template>

<style scoped>
.comment-panel {
  width: 240px;
  background: #fff;
  border-left: 1px solid #e5e5e5;
  overflow-y: auto;
  padding: 12px;
  box-sizing: border-box;
  flex-shrink: 0;
}
.cp-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 12px;
}
.cp-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 14px;
  font-weight: 600;
  color: #1f2329;
}
.cp-exit {
  border: none;
  background: transparent;
  cursor: pointer;
  color: #888;
  font-size: 14px;
  padding: 4px;
}
.cp-exit:hover {
  color: #e5484d;
}
.cp-section {
  margin-bottom: 12px;
}
.cp-section label {
  display: block;
  font-size: 12px;
  color: #888;
  margin-bottom: 6px;
}
.cp-path {
  display: block;
  padding: 8px 10px;
  background: #f4f6fa;
  border: 1px solid #e2e6ee;
  border-radius: 4px;
  font-size: 13px;
  color: #4472c4;
  word-break: break-all;
}
.cp-empty {
  font-size: 12px;
  color: #999;
  line-height: 1.6;
  padding: 6px 0;
}
.cp-textarea {
  width: 100%;
  min-height: 120px;
  box-sizing: border-box;
  padding: 8px;
  border: 1px solid #d8d8d8;
  border-radius: 4px;
  font-size: 13px;
  resize: vertical;
  outline: none;
}
.cp-textarea:focus {
  border-color: #4472c4;
}
.cp-textarea:disabled {
  background: #f7f8fa;
  color: #999;
}
.cp-actions {
  display: flex;
  gap: 8px;
  margin-bottom: 12px;
}
.cp-save {
  flex: 1;
  padding: 7px 0;
  border: none;
  border-radius: 4px;
  background: #4472c4;
  color: #fff;
  cursor: pointer;
  font-size: 13px;
}
.cp-save:disabled {
  background: #b9c6de;
  cursor: not-allowed;
}
.cp-clear {
  flex: 0 0 auto;
  padding: 7px 12px;
  border: 1px solid #e3e5e9;
  background: #fff;
  border-radius: 4px;
  color: #666;
  cursor: pointer;
  font-size: 13px;
}
.cp-clear:hover:not(:disabled) {
  border-color: #e5484d;
  color: #e5484d;
}
.cp-clear:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
.cp-hint {
  font-size: 11px;
  color: #999;
}
</style>
