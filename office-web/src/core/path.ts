//! CLI 路径构造：与三个 CLI 的寻址语法严格一致，用于悬浮路径卡片与回写定位
//!
//! - pptx：`/slide[N]/text[2]`、分组内 `/slide[1]/group[1]/shape[2]`、稳定 ID `/slide[1]/text[@id=5]`
//! - xlsx：`/sheet[1]/cell[B2]`、`/sheet[1]/row[3]`、`/sheet[1]/col[C]`、`/sheet[1]/chart[1]`
//! - docx：`/part[1]/paragraph[3]`、表格 `/part[1]/table[1]/row[2]/cell[1]/paragraph[1]`

/** 1-based 列号 → 列字母（1 → A, 27 → AA） */
export function colLetter(index: number): string {
  let n = index
  let s = ''
  while (n > 0) {
    const rem = (n - 1) % 26
    s = String.fromCharCode(65 + rem) + s
    n = Math.floor((n - 1) / 26)
  }
  return s
}

/** 单元格引用（列号 + 行号，均 1-based） */
export function cellRef(col: number, row: number): string {
  return `${colLetter(col)}${row}`
}

/** xlsx：工作表段 */
export function sheetPath(sheetIndex: number, name?: string): string {
  return name ? `/sheet[${sheetIndex + 1}]` : `/sheet[${sheetIndex + 1}]`
}

/** xlsx：单元格路径 */
export function xlsxCellPath(sheetIndex: number, col: number, row: number): string {
  return `/sheet[${sheetIndex + 1}]/cell[${cellRef(col, row)}]`
}

/** pptx：数字索引链 → CLI 路径（与 ai-ppt/web 的 elementCliPath 同构） */
export function pptxElementPath(
  slideIndex: number,
  chain: number[],
  containerTypes: string[][],
): string {
  const nodes: string[] = [`slide[${slideIndex + 1}]`]
  chain.forEach((idx, depth) => {
    const types = containerTypes[depth] ?? []
    const type = types[idx]
    if (!type) return
    let k = 0
    for (let j = 0; j <= idx; j += 1) if (types[j] === type) k += 1
    nodes.push(`${type}[${k}]`)
  })
  return `/${nodes.join('/')}`
}

/** docx：块路径（容器链由调用方按 类型[序号] 拼好） */
export function docxPath(partIndex: number, segments: string[]): string {
  return `/part[${partIndex + 1}]${segments.map((s) => `/${s}`).join('')}`
}

/** pdf：数字索引链 → CLI 路径（/page[N]/text[2]，首段为 page） */
export function pdfElementPath(
  pageIndex: number,
  chain: number[],
  containerTypes: string[][],
): string {
  const nodes: string[] = [`page[${pageIndex + 1}]`]
  chain.forEach((idx, depth) => {
    const types = containerTypes[depth] ?? []
    const type = types[idx]
    if (!type) return
    let k = 0
    for (let j = 0; j <= idx; j += 1) if (types[j] === type) k += 1
    nodes.push(`${type}[${k}]`)
  })
  return `/${nodes.join('/')}`
}

/** 在同类型元素中计算 1-based 序号 */
export function typeOrdinal(types: string[], index: number): number {
  const t = types[index]
  let k = 0
  for (let i = 0; i <= index; i += 1) if (types[i] === t) k += 1
  return k
}
