/**
 * IPC 契约 · 唯一事实源
 * 与 src-tauri/modules 的 serde 结构同步（架构 §5）。
 * 铁律：IPC 出入参只在本文档出现一次；
 *       新增 Rust 命令 = 在此声明 payload/result + Rust 模块实现 + lib.rs 装配一行。
 */

// ── 出参结构 ──

/** 剪贴板历史记录 */
export interface ClipboardRecord {
  id: string;
  kind: "text" | "image" | "file";
  content: string;
  preview: string;
  pinned: boolean;
  createdAt: number;
}

/** 应用设置（Rust modules/settings.rs 全量读写） */
export interface AppSettings {
  theme: "light" | "dark" | "system";
  language: "zh-CN" | "en-US";
  globalHotkey: string;
  launchAtStartup: boolean;
  clipboard: { enabled: boolean; historyLimit: number; ignore: string[] };
  recentTools: string[];
  tools: Record<string, Record<string, unknown>>;
}

/** 屏幕取色结果 */
export interface PickColorResult {
  hex: string;
  rgb: [number, number, number];
}

/** 接口列表记录（Params/Headers 为 KvRow 的 JSON 字符串） */
export interface ApiRecord {
  id: number;
  type: "http" | "ws";
  name: string;
  method: string;
  url: string;
  params: string;
  headers: string;
  bodyMode: string;
  body: string;
  updatedAt: string;
}

/** 窗口状态 */
export interface WindowState {
  visible: boolean;
}

/* ── HTTP/WS 调试（第二批，modules/http_ws）── */

export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";

/** HTTP 请求载荷 */
export interface HttpRequestPayload {
  method: HttpMethod;
  url: string;
  headers: [string, string][];
  body?: string;
  timeoutMs?: number;
}

/** HTTP 响应结果 */
export interface HttpResponseResult {
  ok: boolean;
  status: number;
  statusText: string;
  headers: [string, string][];
  body: string;
  bodySize: number;
  durationMs: number;
  error?: string;
}

/** WebSocket 连接请求 */
export interface WsConnectPayload {
  url: string;
  headers?: [string, string][];
}

/** WS 消息（方向 + 内容 + 时间） */
export interface WsMessage {
  direction: "sent" | "received";
  content: string;
  time: number;
}

/** WS 会话快照 */
export interface WsSession {
  id: string;
  url: string;
  connectedAt: number;
  open: boolean;
  messages: WsMessage[];
}

/** WS 会话操作结果 */
export interface WsActionResult {
  ok: boolean;
  message?: string;
}

/* ── SQLite 数据库（第二批，modules/db，sqlx 统一三方言）── */

/** 打开数据库结果 */
export interface DbOpenResult {
  ok: boolean;
  tables: string[];
  error?: string;
}

/** SQL 执行结果（查询返回表格，非查询返回影响行数） */
export interface DbQueryResult {
  ok: boolean;
  columns: string[];
  rows: string[][];
  rowsAffected: number;
  isQuery: boolean;
  error?: string;
}

/* ── hosts 修改（第二批，modules/hosts，UAC 提权）── */

/** hosts 读取/保存结果 */
export interface HostsResult {
  ok: boolean;
  content: string;
  error?: string;
}

// ── 命令清单（IPC 出入参的唯一出处）──

export const commandNames = {
  settingsGet: "settings_get",
  settingsSet: "settings_set",
  clipboardList: "clipboard_list",
  clipboardDelete: "clipboard_delete",
  clipboardClear: "clipboard_clear",
  clipboardTogglePin: "clipboard_toggle_pin",
  colorPickScreen: "color_pick_screen",
  windowToggle: "window_toggle",
  windowHide: "window_hide",
  openExternal: "open_external",
  httpRequest: "http_request",
  apiSave: "api_save",
  apiList: "api_list",
  apiDelete: "api_delete",
  apiClear: "api_clear",
  wsConnect: "ws_connect",
  wsSend: "ws_send",
  wsRecv: "ws_recv",
  wsClose: "ws_close",
  wsSessions: "ws_sessions",
  dbOpen: "db_open",
  dbClose: "db_close",
  dbTables: "db_tables",
  dbExecute: "db_execute",
  dbQueryTable: "db_query_table",
  hostsRead: "hosts_read",
  hostsSave: "hosts_save",
} as const;

/** 各命令入参（Record<string, never> = 无参命令） */
export type IpcPayloads = {
  settings_get: { key?: string };
  settings_set: { key: string; value: unknown };
  clipboard_list: { limit?: number; pinnedOnly?: boolean };
  clipboard_delete: { id: string };
  clipboard_clear: Record<string, never>;
  clipboard_toggle_pin: { id: string };
  color_pick_screen: Record<string, never>;
  window_toggle: Record<string, never>;
  window_hide: Record<string, never>;
  open_external: { url: string };
  http_request: { payload: HttpRequestPayload };
  api_save: {
    id?: number;
    type: "http" | "ws";
    name: string;
    method: string;
    url: string;
    params: string;
    headers: string;
    bodyMode: string;
    body: string;
  };
  api_list: Record<string, never>;
  api_delete: { id: number };
  api_clear: Record<string, never>;
  ws_connect: WsConnectPayload;
  ws_send: { id: string; message: string };
  ws_recv: { id: string };
  ws_close: { id: string };
  ws_sessions: Record<string, never>;
  db_open: { path: string };
  db_close: Record<string, never>;
  db_tables: Record<string, never>;
  db_execute: { sql: string };
  db_query_table: { table: string; limit?: number };
  hosts_read: Record<string, never>;
  hosts_save: { content: string };
};

/** 各命令返回 */
export type IpcResults = {
  settings_get: AppSettings;
  settings_set: void;
  clipboard_list: ClipboardRecord[];
  clipboard_delete: void;
  clipboard_clear: void;
  clipboard_toggle_pin: void;
  color_pick_screen: PickColorResult;
  window_toggle: WindowState;
  window_hide: void;
  open_external: void;
  http_request: HttpResponseResult;
  api_save: number;
  api_list: ApiRecord[];
  api_delete: void;
  api_clear: void;
  ws_connect: WsSession;
  ws_send: WsActionResult;
  ws_recv: WsSession;
  ws_close: WsActionResult;
  ws_sessions: WsSession[];
  db_open: DbOpenResult;
  db_close: void;
  db_tables: string[];
  db_execute: DbQueryResult;
  db_query_table: DbQueryResult;
  hosts_read: HostsResult;
  hosts_save: HostsResult;
};
