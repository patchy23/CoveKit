<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/**
 * ClientPicker · 档案详情里的客户端选择器
 * 「跟随默认」是显式选项（空值），避免用户以为必须逐个手选；
 * 绑定项的文件被删时不静默回落，而是把原因显示出来——服务端有版本限制时，
 * 「悄悄换了客户端」比启动失败更难排查。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiSelect } from '@/core/ui'
import type { FrpClient } from '../contracts'
import {
  clientOptions,
  effectiveClient,
  clientTitle,
  FOLLOW_DEFAULT_VALUE,
  toBoundId,
  toSelectValue,
} from './frpClient'

const props = defineProps<{
  /** 已登记的客户端清单 */
  clients: FrpClient[]
  /** 默认客户端 id */
  defaultId?: string
  /** 本档案绑定的客户端 id（空串 / 未定义 = 跟随默认） */
  boundId?: string
  /** 运行中或保存中时禁止切换 */
  disabled: boolean
}>()
const emit = defineEmits<{
  /** 绑定变化（空串 = 跟随默认） */
  change: [clientId: string]
}>()

const { t } = useI18n()

/** 下拉选项：「跟随默认」置首，其余按默认项优先排列 */
const options = computed(() => [
  { value: FOLLOW_DEFAULT_VALUE, label: t('frp.clientFollowDefault') },
  ...clientOptions(props.clients, props.defaultId, t),
])

/** 当前选中的值；未绑定时显示「跟随默认」的哨兵项，变更时再换算回绑定 id */
const current = computed({
  get: () => toSelectValue(props.boundId),
  set: (value: string) => emit('change', toBoundId(value)),
})

/** 当前实际生效的客户端（未绑定时即默认项） */
const effective = computed(() =>
  effectiveClient(props.clients, props.defaultId, props.boundId ?? undefined)
)

/** 绑定项的文件已消失：给可见提示，不要等启动时才报错 */
const missing = computed(() => {
  const bound = (props.boundId ?? '') !== ''
  return bound && effective.value !== undefined && !effective.value.exists
})

/** 实际生效的提示文案 */
const effectiveText = computed(() => {
  const client = effective.value
  if (client === undefined) return t('frp.clientNoneAvailable')
  return t('frp.clientEffective', { name: clientTitle(client) })
})
</script>

<template>
  <div class="flex min-w-0 items-center gap-[6px]">
    <span class="shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
      {{ t('frp.clientLabel') }}
    </span>
    <UiSelect
      v-model="current"
      class="w-[190px]"
      size="sm"
      :disabled="props.disabled"
      :options="options"
    />
    <UiTooltip :content="effectiveText">
      <span
        class="min-w-0 max-w-[160px] truncate text-caption"
        :class="
          missing
            ? 'text-danger-strong dark:text-danger-dark'
            : 'text-text-muted dark:text-text-muted-dark'
        "
      >
        {{ missing ? t('frp.clientBoundMissing') : effectiveText }}
      </span>
    </UiTooltip>
  </div>
</template>
