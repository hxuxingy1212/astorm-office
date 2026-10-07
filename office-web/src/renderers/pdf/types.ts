//! PDF 产物目录类型与元素几何辅助（与 json2pdf 的 model 对应）
//!
//! 坐标一律 pt、y 向上（文本 y 为基线）；颜色带 `#`。
//! 元素 8 类：text / rect / pattern_rect / path / polyline / shading / image / group。

export interface PdfPageSize {
  width: number
  height: number
}

export interface PdfMeta {
  title?: string
  author?: string
  subject?: string
  creator?: string
  producer?: string
}

/** PDF 元素（serde tagged union，type 判别；字段与 json2pdf model.rs 一致） */
export interface PdfElement {
  type: string
  // text
  text?: string
  x?: number
  y?: number
  w?: number
  h?: number
  size?: number
  font?: string
  font_id?: string
  color?: string
  rotation?: number
  /** 文本渲染模式（Tr：4-7 参与裁剪） */
  render_mode?: number
  alpha?: number
  fill?: string | null
  stroke?: string | null
  stroke_color?: string | null
  line_width?: number
  bold?: boolean
  italic?: boolean
  // polyline
  points?: [number, number][]
  close?: boolean
  dash?: number[] | null
  dash_phase?: number
  // path
  segments?: { op: string; points?: [number, number][] }[]
  even_odd?: boolean
  // image
  src?: string
  /** 预览图（unpack 对 JPX/JBIG2/CCITT 解出的 PNG，预览优先于 src） */
  preview?: string
  /** 裁剪路径（设备空间多边形） */
  clip?: { op: string; points?: [number, number][] }[]
  // shading
  kind?: string
  coords?: number[]
  stops?: { offset: number; color: string }[]
  bbox?: [number, number, number, number] | null
  // pattern_rect
  pattern?: unknown
  // group
  matrix?: [number, number, number, number, number, number]
  group?: unknown
  children?: PdfElement[]
}

export interface PdfPage {
  width?: number
  height?: number
  rotation?: number
  crop?: [number, number, number, number] | null
  mediabox?: [number, number, number, number] | null
  id?: number
  elements: PdfElement[]
}

export interface PdfDoc {
  version?: number
  meta?: PdfMeta
  page_size: PdfPageSize
  pages: PdfPage[]
}

/** 元素类型名（寻址段同名） */
export function pdfElementType(el: PdfElement): string {
  return el.type
}

/** 分组 children（唯一可下钻容器） */
export function pdfChildren(el: PdfElement): PdfElement[] | undefined {
  return el.type === 'group' ? el.children : undefined
}

/** 元素文本内容（仅 text） */
export function pdfText(el: PdfElement): string | undefined {
  return el.type === 'text' ? el.text : undefined
}

/** 元素包围盒（pt，页面坐标 y 向上；用于悬浮命中区与路径卡片） */
export function pdfElementBBox(
  el: PdfElement,
  pageW: number,
  pageH: number,
): [number, number, number, number] {
  const e = 0.5
  switch (el.type) {
    case 'text': {
      const size = el.size ?? 12
      const x = el.x ?? 0
      const y = el.y ?? 0
      const w = Math.min(pageW, Math.max(size, (el.text?.length ?? 1) * size * 0.6))
      return [x - e, Math.max(0, y - size - e), Math.min(pageW, x + w + e), Math.min(pageH, y + e)]
    }
    case 'rect':
    case 'pattern_rect':
    case 'image': {
      const x = el.x ?? 0
      const y = el.y ?? 0
      const w = el.w ?? 0
      const h = el.h ?? 0
      return [x - e, y - e, x + w + e, y + h + e]
    }
    case 'polyline': {
      const pts = el.points ?? []
      return bboxOfPoints(pts, pageW, pageH)
    }
    case 'path': {
      const pts = (el.segments ?? []).flatMap((s) => s.points ?? [])
      return bboxOfPoints(pts, pageW, pageH)
    }
    case 'shading': {
      const b = el.bbox
      if (b) return [b[0] - e, b[1] - e, b[2] + e, b[3] + e]
      const c = el.coords ?? []
      if (el.kind === 'radial' && c.length >= 3) return bboxOfPoints([[c[0], c[1]]], pageW, pageH)
      if (c.length >= 4) return bboxOfPoints([[c[0], c[1]], [c[2], c[3]]], pageW, pageH)
      return [0, 0, pageW, pageH]
    }
    case 'group': {
      const b = el.bbox
      if (b && b.length >= 4) return [b[0] - e, b[1] - e, b[2] + e, b[3] + e]
      return [0, 0, pageW, pageH]
    }
    default:
      return [0, 0, pageW, pageH]
  }
}

