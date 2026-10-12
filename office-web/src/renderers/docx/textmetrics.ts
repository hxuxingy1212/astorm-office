//! 文本量宽（canvas）：断行要用「同一个字体栈、同一个字号」下的真实字宽。
//!
//! canvas 与 DOM 用同一套字体匹配（同字体、同字号时 advance 一致），而在隐藏 canvas 上量宽
//! 不触碰文档布局——往页面里插 span 量一次就会触发一次全文档强制重排，150 段正文量几千次
//! 直接卡死。每个「字体简写 + 文本」组合缓存一次，全篇只有几百个不同组合。

const cache = new Map<string, number>()
let ctx: CanvasRenderingContext2D | null = null

function context(): CanvasRenderingContext2D | null {
  if (ctx) return ctx
  if (typeof document === 'undefined') return null
  const c = document.createElement('canvas')
  ctx = c.getContext('2d')
  return ctx
}

/** 文本在指定字体下的宽度（px）。量不到（SSR/无 canvas）时按 0 处理，调用方会退回浏览器断行。
 *
 *  与 DOM 实测同串对比：canvas 系统性偏窄约 0.004px/字（DOM 的 rect 带亚像素取整），
 *  这里按 0.005px/字 补回来——宁可略微高估也不低估：高估只会让断行早半像素，
 *  低估会让一行超出半个像素（悬挂/拉伸都会跟着错位）。 */
export function textWidth(text: string, font: string): number {
  if (!text) return 0
  const key = `${font}\u0000${text}`
  const hit = cache.get(key)
  if (hit !== undefined) return hit
  const c = context()
  if (!c) return 0
  c.font = font
  const w = c.measureText(text).width + 0.005 * text.length
  cache.set(key, w)
  return w
}

/** 字体就位/更换后字宽会变（webfont、字体兜底）：清缓存 */
export function resetTextWidthCache(): void {
  cache.clear()
}
