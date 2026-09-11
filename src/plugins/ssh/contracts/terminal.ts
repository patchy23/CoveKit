/** SSH 契约 · 终端（会话/数据块/关闭通知） */

/* ── SSH 契约 · 终端（会话/数据块/关闭通知） ── */

/** 终端会话快照 */

export interface TerminalSession {
  /** 终端唯一 id（term-<毫秒时间戳>） */

  id: string

  /** 关联的连接 sessionId */

  connectionId: string

  /** 终端标题（默认 profile 名称） */

  title: string

  /** 当前行列数 */

  cols: number

  rows: number

  /** 是否活跃（前端正在展示） */

  active: boolean
}

/** 终端数据块（Rust 推送 → 前端渲染） */

export interface TerminalData {
  /** 关联的终端 id */

  terminalId: string

  /** 所属连接 id（空闲断开判定用：终端输出视为会话活跃） */

  connectionId: string

  /** 数据内容（原始字节，含 ANSI 转义序列） */

  data: string

  /** 毫秒时间戳 */

  time: number
}

/** 后端 PTY 通道已经关闭。 */

export interface TerminalClosed {
  terminalId: string

  /** 所属连接会话 id（前端据此触发断线自动重连） */

  connectionId: string
}

/** 会话日志操作结果（开始 / 停止共用的返回） */
export interface LogActionResult {
  /** 日志文件绝对路径 */
  path: string
  /** 已写入字节数 */
  bytes: number
}

/** 终端会话日志写盘失败通知（前端据此停止录制并提示） */
export interface TerminalLogError {
  /** 终端 id */
  terminalId: string
  /** 失败原因（含路径） */
  message: string
}
