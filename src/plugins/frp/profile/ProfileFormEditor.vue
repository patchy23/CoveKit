<script setup lang="ts">
/**
 * ProfileFormEditor · 表单模式
 *
 * 排版：每个分节是一张带边框的卡片（标题 + 左侧色条），节内字段一律 UiField 带 label、
 * 竖向 gap-[10px]；相关字段同行（地址 + 端口、协议 + 连接池），数值走窄列避免大片空白。
 * 覆盖服务器 / 认证 / 传输与日志 + 代理列表；未覆盖字段（healthCheck、metadatas、自定义段落）
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
  <div class="h-full min-h-0 overflow-y-auto p-[12px]">
    <div class="flex flex-col gap-[12px]">
      <!-- 服务器 -->
      <section class="rounded-md border border-border p-[12px] dark:border-border-dark">
        <h4
          class="mb-[10px] flex items-center gap-[6px] text-body-sm font-semibold text-primary dark:text-primary-dark"
        >
          <span class="h-[12px] w-[2px] shrink-0 rounded-full bg-tertiary" />
          {{ t('frp.formSectionServer') }}
        </h4>
        <div class="flex flex-col gap-[10px]">
          <div class="grid grid-cols-[1fr_120px] gap-[10px]">
            <UiField :label="t('frp.formServerAddr')" required>
              <UiInput
                :model-value="props.modelValue.serverAddr"
                :placeholder="t('frp.formServerAddrPlaceholder')"
                @update:model-value="setField('serverAddr', String($event))"
              />
            </UiField>
            <UiField :label="t('frp.formServerPort')">
              <UiInput
                type="number"
                :model-value="props.modelValue.serverPort ?? ''"
                placeholder="7000"
                @update:model-value="setNumber('serverPort', $event)"
              />
            </UiField>
          </div>
          <UiField :label="t('frp.formUser')">
            <UiInput
              :model-value="props.modelValue.user"
              @update:model-value="setField('user', String($event))"
            />
          </UiField>
        </div>
      </section>

      <!-- 认证 -->
      <section class="rounded-md border border-border p-[12px] dark:border-border-dark">
        <h4
          class="mb-[10px] flex items-center gap-[6px] text-body-sm font-semibold text-primary dark:text-primary-dark"
        >
          <span class="h-[12px] w-[2px] shrink-0 rounded-full bg-tertiary" />
          {{ t('frp.formSectionAuth') }}
        </h4>
        <div class="flex flex-col gap-[10px]">
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
      </section>

      <!-- 传输与日志 -->
      <section class="rounded-md border border-border p-[12px] dark:border-border-dark">
        <h4
          class="mb-[10px] flex items-center gap-[6px] text-body-sm font-semibold text-primary dark:text-primary-dark"
        >
          <span class="h-[12px] w-[2px] shrink-0 rounded-full bg-tertiary" />
          {{ t('frp.formSectionTransport') }}
        </h4>
        <div class="flex flex-col gap-[10px]">
          <div class="grid grid-cols-[1fr_120px] gap-[10px]">
            <UiField :label="t('frp.formProtocol')">
              <UiSelect
                :model-value="props.modelValue.protocol"
                :options="protocolOptions"
                @update:model-value="setField('protocol', String($event))"
              />
            </UiField>
            <UiField :label="t('frp.formPoolCount')" :description="t('frp.formPoolCountHint')">
              <UiInput
                type="number"
                :model-value="props.modelValue.poolCount ?? ''"
                @update:model-value="setNumber('poolCount', $event)"
              />
            </UiField>
          </div>
          <!-- 开关与字段不同构：做成左标签右开关的一行，避免独占半列留出大片空白 -->
          <div class="flex items-center justify-between gap-[10px]">
            <span class="field-label text-body">{{ t('frp.formTls') }}</span>
            <UiSwitch
              :model-value="props.modelValue.tlsEnable"
              @update:model-value="setField('tlsEnable', Boolean($event))"
            />
          </div>
          <UiField :label="t('frp.formTlsServerName')">
            <UiInput
              :model-value="props.modelValue.tlsServerName"
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
      </section>

      <!-- 代理列表 -->
      <section class="rounded-md border border-border p-[12px] dark:border-border-dark">
        <div class="mb-[10px] flex items-center gap-[8px]">
          <h4
            class="flex min-w-0 flex-1 items-center gap-[6px] text-body-sm font-semibold text-primary dark:text-primary-dark"
          >
            <span class="h-[12px] w-[2px] shrink-0 rounded-full bg-tertiary" />
            {{ t('frp.formSectionProxies', { count: props.modelValue.proxies.length }) }}
          </h4>
          <UiButton size="xs" @click="addProxy">{{ t('frp.formProxyAdd') }}</UiButton>
        </div>
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
      </section>

      <p class="text-caption text-text-muted dark:text-text-muted-dark">
        {{ t('frp.formAdvancedHint') }}
      </p>
    </div>
  </div>
</template>
