<script setup lang="ts">
import { UiTooltip } from '@/core/ui'
/**
 * ApiSidebar · 接口列表侧栏（Postman Collections 式：保存的接口随时切换）
 */
import type { ApiRecord } from './contracts'
import { formatRelativeTime, methodBadgeClass } from './useHttp'
import { UiBadge, UiButton, UiIcon, UiIconButton, UiListRow } from '@/core/ui'

defineProps<{
  apis: ApiRecord[]
  activeId: number | null
}>()

const emit = defineEmits<{
  (e: 'select', r: ApiRecord): void
  (e: 'rename', r: ApiRecord): void
  (e: 'delete', id: number): void
  (e: 'new'): void
}>()
</script>

<template>
  <div class="flex w-[240px] shrink-0 flex-col border-r border-border dark:border-border-dark">
    <div class="flex shrink-0 items-center justify-between px-[12px] py-[10px]">
      <span class="text-caption font-medium text-text-muted dark:text-text-muted-dark">
        接口列表（{{ apis.length }}）
      </span>
      <div class="flex items-center gap-[4px]">
        <UiButton
          variant="secondary"
          size="xs"
          title="新建接口（清空当前表单）"
          @click="emit('new')"
        >
          + 新建
        </UiButton>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto px-[6px] pb-[8px]">
      <UiTooltip v-for="a in apis" :key="a.id" :content="`${a.method} ${a.url}\n${a.updatedAt}`">
        <UiListRow
          class="group"
          cursor="pointer"
          :active="a.id === activeId"
          @click="emit('select', a)"
        >
          <div class="flex min-w-0 flex-1 flex-col gap-[3px]">
            <div class="flex items-center gap-[8px]">
              <UiBadge
                size="xs"
                class="w-[46px] shrink-0 rounded-[4px] px-[4px] py-[1px] text-center font-mono text-caption font-medium"
                :class="methodBadgeClass(a.method, a.type)"
                >{{ a.type === 'ws' ? 'WS' : a.method }}</UiBadge
              >
              <span
                class="min-w-0 flex-1 truncate text-body font-medium text-primary dark:text-primary-dark"
              >
                {{ a.name || '(未命名)' }}
              </span>
              <UiIconButton
                label="重命名接口"
                size="xs"
                class="hidden shrink-0 text-text-muted hover:text-info-strong group-hover:inline-flex dark:text-text-muted-dark dark:hover:text-info-dark"
                @click.stop="emit('rename', a)"
              >
                <UiIcon name="pencil" :size="12" />
              </UiIconButton>
              <UiIconButton
                label="删除接口"
                size="xs"
                class="hidden shrink-0 text-text-muted hover:text-tertiary-strong group-hover:inline-flex dark:text-text-muted-dark dark:hover:text-tertiary-dark"
                @click.stop="emit('delete', a.id)"
              >
                <UiIcon name="trash" :size="12" />
              </UiIconButton>
            </div>
            <div class="flex items-center gap-[6px]">
              <span class="truncate font-mono text-body-sm text-secondary dark:text-secondary-dark">
                {{ a.url }}
              </span>
              <span class="ml-auto shrink-0 text-caption text-text-muted dark:text-text-muted-dark">
                {{ formatRelativeTime(a.updatedAt) }}
              </span>
            </div>
          </div>
        </UiListRow>
      </UiTooltip>

      <p
        v-if="!apis.length"
        class="px-[8px] py-[16px] text-center text-body-sm text-text-muted dark:text-text-muted-dark"
      >
        暂无接口<br />填写请求后点「保存」加入列表
      </p>
    </div>
  </div>
</template>
