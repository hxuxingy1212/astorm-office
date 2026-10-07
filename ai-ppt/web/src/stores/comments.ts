//! 批注（评论）store
//!
//! 右键/图标进入批注模式后，点击画布中的元素标记目标路径，
//! 右侧面板显示路径并写入文本。批注按「幻灯片索引 → 元素路径」存储。

import { defineStore } from 'pinia'

export const useCommentsStore = defineStore('comments', {
  state: () => ({
    /** 批注模式是否开启 */
    active: false,
    /** 当前标记的元素路径 */
    targetPath: '',
    /** 当前批注文本 */
    text: '',
    /** 当前批注所属幻灯片索引 */
    activeSlideIndex: undefined as number | undefined,
    /** 已保存批注：slideIndex → path → text */
    items: {} as Record<number, Record<string, string>>,
  }),

  getters: {
    /** 当前路径下已保存的批注（无则空串） */
    saved(state): string | null {
      const slide = state.items[state.activeSlideIndex ?? -1]
      return slide ? (slide[state.targetPath] ?? null) : null
    },
  },

  actions: {
    /** 进入批注模式 */
    start() {
      this.active = true
      this.targetPath = ''
      this.text = ''
    },

    /** 退出批注模式 */
    stop() {
      this.active = false
      this.targetPath = ''
      this.text = ''
    },

    /** 标记元素路径并载入已存批注 */
    setTarget(slideIndex: number, path: string) {
      this.targetPath = path
      this.text = this.items[slideIndex]?.[path] ?? ''
      this.activeSlideIndex = slideIndex
    },

    /** 保存当前批注（空文本则删除该条） */
    save() {
      if (!this.targetPath || this.activeSlideIndex === undefined) return
      const key = this.activeSlideIndex
      if (!this.items[key]) this.items[key] = {}
      if (this.text.trim()) this.items[key][this.targetPath] = this.text.trim()
      else delete this.items[key][this.targetPath]
    },

    /** 设为可编辑空文本 */
    reset() {
      this.text = ''
    },
  },
})
