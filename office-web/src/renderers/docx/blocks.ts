//! docx 块/运行样式解析：命名样式（styles + based_on 链）与块内联属性 → CSS 声明
import type { DocxBlock, DocxDocument, DocxRun, DocxStyle } from '@/types'
import { ptToPx, cssColor } from '@/core/units'

/** 字体栈：Windows 中文字体在 macOS 上多半缺失，按字形风格给等价替换。
 *  黑体系首选 Hiragino Sans GB——它正是 macOS 上 Word/WPS/LibreOffice 对微软雅黑的替换字体，
 *  替换一致，预览的字形与行距才和真实文档对得上；宋体系落到 Songti。
 *
 *  西文先补一层：中文字体缺失时浏览器会把西文/数字也交给替换的中文字体，
 *  而 Hiragino 的数字 ≈0.66em、比微软雅黑 0.52em 宽近三成，整行偏宽、断行提前，
 *  页数就和 Word 对不上（实测说明文档 18 页 vs 17 页）。Word 里西文本就走 rFonts w:ascii，
 *  字体缺失时同样先找西文替身，所以西文替换要排在中文字体替身之前。 */
const LATIN_SUB = "'Helvetica Neue', Arial"

export function fontStack(family?: string | null): string {
  const f = (family ?? '').trim()
  const key = f.toLowerCase().replace(/[\s'"]/g, '')
  const black = /yahei|雅黑|heiti|黑体|simhei|pingfang|dengxian|等线|sourcehansans|notosanssc/.test(key)
  const song = /simsun|宋体|songti|stsong|fangsong|仿宋|kaiti|楷体|ming|mincho/.test(key)
  const fallback = black
    ? `${LATIN_SUB}, 'Hiragino Sans GB', 'PingFang SC', 'Heiti SC', 'Microsoft YaHei', 'Noto Sans CJK SC', sans-serif`
    : song
      ? "'Songti SC', 'STSong', SimSun, 'Noto Serif CJK SC', serif"
      : `${LATIN_SUB}, 'Hiragino Sans GB', 'PingFang SC', 'Songti SC', 'Microsoft YaHei', SimSun, serif`
  return f ? `'${f}', ${fallback}` : fallback
}

/**
 * 单倍行距比（em）。Word 的单倍行距 = 字体 ascent + descent + lineGap，这正是浏览器
 * `line-height: normal` 的口径——所以**优先实测**：把同一字体栈的探针放进当前文档量一次，
 * 量到谁就是谁（预览与 Word/WPS 用同一套替换字体，行距才对得上）。
 * 平台表只在无 document（SSR/单测）或探针失败时兜底：macOS 上黑体替换为 Hiragino Sans GB（1.5）、
 * 宋体为 Songti（1.4）、西文 Arial（1.15）；Windows 上微软雅黑 1.32、宋体 1.4。
 *
 * 例外是「东亚语言标成西文」的段落（w:lang w:eastAsia="en-US"，简历模板常见）：
 * Word/LibreOffice 实测按**西文行距**排（同一字体：中文 1.77 em → 西文 1.14 em），
 * 所以这类段落统一取西文字体的度量。
 */
// 黑体类单倍行高比：1.765 = 参考 PDF（LibreOffice 同一替换字体）93 个行距样本的中位 27.8pt
// ÷（1.5 倍行距 × 10.5pt）。这个值是全篇联动的：早先用 1.75 是为了抵消「单元格纵向内边距偏大
// 1.6px/行」的反向偏差；把内边距按实测校准成 1px 之后，1.765 既能对上行距，章节起始页也仍然
// 与文档自带目录逐章吻合（3/4/6/8/9/10/12/15/16/17）。改这里请同时复测说明文档与简历两份标尺。
const MAC_LINE_RATIO: Record<string, number> = { black: 1.765, song: 1.4, latin: 1.15 }
const WIN_LINE_RATIO: Record<string, number> = { black: 1.32, song: 1.4, latin: 1.15 }
/** 东亚语言为西文时的行距比（实测 LO/Word：Arial 等西文字体 ≈1.14–1.16 em） */
const WESTERN_LINE_RATIO = 1.15

/** 东亚语言标签是否西文（zh/ja/ko 之外都按西文行距排） */
export function isWesternLang(langEa?: string | null): boolean {
  const v = (langEa ?? '').trim().toLowerCase()
  if (!v) return false
  return !/^(zh|ja|ko|cmn|yue)\b|-han/i.test(v)
}

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

/** 字体栈 → 单倍行高比（em）：探针量一行「中文+西文」的行盒高（浏览器按行内最大字体算，与 Word 同） */
const ratioCache = new Map<string, number>()

function probeLineRatio(stack: string): number | null {
  if (typeof document === 'undefined' || !document.body) return null
  try {
    const host = document.createElement('div')
    host.style.cssText = 'position:fixed;left:-9999px;top:0;visibility:hidden;pointer-events:none'
    const probe = document.createElement('div')
    probe.style.cssText = `font-family:${stack};font-size:100px;line-height:normal;white-space:pre;margin:0;padding:0;border:0`
    probe.textContent = '汉Hg'
    host.appendChild(probe)
    document.body.appendChild(host)
    const h = probe.offsetHeight
    host.remove()
    return h > 1 ? h / 100 : null
  } catch {
    return null
  }
}

export function normalLineRatio(fontFamily?: string | null, western = false): number {
  const kind = familyKind(fontFamily)
  // 中文字族按平台实测值（macOS/LibreOffice·WPS 实测：黑体类 1.75–1.77、宋体 1.4；Windows 微软雅黑 1.32）
  if (kind && !western) return (isMacHost() ? MAC_LINE_RATIO : WIN_LINE_RATIO)[kind]
  // 西文行距（w:lang 标成西文，或本来就是西文字体）：Arial 等 ≈1.15 em
  if (western) return WESTERN_LINE_RATIO
  const stack = fontStack(fontFamily)
  const cached = ratioCache.get(stack)
  if (cached !== undefined) return cached
  const ratio = probeLineRatio(stack) ?? 1.2
  ratioCache.set(stack, ratio)
  return ratio
}

/** 字体就位/更换后自然行高会变（webfont、字体兜底）：清缓存后重新分页 */
export function resetLineRatioCache(): void {
  ratioCache.clear()
}

/** 行高（严格按 Word 语义）：多倍行距 = N × 字体单倍行高；固定行距 exact 钉死磅值
 *  （哪怕叠字——Word 就这么排）、atLeast 才允许按字体自然行高长高 */
function lineHeightOf(
  fontFamily: string | undefined,
  size: number | undefined,
  lineSpacing: number | undefined,
  linePt: number | undefined,
  lineRule: string | undefined,
  western = false,
): string | undefined {
  if (typeof linePt === 'number' && linePt > 0) {
    if (lineRule === 'atLeast' && size) {
      const natural = normalLineRatio(fontFamily, western) * ptToPx(size)
      return `${Math.round(Math.max(natural, ptToPx(linePt)))}px`
    }
    return `${Math.round(ptToPx(linePt))}px`
  }
  if (typeof lineSpacing === 'number' && lineSpacing > 0) {
    return String(lineSpacing * normalLineRatio(fontFamily, western))
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

/** 块级样式（段落/标题共用）。字体/语言按 OOXML 继承链回退：块 → 命名样式 → docDefaults */
export function blockStyle(
  doc: DocxDocument | undefined,
  b: DocxBlock,
  namedOverride?: DocxStyle,
): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  const named =
    namedOverride ??
    styleOf(doc, typeof anyB.style === 'string' ? (anyB.style as string) : undefined)
  const dflt = doc?.defaults
  const out: Record<string, string> = {}
  const fontFamily =
    ((anyB.font_family as string) ??
      named?.font_family ??
      named?.east_asia_font ??
      named?.font ??
      dflt?.font_family ??
      dflt?.east_asia_font) ||
    undefined
  if (fontFamily) out['font-family'] = fontStack(fontFamily)
  // 字号同样要兜到 docDefaults：说明文档这类模板把 10.5pt 挂在文档默认上，
  // 块本身不带字号。断行量宽按这个字号算（读不到就落到 16px 兜底，整篇宽出一成多，
  // 断行会提前、多出整行），所以这里必须把继承链走完。
  const size = (anyB.font_size as number) ?? named?.font_size ?? named?.size ?? dflt?.font_size
  if (size) out['font-size'] = `${ptToPx(size)}px`
  const bold = (anyB.bold as boolean) ?? named?.bold
  if (bold) out['font-weight'] = 'bold'
  const italic = (anyB.italic as boolean) ?? named?.italic
  if (italic) out['font-style'] = 'italic'
  const color = (anyB.color as string) ?? named?.color
  if (color) out['color'] = cssColor(color)
  const align = (anyB.align as string) ?? named?.align
  if (align) out['text-align'] = align === 'both' ? 'justify' : align
  // 东亚语言标成西文（en-US 等）时按西文行距排版（与 Word/LO 一致）
  const westernLang = isWesternLang(
    (anyB.lang_ea as string) ?? named?.lang_ea ?? dflt?.lang_ea,
  )
  // 行距兜底要走 Normal：Word 里无样式的段落就是 Normal 样式，1.5 倍行距这类设置常挂在它上面。
  // 表格单元格正是这种情况——解析产物里只有字号（单元格自身的 w:spacing 与表格样式都没带出来），
  // 只落到 docDefaults 的话单倍行距会把整张表压矮三成，和真实文档对不上。
  const normal = styleOf(doc, 'Normal')
  const lh = lineHeightOf(
    fontFamily,
    size,
    ((anyB.line_spacing as number) ?? named?.line_spacing ?? normal?.line_spacing) || undefined,
    (anyB.line_pt as number) ?? named?.line_pt ?? normal?.line_pt ?? undefined,
    ((anyB.line_rule as string) ?? named?.line_rule ?? normal?.line_rule) || undefined,
    westernLang,
  )
  if (lh) out['line-height'] = lh
  // 段距：显式 0 也要落（否则会吃到 CSS 默认底距，比真实文档松）；同样按继承链回退到 docDefaults
  const sb = (anyB.space_before as number) ?? named?.space_before ?? dflt?.space_before
  if (typeof sb === 'number') out['margin-top'] = `${ptToPx(sb)}px`
  const sa = (anyB.space_after as number) ?? named?.space_after ?? dflt?.space_after
  if (typeof sa === 'number') out['margin-bottom'] = `${ptToPx(sa)}px`
  const fi = (anyB.first_line_indent as number) ?? named?.first_line_indent
  if (typeof fi === 'number') out['text-indent'] = `${ptToPx(fi)}px`
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

/** 断行量宽用：块（+可选 run）的字体 CSS 简写与字号 px。口径必须和 runStyle/blockStyle
 *  落到 DOM 上的完全一致，否则「量出来的行宽」和「排出来的行宽」对不上。 */
export function fontMetricsOf(
  base: Record<string, string>,
  run?: DocxRun,
): { font: string; size: number } {
  const family =
    (run?.code
      ? 'ui-monospace, Menlo, monospace'
      : run?.font_family
        ? fontStack(run.font_family)
        : base['font-family']) || 'sans-serif'
  const size = run?.font_size ? ptToPx(run.font_size) : parseFloat(base['font-size'] ?? '') || 16
  const weight = run?.bold ? 'bold' : (base['font-weight'] ?? 'normal')
  const style = run?.italic ? 'italic' : (base['font-style'] ?? 'normal')
  return { font: `${style} ${weight} ${size}px ${family}`, size }
}

export const HEADING_SCALE: Record<number, number> = { 1: 22, 2: 18, 3: 15.5, 4: 14, 5: 13, 6: 12.5 }

export function headingStyle(doc: DocxDocument | undefined, b: DocxBlock): Record<string, string> {
  const anyB = b as unknown as Record<string, unknown>
  const level = Number(anyB.level ?? 1)
  const named = styleOf(doc, (anyB.style as string) ?? `Heading${level}`) ?? styleOf(doc, 'Heading')
  const out = blockStyle(doc, b, named)
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
