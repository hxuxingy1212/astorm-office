//! docx 物理分页（纯逻辑，可离线单测）
//!
//! 产物模型里的 `parts` 是**编辑分片**（分节符 / 一级标题切块），不是页面：一个分片
//! 往往横跨多页。本模块把分片内的块序列切成物理页：
//!   · 硬分页——`page_break` 块（页在此结束，块本身不渲染）与块的 `page_break_before`；
//!   · 软分页——块自身底端超出页面可用高度时，从该块起另起一页（块不可再切）。
//! 高度由调用方离屏度量给出（浏览器排版 ≠ Word 排版，只求页数与分页位置近似）。

/** 单个块的分页信息（与块流一一对应） */
export interface PaginateBlock {
  /** 块顶端/底端相对流起点的自然 px；不参与排版的块（分页符/未度量）为 null */
  top: number | null
  bottom: number | null
  /** 块前强制分页（page_break_before / 分节符 / 纸张变化） */
  breakBefore?: boolean
  /** 该块自身是分页符：当前页在此结束 */
  breakHere?: boolean
  /** 本块所属分片的页面可用高度（分片纸张/页边距可以不同；缺省用调用参数） */
  pageH?: number
}

/** 物理页在块流中的区间与内容范围（top/bottom 相对流起点） */
export interface DocxPageRange {
  /** 块区间 [from, to)（流下标） */
  from: number
  to: number
  top: number
  bottom: number
}

/**
 * 把块流切成物理页。
 *
 * @param blocks   逐块分页信息（长度 = 块数，坐标同一坐标系）
 * @param pageH    页面可用高度（纸张高 − 上下页边距）缺省值；块自带 pageH 时以块为准
 * @param contentH 度量到的内容总高（末块底端缺失时兜底）
 */
export function paginateBlocks(
  blocks: readonly PaginateBlock[],
  pageH: number,
  contentH: number,
): DocxPageRange[] {
  const pages: DocxPageRange[] = []
  /** 收口一页：顶端取首块，底端取末块自身（缺省用内容末尾） */
  const close = (from: number, to: number) => {
    const first = blocks[from]
    const last = blocks[to - 1]
    const top = first?.top ?? 0
    pages.push({ from, to, top, bottom: last?.bottom ?? contentH })
  }

  let from = -1
  let startTop = 0
  for (let i = 0; i < blocks.length; i += 1) {
    const b = blocks[i]
    if (b.breakHere) {
      if (from >= 0) {
        close(from, i)
        from = -1
      }
      continue
    }
    if (b.breakBefore && from >= 0) {
      close(from, i)
      from = -1
    }
    if (from < 0) {
      from = i
      startTop = b.top ?? 0
      continue
    }
    // 未度量的块：留在本页，不做溢出判断
    if (b.top === null || b.bottom === null) continue
    // 本块能否留在当前页（按块自身底端判断；块后间距留给下一页的开头）
    if (b.bottom - startTop > (b.pageH ?? pageH)) {
      close(from, i)
      from = i
      startTop = b.top
    }
  }
  if (from >= 0) close(from, blocks.length)
  if (!pages.length) pages.push({ from: 0, to: 0, top: 0, bottom: 0 })
  return pages
}
