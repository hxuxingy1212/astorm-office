//! 文本内容解析：TextContent（string | Paragraph[]）→ HTML
//!
//! 与 Rust 侧 `TextContent` untagged 枚举对应。

import type { Paragraph, TextRun } from '@/types/presentation'
import { ptToPx, withHash } from '@/utils/convert'

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function runStyle(r: TextRun): string {
  const parts: string[] = []
  if (r.font_size) parts.push(`font-size:${ptToPx(r.font_size)}px`)
  if (r.bold) parts.push('font-weight:bold')
  if (r.italic) parts.push('font-style:italic')
  if (r.underline) parts.push('text-decoration:underline')
  if (r.color) parts.push(`color:${withHash(r.color)}`)
  if (r.font_family) parts.push(`font-family:${r.font_family}`)
  return parts.join(';')
}

function runHtml(r: TextRun): string {
  const content = escapeHtml(r.text)
  const inner = r.hyperlink
    ? `<a href="${escapeHtml(r.hyperlink)}" target="_blank" rel="noopener" style="color:inherit">${content}</a>`
    : content
  return `<span style="${runStyle(r)}">${inner}</span>`
}

function paraStyle(p: Paragraph): string {
  const parts: string[] = []
  if (p.align) parts.push(`text-align:${p.align}`)
  if (p.font_size) parts.push(`font-size:${ptToPx(p.font_size)}px`)
  // 行距倍数（无单位）
  if (p.line_spacing) parts.push(`line-height:${p.line_spacing}`)
  if (p.bold) parts.push('font-weight:bold')
  if (p.italic) parts.push('font-style:italic')
  if (p.color) parts.push(`color:${withHash(p.color)}`)
  return parts.join(';')
}

function paraHtml(p: Paragraph, defaultLineSpacing?: number): string {
  const bullet = p.bullet ? '• ' : ''
  let content = ''
  if (p.runs && p.runs.length > 0) {
    content = p.runs.map(runHtml).join('')
  } else if (p.text !== undefined && p.text !== null) {
    content = escapeHtml(p.text)
  }
  // 段落若无自身字号则以首个 run 的字号为准（决定行高）
  const eff: Paragraph = { ...p }
  if (!eff.font_size && p.runs?.length) {
    const first = p.runs[0]
    if (first.font_size) eff.font_size = first.font_size
  }
  // 段落无行距时用元素级默认
  if (!eff.line_spacing && defaultLineSpacing) eff.line_spacing = defaultLineSpacing
  const style = paraStyle(eff)
  return `<p style="${style}">${bullet}${content}</p>`
}

/** TextContent → HTML（多段落） */
export function textContentHtml(tc: string | Paragraph[], defaultLineSpacing?: number): string {
  if (typeof tc === 'string') {
    // 简单文本：按 \n 拆段
    return tc
      .split('\n')
      .map((line) => `<p>${escapeHtml(line)}</p>`)
      .join('')
  }
  return tc.map((p) => paraHtml(p, defaultLineSpacing)).join('')
}

/** 获取纯文本（用于编辑框与 get 显示） */
export function textContentPlain(tc: string | Paragraph[]): string {
  if (typeof tc === 'string') return tc
  return tc
    .map((p) => {
      if (p.text !== undefined && p.text !== null) return p.text
      if (p.runs) return p.runs.map((r) => r.text).join('')
      return ''
    })
    .join('\n')
}
