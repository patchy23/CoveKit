<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
/**
 * ClientManagerDialog · frpc 客户端管理（xl 弹窗，不随遮罩关闭以防误触）
 * 两个添加入口：引用外部已有可执行文件（只登记路径不复制，适合自编译产物持续更新）
 * 与一键下载官方版本。清单里可设默认客户端、移除登记（只删记录不删文件，移除前二次确认）。
 */
import { computed, inject, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { UiConfirmDialog } from '@/core/ui'
import { UiAlert, UiButton, UiEmptyState, UiModal, UiSpinner } from '@/core/ui'
import { useUiStore } from '@/stores/ui'
import { FRP_CLIENTS_KEY } from './context'
import { hasUsableClient } from './frpClient'
import ClientDownloadPanel from './ClientDownloadPanel.vue'
import ClientRow from './ClientRow.vue'

const props = defineProps<{
  /** 是否可见 */
  open: boolean
}>()
const emit = defineEmits<{
  /** 清单或默认项变化（工作台刷新档案列表的绑定展示） */
  changed: []
  close: []
}>()

const { t } = useI18n()
const ui = useUiStore()
// inject 的返回值类型是 `T | undefined`，且类型收窄不跨闭包，
// 因此先收到局部常量，后续 computed 与事件处理里都不必再判空
const injected = inject(FRP_CLIENTS_KEY)
if (!injected) {
  throw new Error('FRP 客户端状态未注入（应由 FrpWorkbench 提供）')
}
const clients = injected

/** 待移除的客户端（null = 无确认弹窗） */
const pendingRemove = ref<string | null>(null)

/** 清单里至少有一个可用文件（没有时给出显式提醒，避免用户以为已配好） */
const usable = computed(() => hasUsableClient(clients.clients.value))

/** 引用外部已有可执行文件（用户自己维护的目录，不复制进工具目录） */
async function pickExternal() {
  const selected = await dialogOpen({
    multiple: false,
    directory: false,
    title: t('frp.clientPickTitle'),
  })
  if (typeof selected !== 'string') return
  if (await clients.addExternal(selected)) emit('changed')
}

/** 设为默认客户端 */
async function onSetDefault(id: string) {
  if (await clients.setDefault(id)) emit('changed')
}

/** 确认移除登记（只删记录，磁盘文件保持原样） */
async function onRemoveConfirmed() {
  const id = pendingRemove.value
  pendingRemove.value = null
  if (id === null) return
  if (await clients.remove(id)) emit('changed')
}

/** 下载完成：刷新清单并通知工作台（新客户端可立即被档案绑定） */
async function onInstalled(version: string) {
  await clients.refresh()
  ui.toast(t('frp.clientDownloaded', { version }))
  emit('changed')
}
</script>

<template>
  <UiModal :open="props.open" :title="t('frp.clientManagerTitle')" size="xl" @close="emit('close')">
    <p class="text-body-sm text-text-muted dark:text-text-muted-dark">
      {{ t('frp.clientManagerHint') }}
    </p>

    <!-- 添加入口：引用外部文件 + 下载官方版本 -->
    <div class="mt-[12px] flex items-start gap-[10px]">
      <UiButton size="sm" variant="secondary" @click="pickExternal">
        {{ t('frp.clientPickButton') }}
      </UiButton>
      <span class="mt-[6px] shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
        {{ t('frp.clientPickHint') }}
      </span>
    </div>
    <div class="mt-[10px]">
      <ClientDownloadPanel @installed="onInstalled" />
    </div>

    <div class="my-[12px] border-t border-border dark:border-border-dark" />

    <!-- 清单 -->
    <div class="flex items-center justify-between gap-[8px]">
      <span class="text-body-sm font-medium dark:text-primary-dark">
        {{ t('frp.clientListTitle', { count: clients.clients.value.length }) }}
      </span>
      <UiSpinner v-if="clients.loading.value" size="sm" />
    </div>

    <p
      v-if="clients.error.value !== ''"
      class="select-text mt-[6px] text-body-sm text-danger-strong dark:text-danger-dark"
    >
      {{ clients.error.value }}
    </p>

    <div v-else-if="clients.clients.value.length === 0" class="mt-[8px]">
      <UiEmptyState :title="t('frp.clientEmpty')" :description="t('frp.clientEmptyHint')" />
    </div>

    <UiScrollArea v-else as-child axis="vertical">
      <div class="mt-[6px] flex max-h-[300px] flex-col gap-[4px]">
        <ClientRow
          v-for="item in clients.clients.value"
          :key="item.id"
          :client="item"
          @set-default="onSetDefault"
          @remove="pendingRemove = $event"
        />
      </div>
    </UiScrollArea>

    <!-- 无可用文件时显式提醒：清单里有记录不等于能启动 -->
    <UiAlert v-if="!usable && clients.clients.value.length > 0" class="mt-[10px]" tone="warning">
      {{ t('frp.clientNoneUsable') }}
    </UiAlert>

    <div class="mt-[14px] flex justify-end">
      <UiButton variant="secondary" @click="emit('close')">
        {{ t('frp.dialogClose') }}
      </UiButton>
    </div>

    <!-- 移除确认：文案写明「不会删除磁盘上的文件」 -->
    <UiConfirmDialog
      :open="pendingRemove !== null"
      danger
      :title="t('frp.clientRemoveTitle')"
      :message="t('frp.clientRemoveMessage')"
      :confirm-label="t('frp.clientRemove')"
      @confirm="onRemoveConfirmed"
      @close="pendingRemove = null"
    />
  </UiModal>
</template>
