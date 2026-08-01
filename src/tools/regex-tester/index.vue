<script setup lang="ts">
/**
 * 正则测试 · 实时匹配 + 常用表达式速查
 */
import { computed, ref } from "vue";
import { COMMON_PATTERNS, testRegex } from "./useRegex";

const pattern = ref("\\d+");
const flags = ref("g");
const text = ref("订单号 1001 与 1002 已发货");

const result = computed(() => testRegex(pattern.value, flags.value, text.value));
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <div class="grid grid-cols-[1fr_auto] gap-[10px]">
      <div>
        <label class="mb-[6px] block text-[12px] font-semibold text-secondary">正则表达式</label>
        <input
          v-model="pattern"
          type="text"
          spellcheck="false"
          class="w-full rounded-md border border-border-strong bg-surface-muted px-[13px] py-[11px] font-mono text-[12.5px] text-primary outline-none transition-colors focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
          placeholder="\\d+"
        />
      </div>
      <div>
        <label class="mb-[6px] block text-[12px] font-semibold text-secondary">标志</label>
        <input
          v-model="flags"
          type="text"
          spellcheck="false"
          class="w-[72px] rounded-md border border-border-strong bg-surface-muted px-[13px] py-[11px] font-mono text-[12.5px] text-primary outline-none transition-colors focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
          placeholder="g"
        />
      </div>
    </div>

    <div class="flex flex-wrap gap-[6px]">
      <button
        v-for="p in COMMON_PATTERNS"
        :key="p.label"
        class="rounded-full border border-border bg-surface px-[11px] py-[5px] text-[11.5px] font-medium text-secondary transition-colors hover:border-tertiary hover:text-tertiary-strong dark:border-border-dark dark:bg-surface-dark dark:text-secondary-dark dark:hover:border-tertiary-dark dark:hover:text-tertiary-dark"
        @click="pattern = p.pattern"
      >
        {{ p.label }}
      </button>
    </div>

    <p
      v-if="!result.ok"
      class="rounded-sm bg-tertiary-soft px-[12px] py-[9px] font-mono text-[12.5px] text-tertiary-strong dark:bg-tertiary-soft-dark dark:text-tertiary-dark"
    >
      正则错误：{{ result.error }}
    </p>

    <div>
      <div class="mb-[6px] flex items-center justify-between">
        <label class="text-[12px] font-semibold text-secondary">测试文本</label>
        <span
          class="rounded-full bg-neutral px-[9px] py-[2px] text-[11.5px] font-semibold text-tertiary-strong dark:bg-neutral-dark dark:text-tertiary-dark"
          >{{ result.ok ? `${result.count} 处匹配` : "—" }}</span
        >
      </div>
      <textarea
        v-model="text"
        rows="6"
        spellcheck="false"
        class="w-full resize-y rounded-md border border-border-strong bg-surface-muted p-[11px] font-mono text-[12.5px] leading-relaxed text-primary outline-none transition-colors focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
      />
    </div>

    <div
      v-if="result.ok && result.count"
      class="overflow-hidden rounded-md border border-border dark:border-border-dark"
    >
      <div
        v-for="(m, i) in result.matches.slice(0, 100)"
        :key="i"
        class="flex items-baseline gap-[10px] border-b border-border px-[12px] py-[6px] font-mono text-[12.5px] last:border-b-0 dark:border-border-dark"
      >
        <span class="w-[56px] shrink-0 select-none text-[11px] text-text-muted"
          >#{{ i + 1 }} @{{ m.index }}</span
        >
        <span class="whitespace-pre-wrap break-all text-tertiary-strong dark:text-tertiary-dark">{{
          m.text
        }}</span>
      </div>
      <p v-if="result.count > 100" class="px-[12px] py-[6px] text-[11.5px] text-text-muted">
        仅显示前 100 条（共 {{ result.count }} 条）
      </p>
    </div>
  </div>
</template>
