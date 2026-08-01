/**
 * 会话模型 · 第二批预留（DB / SSH / WS 连接句柄）
 * 第一批不创建实际会话；此处仅锁定统一连接配置形状（ConnectionProfile）。
 * 第二批实现：sessions 注册表 + 生命周期 + 空闲超时回收（架构 §4.3 / §7）。
 */
export type { ConnectionProfile } from "@/core/registry/types";
