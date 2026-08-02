<script setup lang="ts">
/**
 * 哈希计算 · 实时计算 MD5 / SHA-1 / SHA-256 / SHA-384 / SHA-512
 */
import { computed, ref, watch } from "vue";
import { computeAllHashes, type HashResult } from "./useHash";
import { useCopy } from "@/tools/shared/useClipboard";
import LineNumberTextarea from "@/tools/shared/LineNumberTextarea.vue";

const { copyText } = useCopy();

const input = ref("hello world");
const hashes = ref<HashResult | null>(null);
const computing = ref(false);

const ALGOS: { key: keyof HashResult; label: string; note: string }[] = [
  { key: "md5", label: "MD5", note: "128-bit · 非加密用途（校验/兼容）" },
  { key: "sha1", label: "SHA-1", note: "160-bit · 已不推荐安全用途" },
  { key: "sha256", label: "SHA-256", note: "256-bit · 推荐" },
  { key: "sha384", label: "SHA-384", note: "384-bit" },
  { key: "sha512", label: "SHA-512", note: "512-bit" },
];

const byteCount = computed(() => new TextEncoder().encode(input.value).length);

let timer: ReturnType<typeof setTimeout> | null = null;
watch(
  input,
  () => {
    if (timer) clearTimeout(timer);
    if (!input.value) {
      hashes.value = null;
      return;
    }
    timer = setTimeout(async () => {
      computing.value = true;
      hashes.value = await computeAllHashes(input.value);
      computing.value = false;
    }, 150);
  },
  { immediate: true }
);
</script>

<template>
  <div class="flex flex-col gap-[12px]">
    <div>
      <label class="mb-[6px] field-label">输入内容（文本）</label>
      <LineNumberTextarea v-model="input" min-height="140px" placeholder="输入要计算哈希的文本…" />
    </div>
    <div class="flex items-center gap-[8px] text-body-sm text-text-muted dark:text-text-muted-dark">
      <span>{{ byteCount }} 字节</span>
      <span v-if="computing" class="flex items-center gap-[6px]">
        <span
          class="h-3 w-3 animate-spin rounded-full border-2 border-border-strong border-t-tertiary-strong"
        />
        计算中…
      </span>
    </div>
    <div v-if="hashes" class="flex flex-col gap-[8px]">
      <div
        v-for="a in ALGOS"
        :key="a.key"
        class="flex items-center gap-[12px] rounded-md border border-border bg-surface-muted px-[14px] py-[10px] dark:border-border-dark dark:bg-surface-muted-dark"
      >
        <div class="w-[86px] shrink-0">
          <div class="font-mono text-body font-semibold text-primary dark:text-primary-dark">
            {{ a.label }}
          </div>
          <div class="text-body-sm text-text-muted dark:text-text-muted-dark">{{ a.note }}</div>
        </div>
        <code
          class="min-w-0 flex-1 break-all font-mono text-body-sm leading-relaxed text-secondary dark:text-secondary-dark"
          >{{ hashes[a.key] }}</code
        >
        <button class="btn-ghost shrink-0" @click="copyText(hashes[a.key], `${a.label} 已复制`)">
          复制
        </button>
      </div>
    </div>
    <p v-else class="text-body-sm text-text-muted dark:text-text-muted-dark">
      输入内容后实时计算全部哈希
    </p>
  </div>
</template>
