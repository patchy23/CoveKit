<script setup lang="ts">
/**
 * ProfileFormEditor · 表单模式
 * 覆盖服务器 / 认证 / 传输 / 日志 + 代理列表；未覆盖字段（healthCheck、metadatas、自定义段落）
 * 由 frpForm.mergeFormModel 在上游保留，这里只提示「高级字段请用源码模式」。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiButton, UiField, UiInput, UiSelect, UiSwitch } from '@/core/ui'
import ProxyEntryCard from './ProxyEntryCard.vue'
import {
  AUTH_METHOD_OPTIONS,
  LOG_LEVEL_OPTIONS,
  PROTOCOL_OPTIONS,
  emptyProxy,
  type FrpFormModel,
  type FrpFormProxy,
} from './frpForm'

const props = defineProps<{
  /** 表单模型（v-model） */
  modelValue: FrpFormModel
}>()
const emit = defineEmits<{ 'update:modelValue': [value: FrpFormModel] }>()

const { t } = useI18n()

/** 写入单个顶层字段（不可变更新） */
function setField<K extends keyof FrpFormModel>(key: K, value: FrpFormModel[K]) {
  emit('update:modelValue', { ...props.modelValue, [key]: value })
}

/** 数字字段写入（空串 → null，表示未填写） */
function setNumber(key: 'serverPort' | 'poolCount', value: string | number) {
  const text = String(value).trim()
  setField(key, text === '' ? null : Number(text))
}

/** 更新第 index 个代理 */
function updateProxy(index: number, proxy: FrpFormProxy) {
  const next = [...props.modelValue.proxies]
  next[index] = proxy
  setField('proxies', next)
}

/** 追加一个空代理（默认 tcp） */
function addProxy() {
  setField('proxies', [...props.modelValue.proxies, emptyProxy()])
}

/** 移除第 index 个代理 */
function removeProxy(index: number) {
  setField(
    'proxies',
    props.modelValue.proxies.filter((_, current) => current !== index)
  )
}

/** 协议 / 认证 / 日志等级选项 */
const protocolOptions = computed(() =>
  PROTOCOL_OPTIONS.map((value) => ({ value, label: value.toUpperCase() }))
)
const authOptions = computed(() =>
  AUTH_METHOD_OPTIONS.map((value) => ({ value, label: value.toUpperCase() }))
)
const logLevelOptions = computed(() => LOG_LEVEL_OPTIONS.map((value) => ({ value, label: value })))
</script>

<template>
  <div class="h-full min-h-0 overflow-y-auto px-[12px] py-[10px]">
    <!-- 服务器 -->
    <h4 class="mb-[8px] text-body-sm font-semibold dark:text-primary-dark">
      {{ t('frp.formSectionServer') }}
    </h4>
    <div class="grid grid-cols-2 gap-[10px]">
      <UiField :label="t('frp.formServerAddr')" required>
        <UiInput
          :model-value="props.modelValue.serverAddr"
          :placeholder="t('frp.formServerAddrPlaceholder')"
          @update:model-value="setField('serverAddr', String($event))"
        />
      </UiField>
      <UiField :label="t('frp.formServerPort')">
        <UiInput
          :model-value="props.modelValue.serverPort ?? ''"
          placeholder="7000"
          @update:model-value="setNumber('serverPort', $event)"
        />
      </UiField>
      <UiField :label="t('frp.formUser')" class="col-span-2">
        <UiInput
          :model-value="props.modelValue.user"
          :placeholder="t('frp.formOptional')"
          @update:model-value="setField('user', String($event))"
        />
      </UiField>
    </div>

    <!-- 认证 -->
    <h4 class="mb-[8px] mt-[16px] text-body-sm font-semibold dark:text-primary-dark">
      {{ t('frp.formSectionAuth') }}
    </h4>
    <div class="grid grid-cols-2 gap-[10px]">
      <UiField :label="t('frp.formAuthMethod')">
        <UiSelect
          :model-value="props.modelValue.authMethod"
          :options="authOptions"
          @update:model-value="setField('authMethod', String($event))"
        />
      </UiField>
      <UiField :label="t('frp.formAuthToken')" :description="t('frp.formAuthTokenHint')">
        <UiInput
          :model-value="props.modelValue.authToken"
          type="password"
          @update:model-value="setField('authToken', String($event))"
        />
      </UiField>
    </div>

    <!-- 传输 -->
    <h4 class="mb-[8px] mt-[16px] text-body-sm font-semibold dark:text-primary-dark">
      {{ t('frp.formSectionTransport') }}
    </h4>
    <div class="grid grid-cols-2 gap-[10px]">
      <UiField :label="t('frp.formProtocol')">
        <UiSelect
          :model-value="props.modelValue.protocol"
          :options="protocolOptions"
          @update:model-value="setField('protocol', String($event))"
        />
      </UiField>
      <UiField :label="t('frp.formPoolCount')" :description="t('frp.formPoolCountHint')">
        <UiInput
          :model-value="props.modelValue.poolCount ?? ''"
          @update:model-value="setNumber('poolCount', $event)"
        />
      </UiField>
      <UiField :label="t('frp.formTls')">
        <UiSwitch
          :model-value="props.modelValue.tlsEnable"
          @update:model-value="setField('tlsEnable', Boolean($event))"
        />
      </UiField>
      <UiField :label="t('frp.formTlsServerName')">
        <UiInput
          :model-value="props.modelValue.tlsServerName"
          :placeholder="t('frp.formOptional')"
          @update:model-value="setField('tlsServerName', String($event))"
        />
      </UiField>
      <UiField :label="t('frp.formLogLevel')">
        <UiSelect
          :model-value="props.modelValue.logLevel"
          :options="logLevelOptions"
          @update:model-value="setField('logLevel', String($event))"
        />
      </UiField>
    </div>

    <!-- 代理列表 -->
    <div class="mb-[8px] mt-[16px] flex items-center gap-[8px]">
      <h4 class="min-w-0 flex-1 text-body-sm font-semibold dark:text-primary-dark">
        {{ t('frp.formSectionProxies', { count: props.modelValue.proxies.length }) }}
      </h4>
      <UiButton size="xs" @click="addProxy">{{ t('frp.formProxyAdd') }}</UiButton>
    </div>
    <div
      v-if="props.modelValue.proxies.length === 0"
      class="py-[8px] text-body-sm text-text-muted dark:text-text-muted-dark"
    >
      {{ t('frp.formProxyEmpty') }}
    </div>
    <div class="flex flex-col gap-[8px]">
      <ProxyEntryCard
        v-for="(proxy, index) in props.modelValue.proxies"
        :key="index"
        :model-value="proxy"
        :index="index"
        @update:model-value="updateProxy(index, $event)"
        @remove="removeProxy(index)"
      />
    </div>

    <p class="mt-[12px] text-caption text-text-muted dark:text-text-muted-dark">
      {{ t('frp.formAdvancedHint') }}
    </p>
  </div>
</template>
