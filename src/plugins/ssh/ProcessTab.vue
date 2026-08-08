<script setup lang="ts">
/**
 * ProcessTab · 进程管理子页签（当前为假数据演示）
 */
import { computed, ref } from "vue";
import type { ServerConnection, ServerProfile, ProcessInfo } from "./contracts";
import { formatBytes } from "./useSsh";
import { useUiStore } from "@/stores/ui";

defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const keyword = ref("");
const sortBy = ref<"cpu" | "memory" | "pid">("cpu");

const mockProcesses: ProcessInfo[] = [
  { pid: 1, user: "root", cpuPercent: 0.1, memoryPercent: 0.3, memoryBytes: 50 * 1024 * 1024, startedAt: Date.now() - 86400000 * 7, command: "/sbin/init" },
  { pid: 1234, user: "www-data", cpuPercent: 15.2, memoryPercent: 8.1, memoryBytes: 1.2 * 1024 * 1024 * 1024, startedAt: Date.now() - 3600000 * 5, command: "nginx: worker process" },
  { pid: 5678, user: "mysql", cpuPercent: 45.0, memoryPercent: 22.3, memoryBytes: 3.5 * 1024 * 1024 * 1024, startedAt: Date.now() - 86400000 * 3, command: "/usr/sbin/mysqld" },
  { pid: 9012, user: "root", cpuPercent: 2.3, memoryPercent: 1.5, memoryBytes: 256 * 1024 * 1024, startedAt: Date.now() - 3600000, command: "/usr/sbin/sshd -D" },
  { pid: 3456, user: "redis", cpuPercent: 8.7, memoryPercent: 4.2, memoryBytes: 640 * 1024 * 1024, startedAt: Date.now() - 86400000 * 2, command: "redis-server 127.0.0.1:6379" },
];

const filtered = computed(() => {
  let list = [...mockProcesses];
  const kw = keyword.value.trim().toLowerCase();
  if (kw) {
    list = list.filter(
      (p) =>
        p.command.toLowerCase().includes(kw) ||
        p.user.toLowerCase().includes(kw) ||
        String(p.pid).includes(kw),
    );
  }
  list.sort((a, b) => {
    if (sortBy.value === "cpu") return b.cpuPercent - a.cpuPercent;
    if (sortBy.value === "memory") return b.memoryPercent - a.memoryPercent;
    return a.pid - b.pid;
  });
  return list;
});

function kill(pid: number, force = false) {
  ui.toast(`${force ? "强制结束" : "结束"}进程 ${pid}（待后端 IPC 接入）`);
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? "未连接" }} · 进程管理
      </span>
      <input
        v-model="keyword"
        class="field-input !h-[28px] !w-[180px] !py-[4px] text-caption"
        placeholder="搜索进程/PID/用户"
      />
      <select v-model="sortBy" class="field-input !h-[28px] !w-[100px] !py-[4px] text-caption">
        <option value="cpu">按 CPU</option>
        <option value="memory">按内存</option>
        <option value="pid">按 PID</option>
      </select>
      <div class="ml-auto">
        <button class="btn-ghost !px-[8px] !py-[3px] text-caption">刷新</button>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto">
      <table class="w-full text-left text-body">
        <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
          <tr class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark">
            <th class="w-[70px] px-[12px] py-[8px] font-medium">PID</th>
            <th class="w-[90px] px-[12px] py-[8px] font-medium">用户</th>
            <th class="w-[80px] px-[12px] py-[8px] font-medium">CPU%</th>
            <th class="w-[80px] px-[12px] py-[8px] font-medium">MEM%</th>
            <th class="w-[100px] px-[12px] py-[8px] font-medium">内存</th>
            <th class="px-[12px] py-[8px] font-medium">命令</th>
            <th class="w-[90px] px-[12px] py-[8px] font-medium">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="p in filtered"
            :key="p.pid"
            class="border-b border-border/50 transition-colors hover:bg-surface-muted dark:border-border-dark/50 dark:hover:bg-surface-muted-dark"
          >
            <td class="px-[12px] py-[8px] font-mono text-body-sm">{{ p.pid }}</td>
            <td class="px-[12px] py-[8px] text-body-sm">{{ p.user }}</td>
            <td class="px-[12px] py-[8px] font-mono text-body-sm">{{ p.cpuPercent.toFixed(1) }}</td>
            <td class="px-[12px] py-[8px] font-mono text-body-sm">{{ p.memoryPercent.toFixed(1) }}</td>
            <td class="px-[12px] py-[8px] font-mono text-body-sm">{{ formatBytes(p.memoryBytes) }}</td>
            <td class="max-w-[200px] truncate px-[12px] py-[8px] font-mono text-body-sm" :title="p.command">
              {{ p.command }}
            </td>
            <td class="px-[12px] py-[8px]">
              <button class="btn-ghost !px-[6px] !py-[2px] text-caption" @click="kill(p.pid)">结束</button>
              <button class="btn-ghost !px-[6px] !py-[2px] text-caption text-danger-strong dark:text-danger-dark" @click="kill(p.pid, true)">强杀</button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>共 {{ filtered.length }} 个进程</span>
      <span class="ml-auto">{{ connection?.status === "connected" ? "就绪" : "未连接" }}</span>
    </div>
  </div>
</template>
