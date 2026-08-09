<script setup lang="ts">
/** DockerTable · SSH Docker 容器列表与行级操作。 */
import type { DockerContainer } from "./contracts";
import { shortContainerId } from "./useSsh";

defineProps<{ containers: DockerContainer[]; busyContainerId?: string | null }>();
const emit = defineEmits<{
  (
    event: "action",
    container: DockerContainer,
    action: "start" | "stop" | "restart" | "remove"
  ): void;
  (event: "logs", container: DockerContainer): void;
  (event: "terminal", container: DockerContainer): void;
}>();

function stateClass(status: string): string {
  if (status === "running") return "text-success-strong dark:text-success-dark";
  return "text-text-muted dark:text-text-muted-dark";
}
</script>

<template>
  <div class="min-h-0 flex-1 overflow-y-auto">
    <table class="w-full text-left text-body">
      <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
        <tr
          class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
        >
          <th class="w-[112px] whitespace-nowrap px-[12px] py-[8px] font-medium">容器 ID</th>
          <th class="w-[200px] px-[12px] py-[8px] font-medium">名称</th>
          <th class="w-[140px] px-[12px] py-[8px] font-medium">镜像</th>
          <th class="w-[90px] whitespace-nowrap px-[12px] py-[8px] font-medium">状态</th>
          <th class="w-[120px] whitespace-nowrap px-[12px] py-[8px] font-medium">运行时间</th>
          <th class="w-[140px] px-[12px] py-[8px] font-medium">端口</th>
          <th class="w-[240px] whitespace-nowrap px-[12px] py-[8px] font-medium">操作</th>
        </tr>
      </thead>
      <tbody>
        <tr
          v-for="container in containers"
          :key="container.id"
          class="border-b border-border/50 transition-colors hover:bg-surface-muted dark:border-border-dark/50 dark:hover:bg-surface-muted-dark"
        >
          <td
            class="w-[112px] whitespace-nowrap px-[12px] py-[8px] font-mono text-body-sm"
            :title="container.id"
          >
            {{ shortContainerId(container.id) }}
          </td>
          <td
            class="max-w-[200px] truncate px-[12px] py-[8px] text-body-sm"
            :title="container.name"
          >
            {{ container.name }}
          </td>
          <td
            class="max-w-[140px] truncate px-[12px] py-[8px] font-mono text-body-sm"
            :title="container.image"
          >
            {{ container.image }}
          </td>
          <td class="whitespace-nowrap px-[12px] py-[8px]">
            <span
              v-if="busyContainerId === container.id"
              class="animate-pulse text-tertiary-strong dark:text-tertiary-dark"
            >
              更新中…
            </span>
            <span v-else :class="stateClass(container.status)">{{ container.status }}</span>
          </td>
          <td class="whitespace-nowrap px-[12px] py-[8px] font-mono text-body-sm">
            {{ container.uptime }}
          </td>
          <td
            class="max-w-[140px] truncate px-[12px] py-[8px] font-mono text-body-sm"
            :title="container.ports"
          >
            {{ container.ports }}
          </td>
          <td class="whitespace-nowrap px-[12px] py-[8px]">
            <div class="flex items-center justify-end gap-[4px]">
              <button
                v-if="container.status !== 'running'"
                class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                @click="emit('action', container, 'start')"
              >
                启动
              </button>
              <button
                v-if="container.status === 'running'"
                class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                @click="emit('action', container, 'stop')"
              >
                停止
              </button>
              <button
                v-if="container.status === 'running'"
                class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                @click="emit('action', container, 'restart')"
              >
                重启
              </button>
              <button
                class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                @click="emit('logs', container)"
              >
                日志
              </button>
              <button
                class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                @click="emit('terminal', container)"
              >
                终端
              </button>
              <button
                class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption text-danger-strong dark:text-danger-dark"
                @click="emit('action', container, 'remove')"
              >
                删除
              </button>
            </div>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
