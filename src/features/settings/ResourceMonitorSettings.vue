<script setup lang="ts">
/** 资源监测设置：串行保存期间禁用修改，失败由设置存储回滚并就地展示。 */
import { nextTick, onMounted, ref, watch } from 'vue'
import { UiButton, UiCheckbox, UiSwitch } from '@/core/ui'
import { useSettingsStore } from '@/stores/settings'
import { useResourceMonitorStore } from '@/stores/resourceMonitor'
const settings = useSettingsStore()
const monitor = useResourceMonitorStore()
const busy = ref(false)
const saveError = ref('')
const section = ref<HTMLElement>()
async function save(
  key: 'resourceMonitorEnabled' | 'resourceMonitorTools',
  value: boolean | string[]
) {
  busy.value = true
  saveError.value = ''
  try {
    if (key === 'resourceMonitorEnabled') await settings.set(key, value as boolean)
    else await settings.set(key, value as string[])
  } catch (error) {
    saveError.value = String(error)
    // 本分区就地显示同一保存错误，避免同时在页面顶部重复提示。
    if (settings.saveError === saveError.value) settings.clearSaveError()
  } finally {
    busy.value = false
  }
}
function select(id: string, value: boolean) {
  void save(
    'resourceMonitorTools',
    value ? [...new Set([...monitor.selected, id])] : monitor.selected.filter((item) => item !== id)
  )
}
async function reveal() {
  if (!monitor.focusSettings) return
  monitor.focusSettings = false
  await nextTick()
  section.value?.scrollIntoView({ block: 'start' })
  section.value?.focus({ preventScroll: true })
}
onMounted(() => {
  void monitor.loadCatalog()
  void reveal()
})
watch(
  () => monitor.focusSettings,
  () => void reveal()
)
</script>

<template>
  <section
    ref="section"
    tabindex="-1"
    aria-label="资源监测"
    class="rounded-lg border border-border p-[16px] dark:border-border-dark"
  >
    <h3
      class="mb-md border-l-[3px] border-tertiary pl-sm text-h2 text-primary dark:text-primary-dark"
    >
      资源监测
    </h3>
    <UiSwitch
      class="w-full flex-row-reverse justify-between"
      :model-value="monitor.enabled"
      :disabled="busy"
      label="启用资源监测"
      description="在侧栏显示应用资源占用，并采集所选工具的详细指标。"
      @update:model-value="save('resourceMonitorEnabled', $event)"
    />
    <div class="mt-md">
      <div class="flex flex-wrap items-center justify-between gap-sm">
        <p class="text-body font-medium text-primary dark:text-primary-dark">
          工具详细统计
          <span class="text-secondary dark:text-secondary-dark"
            >已选 {{ monitor.selected.length }}</span
          >
        </p>
        <div class="flex gap-xs">
          <UiButton
            variant="ghost"
            size="xs"
            :disabled="!monitor.enabled || busy || !monitor.supportedTools.length"
            @click="
              save(
                'resourceMonitorTools',
                monitor.supportedTools.map((tool) => tool.id)
              )
            "
            >全选</UiButton
          >
          <UiButton
            variant="ghost"
            size="xs"
            :disabled="!monitor.enabled || busy || !monitor.selected.length"
            @click="save('resourceMonitorTools', [])"
            >清空</UiButton
          >
        </div>
      </div>
      <p
        v-if="monitor.catalogLoading"
        class="mt-sm text-body-sm text-secondary dark:text-secondary-dark"
      >
        正在读取支持的工具…
      </p>
      <div v-else-if="monitor.catalogError" class="mt-sm">
        <p class="select-text text-body-sm text-warning-strong dark:text-warning-dark">
          {{ monitor.catalogError }}
        </p>
        <UiButton size="xs" class="mt-xs" @click="monitor.loadCatalog()">重试</UiButton>
      </div>
      <div v-else class="mt-sm grid grid-cols-1 gap-sm sm:grid-cols-2">
        <div v-for="tool in monitor.supportedTools" :key="tool.id">
          <UiCheckbox
            :model-value="monitor.selected.includes(tool.id)"
            :label="tool.name"
            :disabled="!monitor.enabled || busy"
            @update:model-value="select(tool.id, $event)"
          />
          <p class="ml-[24px] text-caption text-secondary dark:text-secondary-dark">
            IPC 请求、已接入的订阅与定时器
          </p>
        </div>
      </div>
      <p v-if="!monitor.enabled" class="mt-sm text-body-sm text-secondary dark:text-secondary-dark">
        开启资源监测后可调整；已选工具会保留。
      </p>
      <p class="mt-sm text-body-sm text-secondary dark:text-secondary-dark">
        应用整体占用始终包含所有工具；取消勾选仅停止该工具的详细统计。全部取消时只显示应用整体。
      </p>
      <p
        v-if="saveError"
        role="alert"
        class="select-text mt-sm text-body-sm text-warning-strong dark:text-warning-dark"
      >
        保存失败：{{ saveError }}
      </p>
    </div>
  </section>
</template>
