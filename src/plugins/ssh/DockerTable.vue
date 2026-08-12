<script setup lang="ts">
/** DockerTable · SSH Docker 容器列表与行级操作。 */
import type { DockerContainer } from './contracts'
import { shortContainerId } from './useSsh'
import { UiButton } from '@/core/ui'

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
    <table class="data-table table-fixed">
      <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
        <tr
          class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
        >
          <th class="w-[92px] whitespace-nowrap px-[8px] py-[8px] font-medium">容器 ID</th>
          <th class="w-[110px] px-[8px] py-[8px] font-medium">名称</th>
          <th class="w-[110px] px-[8px] py-[8px] font-medium">镜像</th>
          <th class="w-[70px] whitespace-nowrap px-[8px] py-[8px] font-medium">状态</th>
          <th class="w-[96px] whitespace-nowrap px-[8px] py-[8px] font-medium">运行时间</th>
          <th class="w-[110px] px-[8px] py-[8px] font-medium">端口</th>
          <th class="w-[210px] whitespace-nowrap px-[8px] py-[8px] font-medium">操作</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="container in containers"
          :key="container.id"
          class="border-b border-border/50 transition-colors hover:bg-surface-muted dark:border-border-dark/50 dark:hover:bg-surface-muted-dark"
        >
          <td
            class="data-cell-tech w-[92px] whitespace-nowrap px-[8px] py-[8px]"
            :title="container.id"
          >
            {{ shortContainerId(container.id) }}
          </td>
          <td class="data-cell-tech truncate px-[8px] py-[8px]" :title="container.name">
            {{ container.name }}
          </td>
          <td class="data-cell-tech truncate px-[8px] py-[8px]" :title="container.image">
            {{ container.image }}
          </td>
          <td class="data-cell-tech whitespace-nowrap px-[8px] py-[8px]">
            <span
              v-if="busyContainerId === container.id"
              class="animate-pulse font-sans text-tertiary-strong dark:text-tertiary-dark"
            >
              更新中
            </span>
            <span v-else :class="stateClass(container.status)">{{ container.status }}</span>
          </td>
          <td
            class="data-cell-tech truncate whitespace-nowrap px-[8px] py-[8px]"
            :title="container.uptime"
          >
            {{ container.uptime }}
          </td>
          <td class="data-cell-tech truncate px-[8px] py-[8px]" :title="container.ports">
            {{ container.ports }}
          </td>
          <td class="data-cell-action whitespace-nowrap px-[8px] py-[8px]">
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
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
