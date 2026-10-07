//! 元素工厂与几何工具

import type { Element, Position, TextElement, TableElement, BarChartElement } from '@/types/presentation'

let idCounter = 0

/** 生成元素显示名（与 PowerPoint 命名风格一致） */
export function genName(type: string): string {
  idCounter += 1
  const label = type.charAt(0).toUpperCase() + type.slice(1)
  return `${label} ${idCounter + 1}`
}

/** 新建文本元素 */
export function createTextElement(x: number, y: number, w = 4, h = 1, vert?: string): TextElement {
  const el: TextElement = {
    type: 'text',
    text: '双击编辑文本',
    position: { x, y, w, h },
    font_size: 24,
    color: '000000',
    name: genName('text'),
  }
  if (vert && vert !== 'horz') el.vert = vert
  return el
}

/** 新建形状元素 */
export function createShapeElement(shapeType: string, x: number, y: number): Element {
  return {
    type: 'shape',
    shape_type: shapeType,
    position: { x, y, w: 2, h: 1.5 },
    fill: '4472C4',
    name: genName('shape'),
  }
}

/** 新建线条元素 */
export function createLineElement(x: number, y: number): Element {
  return {
    type: 'line',
    points: [
      [0, 0.5],
      [4, 0.5],
    ],
    position: { x, y, w: 4, h: 1 },
    color: '4472C4',
    width: 2,
    dash: 'solid',
    arrow_start: 'none',
    arrow_end: 'none',
    name: genName('line'),
  }
}

/** 新建图片元素 */
export function createImageElement(src: string, x: number, y: number): Element {
  return {
    type: 'image',
    src,
    position: { x, y, w: 4, h: 2.25 },
    name: genName('image'),
  }
}

/** 新建表格元素 */
export function createTableElement(x: number, y: number): TableElement {
  return {
    type: 'table',
    position: { x, y, w: 6, h: 2 },
    rows: [
      [
        { text: '列1', bold: true, fill: 'EEF2FF', align: 'center' },
        { text: '列2', bold: true, fill: 'EEF2FF', align: 'center' },
        { text: '列3', bold: true, fill: 'EEF2FF', align: 'center' },
      ],
      [
        { text: '数据', align: 'center' },
        { text: '数据', align: 'center' },
        { text: '数据', align: 'center' },
      ],
      [
        { text: '数据', align: 'center' },
        { text: '数据', align: 'center' },
        { text: '数据', align: 'center' },
      ],
    ],
    header_row: true,
    font_size: 14,
    color: '333333',
    name: genName('table'),
  }
}

/** 新建柱状图元素 */
export function createBarChartElement(x: number, y: number): BarChartElement {
  return {
    type: 'barChart',
    position: { x, y, w: 5, h: 3 },
    data: [30, 50, 40, 70, 60],
    labels: ['一月', '二月', '三月', '四月', '五月'],
    max: 100,
    show_values: true,
    label_color: '666666',
    font_size: 12,
  }
}

/** 新建图标元素 */
export function createIconElement(icon: string, x: number, y: number): Element {
  return {
    type: 'icon',
    icon,
    position: { x, y, w: 1.2, h: 1.2 },
    color: '4472C4',
    name: genName('icon'),
  }
}

/** 新建公式元素 */
export function createFormulaElement(latex: string, x: number, y: number): Element {
  return {
    type: 'formula',
    latex,
    position: { x, y, w: 3, h: 1 },
    color: '000000',
    font_size: 24,
    name: genName('formula'),
  }
}

/** 判断元素是否为高级组件 */
export function isComponent(el: Element): boolean {
  return [
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
  ].includes(el.type)
}

/** 判断元素是否包含可编辑文本（text 元素或 shape 内文字） */
export function hasEditableText(el: Element): boolean {
  return el.type === 'text' || (el.type === 'shape' && !!el.text)
}

/** 元素位置（16 种类型通用） */
export function elementPosition(el: Element): Position {
  return el.position
}

/** 元素旋转（仅 shape/line/image/group 支持；text/table/组件无 rotation 字段） */
export function elementRotation(el: Element): number | undefined {
  return 'rotation' in el ? (el as { rotation?: number }).rotation : undefined
}

/** 元素类型中文名 */
export function typeLabel(type: string): string {
  const map: Record<string, string> = {
    text: '文本',
    shape: '形状',
    line: '线条',
    image: '图片',
    icon: '图标',
    formula: '公式',
    table: '表格',
    group: '分组',
    progressBar: '进度条',
    progressRing: '环形进度',
    barChart: '柱状图',
    lineChart: '折线图',
    pieChart: '饼图',
    ringChart: '环形图',
    kpiCard: 'KPI 卡片',
    ratingStars: '星级评分',
    timeline: '时间线',
    processFlow: '流程步骤',
  }
  return map[type] ?? type
}

/** 获取元素可编辑的核心数据字段（组件） */
export function componentData(el: Element): Record<string, unknown> | null {
  switch (el.type) {
    case 'progressBar':
    case 'progressRing':
      return { value: el.value }
    case 'barChart':
    case 'lineChart':
    case 'pieChart':
    case 'ringChart':
      return { data: el.data, labels: el.labels, colors: el.colors }
    case 'kpiCard':
      return { value: el.value, label: el.label, delta: el.delta }
    case 'ratingStars':
      return { rating: el.rating, max: el.max }
    case 'timeline':
      return { items: el.items.length }
    case 'processFlow':
      return { steps: el.steps }
    default:
      return null
  }
}
