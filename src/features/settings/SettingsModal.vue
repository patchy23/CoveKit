<script setup lang="ts">
/**
 * SettingsModal · 设置弹窗
 * 外观 / 快捷键 / 通用 / 剪贴板策略 / 工具级设置（settingsSchema 自动渲染表单，架构 §8）
 */
import { computed } from "vue";
import { getTools } from "@/core/registry/toolRegistry";
import type { SettingsField } from "@/core/registry/types";
import BaseModal from "@/features/ui/BaseModal.vue";
import AppIcon from "@/features/ui/AppIcon.vue";
import { useSettingsStore } from "@/stores/settings";
import { useUiStore } from "@/stores/ui";

const ui = useUiStore();
const settings = useSettingsStore();

const toolsWithSettings = computed(() => getTools().filter((t) => t.settingsSchema?.length));

function fieldValue(field: SettingsField, toolId: string) {
  return settings.getToolSetting(toolId, field.key, field.default ?? "");
}
function onFieldChange(field: SettingsField, toolId: string, value: unknown) {
  settings.setToolSetting(toolId, field.key, value);
}
</script>

<template>
  <BaseModal
    :open="ui.settingsVisible"
    width="min(640px, 92vw)"
    @close="ui.settingsVisible = false"
  >
    <div class="flex items-center gap-[12px]">
      <div
        class="grid h-[46px] w-[46px] shrink-0 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
      >
        <AppIcon name="gear" :size="23" class="text-tertiary-strong dark:text-tertiary-dark" />
      </div>
      <div>
        <h2 class="text-h1 font-extrabold tracking-[-0.02em] dark:text-primary-dark">偏好设置</h2>
        <p class="mt-[3px] text-body text-secondary dark:text-secondary-dark">
          外观、快捷键与工具级配置
        </p>
      </div>
      <button
        class="ml-auto grid h-8 w-8 shrink-0 cursor-pointer place-items-center rounded-[9px] bg-neutral text-secondary transition-colors duration-150 hover:bg-border hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
        title="关闭"
        @click="ui.settingsVisible = false"
      >
        <AppIcon name="close" :size="14" />
      </button>
    </div>

    <div class="mt-[20px] flex flex-col gap-md">
      <!-- 外观 -->
      <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
        <h3 class="text-h2 font-bold dark:text-primary-dark">外观</h3>
        <div class="mt-sm grid grid-cols-2 gap-sm">
          <label
            class="flex flex-col gap-[6px] text-body font-medium text-secondary dark:text-secondary-dark"
          >
            主题
            <select
              class="rounded-sm border border-border-strong bg-surface-muted px-[10px] py-[8px] text-body text-primary outline-none focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
              :value="settings.settings.theme"
              @change="
                settings.set(
                  'theme',
                  ($event.target as HTMLSelectElement).value as 'light' | 'dark' | 'system'
                )
              "
            >
              <option value="system">跟随系统</option>
              <option value="light">浅色</option>
              <option value="dark">深色</option>
            </select>
          </label>
          <label
            class="flex flex-col gap-[6px] text-body font-medium text-secondary dark:text-secondary-dark"
          >
            语言
            <select
              class="rounded-sm border border-border-strong bg-surface-muted px-[10px] py-[8px] text-body text-primary outline-none focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
              :value="settings.settings.language"
              @change="
                settings.set(
                  'language',
                  ($event.target as HTMLSelectElement).value as 'zh-CN' | 'en-US'
                )
              "
            >
              <option value="zh-CN">简体中文</option>
              <option value="en-US" disabled>English（M4）</option>
            </select>
          </label>
        </div>
      </section>

      <!-- 快捷键与通用 -->
      <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
        <h3 class="text-h2 font-bold dark:text-primary-dark">快捷键与通用</h3>
        <div class="mt-sm flex flex-col gap-sm">
          <label
            class="flex flex-col gap-[6px] text-body font-medium text-secondary dark:text-secondary-dark"
          >
            全局唤起快捷键
            <input
              class="rounded-sm border border-border-strong bg-surface-muted px-[10px] py-[8px] text-body text-primary outline-none focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
              :value="settings.settings.globalHotkey"
              spellcheck="false"
              @change="settings.set('globalHotkey', ($event.target as HTMLInputElement).value)"
            />
          </label>
          <label
            class="flex cursor-pointer items-center justify-between rounded-sm border border-border px-[12px] py-[9px] text-body font-medium dark:border-border-dark"
          >
            <span class="dark:text-primary-dark">开机自启</span>
            <input
              type="checkbox"
              class="h-4 w-4 accent-[var(--color-tertiary)]"
              :checked="settings.settings.launchAtStartup"
              @change="settings.set('launchAtStartup', ($event.target as HTMLInputElement).checked)"
            />
          </label>
        </div>
      </section>

      <!-- 剪贴板策略 -->
      <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
        <h3 class="text-h2 font-bold dark:text-primary-dark">剪贴板</h3>
        <div class="mt-sm flex flex-col gap-sm">
          <label
            class="flex cursor-pointer items-center justify-between rounded-sm border border-border px-[12px] py-[9px] text-body font-medium dark:border-border-dark"
          >
            <span class="dark:text-primary-dark">启用剪贴板历史</span>
            <input
              type="checkbox"
              class="h-4 w-4 accent-[var(--color-tertiary)]"
              :checked="settings.settings.clipboard.enabled"
              @change="
                settings.set('clipboard', {
                  ...settings.settings.clipboard,
                  enabled: ($event.target as HTMLInputElement).checked,
                })
              "
            />
          </label>
          <label
            class="flex flex-col gap-[6px] text-body font-medium text-secondary dark:text-secondary-dark"
          >
            历史上限
            <input
              type="number"
              class="rounded-sm border border-border-strong bg-surface-muted px-[10px] py-[8px] text-body text-primary outline-none focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
              :value="settings.settings.clipboard.historyLimit"
              @change="
                settings.set('clipboard', {
                  ...settings.settings.clipboard,
                  historyLimit: Number(($event.target as HTMLInputElement).value),
                })
              "
            />
          </label>
          <label
            class="flex flex-col gap-[6px] text-body font-medium text-secondary dark:text-secondary-dark"
          >
            忽略的应用关键词（逗号分隔）
            <input
              class="rounded-sm border border-border-strong bg-surface-muted px-[10px] py-[8px] text-body text-primary outline-none focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
              :value="settings.settings.clipboard.ignore.join(', ')"
              placeholder="密码管理器, 密钥存储"
              @change="
                settings.set('clipboard', {
                  ...settings.settings.clipboard,
                  ignore: ($event.target as HTMLInputElement).value
                    .split(',')
                    .map((s) => s.trim())
                    .filter(Boolean),
                })
              "
            />
          </label>
        </div>
      </section>

      <!-- 工具级设置（settingsSchema 自动渲染） -->
      <section
        v-for="t in toolsWithSettings"
        :key="t.id"
        class="rounded-lg border border-border p-[16px] dark:border-border-dark"
      >
        <h3 class="flex items-center gap-[8px] text-h2 font-bold dark:text-primary-dark">
          <AppIcon :name="t.icon" :size="15" class="text-tertiary-strong dark:text-tertiary-dark" />
          {{ t.name }} 设置
        </h3>
        <div class="mt-sm flex flex-col gap-sm">
          <label
            v-for="field in t.settingsSchema"
            :key="field.key"
            class="flex flex-col gap-[6px] text-body font-medium text-secondary dark:text-secondary-dark"
          >
            {{ field.label }}
            <select
              v-if="field.type === 'select'"
              class="rounded-sm border border-border-strong bg-surface-muted px-[10px] py-[8px] text-body text-primary outline-none focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
              :value="String(fieldValue(field, t.id))"
              @change="onFieldChange(field, t.id, ($event.target as HTMLSelectElement).value)"
            >
              <option v-for="opt in field.options" :key="opt.value" :value="opt.value">
                {{ opt.label }}
              </option>
            </select>
            <input
              v-else-if="field.type === 'toggle'"
              type="checkbox"
              class="h-4 w-4 accent-[var(--color-tertiary)]"
              :checked="Boolean(fieldValue(field, t.id))"
              @change="onFieldChange(field, t.id, ($event.target as HTMLInputElement).checked)"
            />
            <input
              v-else-if="field.type === 'number'"
              type="number"
              class="rounded-sm border border-border-strong bg-surface-muted px-[10px] py-[8px] text-body text-primary outline-none focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
              :value="String(fieldValue(field, t.id))"
              @change="
                onFieldChange(field, t.id, Number(($event.target as HTMLInputElement).value))
              "
            />
            <input
              v-else
              class="rounded-sm border border-border-strong bg-surface-muted px-[10px] py-[8px] text-body text-primary outline-none focus:border-tertiary dark:border-border-strong-dark dark:bg-surface-muted-dark dark:text-primary-dark"
              :value="String(fieldValue(field, t.id))"
              @change="onFieldChange(field, t.id, ($event.target as HTMLInputElement).value)"
            />
          </label>
        </div>
      </section>

      <p v-if="!toolsWithSettings.length" class="text-body-sm text-text-muted">
        暂无带设置项的工具（首批工具设置将由 M1 工具声明 settingsSchema 后出现）
      </p>
    </div>
  </BaseModal>
</template>
