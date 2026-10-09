//! docx 块/运行样式解析：命名样式（styles + based_on 链）与块内联属性 → CSS 声明
import type { DocxBlock, DocxDocument, DocxRun, DocxStyle } from '@/types'
import { ptToPx, cssColor } from '@/core/units'

/** 字体栈：Windows 中文字体在 macOS 上多半缺失，按字形风格给等价替换。
 *  黑体系首选 Hiragino Sans GB——它正是 macOS 上 Word/WPS/LibreOffice 对微软雅黑的替换字体，
 *  替换一致，预览的字形与行距才和真实文档对得上；宋体系落到 Songti。 */
export function fontStack(family?: string | null): string {
  const f = (family ?? '').trim()
  const key = f.toLowerCase().replace(/[\s'"]/g, '')
  const black = /yahei|雅黑|heiti|黑体|simhei|pingfang|dengxian|等线|sourcehansans|notosanssc/.test(key)
  const song = /simsun|宋体|songti|stsong|fangsong|仿宋|kaiti|楷体|ming|mincho/.test(key)
  const fallback = black
    ? "'Hiragino Sans GB', 'PingFang SC', 'Heiti SC', 'Microsoft YaHei', 'Noto Sans CJK SC', sans-serif"
    : song
      ? "'Songti SC', 'STSong', SimSun, 'Noto Serif CJK SC', serif"
      : "'Hiragino Sans GB', 'PingFang SC', 'Songti SC', 'Microsoft YaHei', SimSun, serif"
  return f ? `'${f}', ${fallback}` : fallback
}

/**
 * 单倍行距比（em）。Word/WPS/LibreOffice 的单倍行距 = 字体 usWinAscent + usWinDescent + lineGap，
 * 与浏览器 line-height:normal（hhea asc+desc+gap）不是同一个数——差这一截，页面的行密度会明显比真实文档密。
 *
 * 关键是「字体实际落到谁」：
 *   · macOS 缺 Windows 中文字体，Word/WPS/LibreOffice 把黑体系替换成 Hiragino Sans GB（1.74）、
 *     宋体系替换成 Songti（1.4）——按同样的替换取度量，预览页数才与文档自带目录一致；
 *   · Windows 上就是原字体（微软雅黑 1.32、宋体 1.4）。
 * 表外字体（西文/未知）退回浏览器实测 line-height:normal。
 */
const MAC_LINE_RATIO: Record<string, number> = { black: 1.74, song: 1.4, latin: 1.2 }
const WIN_LINE_RATIO: Record<string, number> = { black: 1.32, song: 1.4, latin: 1.2 }

function familyKind(family?: string | null): 'black' | 'song' | 'latin' | undefined {
  const key = (family ?? '').toLowerCase().replace(/[\s'"]/g, '')
  if (!key) return undefined
  if (/yahei|雅黑|heiti|黑体|simhei|pingfang|dengxian|等线|hiragino|sourcehansans|notosanssc/.test(key)) {
    return 'black'
  }
  if (/simsun|宋体|songti|stsong|ming|fangsong|仿宋|kaiti|楷体|mincho/.test(key)) return 'song'
  if (/times|arial|helvetica|calibri|georgia|cambria|latin|mono|consol|courier/.test(key)) return 'latin'
  return undefined
}

function isMacHost(): boolean {
  if (typeof navigator === 'undefined') return true
  return /mac/i.test(navigator.userAgent)
}

const ratioCache = new Map<string, number>()

export function normalLineRatio(fontFamily?: string | null): number {
  const kind = familyKind(fontFamily)
  if (kind) return (isMacHost() ? MAC_LINE_RATIO : WIN_LINE_RATIO)[kind]
  const key = fontFamily ?? ''
  const cached = ratioCache.get(key)
  if (cached !== undefined) return cached
  let ratio = 1.2
  if (typeof document !== 'undefined' && document.body) {
    const probe = document.createElement('span')
    probe.textContent = '汉Hg'
    probe.style.position = 'absolute'
    probe.style.left = '-9999px'
    probe.style.visibility = 'hidden'
    probe.style.whiteSpace = 'nowrap'
    probe.style.lineHeight = 'normal'
    probe.style.fontSize = '100px'
    probe.style.fontFamily = fontStack(key)
    document.body.appendChild(probe)
    const h = probe.getBoundingClientRect().height
    probe.remove()
    if (h > 1) ratio = h / 100
  }
  ratioCache.set(key, ratio)
  return ratio
}

/** 字体就位/更换后自然行高会变（webfont、字体兜底）：清缓存后重新分页 */
export function resetLineRatioCache(): void {
  ratioCache.clear()
}

/** 行高（行距倍数按字体自然行高换算；固定行距按磅值） */
function lineHeightOf(
  fontFamily: string | undefined,
  size: number | undefined,
  lineSpacing: number | undefined,
  linePt: number | undefined,
): string | undefined {
  if (typeof linePt === 'number' && linePt > 0) {
    const natural = size ? normalLineRatio(fontFamily) * ptToPx(size) : 0
    // exact/atLeast：至少不低于字体自然行高，避免行挤在一起
    return `${Math.round(Math.max(ptToPx(linePt), natural))}px`
  }
  if (typeof lineSpacing === 'number' && lineSpacing > 0) {
    return String(lineSpacing * normalLineRatio(fontFamily))
  }
  return undefined
}

/** 命名样式：递归解析 based_on */
export function styleOf(
  doc: DocxDocument | undefined,
  name?: string | null,
  depth = 0,
): DocxStyle | undefined {
  if (!name || depth > 4) return undefined
  const s = doc?.styles?.[name]
  if (!s) return undefined
  const base = styleOf(doc, s.based_on ?? undefined, depth + 1)
  return base ? { ...base, ...stripUndefined(s) } : s
}

function stripUndefined<T extends object>(o: T): T {
  const out: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(o)) if (v !== undefined && v !== null) out[k] = v
  return out as T
}

/** 块级样式（段落/标题共用） */
export function blockStyle(doc: DocxDocument | undefined, b: DocxBlock): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  const named = styleOf(doc, typeof anyB.style === 'string' ? (anyB.style as string) : undefined)
  const out: Record<string, string> = {}
  const fontFamily = ((anyB.font_family as string) ?? named?.font) || undefined
  if (fontFamily) out['font-family'] = fontStack(fontFamily)
  const size = (anyB.font_size as number) ?? named?.size
  if (size) out['font-size'] = `${ptToPx(size)}px`
  const bold = (anyB.bold as boolean) ?? named?.bold
  if (bold) out['font-weight'] = 'bold'
  const italic = (anyB.italic as boolean) ?? named?.italic
  if (italic) out['font-style'] = 'italic'
  const color = (anyB.color as string) ?? named?.color
  if (color) out['color'] = cssColor(color)
  const align = (anyB.align as string) ?? named?.align
  if (align) out['text-align'] = align === 'both' ? 'justify' : align
  const lh = lineHeightOf(
    fontFamily,
    size,
    ((anyB.line_spacing as number) ?? named?.line_spacing) || undefined,
    (anyB.line_pt as number) ?? named?.line_pt ?? undefined,
  )
  if (lh) out['line-height'] = lh
  const sb = (anyB.space_before as number) ?? named?.space_before
  if (sb) out['margin-top'] = `${ptToPx(sb)}px`
  const sa = (anyB.space_after as number) ?? named?.space_after
  if (sa) out['margin-bottom'] = `${ptToPx(sa)}px`
  const fi = (anyB.first_line_indent as number) ?? named?.first_line_indent
  if (fi) out['text-indent'] = `${ptToPx(fi)}px`
  const indent = (anyB.indent as number) ?? named?.indent
  if (indent) out['margin-left'] = `${ptToPx(indent)}px`
  const hanging = anyB.hanging_indent as number
  if (hanging) out['padding-left'] = `${ptToPx(hanging)}px`
  const shading = anyB.shading as string
  if (shading) out['background-color'] = cssColor(shading)
  if (anyB.page_break_before) out['break-before'] = 'page'
  return out
}

/** 图片块对齐样式 */
export function figureStyle(b: DocxBlock): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  return { textAlign: (anyB.align as string) ?? 'center' }
}

/** 形状/文本框样式 */
export function shapeStyle(doc: DocxDocument | undefined, b: DocxBlock): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  const out: Record<string, string> = { ...blockStyle(doc, b) }
  if (anyB.fill) out['background'] = cssColor(anyB.fill as string)
  if (anyB.line) out['border'] = `1px solid ${cssColor(anyB.line as string)}`
  return out
}

