//! @astorm/office-viewer — astorm-office 四格式高保真预览组件库
//!
//! ```ts
//! import { OfficeViewer, PptxViewer, DocxViewer, XlsxViewer } from '@astorm/office-viewer'
//! import '@astorm/office-viewer/style.css'
//! ```
//! 组件接收 CLI `unpack` 产物 JSON，渲染高保真预览；
//! 鼠标悬浮任意元素高亮并显示路径卡片（元素名/路径/内容），点击回传 `select` 事件。

// 设计令牌与共享控件样式必须随库入口打包，lib 构建抽取进 dist/office-viewer.css
import './styles.css'

export { default as OfficeViewer } from '@/components/OfficeViewer.vue'
export { default as PptxViewer } from '@/components/PptxViewer.vue'
export { default as DocxViewer } from '@/components/DocxViewer.vue'
export { default as XlsxViewer } from '@/components/XlsxViewer.vue'
export { default as PdfViewer } from '@/components/PdfViewer.vue'
export { default as PathCard } from '@/components/PathCard.vue'

export { loadDocx, loadPdf, loadPptx, loadProduct, loadXlsx, loadXlsxStreamed } from '@/core/loader'
export type { ProductKind } from '@/core/loader'
export { usePathHover, jsonSnippet } from '@/core/hover'
export type { PathCardInfo, PathCardState, PathHoverApi, SelectInfo } from '@/core/hover'
export { colLetter, cellRef, xlsxCellPath, pptxElementPath, pdfElementPath, docxPath, typeOrdinal } from '@/core/path'
export { inchToPx, ptToPx, emuToPx, colCharsToPx, rowPtToPx, cssColor, normalizeColor } from '@/core/units'
export { formatNumber, displayValue } from '@/renderers/xlsx/numberFormat'
export {
  adapterFor,
  isPackedSheet,
  packSheet,
  packedMB,
  packedCellAt,
  sheetMeta,
} from '@/renderers/xlsx/packed'
export type { PackedSheet, PackResult, SheetAdapter } from '@/renderers/xlsx/packed'
export { buildRowLayout, rowTop, visibleRows, canvasScale } from '@/renderers/xlsx/virtual'
export { packSheetFromStream, RowScanner, matchObjectEnd } from '@/renderers/xlsx/stream'
export { latexToHtml, LATEX_CSS } from '@/renderers/docx/latex'
export { pdfElementContent, pdfElementType, type PdfDoc, type PdfElement, type PdfPage } from '@/renderers/pdf/types'
export { provideMediaResolver, MEDIA_RESOLVER } from '@/renderers/pptx/media'
export type { MediaResolver } from '@/renderers/pptx/media'

export type {
  Element,
  PptxDoc,
  Slide,
} from '@/renderers/pptx/presentation'
export type {
  DocxBlock,
  DocxDocument,
  DocxPart,
  DocxRun,
  XlsxCell,
  XlsxChart,
  XlsxSheet,
  XlsxStyle,
  XlsxWorkbook,
} from '@/types'
