<script setup lang="ts">
//! PDF 单元素渲染（与 json2pdf render html 同构的近似还原）：
//! - 坐标 pt → px（96dpi），y 轴按 flip 翻转（页面层 y 向上 → 屏幕；group 内 raw）
//! - 视口偏移（crop 打印态语义）：元素坐标平移 -(vx0, vy0)
//! - text/rect/image 用定位 div，polyline/path 用整页 SVG（矩阵含 y 翻转与视口平移），
//!   shading 近似 CSS 渐变（无色标的 mesh/func0 兜底中灰），pattern_rect 画占位底色
//! - 悬浮命中：自带盒模型类型用本体，向量/渐变类型用包围盒透明命中层
import { computed } from 'vue'
import { ptToPx } from '@/core/units'
import { useMediaResolver } from '@/renderers/pptx/media'
import {
  pdfChildren,
  pdfCssFont,
  pdfElementBBox,
  type PdfElement,
} from '@/renderers/pdf/types'

const props = withDefaults(
  defineProps<{
    el: PdfElement
    /** 可视页宽/页高（pt，crop 后） */
    pageW: number
    pageH: number
    /** 视口原点（crop 左下角） */
    vx0?: number
    vy0?: number
    /** 页面层 y 向上翻转；group 内部为 false（raw，配合矩阵映射） */
    flip?: boolean
    /** CLI 路径（/page[1]/text[2]） */
    path: string
    /** Tr≥4 裁剪文本配对后的填充色（透过字形填充） */
    pairFill?: string | null
    /** 纯裁剪文本（不可见） */
    hidden?: boolean
  }>(),
  { vx0: 0, vy0: 0, flip: true, pairFill: null, hidden: false },
)

const emit = defineEmits<{
  (e: 'element-hover', ev: MouseEvent, path: string): void
  (e: 'element-move', ev: MouseEvent): void
  (e: 'element-leave'): void
  (e: 'element-mousedown', ev: MouseEvent, path: string): void
}>()

const resolveMedia = useMediaResolver()

const ppp = ptToPx(1)

/** y 换算：flip 时屏幕 y = 页高 − (pdf y − vy0) */
function fy(y: number): number {
  return props.flip ? props.pageH - (y - props.vy0) : y - props.vy0
}

function onHover(ev: MouseEvent) {
  emit('element-hover', ev, props.path)
}

function onSelect(ev: MouseEvent) {
  emit('element-mousedown', ev, props.path)
}

const el = computed(() => props.el)

/** 主元素样式（text/rect/pattern_rect/image 共用的定位盒） */
const boxStyle = computed(() => {
  const e = el.value
  if (!['text', 'rect', 'pattern_rect', 'image'].includes(e.type)) return null
  const x = e.x ?? 0
  const y = e.y ?? 0
  const w = e.w ?? 0
  const h = e.h ?? 0
  const size = e.size ?? 12
  const top = e.type === 'text' ? fy(y) - size : fy(y) - h
  const s: Record<string, string> = {
    left: `${(x - props.vx0) * ppp}px`,
    top: `${top * ppp}px`,
    opacity: String(e.alpha ?? 1),
  }
  if (e.type === 'text') {
    s.width = 'max-content'
    s.maxWidth = `${props.pageW * ppp}px`
    s.fontSize = `${size * ppp}px`
    s.fontFamily = pdfCssFont(e.font)
    s.color = props.pairFill ?? (props.hidden ? 'transparent' : e.color ?? '#000')
    s.whiteSpace = 'pre'
    if (e.bold) s.fontWeight = '700'
    if (e.italic) s.fontStyle = 'italic'
    if (e.rotation) {
      s.transformOrigin = '0 100%'
      s.transform = `rotate(${-e.rotation}deg)`
    }
  } else {
    s.width = `${w * ppp}px`
    s.height = `${h * ppp}px`
    if (e.type === 'rect') {
      if (e.fill) s.background = e.fill
      if (e.stroke) s.border = `${(e.line_width ?? 1) * ppp}px solid ${e.stroke}`
      if (e.rotation) {
        s.transformOrigin = '0 100%'
        s.transform = `rotate(${-e.rotation}deg)`
      }
    } else if (e.type === 'pattern_rect') {
      s.background = '#eef1f5'
      s.border = '1px dashed #9aa7b4'
    } else if (e.rotation) {
      s.transformOrigin = '0 0'
      s.transform = `rotate(${-e.rotation}deg)`
    }
  }
  return s
})

