/** 请求草稿转换与构建：持久化和网络载荷分开，临时认证不落盘。 */
import type {
  ApiKind,
  ApiRecord,
  ApiSavePayload,
  BodyMode,
  HttpMethod,
  HttpRequestPayload,
  RequestAuth,
} from './contracts'
import { mergeQuery, newKvId, type KvRow } from './useHttp'

export interface RequestDraft {
  type: ApiKind
  method: HttpMethod
  url: string
  params: KvRow[]
  headers: KvRow[]
  bodyMode: BodyMode
  body: string
  form: KvRow[]
  timeoutMs: number
  auth: RequestAuth
}
export function emptyRow(): KvRow {
  return { id: newKvId(), key: '', value: '', enabled: true }
}
export function newDraft(type: ApiKind): RequestDraft {
  return {
    type,
    method: 'GET',
    url: '',
    params: [emptyRow()],
    headers: [emptyRow()],
    bodyMode: 'none',
    body: '',
    form: [emptyRow()],
    timeoutMs: 15000,
    auth: { mode: 'none', credentialId: '', username: '', secret: '' },
  }
}
function rows(value: string): KvRow[] {
  const parsed: unknown = JSON.parse(value || '[]')
  if (
    !Array.isArray(parsed) ||
    parsed.some((r) => !r || typeof (r.key ?? r.name) !== 'string' || typeof r.value !== 'string')
  )
    throw new Error('接口键值数据损坏，无法打开')
  return parsed.length
    ? parsed.map((r) => ({
        id: newKvId(),
        key: r.key ?? r.name,
        value: r.value,
        enabled: r.enabled !== false,
      }))
    : [emptyRow()]
}
export function fromRecord(record: ApiRecord): RequestDraft {
  const draft = newDraft(record.type)
  const options = JSON.parse(record.options || '{}')
  if (!options || typeof options !== 'object') throw new Error('接口设置数据损坏')
  draft.method = (record.type === 'ws' ? 'GET' : record.method || 'GET') as HttpMethod
  draft.url = record.url
  draft.params = rows(record.params)
  draft.headers = rows(record.headers)
  if (!['none', 'json', 'text', 'form', 'raw'].includes(record.bodyMode))
    throw new Error('此接口的请求体格式暂不支持')
  draft.bodyMode = record.bodyMode === 'raw' ? 'text' : (record.bodyMode as BodyMode)
  draft.body = record.body
  draft.form = record.bodyMode === 'form' ? rows(record.body) : [emptyRow()]
  if (Number.isFinite(options.timeoutMs) && options.timeoutMs > 0)
    draft.timeoutMs = options.timeoutMs
  if (options.auth && ['none', 'basic', 'bearer'].includes(options.auth.mode))
    draft.auth = {
      mode: options.auth.mode,
      credentialId: typeof options.auth.credentialId === 'string' ? options.auth.credentialId : '',
      username: '',
      secret: '',
    }
  return draft
}
export function toRecord(
  draft: RequestDraft,
  name: string,
  groupName: string,
  id = 0
): ApiSavePayload {
  return {
    id,
    kind: draft.type,
    name: name.trim(),
    groupName: groupName.trim(),
    method: draft.type === 'ws' ? 'WEBSOCKET' : draft.method,
    url: draft.url.trim(),
    params: JSON.stringify(draft.params),
    headers: JSON.stringify(draft.headers),
    bodyMode: draft.bodyMode,
    body: draft.bodyMode === 'form' ? JSON.stringify(draft.form) : draft.body,
    options: JSON.stringify({
      timeoutMs: draft.timeoutMs,
      auth: { mode: draft.auth.mode, credentialId: draft.auth.credentialId },
    }),
  }
}
/** 行 ID 不参与脏标记；刷新/重载产生的新 ID 不应产生伪修改。 */
export function fingerprint(draft: RequestDraft): string {
  return JSON.stringify(draft, (key, value) => (key === 'id' ? undefined : value))
}
export function requestPayload(draft: RequestDraft): HttpRequestPayload {
  const query = draft.params
    .filter((r) => r.enabled !== false && r.key.trim())
    .map((r) => `${encodeURIComponent(r.key.trim())}=${encodeURIComponent(r.value)}`)
    .join('&')
  const url = mergeQuery(draft.url.trim(), query)
  let parsed: URL
  try {
    parsed = new URL(url)
  } catch {
    throw new Error('请输入完整的请求地址')
  }
  const protocols = draft.type === 'ws' ? ['ws:', 'wss:'] : ['http:', 'https:']
  if (!protocols.includes(parsed.protocol)) throw new Error(`地址需使用 ${protocols.join(' / ')}`)
  const headers: [string, string][] = draft.headers
    .filter((r) => r.enabled !== false && r.key.trim())
    .map((r) => [r.key.trim(), r.value])
  let body: string | undefined
  if (draft.type !== 'ws' && draft.bodyMode !== 'none') {
    body =
      draft.bodyMode === 'form'
        ? draft.form
            .filter((r) => r.enabled !== false && r.key.trim())
            .map((r) => `${encodeURIComponent(r.key.trim())}=${encodeURIComponent(r.value)}`)
            .join('&')
        : draft.body
    if (draft.bodyMode === 'json') {
      try {
        JSON.parse(body)
      } catch {
        throw new Error('JSON 请求体无效，请检查语法')
      }
    }
    const contentType = {
      json: 'application/json',
      text: 'text/plain; charset=utf-8',
      form: 'application/x-www-form-urlencoded',
    }[draft.bodyMode]
    if (!headers.some(([key]) => key.toLowerCase() === 'content-type'))
      headers.push(['Content-Type', contentType])
  }
  if (draft.type === 'sse' && !headers.some(([key]) => key.toLowerCase() === 'accept'))
    headers.push(['Accept', 'text/event-stream'])
  if (draft.auth.mode !== 'none' && !draft.auth.credentialId && !draft.auth.secret)
    throw new Error('请选择凭证或填写临时认证信息')
  return {
    method: draft.method,
    url,
    headers,
    body,
    timeoutMs: draft.timeoutMs,
    auth: { ...draft.auth },
  }
}
