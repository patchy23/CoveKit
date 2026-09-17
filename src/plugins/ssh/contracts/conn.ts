/** SSH 契约 · 连接（主机密钥/分阶段/结构化结果/导入） */

/* ── SSH 契约 · 连接（主机密钥/分阶段/结构化结果/导入） ── */

/** 已知主机条目（patchyBox 私有 known_hosts） */

import type { ServerConnection } from './common'

export interface KnownHostEntry {
  /** 主机地址 */

  host: string

  /** 端口 */

  port: number

  /** 公钥算法名（如 ssh-ed25519） */

  algorithm: string

  /** SHA256 指纹（SHA256:base64） */

  fingerprint: string
}

/** 主机密钥人工确认请求（后端在握手回调中推送，等待 ssh_host_key_respond） */

export interface HostKeyVerifyRequest {
  /** 本次连接尝试的唯一请求 id */

  requestId: string

  /** unknown = 首次连接；mismatch = 与已保存指纹不一致 */

  kind: 'unknown' | 'mismatch'

  host: string

  port: number

  algorithm: string

  fingerprint: string

  /** kind=mismatch 时已保存的指纹列表 */

  savedFingerprints: string[]
}

/** 连接阶段事件 */

export interface ConnectStage {
  /** 本次连接尝试的请求 id（对应连接结果中的 requestId） */

  requestId: string

  /** 所属服务器配置 id（前端据此把进度关联到工作区） */

  profileId: string /** resolve / tcp / handshake / verify / auth / session */

  stage: string

  /** start / ok / fail */

  status: string

  message?: string
}

/** 稳定错误码 + 中文文案（detail 可复制，已脱敏） */

export interface SshConnectError {
  code: string

  message: string

  detail?: string
}

/** ssh_connect / ssh_reconnect 的结构化返回（业务失败不抛 IPC 异常） */

export interface SshConnectOutcome {
  ok: boolean

  connection?: ServerConnection

  requestId: string

  error?: SshConnectError
}
