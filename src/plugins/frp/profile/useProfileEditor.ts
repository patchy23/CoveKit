/**
 * 单个档案的编辑状态（源码 / 表单双模式、保存、校验）
 * 内容与脏标记集中在此，组件只做渲染；保存路径按模式分派到不同 IPC 命令。
 */
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useUiStore } from '@/stores/ui'
import { ipc } from '../ipc'
import type { FrpVerifyError } from '../contracts'
import {
  emptyFormModel,
  mergeFormModel,
  toFormModel,
  validateFormModel,
  type FrpFormModel,
} from './frpForm'

/** 档案编辑器状态 */
export function useProfileEditor() {
  const { t } = useI18n()
  const ui = useUiStore()

  /** 当前档案文件名 */
  const fileName = ref('')
  /** TOML 原文（源码模式数据源） */
  const content = ref('')
  /** 表单模型（表单模式数据源） */
  const model = ref<FrpFormModel>(emptyFormModel())
  /** 解析后的原始对象（表单保存的基底，保留未知字段） */
  const parsed = ref<Record<string, unknown>>({})
  /** 原文是否含注释（表单保存会丢注释，保存前提示） */
  const hasComments = ref(false)
  /** 是否有未保存改动 */
  const dirty = ref(false)
  /** 加载中的文件名（用于骨架/禁用） */
  const loading = ref(false)
  /** 加载或保存失败原因 */
  const error = ref('')
  /** 保存中 */
  const saving = ref(false)
  /** 校验中 */
  const verifying = ref(false)
  /** 是否已执行过校验（区分「未校验」与「校验通过」） */
  const verified = ref(false)
  /** 校验错误列表 */
  const verifyErrors = ref<FrpVerifyError[]>([])

  /** 表单模式下锁定的保存按钮（校验不通过不给存） */
  const formValidationKey = computed(() => validateFormModel(model.value))

  /** 切换档案：读原文与解析结果，重置脏标记与校验结果 */
  async function load(name: string): Promise<void> {
    fileName.value = name
    loading.value = true
    error.value = ''
    dirty.value = false
    verified.value = false
    verifyErrors.value = []
    try {
      const result = await ipc.profileRead(name)
      if (!result.ok) throw new Error(result.error ?? t('frp.unknownError'))
      content.value = result.content
      parsed.value = result.parsed ?? {}
      model.value = toFormModel(parsed.value)
      hasComments.value = result.hasComments
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : String(reason)
      content.value = ''
      parsed.value = {}
      model.value = emptyFormModel()
    } finally {
      loading.value = false
    }
  }

  /** 源码编辑回调（标记脏） */
  function updateContent(value: string): void {
    content.value = value
    dirty.value = true
  }

  /** 表单编辑回调（标记脏） */
  function updateModel(value: FrpFormModel): void {
    model.value = value
    dirty.value = true
  }

  /** 保存源码原文 */
  async function saveText(): Promise<boolean> {
    saving.value = true
    error.value = ''
    try {
      const result = await ipc.profileSaveText(fileName.value, content.value)
      if (!result.ok) throw new Error(result.error ?? t('frp.unknownError'))
      ui.toast(t('frp.saved'))
      dirty.value = false
      await load(fileName.value)
      return true
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : String(reason)
      ui.toast(t('frp.opFailed', { message: error.value }))
      return false
    } finally {
      saving.value = false
    }
  }

  /** 保存表单（由 Rust 用 parsed 重建 TOML，未知字段保留、注释会丢） */
  async function saveForm(): Promise<boolean> {
    const invalid = formValidationKey.value
    if (invalid !== '') {
      ui.toast(t(invalid))
      return false
    }
    saving.value = true
    error.value = ''
    try {
      const merged = mergeFormModel(parsed.value, model.value)
      const result = await ipc.profileSaveForm(fileName.value, merged)
      if (!result.ok) throw new Error(result.error ?? t('frp.unknownError'))
      ui.toast(hasComments.value ? t('frp.savedFormCommentsLost') : t('frp.saved'))
      dirty.value = false
      await load(fileName.value)
      return true
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : String(reason)
      ui.toast(t('frp.opFailed', { message: error.value }))
      return false
    } finally {
      saving.value = false
    }
  }

  /** 调 frpc verify 校验当前文件（结果同时供源码模式与表单模式展示） */
  async function verify(): Promise<void> {
    verifying.value = true
    try {
      const result = await ipc.verify(fileName.value)
      verifyErrors.value = result.errors
      verified.value = true
      if (result.ok) {
        ui.toast(t('frp.verifyPassed'))
      } else {
        ui.toast(t('frp.verifyFailedCount', { count: result.errors.length }))
      }
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : String(reason)
      ui.toast(t('frp.opFailed', { message: error.value }))
    } finally {
      verifying.value = false
    }
  }

  return {
    fileName,
    content,
    model,
    parsed,
    hasComments,
    dirty,
    loading,
    error,
    saving,
    verifying,
    verified,
    verifyErrors,
    formValidationKey,
    load,
    updateContent,
    updateModel,
    saveText,
    saveForm,
    verify,
  }
}
