<script setup lang="ts">
/**
 * ServerForm · 添加/编辑服务器弹窗
 * 表单：名称 / host / port / 用户名 / 认证方式（含凭证库条目，合并到一个下拉）/ 手工密钥 / 备注
 * 认证方式 = 手工三档 + 凭证库条目（vault:<id>）；选中凭证即用它连接（用户名由凭证覆盖），
 * 凭证类型与 SSH 认证不符时由后端在连接时给出明确报错。
 */
import { computed, onMounted, reactive, ref, watch } from 'vue'
import type { CredentialSummary } from '@/core/ipc/contracts'
import { ipc } from '@/core/ipc/ipc'
import { KIND_LABEL } from '@/core/vault/useVault'
import CredentialForm from '@/core/vault/CredentialForm.vue'
import type { ServerProfile, AuthMethod } from './contracts'
import {
  UiButton,
  UiCombobox,
  UiField,
  UiInput,
  UiModal,
  UiSelect as Select,
  UiTextarea,
} from '@/core/ui'
import type { SelectOption } from '@/core/ui/UiSelect.vue'

const props = defineProps<{
  profile: ServerProfile | null
}>()

const emit = defineEmits<{
  (
    e: 'save',
    p: ServerProfile,
    creds: { password?: string; privateKey?: string; passphrase?: string }
  ): void
  (e: 'cancel'): void
  (e: 'error', msg: string): void
}>()

const form = reactive({
  id: '',
  name: '',
  host: '',
  port: 22,
  username: '',
  authMethod: 'password' as AuthMethod,
  secretRef: '',
  password: '',
  privateKey: '',
  passphrase: '',
  remark: '',
})

/** 认证方式是否处于「凭证」档（UI 层状态；选了凭证后 secretRef 才有值；须在 watch 之前声明） */
const credentialMode = ref(false)

watch(
  () => props.profile,
  (p) => {
    // 凭证永不回填到表单；每次打开都清空，避免上一次输入泄漏到另一台服务器。
    form.password = ''
    form.privateKey = ''
    form.passphrase = ''
    if (p) {
      form.id = p.id
      form.name = p.name
      form.host = p.host
      form.port = p.port
      form.username = p.username
      form.authMethod = p.authMethod
      form.secretRef = p.secretRef ?? ''
      credentialMode.value = Boolean(p.secretRef)
      form.remark = p.remark ?? ''
    } else {
      form.id = ''
      form.name = ''
      form.host = ''
      form.port = 22
      form.username = ''
      form.authMethod = 'password'
      form.secretRef = ''
      credentialMode.value = false
      form.remark = ''
    }
  },
  { immediate: true }
)

/** 新建凭证的哨兵值（选中它 = 打开新建表单） */
const CREATE_VALUE = '__create__'

/** 凭证库条目（只列 SSH 可用的两类：用户名密码 / SSH 私钥） */
const credentials = ref<CredentialSummary[]>([])
const credentialsLoaded = ref(false)
const credentialsFailed = ref(false)
const credFormOpen = ref(false)

async function loadCredentials() {
  credentialsFailed.value = false
  try {
    const all = await ipc.vaultList()
    credentials.value = all.filter((c) => c.kind === 'password' || c.kind === 'ssh-key')
  } catch (e) {
    credentials.value = []
    credentialsFailed.value = true
    console.warn('[ssh] 凭证库加载失败', e)
  } finally {
    credentialsLoaded.value = true
  }
}
onMounted(loadCredentials)

/** 认证方式下拉选中值：凭证档为 credential，否则为手工认证方式 */
const authValue = computed(() => (credentialMode.value ? 'credential' : form.authMethod))

const AUTH_OPTIONS: SelectOption[] = [
  { value: 'password', label: '密码（手工输入）' },
  { value: 'privateKey', label: '私钥（手工输入）' },
  { value: 'privateKeyWithPassphrase', label: '私钥 + Passphrase（手工输入）' },
  { value: 'credential', label: '凭证（凭证库选择）' },
]

/** 凭证下拉选项（可搜索：匹配名称/类型/掩码摘要），末尾新建入口 */
const credentialOptions = computed(() => [
  ...credentials.value.map((c) => ({
    value: c.id,
    label: `${c.name}（${KIND_LABEL[c.kind]} · ${c.masked}）`,
    keywords: `${c.name} ${c.masked} ${KIND_LABEL[c.kind]}`,
  })),
  { value: CREATE_VALUE, label: '+ 新建凭证' },
])

/** 当前选中凭证（展示用） */
const selectedCredential = computed(() => credentials.value.find((c) => c.id === form.secretRef))

function onAuthChange(value: string | number) {
  const v = String(value)
  if (v === 'credential') {
    credentialMode.value = true
    return
  }
  // 切回手工输入：清空凭证引用
  credentialMode.value = false
  form.secretRef = ''
  form.authMethod = v as AuthMethod
}

/** 选择凭证：记录引用并把认证方式推导为凭证类型对应的手工档（后端连接时按凭证内容认证） */
function onCredentialSelect(value: string) {
  if (value === CREATE_VALUE) {
    credFormOpen.value = true
    return
  }
  const cred = credentials.value.find((c) => c.id === value)
  if (!cred) return
  form.secretRef = cred.id
  // 密码凭证→password；SSH 私钥凭证→privateKey（passphrase 随凭证内容走）
  form.authMethod = cred.kind === 'password' ? 'password' : 'privateKey'
}

