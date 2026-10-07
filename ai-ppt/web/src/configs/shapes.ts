//! OOXML prstGeom 预设形状 → SVG path 映射
//!
//! 与 Rust 侧 `src/utils/constants.rs` 的 SHAPE_TYPES 对应（86 种）。
//! 多边形 / 星形 / 齿轮等用程序化生成，其余手写常量 path（viewBox 200×200）。
//! 未覆盖的形状返回 null，渲染为 rect + 居中形状名（可编辑，渲染近似）。

// === 程序化生成 ===

/** 正多边形 path（cx/cy 中心，rx/ry 半径，rotation 起始角） */
function polygonPath(
  n: number,
  rx = 100,
  ry = 100,
  cx = 100,
  cy = 100,
  rotation = -90,
): string {
  const pts: string[] = []
  for (let i = 0; i < n; i += 1) {
    const ang = ((rotation + (360 / n) * i) * Math.PI) / 180
    pts.push(`${(cx + rx * Math.cos(ang)).toFixed(1)} ${(cy + ry * Math.sin(ang)).toFixed(1)}`)
  }
  return `M ${pts.join(' L ')} Z`
}

/** 星形 path（n 角星，内外半径交替） */
function starPath(n: number, outer = 100, inner = 42): string {
  const pts: string[] = []
  for (let i = 0; i < n * 2; i += 1) {
    const r = i % 2 === 0 ? outer : inner
    const ang = ((-90 + (360 / (n * 2)) * i) * Math.PI) / 180
    pts.push(`${(100 + r * Math.cos(ang)).toFixed(1)} ${(100 + r * Math.sin(ang)).toFixed(1)}`)
  }
  return `M ${pts.join(' L ')} Z`
}

/** 齿轮 path（n 齿，简化多边形近似） */
function gearPath(n: number, outer = 95, inner = 70, teeth = 18): string {
  const pts: string[] = []
  const step = 360 / (n * 4)
  for (let i = 0; i < n * 4; i += 1) {
    const isTooth = i % 2 === 0
    const r = isTooth ? outer : inner
    const ang = ((-90 + step * i) * Math.PI) / 180
    pts.push(`${(100 + r * Math.cos(ang)).toFixed(1)} ${(100 + r * Math.sin(ang)).toFixed(1)}`)
  }
  void teeth
  return `M ${pts.join(' L ')} Z`
}

/** 太阳：圆 + 8 射线 */
function sunPath(): string {
  const rays: string[] = []
  for (let i = 0; i < 8; i += 1) {
    const ang = ((i * 45) * Math.PI) / 180
    const x1 = 100 + 78 * Math.cos(ang)
    const y1 = 100 + 78 * Math.sin(ang)
    const x2 = 100 + 95 * Math.cos(ang)
    const y2 = 100 + 95 * Math.sin(ang)
    rays.push(`M ${x1.toFixed(1)} ${y1.toFixed(1)} L ${x2.toFixed(1)} ${y2.toFixed(1)}`)
  }
  return `M 100 35 A 65 65 0 1 1 100 165 A 65 65 0 1 1 100 35 Z ${rays.join(' ')}`
}

/** 圆形箭头：环形 + 箭头头部 */
function circularArrowPath(): string {
  return [
    'M 100 25 A 75 75 0 1 1 38 55',
    'L 38 85 L 8 48 L 32 18 L 58 40 L 58 60',
    'A 60 60 0 1 0 100 40 Z',
  ].join(' ')
}

// === 手写常量（viewBox 200×200） ===

