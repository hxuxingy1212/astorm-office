//! 迷你 LaTeX → HTML：覆盖学术文档常见构造（上下标/分式/根式/求和积分/希腊字母/常用符号）
//! 说明：非完整 TeX 引擎；未知命令原样保留（带 title 提示），保证可读且不破坏排版。

const GREEK: Record<string, string> = {
  alpha: 'α', beta: 'β', gamma: 'γ', delta: 'δ', epsilon: 'ε', varepsilon: 'ε', zeta: 'ζ',
  eta: 'η', theta: 'θ', vartheta: 'ϑ', iota: 'ι', kappa: 'κ', lambda: 'λ', mu: 'μ', nu: 'ν',
  xi: 'ξ', pi: 'π', rho: 'ρ', sigma: 'σ', tau: 'τ', upsilon: 'υ', phi: 'φ', varphi: 'φ',
  chi: 'χ', psi: 'ψ', omega: 'ω', Gamma: 'Γ', Delta: 'Δ', Theta: 'Θ', Lambda: 'Λ', Xi: 'Ξ',
  Pi: 'Π', Sigma: 'Σ', Phi: 'Φ', Psi: 'Ψ', Omega: 'Ω',
}

const SYMBOLS: Record<string, string> = {
  times: '×', div: '÷', pm: '±', mp: '∓', leq: '≤', le: '≤', geq: '≥', ge: '≥', neq: '≠',
  ne: '≠', approx: '≈', equiv: '≡', infty: '∞', partial: '∂', nabla: '∇', cdot: '·', cdots: '⋯',
  ldots: '…', dots: '…', to: '→', rightarrow: '→', leftarrow: '←', Rightarrow: '⇒',
  Leftrightarrow: '⇔', leftrightarrow: '↔', sum: '∑', prod: '∏', int: '∫', oint: '∮',
  in: '∈', notin: '∉', subset: '⊂', subseteq: '⊆', supset: '⊃', cup: '∪', cap: '∩',
  forall: '∀', exists: '∃', angle: '∠', perp: '⊥', parallel: '∥', sim: '∼', propto: '∝',
  circ: '∘', star: '⋆', bullet: '•', therefore: '∴', because: '∵', prime: '′', degree: '°',
}

const FUNCS = [
  'sin', 'cos', 'tan', 'cot', 'sec', 'csc', 'log', 'ln', 'exp', 'lim', 'max', 'min',
  'det', 'dim', 'ker', 'deg', 'gcd', 'arg', 'sup', 'inf', 'mod',
]

const esc = (s: string): string =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

/** 取配对的 {...}（含嵌套）；start 指向 '{' */
function takeGroup(src: string, start: number): { body: string; end: number } | null {
  if (src[start] !== '{') return null
  let depth = 0
  for (let i = start; i < src.length; i += 1) {
    if (src[i] === '{') depth += 1
    else if (src[i] === '}') {
      depth -= 1
      if (depth === 0) return { body: src.slice(start + 1, i), end: i + 1 }
    }
  }
  return null
}

/** 单条命令 → HTML（arg 读取其参数） */
function commandHtml(name: string, arg: () => string): string {
  if (GREEK[name]) return `<span class="lx-var">${GREEK[name]}</span>`
  if (SYMBOLS[name]) return `<span class="lx-sym">${SYMBOLS[name]}</span>`
  if (FUNCS.includes(name)) return `<span class="lx-fn">${name}</span>`
  switch (name) {
    case 'frac':
    case 'dfrac':
    case 'tfrac': {
      const a = arg()
      const b = arg()
      return `<span class="lx-frac"><span class="lx-num">${a}</span><span class="lx-den">${b}</span></span>`
    }
    case 'sqrt': {
      const a = arg()
      return `<span class="lx-sqrt"><span class="lx-radical">√</span><span class="lx-radicand">${a}</span></span>`
    }
    case 'text':
    case 'mathrm':
    case 'operatorname':
    case 'mbox':
      return `<span class="lx-text">${arg()}</span>`
    case 'mathbf':
    case 'boldsymbol':
      return `<strong>${arg()}</strong>`
    case 'mathit':
      return `<em>${arg()}</em>`
    case 'left':
    case 'right':
    case 'displaystyle':
    case 'limits':
    case 'nolimits':
      return ''
    case 'quad':
    case 'qquad':
      return '<span class="lx-space"></span>'
    case 'hspace':
      arg()
      return '<span class="lx-space"></span>'
    case ',':
    case ';':
      return '<span class="lx-thinspace"></span>'
    default:
      return `<span class="lx-unknown" title="\\${name}">${esc(name)}</span>`
  }
}

/** LaTeX → HTML 片段（调用方以 v-html 注入） */
export function latexToHtml(latex: string): string {
  /** 解析任意片段（递归入口） */
  function parse(text: string): string {
    let j = 0
    const out: string[] = []
    const argHere = (): string => {
      while (text[j] === ' ') j += 1
      if (text[j] === '{') {
        const g = takeGroup(text, j)
        if (g) {
          j = g.end
          return parse(g.body)
        }
      }
      const ch = text[j] ?? ''
      j += 1
      return esc(ch)
    }
    while (j < text.length) {
      const c = text[j]
      if (c === '\\') {
        j += 1
        const m = /^[A-Za-z]+/.exec(text.slice(j))
        if (m) {
          j += m[0].length
          out.push(commandHtml(m[0], argHere))
        } else {
          // 转义符号（\, \; \{ \% 等）：原样输出下一个字符
          const ch = text[j] ?? ''
          j += 1
          out.push(ch === '\\' ? '\\' : esc(ch))
        }
        continue
      }
      if (c === '^' || c === '_') {
        j += 1
        const arg = argHere()
        out.push(c === '^' ? `<sup>${arg}</sup>` : `<sub>${arg}</sub>`)
        continue
      }
      if (c === '{') {
        const g = takeGroup(text, j)
        if (g) {
          j = g.end
          out.push(parse(g.body))
          continue
        }
      }
      if (c === '&') {
        j += 1
        out.push('&amp;')
        continue
      }
      if (c === '\n') {
        j += 1
        continue
      }
      j += 1
      out.push(esc(c))
    }
    return out.join('')
  }

  return parse(latex)
}

/** 公式 HTML 的配套样式（DocxViewer 注入一次即可） */
export const LATEX_CSS = `
.lx-frac{display:inline-flex;flex-direction:column;vertical-align:middle;text-align:center;margin:0 .15em;line-height:1.15}
.lx-frac .lx-num{border-bottom:1px solid currentColor;padding:0 .2em}
.lx-frac .lx-den{padding:0 .2em}
.lx-sqrt{display:inline-flex;align-items:flex-start}
.lx-sqrt .lx-radical{font-size:1.05em}
.lx-sqrt .lx-radicand{border-top:1px solid currentColor;padding:0 .12em}
.lx-fn{font-style:italic;margin-right:.1em}
.lx-var{font-style:italic}
.lx-text{font-style:normal}
.lx-sym{margin:0 .1em}
.lx-op{font-size:1.15em;margin:0 .1em}
.lx-space{display:inline-block;width:.8em}
.lx-thinspace{display:inline-block;width:.22em}
.lx-unknown{color:#b45309;font-style:italic}
`
