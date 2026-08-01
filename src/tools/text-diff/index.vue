<script setup lang="ts">
/**
 * 文本对比 · 左右输入，LCS 逐行 diff 高亮（增/删/同）
 */
import { computed, ref } from "vue";
import { diffLines, diffStats } from "./useDiff";
import LineNumberTextarea from "@/tools/shared/LineNumberTextarea.vue";

const left = ref("第一行\n第二行\n第三行");
const right = ref("第一行\n第二行（修改）\n第四行");

const diff = computed(() => diffLines(left.value, right.value));
const stats = computed(() => diffStats(diff.value));
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <div class="grid grid-cols-2 gap-[12px]">
      <div>
        <label class="mb-[6px] field-label">原文</label>
        <LineNumberTextarea v-model="left" />
      </div>
      <div>
        <label class="mb-[6px] field-label">新文</label>
        <LineNumberTextarea v-model="right" />
      </div>
    </div>

    <div class="flex items-center gap-[10px] text-body-sm text-text-muted">
      <span
        class="rounded-full bg-success-soft px-[9px] py-[2px] font-semibold text-success-strong dark:bg-success-soft-dark dark:text-success-dark"
      >
        +{{ stats.adds }} 新增
      </span>
      <span
        class="rounded-full bg-tertiary-soft px-[9px] py-[2px] font-semibold text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
      >
        -{{ stats.removes }} 删除
      </span>
      <span class="rounded-full bg-neutral px-[9px] py-[2px] font-medium dark:bg-neutral-dark">
        {{ stats.unchanged }} 行相同
      </span>
    </div>

    <div class="overflow-hidden rounded-md border border-border dark:border-border-dark">
      <div class="flex flex-col">
        <div
          v-for="(line, i) in diff"
          :key="i"
          class="flex gap-[10px] border-b border-border px-[12px] py-[5px] font-mono text-body leading-relaxed last:border-b-0 dark:border-border-dark"
          :class="
            line.type === 'add'
              ? 'bg-success-soft dark:bg-success-soft-dark'
              : line.type === 'remove'
                ? 'bg-tertiary-soft dark:bg-tertiary-soft-dark'
                : 'dark:bg-surface-dark'
          "
        >
          <span
            class="w-[22px] shrink-0 select-none text-center font-semibold"
            :class="
              line.type === 'add'
                ? 'text-success-strong dark:text-success-dark'
                : line.type === 'remove'
                  ? 'text-tertiary-strong dark:text-tertiary-dark'
                  : 'text-text-muted'
            "
            >{{ line.type === "add" ? "+" : line.type === "remove" ? "-" : " " }}</span
          >
          <span class="whitespace-pre-wrap break-all dark:text-primary-dark">{{ line.text }}</span>
        </div>
        <p v-if="!diff.length" class="p-[14px] text-body text-text-muted">两侧内容相同或为空</p>
      </div>
    </div>
  </div>
</template>
