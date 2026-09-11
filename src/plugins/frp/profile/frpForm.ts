/**
 * frp 表单模型 ⇄ TOML 解析结果（parsed）的互相转换（纯函数，带单测）
 *
 * 设计红线（任务书 §6.2）：**未覆盖字段与未知段落必须原样保留**——
 * 表单只负责「读出来的已知字段」，写回时以原 parsed 为基底做字段级覆盖，
 * 绝不重建对象，避免把用户手写的 `healthCheck`、`metadatas`、自定义段落改没。
 */
import type { FrpTemplateId } from '../contracts'

/** 表单里的单个代理条目 */
export interface FrpFormProxy {
  /** 代理名（frp 要求全局唯一；为空视为未填写） */
  name: string
  /** 代理类型：tcp / udp / http / https / stcp */
  type: string
  /** 本地 IP（空串表示未填写，由 frpc 用默认值） */
  localIP: string
  /** 本地端口（null 表示未填写） */
  localPort: number | null
  /** 远端端口（null 表示未填写；http/https/stcp 不适用） */
  remotePort: number | null
  /** 自定义域名（UI 用逗号分隔的字符串，写回时拆成数组） */
  customDomains: string
  /** stcp/xtcp 的密钥 */
  secretKey: string
  /** 是否启用（对应 v0.66.0 起的 proxy.enabled） */
  enabled: boolean
}

/** 表单模式覆盖的字段集合（其余字段一律透传） */
export interface FrpFormModel {
  serverAddr: string
  serverPort: number | null
  user: string
  authMethod: string
  authToken: string
  logLevel: string
  protocol: string
  tlsEnable: boolean
  tlsServerName: string
  poolCount: number | null
  proxies: FrpFormProxy[]
}

/** 传输协议可选值 */
export const PROTOCOL_OPTIONS = ['tcp', 'kcp', 'quic', 'websocket', 'wss']
/** 日志等级可选值 */
export const LOG_LEVEL_OPTIONS = ['trace', 'debug', 'info', 'warn', 'error']
/** 代理类型可选值（v1 表单支持；xtcp 等高级类型走源码模式） */
export const PROXY_TYPE_OPTIONS = ['tcp', 'udp', 'http', 'https', 'stcp']
/** 需要远端端口的代理类型 */
export const NEEDS_REMOTE_PORT = ['tcp', 'udp']
/** 需要自定义域名的代理类型 */
export const NEEDS_CUSTOM_DOMAINS = ['http', 'https']
/** 需要密钥的代理类型 */
export const NEEDS_SECRET_KEY = ['stcp']

/** 认证方式可选值 */
export const AUTH_METHOD_OPTIONS = ['token', 'oidc']

/** 新建模板（id 与 Rust `frp_profile_create` 的 template 参数一致） */
export const TEMPLATE_IDS: FrpTemplateId[] = ['tcp', 'http', 'stcp', 'empty']

/** 空表单模型（默认值对齐 frp 官方默认：端口 7000、token 认证、info 日志） */
export function emptyFormModel(): FrpFormModel {
  return {
    serverAddr: '',
    serverPort: 7000,
    user: '',
    authMethod: 'token',
    authToken: '',
    logLevel: 'info',
    protocol: 'tcp',
    tlsEnable: false,
    tlsServerName: '',
    poolCount: null,
    proxies: [],
  }
}

/** 新建空代理条目 */
export function emptyProxy(): FrpFormProxy {
  return {
    name: '',
    type: 'tcp',
    localIP: '127.0.0.1',
    localPort: null,
    remotePort: null,
    customDomains: '',
    secretKey: '',
    enabled: true,
  }
}

/** 安全取对象（非对象一律给空对象，避免表单崩） */
function asObject(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {}
}

/** 安全取数组 */
function asArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : []
}

/** 安全取字符串（数字也接受，TOML 里偶有数字型字符串） */
function asString(value: unknown): string {
  if (typeof value === 'string') return value
  if (typeof value === 'number' || typeof value === 'boolean') return String(value)
  return ''
}

/** 安全取数字（取不到给 null，区别于「用户填了 0」） */
function asNumber(value: unknown): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) return value
  if (typeof value === 'string' && value.trim() !== '') {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return null
}

/** 安全取布尔 */
function asBool(value: unknown, fallback: boolean): boolean {
  return typeof value === 'boolean' ? value : fallback
}

/** 单个代理条目 → 表单模型（缺失字段用默认值，不丢原始对象） */
function toProxyModel(value: unknown): FrpFormProxy {
  const raw = asObject(value)
  const domains = asArray(raw.customDomains)
    .map((item) => asString(item))
    .filter((item) => item !== '')
  return {
    name: asString(raw.name),
    type: asString(raw.type) || 'tcp',
    localIP: asString(raw.localIP),
    localPort: asNumber(raw.localPort),
    remotePort: asNumber(raw.remotePort),
    customDomains: domains.join(', '),
    secretKey: asString(raw.secretKey),
    enabled: asBool(raw.enabled, true),
  }
}

