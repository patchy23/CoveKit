<script setup lang="ts">
/**
 * ClientRow · 客户端清单中的一行
 * 展示版本 / 路径 / 来源与状态标记；动作向上抛出，命令调用与刷新由弹窗统一处理。
 */
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiBadge, UiButton, UiListRow } from '@/core/ui'
import type { FrpClient } from '../contracts'
import { clientTitle, sourceLabelKey } from './frpClient'

const props = defineProps<{
  /** 客户端记录 */
  client: FrpClient
}>()
const emit = defineEmits<{
  /** 设为默认客户端 */
  setDefault: [id: string]
  /** 移除登记（只删记录不删文件） */
  remove: [id: string]
}>()

const { t } = useI18n()

/** 展示标题（版本优先，无版本时退回文件名） */
const title = computed(() => clientTitle(props.client))
</script>

<template>
  <UiListRow class="items-start gap-[10px] px-[10px] py-[8px]">
    <span class="min-w-0 flex-1">
      <span class="flex items-center gap-[6px]">
        <span class="truncate text-body-sm dark:text-primary-dark">{{ title }}</span>
        <UiBadge v-if="props.client.isDefault" tone="success">
          {{ t('frp.clientDefaultTag') }}
        </UiBadge>
        <UiBadge v-if="!props.client.exists" tone="danger">
          {{ t('frp.clientMissingTag') }}
        </UiBadge>
        <UiBadge tone="neutral">{{ t(sourceLabelKey(props.client)) }}</UiBadge>
      </span>
      <span
        class="mt-[2px] block truncate text-caption text-text-muted dark:text-text-muted-dark"
        :title="props.client.path"
      >
        {{ props.client.path }}
      </span>
    </span>
    <span class="flex shrink-0 items-center gap-[4px]">
      <UiButton
        v-if="!props.client.isDefault"
        size="xs"
        variant="ghost"
        @click="emit('setDefault', props.client.id)"
      >
        {{ t('frp.clientSetDefault') }}
      </UiButton>
      <UiButton size="xs" variant="ghost" @click="emit('remove', props.client.id)">
        {{ t('frp.clientRemove') }}
      </UiButton>
    </span>
  </UiListRow>
</template>
