import { Change, Chunk } from '@codemirror/merge'
import { Text } from '@codemirror/state'
import { diffStats } from './diff'
import type { DiffStats } from './types'

export interface DiffRequest {
  original: string
  modified: string
  versionA: number
  versionB: number
}
export interface DiffResult {
  versionA: number
  versionB: number
  chunks: ReadonlyArray<{
    fromA: number
    toA: number
    fromB: number
    toB: number
    precise: boolean
    changes: ReadonlyArray<{ fromA: number; toA: number; fromB: number; toB: number }>
  }>
  stats: DiffStats
}

/** 后台与直接路径共用同一差异算法、默认扫描预算和统计口径。 */
export function computeDiff(request: DiffRequest): DiffResult {
  const { original, modified, versionA, versionB } = request
  return {
    versionA,
    versionB,
    chunks: Chunk.build(Text.of(original.split('\n')), Text.of(modified.split('\n')), {
      scanLimit: 500,
    }),
    stats: diffStats(original, modified),
  }
}

/** structured clone 不保留 Chunk/Change 原型，必须恢复其坐标与方法语义。 */
export function restoreChunks(result: DiffResult): Chunk[] {
  return result.chunks.map(
    (chunk) =>
      new Chunk(
        chunk.changes.map(
          (change) => new Change(change.fromA, change.toA, change.fromB, change.toB)
        ),
        chunk.fromA,
        chunk.toA,
        chunk.fromB,
        chunk.toB,
        chunk.precise
      )
  )
}