/** TOML 解析结果 → 表单模型（只读已知字段，缺失给默认值） */
export function toFormModel(parsed: Record<string, unknown>): FrpFormModel {
  const base = emptyFormModel()
  const auth = asObject(parsed.auth)
  const log = asObject(parsed.log)
  const transport = asObject(parsed.transport)
  const tls = asObject(transport.tls)
  return {
    serverAddr: asString(parsed.serverAddr),
    serverPort: asNumber(parsed.serverPort) ?? base.serverPort,
    user: asString(parsed.user),
    authMethod: asString(auth.method) || base.authMethod,
    authToken: asString(auth.token),
    logLevel: asString(log.level) || base.logLevel,
    protocol: asString(transport.protocol) || base.protocol,
    tlsEnable: asBool(tls.enable, base.tlsEnable),
    tlsServerName: asString(tls.serverName),
    poolCount: asNumber(transport.poolCount),
    proxies: asArray(parsed.proxies).map(toProxyModel),
  }
}

/**
 * 表单字段写入目标对象：空串 / null 视为「用户清空了该字段」→ 删除键
 * （与 Rust 侧 `strip_nulls` 语义一致；TOML 没有 null，写空串会污染配置）。
 */
function assign(target: Record<string, unknown>, key: string, value: unknown): void {
  if (value === '' || value === null || value === undefined) {
    delete target[key]
    return
  }
  target[key] = value
}

/** 在嵌套表内写入字段（父表为空则一并清掉，保持配置干净） */
function assignNested(
  parent: Record<string, unknown>,
  parentKey: string,
  fields: Record<string, unknown>
): void {
  const current = asObject(parent[parentKey])
  const next: Record<string, unknown> = { ...current }
  for (const [key, value] of Object.entries(fields)) {
    assign(next, key, value)
  }
  if (Object.keys(next).length === 0) {
    delete parent[parentKey]
    return
  }
  parent[parentKey] = next
}

/** 表单代理 → 写回对象（以原始条目为基底，保留 healthCheck 等未知字段） */
function mergeProxy(base: Record<string, unknown>, proxy: FrpFormProxy): Record<string, unknown> {
  const next: Record<string, unknown> = { ...base }
  assign(next, 'name', proxy.name)
  assign(next, 'type', proxy.type)
  assign(next, 'localIP', proxy.localIP)
  assign(next, 'localPort', proxy.localPort)
  assign(next, 'remotePort', proxy.remotePort)
  assign(next, 'secretKey', proxy.secretKey)
  const domains = proxy.customDomains
    .split(',')
    .map((item) => item.trim())
    .filter((item) => item !== '')
  if (domains.length > 0) {
    next.customDomains = domains
  } else {
    delete next.customDomains
  }
  // enabled 默认即为 true：只在显式关闭时写入，避免给用户配置塞无意义字段
  if (proxy.enabled) {
    delete next.enabled
  } else {
    next.enabled = false
  }
  return next
}

/**
 * 表单模型 → 写回对象（**以原 parsed 为基底**逐字段覆盖，未知字段与段落顺序语义不变）。
 * 代理条目优先按 `name` 匹配原始条目以保留其未知字段，匹配不到时按位置回退。
 */
export function mergeFormModel(
  parsed: Record<string, unknown>,
  model: FrpFormModel
): Record<string, unknown> {
  const next: Record<string, unknown> = { ...parsed }

  assign(next, 'serverAddr', model.serverAddr)
  assign(next, 'serverPort', model.serverPort)
  assign(next, 'user', model.user)

  assignNested(next, 'auth', { method: model.authMethod, token: model.authToken })
  assignNested(next, 'log', { level: model.logLevel })
  assignNested(next, 'transport', {
    protocol: model.protocol,
    poolCount: model.poolCount,
    tls: { enable: model.tlsEnable, serverName: model.tlsServerName },
  })

  const originals = asArray(parsed.proxies)
  const byName = new Map<string, Record<string, unknown>>()
  for (const item of originals) {
    const raw = asObject(item)
    const name = asString(raw.name)
    if (name !== '') byName.set(name, raw)
  }
  const merged = model.proxies.map((proxy, index) => {
    const base = byName.get(proxy.name) ?? asObject(originals[index])
    return mergeProxy(base, proxy)
  })
  if (merged.length > 0) {
    next.proxies = merged
  } else {
    delete next.proxies
  }
  return next
}

/** 表单是否含空的服务器地址（保存前的基本校验，返回中文原因） */
export function validateFormModel(model: FrpFormModel): string {
  if (model.serverAddr.trim() === '') return 'frp.formServerRequired'
  if (model.serverPort === null || model.serverPort <= 0 || model.serverPort > 65535) {
    return 'frp.formServerPortInvalid'
  }
  for (const [index, proxy] of model.proxies.entries()) {
    if (proxy.name.trim() === '') return 'frp.formProxyNameRequired'
    if (proxy.localPort !== null && (proxy.localPort <= 0 || proxy.localPort > 65535)) {
      return 'frp.formProxyPortInvalid'
    }
    if (NEEDS_REMOTE_PORT.includes(proxy.type) && proxy.remotePort === null) {
      return 'frp.formProxyRemotePortRequired'
    }
    void index
  }
  return ''
}
