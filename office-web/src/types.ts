//! 四格式模型类型（从各 CLI 的 schema.json 提炼的只读子集）
//! 完整 schema 见 ai-word/ai-excel/ai-ppt 的 skill/references/schema.json

// ------------------------------------------------------------------ xlsx ---

export interface XlsxFont {
  name?: string
  size?: number
  bold?: boolean
  italic?: boolean
  strike?: boolean
  underline?: string | null
  color?: string | null
}

export interface XlsxBorder {
  all?: string | null
  top?: string | null
  bottom?: string | null
  left?: string | null
  right?: string | null
  color?: string | null
}

export interface XlsxAlignment {
  horizontal?: string | null
  vertical?: string | null
  wrap_text?: boolean
}

export interface XlsxStyle {
  font?: XlsxFont | null
  fill?: string | null
  border?: XlsxBorder | null
  alignment?: XlsxAlignment | null
  locked?: boolean | null
  num_fmt?: string | null
}

export interface XlsxCell {
  reference?: string
  value?: string | number | boolean | null
  formula?: string | null
  cell_type?: string | null
  number_format?: string | null
  style?: string | { [k: string]: unknown } | null
  link?: string | null
  link_tooltip?: string | null
  comment?: string | null
}

export interface XlsxRow {
  index?: number
  height?: number | null
  cells: XlsxCell[]
}

export interface XlsxColumn {
  width?: number | null
  hidden?: boolean
}

export interface XlsxChart {
  type: string
  data_range?: string | null
  categories?: string | null
  title?: string | null
  legend?: string | null
  anchor?: string | null
  size?: { w: number; h: number } | null
  colors?: string[]
  show_values?: boolean
  series?: { name?: string | null; color?: string | null; kind?: string | null }[]
}

export interface XlsxImage {
  src: string
  anchor?: string | null
  size?: { w: number; h: number } | null
}

export interface XlsxSheet {
  name: string
  rows: XlsxRow[]
  columns?: XlsxColumn[]
  merges?: string[]
  charts?: XlsxChart[]
  images?: XlsxImage[]
  hidden?: boolean
  freeze?: string | null
  auto_filter?: string | null
  gridlines?: boolean | null
  headings?: boolean | null
  default_col_width?: number | null
  base_col_width?: number | null
  default_row_height?: number | null
  background?: string | null
  tab_color?: string | null
  print?: XlsxPrintSettings | null
  conditional_formats?: unknown[]
}

export interface XlsxPrintSettings {
  orientation?: string | null
  paper_size?: number | null
  scale?: number | null
  margin_left?: number | null
  margin_right?: number | null
  margin_top?: number | null
  margin_bottom?: number | null
}

export interface XlsxWorkbook {
  sheets: XlsxSheet[]
  styles?: Record<string, XlsxStyle>
  named_ranges?: unknown[]
}

// ------------------------------------------------------------------ docx ---

export interface DocxRun {
  text: string
  bold?: boolean
  italic?: boolean
  underline?: boolean
  strike?: boolean
  color?: string | null
  font_size?: number | null
  font_family?: string | null
  highlight?: string | null
  vert_align?: string | null
  link?: string | null
  code?: boolean
}

export interface DocxBlockBase {
  type: string
}

export interface DocxParagraphBlock extends DocxBlockBase {
  type: 'paragraph'
  text?: string | null
  runs?: DocxRun[] | null
  style?: string | null
  align?: string | null
  para_id?: string | null
  first_line_indent?: number | null
  indent?: number | null
  hanging_indent?: number | null
  line_spacing?: number | null
  space_before?: number | null
  space_after?: number | null
  bold?: boolean
  italic?: boolean
  font_size?: number | null
  color?: string | null
  font_family?: string | null
  shading?: string | null
  border?: string | null
  keep_next?: boolean | null
  page_break_before?: boolean | null
}

export interface DocxHeadingBlock extends DocxBlockBase {
  type: 'heading'
  level: number
  text: string
  runs?: DocxRun[] | null
  align?: string | null
  numbering?: boolean | null
}

export interface DocxListBlock extends DocxBlockBase {
  type: 'list'
  ordered: boolean
  items: { blocks: DocxBlock[] }[]
  level?: number | null
  start?: number | null
}

export interface DocxTableBlock extends DocxBlockBase {
  type: 'table'
  rows: { cells: { blocks: DocxBlock[]; colspan?: number | null; align?: string | null }[] }[]
  header_row?: boolean | null
  column_widths?: number[] | null
  style?: string | null
  borders?: boolean | null
}

export interface DocxImageBlock extends DocxBlockBase {
  type: 'image'
  src: string
  width?: number | null
  height?: number | null
  align?: string | null
  caption?: string | null
  alt?: string | null
}

export interface DocxFormulaBlock extends DocxBlockBase {
  type: 'formula'
  latex: string
  display?: boolean
  number?: string | null
}

export interface DocxCodeBlock extends DocxBlockBase {
  type: 'code'
  text: string
  lang?: string | null
}

export interface DocxQuoteBlock extends DocxBlockBase {
  type: 'quote'
  text: string
}

export interface DocxCaptionBlock extends DocxBlockBase {
  type: 'caption'
  text: string
  of?: string | null
}

export interface DocxTocBlock extends DocxBlockBase {
  type: 'toc'
  title?: string | null
  levels?: number[] | null
  entries?: { level: number; text: string }[]
}

export interface DocxBibliographyBlock extends DocxBlockBase {
  type: 'bibliography'
  title?: string | null
  entries: {
    authors?: string[]
    title?: string
    container?: string | null
    year?: number | string | null
    publisher?: string | null
    url?: string | null
  }[]
}

export interface DocxShapeBlock extends DocxBlockBase {
  type: 'shape' | 'textbox'
  text?: string | null
  shape_type?: string | null
  x?: number | null
  y?: number | null
  width?: number | null
  height?: number | null
  fill?: string | null
  line?: string | null
}

export interface DocxPageBreakBlock extends DocxBlockBase {
  type: 'page_break'
}

export type DocxBlock =
  | DocxParagraphBlock
  | DocxHeadingBlock
  | DocxListBlock
  | DocxTableBlock
  | DocxImageBlock
  | DocxFormulaBlock
  | DocxCodeBlock
  | DocxQuoteBlock
  | DocxCaptionBlock
  | DocxTocBlock
  | DocxBibliographyBlock
  | DocxShapeBlock
  | DocxPageBreakBlock
  | (DocxBlockBase & Record<string, unknown>)

export interface DocxSection {
  page?: { size?: string | null; orientation?: string | null; margins?: Record<string, number> | null }
  header?: string | null
  footer_format?: string | null
  footer_page_number?: boolean | null
}

export interface DocxPart {
  blocks: DocxBlock[]
  section?: DocxSection | null
  name?: string | null
}

export interface DocxStyle {
  font?: string | null
  size?: number | null
  bold?: boolean | null
  italic?: boolean | null
  color?: string | null
  align?: string | null
  line_spacing?: number | null
  space_before?: number | null
  space_after?: number | null
  first_line_indent?: number | null
  indent?: number | null
  based_on?: string | null
}

export interface DocxDocument {
  parts: DocxPart[]
  styles?: Record<string, DocxStyle>
  page?: { size?: string | null; orientation?: string | null; margins?: Record<string, number> | null }
  theme?: unknown
  template?: string | null
  watermark?: string | null
  background?: string | null
}
