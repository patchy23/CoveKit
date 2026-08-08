<script setup lang="ts">
/**
 * EditorTab · 远程文件编辑子页签
 * 当前为占位演示：CodeMirror 集成待后端 IPC 与真实文件加载后接入。
 */
import { ref } from "vue";
import type { ServerConnection, ServerProfile } from "./contracts";
import { useUiStore } from "@/stores/ui";

defineProps<{
  connection?: ServerConnection;
  profile?: ServerProfile;
}>();

const ui = useUiStore();

const filePath = ref("/etc/nginx/nginx.conf");
const content = ref(`server {
    listen 80;
    server_name example.com;
    root /var/www/html;
    index index.html;

    location /api {
        proxy_pass http://127.0.0.1:3000;
    }
}`);
const dirty = ref(false);

function save() {
  ui.toast(`保存 ${filePath.value}（待后端 IPC 接入）`);
  dirty.value = false;
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 工具栏 -->
    <div
      class="flex shrink-0 items-center gap-[10px] border-b border-border px-[12px] py-[8px] dark:border-border-dark"
    >
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        {{ profile?.name ?? "未连接" }} · 远程编辑
      </span>
      <input
        v-model="filePath"
        class="field-input !h-[28px] flex-1 !py-[4px] font-mono text-body-sm"
        spellcheck="false"
      />
      <button class="btn-primary !h-[28px] !px-[12px] text-caption" @click="save">保存</button>
    </div>

    <!-- 编辑器占位（CodeMirror 集成点） -->
    <div class="min-h-0 flex-1 overflow-auto bg-surface p-[12px] dark:bg-surface-dark">
      <pre
        class="font-mono text-body-sm leading-relaxed text-primary dark:text-primary-dark"
        @input="dirty = true"
        >{{ content }}</pre
      >
    </div>

    <!-- 状态栏 -->
    <div
      class="flex shrink-0 items-center gap-[12px] border-t border-border px-[12px] py-[6px] text-caption text-text-muted dark:border-border-dark dark:text-text-muted-dark"
    >
      <span>{{ filePath }}</span>
      <span>{{ content.length }} 字符</span>
      <span v-if="dirty" class="text-warning-strong dark:text-warning-dark">已修改（未保存）</span>
      <span v-else class="text-success-strong dark:text-success-dark">已保存</span>
    </div>
  </div>
</template>
