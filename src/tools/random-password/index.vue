<script setup lang="ts">
/**
 * 随机密码 · 安全随机生成（长度/字符集/排除易混字符）
 */
import { computed, ref } from "vue";
import { estimateStrength, generatePassword, type PasswordOptions } from "./usePassword";
import { useCopy } from "@/tools/shared/useClipboard";

const { copyText } = useCopy();

const opts = ref<PasswordOptions>({
  length: 16,
  upper: true,
  lower: true,
  digits: true,
  symbols: true,
  excludeAmbiguous: false,
});

const password = ref("");

const strength = computed(() => estimateStrength(password.value));

const strengthColor = ["", "bg-text-muted", "bg-warning", "bg-success", "bg-success-strong"];

function generate() {
  password.value = generatePassword(opts.value);
}

function toggleFlag(key: keyof PasswordOptions, checked: boolean) {
  opts.value = { ...opts.value, [key]: checked };
  if (!opts.value.upper && !opts.value.lower && !opts.value.digits && !opts.value.symbols) {
    opts.value = { ...opts.value, lower: true };
  }
  generate();
}

function onLength(e: Event) {
  opts.value = { ...opts.value, length: Number((e.target as HTMLInputElement).value) };
  generate();
}

generate();
</script>

<template>
  <div class="flex max-w-[640px] flex-col gap-[16px]">
    <!-- 结果 -->
    <div
      class="flex items-center gap-[12px] rounded-md border border-border bg-surface-muted p-[14px] dark:border-border-dark dark:bg-surface-muted-dark"
    >
      <code
        class="min-w-0 flex-1 break-all font-mono text-card-title text-primary dark:text-primary-dark"
      >
        {{ password }}
      </code>
      <button class="btn-ghost shrink-0" @click="copyText(password, '密码已复制')">复制</button>
      <button class="btn-ghost shrink-0" title="重新生成" @click="generate">刷新</button>
    </div>
    <div class="flex items-center gap-[8px]">
      <div class="h-[5px] w-[140px] overflow-hidden rounded-full bg-border dark:bg-border-dark">
        <div
          class="h-full rounded-full transition-all duration-300"
          :class="strengthColor[strength.score]"
          :style="{ width: `${strength.score * 25}%` }"
        />
      </div>
      <span class="text-body-sm text-secondary dark:text-secondary-dark">
        强度：{{ strength.label }}
      </span>
    </div>

    <!-- 选项 -->
    <div
      class="flex flex-col gap-[14px] rounded-md border border-border p-[14px] dark:border-border-dark"
    >
      <div class="flex items-center gap-[12px]">
        <label class="field-label w-[64px] shrink-0">长度</label>
        <input
          type="range"
          min="8"
          max="64"
          :value="opts.length"
          class="flex-1 accent-tertiary-strong"
          @input="onLength"
        />
        <span class="w-[40px] text-right font-mono text-body text-primary dark:text-primary-dark">
          {{ opts.length }}
        </span>
      </div>
      <div class="grid grid-cols-2 gap-[10px]">
        <label
          class="flex cursor-pointer items-center gap-[8px] text-body text-secondary dark:text-secondary-dark"
        >
          <input
            type="checkbox"
            :checked="opts.upper"
            class="accent-tertiary-strong"
            @change="toggleFlag('upper', ($event.target as HTMLInputElement).checked)"
          />
          大写字母 A-Z
        </label>
        <label
          class="flex cursor-pointer items-center gap-[8px] text-body text-secondary dark:text-secondary-dark"
        >
          <input
            type="checkbox"
            :checked="opts.lower"
            class="accent-tertiary-strong"
            @change="toggleFlag('lower', ($event.target as HTMLInputElement).checked)"
          />
          小写字母 a-z
        </label>
        <label
          class="flex cursor-pointer items-center gap-[8px] text-body text-secondary dark:text-secondary-dark"
        >
          <input
            type="checkbox"
            :checked="opts.digits"
            class="accent-tertiary-strong"
            @change="toggleFlag('digits', ($event.target as HTMLInputElement).checked)"
          />
          数字 0-9
        </label>
        <label
          class="flex cursor-pointer items-center gap-[8px] text-body text-secondary dark:text-secondary-dark"
        >
          <input
            type="checkbox"
            :checked="opts.symbols"
            class="accent-tertiary-strong"
            @change="toggleFlag('symbols', ($event.target as HTMLInputElement).checked)"
          />
          符号 !@#$%…
        </label>
        <label
          class="flex cursor-pointer items-center gap-[8px] text-body text-secondary dark:text-secondary-dark"
        >
          <input
            type="checkbox"
            :checked="opts.excludeAmbiguous"
            class="accent-tertiary-strong"
            @change="toggleFlag('excludeAmbiguous', ($event.target as HTMLInputElement).checked)"
          />
          排除易混字符（0O1lI|…）
        </label>
      </div>
    </div>
  </div>
</template>
