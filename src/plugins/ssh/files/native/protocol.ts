import type { ServerConnection } from '../../contracts'
import type { RemoteDocument } from '../useRemoteEditor'
export interface EditorViewport {
  from: number
  to: number
  top: number
  left: number
}
export interface EditorLayout {
  sidebar: boolean
  width: number
  documents: Record<string, EditorViewport>
}
export interface EditorSnapshot {
  sequence: number
  documents: RemoteDocument[]
  active: string
  directory: string
  error: string
  layout?: EditorLayout
}
export interface EditorInitial {
  snapshot: EditorSnapshot
  connection: ServerConnection
  title: string
}
export interface EditorEnvelope {
  id: string
  reply?: boolean
  type: string
  value?: unknown
  error?: string
}
/** 过期定时同步不能覆盖移回时的最终快照。 */
export function acceptSnapshot(current: number, next: EditorSnapshot) {
  return Number.isSafeInteger(next.sequence) && next.sequence > current
}

/** 平时只同步改变的文档；完整快照仅用于握手与交接。 */
export interface EditorUpdate extends Omit<EditorSnapshot, 'layout'> {
  ids: string[]
}
