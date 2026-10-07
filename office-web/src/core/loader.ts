//! 产物加载器：把 CLI `unpack` 产物目录（JSON 文件引用结构）组装成组件需要的完整对象
//!
//! 产物目录里 `presentation.json` / `document.json` / `workbook.json` 的
//! `slides` / `parts` / `sheets` 字段是**相对路径数组**，本模块负责按路径抓取并内联。
//!
//! ```ts
//! const pptx = await loadPptx('/samples/pptx')          // 目录 URL（末尾可带 /）
//! const docx = await loadDocx('/samples/docx')
//! const xlsx = await loadXlsx('/samples/xlsx')
//! // 也可统一入口：
//! const data = await loadProduct('pptx', '/samples/pptx')
//! ```

import type { DocxDocument, DocxPart, XlsxSheet, XlsxWorkbook } from '@/types'
import { packSheet, type PackedSheet } from '@/renderers/xlsx/packed'
import { packSheetFromStream } from '@/renderers/xlsx/stream'
import type { PptxDoc, Slide } from '@/renderers/pptx/presentation'
import type { PdfDoc, PdfPage } from '@/renderers/pdf/types'

export type ProductKind = 'docx' | 'xlsx' | 'pptx' | 'pdf'

function joinUrl(base: string, rel: string): string {
  return `${base.replace(/\/$/, '')}/${rel.replace(/^\//, '')}`
}

async function fetchJson<T>(url: string): Promise<T> {
  const res = await fetch(url)
  if (!res.ok) throw new Error(`加载 ${url} 失败: ${res.status} ${res.statusText}`)
  return (await res.json()) as T
}

/** pptx 产物目录 → { slides: Slide[], width, height, ... } */
export async function loadPptx(baseUrl: string): Promise<PptxDoc> {
  const pres = await fetchJson<Record<string, unknown>>(joinUrl(baseUrl, 'presentation.json'))
  const paths = (pres.slides as unknown[] | undefined) ?? []
  const slides: Slide[] = []
  for (const p of paths) {
    if (typeof p === 'string') slides.push(await fetchJson<Slide>(joinUrl(baseUrl, p)))
    else slides.push(p as Slide) // 已内联的形态直接使用
  }
  return {
    slides,
    width: Number(pres.width ?? 13.333),
    height: Number(pres.height ?? 7.5),
    meta: pres.meta as PptxDoc['meta'],
    theme: pres.theme as PptxDoc['theme'],
  }
}

/** docx 产物目录 → { parts: Part[], styles, page, ... } */
export async function loadDocx(baseUrl: string): Promise<DocxDocument> {
  const doc = await fetchJson<Record<string, unknown>>(joinUrl(baseUrl, 'document.json'))
  const paths = (doc.parts as unknown[] | undefined) ?? []
  const parts: DocxPart[] = []
  for (const p of paths) {
    if (typeof p === 'string') {
      const part = await fetchJson<DocxPart>(joinUrl(baseUrl, p))
      parts.push(part)
    } else parts.push(p as DocxPart)
  }
  return { ...(doc as unknown as DocxDocument), parts }
}

/**
 * xlsx 产物目录 → 工作簿。
 * `pack: true` 时把工作表转为列式打包形态并清空原始 rows（百万行场景的内存优化：
 * 每单元格约 140B → 约 21B，字符串/数字格式去重）。
 */
export async function loadXlsx(
  baseUrl: string,
  opts: { pack?: boolean | 'stream' } = {},
): Promise<XlsxWorkbook | { sheets: PackedSheet[] }> {
  if (opts.pack === 'stream') return loadXlsxStreamed(baseUrl)
  const wb = await fetchJson<Record<string, unknown>>(joinUrl(baseUrl, 'workbook.json'))
  const paths = (wb.sheets as unknown[] | undefined) ?? []
  const sheets: XlsxSheet[] = []
  for (const p of paths) {
    if (typeof p === 'string') sheets.push(await fetchJson<XlsxSheet>(joinUrl(baseUrl, p)))
    else sheets.push(p as XlsxSheet)
  }
  if (opts.pack) {
    const packed = sheets.map((sh) => {
      const r = packSheet(sh, 20)
      // 打包后释放对象模型（本函数是数据唯一持有者）
      sh.rows = []
      return r.packed
    })
    return { sheets: packed }
  }
  return { ...(wb as unknown as XlsxWorkbook), sheets }
}

/**
 * 流式加载：工作表 JSON 边读边解析成列式数组，解析期不产生「每单元格一个对象」的峰值。
 * 适用于百万行级产物（文本流 + 列式数组即为全部内存）。
 */
export async function loadXlsxStreamed(baseUrl: string): Promise<{ sheets: PackedSheet[] }> {
  const wb = await fetchJson<Record<string, unknown>>(joinUrl(baseUrl, 'workbook.json'))
  const paths = (wb.sheets as unknown[] | undefined) ?? []
  const sheets: PackedSheet[] = []
  for (const p of paths) {
    const url = typeof p === 'string' ? joinUrl(baseUrl, p) : ''
    if (!url) {
      sheets.push(p as PackedSheet)
      continue
    }
    const res = await fetch(url)
    if (!res.ok || !res.body) throw new Error(`加载 ${url} 失败: ${res.status}`)
    const r = await packSheetFromStream(res.body)
    sheets.push(r.packed)
  }
  return { sheets }
}

/** pdf 产物目录 → { pages: PdfPage[], page_size, meta }（pages 按清单内联） */
export async function loadPdf(baseUrl: string): Promise<PdfDoc> {
  const doc = await fetchJson<Record<string, unknown>>(joinUrl(baseUrl, 'document.json'))
  const paths = (doc.pages as unknown[] | undefined) ?? []
  const pages: PdfPage[] = []
  for (const p of paths) {
    if (typeof p === 'string') pages.push(await fetchJson<PdfPage>(joinUrl(baseUrl, p)))
    else pages.push(p as PdfPage) // 已内联的形态直接使用
  }
  const size = (doc.page_size ?? {}) as { width?: number; height?: number }
  return {
    version: Number(doc.version ?? 2),
    meta: doc.meta as PdfDoc['meta'],
    page_size: { width: Number(size.width ?? 595.28), height: Number(size.height ?? 841.89) },
    pages,
  }
}

/** 统一入口：按格式加载产物目录 */
export function loadProduct(kind: ProductKind, baseUrl: string): Promise<unknown> {
  if (kind === 'pptx') return loadPptx(baseUrl)
  if (kind === 'docx') return loadDocx(baseUrl)
  if (kind === 'pdf') return loadPdf(baseUrl)
  return loadXlsx(baseUrl)
}
