//! Excel 数字格式渲染（与 ai-excel/cli/src/render.rs 的引擎同构，逐条对齐）
//!
//! 支持：`;` 分段（正/负/零/文本）、`0`/`#`/`?` 占位、千分位、`%`、`\(`/`_x`/`*x`/`"…"` 转义、
//! 会计括号、`[Red]` 等颜色修饰符、`[$€-x]` 货币符号。

export interface FormattedNumber {
  text: string
  /** 格式自带颜色（如 [Red]） */
  color?: string
}

/** 按 `;` 切分格式段（忽略引号与反斜杠转义内的分隔符） */
export function splitSections(nf: string): string[] {
  const out: string[] = []
  let cur = ''
  for (let i = 0; i < nf.length; i += 1) {
    const c = nf[i]
    if (c === '\\') {
      cur += c
      if (i + 1 < nf.length) cur += nf[i + 1]
      i += 1
    } else if (c === '"') {
      cur += c
      i += 1
      while (i < nf.length) {
        cur += nf[i]
        if (nf[i] === '"') break
        i += 1
      }
    } else if (c === ';') {
      out.push(cur)
      cur = ''
    } else {
      cur += c
    }
  }
  out.push(cur)
  return out
}

const NAMED_COLORS: Record<string, string> = {
  red: 'FF0000',
  blue: '0000FF',
  green: '008000',
  black: '000000',
  white: 'FFFFFF',
  yellow: 'FFFF00',
}

/** 去掉 [Red]/[Color N]/[$€-x] 等修饰符，返回清理后的模板与颜色 */
export function stripModifiers(section: string): { template: string; color?: string } {
  let out = ''
  let color: string | undefined
  for (let i = 0; i < section.length; i += 1) {
    if (section[i] === '[') {
      let inner = ''
      i += 1
      while (i < section.length && section[i] !== ']') {
        inner += section[i]
        i += 1
      }
      const low = inner.toLowerCase()
      if (NAMED_COLORS[low]) color = NAMED_COLORS[low]
      const m = low.match(/^color\s*(\d+)$/)
      if (m) {
        const pal = ['000000', 'FFFFFF', 'FF0000', '00FF00', '0000FF', 'FFFF00', 'FF00FF', '00FFFF']
        color = pal[Number(m[1])] ?? '000000'
      }
      if (inner.startsWith('$')) {
        out += inner.slice(1).split('-')[0]
      }
      continue
    }
    out += section[i]
  }
  return { template: out, color }
}

/** 千分位分组（保留符号与小数） */
export function groupInt(s: string): string {
  const sign = s.startsWith('-') ? '-' : ''
  const body = sign ? s.slice(1) : s
  const [int, frac] = body.includes('.') ? body.split('.') : [body, undefined]
  let out = ''
  for (let i = 0; i < int.length; i += 1) {
    if (i > 0 && (int.length - i) % 3 === 0) out += ','
    out += int[i]
  }
  return frac !== undefined ? `${sign}${out}.${frac}` : `${sign}${out}`
}

/** 依据模式串渲染数字（v 已取绝对值） */
export function numberFromPattern(v: number, pattern: string): string {
  const pct = pattern.includes('%')
  const x = pct ? v * 100 : v
  const [intPatRaw, fracPatRaw] = pattern.includes('.') ? pattern.split('.') : [pattern, '']
  const intPat = intPatRaw.replace(/%/g, '')
  const group = intPat.includes(',')
  const intPlaces = (intPat.match(/[#0?]/g) ?? []).length
  const intOptional = intPlaces > 0 && (intPat.match(/[#0?]/g) ?? []).every((c) => c === '?')

  const frac = (fracPatRaw ?? '')
    .split('')
    .filter((c) => '0#?%'.includes(c))
    .join('')
    .replace(/%/g, '')
  const fracPlaces = (frac.match(/[0#?]/g) ?? []).length
  const fracFixed = frac.length > 0 && frac.includes('0') && frac.split('').every((c) => c === '0')

  let s = x.toFixed(fracPlaces)
  if (fracPlaces > 0 && !fracFixed) {
    const keep = frac.split('').findIndex((c) => c !== '0')
    const keepCount = keep < 0 ? frac.split('').filter((c) => c === '0').length : keep
    const [i, f] = s.split('.')
    let ff = f ?? ''
    while (ff.length > keepCount && ff.endsWith('0')) ff = ff.slice(0, -1)
    s = ff ? `${i}.${ff}` : i
  }
  if (group) s = groupInt(s)
  if (intOptional && intPlaces > 0) {
    const sign = s.startsWith('-') ? '-' : ''
    const body = sign ? s.slice(1) : s
    const [int, f] = body.includes('.') ? body.split('.') : [body, undefined]
    const padded = Math.abs(x) < Number.EPSILON ? ' '.repeat(intPlaces) : int.padStart(intPlaces, ' ')
    s = f !== undefined ? `${sign}${padded}.${f}` : `${sign}${padded}`
  }
  if (pct) s += '%'
  return s
}

/** 渲染单个格式段（v 已取绝对值） */
export function renderSection(v: number, tpl: string, addSign: boolean): string {
  let out = ''
  let pattern = ''
  let first = true
  const flush = () => {
    if (!pattern) return
    if (first) {
      out += numberFromPattern(v, pattern)
      first = false
    }
    pattern = ''
  }
  for (let i = 0; i < tpl.length; i += 1) {
    const c = tpl[i]
    if (c === '\\') {
      flush()
      if (i + 1 < tpl.length) out += tpl[i + 1]
      i += 1
    } else if (c === '"') {
      flush()
      i += 1
      while (i < tpl.length) {
        if (tpl[i] === '"') break
        out += tpl[i]
        i += 1
      }
    } else if (c === '_') {
      flush()
      i += 1 // 占位宽度取下一个字符，用空格近似
      out += ' '
    } else if (c === '*') {
      flush()
      i += 1 // 重复填充字符，忽略
    } else if ('#0?.,'.includes(c)) {
      pattern += c
    } else if (c === '%') {
      pattern += c
    } else {
      flush()
      out += c
    }
  }
  flush()
  return addSign ? `-${out}` : out
}

/** 入口：数值 + 格式串 → 文本与颜色 */
export function formatNumber(v: number, nf: string): FormattedNumber {
  const sections = splitSections(nf)
  let idx = 0
  if (v > 0) idx = 0
  else if (v < 0) idx = sections.length > 1 ? 1 : 0
  else idx = sections.length > 2 ? 2 : 0
  const raw = sections[idx] ?? ''
  const { template, color } = stripModifiers(raw)
  const addSign = v < 0 && sections.length === 1 && !template.includes('-')
  return { text: renderSection(Math.abs(v), template, addSign), color }
}

/** 单元格显示文本：数字走格式引擎，其余原样 */
export function displayValue(value: unknown, numberFormat?: string | null): FormattedNumber {
  if (typeof value === 'number' && numberFormat && numberFormat !== 'General' && numberFormat !== '@') {
    return formatNumber(value, numberFormat)
  }
  if (value === null || value === undefined) return { text: '' }
  if (typeof value === 'boolean') return { text: String(value) }
  return { text: String(value) }
}
