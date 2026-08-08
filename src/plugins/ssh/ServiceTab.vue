<script setup lang="ts">
/**
 * ServiceTab · systemd 服务管理子页签（当前为假数据演示）
 */
import { ref } from "vue";
import type { ServerConnection, ServerProfile, SystemdService } from "./contracts";
import { useUiStore } from "@/stores/ui";

defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const filter = ref<"all" | "active" | "inactive" | "failed">("all");

const mockServices: SystemdService[] = [
  { name: "nginx.service", description: "Nginx HTTP Server", loadState: "loaded", activeState: "active", subState: "running", enabled: true },
  { name: "docker.service", description: "Docker Application Container Engine", loadState: "loaded", activeState: "active", subState: "running", enabled: true },
  { name: "mysql.service", description: "MySQL Database Server", loadState: "loaded", activeState: "inactive", subState: "dead", enabled: false },
  { name: "redis.service", description: "Redis In-Memory Data Store", loadState: "loaded", activeState: "failed", subState: "failed", enabled: true },
  { name: "ssh.service", description: "OpenSSH Server", loadState: "loaded", activeState: "active", subState: "running", enabled: true },
];

const filtered = ref([...mockServices]);

function applyFilter() {
  if (filter.value === "all") {
    filtered.value = [...mockServices];
  } else {
    filtered.value = mockServices.filter((s) => s.activeState === filter.value);
  }
}

function action(svc: SystemdService, act: "start" | "stop" | "restart") {
  ui.toast(`${act === "start" ? "启动" : act === "stop" ? "停止" : "重启"} ${svc.name}（待后端 IPC 接入）`);
}

function logs(svc: SystemdService) {
  ui.toast(`查看 ${svc.name} 日志（待后端 IPC 接入）`);
}

function stateClass(s: SystemdService): string {
  if (s.activeState === "active") return "text-success-strong dark:text-success-dark";
  if (s.activeState === "failed") return "text-danger-strong dark:text-danger-dark";
  return "text-text-muted dark:text-text-muted-dark";
}

function stateText(s: SystemdService): string {
  if (s.activeState === "active") return "运行中";
  if (s.activeState === "failed") return "失败";
  return "已停止";
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? "未连接" }} · 服务管理
      </span>
      <select
        v-model="filter"
        class="field-input !h-[28px] !w-[110px] !py-[4px] text-caption"
        @change="applyFilter"
      >
        <option value="all">全部</option>
        <option value="active">运行中</option>
        <option value="inactive">已停止</option>
        <option value="failed">失败</option>
      </select>
      <div class="ml-auto">
        <button class="btn-ghost !px-[8px] !py-[3px] text-caption" @click="applyFilter">刷新</button>
      </div>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto">
      <table class="w-full text-left text-body">
        <thead class="sticky top-0 bg-surface dark:bg-surface-dark">
          <tr class="border-b border-border text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark">
            <th class="px-[12px] py-[8px] font-medium">服务名</th>
            <th class="px-[12px] py-[8px] font-medium">描述</th>
            <th class="w-[90px] px-[12px] py-[8px] font-medium">状态</th>
            <th class="w-[140px] px-[12px] py-[8px] font-medium">操作</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="s in filtered"
            :key="s.name"
            class="border-b border-border/50 transition-colors hover:bg-surface-muted dark:border-border-dark/50 dark:hover:bg-surface-muted-dark"
          >
            <td class="px-[12px] py-[8px] font-mono text-body-sm">{{ s.name }}</td>
            <td class="px-[12px] py-[8px] text-body-sm">{{ s.description }}</td>
            <td class="px-[12px] py-[8px]">
              <span :class="stateClass(s)">{{ stateText(s) }}</span>
            </td>
            <td class="px-[12px] py-[8px]">
              <div class="flex gap-[4px]">
                <button
                  v-if="s.activeState !== 'active'"
                  class="btn-ghost !px-[6px] !py-[2px] text-caption"
                  @click="action(s, 'start')"
                >
                  启动
                </button>
                <button
                  v-if="s.activeState === 'active'"
                  class="btn-ghost !px-[6px] !py-[2px] text-caption"
                  @click="action(s, 'stop')"
                >
                  停止
                </button>
                <button
                  v-if="s.activeState === 'active'"
                  class="btn-ghost !px-[6px] !py-[2px] text-caption"
                  @click="action(s, 'restart')"
                >
                  重启
                </button>
                <button class="btn-ghost !px-[6px] !py-[2px] text-caption" @click="logs(s)">日志</button>
              </div>
            </td>
          </tr>
        </tbody>
      </table>
    </div>

    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>共 {{ filtered.length }} 个服务</span>
      <span class="ml-auto">{{ connection?.status === "connected" ? "就绪" : "未连接" }}</span>
    </div>
  </div>
</template>
