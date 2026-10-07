//! 悬浮路径卡片状态：单一实例在 Viewer 内共享
//!
//! 用法：
//! ```ts
//! const hover = usePathHover()
//! // 元素上：@mouseenter="hover.enter(path, info, $event)" @mousemove="hover.move($event)" @mouseleave="hover.leave()"
//! ```
//! `<PathCard :state="hover.state.value" />` 负责渲染。

import { ref, type Ref } from 'vue'

export interface PathCardInfo {
  /** CLI 路径（如 /sheet[1]/cell[B2]） */
  path: string
  /** 元素类型（如 cell / shape / paragraph） */
  type: string
  /** 名称（可选，PPT 形状名/工作表名等） */
  name?: string
  /** 文本摘要（可选） */
  text?: string
  /** 位置/尺寸等附加信息（可选，逐条展示） */
  meta?: Record<string, string | number | undefined>
  /** 原始 JSON 片段（可选，卡片折叠展示） */
  json?: string
}

export interface PathCardState extends PathCardInfo {
  visible: boolean
  x: number
  y: number
}

const OFFSET = 16
const CARD_W = 360
const CARD_H = 200

/** 悬浮高亮：enter 时挂到触发元素上，离开/隐藏时摘除（四格式统一观感） */
const HOVER_CLASS = 'ov-hovering'
let highlightedEl: Element | null = null

function clearHighlight() {
  if (highlightedEl) {
    highlightedEl.classList.remove(HOVER_CLASS)
    highlightedEl = null
  }
}

function clampPosition(x: number, y: number): { x: number; y: number } {
  const vw = typeof window === 'undefined' ? 1440 : window.innerWidth
  const vh = typeof window === 'undefined' ? 900 : window.innerHeight
  return {
    x: Math.max(8, Math.min(x + OFFSET, vw - CARD_W - 8)),
    y: Math.max(8, Math.min(y + OFFSET, vh - CARD_H - 8)),
  }
}

export interface PathHoverApi {
  state: Ref<PathCardState>
  /** 进入元素：记录路径与信息 */
  enter(info: PathCardInfo, ev?: MouseEvent): void
  /** 跟随鼠标 */
  move(ev: MouseEvent): void
  /** 离开（带小延迟，便于移动到卡片上复制路径） */
  leave(): void
  /** 立即隐藏 */
  hide(): void
  /** 当前悬浮路径（无则 null） */
  current(): string | null
}

export function usePathHover(): PathHoverApi {
  const state = ref<PathCardState>({
    visible: false,
    path: '',
    type: '',
    x: 0,
    y: 0,
  })
  let hideTimer: ReturnType<typeof setTimeout> | null = null

  const enter = (info: PathCardInfo, ev?: MouseEvent) => {
    if (hideTimer) {
      clearTimeout(hideTimer)
      hideTimer = null
    }
    clearHighlight()
    const target = ev?.currentTarget
    if (target instanceof Element) {
      target.classList.add(HOVER_CLASS)
      highlightedEl = target
    }
    const pos = ev ? clampPosition(ev.clientX, ev.clientY) : { x: state.value.x, y: state.value.y }
    state.value = { ...info, visible: true, x: pos.x, y: pos.y }
  }

  const move = (ev: MouseEvent) => {
    if (!state.value.visible) return
    const pos = clampPosition(ev.clientX, ev.clientY)
    state.value = { ...state.value, x: pos.x, y: pos.y }
  }

  const leave = () => {
    if (hideTimer) clearTimeout(hideTimer)
    hideTimer = setTimeout(() => {
      state.value = { ...state.value, visible: false }
      clearHighlight()
      hideTimer = null
    }, 180)
  }

  const hide = () => {
    if (hideTimer) clearTimeout(hideTimer)
    state.value = { ...state.value, visible: false }
    clearHighlight()
  }

  const current = () => (state.value.visible ? state.value.path : null)

  return { state, enter, move, leave, hide, current }
}

/** JSON 片段：截断到 n 个字符（路径卡片折叠展示用） */
export function jsonSnippet(value: unknown, n = 800): string {
  try {
    const s = JSON.stringify(value, null, 2)
    return s.length > n ? `${s.slice(0, n)}\n…` : s
  } catch {
    return ''
  }
}
