<script setup lang="ts">
/**
 * SettingsPage · 设置页（框架级整页模式，铺满右侧内容区含页签条区域）
 * 由侧栏「设置」触发 ui.openSettings()，与工作区整体互切（v-show 保留工具页签状态）；
 * 右上角返回按钮退出。外观 / 快捷键与通用 / 凭证管理 / 工具级设置（settingsSchema 自动渲染）。
 */
import { computed, ref, watch, onMounted } from 'vue'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { getTools } from '@/core/registry/toolRegistry'
import type { SettingsField } from '@/core/registry/types'
import { UiButton, UiCheckbox, UiInput, UiModal, UiSelect as Select } from '@/core/ui'
import AppIcon from '@/features/ui/AppIcon.vue'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'
import { ipc } from '@/core/ipc/ipc'
import CredentialManagerPage from '@/features/vault/CredentialManagerPage.vue'

const settings = useSettingsStore()
const ui = useUiStore()

/** 凭证管理弹窗开关（框架功能，不走工具页签） */
const vaultVisible = ref(false)

/** 凭证库条数（进入页面与关闭管理弹窗时刷新；获取失败显示「—」） */
const vaultCount = ref<number | null>(null)
async function refreshVaultCount() {
  try {
    vaultCount.value = (await ipc.vaultList()).length
  } catch {
    vaultCount.value = null
  }
}
onMounted(refreshVaultCount)
watch(vaultVisible, (v) => {
  if (!v) void refreshVaultCount()
})

/** 打开凭证管理弹窗 */
function openVault() {
  vaultVisible.value = true
}

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
  <!-- 整页模式：铺满右侧内容区（含页签条区域），自带滚动 -->
  <div class="min-h-0 flex-1 overflow-y-auto px-md py-md">
    <div class="mx-auto w-full max-w-[760px]">
      <!-- 页头 -->
      <div class="flex items-center gap-[12px]">
        <div
          class="grid h-[46px] w-[46px] shrink-0 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
        >
          <AppIcon name="sliders" :size="23" class="text-tertiary-strong dark:text-tertiary-dark" />
        </div>
        <div>
          <h2 class="text-h1 font-extrabold tracking-[-0.02em] dark:text-primary-dark">设置</h2>
          <p class="mt-[3px] text-body text-secondary dark:text-secondary-dark">
            外观、快捷键、凭证与工具级配置
          </p>
        </div>
        <button
          class="ml-auto grid h-8 w-8 shrink-0 cursor-pointer place-items-center rounded-[9px] bg-neutral text-secondary transition-colors duration-150 hover:bg-border hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
          title="返回"
          @click="ui.closeSettings()"
        >
          <AppIcon name="close" :size="14" />
        </button>
      </div>

      <div class="mt-[20px] flex flex-col gap-md pb-[24px]">
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

        <!-- 凭证管理（框架功能，弹窗承载） -->
        <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
          <h3 class="flex items-center gap-[8px] text-h2 font-bold dark:text-primary-dark">
            <AppIcon name="lock" :size="15" class="text-tertiary-strong dark:text-tertiary-dark" />
            凭证管理
          </h3>
          <div class="mt-sm flex items-center justify-between gap-sm">
            <p class="text-body-sm text-text-muted dark:text-text-muted-dark">
              密码 / 私钥 / Token 加密存本机（keyring 主密钥），插件只引用凭证 ID。已存
              <span class="font-mono">{{ vaultCount ?? '—' }}</span> 条。
            </p>
            <UiButton @click="openVault">管理凭证</UiButton>
          </div>
        </section>

        <!-- 工具级设置（settingsSchema 自动渲染） -->
        <section
          v-for="t in toolsWithSettings"
          :key="t.id"
          class="rounded-lg border border-border p-[16px] dark:border-border-dark"
        >
          <h3 class="flex items-center gap-[8px] text-h2 font-bold dark:text-primary-dark">
            <AppIcon
              :name="t.icon"
              :size="15"
              class="text-tertiary-strong dark:text-tertiary-dark"
            />
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
          暂无带设置项的工具
        </p>
      </div>

      <!-- 凭证管理弹窗（xl；框架功能，独立于工具页签体系） -->
      <UiModal
        :open="vaultVisible"
        title="凭证管理"
        description="秘密加密存储在本机凭证库（keyring 主密钥），插件只引用凭证 ID，明文不出后端"
        size="xl"
        @close="vaultVisible = false"
      >
        <CredentialManagerPage />
      </UiModal>
    </div>
  </div>
</template>
