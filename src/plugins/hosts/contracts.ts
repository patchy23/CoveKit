/**
 * hosts 修改插件 · IPC 契约（本插件私有，独立于框架与其它插件）
 * 与 src-tauri/modules/hosts.rs 的 serde 结构同步。
 */

/** hosts 读取/保存结果 */
export interface HostsResult {
  ok: boolean;
  content: string;
  error?: string;
}

/** 命令清单 */
export const commands = {
  hostsRead: "hosts_read",
  hostsSave: "hosts_save",
} as const;

/** 命令入参 */
export type Payloads = {
  hosts_read: Record<string, never>;
  hosts_save: { content: string };
};

/** 命令返回 */
export type Results = {
  hosts_read: HostsResult;
  hosts_save: HostsResult;
};
