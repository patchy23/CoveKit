<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * ProfileFormEditor · 表单模式
 *
 * 排版目标：**常用字段一屏配完，其余收进「更多配置」**（默认值即可用，多数时候不会改）。
 *  - 常用：服务端地址 + 端口、认证方式 + Token、代理列表
 *  - 更多配置（默认收起）：用户标识、传输协议、连接池大小、TLS、TLS ServerName、日志等级
 * 容器统一用 UiPanel（分节卡片），相关字段同行、数值走窄列，避免大片空白。
 * 覆盖服务器 / 认证 / 传输与日志 + 代理列表；未覆盖字段（healthCheck、metadatas、自定义段落）
 * 由 frpForm.mergeFormModel 在上游保留，这里只提示「高级字段请用源码模式」。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiButton, UiField, UiInput, UiPanel, UiSelect, UiSwitch } from '@/core/ui'
import { CredentialPicker } from '@/core/vault'
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
  <UiScrollArea as-child axis="vertical">
    <div class="h-full min-h-0 p-[12px]">
      <div class="flex flex-col gap-[12px]">
        <!-- 服务器（常用） -->
        <UiPanel :title="t('frp.formSectionServer')" padding="sm">
          <div class="grid grid-cols-[1fr_120px] gap-[10px]">
            <UiField :label="t('frp.formServerAddr')" size="sm" required>
              <UiInput
                size="sm"
                :model-value="props.modelValue.serverAddr"
                :placeholder="t('frp.formServerAddrPlaceholder')"
                @update:model-value="setField('serverAddr', String($event))"
              />
            </UiField>
            <UiField :label="t('frp.formServerPort')" size="sm">
              <UiInput
                size="sm"
                class="frp-port-input"
                type="number"
                :model-value="props.modelValue.serverPort ?? ''"
                placeholder="7000"
                @update:model-value="setNumber('serverPort', $event)"
              />
            </UiField>
          </div>
        </UiPanel>

        <!-- 认证（常用） -->
        <UiPanel :title="t('frp.formSectionAuth')" padding="sm">
          <div class="flex flex-col gap-[10px]">
            <UiField :label="t('frp.formAuthMethod')" size="sm">
              <UiSelect
                size="sm"
                :model-value="props.modelValue.authMethod"
                :options="authOptions"
                @update:model-value="
                  emit('update:modelValue', {
                    ...props.modelValue,
                    authMethod: String($event),
                    authCredentialId: $event === 'token' ? props.modelValue.authCredentialId : '',
                  })
                "
              />
            </UiField>
            <UiField
              v-if="props.modelValue.authMethod === 'token'"
              :label="t('frp.formTokenCredential')"
              size="sm"
            >
              <CredentialPicker
                :model-value="props.modelValue.authCredentialId"
                kind="api-token"
                size="sm"
                @update:model-value="
                  emit('update:modelValue', {
                    ...props.modelValue,
                    authCredentialId: $event,
                    authToken: '',
                  })
                "
              />
              <p class="text-caption text-text-muted dark:text-text-muted-dark">
                {{ t('frp.formTokenCredentialHint') }}
              </p>
            </UiField>
            <UiField
              v-if="props.modelValue.authMethod === 'token' && !props.modelValue.authCredentialId"
              :label="t('frp.formAuthToken')"
              :description="t('frp.formAuthTokenHint')"
              size="sm"
            >
              <UiInput
                size="sm"
                :model-value="props.modelValue.authToken"
                type="password"
                @update:model-value="setField('authToken', String($event))"
              />
            </UiField>
          </div>
        </UiPanel>

        <!-- 更多配置（默认收起，保持默认值即可） -->
        <UiPanel
          collapsible
          :default-open="false"
          :title="t('frp.formSectionAdvanced')"
          :description="t('frp.formSectionAdvancedHint')"
          padding="sm"
        >
          <div class="flex flex-col gap-[10px]">
            <UiField :label="t('frp.formUser')" size="sm">
              <UiInput
                size="sm"
                :model-value="props.modelValue.user"
                @update:model-value="setField('user', String($event))"
              />
            </UiField>
            <div class="grid grid-cols-[1fr_120px] gap-[10px]">
              <UiField :label="t('frp.formProtocol')" size="sm">
                <UiSelect
                  size="sm"
                  :model-value="props.modelValue.protocol"
                  :options="protocolOptions"
                  @update:model-value="setField('protocol', String($event))"
                />
              </UiField>
              <UiField
                :label="t('frp.formPoolCount')"
                :description="t('frp.formPoolCountHint')"
                size="sm"
              >
                <UiInput
                  size="sm"
                  type="number"
                  :model-value="props.modelValue.poolCount ?? ''"
                  @update:model-value="setNumber('poolCount', $event)"
                />
              </UiField>
            </div>
            <!-- 开关与字段不同构：做成左标签右开关的一行，避免独占半列留出大片空白 -->
            <div class="flex items-center justify-between gap-[10px]">
              <span class="field-label text-body-sm">{{ t('frp.formTls') }}</span>
              <UiSwitch
                size="sm"
                :model-value="props.modelValue.tlsEnable"
                @update:model-value="setField('tlsEnable', Boolean($event))"
              />
            </div>
            <UiField :label="t('frp.formTlsServerName')" size="sm">
              <UiInput
                size="sm"
                :model-value="props.modelValue.tlsServerName"
                @update:model-value="setField('tlsServerName', String($event))"
              />
            </UiField>
            <UiField :label="t('frp.formLogLevel')" size="sm">
              <UiSelect
                size="sm"
                :model-value="props.modelValue.logLevel"
                :options="logLevelOptions"
                @update:model-value="setField('logLevel', String($event))"
              />
            </UiField>
          </div>
        </UiPanel>

        <!-- 代理列表（核心，始终展开） -->
        <UiPanel
          :title="t('frp.formSectionProxies', { count: props.modelValue.proxies.length })"
          padding="sm"
        >
          <template #actions>
            <UiButton size="xs" @click="addProxy">{{ t('frp.formProxyAdd') }}</UiButton>
          </template>
          <p
            v-if="props.modelValue.proxies.length === 0"
            class="text-body-sm text-text-muted dark:text-text-muted-dark"
          >
            {{ t('frp.formProxyEmpty') }}
          </p>
          <div v-else class="flex flex-col gap-[10px]">
            <ProxyEntryCard
              v-for="(proxy, index) in props.modelValue.proxies"
              :key="index"
              :model-value="proxy"
              :index="index"
              @update:model-value="updateProxy(index, $event)"
              @remove="removeProxy(index)"
            />
          </div>
        </UiPanel>

        <p class="text-caption text-text-muted dark:text-text-muted-dark">
          {{ t('frp.formAdvancedHint') }}
        </p>
      </div>
    </div>
  </UiScrollArea>
</template>

<style scoped>
/* 端口保留数字输入语义，但不显示逐个增减的步进按钮；同时覆盖代理条目。 */
:deep(.frp-port-input) {
  appearance: textfield;
}

:deep(.frp-port-input::-webkit-inner-spin-button),
:deep(.frp-port-input::-webkit-outer-spin-button) {
  margin: 0;
  -webkit-appearance: none;
}
</style>
