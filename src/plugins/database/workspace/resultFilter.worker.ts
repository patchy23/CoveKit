/** 只接收当前工作批次，完整单元格保持原 toLowerCase/includes 语义。 */
self.onmessage = (event: MessageEvent<{ id: number; rows: string[][]; term: string }>) => {
  const { id, rows, term } = event.data
  const matches: number[] = []
  rows.forEach((row, index) => {
    if (row.some((cell) => cell.toLowerCase().includes(term))) matches.push(index)
  })
  self.postMessage({ id, matches })
}

export {}
