<script setup lang="ts">
// 表格元素渲染：HTML table + colspan/rowspan
import { computed } from 'vue'
import type { TableElement, TableCell } from '@/types/presentation'
import { inchToPx, ptToPx, withHash } from '@/utils/convert'

const props = defineProps<{ el: TableElement }>()

const pos = computed(() => props.el.position)

/** 物理列数（column_widths 长度或 colspan 之和最大值） */
const physicalCols = computed(() => {
  if (props.el.column_widths && props.el.column_widths.length > 0) {
    return props.el.column_widths.length
  }
  return Math.max(
    0,
    ...props.el.rows.map((row) =>
      row.reduce((sum, c) => sum + (c.colspan ?? 1), 0),
    ),
  )
})

/** 列宽比例（%） */
const colWidthPct = computed(() => {
  const widths = props.el.column_widths
  const total = widths ? widths.reduce((a, b) => a + b, 0) : physicalCols.value
  if (!widths || total <= 0) return Array.from({ length: physicalCols.value }, () => 100 / Math.max(1, physicalCols.value))
  return widths.map((w) => (w / total) * 100)
})

/** 逻辑网格 → 物理网格：vMerge 占位补全 */
interface PhysCell {
  cell: TableCell
  colspan: number
  rowspan: number
  vmerge: boolean
}

const physRows = computed<PhysCell[][]>(() => {
  const numCols = physicalCols.value
  const cover: number[] = Array(numCols).fill(0)
  const out: PhysCell[][] = []
  for (const row of props.el.rows) {
    const cells: PhysCell[] = []
    let col = 0
    for (const cell of row) {
      while (col < numCols && cover[col] > 0) {
        cover[col] -= 1
        cells.push({ cell: { text: '' }, colspan: 1, rowspan: 1, vmerge: true })
        col += 1
      }
      const colspan = Math.max(1, cell.colspan ?? 1)
      const rowspan = Math.max(1, cell.rowspan ?? 1)
      if (col + colspan > numCols) break
      cells.push({ cell, colspan, rowspan, vmerge: false })
      if (rowspan > 1) {
        for (let cc = col; cc < Math.min(col + colspan, numCols); cc += 1) {
          cover[cc] = Math.max(cover[cc], rowspan - 1)
        }
      }
      col += colspan
    }
    while (col < numCols) {
      if (cover[col] > 0) {
        cover[col] -= 1
        cells.push({ cell: { text: '' }, colspan: 1, rowspan: 1, vmerge: true })
      } else {
        cells.push({ cell: { text: '' }, colspan: 1, rowspan: 1, vmerge: false })
      }
      col += 1
    }
    out.push(cells)
  }
  return out
})

function cellStyle(c: PhysCell): Record<string, string> {
  const cell = c.cell
  const parts: Record<string, string> = {}
  if (cell.fill) parts.background = withHash(cell.fill)
  if (cell.bold) parts.fontWeight = 'bold'
  if (cell.color) parts.color = withHash(cell.color)
  if (cell.align) parts.textAlign = cell.align
  parts.fontSize = `${ptToPx(cell.font_size ?? props.el.font_size ?? 14)}px`
  return parts
}

const wrapStyle = computed(() => ({
  position: 'absolute' as const,
  left: `${inchToPx(pos.value.x)}px`,
  top: `${inchToPx(pos.value.y)}px`,
  width: `${inchToPx(pos.value.w)}px`,
  height: `${inchToPx(pos.value.h)}px`,
  overflow: 'hidden' as const,
}))
</script>

<template>
  <div class="el el-table" :style="wrapStyle">
    <table style="width: 100%; height: 100%; border-collapse: collapse">
      <colgroup>
        <col v-for="(w, i) in colWidthPct" :key="i" :style="{ width: `${w}%` }" />
      </colgroup>
      <tbody>
        <tr v-for="(row, ri) in physRows" :key="ri">
          <td
            v-for="(c, ci) in row"
            :key="ci"
            :colspan="c.vmerge ? 1 : c.colspan"
            :rowspan="c.vmerge ? 1 : c.rowspan"
            :style="cellStyle(c)"
          >
            <template v-if="!c.vmerge">{{ c.cell.text }}</template>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<style scoped>
.el-table :deep(td) {
  border: 1px solid #cccccc;
  padding: 2px 6px;
  vertical-align: middle;
}
</style>
