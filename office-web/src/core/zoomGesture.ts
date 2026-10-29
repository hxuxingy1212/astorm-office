//! 画布手势缩放装配：四个 viewer 的滚动容器共用。
//! 手势约定与生态一致（Word / 浏览器 / Figma）——
//!   · Ctrl/⌘ + 鼠标滚轮：缩放（macOS 触控板捏合在浏览器里就是合成的 ctrlKey wheel，同一通路）；
//!   · 触屏双指捏合：缩放；
//!   · 普通滚轮 / 单指：保持滚动语义，不抢。
//! 缩放以指针（捏合中点）为锚修正滚动位置；倍率应用走 setZoom 回调——
//! 自主模式写内部 ref（滑杆同步），受控模式汇报宿主（zoom-change 回环）。

import { nextTick, onBeforeUnmount, watch, type Ref } from 'vue'
import { pinchZoomAt, scrollAnchor, wheelZoomAt } from '@/core/zoomMath'

export interface ZoomGestureOptions {
  /** 画布（滚动容器）：事件挂载点与滚动锚定基准 */
  el: Ref<HTMLElement | null>
  /** 当前用户倍率（相对适配基准或自然尺寸） */
  getZoom: () => number
  /** 应用新倍率：自主模式写内部 ref；受控模式汇报宿主 */
  setZoom: (z: number) => void
  /** 倍率钳制范围（与各 viewer 内置滑杆一致） */
  min?: number
  max?: number
}

export function useZoomGesture(opts: ZoomGestureOptions): void {
  const min = opts.min ?? 0.5
  const max = opts.max ?? 3
  let el: HTMLElement | null = null
  /** 进行中的捏合：起点两指距离/倍率 */
  let pinch: { dist: number; zoom: number } | null = null
  /** 最近一次手势产出的倍率（受控模式下宿主回写 prop 前的连续事件从这里累积） */
  let flow = Number.NaN
  let flowAt = 0

  /** 本次手势的基准倍率：宿主已跟上上一次汇报值（或距上次手势较久）就用宿主值；
   *  否则沿用 flow——同一帧内连发的 wheel（触控板捏合常见）不能每次都从旧 prop 出发，
   *  否则中间的增量会被吞掉 */
  function baseZoom(): number {
    const host = Math.min(max, Math.max(min, opts.getZoom()))
    if (Number.isNaN(flow) || Math.abs(host - flow) < 1e-9 || Date.now() - flowAt > 300) return host
    return flow
  }

  function applyGesture(prev: number, next: number, clientX: number, clientY: number): void {
    if (next === prev) return
    anchorWith(next / prev, clientX, clientY)
    flow = next
    flowAt = Date.now()
    opts.setZoom(next)
  }

  /** 以指针/捏合中点为锚修正滚动：等 Vue 按新倍率重排后再写 scroll——
   *  立即写会被旧内容尺寸钳掉（放大时内容还没变长） */
  function anchorWith(ratio: number, clientX: number, clientY: number): void {
    const node = el
    if (!node || !(ratio > 0) || ratio === 1) return
    const rect = node.getBoundingClientRect()
    const ox = clientX - rect.left
    const oy = clientY - rect.top
    void nextTick(() => {
      node.scrollLeft = scrollAnchor(node.scrollLeft, ox, ratio)
      node.scrollTop = scrollAnchor(node.scrollTop, oy, ratio)
    })
  }

  function onWheel(ev: WheelEvent): void {
    if (!ev.ctrlKey && !ev.metaKey) return // 普通滚轮 = 滚动
    ev.preventDefault() // 拦下浏览器对 ctrl+wheel 的页面缩放
    const prev = baseZoom()
    const next = wheelZoomAt(prev, ev.deltaY, ev.deltaMode, min, max)
    applyGesture(prev, next, ev.clientX, ev.clientY)
  }

  function onTouchStart(ev: TouchEvent): void {
    if (ev.touches.length !== 2) return
    const a = ev.touches[0]
    const b = ev.touches[1]
    if (!a || !b) return
    pinch = {
      dist: Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY),
      zoom: baseZoom(),
    }
  }

  function onTouchMove(ev: TouchEvent): void {
    if (!pinch || ev.touches.length < 2) return
    ev.preventDefault() // 双指捏合只调预览缩放，不滚动也不触发浏览器缩放
    const a = ev.touches[0]
    const b = ev.touches[1]
    if (!a || !b) return
    const dist = Math.hypot(a.clientX - b.clientX, a.clientY - b.clientY)
    const next = pinchZoomAt(pinch.zoom, pinch.dist, dist, min, max)
    applyGesture(pinch.zoom, next, (a.clientX + b.clientX) / 2, (a.clientY + b.clientY) / 2)
  }

  function onTouchEnd(ev: TouchEvent): void {
    if (!ev.touches || ev.touches.length < 2) pinch = null
  }

  function detach(): void {
    if (!el) return
    el.removeEventListener('wheel', onWheel)
    el.removeEventListener('touchstart', onTouchStart)
    el.removeEventListener('touchmove', onTouchMove)
    el.removeEventListener('touchend', onTouchEnd)
    el.removeEventListener('touchcancel', onTouchEnd)
    el = null
    pinch = null
    flow = Number.NaN
  }

  watch(
    opts.el,
    (node) => {
      detach()
      if (!node) return
      el = node
      // wheel/touchmove 要 preventDefault：必须非 passive 挂载
      node.addEventListener('wheel', onWheel, { passive: false })
      node.addEventListener('touchstart', onTouchStart, { passive: true })
      node.addEventListener('touchmove', onTouchMove, { passive: false })
      node.addEventListener('touchend', onTouchEnd)
      node.addEventListener('touchcancel', onTouchEnd)
    },
    { immediate: true },
  )
  onBeforeUnmount(detach)
}
