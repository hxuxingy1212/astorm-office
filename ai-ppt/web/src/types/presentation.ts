//! json2pptx 产物 JSON 模型的 TypeScript 类型定义
//!
//! 与 Rust 侧 serde 模型一一对应（src/model/），位置单位为英寸。
//!
//! 权威类型参照 `src/types/generated.ts`（由 `pnpm gen:types` 从
//! skill/references/schema.json 自动生成，schema 又由 Rust `json2pptx schema --json`
//! 生成）；本文件是应用层手写别名/裁剪，新增模型字段时请对照 generated.ts 同步。

/** 位置与尺寸（英寸） */
export interface Position {
  x: number
  y: number
  w: number
  h: number
}

/** 填充：纯色（颜色字符串）/ 渐变 / 图案 / 图片，与后端 untagged 序列化一致 */
export type Fill =
  | string
  | { Gradient: { stops: GradientStop[]; angle?: number | null } }
  | { Pattern: { pattern: string; fg: string; bg: string } }
  | { Image: { src: string } }

export interface GradientStop {
  color: string
  position: number
}

/** 边框线 */
export interface Line {
  color: string
  width: number
}

/** 阴影 */
export interface Shadow {
  blur: number
  distance: number
  angle: number
  opacity: number
  color: string
}

/** 动画（只读展示，编辑 v1 不改动画） */
export interface Animation {
  type: string
  duration?: number
  delay?: number
  trigger?: string
  order?: number
  [key: string]: unknown
}

// === 基础元素 ===

/** 文本 run */
export interface TextRun {
  text: string
  font_size?: number
  bold?: boolean
  italic?: boolean
  underline?: boolean
  color?: string
  font_family?: string
  hyperlink?: string
}

/** 段落 */
export interface Paragraph {
  runs?: TextRun[]
  text?: string
  font_size?: number
  bold?: boolean
  italic?: boolean
  color?: string
  align?: string
  /** 行距倍数（如 1.0 单倍 / 1.2 / 1.5） */
  line_spacing?: number
  bullet?: boolean
}

export interface TextElement {
  type: 'text'
  text: string | Paragraph[]
  position: Position
  name?: string
  font_size?: number
  bold?: boolean
  italic?: boolean
  underline?: boolean
  color?: string
  font_family?: string
  align?: string
  vert_align?: string
  /** 文字方向：horz 横向（默认）/ vert 纵向 */
  vert?: string
  /** 行距倍数（如 1.0 单倍 / 1.2 / 1.5），段落级优先 */
  line_spacing?: number
  wrap?: boolean
  fill?: Fill
  fill_alpha?: number
  line?: Line
  line_alpha?: number
  shadow?: Shadow
  animations?: Animation[]
}

export interface ShapeElement {
  type: 'shape'
  shape_type: string
  position: Position
  name?: string
  rotation?: number
  fill?: Fill
  fill_alpha?: number
  no_fill?: boolean
  line?: Line
  line_alpha?: number
  shadow?: Shadow
  adjust?: Record<string, number>
  text?: string | Paragraph[]
  font_size?: number
  color?: string
  align?: string
  vert_align?: string
  /** 行距倍数（如 1.0 单倍 / 1.2 / 1.5），段落级优先 */
  line_spacing?: number
  animations?: Animation[]
}

export interface LineElement {
  type: 'line'
  position: Position
  name?: string
  points: [number, number][]
  color?: string
  width?: number
  dash?: string
  arrow_start?: string
  arrow_end?: string
  smooth?: boolean
  rotation?: number
  animations?: Animation[]
}

export interface IconElement {
  type: 'icon'
  position: Position
  icon: string
  color?: string
  name?: string
  rotation?: number
  line?: Line
  animations?: Animation[]
}

export interface FormulaElement {
  type: 'formula'
  position: Position
  latex: string
  color?: string
  name?: string
  rotation?: number
  font_size?: number
  animations?: Animation[]
}

export interface Crop {
  left: number
  top: number
  right: number
  bottom: number
}

export interface ImageElement {
  type: 'image'
  src: string
  position: Position
  name?: string
  rotation?: number
  crop?: Crop
  hyperlink?: string
  line?: Line
  animations?: Animation[]
}

