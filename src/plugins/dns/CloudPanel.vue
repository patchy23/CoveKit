<script setup lang="ts">
/**
 * 云解析管理 · 平台切换 + 域名列表
 * 点击域名进入记录管理（RecordPanel）；未配置密钥时给出引导提示。
 */
import { onMounted, ref } from "vue";
import { ipc } from "./ipc";
import { useUiStore } from "@/stores/ui";
import type { CloudDomain } from "./contracts";
import { platformLabel } from "./useDns";
import RecordPanel from "./RecordPanel.vue";

const ui = useUiStore();

const platform = ref<"aliyun" | "dnspod">("aliyun");
const domains = ref<CloudDomain[]>([]);
const busy = ref(false);
const configured = ref(true);

/** 当前选中的域名（非空 = 记录视图） */
const activeDomain = ref<CloudDomain | null>(null);

/** 切换平台：重新拉取域名列表 */
async function switchPlatform(p: "aliyun" | "dnspod") {
  if (platform.value === p) return;
  platform.value = p;
  activeDomain.value = null;
  await loadDomains();
}

/** 拉取域名列表（密钥未配置时 configured=false 引导去设置页） */
async function loadDomains() {
  busy.value = true;
  try {
    const r = await ipc.dnsDomains(platform.value);
    domains.value = r.list;
    configured.value = true;
  } catch (e) {
    // 密钥未配置或网络失败：提示并展示空列表（引导去设置页）
    configured.value = false;
    domains.value = [];
    ui.toast("加载域名失败：" + (e instanceof Error ? e.message : String(e)));
  } finally {
    busy.value = false;
  }
}

/** 从记录视图返回域名列表 */
function backToDomains() {
  activeDomain.value = null;
  loadDomains();
}

onMounted(loadDomains);
</script>

<template>
  <div class="flex h-full min-h-0 flex-col gap-[12px]">
    <!-- 平台切换 + 刷新 -->
    <div class="flex shrink-0 items-center gap-[8px]">
      <button
        v-for="p in ['aliyun', 'dnspod'] as const"
        :key="p"
        class="rounded-md px-[14px] py-[7px] text-body font-medium transition-colors"
        :class="
          platform === p
            ? 'bg-tertiary-soft text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark'
            : 'bg-neutral text-secondary hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:text-primary-dark'
        "
        @click="switchPlatform(p)"
      >
        {{ platformLabel(p) }}
      </button>
      <button class="btn-ghost shrink-0" :disabled="busy" @click="loadDomains">
        {{ busy ? "加载中…" : "刷新" }}
      </button>
    </div>

    <!-- 未配置密钥：引导 -->
    <div
      v-if="!configured"
      class="rounded-md border border-warning/40 bg-warning-soft/40 px-[12px] py-[10px] text-body-sm text-warning-strong dark:border-warning-dark/40 dark:bg-warning-soft-dark/40 dark:text-warning-dark"
    >
      当前平台尚未配置密钥，请先到「密钥设置」页填写
      {{ platformLabel(platform) }} 的 API 密钥。
    </div>

    <!-- 记录视图（点击域名进入） -->
    <RecordPanel
      v-if="activeDomain"
      :platform="platform"
      :domain="activeDomain"
      class="min-h-0 flex-1"
      @back="backToDomains"
    />

    <!-- 域名列表 -->
    <div v-else class="min-h-0 flex-1 overflow-y-auto pr-[2px]">
      <p v-if="!busy && domains.length === 0" class="text-body-sm text-text-muted dark:text-text-muted-dark">
        {{ configured ? "暂无域名（或平台侧无解析域名）" : "—" }}
      </p>
      <div class="grid grid-cols-[repeat(auto-fill,minmax(228px,1fr))] gap-[10px]">
        <button
          v-for="d in domains"
          :key="d.domainId"
          class="flex flex-col items-start gap-[6px] rounded-lg border border-border bg-surface p-[14px] text-left transition-colors hover:border-tertiary/50 dark:border-border-dark dark:bg-surface-dark dark:hover:border-tertiary-dark/50"
          @click="activeDomain = d"
        >
          <span class="font-mono text-body font-medium text-primary dark:text-primary-dark">
            {{ d.domainName }}
          </span>
          <span class="flex items-center gap-[10px] text-body-sm text-text-muted dark:text-text-muted-dark">
            <span>{{ d.recordTotal }} 条记录</span>
            <span v-if="d.createTime" class="truncate">{{ d.createTime }}</span>
          </span>
        </button>
      </div>
    </div>
  </div>
</template>