/** 新建凭证保存成功：刷新列表并自动选中 */
function onCredentialSaved(summary: CredentialSummary) {
  void loadCredentials().then(() => {
    if (summary.kind === 'password' || summary.kind === 'ssh-key') {
      form.secretRef = summary.id
      form.authMethod = summary.kind === 'password' ? 'password' : 'privateKey'
    }
  })
}

/** 编辑场景：引用的凭证已被删除时提示（选择时即拦截，不必等到连接） */
const credentialMissing = computed(
  () =>
    credentialsLoaded.value &&
    form.secretRef !== '' &&
    !credentialsFailed.value &&
    !selectedCredential.value
)

function submit() {
  // 必填校验：名称/主机/用户名任一为空则提示并中止（UI-014）
  if (!form.name.trim()) {
    emit('error', '请输入服务器名称')
    return
  }
  if (!form.host.trim()) {
    emit('error', '请输入主机地址')
    return
  }
  if (!form.username.trim()) {
    emit('error', '请输入登录用户名')
    return
  }
  if (!Number.isInteger(form.port) || form.port < 1 || form.port > 65535) {
    emit('error', '端口必须是 1 到 65535 之间的整数')
    return
  }
  if (credentialMode.value && !form.secretRef) {
    emit('error', '请选择凭证（或从下拉末尾新建）')
    return
  }
  const p: ServerProfile = {
    id: form.id || `profile-${Date.now()}`,
    name: form.name.trim(),
    host: form.host.trim(),
    port: form.port,
    username: form.username.trim(),
    authMethod: form.authMethod,
    secretRef: form.secretRef || undefined,
    remark: form.remark.trim() || undefined,
    lastConnectedAt: props.profile?.lastConnectedAt,
  }
  emit('save', p, {
    password: form.password.trim() || undefined,
    privateKey: form.privateKey.trim() || undefined,
    passphrase: form.passphrase.trim() || undefined,
  })
}
</script>

<template>
  <UiModal
    :open="true"
    :title="props.profile ? '编辑服务器' : '添加服务器'"
    width="min(420px, 92vw)"
    @close="emit('cancel')"
  >
    <div class="space-y-[10px]">
      <UiField label="名称" required>
        <UiInput v-model="form.name" placeholder="如：生产服务器" />
      </UiField>

      <div class="grid grid-cols-2 gap-[10px]">
        <UiField label="主机" required>
          <UiInput v-model="form.host" class="font-mono" placeholder="192.168.1.1" />
        </UiField>
        <UiField label="端口" required>
          <UiInput v-model.number="form.port" type="number" />
        </UiField>
      </div>

      <UiField label="用户名" required>
        <UiInput v-model="form.username" placeholder="root" />
      </UiField>

      <UiField label="认证方式">
        <Select
          :model-value="authValue"
          :options="AUTH_OPTIONS"
          @update:model-value="onAuthChange"
        />
      </UiField>

      <!-- 凭证档：可输入搜索的凭证选择（下方独立下拉） -->
      <UiField v-if="credentialMode" label="凭证">
        <UiCombobox
          :model-value="form.secretRef"
          :options="credentialOptions"
          placeholder="选择凭证"
          search-placeholder="搜索凭证名称…"
          empty-text="凭证库暂无可用凭证（支持用户名密码 / SSH 私钥类型）"
          @update:model-value="onCredentialSelect"
        />
      </UiField>

      <!-- 选中凭证：提示用户名/凭据来自凭证库；手工输入框隐藏 -->
      <p v-if="selectedCredential" class="text-caption text-text-muted dark:text-text-muted-dark">
        将使用凭证「{{ selectedCredential.name }}」中的{{
          selectedCredential.kind === 'password' ? '用户名与密码' : '用户名与私钥'
        }}连接（用户名覆盖上方填写值）；类型不符时连接会明确报错。
      </p>
      <p v-if="credentialMissing" class="text-caption text-danger-strong dark:text-danger-dark">
        引用的凭证已删除或不可用，请重新选择。
      </p>
      <p v-if="credentialsFailed" class="text-caption text-danger-strong dark:text-danger-dark">
        凭证库暂不可用；可稍后重试，或改用手工输入。
      </p>

      <UiField v-if="!credentialMode && form.authMethod === 'password'" label="密码">
        <UiInput v-model="form.password" type="password" />
      </UiField>

      <UiField v-if="!credentialMode && form.authMethod !== 'password'" label="私钥内容">
        <UiTextarea
          v-model="form.privateKey"
          class="font-mono text-body-sm"
          rows="4"
          placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
        />
      </UiField>

      <UiField
        v-if="!credentialMode && form.authMethod === 'privateKeyWithPassphrase'"
        label="Passphrase"
      >
        <UiInput v-model="form.passphrase" type="password" />
      </UiField>

      <UiField label="备注（可选）">
        <UiInput v-model="form.remark" placeholder="用途说明" />
      </UiField>
    </div>

    <template #footer>
      <UiButton variant="ghost" @click="emit('cancel')">取消</UiButton>
      <UiButton variant="primary" @click="submit">保存</UiButton>
    </template>
  </UiModal>

  <!-- 认证方式里的「+ 新建凭证」内嵌表单（保存成功自动选中） -->
  <CredentialForm
    :open="credFormOpen"
    initial-kind="password"
    @close="credFormOpen = false"
    @saved="onCredentialSaved"
  />
</template>
