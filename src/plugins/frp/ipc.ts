/**
 * frp 插件 · IPC 封装（本插件命令，独立于框架）
 * 命令名与入参/返回类型来自 ./contracts（与 Rust models.rs 同步）。
 */
import { invokeCommand } from '@/core/ipc/ipc'
import { commands } from './contracts'
import type {
  FrpBinaryInfo,
  FrpOpResult,
  FrpProfileContent,
  FrpProfileList,
  FrpReleaseInfo,
  FrpRuntimeState,
  FrpTemplateId,
  FrpVerifyResult,
} from './contracts'

export const ipc = {
  /** 列出配置目录下的全部档案（含运行状态与元数据） */
  profilesList: (): Promise<FrpProfileList> => invokeCommand(commands.profilesList, {}),
  /** 读取单个档案（原文 + TOML 解析结果） */
  profileRead: (fileName: string): Promise<FrpProfileContent> =>
    invokeCommand(commands.profileRead, { fileName }),
  /** 源码模式保存（写原文，改前自动备份） */
  profileSaveText: (fileName: string, content: string): Promise<FrpOpResult> =>
    invokeCommand(commands.profileSaveText, { fileName, content }),
  /** 表单模式保存（由 Rust 用 parsed 重建 TOML，保留未知字段） */
  profileSaveForm: (
    fileName: string,
    parsed: Record<string, unknown>
  ): Promise<FrpOpResult> => invokeCommand(commands.profileSaveForm, { fileName, parsed }),
  /** 新建档案（内置模板） */
  profileCreate: (fileName: string, template: FrpTemplateId): Promise<FrpOpResult> =>
    invokeCommand(commands.profileCreate, { fileName, template }),
  /** 复制档案 */
  profileDuplicate: (fileName: string, newName: string): Promise<FrpOpResult> =>
    invokeCommand(commands.profileDuplicate, { fileName, newName }),
  /** 重命名档案 */
  profileRename: (fileName: string, newName: string): Promise<FrpOpResult> =>
    invokeCommand(commands.profileRename, { fileName, newName }),
  /** 删除档案（移入 .trash/，不物理抹除） */
  profileDelete: (fileName: string): Promise<FrpOpResult> =>
    invokeCommand(commands.profileDelete, { fileName }),
  /** 写入档案备注（存 frp.db，不动用户 TOML） */
  profileRemark: (fileName: string, remark: string): Promise<FrpOpResult> =>
    invokeCommand(commands.profileRemark, { fileName, remark }),
  /** 校验档案（frpc verify -c <path>） */
  verify: (fileName: string): Promise<FrpVerifyResult> =>
    invokeCommand(commands.verify, { fileName }),
  /** 启动档案 */
  start: (fileName: string): Promise<FrpRuntimeState> =>
    invokeCommand(commands.start, { fileName }),
  /** 停止档案 */
  stop: (fileName: string): Promise<FrpRuntimeState> =>
    invokeCommand(commands.stop, { fileName }),
  /** 重启档案 */
  restart: (fileName: string): Promise<FrpRuntimeState> =>
    invokeCommand(commands.restart, { fileName }),
  /** 全部档案的当前状态（事件为主，5s 轮询兜底） */
  status: (): Promise<FrpRuntimeState[]> => invokeCommand(commands.status, {}),
  /** 探测 frpc 可执行文件（给定路径 / 设置项 / PATH / 常见位置） */
  binaryDetect: (path?: string): Promise<FrpBinaryInfo> =>
    invokeCommand(commands.binaryDetect, path === undefined ? {} : { path }),
  /** 上游可用版本列表 */
  binaryVersions: (limit?: number): Promise<FrpReleaseInfo[]> =>
    invokeCommand(commands.binaryVersions, limit === undefined ? {} : { limit }),
  /** 下载并安装 frpc（进度走 frp://download 事件） */
  binaryDownload: (version: string): Promise<FrpBinaryInfo> =>
    invokeCommand(commands.binaryDownload, { version }),
}
