<script setup lang="ts">
/**
 * ServiceTab · systemd 服务管理子页签（后端 exec 真实数据）
 */
import { computed, onMounted, ref } from "vue";
import type { ServerConnection, ServerProfile, SystemdService } from "./contracts";
import { useUiStore } from "@/stores/ui";
import Select from "@/features/ui/Select.vue";
import { ipc } from "./ipc";

const props = defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const filter = ref<"all" | "active" | "inactive" | "failed">("all");
const services = ref<SystemdService[]>([]);
const loading = ref(false);

/** 按状态筛选后的服务列表（computed 自动响应 filter 变化） */
const filtered = computed(() => {
  if (filter.value === "all") return services.value;
  return services.value.filter((s) => s.activeState === filter.value);
});

async function refresh() {
  if (!props.connection?.sessionId) return;
  loading.value = true;
  try {
    services.value = await ipc.sshServiceList({
      connectionId: props.connection.sessionId,
      filter: filter.value,
    });
  } catch (e) {
    ui.toast(`服务列表加载失败：${e}`);
  } finally {
    loading.value = false;
  }
}

async function action(svc: SystemdService, act: "start" | "stop" | "restart") {
  if (!props.connection?.sessionId) return;
  try {
    const r = await ipc.sshServiceAction({
      connectionId: props.connection.sessionId,
      serviceName: svc.name,
      action: act,
    });
    if (r.ok) {
      ui.toast(`${act === "start" ? "启动" : act === "stop" ? "停止" : "重启"} ${svc.name} 成功`);
      refresh();
    } else {
      ui.toast(`${act} ${svc.name} 失败：${r.error ?? "未知错误"}`);
    }
  } catch (e) {
    ui.toast(`操作失败：${e}`);
  }
}

function logs(svc: SystemdService) {
  if (!props.connection?.sessionId) return;
  ui.toast(`查看 ${svc.name} 日志（控制台输出）`);
  ipc
    .sshServiceLogs({ connectionId: props.connection.sessionId, serviceName: svc.name, lines: 100 })
    .then((r) => {
      if (r.ok) ui.toast(`日志已获取（${r.logs.length} 字符）`);
    })
    .catch((e) => ui.toast(`日志获取失败：${e}`));
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

onMounted(refresh);
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? "未连接" }} · 服务管理
      </span>
      <Select
        :model-value="filter"
        size="sm"
        class="!w-[110px] shrink-0"
        title="按状态筛选"
        :options="[
          { value: 'all', label: '全部' },
          { value: 'active', label: '运行中' },
          { value: 'inactive', label: '已停止' },
          { value: 'failed', label: '失败' },
        ]"
        @update:model-value="filter = $event as 'all' | 'active' | 'inactive' | 'failed'"
      />
      <div class="ml-auto">
        <button
          class="btn-ghost !px-[8px] !py-[3px] text-caption"
          title="刷新服务列表"
          @click="ui.toast('已刷新（mock）')"
        >
          刷新
        </button>
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