const MANUAL_PATHS: Record<string, string> = {
  // --- 基础 ---
  rect: 'M 0 0 L 200 0 L 200 200 L 0 200 Z',
  roundRect:
    'M 50 0 L 150 0 Q 200 0 200 50 L 200 150 Q 200 200 150 200 L 50 200 Q 0 200 0 150 L 0 50 Q 0 0 50 0 Z',
  ellipse: 'M 100 0 A 50 50 0 1 1 100 200 A 50 50 0 1 1 100 0 Z',
  triangle: 'M 100 10 L 190 190 L 10 190 Z',
  diamond: 'M 100 5 L 195 100 L 100 195 L 5 100 Z',
  isocelesTriangle: 'M 100 10 L 190 180 L 10 180 Z',
  rightTriangle: 'M 15 185 L 15 25 L 185 185 Z',
  trapezoid: 'M 45 40 L 155 40 L 190 190 L 10 190 Z',
  parallelogram: 'M 40 20 L 160 20 L 180 180 L 60 180 Z',
  homePlate: 'M 20 50 L 100 10 L 180 50 L 180 160 L 100 195 L 20 160 Z',
  teardrop:
    'M 100 10 C 165 95 185 130 185 160 A 85 85 0 1 1 15 160 C 15 130 35 95 100 10 Z',
  foldedCorner: 'M 0 0 L 140 0 L 200 60 L 200 200 L 0 200 Z M 140 0 L 140 60 L 200 60 Z',
  plus: 'M 70 0 L 130 0 L 130 70 L 200 70 L 200 130 L 130 130 L 130 200 L 70 200 L 70 130 L 0 130 L 0 70 L 70 70 Z',
  cross: 'M 65 0 L 135 0 L 135 65 L 200 65 L 200 135 L 135 135 L 135 200 L 65 200 L 65 135 L 0 135 L 0 65 L 65 65 Z',
  noSmoking:
    'M 100 20 A 80 80 0 1 1 100 180 A 80 80 0 1 1 100 20 Z M 100 60 A 40 40 0 1 0 100 140 A 40 40 0 1 0 100 60 Z M 30 92 L 170 108 L 164 116 L 24 100 Z',
  moon: 'M 140 10 A 90 90 0 1 0 190 160 A 70 70 0 1 1 140 10 Z',
  smileyFace:
    'M 100 20 A 80 80 0 1 1 100 180 A 80 80 0 1 1 100 20 Z M 65 85 A 10 10 0 1 1 65 84.9 Z M 135 85 A 10 10 0 1 1 135 84.9 Z M 60 120 Q 100 160 140 120',
  frame: 'M 0 0 L 200 0 L 200 200 L 0 200 Z M 20 20 L 20 180 L 180 180 L 180 20 Z',
  can: 'M 30 40 L 170 40 L 160 180 L 40 180 Z M 30 40 C 30 30 170 30 170 40 C 170 50 30 50 30 40 Z',
  funnel: 'M 30 30 L 170 30 L 120 100 L 120 170 L 80 170 L 80 100 Z',
  // --- 括号与弧 ---
  bracketPair:
    'M 55 20 C 40 20 40 85 28 100 C 40 115 40 180 55 180 L 65 180 C 50 180 50 108 40 100 C 50 92 50 20 65 20 Z M 135 20 C 150 20 150 85 172 100 C 150 115 150 180 135 180 L 125 180 C 140 180 140 108 160 100 C 140 92 140 20 125 20 Z',
  leftBrace:
    'M 55 10 C 35 10 35 85 18 100 C 35 115 35 190 55 190 L 65 190 C 47 190 47 108 32 100 C 47 92 47 10 65 10 Z',
  rightBrace:
    'M 135 10 C 155 10 155 85 182 100 C 155 115 155 190 135 190 L 125 190 C 143 190 143 108 168 100 C 143 92 143 10 125 10 Z',
  leftBracket:
    'M 55 20 L 55 180 L 85 180 L 85 160 L 75 160 L 75 40 L 85 40 L 85 20 Z',
  rightBracket:
    'M 145 20 L 145 180 L 115 180 L 115 160 L 125 160 L 125 40 L 115 40 L 115 20 Z',
  chord: 'M 20 100 A 80 80 0 0 1 180 100 L 20 100 Z',
  pie: 'M 100 100 L 100 20 A 80 80 0 0 1 178 60 Z',
  // --- 箭头 ---
  rightArrow: 'M 10 40 L 110 40 L 110 10 L 190 100 L 110 190 L 110 160 L 10 160 Z',
  leftArrow: 'M 190 40 L 90 40 L 90 10 L 10 100 L 90 190 L 90 160 L 190 160 Z',
  upArrow: 'M 40 10 L 160 10 L 160 110 L 190 110 L 100 190 L 10 110 L 40 110 Z',
  downArrow: 'M 40 190 L 160 190 L 160 90 L 190 90 L 100 10 L 10 90 L 40 90 Z',
  chevron: 'M 20 40 L 110 100 L 20 160 L 50 160 L 140 100 L 50 40 Z',
  leftRightArrow:
    'M 30 40 L 90 40 L 90 10 L 170 100 L 90 190 L 90 160 L 30 160 Z M 170 40 L 110 40 L 110 10 L 30 100 L 110 190 L 110 160 L 170 160 Z',
  upDownArrow:
    'M 40 30 L 160 30 L 160 90 L 190 90 L 100 170 L 10 90 L 40 90 Z M 40 170 L 160 170 L 160 110 L 190 110 L 100 30 L 10 110 L 40 110 Z',
  leftUpArrow:
    'M 30 30 L 160 130 L 120 130 L 170 180 L 130 180 L 180 200 L 110 190 L 110 150 L 30 30 Z',
  bentArrow:
    'M 10 40 L 110 40 L 110 10 L 190 100 L 110 190 L 110 160 L 10 160 Z M 110 40 L 170 100 L 110 160 Z',
  notchedRightArrow:
    'M 10 30 L 110 30 L 110 10 L 190 100 L 110 190 L 110 170 L 10 170 Z',
  notchedLeftArrow:
    'M 190 30 L 90 30 L 90 10 L 10 100 L 90 190 L 90 170 L 190 170 Z',
  pentagonChevron:
    'M 10 30 L 130 30 L 190 100 L 130 170 L 10 170 L 60 100 Z M 40 70 L 90 100 L 40 130 Z',
  // --- 流程符号 ---
  flowChartProcess: 'M 20 20 L 180 20 L 180 180 L 20 180 Z',
  flowChartDecision: 'M 100 10 L 190 100 L 100 190 L 10 100 Z',
  flowChartTerminator:
    'M 40 20 L 160 20 Q 185 20 185 100 Q 185 180 160 180 L 40 180 Q 15 180 15 100 Q 15 20 40 20 Z',
  flowChartInputOutput: 'M 30 20 L 170 20 L 130 180 L 20 180 Z',
  flowChartDocument:
    'M 20 20 L 180 20 L 180 150 Q 100 185 20 150 Z M 20 20 L 20 150 Q 100 185 180 150',
  flowChartPredefinedProcess:
    'M 30 20 L 170 20 L 170 180 L 30 180 Z M 60 20 L 60 180 M 140 20 L 140 180',
  flowChartData: 'M 20 100 L 60 20 L 180 20 L 140 100 L 180 180 L 60 180 Z',
  flowChartManualInput: 'M 20 20 L 180 20 L 120 100 L 180 180 L 20 180 Z',
  flowChartPreparation: 'M 40 20 L 160 20 L 190 100 L 160 180 L 40 180 L 10 100 Z',
  flowChartConnector: 'M 100 35 A 65 65 0 1 1 100 165 A 65 65 0 1 1 100 35 Z',
  flowChartMerge: 'M 20 40 L 180 40 L 100 170 Z',
  flowChartOr:
    'M 100 25 A 75 75 0 1 1 100 175 A 75 75 0 1 1 100 25 Z M 25 100 L 175 100',
  // --- 其他 ---
  donut: 'M 100 30 A 70 70 0 1 1 100 170 A 70 70 0 1 1 100 30 Z M 100 75 A 25 25 0 1 0 100 125 A 25 25 0 1 0 100 75 Z',
  blockArc:
    'M 100 100 L 100 20 A 80 80 0 1 1 30 60 L 30 85 A 58 58 0 0 0 100 45 Z',
  line: 'M 10 10 L 190 190',
  cloud:
    'M 60 60 A 25 25 0 0 1 85 42 A 35 35 0 0 1 145 50 A 25 25 0 0 1 165 85 A 22 22 0 0 1 150 115 L 55 115 A 30 30 0 0 1 60 60 Z',
  heart: 'M 100 35 C 55 0 0 55 0 100 C 0 140 55 190 100 215 C 145 190 200 140 200 100 C 200 55 145 0 100 35 Z',
  lightningBolt: 'M 110 10 L 40 110 L 90 110 L 70 190 L 160 85 L 105 85 Z',
  // --- 标注 ---
  callout1:
    'M 50 20 L 150 20 Q 180 20 180 50 L 180 140 Q 180 170 150 170 L 95 170 L 70 195 L 75 170 L 50 170 Q 20 170 20 140 L 20 50 Q 20 20 50 20 Z',
  callout2:
    'M 100 15 A 85 85 0 1 1 100 185 A 85 85 0 1 1 100 15 Z M 95 165 L 70 195 L 85 160 Z',
  roundRectCallout:
    'M 50 20 L 150 20 Q 180 20 180 50 L 180 150 Q 180 180 150 180 L 50 180 Q 20 180 20 150 L 20 50 Q 20 20 50 20 Z M 95 180 L 70 200 L 85 180 Z',
  cloudCallout:
    'M 60 60 A 25 25 0 0 1 85 42 A 35 35 0 0 1 145 50 A 25 25 0 0 1 165 85 A 22 22 0 0 1 150 115 L 95 115 L 75 140 L 80 115 L 55 115 A 30 30 0 0 1 60 60 Z',
  wedgeRectCallout:
    'M 20 40 L 180 40 L 180 160 L 20 160 Z M 100 160 L 100 190 L 60 160 Z',
  wedgeOvalCallout:
    'M 100 20 A 80 80 0 1 1 100 180 A 80 80 0 1 1 100 20 Z M 105 160 L 120 195 L 75 165 Z',
  borderCallout1:
    'M 20 30 L 180 30 L 180 170 L 20 170 Z M 35 45 L 165 45 L 165 155 L 35 155 Z M 90 170 L 90 195 L 65 170 Z',
  borderCallout2:
    'M 20 30 L 180 30 L 180 170 L 20 170 Z M 35 45 L 165 45 L 165 155 L 35 155 Z M 105 170 L 130 195 L 80 170 Z',
}