export function runStyle(r: DocxRun): Record<string, string> {
  const out: Record<string, string> = {}
  if (r.bold) out['font-weight'] = 'bold'
  if (r.italic) out['font-style'] = 'italic'
  const deco: string[] = []
  if (r.underline) deco.push('underline')
  if (r.strike) deco.push('line-through')
  if (deco.length) out['text-decoration'] = deco.join(' ')
  if (r.color) out['color'] = cssColor(r.color)
  if (r.font_size) out['font-size'] = `${ptToPx(r.font_size)}px`
  if (r.font_family) out['font-family'] = fontStack(r.font_family)
  if (r.highlight) out['background-color'] = cssColor(r.highlight)
  if (r.code) out['font-family'] = 'ui-monospace, Menlo, monospace'
  if (r.vert_align === 'superscript') out['vertical-align'] = 'super'
  if (r.vert_align === 'subscript') out['vertical-align'] = 'sub'
  return out
}

export const HEADING_SCALE: Record<number, number> = { 1: 22, 2: 18, 3: 15.5, 4: 14, 5: 13, 6: 12.5 }

export function headingStyle(doc: DocxDocument | undefined, b: DocxBlock): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  const level = Number(anyB.level ?? 1)
  const named = styleOf(doc, `Heading${level}`) ?? styleOf(doc, 'Heading')
  const out = blockStyle(doc, b)
  if (!out['font-size']) out['font-size'] = `${ptToPx(HEADING_SCALE[level] ?? 14)}px`
  if (!out['font-weight']) out['font-weight'] = 'bold'
  if (!out['margin-top']) out['margin-top'] = level === 1 ? '18px' : '12px'
  if (!out['margin-bottom']) out['margin-bottom'] = '8px'
  if (!out['color'] && named?.color) out['color'] = cssColor(named.color)
  return out
}

/** 同类型序号（容器内）：path 与展示编号共用 */
export function ordinals(blocks: readonly DocxBlock[]): number[] {
  const seen: Record<string, number> = {}
  return blocks.map((b) => {
    seen[b.type] = (seen[b.type] ?? 0) + 1
    return seen[b.type]
  })
}
