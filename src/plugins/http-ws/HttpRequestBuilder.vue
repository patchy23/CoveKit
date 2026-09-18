<script setup lang="ts">
/** 每个页签的紧凑请求编辑区，独立保留配置子页与编辑状态。 */
import { computed, ref, watch } from 'vue'
import {
  UiCodeEditor,
  UiIcon,
  UiIconButton,
  UiInput,
  UiScrollArea,
  UiSelect,
  UiTabs,
} from '@/core/ui'
import { CredentialPicker } from '@/core/vault'
import type { BodyMode } from './contracts'
import type { RequestDraft } from './requestDraft'
import RequestRows from './RequestRows.vue'
const draft = defineModel<RequestDraft>({ required: true })
defineEmits<{ save: [] }>()
const tab = ref('params'),
  collapsed = ref(false)
watch(
  () => draft.value.auth.mode,
  () => {
    draft.value.auth.credentialId = ''
    draft.value.auth.username = ''
    draft.value.auth.secret = ''
  }
)
const items = computed(() => [
  { value: 'params', label: '参数', badge: draft.value.params.filter((r) => r.key.trim()).length },
  { value: 'auth', label: '认证' },
  {
    value: 'headers',
    label: '请求头',
    badge: draft.value.headers.filter((r) => r.key.trim()).length,
  },
  ...(draft.value.type === 'ws' ? [] : [{ value: 'body', label: '请求体' }]),
  { value: 'settings', label: '设置' },
])
</script>
<template>
  <section
    class="flex min-h-0 shrink-0 flex-col border-b border-border dark:border-border-dark"
    :style="collapsed ? undefined : 'flex-basis: 38%; min-height: 140px'"
  >
    <div class="flex shrink-0 items-center px-[8px]">
      <UiTabs
        v-model="tab"
        class="min-w-0 flex-1"
        variant="line"
        size="sm"
        :items="items"
      /><UiIconButton
        size="xs"
        :label="collapsed ? '展开请求配置' : '收起请求配置'"
        @click="collapsed = !collapsed"
        ><UiIcon :name="collapsed ? 'chevron-down' : 'chevron-up'" :size="14"
      /></UiIconButton>
    </div>
    <UiScrollArea v-show="!collapsed" class="min-h-0 flex-1" axis="vertical">
      <div class="p-[10px]">
        <RequestRows v-if="tab === 'params'" v-model="draft.params" label="添加参数" />
        <RequestRows v-if="tab === 'headers'" v-model="draft.headers" label="添加请求头" />
        <div v-if="tab === 'auth'" class="space-y-[8px]">
          <div class="flex flex-wrap items-center gap-[8px]">
            <span class="text-body-sm">认证方式</span
            ><UiSelect
              v-model="draft.auth.mode"
              size="sm"
              class="w-[160px]"
              :options="[
                { value: 'none', label: '无认证' },
                { value: 'basic', label: 'Basic Auth' },
                { value: 'bearer', label: 'Bearer Token' },
              ]"
            />
          </div>
          <template v-if="draft.auth.mode !== 'none'">
            <CredentialPicker
              v-model="draft.auth.credentialId"
              :kind="draft.auth.mode === 'basic' ? 'password' : 'api-token'"
              size="sm"
            />
            <div v-if="!draft.auth.credentialId" class="flex flex-wrap gap-[8px]">
              <UiInput
                v-if="draft.auth.mode === 'basic'"
                v-model="draft.auth.username"
                size="sm"
                class="min-w-[120px] flex-1"
                aria-label="临时用户名"
                placeholder="用户名"
              /><UiInput
                v-model="draft.auth.secret"
                type="password"
                size="sm"
                class="min-w-[140px] flex-1"
                :aria-label="draft.auth.mode === 'basic' ? '临时密码' : '临时 Token'"
                :placeholder="draft.auth.mode === 'basic' ? '临时密码' : '临时 Token'"
              />
            </div>
            <p class="text-caption text-secondary dark:text-secondary-dark">
              临时认证仅保留在当前页签；保存接口只保存凭证库引用。
            </p>
          </template>
        </div>
        <div v-if="tab === 'body'" class="space-y-[8px]">
          <UiSelect
            :model-value="draft.bodyMode"
            size="sm"
            class="w-[190px]"
            title="请求体格式"
            :options="[
              { value: 'none', label: '无请求体' },
              { value: 'json', label: 'JSON' },
              { value: 'text', label: 'Text' },
              { value: 'form', label: 'Form URL Encoded' },
            ]"
            @update:model-value="draft.bodyMode = $event as BodyMode"
          />
          <RequestRows v-if="draft.bodyMode === 'form'" v-model="draft.form" label="添加字段" />
          <UiCodeEditor
            v-else-if="draft.bodyMode !== 'none'"
            v-model="draft.body"
            :language="draft.bodyMode === 'json' ? 'json' : 'text'"
            height="160px"
            mode="minimal"
            placeholder="请求体内容"
            @save="$emit('save')"
          />
          <p v-else class="text-caption text-secondary dark:text-secondary-dark">
            此请求不发送请求体。
          </p>
        </div>
        <div v-if="tab === 'settings'" class="space-y-[8px]">
          <div class="flex items-center gap-[8px]">
            <span class="text-body-sm">{{ draft.type === 'http' ? '请求超时' : '连接超时' }}</span
            ><UiSelect
              :model-value="String(draft.timeoutMs)"
              size="sm"
              class="w-[120px]"
              :options="[
                { value: '5000', label: '5 秒' },
                { value: '15000', label: '15 秒' },
                { value: '30000', label: '30 秒' },
                { value: '60000', label: '60 秒' },
              ]"
              @update:model-value="draft.timeoutMs = Number($event)"
            />
          </div>
          <p class="text-caption text-secondary dark:text-secondary-dark">
            {{
              draft.type === 'http'
                ? '校验 TLS 证书，跟随重定向；文本响应上限 20 MiB。'
                : '连接成功后持续接收，切换页签不断开；不自动重连。'
            }}
          </p>
        </div>
      </div>
    </UiScrollArea>
  </section>
</template>
