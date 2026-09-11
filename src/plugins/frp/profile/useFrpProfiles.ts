/**
 * frp 档案列表与增删改（列表数据 + 工具侧元数据）
 * 运行状态与日志不在此 composable（见 runtime/useFrpRuntime），保持单一职责。
 */
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { FrpOpResult, FrpProfileSummary, FrpTemplateId } from '../contracts'

/** 档案列表状态与操作 */
export function useFrpProfiles() {
  const { t } = useI18n()
  const ui = useUiStore()

  /** 档案摘要列表（服务端顺序） */
  const items = ref<FrpProfileSummary[]>([])
  /** 实际生效的配置目录（排障与「打开目录」用） */
  const dir = ref('')
  /** 列表加载中 */
  const loading = ref(false)
  /** 列表加载失败原因（展示为错误行，不静默） */
  const error = ref('')

  /** 拉取档案列表 */
  async function refresh(): Promise<void> {
    loading.value = true
    error.value = ''
    try {
      const result = await ipc.profilesList()
      if (!result.ok) throw new Error(result.error ?? t('frp.unknownError'))
      items.value = result.profiles
      dir.value = result.dir
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : String(reason)
    } finally {
      loading.value = false
    }
  }

  /**
   * 统一执行写操作：成功 → toast + 刷新列表；失败 → toast 错误并返回 false。
   * 所有写操作都走这里，避免各调用点漏掉反馈或漏刷新。
   */
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

  /** 新建档案（内置模板） */
  function create(fileName: string, template: FrpTemplateId): Promise<boolean> {
    return runOp(ipc.profileCreate(fileName, template), 'frp.created')
  }

  /** 复制档案 */
  function duplicate(fileName: string, newName: string): Promise<boolean> {
    return runOp(ipc.profileDuplicate(fileName, newName), 'frp.duplicated')
  }

  /** 重命名档案 */
  function rename(fileName: string, newName: string): Promise<boolean> {
    return runOp(ipc.profileRename(fileName, newName), 'frp.renamed')
  }

  /** 删除档案（移入 .trash/，不物理抹除） */
  function remove(fileName: string): Promise<boolean> {
    return runOp(ipc.profileDelete(fileName), 'frp.deleted')
  }

  /** 写入备注（存 frp.db，不污染用户 TOML） */
  function setRemark(fileName: string, remark: string): Promise<boolean> {
    return runOp(ipc.profileRemark(fileName, remark), 'frp.remarkSaved')
  }

  /** 关键字过滤（文件名 / 展示名 / 备注 / 服务器地址，大小写不敏感） */
  function filter(keyword: string): FrpProfileSummary[] {
    const needle = keyword.trim().toLowerCase()
    if (needle === '') return items.value
    return items.value.filter((item) =>
      [item.fileName, item.displayName, item.remark, item.serverAddr]
        .join('\n')
        .toLowerCase()
        .includes(needle)
    )
  }

  return {
    items,
    dir,
    loading,
    error,
    refresh,
    create,
    duplicate,
    rename,
    remove,
    setRemark,
    filter,
  }
}
