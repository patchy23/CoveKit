<script setup lang="ts">
/** SSH 命令文本输出弹窗，用于 systemd 与 Docker 日志。 */
defineProps<{ title: string; content: string }>();
defineEmits<{ (event: "close"): void }>();
</script>

<template>
  <Teleport to="body">
    <div
      class="fixed inset-0 z-[150] grid place-items-center bg-black/30 p-[40px]"
    >
      <div
        class="flex h-full w-full max-w-[900px] flex-col overflow-hidden rounded-lg border border-border bg-surface shadow-[0_16px_48px_rgba(16,24,40,0.25)] dark:border-border-dark dark:bg-surface-dark"
      >
        <div
          class="flex shrink-0 items-center border-b border-border px-[16px] py-[10px] dark:border-border-dark"
        >
          <span class="text-body font-medium text-primary dark:text-primary-dark">{{ title }}</span>
          <button class="btn-ghost ml-auto text-body-sm" @click="$emit('close')">关闭</button>
        </div>
        <pre
          class="min-h-0 flex-1 overflow-auto whitespace-pre-wrap bg-surface-muted p-[14px] font-mono text-body-sm text-primary dark:bg-surface-muted-dark dark:text-primary-dark"
          >{{ content || "（没有输出）" }}</pre>
      </div>
    </div>
  </Teleport>
</template>
