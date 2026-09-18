<script setup lang="ts">
/**
 * ServerForm · 添加/编辑服务器弹窗
 * 表单：名称 / host / port / 用户名 / 认证方式（含凭证库条目，合并到一个下拉）/ 手工密钥 / 备注
 * 认证方式 = 手工三档 + 凭证库条目（vault:<id>）；选中凭证即用它连接（用户名由凭证覆盖），
 * 凭证类型与 SSH 认证不符时由后端在连接时给出明确报错。
 */
import { computed, reactive, ref, watch } from 'vue'
import { CredentialForm } from '@/core/vault'
import type { ServerProfile, AuthMethod } from '../contracts'
import { useServerCredentialChoice } from './useServerCredentialChoice'
import type { ServerGroup } from './useServerGroups'
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

const props = withDefaults(
  defineProps<{
    profile: ServerProfile | null
    groups?: ServerGroup[]
    defaultGroupId?: string | null
  }>(),
  { groups: () => [], defaultGroupId: null }
)
const UNGROUPED = '__ungrouped__'
const groupOptions = computed(() => [
  { value: UNGROUPED, label: '未分组' },
  ...props.groups.map((group) => ({ value: group.id, label: group.name })),
])

const emit = defineEmits<{
  (
    e: 'save',
    p: ServerProfile,
    creds: { password?: string; privateKey?: string; passphrase?: string },
    saveCredential: boolean,
    saveLocal: boolean
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
  credentialRef: '',
  password: '',
  privateKey: '',
  passphrase: '',
  remark: '',
  groupId: UNGROUPED,
})

/** 手工认证的存储选择；不保存时仅保留当前工具实例的内存副本。 */
const saveMode = ref<'local' | 'vault' | 'session'>('local')
const saveOptions = [
  { value: 'local', label: '本地保存' },
  { value: 'session', label: '不保存' },
  { value: 'vault', label: '保存到凭证库' },
]
const credentialMode = ref(true)

watch(
  () => props.profile,
  (p) => {
    // 凭证永不回填到表单；每次打开都清空，避免上一次输入泄漏到另一台服务器。
    form.password = ''
    form.privateKey = ''
    form.passphrase = ''
    saveMode.value = props.profile ? (props.profile.hasLocalAuth ? 'local' : 'session') : 'local'
    if (p) {
      form.id = p.id
      form.name = p.name
      form.host = p.host
      form.port = p.port
      form.username = p.username
      form.authMethod = p.authMethod
      form.credentialRef = p.credentialRef ?? ''
      credentialMode.value = Boolean(p.credentialRef)
      form.remark = p.remark ?? ''
      form.groupId = p.groupId ?? UNGROUPED
    } else {
      form.id = ''
      form.name = ''
      form.host = ''
      form.port = 22
      form.username = ''
      form.authMethod = 'password'
      form.credentialRef = ''
      credentialMode.value = true
      form.remark = ''
      form.groupId = props.defaultGroupId ?? UNGROUPED
    }
  },
  { immediate: true }
)

/* ── 认证方式选择（手工三档 + 凭证档） ── */
const authValue = computed(() => (credentialMode.value ? 'credential' : form.authMethod))

const AUTH_OPTIONS: SelectOption[] = [
  { value: 'password', label: '密码（手工输入）' },
  { value: 'privateKey', label: '私钥（手工输入）' },
  { value: 'privateKeyWithPassphrase', label: '私钥 + Passphrase（手工输入）' },
  { value: 'credential', label: '凭证（凭证库选择）' },
]

function onAuthChange(value: string | number) {
  const v = String(value)
  if (v === 'credential') {
    credentialMode.value = true
    return
  }
  // 切回手工输入：清空凭证引用
  credentialMode.value = false
  if (form.credentialRef) saveMode.value = 'local'
  form.credentialRef = ''
  form.authMethod = v as AuthMethod
}

/* ── 凭证档逻辑（加载/选项/失效检测在 useServerCredentialChoice） ── */
const {
  credentialsFailed,
  credFormOpen,
  credentialOptions,
  selectedCredential,
  credentialMissing,
  onCredentialSelect,
  onCredentialSaved,
} = useServerCredentialChoice(form, (m) => (form.authMethod = m))

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
  if (credentialMode.value && !form.credentialRef) {
    emit('error', '请选择凭证（或从下拉末尾新建）')
    return
  }
  const p: ServerProfile = {
    id: form.id || `profile-${crypto.randomUUID()}`,
    name: form.name.trim(),
    host: form.host.trim(),
    port: form.port,
    username: form.username.trim(),
    authMethod: form.authMethod,
    credentialRef: form.credentialRef || undefined,
    hasLocalAuth: props.profile?.hasLocalAuth,
    remark: form.remark.trim() || undefined,
    lastConnectedAt: props.profile?.lastConnectedAt,
    groupId: form.groupId === UNGROUPED ? undefined : form.groupId,
  }
  emit(
    'save',
    p,
    {
      password: form.password || undefined,
      privateKey: form.privateKey.trim() || undefined,
      passphrase: form.passphrase || undefined,
    },
    !credentialMode.value && saveMode.value === 'vault',
    !credentialMode.value && saveMode.value === 'local'
  )
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

      <UiField label="分组">
        <Select
          :model-value="form.groupId"
          :options="groupOptions"
          @update:model-value="form.groupId = String($event)"
        />
      </UiField>

      <div class="grid grid-cols-2 gap-[10px]">
        <UiField label="主机" required>
          <UiInput v-model="form.host" class="font-mono" placeholder="192.168.1.1" />
        </UiField>
        <UiField label="端口" required>
          <UiInput v-model.number="form.port" inputmode="numeric" />
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
          :model-value="form.credentialRef"
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
        <UiInput
          v-model="form.password"
          type="password"
          :placeholder="props.profile?.hasLocalAuth ? '已本地保存，留空不修改' : '请输入密码'"
        />
      </UiField>
      <UiField
        v-if="!credentialMode"
        label="保存方式"
        :description="
          saveMode === 'local'
            ? '直接保存在本机 SSH 数据库，不使用凭证库保护。'
            : saveMode === 'session'
              ? '关闭 SSH 工具后忘记。'
              : '加密保存为凭证库条目。'
        "
      >
        <Select
          :model-value="saveMode"
          :options="saveOptions"
          @update:model-value="saveMode = $event as typeof saveMode"
        />
      </UiField>

      <UiField v-if="!credentialMode && form.authMethod !== 'password'" label="私钥内容">
        <UiTextarea
          v-model="form.privateKey"
          class="font-mono text-body-sm"
          rows="4"
          :placeholder="
            props.profile?.hasLocalAuth
              ? '已本地保存，全部留空不修改；修改时请填写完整认证信息'
              : '-----BEGIN OPENSSH PRIVATE KEY-----'
          "
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
