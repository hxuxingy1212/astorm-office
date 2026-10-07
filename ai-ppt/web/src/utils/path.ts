//! 元素路径工具：把内部「数字索引路径」转为 CLI 路径语法（与后端 get/view 一致）
//!
//! CLI 语法：`/slide[N]/type[K]`，K 是该容器内同类型元素的 1-based 序号（递归支持 group）。
//! 内部路径如 `0` / `2.1` 表示 elements[0] / elements[2].children[1]。

import { parsePath } from '@/stores/slides'
import type { Slide } from '@/types/presentation'

/** 数字索引路径 → CLI 路径（如 /slide[1]/text[2]）；path 为空返回 /slide[N] */
export function elementCliPath(slideIndex: number, path: string, slides: Slide[]): string {
  const segs = parsePath(path)
  let container = slides[slideIndex]?.elements ?? []
  const nodes: string[] = [`slide[${slideIndex + 1}]`]
  for (const idx of segs) {
    const el = container[idx]
    if (!el) break
    const t = el.type
    let k = 0
    for (let j = 0; j <= idx; j += 1) {
      if (container[j].type === t) k += 1
    }
    nodes.push(`${t}[${k}]`)
    if (el.type === 'group') container = el.children
    else break
  }
  return `/${nodes.join('/')}`
}
