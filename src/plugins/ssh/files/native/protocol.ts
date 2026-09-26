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
export function acceptSnapshot(current: number, next: Pick<EditorSnapshot, 'sequence'>) {
  return Number.isSafeInteger(next.sequence) && next.sequence > current
}

type DocumentFields = Omit<RemoteDocument, 'id'>
export type EditorDocumentUpdate =
  | { id: string; document: RemoteDocument }
  | { id: string; changes: Partial<DocumentFields>; cleared: (keyof DocumentFields)[] }

/** 平时只同步变化字段；新文档、失步恢复和交接保留完整快照。 */
export interface EditorUpdate extends Omit<EditorSnapshot, 'layout' | 'documents'> {
  baseSequence?: number
  documents: EditorDocumentUpdate[]
  ids: string[]
}
