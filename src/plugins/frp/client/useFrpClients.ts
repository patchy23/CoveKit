/**
 * frp 客户端清单与操作（客户端管理弹窗、档案选择器、工作台共用一份状态）
 * 写操作统一走 runOp：成功 toast + 刷新清单；失败 toast 错误并返回 false，绝不静默吞。
 */
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { FrpClient, FrpOpResult } from '../contracts'

/** 客户端清单状态与操作 */
export function useFrpClients() {
  const { t } = useI18n()
  const ui = useUiStore()

  /** 已登记的客户端（服务端顺序） */
  const clients = ref<FrpClient[]>([])
  /** 默认客户端 id（无默认时为 undefined） */
  const defaultId = ref<string | undefined>(undefined)
  /** 清单加载中 */
  const loading = ref(false)
  /** 加载失败原因（展示为错误行，不静默） */
  const error = ref('')

  /** 拉取客户端清单 */
  async function refresh(): Promise<void> {
    loading.value = true
    error.value = ''
    try {
      const result = await ipc.clientList()
      if (!result.ok) throw new Error(result.error ?? t('frp.unknownError'))
      clients.value = result.clients
      defaultId.value = result.defaultId
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : String(reason)
    } finally {
      loading.value = false
    }
  }

  /** 统一执行写操作：成功 → toast + 刷新；失败 → toast 错误并返回 false */
  async function runOp(task: Promise<FrpOpResult>, successKey: string): Promise<boolean> {
    try {
      const result = await task
      if (!result.ok) throw new Error(result.error ?? t('frp.unknownError'))
      ui.toast(t(successKey))
      await refresh()
      return true
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason)
      ui.toast(t('frp.opFailed', { message }))
      return false
    }
  }

  /** 登记外部可执行文件（选择器选完路径后调用；重复登记同一路径是幂等的） */
  async function addExternal(path: string): Promise<boolean> {
    try {
      await ipc.clientAdd(path)
      ui.toast(t('frp.clientAdded'))
      await refresh()
      return true
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason)
      ui.toast(t('frp.opFailed', { message }))
      return false
    }
  }

  /** 移除客户端登记（只删记录不删文件） */
  function remove(id: string): Promise<boolean> {
    return runOp(ipc.clientRemove(id), 'frp.clientRemoved')
  }

  /** 设为默认客户端 */
  function setDefault(id: string): Promise<boolean> {
    return runOp(ipc.clientSetDefault(id), 'frp.clientDefaultSet')
  }

  /** 设置档案绑定的客户端（clientId 传 undefined 等于解除绑定、回到跟随默认） */
  function bindProfile(fileName: string, clientId?: string): Promise<boolean> {
    return runOp(ipc.profileClientSet(fileName, clientId), 'frp.clientBound')
  }

  /** 下载安装指定官方版本（进度由调用方订阅 frp://download 展示） */
  async function download(version: string): Promise<boolean> {
    try {
      await ipc.binaryDownload(version)
      ui.toast(t('frp.clientDownloaded', { version }))
      await refresh()
      return true
    } catch (reason) {
      const message = reason instanceof Error ? reason.message : String(reason)
      ui.toast(t('frp.opFailed', { message }))
      await refresh()
      return false
    }
  }

  return {
    clients,
    defaultId,
    loading,
    error,
    refresh,
    addExternal,
    remove,
    setDefault,
    bindProfile,
    download,
  }
}
