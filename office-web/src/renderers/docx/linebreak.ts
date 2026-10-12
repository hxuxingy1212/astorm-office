//! Word 兼容断行（纯逻辑，量宽由调用方注入）：贪心排字 + 中文禁则 + 行末标点悬挂。
//!
//! 浏览器断行缺两条 Word 规则：
//!  ① **标点悬挂**（w:overflowPunct，默认开）：行末放不下的句读可以探出版心一个字，
//!     于是这一行能多排一个字。参考 PDF 实测：307 个正文行里有 16 行行尾越出版心
//!     约 10.7pt（正好一个全角字），越界的都是「，。、」这类句读。
//!  ② **行首/行尾禁则**：句读不能落在行首、开括号不能落在行末。
//! 缺这两条会让「…、」「…，」结尾的行各少排一个字，长文逐段累积就多出整页
//! （实测说明文档 18 页 vs 参考 17 页，差的几段全部是「行末句读被挤到下一行」）。

/** 行首禁则字符：不能出现在行首（放不下时悬挂到版心外，或连前一个字一起下移） */
export const NO_START = '，。、；：？！）〕〉》」』】〗｝”’·…‥%℃'
/** 行尾禁则字符：不能出现在行末（放不下时整个挪到下一行） */
export const NO_END = '（〔〈《「『【〖｛“‘'

/** 全角（CJK 系）字符：自成断行单元，不与前后西文粘连 */
const WIDE =
  /[\u1100-\u115f\u2e80-\u303f\u3040-\u33ff\u3400-\u4dbf\u4e00-\u9fff\ua960-\ua97f\uac00-\ud7ff\uf900-\ufaff\ufe10-\ufe19\ufe30-\ufe6f\uff00-\uff60\uffe0-\uffe6]/

export function isWideChar(ch: string): boolean {
  return WIDE.test(ch)
}

/** 带样式的字符：font 是 canvas 量宽用的 CSS font 简写，size 是字号（px，全角字按 1em 计），
 *  style 是渲染用的行内样式 */
export interface StyledChar {
  ch: string
  font: string
  size: number
  style?: Record<string, string>
}

export interface BreakUnit {
  text: string
  width: number
  /** 字符下标区间 [ci, cj) */
  ci: number
  cj: number
  noStart?: boolean
  noEnd?: boolean
  space?: boolean
}

/** 断行结果：本行覆盖的字符区间 [from, to)，hang=true 时末字符悬挂出版心 */
export interface BreakLine {
  from: number
  to: number
  hang: boolean
}

export type MeasureText = (text: string, font: string) => number

/** 把字符流切成断行单元：全角字各成一个单元；西文/数字连续串并成一个（不拆词）；
 *  空白单独成单元（Word 断行会吞掉行尾空格，得能单独丢）
 *
 *  全角字宽取 1em（font-size）而不是 canvas 量宽：DOM 实测「字，字」= 3em 精确成立，
 *  而 canvas 对中文标点的推进与 DOM 不一致（连续标点会被 Chrome 做标点挤压，两者挤的
 *  幅度还不一样）。代价是**相邻标点**那几处会高估 ≤0.25em/对，属少数行。 */
export function buildUnits(chars: readonly StyledChar[], measure: MeasureText): BreakUnit[] {
  const out: BreakUnit[] = []
  let buf = ''
  let bufFont = ''
  let bufFrom = 0
  const flush = (endIdx: number) => {
    if (!buf) return
    out.push({ text: buf, width: measure(buf, bufFont), ci: bufFrom, cj: endIdx })
    buf = ''
  }
  chars.forEach((c, i) => {
    const ch = c.ch
    if (ch === ' ' || ch === '\t' || ch === '\u00a0') {
      flush(i)
      out.push({ text: ch, width: measure(ch, c.font), ci: i, cj: i + 1, space: true })
      return
    }
    if (isWideChar(ch)) {
      flush(i)
      out.push({
        text: ch,
        width: c.size,
        ci: i,
        cj: i + 1,
        noStart: NO_START.includes(ch),
        noEnd: NO_END.includes(ch),
      })
      return
    }
    if (!buf) bufFrom = i
    if (buf && c.font !== bufFont) flush(i)
    bufFont = c.font
    buf += ch
  })
  flush(chars.length)
  return out
}

