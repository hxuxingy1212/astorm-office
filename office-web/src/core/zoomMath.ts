//! 手势缩放纯函数：滚轮 / 触控捏合 → 用户倍率，以及缩放时的滚动锚定。
//! 与 DOM 装配（zoomGesture.ts 的 composable）分离，便于离线单测。

/** 指数缩放灵敏度：滚轮一格（ΔY≈100px）≈ ×1.25；触控板捏合的连续小 ΔY 得到平滑倍率 */
export const WHEEL_ZOOM_SENSITIVITY = 0.0022

function clampZoom(z: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, z))
}

/** 滚轮事件 → 新的用户倍率（钳制在 [min,max]；越界输入钳回边界）。
 *  deltaY 正 = 缩小（滚轮下滚），负 = 放大；deltaMode 1=行 / 2=页（Firefox 鼠标滚轮
 *  常报行），先归一到像素量级再进指数，保证各浏览器一格的观感一致。 */
export function wheelZoomAt(
  prev: number,
  deltaY: number,
  deltaMode = 0,
  min = 0.5,
  max = 3,
): number {
  const unit = deltaMode === 1 ? 16 : deltaMode === 2 ? 100 : 1
  return clampZoom(prev * Math.exp(-deltaY * unit * WHEEL_ZOOM_SENSITIVITY), min, max)
}

/** 双指捏合 → 新的用户倍率：距离比 = 倍率比（从捏合起点的绝对映射，无累计漂移）。 */
export function pinchZoomAt(
  prev: number,
  startDist: number,
  dist: number,
  min = 0.5,
  max = 3,
): number {
  if (!(startDist > 0)) return clampZoom(prev, min, max)
  return clampZoom(prev * (dist / startDist), min, max)
}

/** 缩放滚动锚定：让「滚动偏移 + 指针在视口内的偏移」这个内容点在缩放后仍停在指针下。
 *  内容小于视口时容器把结果钳回 0，自然退化为保持居中。ratio = 新倍率 / 旧倍率。 */
export function scrollAnchor(scroll: number, pointerOffset: number, ratio: number): number {
  return (scroll + pointerOffset) * ratio - pointerOffset
}