function bboxOfPoints(
  pts: [number, number][],
  pageW: number,
  pageH: number,
): [number, number, number, number] {
  if (!pts.length) return [0, 0, pageW, pageH]
  let x0 = Infinity
  let y0 = Infinity
  let x1 = -Infinity
  let y1 = -Infinity
  for (const [x, y] of pts) {
    x0 = Math.min(x0, x)
    y0 = Math.min(y0, y)
    x1 = Math.max(x1, x)
    y1 = Math.max(y1, y)
  }
  return [x0 - 1, y0 - 1, x1 + 1, y1 + 1]
}

/** 元素内容摘要（路径卡片内容行） */
export function pdfElementContent(el: PdfElement): string | undefined {
  if (el.type === 'text') return el.text?.slice(0, 160) || undefined
  if (el.type === 'image') return el.src || undefined
  if (el.type === 'polyline') return `${el.points?.length ?? 0} 个顶点`
  if (el.type === 'path') return `${el.segments?.length ?? 0} 段`
  if (el.type === 'shading') return `${el.kind ?? 'axial'} · ${el.stops?.length ?? 0} 色标`
  if (el.type === 'rect') return el.fill ?? el.stroke ?? undefined
  return undefined
}

/** 标准十四字体 / system: → CSS font-family */
export function pdfCssFont(font?: string): string {
  const f = font || 'Helvetica'
  if (f.startsWith('system:')) return `'${f.slice(7)}', sans-serif`
  if (f.startsWith('Times')) return "'Times New Roman', Times, serif"
  if (f.startsWith('Courier')) return "'Courier New', Courier, monospace"
  if (f === 'Symbol' || f === 'ZapfDingbats') return 'serif'
  if (f.startsWith('Helvetica')) return 'Helvetica, Arial, sans-serif'
  return `'${f}', sans-serif`
}

/// 页面可见区（打印态）：crop 优先，其次 mediabox，缺省整页（与 json2pdf render 一致）
export function pdfViewport(
  page: PdfPage,
  pageSize: PdfPageSize,
): { vx0: number; vy0: number; vw: number; vh: number } {
  if (page.crop) return { vx0: page.crop[0], vy0: page.crop[1], vw: page.crop[2] - page.crop[0], vh: page.crop[3] - page.crop[1] }
  if (page.mediabox)
    return { vx0: page.mediabox[0], vy0: page.mediabox[1], vw: page.mediabox[2] - page.mediabox[0], vh: page.mediabox[3] - page.mediabox[1] }
  return { vx0: 0, vy0: 0, vw: pageSize.width, vh: pageSize.height }
}

/** 渲染项：配对后的元素（Tr≥4 裁剪文本与后随实心矩形合并为「透过字形填充」） */
export type PdfRenderItem =
  | { kind: 'pair'; text: PdfElement; fill: string; idx: number }
  | { kind: 'hidden'; text: PdfElement; idx: number }
  | { kind: 'el'; el: PdfElement; idx: number }

/** Tr≥4 裁剪文本的呈现预处理（与 CLI render 的 elements_html 配对逻辑同构） */
export function prepareElements(els: PdfElement[]): PdfRenderItem[] {
  const out: PdfRenderItem[] = []
  for (let i = 0; i < els.length; i += 1) {
    const el = els[i]
    if (el.type === 'text' && (el.render_mode ?? 0) >= 4) {
      const next = els[i + 1]
      const covers =
        next?.type === 'rect' &&
        typeof next.fill === 'string' &&
        (next.x ?? 0) <= (el.x ?? 0) &&
        (next.y ?? 0) <= (el.y ?? 0) &&
        (next.x ?? 0) + (next.w ?? 0) >= (el.x ?? 0) &&
        (next.y ?? 0) + (next.h ?? 0) >= (el.y ?? 0)
      if (covers) {
        out.push({ kind: 'pair', text: el, fill: (next as { fill: string }).fill, idx: i })
        i += 1
      } else if ((el.render_mode ?? 0) === 5) {
        out.push({ kind: 'el', el, idx: i })
      } else {
        // Tr 7 纯裁剪 / Tr 3 无填充无描边 → 不可见
        out.push({ kind: 'hidden', text: el, idx: i })
      }
      continue
    }
    out.push({ kind: 'el', el, idx: i })
  }
  return out
}