// === 汇总 ===

function buildShapePaths(): Record<string, string> {
  const paths: Record<string, string> = { ...MANUAL_PATHS }

  // 程序化生成
  paths.pentagon = polygonPath(5)
  paths.hexagon = polygonPath(6)
  paths.octagon = polygonPath(8)
  paths.regularPentagon = polygonPath(5, 100, 100, 100, 95)
  paths.star5 = starPath(5)
  paths.star6 = starPath(6)
  paths.star8 = starPath(8)
  paths.star10 = starPath(10)
  paths.star16 = starPath(16, 95, 40)
  paths.star24 = starPath(24, 95, 45)
  paths.sun = sunPath()
  paths.gear6 = gearPath(6)
  paths.gear9 = gearPath(9)
  paths.circularArrow = circularArrowPath()
  // 曲线箭头（近似：弯曲尾 + 箭头尖）
  paths.curvedRightArrow =
    'M 20 120 Q 20 180 100 180 Q 150 180 170 150 L 170 170 L 195 130 L 165 100 L 165 130 Q 145 155 100 155 Q 45 155 45 120 Z'
  paths.curvedLeftArrow =
    'M 180 120 Q 180 180 100 180 Q 50 180 30 150 L 30 170 L 5 130 L 35 100 L 35 130 Q 55 155 100 155 Q 155 155 155 120 Z'
  paths.curvedUpArrow =
    'M 120 20 Q 180 20 180 100 Q 180 150 150 170 L 170 170 L 130 195 L 100 165 L 130 165 L 130 145 Q 155 145 155 100 Q 155 45 120 45 Z'
  paths.curvedDownArrow =
    'M 120 180 Q 180 180 180 100 Q 180 50 150 30 L 170 30 L 130 5 L 100 35 L 130 35 L 130 55 Q 155 55 155 100 Q 155 155 120 155 Z'
  paths.quadArrow =
    'M 90 10 L 110 10 L 110 90 L 190 90 L 190 110 L 110 110 L 110 190 L 90 190 L 90 110 L 10 110 L 10 90 L 90 90 Z'

  return paths
}

export const SHAPE_PATHS: Record<string, string> = buildShapePaths()

/** 获取形状 SVG path（未覆盖返回 null） */
export function shapePath(shapeType: string): string | null {
  return SHAPE_PATHS[shapeType] ?? null
}
