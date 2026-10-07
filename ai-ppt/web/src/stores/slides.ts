//! 幻灯片数据核心 store
//!
//! 持有 overview 数据（宽度/高度/主题/幻灯片列表）与全部编辑动作。
//! 元素通过「索引路径」定位（如 "3" 或 "2.1" = 第 3 个元素的分组内第 2 个子元素），
//! 因为产物 JSON 中元素没有唯一 id 字段。

import { defineStore } from 'pinia'
import type { Element, Overview, Slide } from '@/types/presentation'
import { inchToPx } from '@/utils/convert'

/** 解析元素索引路径："2.1" → [2, 1] */
export function parsePath(path: string): number[] {
  return path.split('.').map((s) => Number(s))
}

/** 按索引路径获取元素（slide 元素数组内定位，group 递归） */
export function getElementByPath(elements: Element[], path: number[]): Element | null {
  let current: Element[] = elements
  let el: Element | null = null
  for (const idx of path) {
    if (idx >= current.length) return null
    el = current[idx]
    if (el.type === 'group') current = el.children
  }
  return el
}

/** 按索引路径替换/删除元素（不可变更新） */
export function updateElements(
  elements: Element[],
  path: number[],
  mutate: (el: Element) => Element | null,
): Element[] {
  const idx = path[0]
  if (path.length === 1) {
    const el = elements[idx]
    if (!el) return elements
    const next = mutate(el)
    const copy = elements.slice()
    if (next === null) {
      copy.splice(idx, 1)
    } else {
      copy[idx] = next
    }
    return copy
  }
  return elements.map((el, i) => {
    if (i !== idx || el.type !== 'group') return el
    return { ...el, children: updateElements(el.children, path.slice(1), mutate) }
  })
}

/** 在容器（slide 或 group children）末尾追加元素 */
export function appendElement(elements: Element[], path: number[], el: Element): Element[] {
  if (path.length === 0) return [...elements, el]
  const idx = path[0]
  return elements.map((e, i) => {
    if (i !== idx || e.type !== 'group') return e
    return { ...e, children: appendElement(e.children, path.slice(1), el) }
  })
}

/** 查找元素所在容器（供追加） */
export function findContainer(elements: Element[], path: number[]): Element[] | null {
  if (path.length === 0) return elements
  const el = elements[path[0]]
  if (!el) return null
  if (el.type === 'group') return findContainer(el.children, path.slice(1))
  return null
}

export const useSlidesStore = defineStore('slides', {
  state: () => ({
    loaded: false,
    name: '',
    width: 13.333,
    height: 7.5,
    meta: null as Overview['meta'] | null,
    theme: null as Overview['theme'] | null,
    media: [] as string[],
    slides: [] as Slide[],
    slideIndex: 0,
  }),

  getters: {
    currentSlide(state): Slide | null {
      return state.slides[state.slideIndex] ?? null
    },
    canvasWidthPx(): number {
      return inchToPx(this.width)
    },
    canvasHeightPx(): number {
      return inchToPx(this.height)
    },
  },

  actions: {
    /** 载入 overview（上传后或刷新恢复） */
    loadOverview(overview: Overview) {
      this.name = overview.name
      this.width = overview.width
      this.height = overview.height
      this.meta = overview.meta ?? null
      this.theme = overview.theme ?? null
      this.media = overview.media ?? []
      this.slides = overview.slides.map((s) => s.data)
      this.slideIndex = 0
      this.loaded = true
    },

    setSlideIndex(index: number) {
      if (index >= 0 && index < this.slides.length) this.slideIndex = index
    },

    /** 设置画布尺寸（英寸，全局） */
    setCanvasSize(w: number, h: number) {
      if (w > 0 && h > 0) {
        this.width = w
        this.height = h
      }
    },

    /** 设置演示标题 */
    setDocTitle(title: string) {
      this.meta = { ...(this.meta ?? {}), title }
    },

    /** 应用主题（配色/字体） */
    applyTheme(theme: Overview['theme']) {
      this.theme = theme
    },

    /** 新增页：在当前选中页之后插入（blank=true 空白，否则复制当前页），并选中新页 */
    addSlide(blank = false) {
      const base: Slide = blank
        ? { elements: [] }
        : JSON.parse(JSON.stringify(this.currentSlide ?? { elements: [] })) as Slide
      const insertAt = this.slideIndex + 1
      this.slides.splice(insertAt, 0, base)
      this.slideIndex = insertAt
    },

    /** 删除当前页 */
    deleteSlide() {
      if (this.slides.length <= 1) return
      this.slides.splice(this.slideIndex, 1)
      if (this.slideIndex >= this.slides.length) this.slideIndex = this.slides.length - 1
    },

    /** 更新幻灯片级字段（背景/备注等） */
    updateSlide(props: Partial<Slide>) {
      const slide = this.currentSlide
      if (!slide) return
      this.slides[this.slideIndex] = { ...slide, ...props }
    },

    /** 替换元素（path 为空表示替换整个元素列表） */
    setElements(elements: Element[]) {
      const slide = this.currentSlide
      if (!slide) return
      this.slides[this.slideIndex] = { ...slide, elements }
    },

    /** 更新元素（props 浅合并；mutate 返回 null 表示删除） */
    updateElement(path: string, props: Partial<Element>) {
      const slide = this.currentSlide
      if (!slide) return
      const p = parsePath(path)
      const elements = updateElements(slide.elements, p, (el) => ({ ...el, ...props }) as Element)
      this.slides[this.slideIndex] = { ...slide, elements }
    },

    /** 替换元素（整体替换，用于拖拽过程中的本地副本提交） */
    replaceElement(path: string, el: Element) {
      const slide = this.currentSlide
      if (!slide) return
      const p = parsePath(path)
      const elements = updateElements(slide.elements, p, () => el)
      this.slides[this.slideIndex] = { ...slide, elements }
    },

    /** 删除元素 */
    deleteElement(path: string) {
      const slide = this.currentSlide
      if (!slide) return
      const p = parsePath(path)
      const elements = updateElements(slide.elements, p, () => null)
      this.slides[this.slideIndex] = { ...slide, elements }
    },

    /** 在容器（slide 或 group）下追加元素 */
    addElement(containerPath: string, el: Element) {
      const slide = this.currentSlide
      if (!slide) return
      const p = containerPath === '' ? [] : parsePath(containerPath)
      const elements = appendElement(slide.elements, p, el)
      this.slides[this.slideIndex] = { ...slide, elements }
    },
  },
})