/** 向量元素（polyline/path）的整页 SVG 与 y 翻转 + 视口平移矩阵 */
const vecSvg = computed(() => {
  const e = el.value
  if (e.type !== 'polyline' && e.type !== 'path') return null
  const sy = props.flip ? -ppp : ppp
  const ty = props.flip ? (props.pageH + props.vy0) * ppp : -props.vy0 * ppp
  const ex = -props.vx0 * ppp
  let d = ''
  if (e.type === 'polyline') {
    d = (e.points ?? []).map((p) => `${p[0]} ${p[1]}`).join(' ')
    if (!d) return null
    return {
      transform: `matrix(${ppp},0,0,${sy},${ex},${ty})`,
      polyline: d,
      d: '',
      fill: 'none',
      stroke: e.stroke ?? '#000',
      width: e.line_width ?? 1,
      fillRule: 'nonzero' as const,
      dash: e.dash ?? null,
    }
  }
  for (const seg of e.segments ?? []) {
    const pts = seg.points ?? []
    if (seg.op === 'm' && pts.length) d += `M ${pts[0][0]} ${pts[0][1]} `
    else if (seg.op === 'l' && pts.length) d += `L ${pts[0][0]} ${pts[0][1]} `
    else if (seg.op === 'c' && pts.length >= 3)
      d += `C ${pts[0][0]} ${pts[0][1]} ${pts[1][0]} ${pts[1][1]} ${pts[2][0]} ${pts[2][1]} `
    else if (seg.op === 'h') d += 'Z '
  }
  if (!d) return null
  return {
    transform: `matrix(${ppp},0,0,${sy},${ex},${ty})`,
    polyline: '',
    d: d.trimEnd(),
    fill: e.fill ?? 'none',
    stroke: e.stroke ?? 'none',
    width: e.line_width ?? 1,
    fillRule: (e.even_odd ? 'evenodd' : 'nonzero') as 'evenodd' | 'nonzero',
    dash: e.dash ?? null,
  }
})

/** shading → CSS 渐变（近似；clip/mask/网格/采样函数不还原）。
 * mesh/func0 直通渐变无色标：有 bbox 画兜底灰，无 bbox 跳过（避免整页误刷） */
const shadingStyle = computed(() => {
  const e = el.value
  if (e.type !== 'shading') return null
  if (!(e.stops ?? []).length && !e.bbox) return null
  const b = e.bbox ?? [props.vx0, props.vy0, props.vx0 + props.pageW, props.vy0 + props.pageH]
  const bw = b[2] - b[0]
  const bh = b[3] - b[1]
  const stops = (e.stops ?? []).length
    ? (e.stops ?? []).map((s) => `${s.color} ${Math.round(s.offset * 100)}%`).join(', ')
    : '#7a7a7a'
  const s: Record<string, string> = {
    left: `${(b[0] - props.vx0) * ppp}px`,
    top: `${(fy(b[1]) - bh) * ppp}px`,
    width: `${bw * ppp}px`,
    height: `${bh * ppp}px`,
    opacity: String(e.alpha ?? 1),
  }
  if (e.kind === 'radial') {
    const cx = ((e.coords?.[0] ?? 0) - b[0]) / Math.max(bw, 1e-6)
    const cy = ((e.coords?.[1] ?? 0) - b[1]) / Math.max(bh, 1e-6)
    s.background = `radial-gradient(circle at ${(cx * 100).toFixed(1)}% ${(cy * 100).toFixed(1)}%, ${stops})`
  } else {
    const x0 = e.coords?.[0] ?? 0
    const y0 = fy(e.coords?.[1] ?? 0)
    const x1 = e.coords?.[2] ?? 0
    const y1 = fy(e.coords?.[3] ?? 0)
    const ang = (Math.atan2(-(y1 - y0), x1 - x0) * 180) / Math.PI + 90
    s.background = `linear-gradient(${ang.toFixed(1)}deg, ${stops})`
  }
  return s
})

/** group 的矩阵映射：transform-origin 固定 0 0（缺省中心会让矩阵绕中心、整体错位） */
const groupStyle = computed(() => {
  const e = el.value
  if (e.type !== 'group') return null
  const m = e.matrix ?? [1, 0, 0, 1, 0, 0]
  return {
    left: '0',
    top: '0',
    width: '100%',
    height: '100%',
    opacity: String(e.alpha ?? 1),
    transformOrigin: '0 0',
    transform: `matrix(${m[0]},${-m[1]},${m[2]},${-m[3]},${(m[4] - props.vx0) * ppp},${fy(m[5]) * ppp})`,
  }
})

/** 悬浮命中盒（向量/渐变/图案等无本体盒的类型；pt → 页面层像素） */
const hitStyle = computed(() => {
  const e = el.value
  if (['text', 'rect', 'image'].includes(e.type)) return null
  const [x0, y0, x1, y1] = pdfElementBBox(e, props.pageW + props.vx0, props.pageH + props.vy0)
  const topPx = props.flip ? props.pageH - (y1 - props.vy0) : y0 - props.vy0
  return {
    left: `${(x0 - props.vx0) * ppp}px`,
    top: `${topPx * ppp}px`,
    width: `${(x1 - x0) * ppp}px`,
    height: `${(y1 - y0) * ppp}px`,
  }
})

