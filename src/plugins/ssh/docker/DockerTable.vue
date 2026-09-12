<script setup lang="ts">
/** DockerTable · SSH Docker 容器列表与行级操作。 */
import type { DockerContainer } from '../contracts'
import { shortContainerId } from '../connection/useSsh'
import { UiButton, UiTable, UiTableCell } from '@/core/ui'

defineProps<{ containers: DockerContainer[]; busyContainerId?: string | null }>()
const emit = defineEmits<{
  (
    event: 'action',
    container: DockerContainer,
    action: 'start' | 'stop' | 'restart' | 'remove'
  ): void
  (event: 'logs', container: DockerContainer): void
  (event: 'terminal', container: DockerContainer): void
}>()

function stateClass(status: string): string {
  if (status === 'running') return 'text-success-strong dark:text-success-dark'
  return 'text-text-muted dark:text-text-muted-dark'
}
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto">
    <UiTable :framed="false" :styled="false" table-class="table-fixed text-body-sm">
      <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
        <tr
          class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
        >
          <UiTableCell as="th" class="w-[92px] whitespace-nowrap px-[8px] py-[8px]"
            >容器 ID</UiTableCell
          >
          <UiTableCell as="th" class="w-[110px] px-[8px] py-[8px]">名称</UiTableCell>
          <UiTableCell as="th" class="w-[110px] px-[8px] py-[8px]">镜像</UiTableCell>
          <UiTableCell as="th" class="w-[70px] whitespace-nowrap px-[8px] py-[8px]"
            >状态</UiTableCell
          >
          <UiTableCell as="th" class="w-[96px] whitespace-nowrap px-[8px] py-[8px]"
            >运行时间</UiTableCell
          >
          <UiTableCell as="th" class="w-[110px] px-[8px] py-[8px]">端口</UiTableCell>
          <UiTableCell as="th" class="w-[210px] whitespace-nowrap px-[8px] py-[8px]"
            >操作</UiTableCell
          >
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="container in containers"
          :key="container.id"
          class="border-b border-border/50 transition-colors dark:border-border-dark/50"
        >
          <UiTableCell
            content="technical"
            class="w-[92px] whitespace-nowrap px-[8px] py-[8px]"
            :title="container.id"
          >
            {{ shortContainerId(container.id) }}
          </UiTableCell>
          <UiTableCell
            content="technical"
            class="truncate px-[8px] py-[8px]"
            :title="container.name"
          >
            {{ container.name }}
          </UiTableCell>
          <UiTableCell
            content="technical"
            class="truncate px-[8px] py-[8px]"
            :title="container.image"
          >
            {{ container.image }}
          </UiTableCell>
          <UiTableCell content="status" class="whitespace-nowrap px-[8px] py-[8px]">
            <span
              v-if="busyContainerId === container.id"
              class="animate-pulse font-sans text-tertiary-strong dark:text-tertiary-dark"
            >
              更新中
            </span>
            <span v-else :class="stateClass(container.status)">{{ container.status }}</span>
          </UiTableCell>
          <UiTableCell
            content="technical"
            class="truncate whitespace-nowrap px-[8px] py-[8px]"
            :title="container.uptime"
          >
            {{ container.uptime }}
          </UiTableCell>
          <UiTableCell
            content="technical"
            class="truncate px-[8px] py-[8px]"
            :title="container.ports"
          >
            {{ container.ports }}
          </UiTableCell>
          <UiTableCell content="action" class="whitespace-nowrap px-[8px] py-[8px]">
            <div class="flex items-center justify-end gap-[2px]">
              <UiButton
                v-if="container.status !== 'running'"
                variant="ghost"
                size="xs"
                @click="emit('action', container, 'start')"
              >
                启动
              </UiButton>
              <UiButton
                v-if="container.status === 'running'"
                variant="ghost"
                size="xs"
                @click="emit('action', container, 'stop')"
              >
                停止
              </UiButton>
              <UiButton
                v-if="container.status === 'running'"
                variant="ghost"
                size="xs"
                @click="emit('action', container, 'restart')"
              >
                重启
              </UiButton>
              <UiButton variant="ghost" size="xs" @click="emit('logs', container)"> 日志 </UiButton>
              <UiButton variant="ghost" size="xs" @click="emit('terminal', container)">
                终端
              </UiButton>
              <UiButton
                variant="ghost"
                size="xs"
                class="text-danger-strong dark:text-danger-dark"
                @click="emit('action', container, 'remove')"
              >
                删除
              </UiButton>
            </div>
          </UiTableCell>
        </tr>
      </tbody>
    </UiTable>
  </div>
</template>
