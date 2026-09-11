<script setup lang="ts">
/**
 * ProxyEntryCard · 单个代理条目表单（`[[proxies]]`）
 * 按代理类型显隐字段（tcp/udp 要远端端口、http/https 要域名、stcp 要密钥），
 * 未知字段由上游 merge 保留，这里只呈现表单覆盖的字段。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiButton, UiIcon, UiIconButton, UiInput, UiSelect, UiSwitch } from '@/core/ui'
import {
  NEEDS_CUSTOM_DOMAINS,
  NEEDS_REMOTE_PORT,
  NEEDS_SECRET_KEY,
  PROXY_TYPE_OPTIONS,
  type FrpFormProxy,
} from './frpForm'

const props = defineProps<{
  /** 代理模型（v-model） */
  modelValue: FrpFormProxy
  /** 在列表中的序号（标题展示） */
  index: number
}>()
const emit = defineEmits<{
  'update:modelValue': [value: FrpFormProxy]
  remove: []
}>()

const { t } = useI18n()

/** 写入单个字段（不可变更新，触发响应式） */
function setField<K extends keyof FrpFormProxy>(key: K, value: FrpFormProxy[K]) {
  emit('update:modelValue', { ...props.modelValue, [key]: value })
}

/** 数字字段写入（空串视为未填写 → null） */
function setNumber(key: 'localPort' | 'remotePort', value: string | number) {
  const text = String(value).trim()
  setField(key, text === '' ? null : Number(text))
}

/** 代理类型选项 */
const typeOptions = computed(() =>
  PROXY_TYPE_OPTIONS.map((value) => ({ value, label: value.toUpperCase() }))
)

/** 是否需要远端端口 */
const needsRemotePort = computed(() => NEEDS_REMOTE_PORT.includes(props.modelValue.type))
/** 是否需要自定义域名 */
const needsDomains = computed(() => NEEDS_CUSTOM_DOMAINS.includes(props.modelValue.type))
/** 是否需要密钥 */
const needsSecret = computed(() => NEEDS_SECRET_KEY.includes(props.modelValue.type))
</script>

<template>
  <div
    class="flex flex-col gap-[8px] rounded-sm border border-border p-[10px] dark:border-border-dark"
  >
    <!-- 头部：类型 + 名称 + 启用开关 + 删除 -->
    <div class="flex items-center gap-[8px]">
      <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
        {{ t('frp.formProxyIndex', { index: props.index + 1 }) }}
      </span>
      <UiSelect
        :model-value="props.modelValue.type"
        size="sm"
        class="w-[104px] shrink-0"
        :options="typeOptions"
        @update:model-value="setField('type', String($event))"
      />
      <UiInput
        :model-value="props.modelValue.name"
        size="sm"
        class="min-w-0 flex-1"
        :placeholder="t('frp.formProxyName')"
        @update:model-value="setField('name', String($event))"
      />
      <UiSwitch
        :model-value="props.modelValue.enabled"
        size="sm"
        :title="t('frp.formProxyEnabled')"
        @update:model-value="setField('enabled', Boolean($event))"
      />
      <UiIconButton :label="t('frp.formProxyRemove')" size="xs" @click="emit('remove')">
        <UiIcon name="trash" :size="14" />
      </UiIconButton>
    </div>

    <!-- 地址与端口 -->
    <div class="grid grid-cols-2 gap-[8px]">
      <UiInput
        :model-value="props.modelValue.localIP"
        size="sm"
        :placeholder="t('frp.formProxyLocalIp')"
        @update:model-value="setField('localIP', String($event))"
      />
      <UiInput
        :model-value="props.modelValue.localPort ?? ''"
        size="sm"
        :placeholder="t('frp.formProxyLocalPort')"
        @update:model-value="setNumber('localPort', $event)"
      />
      <UiInput
        v-if="needsRemotePort"
        :model-value="props.modelValue.remotePort ?? ''"
        size="sm"
        class="col-span-2"
        :placeholder="t('frp.formProxyRemotePort')"
        @update:model-value="setNumber('remotePort', $event)"
      />
      <UiInput
        v-if="needsDomains"
        :model-value="props.modelValue.customDomains"
        size="sm"
        class="col-span-2"
        :placeholder="t('frp.formProxyDomains')"
        @update:model-value="setField('customDomains', String($event))"
      />
      <UiInput
        v-if="needsSecret"
        :model-value="props.modelValue.secretKey"
        type="password"
        size="sm"
        class="col-span-2"
        :placeholder="t('frp.formProxySecretKey')"
        @update:model-value="setField('secretKey', String($event))"
      />
    </div>

    <p class="text-caption text-text-muted dark:text-text-muted-dark">
      {{ t('frp.formProxyAdvancedHint') }}
    </p>

    <div class="flex justify-end">
      <UiButton size="xs" variant="ghost" @click="emit('remove')">
        {{ t('frp.formProxyRemove') }}
      </UiButton>
    </div>
  </div>
</template>