/**
 * 贪心断行。units 是 buildUnits 的产物；avail 是版心宽（px）；hangMax 是允许悬挂的宽度上限
 * （一个字宽——标点比字宽时不敢让它探出去，退回禁则处理）；firstAvail 是首行可用宽
 * （首行缩进时更窄，缺省同 avail）。
 */
export function breakUnits(
  units: readonly BreakUnit[],
  avail: number,
  hangMax: number,
  firstAvail?: number,
): BreakLine[] {
  const lines: BreakLine[] = []
  const n = units.length
  const EPS = 0.02
  let i = 0
  while (i < n) {
    const availNow = lines.length === 0 && firstAvail !== undefined ? firstAvail : avail
    // 软断行后行首的空白丢弃（Word 断行吞掉断点处的空格），段首的首行保留
    if (i > 0) {
      while (i < n && units[i].space) i += 1
      if (i >= n) break
    }
    let end = i
    let w = 0
    while (end < n && w + units[end].width <= availNow + EPS) {
      w += units[end].width
      end += 1
    }
    // 行尾空白先丢：它不参与断行，也让后面的句读能挤进本行
    while (end > i && units[end - 1].space) {
      end -= 1
      w -= units[end].width
    }
    let hang = false
    if (end < n && end > i) {
      const u = units[end]
      if (u.noStart && u.width <= hangMax + EPS && end > i) {
        // 标点悬挂：句读留在本行、探出版心（Word 的 w:overflowPunct）
        end += 1
        hang = true
      } else if (u.noStart && end - i >= 2) {
        // 悬挂不了（比字还宽）：连前一个字一起挪到下一行
        end -= 1
      } else if (units[end - 1].noEnd) {
        // 行尾禁则：开括号整体挪到下一行
        end -= 1
      }
    }
    if (end === i) end = i + 1 // 单个单元比版心还宽（超长西文串）：独占一行，保证推进
    lines.push({ from: units[i].ci, to: units[end - 1].cj, hang })
    i = end
  }
  return lines
}

/** 段内切片：按 \n 分段，每段独立断行；返回的行区间是**整段字符下标**（含被跳过的 \n 偏移） */
export function layoutParagraph(
  chars: readonly StyledChar[],
  opts: {
    /** 首行可用宽（版心宽 − 首行缩进等） */
    firstAvail: number
    /** 其余行可用宽 */
    avail: number
    /** 悬挂宽度上限（一个字宽） */
    hangMax: number
    measure: MeasureText
  },
): BreakLine[] {
  const out: BreakLine[] = []
  const segs: [number, number][] = []
  let start = 0
  for (let i = 0; i < chars.length; i += 1) {
    if (chars[i].ch === '\n') {
      segs.push([start, i])
      start = i + 1
    }
  }
  segs.push([start, chars.length])
  segs.forEach(([s0, s1]) => {
    if (s1 <= s0) return
    const units = buildUnits(chars.slice(s0, s1), opts.measure)
    const lines = breakUnits(
      units,
      opts.avail,
      opts.hangMax,
      out.length === 0 ? opts.firstAvail : undefined,
    )
    lines.forEach((l) => out.push({ from: l.from + s0, to: l.to + s0, hang: l.hang }))
  })
  return out
}

/** 按行区间把字符切成「同样式连续段」，供模板渲染 */
export function spansOf(
  chars: readonly StyledChar[],
  from: number,
  to: number,
): { text: string; style?: Record<string, string> }[] {
  const out: { text: string; style?: Record<string, string> }[] = []
  for (let i = from; i < to; i += 1) {
    const c = chars[i]
    if (!c) continue
    const last = out[out.length - 1]
    if (last && sameStyle(last.style, c.style)) last.text += c.ch
    else out.push({ text: c.ch, style: c.style })
  }
  return out
}

function sameStyle(a?: Record<string, string>, b?: Record<string, string>): boolean {
  if (a === b) return true
  if (!a || !b) return false
  const ka = Object.keys(a)
  if (ka.length !== Object.keys(b).length) return false
  return ka.every((k) => a[k] === b[k])
}
