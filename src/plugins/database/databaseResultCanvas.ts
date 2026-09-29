export const ROW_HEIGHT = 25
export const HEADER_HEIGHT = 40
export const ROW_NUMBER_WIDTH = 44

export interface GridCellPosition {
  row: number
  column: number
}

export interface GridTheme {
  background: string
  foreground: string
  muted: string
  border: string
  selected: string
  dirty: string
  font: string
}

export interface DrawGridOptions {
  width: number
  height: number
  scrollLeft: number
  scrollTop: number
  widths: number[]
  rowCount: number
  text: (row: number, column: number) => string
  rowLabel: (row: number) => string
  selected?: GridCellPosition
  dirty: (row: number, column: number) => boolean
  theme: GridTheme
}

export interface GridColumns {
  /** 每列内容区的起点；第一个值从行号列右侧开始。 */
  offsets: number[]
  totalWidth: number
}

export interface GridVisibleRange {
  start: number
  end: number
}

/** 行号列固定在左侧，其他列按当前宽度顺序横向排列。 */
export function gridColumns(widths: number[]): GridColumns {
  const offsets = new Array<number>(widths.length + 1)
  offsets[0] = ROW_NUMBER_WIDTH
  for (let column = 0; column < widths.length; column += 1) {
    offsets[column + 1] = offsets[column] + Math.max(1, widths[column])
  }
  return { offsets, totalWidth: offsets.at(-1) ?? ROW_NUMBER_WIDTH }
}

/** `y` 是包含表头的滚动视口坐标；行号列使用 column=-1。 */
export function hitGridCell(
  x: number,
  y: number,
  scrollLeft: number,
  scrollTop: number,
  widths: number[],
  rowCount: number
): GridCellPosition | null {
  if (x < 0 || y < HEADER_HEIGHT || y < 0) return null
  const row = Math.floor((scrollTop + y - HEADER_HEIGHT) / ROW_HEIGHT)
  if (row < 0 || row >= rowCount) return null
  if (x < ROW_NUMBER_WIDTH) return { row, column: -1 }
  const { offsets } = gridColumns(widths)
  const contentX = x + scrollLeft
  let low = 0
  let high = widths.length
  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    if (offsets[middle + 1] <= contentX) low = middle + 1
    else high = middle
  }
  return low < widths.length && contentX >= offsets[low] ? { row, column: low } : null
}

/** 返回与视口相交的行，end 不包含在结果内。 */
export function visibleGridRange(
  scrollTop: number,
  height: number,
  rowCount: number
): GridVisibleRange {
  const count = Math.max(0, Math.floor(rowCount))
  const start = Math.min(count, Math.max(0, Math.floor(Math.max(0, scrollTop) / ROW_HEIGHT)))
  const end = Math.min(
    count,
    Math.max(start, Math.ceil((Math.max(0, scrollTop) + Math.max(0, height)) / ROW_HEIGHT))
  )
  return { start, end }
}

/** 先限制内容长度，再按像素宽度裁切，避免画布持有或绘制过长文本。 */
export function fitGridText(
  ctx: CanvasRenderingContext2D,
  value: string,
  maxWidth: number
): string {
  if (maxWidth <= 0) return ''
  const text = previewGridText(value)
  if (ctx.measureText(text).width <= maxWidth) return text
  const ellipsis = '…'
  const ellipsisWidth = ctx.measureText(ellipsis).width
  if (ellipsisWidth > maxWidth) return ''
  let low = 0
  let high = text.length
  while (low < high) {
    const middle = Math.ceil((low + high) / 2)
    if (ctx.measureText(text.slice(0, middle)).width + ellipsisWidth <= maxWidth) low = middle
    else high = middle - 1
  }
  let end = low
  const last = text.charCodeAt(end - 1)
  if (last >= 0xd800 && last <= 0xdbff) end -= 1
  return text.slice(0, end) + ellipsis
}

