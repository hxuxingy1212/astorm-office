//! 编辑器 UI 状态 store

import { defineStore } from 'pinia'

export const useEditorStore = defineStore('editor', {
  state: () => ({
    /** 选中元素路径列表（支持多选） */
    activeElementIds: [] as string[],
    /** 正在操作的元素（单选，最后点击的） */
    handleElementId: '',
    /** 画布缩放百分比（30 ~ 200） */
    canvasPercentage: 90,
    /** 拖拽/缩放进行中（用于抑制快照） */
    isScaling: false,
    /** 面板可见性 */
    showNotes: false,
    /** 拖拽绘制插入模式（如文本框）：null 表示未开启 */
    drawMode: null as { type: 'text'; vert: string } | null,
  }),

  getters: {
    canvasScale(state): number {
      return state.canvasPercentage / 100
    },
    handleElementIdSafe(state): string {
      return state.handleElementId || state.activeElementIds[0] || ''
    },
  },

  actions: {
    select(path: string, additive = false) {
      if (additive) {
        if (this.activeElementIds.includes(path)) {
          this.activeElementIds = this.activeElementIds.filter((p) => p !== path)
        } else {
          this.activeElementIds = [...this.activeElementIds, path]
        }
      } else {
        this.activeElementIds = [path]
      }
      this.handleElementId = path
    },

    clearSelection() {
      this.activeElementIds = []
      this.handleElementId = ''
    },

    /** 开启拖拽绘制插入模式 */
    startDraw(type: 'text', vert: string) {
      this.drawMode = { type, vert }
      this.clearSelection()
    },

    /** 关闭拖拽绘制插入模式 */
    stopDraw() {
      this.drawMode = null
    },

    setCanvasPercentage(v: number) {
      this.canvasPercentage = Math.min(200, Math.max(30, v))
    },

    zoomIn() {
      this.setCanvasPercentage(this.canvasPercentage + 10)
    },

    zoomOut() {
      this.setCanvasPercentage(this.canvasPercentage - 10)
    },
  },
})
