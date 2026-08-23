<script setup lang="ts">
/**
 * ServerForm · 添加/编辑服务器弹窗
 * 表单：名称 / host / port / 用户名 / 认证方式 / 密码/密钥 / 备注
 */
import { computed, reactive, watch } from 'vue'
import type { CredentialKind } from '@/core/ipc/contracts'
import type { ServerProfile, AuthMethod } from './contracts'
import {
  CredentialPicker,
  UiButton,
  UiField,
  UiInput,
  UiModal,
  UiSelect as Select,
  UiTextarea,
} from '@/core/ui'

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
      form.remark = p.remark ?? ''
    } else {
      form.id = ''
      form.name = ''
      form.host = ''
      form.port = 22
      form.username = ''
      form.authMethod = 'password'
      form.secretRef = ''
      form.remark = ''
    }
  },
  { immediate: true }
)

/** 当前认证方式允许选择的公共 Vault 凭证类型。 */
const credentialKind = computed<CredentialKind>(() =>
  form.authMethod === 'password' ? 'password' : 'ssh-key'
)

function changeAuthMethod(value: string) {
  form.authMethod = value as AuthMethod
  // 不同认证方式的 Vault 类型不兼容，切换后要求重新选择。
  form.secretRef = ''
}

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
          :model-value="form.authMethod"
          :options="[
            { value: 'password', label: '密码' },
            { value: 'privateKey', label: '私钥' },
            { value: 'privateKeyWithPassphrase', label: '私钥 + Passphrase' },
          ]"
          @update:model-value="changeAuthMethod"
        />
      </UiField>

      <UiField
        label="凭证库（可选）"
        description="选择后由后端直接读取凭证，凭证中的用户名会覆盖上方用户名；清空选择即可继续使用手工输入。"
      >
        <CredentialPicker
          v-model="form.secretRef"
          :kind="credentialKind"
          :placeholder="credentialKind === 'password' ? '选择用户名密码凭证' : '选择 SSH 私钥凭证'"
        />
      </UiField>

      <UiField v-if="form.authMethod === 'password'" label="手工密码（可选）">
        <UiInput v-model="form.password" type="password" />
      </UiField>

      <UiField v-if="form.authMethod !== 'password'" label="手工私钥内容（可选）">
        <UiTextarea
          v-model="form.privateKey"
          class="font-mono text-body-sm"
          rows="4"
          placeholder="-----BEGIN OPENSSH PRIVATE KEY-----"
        />
      </UiField>

      <UiField
        v-if="form.authMethod === 'privateKeyWithPassphrase'"
        label="手工 Passphrase（可选）"
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
</template>