function previewGridText(value: string): string {
  if (value.length <= 500) return value
  let end = 499
  const last = value.charCodeAt(end - 1)
  if (last >= 0xd800 && last <= 0xdbff) end -= 1
  return value.slice(0, end) + '…'
}

/** 只访问视口中可见的行和列，行数据通过回调按需读取，不复制结果矩阵。 */
export function drawGrid(ctx: CanvasRenderingContext2D, options: DrawGridOptions): void {
  const width = Math.max(0, options.width)
  const height = Math.max(0, options.height)
  if (!width || !height) return

  const { offsets, totalWidth } = gridColumns(options.widths)
  const scrollLeft = Math.max(0, options.scrollLeft)
  const scrollTop = Math.max(0, options.scrollTop)
  const columns = options.widths.length
  const rows = Math.max(0, Math.floor(options.rowCount))
  const visibleRows = visibleGridRange(scrollTop, height, rows)
  const firstContentX = scrollLeft + ROW_NUMBER_WIDTH
  const lastContentX = scrollLeft + width
  let low = 0
  let high = columns
  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    if (offsets[middle + 1] <= firstContentX) low = middle + 1
    else high = middle
  }
  const firstColumn = low
  low = firstColumn
  high = columns
  while (low < high) {
    const middle = Math.floor((low + high) / 2)
    if (offsets[middle] < lastContentX) low = middle + 1
    else high = middle
  }
  const lastColumn = low

  ctx.save()
  ctx.font = options.theme.font
  ctx.textBaseline = 'middle'
  ctx.clearRect(0, 0, width, height)
  ctx.fillStyle = options.theme.background
  ctx.fillRect(0, 0, width, height)

  for (let row = visibleRows.start; row < visibleRows.end; row += 1) {
    const y = row * ROW_HEIGHT - scrollTop
    const selectedRow = options.selected?.row === row
    const rowFill = selectedRow ? options.theme.selected : options.theme.background
    ctx.fillStyle = rowFill
    ctx.fillRect(0, y, Math.min(width, ROW_NUMBER_WIDTH), ROW_HEIGHT)
    ctx.fillStyle = options.theme.muted
    ctx.textAlign = 'center'
    ctx.fillText(
      fitGridText(ctx, options.rowLabel(row), ROW_NUMBER_WIDTH - 8),
      ROW_NUMBER_WIDTH / 2,
      y + ROW_HEIGHT / 2
    )

    ctx.save()
    ctx.beginPath()
    ctx.rect(ROW_NUMBER_WIDTH, 0, Math.max(0, width - ROW_NUMBER_WIDTH), height)
    ctx.clip()
    for (let column = firstColumn; column < lastColumn; column += 1) {
      const cellX = offsets[column] - scrollLeft
      const cellWidth = options.widths[column]
      const selectedCell = options.selected?.row === row && options.selected.column === column
      const isDirty = options.dirty(row, column)
      ctx.fillStyle = selectedCell
        ? options.theme.selected
        : isDirty
          ? options.theme.dirty
          : rowFill
      ctx.fillRect(cellX, y, cellWidth, ROW_HEIGHT)
      ctx.fillStyle = options.theme.border
      ctx.fillRect(cellX + cellWidth - 1, y, 1, ROW_HEIGHT)
      const rawText = options.text(row, column)
      ctx.fillStyle = options.theme.foreground
      ctx.textAlign = 'left'
      ctx.fillText(fitGridText(ctx, rawText, cellWidth - 14), cellX + 7, y + ROW_HEIGHT / 2)
      if (selectedCell) {
        ctx.strokeStyle = options.theme.border
        ctx.strokeRect(cellX + 0.5, y + 0.5, Math.max(0, cellWidth - 1), ROW_HEIGHT - 1)
      }
    }
    ctx.restore()

    ctx.fillStyle = options.theme.border
    ctx.fillRect(0, y + ROW_HEIGHT - 1, Math.min(width, totalWidth), 1)
  }
  ctx.restore()
}