export interface TableCell {
  text: string
  font_size?: number
  bold?: boolean
  color?: string
  fill?: string
  align?: string
  colspan?: number
  rowspan?: number
}

export interface TableElement {
  type: 'table'
  position: Position
  name?: string
  rows: TableCell[][]
  column_widths?: number[]
  header_row?: boolean
  font_size?: number
  color?: string
  animations?: Animation[]
}

export interface GroupElement {
  type: 'group'
  position: Position
  name?: string
  rotation?: number
  children: Element[]
  animations?: Animation[]
}

// === 高级组件 ===

export interface ProgressBarElement {
  type: 'progressBar'
  position: Position
  value?: number
  color?: string
  track_color?: string
  rounded?: boolean
  show_label?: boolean
  label?: string
  text_color?: string
  font_size?: number
}

export interface ProgressRingElement {
  type: 'progressRing'
  position: Position
  value?: number
  color?: string
  track_color?: string
  thickness?: number
  show_label?: boolean
  text_color?: string
  font_size?: number
}

export interface BarChartElement {
  type: 'barChart'
  position: Position
  data: number[]
  labels?: string[]
  colors?: string[]
  max?: number
  show_values?: boolean
  axis?: boolean
  label_color?: string
  font_size?: number
}

export interface LineChartElement {
  type: 'lineChart'
  position: Position
  data: number[]
  labels?: string[]
  colors?: string[]
  max?: number
  show_values?: boolean
  axis?: boolean
  smooth?: boolean
  label_color?: string
  font_size?: number
}

export interface PieChartElement {
  type: 'pieChart'
  position: Position
  data: number[]
  labels?: string[]
  colors?: string[]
  show_values?: boolean
  label_color?: string
  font_size?: number
}

export interface RingChartElement {
  type: 'ringChart'
  position: Position
  data: number[]
  labels?: string[]
  colors?: string[]
  thickness?: number
  show_values?: boolean
  label_color?: string
  font_size?: number
}

export interface KpiCardElement {
  type: 'kpiCard'
  position: Position
  value: string
  label?: string
  delta?: string
  bg?: string
  accent?: string
  rounded?: boolean
  text_color?: string
}

export interface RatingStarsElement {
  type: 'ratingStars'
  position: Position
  rating: number
  max?: number
  color?: string
  empty_color?: string
}

export interface TimelineElement {
  type: 'timeline'
  position: Position
  items: Record<string, unknown>[]
  line_color?: string
  dot_color?: string
  label_color?: string
}

export interface ProcessFlowElement {
  type: 'processFlow'
  position: Position
  steps: string[]
  colors?: string[]
  text_color?: string
  font_size?: number
}

/** 全部元素联合类型 */
export type Element =
  | TextElement
  | ShapeElement
  | LineElement
  | ImageElement
  | IconElement
  | FormulaElement
  | TableElement
  | GroupElement
  | ProgressBarElement
  | ProgressRingElement
  | BarChartElement
  | LineChartElement
  | PieChartElement
  | RingChartElement
  | KpiCardElement
  | RatingStarsElement
  | TimelineElement
  | ProcessFlowElement

/** 组件类型列表（编辑 v1 支持修改核心字段） */
export const COMPONENT_TYPES = [
  'progressBar',
  'progressRing',
  'barChart',
  'lineChart',
  'pieChart',
  'ringChart',
  'kpiCard',
  'ratingStars',
  'timeline',
  'processFlow',
] as const

// === 幻灯片与演示 ===

export interface Transition {
  type: string
  speed?: string
  advance_on_click?: boolean
  advance_after?: number
}

export interface Slide {
  elements: Element[]
  background?: string | Record<string, unknown>
  transition?: Transition
  notes?: string
}

export interface Theme {
  colors?: Record<string, string>
  major_font?: string
  minor_font?: string
}

export interface Meta {
  title?: string
  author?: string
}

/** 后端 overview 响应 */
export interface Overview {
  name: string
  width: number
  height: number
  meta?: Meta
  theme?: Theme
  slides: { index: number; data: Slide }[]
  media: string[]
}
