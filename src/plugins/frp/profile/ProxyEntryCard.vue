<script setup lang="ts">
/**
 * ProxyEntryCard · 单个代理条目表单（`[[proxies]]`）
 *
 * 每个字段都带 label（此前只有 placeholder，填完值就分不清哪个框是 localIP 还是 remotePort）；
 * 地址 + 端口同行、端口走窄列；按代理类型显隐字段（tcp/udp 要远端端口、http/https 要域名、
 * stcp 要密钥）。删除入口只保留标题行的图标按钮，去掉底部重复的文字按钮。
 * 未知字段由上游 merge 保留，这里只呈现表单覆盖的字段。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiField, UiIcon, UiIconButton, UiInput, UiSelect, UiSwitch } from '@/core/ui'
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
  <div class="border-b border-border pb-[16px] last:border-b-0 dark:border-border-dark">
    <!-- 标题行：序号 + 启用开关 + 删除（删除只此一处） -->
    <div class="mb-[10px] flex items-center gap-[8px]">
      <span class="min-w-0 flex-1 text-caption font-medium text-secondary dark:text-secondary-dark">
        {{ t('frp.formProxyIndex', { index: props.index + 1 }) }}
      </span>
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

    <div class="flex flex-col gap-[8px]">
      <!-- 类型 + 名称 -->
      <div class="grid grid-cols-[110px_1fr] gap-[8px]">
        <UiField :label="t('frp.formProxyType')" size="sm">
          <UiSelect
            :model-value="props.modelValue.type"
            size="sm"
            :options="typeOptions"
            @update:model-value="setField('type', String($event))"
          />
        </UiField>
        <UiField :label="t('frp.formProxyName')" size="sm">
          <UiInput
            :model-value="props.modelValue.name"
            size="sm"
            @update:model-value="setField('name', String($event))"
          />
        </UiField>
      </div>

      <!-- 本地地址 + 本地端口 -->
      <div class="grid grid-cols-[1fr_120px] gap-[8px]">
        <UiField :label="t('frp.formProxyLocalIp')" size="sm">
          <UiInput
            :model-value="props.modelValue.localIP"
            size="sm"
            :placeholder="t('frp.formProxyLocalIpPlaceholder')"
            @update:model-value="setField('localIP', String($event))"
          />
        </UiField>
        <UiField :label="t('frp.formProxyLocalPort')" size="sm">
          <UiInput
            type="number"
            :model-value="props.modelValue.localPort ?? ''"
            class="frp-port-input"
            size="sm"
            @update:model-value="setNumber('localPort', $event)"
          />
        </UiField>
      </div>

      <!-- 按类型显隐 -->
      <div v-if="needsRemotePort" class="grid grid-cols-[120px_1fr] items-end gap-[8px]">
        <UiField :label="t('frp.formProxyRemotePort')" size="sm">
          <UiInput
            type="number"
            :model-value="props.modelValue.remotePort ?? ''"
            class="frp-port-input"
            size="sm"
            @update:model-value="setNumber('remotePort', $event)"
          />
        </UiField>
        <p class="pb-[5px] text-caption text-text-muted dark:text-text-muted-dark">
          {{ t('frp.formProxyRemotePortHint') }}
        </p>
      </div>
      <UiField
        v-if="needsDomains"
        :label="t('frp.formProxyDomains')"
        :description="t('frp.formProxyDomainsHint')"
        size="sm"
      >
        <UiInput
          :model-value="props.modelValue.customDomains"
          size="sm"
          :placeholder="t('frp.formProxyDomainsPlaceholder')"
          @update:model-value="setField('customDomains', String($event))"
        />
      </UiField>
      <UiField v-if="needsSecret" :label="t('frp.formProxySecretKey')" size="sm">
        <UiInput
          type="password"
          :model-value="props.modelValue.secretKey"
          size="sm"
          :placeholder="t('frp.formProxySecretKeyPlaceholder')"
          @update:model-value="setField('secretKey', String($event))"
        />
      </UiField>
    </div>
  </div>
</template>
