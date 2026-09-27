import { afterEach, expect, it, vi } from 'vitest'

afterEach(() => vi.unstubAllGlobals())

it('线程算法保留 Unicode 大小写上下文和逐单元格匹配语义', async () => {
  const worker = {
    onmessage: undefined as undefined | ((event: { data: unknown }) => void),
    postMessage: vi.fn(),
  }
  vi.stubGlobal('self', worker)
  await import('./resultFilter.worker')
  const rows = [['ΟΣ'], ['ΟΣΑ'], ['İ中文🙂'], ['ab', 'cd'], [''], ['x'.repeat(1000000) + 'END']]
  for (const term of ['ος', 'οσα', 'i̇', '中文🙂', 'bc', '', 'end']) {
    worker.onmessage!({ data: { id: 1, rows, term } })
    expect(worker.postMessage).toHaveBeenLastCalledWith({
      id: 1,
      matches: rows.flatMap((row, i) =>
        row.some((cell) => cell.toLowerCase().includes(term)) ? [i] : []
      ),
    })
  }
})