/** 图片裁剪路径（设备空间多边形）→ CSS clip-path（坐标相对元素自身盒子） */
const imgClip = computed(() => {
  const e = el.value
  const clip = e.clip
  if (!clip) return null
  const x = e.x ?? 0
  const y = e.y ?? 0
  const h = e.h ?? 0
  const originY = props.flip ? y + h : y
  const pts: string[] = []
  for (const seg of clip) {
    for (const pt of seg.points ?? []) {
      const ly = props.flip ? originY - (pt[1] - props.vy0) : pt[1] - props.vy0 - originY
      pts.push(`${((pt[0] - props.vx0 - x) * ppp).toFixed(1)}px ${(ly * ppp).toFixed(1)}px`)
    }
  }
  return pts.length >= 3 ? `polygon(${pts.join(',')})` : null
})
</script>

<template>
  <!-- 带本体盒的类型：直接渲染，hover 走本体 -->
  <div
    v-if="boxStyle && (el.type === 'text' || el.type === 'rect')"
    class="ov-pdf-el"
    :style="boxStyle"
    @mouseenter="onHover"
    @mousemove="emit('element-move', $event)"
    @mouseleave="emit('element-leave')"
    @mousedown="onSelect"
  >{{ el.type === 'text' ? el.text : '' }}</div>
  <div
    v-else-if="boxStyle && el.type === 'image'"
    class="ov-pdf-el"
    :style="{ ...boxStyle, clipPath: imgClip ?? undefined }"
    @mouseenter="onHover"
    @mousemove="emit('element-move', $event)"
    @mouseleave="emit('element-leave')"
    @mousedown="onSelect"
  >
    <img
      v-if="el.src"
      :src="resolveMedia(el.preview ?? el.src)"
      style="width: 100%; height: 100%; display: block"
      alt=""
      onerror="this.parentElement.style.background='#e8eaed';this.parentElement.style.border='1px dashed #9aa0a6';this.remove()"
    >
  </div>
  <div
    v-else-if="boxStyle"
    class="ov-pdf-el"
    :style="boxStyle"
    @mouseenter="onHover"
    @mousemove="emit('element-move', $event)"
    @mouseleave="emit('element-leave')"
    @mousedown="onSelect"
  ></div>

  <!-- 向量：整页 SVG（不拦截指针）+ 包围盒命中层 -->
  <template v-else-if="vecSvg">
    <svg
      class="ov-pdf-svg"
      :width="`${pageW * ppp}px`"
      :height="`${pageH * ppp}px`"
      :viewBox="`0 0 ${pageW} ${pageH}`"
    >
      <g :transform="vecSvg.transform">
        <polyline
          v-if="vecSvg.polyline"
          :points="vecSvg.polyline"
          fill="none"
          :stroke="vecSvg.stroke"
          :stroke-width="vecSvg.width"
          :stroke-dasharray="vecSvg.dash ? vecSvg.dash.join(',') : undefined"
        />
        <path
          v-else
          :d="vecSvg.d"
          :fill="vecSvg.fill"
          :stroke="vecSvg.stroke"
          :stroke-width="vecSvg.width"
          :fill-rule="vecSvg.fillRule"
          :stroke-dasharray="vecSvg.dash ? vecSvg.dash.join(',') : undefined"
        />
      </g>
    </svg>
    <div
      class="ov-pdf-hit"
      :style="hitStyle ?? undefined"
      @mouseenter="onHover"
      @mousemove="emit('element-move', $event)"
      @mouseleave="emit('element-leave')"
      @mousedown="onSelect"
    ></div>
  </template>

  <!-- 渐变 -->
  <div
    v-else-if="shadingStyle"
    class="ov-pdf-el"
    :style="shadingStyle"
    @mouseenter="onHover"
    @mousemove="emit('element-move', $event)"
    @mouseleave="emit('element-leave')"
    @mousedown="onSelect"
  ></div>

  <!-- 透明组：矩阵映射（origin 0 0）+ 递归子元素（raw 坐标） -->
  <div v-else-if="groupStyle" class="ov-pdf-el" :style="groupStyle">
    <PdfElementView
      v-for="(c, i) in pdfChildren(el) ?? []"
      :key="i"
      :el="c"
      :page-w="pageW"
      :page-h="pageH"
      :vx0="0"
      :vy0="0"
      :flip="false"
      :path="`${path}/${c.type}[${i + 1}]`"
      @element-hover="(ev, p) => emit('element-hover', ev, p)"
      @element-move="emit('element-move', $event)"
      @element-leave="emit('element-leave')"
      @element-mousedown="(ev, p) => emit('element-mousedown', ev, p)"
    />
    <!-- 组整体命中（子元素命中优先级高于它，由 DOM 顺序保证） -->
    <div
      class="ov-pdf-hit"
      style="inset: 0"
      @mouseenter="onHover"
      @mousemove="emit('element-move', $event)"
      @mouseleave="emit('element-leave')"
      @mousedown="onSelect"
    ></div>
  </div>
</template>

<style>
.ov-pdf-el {
  position: absolute;
}
.ov-pdf-svg {
  position: absolute;
  left: 0;
  top: 0;
  pointer-events: none;
  overflow: visible;
}
.ov-pdf-hit {
  position: absolute;
  pointer-events: auto;
}
</style>
