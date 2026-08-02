<script setup lang="ts">
/**
 * hosts 修改 · 读取/编辑/校验/保存（UAC 提权 + 自动备份）
 */
import { computed, onMounted, ref } from "vue";
import { ipc } from "@/core/ipc/ipc";
import { useUiStore } from "@/stores/ui";
import LineNumberTextarea from "@/tools/shared/LineNumberTextarea.vue";
import { countErrors, countMappings, parseHostsLines } from "./useHosts";

const ui = useUiStore();

const content = ref("");
const loaded = ref(false);
const busy = ref(false);
const status = ref("");

const lines = computed(() => parseHostsLines(content.value));
const errors = computed(() => countErrors(lines.value));
const mappings = computed(() => countMappings(lines.value));

async function load() {
  busy.value = true;
  try {
    const r = await ipc.hostsRead();
    if (!r.ok) {
      ui.toast(r.error ?? "读取失败");
      return;
    }
    content.value = r.content;
    loaded.value = true;
    status.value = `已读取（${mappings.value} 条映射）`;
  } catch (e) {
    ui.toast("读取失败：" + (e instanceof Error ? e.message : String(e)));
  } finally {
    busy.value = false;
  }
}

async function save() {
  if (errors.value > 0) {
    ui.toast(`存在 ${errors.value} 处语法错误，请先修复`);
    return;
  }
  busy.value = true;
  try {
    const r = await ipc.hostsSave(content.value);
    if (!r.ok) {
      ui.toast(r.error ?? "保存失败");
      return;
    }
    content.value = r.content;
    status.value = "已保存（修改前已自动备份）";
    ui.toast("hosts 已保存（已备份原文件）");
  } catch (e) {
    ui.toast("保存失败：" + (e instanceof Error ? e.message : String(e)));
  } finally {
    busy.value = false;
  }
}

function reset() {
  content.value = "";
  status.value = "";
}

onMounted(load);
</script>

<template>
  <div class="flex max-w-[1000px] flex-col gap-[12px]">
    <!-- 工具栏 -->
    <div class="flex items-center gap-[10px]">
      <button class="btn-secondary shrink-0" :disabled="busy" @click="load">重新读取</button>
      <button class="btn-primary shrink-0" :disabled="busy || !loaded" @click="save">
        {{ busy ? "处理中…" : "保存（需管理员授权）" }}
      </button>
      <button class="btn-ghost shrink-0" @click="reset">清空编辑区</button>
      <span class="truncate text-body-sm text-text-muted dark:text-text-muted-dark">{{
        status
      }}</span>
    </div>

    <!-- 校验状态 -->
    <div
      v-if="loaded"
      class="flex items-center gap-[14px] rounded-md border px-[12px] py-[8px]"
      :class="
        errors > 0
          ? 'border-tertiary/40 text-tertiary-strong dark:border-tertiary-dark/40 dark:text-tertiary-dark'
          : 'border-border text-secondary dark:border-border-dark dark:text-secondary-dark'
      "
    >
      <span class="text-body-sm font-medium">{{ mappings }} 条有效映射</span>
      <span v-if="errors > 0" class="text-body-sm font-medium">
        ⚠ {{ errors }} 处语法错误（保存将被阻止）
      </span>
      <span v-else class="text-body-sm">语法检查通过</span>
      <span class="ml-auto text-body-sm text-text-muted dark:text-text-muted-dark">
        保存前自动备份为 hosts.bak-&lt;时间戳&gt;
      </span>
    </div>

    <!-- 错误明细 -->
    <div v-if="errors > 0" class="flex flex-col gap-[4px]">
      <p
        v-for="(l, i) in lines.filter((x) => !x.valid)"
        :key="i"
        class="font-mono text-body-sm text-tertiary-strong dark:text-tertiary-dark"
      >
        第 {{ i + 1 }} 行：{{ l.error }} — {{ l.raw.trim().slice(0, 60) }}
      </p>
    </div>

    <!-- 编辑区 -->
    <div v-if="loaded">
      <label class="mb-[6px] field-label"
        >hosts 文件（C:\Windows\System32\drivers\etc\hosts）</label
      >
      <LineNumberTextarea v-model="content" min-height="320px" class="!font-mono" />
    </div>
    <p v-else class="text-body-sm text-text-muted dark:text-text-muted-dark">
      正在读取 hosts 文件…
    </p>
  </div>
</template>
