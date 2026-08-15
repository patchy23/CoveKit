/**
 * Vault 凭证管理 · 纯函数逻辑（框架级，供凭证管理页 / CredentialForm / CredentialPicker 共用）
 * 契约类型来自 @/core/ipc/contracts（唯一事实源）；本文件只做客户端展示与表单建模。
 */
import type {
  Credential,
  CredentialFields,
  CredentialKind,
  CredentialSavePayload,
  CredentialSummary,
  CustomEntry,
} from '@/core/ipc/contracts'
import type { UiTone } from '@/core/ui/types'

/** 全部凭证类型（固定顺序，与后端枚举一致） */
export const CREDENTIAL_KINDS: CredentialKind[] = [
  'password',
  'ssh-key',
  'api-token',
  'access-key-pair',
  'custom',
]

/** 类型中文显示名 */
export const KIND_LABEL: Record<CredentialKind, string> = {
  password: '用户名密码',
  'ssh-key': 'SSH 私钥',
  'api-token': 'API Token',
  'access-key-pair': 'AccessKey 对',
  custom: '自定义',
}

/** 类型徽标色调（UiBadge tone） */
export const KIND_TONE: Record<CredentialKind, UiTone> = {
  password: 'info',
  'ssh-key': 'purple',
  'api-token': 'accent',
  'access-key-pair': 'warning',
  custom: 'neutral',
}

/** 列表筛选 + 搜索（名称 / 备注 / 掩码摘要，大小写不敏感） */
export function filterCredentials(
  list: CredentialSummary[],
  kind: CredentialKind | 'all',
  query: string
): CredentialSummary[] {
  const q = query.trim().toLowerCase()
  return list.filter((c) => {
    if (kind !== 'all' && c.kind !== kind) return false
    if (!q) return true
    return (
      c.name.toLowerCase().includes(q) ||
      c.note.toLowerCase().includes(q) ||
      c.masked.toLowerCase().includes(q)
    )
  })
}

