<script setup lang="ts">
/**
 * DockerTab · Docker 容器管理子页签（当前为假数据演示）
 */
import { ref } from "vue";
import type { ServerConnection, ServerProfile, DockerContainer } from "./contracts";
import { useUiStore } from "@/stores/ui";

defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const mockContainers: DockerContainer[] = [
  { id: "a1b2c3d4", name: "nginx", image: "nginx:latest", status: "running", ports: "80:80,443:443", createdAt: Date.now() - 86400000 * 7 },
  { id: "e5f6g7h8", name: "mysql", image: "mysql:8.0", status: "running", ports: "3306:3306", createdAt: Date.now() - 86400000 * 14 },
  { id: "i9j0k1l2", name: "redis", image: "redis:alpine", status: "exited", ports: "6379:6379", createdAt: Date.now() - 86400000 * 30 },
  { id: "m3n4o5p6", name: "app-api", image: "registry.local/api:v2.1", status: "running", ports: "8080:8080", createdAt: Date.now() - 86400000 * 3 },
];

const containers = ref([...mockContainers]);

function action(c: DockerContainer, act: "start" | "stop" | "restart" | "remove") {
  ui.toast(`${act === "start" ? "启动" : act === "stop" ? "停止" : act === "restart" ? "重启" : "删除"}容器 ${c.name}（待后端 IPC 接入）`);
}

function logs(c: DockerContainer) {
  ui.toast(`查看容器 ${c.name} 日志（待后端 IPC 接入）`);
}

function exec(c: DockerContainer) {
  ui.toast(`进入容器 ${c.name} 终端（待后端 IPC 接入）`);
}

function stateClass(s: string): string {
  if (s === "running") return "text-success-strong dark:text-success-dark";
  return "text-text-muted dark:text-text-muted-dark";
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? "未连接" }} · Docker 容器
      </span>
      <div class="ml-auto">
        <button class="btn-ghost !px-[8px] !py-[3px] text-caption">刷新</button>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto">
      <table class="w-full text-left text-body">
        <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
          <tr class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark">
            <th class="w-[90px] whitespace-nowrap px-[12px] py-[8px] font-medium">容器 ID</th>
            <th class="w-[200px] px-[12px] py-[8px] font-medium">名称</th>
            <th class="w-[140px] px-[12px] py-[8px] font-medium">镜像</th>
            <th class="w-[90px] whitespace-nowrap px-[12px] py-[8px] font-medium">状态</th>
            <th class="w-[140px] px-[12px] py-[8px] font-medium">端口</th>
            <th class="w-[240px] whitespace-nowrap px-[12px] py-[8px] font-medium">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="c in containers"
            :key="c.id"
            class="border-b border-border/50 transition-colors hover:bg-surface-muted dark:border-border-dark/50 dark:hover:bg-surface-muted-dark"
          >
            <td class="whitespace-nowrap px-[12px] py-[8px] font-mono text-body-sm">{{ c.id }}</td>
            <td class="max-w-[200px] truncate px-[12px] py-[8px] text-body-sm" :title="c.name">{{ c.name }}</td>
            <td class="max-w-[140px] truncate px-[12px] py-[8px] font-mono text-body-sm" :title="c.image">
              {{ c.image }}
            </td>
            <td class="whitespace-nowrap px-[12px] py-[8px]">
              <span :class="stateClass(c.status)">{{ c.status }}</span>
            </td>
            <td class="max-w-[140px] truncate px-[12px] py-[8px] font-mono text-body-sm" :title="c.ports">{{ c.ports }}</td>
            <td class="whitespace-nowrap px-[12px] py-[8px]">
              <div class="flex items-center justify-end gap-[4px]">
                <button
                  v-if="c.status !== 'running'"
                  class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                  @click="action(c, 'start')"
                >
                  启动
                </button>
                <button
                  v-if="c.status === 'running'"
                  class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                  @click="action(c, 'stop')"
                >
                  停止
                </button>
                <button
                  v-if="c.status === 'running'"
                  class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                  @click="action(c, 'restart')"
                >
                  重启
                </button>
                <button
                  class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                  @click="logs(c)"
                >
                  日志
                </button>
                <button
                  class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption"
                  @click="exec(c)"
                >
                  终端
                </button>
                <button
                  class="btn-ghost shrink-0 whitespace-nowrap !px-[6px] !py-[2px] text-caption text-danger-strong dark:text-danger-dark"
                  @click="action(c, 'remove')"
                >
                  删除
                </button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>共 {{ containers.length }} 个容器</span>
      <span class="ml-auto">{{ connection?.status === "connected" ? "就绪" : "未连接" }}</span>
    </div>
  </div>
</template>
