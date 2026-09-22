<script setup lang="ts">
import { UiScrollArea } from '@/core/ui'
import { UiTooltip } from '@/core/ui'
/**
 * SettingsPage · 设置页（框架级整页模式，铺满右侧内容区含页签条区域）
 * 由侧栏「设置」触发 ui.openSettings()，与工作区整体互切（v-show 保留工具页签状态）；
 * 右上角返回按钮退出。外观 / 启动与通用 / 凭证管理 / 工具级设置（settingsSchema 自动渲染）。
 */
import { computed, ref, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { open as dialogOpen } from '@tauri-apps/plugin-dialog'
import { getTools } from '@/core/registry/toolRegistry'
import type { SettingsField } from '@/core/registry/types'
import { UiBadge, UiButton, UiCheckbox, UiInput, UiModal, UiSelect as Select } from '@/core/ui'
import type { UiTone } from '@/core/ui'
import type { VaultDomainProtection, VaultProtectionStatus } from '@/core/ipc/contracts'
import AppIcon from '@/features/ui/AppIcon.vue'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'
import { ipc } from '@/core/ipc/ipc'
import CredentialManagerPage from '@/features/vault/CredentialManagerPage.vue'
import UpdateSettingsCard from './UpdateSettingsCard.vue'
import StorageSettingsCard from './StorageSettingsCard.vue'
import DiagnosticsCard from './DiagnosticsCard.vue'
import ResourceMonitorSettings from './ResourceMonitorSettings.vue'
import DataCard from './data/DataCard.vue'

const settings = useSettingsStore()
const ui = useUiStore()
const { t } = useI18n()
const themeOptions = computed(() => [
  { value: 'system', label: t('settings.themeSystem') },
  { value: 'light', label: t('settings.themeLight') },
  { value: 'dark', label: t('settings.themeDark') },
])
const languageOptions = [
  { value: 'zh-CN', label: '简体中文' },
  { value: 'en-US', label: 'English' },
]

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

/**
 * 凭证保护状态（T04-5）：主密钥实际存放在系统密钥库还是降级文件、能否解锁，都要如实展示。
 * 读取失败保持 null（显示「状态未知」），不猜测也不阻塞页面。
 */
const protection = ref<VaultProtectionStatus | null>(null)
async function refreshProtection() {
  try {
    protection.value = await ipc.vaultProtectionStatus()
  } catch {
    protection.value = null
  }
}
onMounted(refreshProtection)

/** 保护状态主文案（按可用性与后端组合，不把降级说成系统密钥库） */
function protectionLabel(domain: VaultDomainProtection): string {
  if (domain.availability === 'locked') return t('settings.protectionLocked')
  if (domain.availability === 'uninitialized') return t('settings.protectionUninitialized')
  if (domain.backend === 'system-keyring') return t('settings.protectionSystem')
  if (domain.backend === 'file-fallback') return t('settings.protectionFallback')
  return t('settings.protectionUnavailable')
}
/** 保护状态色调：绿 = 系统密钥库保护，橙 = 降级密钥文件，红 = 不可用/无法解锁 */
function protectionTone(domain: VaultDomainProtection): UiTone {
  if (domain.availability === 'locked' || domain.backend === 'unavailable') return 'danger'
  if (domain.backend === 'system-keyring') return 'success'
  return 'warning'
}
/** 数据域显示名 */
function protectionDomainName(domain: VaultDomainProtection): string {
  return domain.domain === 'vault'
    ? t('settings.protectionDomainVault')
    : t('settings.protectionDomainCredentials')
}
/** 细节说明（Rust 侧文案不含密钥材料）：锁死时附上备份恢复入口，降级时说明原因 */
const protectionDetail = computed(() => {
  const domains = protection.value?.domains ?? []
  const locked = domains.find((d) => d.availability === 'locked')
  if (locked) return t('settings.protectionRecover', { reason: locked.fallbackReason ?? '' })
  return domains.find((d) => d.fallbackReason)?.fallbackReason ?? ''
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
  <UiScrollArea as-child axis="vertical">
    <div class="min-h-0 flex-1 px-md py-md">
      <div class="mx-auto w-full max-w-[760px]">
        <!-- 页头 -->
        <div class="flex items-center gap-[12px]">
          <div
            class="grid h-[46px] w-[46px] shrink-0 place-items-center rounded-[12px] bg-tertiary-soft dark:bg-tertiary-soft-dark"
          >
            <AppIcon
              name="sliders"
              :size="23"
              class="text-tertiary-strong dark:text-tertiary-dark"
            />
          </div>
          <div>
            <h2 class="text-h1 font-extrabold tracking-[-0.02em] dark:text-primary-dark">
              {{ t('common.settings') }}
            </h2>
            <p class="mt-[3px] text-body text-secondary dark:text-secondary-dark">
              {{ t('settings.subtitle') }}
            </p>
          </div>
          <UiTooltip :content="t('common.back')">
            <button
              class="ml-auto grid h-8 w-8 shrink-0 cursor-pointer place-items-center rounded-[9px] bg-neutral text-secondary transition-colors duration-150 hover:bg-border hover:text-primary dark:bg-neutral-dark dark:text-secondary-dark dark:hover:bg-border-dark dark:hover:text-primary-dark"
              :aria-label="t('common.back')"
              @click="ui.closeSettings()"
            >
              <AppIcon name="close" :size="14" />
            </button>
          </UiTooltip>
        </div>

        <!-- 保存失败提示：任何分区的保存失败都在这里显示，不静默吞错误 -->
        <p
          v-if="settings.saveError"
          class="select-text mt-[10px] text-body-sm text-warning-strong dark:text-warning-dark"
        >
          {{ t('settings.saveFailed', { message: settings.saveError }) }}
        </p>

        <div class="mt-[20px] flex flex-col gap-md pb-[24px]">
          <!-- 外观 -->
          <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
            <h3 class="text-h2 font-bold dark:text-primary-dark">{{ t('settings.appearance') }}</h3>
            <div class="mt-sm grid grid-cols-2 gap-sm">
              <label class="field-label flex flex-col gap-[6px]">
                {{ t('settings.theme') }}
                <Select
                  :model-value="settings.settings.theme"
                  :options="themeOptions"
                  @update:model-value="settings.set('theme', $event as 'light' | 'dark' | 'system')"
                />
              </label>
              <label class="field-label flex flex-col gap-[6px]">
                {{ t('settings.language') }}
                <Select
                  :model-value="settings.settings.language"
                  :options="languageOptions"
                  @update:model-value="settings.set('language', $event as 'zh-CN' | 'en-US')"
                />
              </label>
            </div>
          </section>

          <!-- 启动与通用 -->
          <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
            <h3 class="text-h2 font-bold dark:text-primary-dark">{{ t('settings.general') }}</h3>
            <div class="mt-sm flex flex-col gap-sm">
              <label
                class="flex cursor-pointer items-center justify-between rounded-sm border border-border px-[12px] py-[9px] text-body font-medium dark:border-border-dark"
              >
                <span class="dark:text-primary-dark">{{ t('settings.launchAtStartup') }}</span>
                <UiCheckbox
                  :model-value="settings.settings.launchAtStartup"
                  @update:model-value="settings.set('launchAtStartup', $event)"
                />
              </label>
              <label class="field-label flex flex-col gap-[6px]">
                {{ t('settings.downloadDirectory') }}
                <div class="flex gap-[8px]">
                  <UiInput
                    :model-value="settings.settings.defaultDownloadDirectory"
                    class="flex-1 font-mono"
                    :placeholder="t('settings.downloadPlaceholder')"
                    @update:model-value="settings.set('defaultDownloadDirectory', String($event))"
                  />
                  <UiButton @click="chooseDownloadDirectory">{{
                    t('settings.chooseDirectory')
                  }}</UiButton>
                </div>
                <span class="text-body-sm text-text-muted dark:text-text-muted-dark">
                  {{ t('settings.downloadHint') }}
                </span>
              </label>
            </div>
          </section>

          <!-- 凭证管理（框架功能，弹窗承载） -->
          <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
            <h3 class="flex items-center gap-[8px] text-h2 font-bold dark:text-primary-dark">
              <AppIcon
                name="lock"
                :size="15"
                class="text-tertiary-strong dark:text-tertiary-dark"
              />
              {{ t('settings.vault') }}
            </h3>
            <div class="mt-sm flex items-center justify-between gap-sm">
              <p class="text-body-sm text-text-muted dark:text-text-muted-dark">
                {{ t('settings.vaultSummary', { count: vaultCount ?? '—' }) }}
              </p>
              <UiButton @click="openVault">{{ t('settings.manageVault') }}</UiButton>
            </div>
            <!-- 保护状态（T04-5）：主密钥实际来源与可用性，降级 / 无法解锁必须如实可见 -->
            <div class="mt-[10px] space-y-[6px]">
              <div
                v-for="domain in protection?.domains ?? []"
                :key="domain.domain"
                class="flex items-center gap-[8px] text-body-sm"
              >
                <span class="w-[64px] shrink-0 text-text-muted dark:text-text-muted-dark">
                  {{ protectionDomainName(domain) }}
                </span>
                <UiBadge :tone="protectionTone(domain)">{{ protectionLabel(domain) }}</UiBadge>
              </div>
              <p v-if="!protection" class="text-body-sm text-text-muted dark:text-text-muted-dark">
                {{ t('settings.protectionUnknown') }}
              </p>
              <p
                v-else-if="protectionDetail"
                class="text-body-sm text-text-muted dark:text-text-muted-dark"
              >
                {{ protectionDetail }}
              </p>
            </div>
          </section>

          <StorageSettingsCard />

          <DataCard />

          <UpdateSettingsCard />
          <DiagnosticsCard />
          <ResourceMonitorSettings />

          <!-- 工具级设置（settingsSchema 自动渲染） -->
          <section
            v-for="tool in toolsWithSettings"
            :key="tool.id"
            class="rounded-lg border border-border p-[16px] dark:border-border-dark"
          >
            <h3 class="flex items-center gap-[8px] text-h2 font-bold dark:text-primary-dark">
              <AppIcon
                :name="tool.icon"
                :size="15"
                class="text-tertiary-strong dark:text-tertiary-dark"
              />
              {{ t('settings.toolSettings', { name: tool.name }) }}
            </h3>
            <div class="mt-sm flex flex-col gap-sm">
              <label
                v-for="field in tool.settingsSchema"
                :key="field.key"
                class="field-label flex flex-col gap-[6px]"
              >
                {{ field.label }}
                <Select
                  v-if="field.type === 'select'"
                  :model-value="String(fieldValue(field, tool.id))"
                  :options="field.options ?? []"
                  @update:model-value="onFieldChange(field, tool.id, $event)"
                />
                <UiCheckbox
                  v-else-if="field.type === 'toggle'"
                  :model-value="Boolean(fieldValue(field, tool.id))"
                  @update:model-value="onFieldChange(field, tool.id, $event)"
                />
                <UiInput
                  v-else-if="field.type === 'number'"
                  type="number"
                  :model-value="Number(fieldValue(field, tool.id))"
                  @update:model-value="onFieldChange(field, tool.id, Number($event))"
                />
                <UiInput
                  v-else
                  :model-value="String(fieldValue(field, tool.id))"
                  @update:model-value="onFieldChange(field, tool.id, $event)"
                />
              </label>
            </div>
          </section>

          <p v-if="!toolsWithSettings.length" class="text-body-sm text-text-muted">
            {{ t('settings.noToolSettings') }}
          </p>
        </div>

        <!-- 凭证管理弹窗（xl；框架功能，独立于工具页签体系） -->
        <UiModal
          :open="vaultVisible"
          :title="t('settings.vault')"
          :description="t('settings.vaultDescription')"
          size="xl"
          @close="vaultVisible = false"
        >
          <CredentialManagerPage />
        </UiModal>
      </div>
    </div>
  </UiScrollArea>
</template>