/** 秒级时间戳 → 本地 `YYYY-MM-DD HH:mm` */
export function formatTimestamp(seconds: number): string {
  const d = new Date(seconds * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

// ── 动态表单建模（按 kind 渲染，秘密字段眼睛切换）──

/** 表单字段描述（custom 类型不走字段表，用键值条目编辑器） */
export interface FormFieldDef {
  /** CredentialFields 变体上的字段 key */
  key: string
  /** 表单 label */
  label: string
  /** 秘密字段（password 输入 + 眼睛切换） */
  secret: boolean
  /** 多行文本（私钥等大段内容） */
  multiline?: boolean
  /** 可选（留空 = 不填） */
  optional?: boolean
}

/** 按类型返回字段表（决定编辑表单的动态渲染） */
export function formFieldsFor(kind: CredentialKind): FormFieldDef[] {
  switch (kind) {
    case 'password':
      return [
        { key: 'username', label: '用户名', secret: false },
        { key: 'password', label: '密码', secret: true },
      ]
    case 'ssh-key':
      return [
        { key: 'username', label: '用户名', secret: false },
        { key: 'privateKey', label: '私钥', secret: true, multiline: true },
        { key: 'passphrase', label: '私钥口令', secret: true, optional: true },
      ]
    case 'api-token':
      return [{ key: 'token', label: 'Token', secret: true }]
    case 'access-key-pair':
      return [
        { key: 'accessKeyId', label: 'AccessKey ID', secret: false },
        { key: 'accessKeySecret', label: 'AccessKey Secret', secret: true },
      ]
    case 'custom':
      return []
  }
}

/** 编辑表单状态（custom 走 entries，其余走 values） */
export interface CredentialFormState {
  /** 显示名 */
  name: string
  /** 类型 */
  kind: CredentialKind
  /** 备注 */
  note: string
  /** 字段值表（key = FormFieldDef.key） */
  values: Record<string, string>
  /** 自定义键值条目（kind = custom） */
  entries: CustomEntry[]
}

/** 空白表单（新建；默认 password 类型） */
export function emptyFormState(kind: CredentialKind = 'password'): CredentialFormState {
  const values: Record<string, string> = {}
  for (const f of formFieldsFor(kind)) values[f.key] = ''
  return { name: '', kind, note: '', values, entries: [{ key: '', value: '', secret: true }] }
}

/** 从已有凭证（reveal 明文）构建编辑表单 */
export function formStateFromCredential(credential: Credential): CredentialFormState {
  const state = emptyFormState(credential.kind)
  state.name = credential.name
  state.note = credential.note
  if (credential.fields.type === 'custom') {
    state.entries = credential.fields.entries.map((e) => ({ ...e }))
    if (!state.entries.length) state.entries = [{ key: '', value: '', secret: true }]
  } else {
    for (const [k, v] of Object.entries(credential.fields)) {
      if (k !== 'type' && typeof v === 'string') state.values[k] = v
    }
  }
  return state
}

/** 表单校验（返回错误文案，null = 通过；与后端 validate_payload 同规则） */
export function validateFormState(state: CredentialFormState): string | null {
  if (!state.name.trim()) return '凭证名称不能为空'
  if (state.kind === 'custom') {
    const filled = state.entries.filter((e) => e.key.trim() || e.value)
    if (!filled.length) return '自定义凭证至少需要一个字段'
    if (filled.some((e) => !e.key.trim())) return '自定义字段的键名不能为空'
    return null
  }
  for (const f of formFieldsFor(state.kind)) {
    const v = (state.values[f.key] ?? '').trim()
    if (!f.optional && !v) return `${f.label}不能为空`
  }
  return null
}

/** 表单状态 → fields tagged union（custom 过滤空行；可选字段空串转 null） */
export function fieldsFromFormState(state: CredentialFormState): CredentialFields {
  switch (state.kind) {
    case 'password':
      return {
        type: 'password',
        username: state.values.username?.trim() ?? '',
        password: state.values.password ?? '',
      }
    case 'ssh-key':
      return {
        type: 'ssh-key',
        username: state.values.username?.trim() ?? '',
        privateKey: state.values.privateKey ?? '',
        passphrase: state.values.passphrase?.trim() ? state.values.passphrase : null,
      }
    case 'api-token':
      return { type: 'api-token', token: state.values.token ?? '' }
    case 'access-key-pair':
      return {
        type: 'access-key-pair',
        accessKeyId: state.values.accessKeyId?.trim() ?? '',
        accessKeySecret: state.values.accessKeySecret ?? '',
      }
    case 'custom':
      return {
        type: 'custom',
        entries: state.entries
          .filter((e) => e.key.trim() || e.value)
          .map((e) => ({ key: e.key.trim(), value: e.value, secret: e.secret })),
      }
  }
}

/** 表单状态 → vault_save 入参（调用前须 validateFormState 通过） */
export function payloadFromFormState(
  state: CredentialFormState,
  id: string | null = null
): CredentialSavePayload {
  return {
    id,
    name: state.name.trim(),
    kind: state.kind,
    fields: fieldsFromFormState(state),
    note: state.note.trim(),
  }
}

/** 「复制值」取各类型的主秘密（右键复制 / 未来引用方展示用） */
export function primarySecret(credential: Credential): string {
  switch (credential.fields.type) {
    case 'password':
      return credential.fields.password
    case 'ssh-key':
      return credential.fields.privateKey
    case 'api-token':
      return credential.fields.token
    case 'access-key-pair':
      return credential.fields.accessKeySecret
    case 'custom':
      // 优先取第一个标记为秘密的条目，否则第一个条目
      return (
        credential.fields.entries.find((e) => e.secret)?.value ??
        credential.fields.entries[0]?.value ??
        ''
      )
  }
}
