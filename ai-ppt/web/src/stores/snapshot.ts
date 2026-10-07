//! 撤销/重做（快照栈，20 条上限，防抖提交）

import { defineStore } from 'pinia'
import type { Slide } from '@/types/presentation'

interface Snapshot {
  slides: Slide[]
  slideIndex: number
}

let debounceTimer: ReturnType<typeof setTimeout> | null = null

export const useSnapshotStore = defineStore('snapshot', {
  state: () => ({
    undoStack: [] as Snapshot[],
    redoStack: [] as Snapshot[],
  }),

  getters: {
    canUndo: (state) => state.undoStack.length > 0,
    canRedo: (state) => state.redoStack.length > 0,
  },

  actions: {
    /** 提交快照（防抖 300ms，连续拖拽只记一次） */
    addHistorySnapshot(slides: Slide[], slideIndex: number) {
      if (debounceTimer) clearTimeout(debounceTimer)
      debounceTimer = setTimeout(() => {
        const snap: Snapshot = {
          slides: JSON.parse(JSON.stringify(slides)) as Slide[],
          slideIndex,
        }
        this.undoStack.push(snap)
        if (this.undoStack.length > 20) this.undoStack.shift()
        this.redoStack = []
      }, 300)
    },

    /** 强制提交（立即生效，用于撤销后不可再提交的场景） */
    flush() {
      if (debounceTimer) {
        clearTimeout(debounceTimer)
        debounceTimer = null
      }
    },

    undo(): Snapshot | null {
      this.flush()
      const snap = this.undoStack.pop()
      if (!snap) return null
      this.redoStack.push(snap)
      return snap
    },

    redo(): Snapshot | null {
      const snap = this.redoStack.pop()
      if (!snap) return null
      this.undoStack.push(snap)
      return snap
    },
  },
})
