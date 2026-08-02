<script setup lang="ts">
/**
 * 颜色选择器 · hex/rgb/hsl 互转 + 随机色 + 屏幕取色（Tauri）+ WCAG 对比度提示
 */
import { ref } from "vue";
import {
  contrastRatio,
  hexToRgb,
  hslToHex,
  parseColorInput,
  randomColor,
  rgbToHsl,
} from "./useColor";
import { useCopy } from "@/tools/shared/useClipboard";

const { copyText } = useCopy();

const current = ref("#F0562C");
const inputText = ref("#F0562C");
const pickError = ref("");

const rgb = computed(() => hexToRgb(current.value)!);
const hsl = computed(() => rgbToHsl(rgb.value));

const contrastWhite = computed(() => contrastRatio(rgb.value, { r: 255, g: 255, b: 255 }));
const contrastBlack = computed(() => contrastRatio(rgb.value, { r: 0, g: 0, b: 0 }));
const bestContrast = computed(() => Math.max(contrastWhite.value, contrastBlack.value));
const bestTextColor = computed(() =>
  contrastWhite.value >= contrastBlack.value ? "#FFFFFF" : "#000000"
);

function applyInput() {
  const p = parseColorInput(inputText.value);
  if (p) {
    current.value = p.hex;
    inputText.value = p.hex.toUpperCase();
    pickError.value = "";
  } else {
    pickError.value = "无法解析，支持 #fff / #ffffff / rgb(r,g,b)";
  }
}

function applyHex(hex: string) {
  current.value = hex;
  inputText.value = hex.toUpperCase();
}

function random() {
  applyHex(randomColor());
}

function onHslInput(part: keyof typeof hsl.value, v: number) {
  applyHex(hslToHex({ ...hsl.value, [part]: v }));
}

const PRESETS = [
  "#F0562C",
  "#C2410C",
  "#F97066",
  "#067647",
  "#0A7A3F",
  "#12B76A",
  "#175CD3",
  "#2E90FA",
  "#175CD3",
  "#7A5AF8",
  "#B54708",
  "#F79009",
  "#98A2AD",
  "#344054",
  "#101828",
  "#F2F4F7",
];
</script>

<template>
  <div class="flex max-w-[720px] flex-col gap-[14px]">
    <div class="flex gap-[14px]">
      <!-- 色块 + 输入 -->
      <div class="flex flex-col gap-[10px]">
        <div
          class="grid h-[120px] w-[120px] place-items-center rounded-lg border border-border transition-colors duration-200 dark:border-border-dark"
          :style="{ backgroundColor: current }"
        >
          <span
            class="rounded-[4px] bg-black/20 px-[8px] py-[2px] font-mono text-body font-medium text-white"
            >{{ current }}</span
          >
        </div>
        <button class="btn-secondary" @click="random">随机色</button>
        <!-- 色盘：原生全色系选择器（Windows 系统色盘） -->
        <label
          class="btn-secondary flex cursor-pointer items-center justify-center gap-[8px]"
          title="打开色盘选择"
        >
          <span
            class="h-[14px] w-[14px] rounded-[3px] border border-border-strong"
            :style="{ backgroundColor: current }"
          />
          色盘取号
          <input
            type="color"
            class="hidden"
            :value="current"
            @input="applyHex(($event.target as HTMLInputElement).value)"
          />
        </label>
      </div>

      <!-- 数值区 -->
      <div class="min-w-0 flex-1">
        <div class="flex items-center gap-[8px]">
          <input
            :value="inputText"
            class="field-input font-mono"
            spellcheck="false"
            @input="inputText = ($event.target as HTMLInputElement).value"
            @keyup.enter="applyInput"
            @blur="applyInput"
          />
          <button class="btn-primary shrink-0" @click="applyInput">应用</button>
        </div>
        <p
          v-if="pickError"
          class="mt-[6px] text-body-sm text-tertiary-strong dark:text-tertiary-dark"
        >
          {{ pickError }}
        </p>

        <div class="mt-[12px] flex flex-col gap-[8px]">
          <div class="flex items-center gap-[10px] text-body">
            <span class="w-[44px] text-body-sm text-text-muted dark:text-text-muted-dark">HEX</span>
            <code class="min-w-0 flex-1 font-mono text-primary dark:text-primary-dark">{{
              current.toUpperCase()
            }}</code>
            <button class="btn-ghost" @click="copyText(current.toUpperCase(), 'HEX 已复制')">
              复制
            </button>
          </div>
          <div class="flex items-center gap-[10px] text-body">
            <span class="w-[44px] text-body-sm text-text-muted dark:text-text-muted-dark">RGB</span>
            <code class="min-w-0 flex-1 font-mono text-primary dark:text-primary-dark"
              >rgb({{ rgb.r }}, {{ rgb.g }}, {{ rgb.b }})</code
            >
            <button
              class="btn-ghost"
              @click="copyText(`rgb(${rgb.r}, ${rgb.g}, ${rgb.b})`, 'RGB 已复制')"
            >
              复制
            </button>
          </div>
          <div class="flex items-center gap-[10px]">
            <span class="w-[44px] shrink-0 text-body-sm text-text-muted dark:text-text-muted-dark"
              >HSL</span
            >
            <input
              type="range"
              min="0"
              max="360"
              :value="hsl.h"
              class="flex-1 accent-tertiary-strong"
              title="色相 H"
              @input="onHslInput('h', Number(($event.target as HTMLInputElement).value))"
            />
            <input
              type="range"
              min="0"
              max="100"
              :value="hsl.s"
              class="w-[100px] accent-tertiary-strong"
              title="饱和度 S"
              @input="onHslInput('s', Number(($event.target as HTMLInputElement).value))"
            />
            <input
              type="range"
              min="0"
              max="100"
              :value="hsl.l"
              class="w-[100px] accent-tertiary-strong"
              title="亮度 L"
              @input="onHslInput('l', Number(($event.target as HTMLInputElement).value))"
            />
            <code
              class="w-[110px] shrink-0 text-right font-mono text-body-sm text-secondary dark:text-secondary-dark"
              >{{ hsl.h }}° {{ hsl.s }}% {{ hsl.l }}%</code
            >
          </div>
        </div>

        <!-- 对比度提示 -->
        <div
          class="mt-[12px] flex items-center gap-[8px] rounded-md border border-border px-[12px] py-[8px] text-body-sm dark:border-border-dark"
          :style="{ backgroundColor: current, color: bestTextColor }"
        >
          <span class="font-medium">对比度 {{ bestContrast.toFixed(2) }} : 1</span>
          <span :class="bestContrast >= 4.5 ? 'text-success' : 'opacity-80'">
            {{ bestContrast >= 4.5 ? "✓ AA 达标" : "低于 AA（4.5:1）" }}
          </span>
        </div>
      </div>
    </div>

    <!-- 预设色板 -->
    <div class="flex flex-wrap gap-[8px]">
      <button
        v-for="c in PRESETS"
        :key="c"
        class="h-[28px] w-[28px] rounded-md border border-border transition-transform hover:scale-110 dark:border-border-dark"
        :class="
          current === c ? 'ring-2 ring-tertiary-strong ring-offset-2 dark:ring-tertiary-dark' : ''
        "
        :style="{ backgroundColor: c }"
        :title="c"
        @click="applyHex(c)"
      />
    </div>
  </div>
</template>
