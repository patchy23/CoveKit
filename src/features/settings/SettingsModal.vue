<script setup lang="ts">
/**
 * SettingsModal · 设置弹窗
 * 外观 / 快捷键 / 通用 / 剪贴板策略 / 工具级设置（settingsSchema 自动渲染表单，架构 §8）
 */
import { computed } from 'vue'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { getTools } from '@/core/registry/toolRegistry'
import type { SettingsField } from '@/core/registry/types'
import { UiButton, UiCheckbox, UiInput, UiModal as BaseModal, UiSelect as Select } from '@/core/ui'
import AppIcon from '@/features/ui/AppIcon.vue'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()
const settings = useSettingsStore()

const toolsWithSettings = computed(() => getTools().filter((t) => t.settingsSchema?.length))

function fieldValue(field: SettingsField, toolId: string) {
  return settings.getToolSetting(toolId, field.key, field.default ?? '')
}
function onFieldChange(field: SettingsField, toolId: string, value: unknown) {
  settings.setToolSetting(toolId, field.key, value)
}

async function chooseDownloadDirectory() {
  const selected = await dialogOpen({
    directory: true,
    multiple: false,
    defaultPath: settings.settings.defaultDownloadDirectory || undefined,
  })
  if (typeof selected === 'string') await settings.set('defaultDownloadDirectory', selected)
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
          <label class="field-label flex flex-col gap-[6px]">
            主题
            <Select
              :model-value="settings.settings.theme"
              :options="[
                { value: 'system', label: '跟随系统' },
                { value: 'light', label: '浅色' },
                { value: 'dark', label: '深色' },
              ]"
              @update:model-value="settings.set('theme', $event as 'light' | 'dark' | 'system')"
            />
          </label>
          <label class="field-label flex flex-col gap-[6px]">
            语言
            <Select
              :model-value="settings.settings.language"
              :options="[
                { value: 'zh-CN', label: '简体中文' },
                { value: 'en-US', label: 'English（M4）' },
              ]"
              @update:model-value="settings.set('language', $event as 'zh-CN' | 'en-US')"
            />
          </label>
        </div>
      </section>

      <!-- 快捷键与通用 -->
      <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
        <h3 class="text-h2 font-bold dark:text-primary-dark">快捷键与通用</h3>
        <div class="mt-sm flex flex-col gap-sm">
          <label class="field-label flex flex-col gap-[6px]">
            全局唤起快捷键
            <Select
              :model-value="settings.settings.globalHotkey"
              :options="[
                { value: 'Ctrl+Shift+Space', label: 'Ctrl + Shift + Space' },
                { value: 'Alt+Space', label: 'Alt + Space' },
                { value: 'Ctrl+Alt+Space', label: 'Ctrl + Alt + Space' },
                { value: 'Ctrl+Shift+`', label: 'Ctrl + Shift + `' },
                { value: 'Ctrl+Shift+O', label: 'Ctrl + Shift + O' },
              ]"
              @update:model-value="settings.set('globalHotkey', $event)"
            />
            <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
              保存后立即生效；被系统占用时自动降级并提示
            </span>
          </label>
          <label
            class="flex cursor-pointer items-center justify-between rounded-sm border border-border px-[12px] py-[9px] text-body font-medium dark:border-border-dark"
          >
            <span class="dark:text-primary-dark">开机自启</span>
            <UiCheckbox
              :model-value="settings.settings.launchAtStartup"
              @update:model-value="settings.set('launchAtStartup', $event)"
            />
          </label>
          <label class="field-label flex flex-col gap-[6px]">
            默认下载目录
            <div class="flex gap-[8px]">
              <UiInput
                :model-value="settings.settings.defaultDownloadDirectory"
                class="flex-1 font-mono"
                placeholder="未设置时使用系统保存位置"
                @update:model-value="settings.set('defaultDownloadDirectory', String($event))"
              />
              <UiButton @click="chooseDownloadDirectory">选择目录</UiButton>
            </div>
            <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
              SSH 下载及后续支持下载的工具会默认从此目录保存
            </span>
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
            class="field-label flex flex-col gap-[6px]"
          >
            {{ field.label }}
            <Select
              v-if="field.type === 'select'"
              :model-value="String(fieldValue(field, t.id))"
              :options="field.options ?? []"
              @update:model-value="onFieldChange(field, t.id, $event)"
            />
            <UiCheckbox
              v-else-if="field.type === 'toggle'"
              :model-value="Boolean(fieldValue(field, t.id))"
              @update:model-value="onFieldChange(field, t.id, $event)"
            />
            <UiInput
              v-else-if="field.type === 'number'"
              type="number"
              :model-value="Number(fieldValue(field, t.id))"
              @update:model-value="onFieldChange(field, t.id, Number($event))"
            />
            <UiInput
              v-else
              :model-value="String(fieldValue(field, t.id))"
              @update:model-value="onFieldChange(field, t.id, $event)"
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
