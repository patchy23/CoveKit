/**
 * SSH 服务器表单的凭证选择逻辑（从 ServerForm 拆出，300 行红线）
 * 负责：凭证库加载（含失败可见）、下拉选项（含 + 新建入口）、选中推导认证方式、失效检测。
 */
import { computed, onMounted, ref, type ComputedRef } from 'vue'
import { ipc } from '@/core/ipc/ipc'
import { KIND_LABEL } from '@/core/vault/useVault'
import type { CredentialSummary } from '@/core/ipc/contracts'

/** 新建凭证的哨兵值（选中它 = 打开新建表单） */
const CREATE_VALUE = '__create__'

interface FormLike {
  authMethod: string
  secretRef: string
}

export function useServerCredentialChoice(
  form: FormLike,
  /** 选中凭证后按凭证类型推导认证方式（password→password；ssh-key→privateKey） */
  setAuthMethod: (m: 'password' | 'privateKey') => void
) {
  /** 凭证库条目（只列 SSH 可用的两类：用户名密码 / SSH 私钥） */
  const credentials = ref<CredentialSummary[]>([])
  const credentialsLoaded = ref(false)
  const credentialsFailed = ref(false)
  /** 内嵌新建凭证表单开关 */
  const credFormOpen = ref(false)

  async function loadCredentials() {
    try {
      const all = await ipc.vaultList()
      credentials.value = all.filter((c) => c.kind === 'password' || c.kind === 'ssh-key')
      credentialsFailed.value = false
    } catch {
      // 凭证库不可用要可见（原 CredentialPicker 静默吞掉导致「下拉是空的」无反馈）
      credentialsFailed.value = true
    } finally {
      credentialsLoaded.value = true
    }
  }
  onMounted(loadCredentials)

  /** 凭证下拉选项：条目 + 新建入口 */
  const credentialOptions = computed(() => [
    ...credentials.value.map((c) => ({
      value: c.id,
      label: `${c.name}（${KIND_LABEL[c.kind]} · ${c.masked}）`,
    })),
    { value: CREATE_VALUE, label: '+ 新建凭证' },
  ])

  const selectedCredential: ComputedRef<CredentialSummary | undefined> = computed(() =>
    credentials.value.find((c) => c.id === form.secretRef)
  )

  /** 编辑场景：引用的凭证已被删除/不可用 */
  const credentialMissing = computed(
    () =>
      Boolean(form.secretRef) &&
      credentialsLoaded.value &&
      !credentialsFailed.value &&
      !selectedCredential.value
  )

  /** 凭证下拉选择：新建哨兵开表单，否则写入引用并按类型推导认证方式 */
  function onCredentialSelect(value: string) {
    if (value === CREATE_VALUE) {
      credFormOpen.value = true
      return
    }
    form.secretRef = value
    const cred = credentials.value.find((c) => c.id === value)
    if (cred) setAuthMethod(cred.kind === 'password' ? 'password' : 'privateKey')
  }

  /** 新建凭证保存成功：回插列表并自动选中 */
  function onCredentialSaved(summary: CredentialSummary) {
    credFormOpen.value = false
    if (summary.kind === 'password' || summary.kind === 'ssh-key') {
      credentials.value = [...credentials.value, summary]
      onCredentialSelect(summary.id)
    }
  }

  return {
    credentials,
    credentialsFailed,
    credFormOpen,
    credentialOptions,
    selectedCredential,
    credentialMissing,
    onCredentialSelect,
    onCredentialSaved,
  }
}
