<script setup lang="ts">
/**
 * 二维码 · 实时生成 + 容错级别自适应 + 下载 PNG
 */
import { computed, ref, watch } from "vue";
import { renderQrDataUrl, suggestLevel, type QrOptions } from "./useQr";
import LineNumberTextarea from "@/tools/shared/LineNumberTextarea.vue";

const input = ref("https://github.com/patchy23");
const level = ref<NonNullable<QrOptions["errorCorrectionLevel"]>>("M");
const size = ref(240);
const dataUrl = ref("");
const error = ref("");

const SIZES = [
  { label: "小 160", value: 160 },
  { label: "中 240", value: 240 },
  { label: "大 320", value: 320 },
];

const LEVELS: { value: NonNullable<QrOptions["errorCorrectionLevel"]>; label: string }[] = [
  { value: "L", label: "L（7%）" },
  { value: "M", label: "M（15%）" },
  { value: "Q", label: "Q（25%）" },
  { value: "H", label: "H（30%）" },
];

let timer: ReturnType<typeof setTimeout> | null = null;
async function generate() {
  if (timer) clearTimeout(timer);
  timer = setTimeout(async () => {
    error.value = "";
    if (!input.value.trim()) {
      dataUrl.value = "";
      return;
    }
    try {
      dataUrl.value = await renderQrDataUrl(input.value, {
        width: size.value,
        errorCorrectionLevel: level.value,
      });
    } catch (e) {
      error.value = "二维码生成失败：" + (e instanceof Error ? e.message : String(e));
      dataUrl.value = "";
    }
  }, 200);
}

watch([input, level, size], generate, { immediate: true });

function autoLevel() {
  level.value = suggestLevel(input.value);
}

const charCount = computed(() => [...input.value].length);

function onLevelChange(e: Event) {
  level.value = (e.target as HTMLSelectElement).value as NonNullable<
    QrOptions["errorCorrectionLevel"]
  >;
}

function onSizeChange(e: Event) {
  size.value = Number((e.target as HTMLSelectElement).value);
}

function download() {
  if (!dataUrl.value) return;
  const a = document.createElement("a");
  a.href = dataUrl.value;
  a.download = "qrcode.png";
  a.click();
}
</script>

<template>
  <div class="flex max-w-[760px] flex-col gap-[12px]">
    <div>
      <label class="mb-[6px] field-label">文本 / 链接</label>
      <LineNumberTextarea
        v-model="input"
        min-height="120px"
        placeholder="输入要生成二维码的文本或 URL…"
      />
    </div>

    <div class="flex flex-wrap items-center gap-[16px]">
      <div class="flex items-center gap-[8px]">
        <span class="text-body-sm text-text-muted dark:text-text-muted-dark">容错</span>
        <select
          :value="level"
          class="field-input !w-auto !px-[10px] !py-[6px]"
          @change="onLevelChange"
        >
          <option v-for="l in LEVELS" :key="l.value" :value="l.value">{{ l.label }}</option>
        </select>
        <button class="btn-ghost" @click="autoLevel">
          自动（当前建议 {{ suggestLevel(input) }}）
        </button>
      </div>
      <div class="flex items-center gap-[8px]">
        <span class="text-body-sm text-text-muted dark:text-text-muted-dark">尺寸</span>
        <select
          :value="size"
          class="field-input !w-auto !px-[10px] !py-[6px]"
          @change="onSizeChange"
        >
          <option v-for="s in SIZES" :key="s.value" :value="s.value">{{ s.label }}</option>
        </select>
      </div>
      <span class="text-body-sm text-text-muted dark:text-text-muted-dark"
        >{{ charCount }} 字符</span
      >
    </div>

    <div class="flex items-start gap-[16px]">
      <div
        class="grid h-[280px] w-[280px] shrink-0 place-items-center rounded-lg border border-border bg-white p-[12px] dark:border-border-dark"
      >
        <img v-if="dataUrl" :src="dataUrl" alt="二维码" class="h-full w-full" />
        <span v-else class="text-body-sm text-text-muted dark:text-text-muted-dark">等待输入…</span>
      </div>
      <div class="flex flex-col gap-[10px]">
        <button class="btn-primary" :disabled="!dataUrl" @click="download">下载 PNG</button>
        <p
          class="max-w-[280px] text-body-sm leading-[1.55] text-text-muted dark:text-text-muted-dark"
        >
          容错级别越高，二维码越耐遮挡/破损，但信息密度下降；内容较长时建议用自动建议。
        </p>
      </div>
    </div>
    <p v-if="error" class="text-body-sm text-tertiary-strong dark:text-tertiary-dark">
      {{ error }}
    </p>
  </div>
</template>
