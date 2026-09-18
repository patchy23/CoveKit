<script setup lang="ts">
import { ref } from 'vue'
import { UiAlert, UiButton, UiField, UiInput, UiModal, UiTextarea } from '@/core/ui'
import type { CredentialOverride, ServerProfile } from '../contracts'

const props = defineProps<{ profile: ServerProfile }>()
const emit = defineEmits<{ confirm: [value: CredentialOverride]; cancel: [] }>()
const password = ref('')
const privateKey = ref('')
const passphrase = ref('')
const error = ref('')
function submit() {
  if (
    props.profile.authMethod === 'password'
      ? !password.value
      : !privateKey.value.trim() ||
        (props.profile.authMethod === 'privateKeyWithPassphrase' && !passphrase.value)
  ) {
    error.value = '请填写完整认证信息'
    return
  }
  emit(
    'confirm',
    props.profile.authMethod === 'password'
      ? { password: password.value }
      : { privateKey: privateKey.value.trim(), passphrase: passphrase.value || undefined }
  )
}
</script>

<template>
  <UiModal :open="true" title="输入 SSH 认证信息" @close="emit('cancel')">
    <form class="space-y-md" @submit.prevent="submit">
      <p class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile.name }} · {{ profile.username }}@{{ profile.host }}
      </p>
      <UiField v-if="profile.authMethod === 'password'" label="密码" required>
        <UiInput v-model="password" type="password" autocomplete="off" autofocus />
      </UiField>
      <template v-else>
        <UiField label="私钥" required><UiTextarea v-model="privateKey" :rows="6" /></UiField>
        <UiField v-if="profile.authMethod === 'privateKeyWithPassphrase'" label="私钥口令" required>
          <UiInput v-model="passphrase" type="password" autocomplete="off" />
        </UiField>
      </template>
      <p class="text-caption text-text-muted dark:text-text-muted-dark">
        仅在当前 SSH 工具打开期间用于连接和重连，不保存到凭证库。
      </p>
      <UiAlert v-if="error" tone="danger">{{ error }}</UiAlert>
      <div class="flex justify-end gap-sm">
        <UiButton @click="emit('cancel')">取消</UiButton>
        <UiButton type="submit" variant="primary">连接</UiButton>
      </div>
    </form>
  </UiModal>
</template>
