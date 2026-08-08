<script setup lang="ts">
/**
 * DockerTab · Docker 容器管理子页签（后端 docker 命令真实数据）
 * 搜索（名称/ID/镜像）+ 状态筛选。
 */
import { computed, onMounted, ref } from "vue";
import type { ServerConnection, ServerProfile, DockerContainer } from "./contracts";
import { useUiStore } from "@/stores/ui";
import Select from "@/features/ui/Select.vue";
import { ipc } from "./ipc";

const props = defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const containers = ref<DockerContainer[]>([]);
const keyword = ref("");
const statusFilter = ref<"all" | "running" | "exited">("all");

/** 过滤后的容器列表（搜索 + 状态筛选） */
const filtered = computed(() => {
  let list = containers.value;
  const kw = keyword.value.trim().toLowerCase();
  if (kw) {
    list = list.filter(
      (c) =>
        c.name.toLowerCase().includes(kw) ||
        c.id.toLowerCase().includes(kw) ||
        c.image.toLowerCase().includes(kw),
    );
  }
  if (statusFilter.value !== "all") {
    list = list.filter((c) => c.status === statusFilter.value);
  }
  return list;
});

async function refresh() {
  if (!props.connection?.sessionId) return;
  try {
    containers.value = await ipc.sshDockerList(props.connection.sessionId);
  } catch (e) {
    ui.toast(`容器列表加载失败：${e}`);
  }
}

async function action(c: DockerContainer, act: "start" | "stop" | "restart" | "remove") {
  if (!props.connection?.sessionId) return;
  if (act === "remove" && !window.confirm(`删除容器「${c.name}」？此操作不可恢复。`)) return;
  try {
    const r = await ipc.sshDockerAction({
      connectionId: props.connection.sessionId,
      containerId: c.id,
      action: act,
    });
    if (r.ok) {
      ui.toast(`${act === "start" ? "启动" : act === "stop" ? "停止" : act === "restart" ? "重启" : "删除"}容器 ${c.name} 成功`);
      refresh();
    } else {
      ui.toast(`操作失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`操作失败：${e}`);
  }
}

function logs(c: DockerContainer) {
  if (!props.connection?.sessionId) return;
  ipc
    .sshDockerLogs({ connectionId: props.connection.sessionId, containerId: c.id, lines: 100 })
    .then((r) => {
      if (r.ok) ui.toast(`日志已获取（${r.logs.length} 字符）`);
    })
    .catch((e) => ui.toast(`日志获取失败：${e}`));
}

function exec(c: DockerContainer) {
  ui.toast(`进入容器 ${c.name} 终端（终端通道联调后接入）`);
}

function stateClass(s: string): string {
  if (s === "running") return "text-success-strong dark:text-success-dark";
  return "text-text-muted dark:text-text-muted-dark";
}

onMounted(refresh);
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? "未连接" }} · Docker 容器
      </span>
      <input
        v-model="keyword"
        class="field-input !h-[28px] !w-[180px] !py-[4px] text-caption"
        placeholder="搜索名称/ID/镜像"
      />
      <Select
        :model-value="statusFilter"
        size="sm"
        class="!w-[100px] shrink-0"
        title="按状态筛选"
        :options="[
          { value: 'all', label: '全部' },
          { value: 'running', label: '运行中' },
          { value: 'exited', label: '已停止' },
        ]"
        @update:model-value="statusFilter = $event as 'all' | 'running' | 'exited'"
      />
      <div class="ml-auto">
        <button
          class="btn-ghost !px-[8px] !py-[3px] text-caption"
          title="刷新容器列表"
          @click="ui.toast('已刷新（mock）')"
        >刷新</button>
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
            v-for="c in filtered"
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
      <span>共 {{ filtered.length }} 个容器</span>
      <span class="ml-auto">{{ connection?.status === "connected" ? "就绪" : "未连接" }}</span>
    </div>
  </div>
</template>
