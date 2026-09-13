<script setup lang="ts">
/**
 * DiagnosticsCard · 本地诊断（可靠性 T11-4/T11-5）
 *
 * 用户视角要解决的问题：出问题时只有一闪而过的 toast，说不清发生了什么。
 * 这里把错误码清单与运行环境集中展示，并提供「复制诊断信息」——
 * 报告只在本地生成，用户主动复制才离开本机，不做任何自动上传。
 *
 * 有界展示：列表最多 5 条，条数上限由 core/diagnostics 控制，这里不重复实现。
 */
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { UiButton } from '@/core/ui'
import { useTasksStore } from '@/stores/tasks'
import {
  clearErrors,
  collectDiagnostics,
  formatDiagnostics,
  listErrors,
  subscribeErrors,
  type AppErrorEntry,
} from '@/core/diagnostics'

const { t } = useI18n()
const tasks = useTasksStore()
const errors = ref<AppErrorEntry[]>([])
const copied = ref('')
const copyError = ref('')
/** 展示上限（只是显示截断，清单本身按 core 的上限保留） */
const VISIBLE_ERRORS = 5

let stopSubscription: (() => void) | null = null

onMounted(async () => {
  stopSubscription = subscribeErrors((next) => {
    errors.value = next
  })
  await tasks.start()
})

onUnmounted(() => {
  stopSubscription?.()
  stopSubscription = null
})

const visibleErrors = computed(() => errors.value.slice(0, VISIBLE_ERRORS))
const hiddenCount = computed(() => Math.max(0, errors.value.length - VISIBLE_ERRORS))
/** 最近一次框架长任务的结束结果（迁移失败这类事实要看得见） */
const lastFinished = computed(() => tasks.finished[0] ?? null)

/** 复制诊断信息（失败必须说出来：剪贴板可能被系统策略拒绝） */
async function copyDiagnostics() {
  copyError.value = ''
  try {
    const report = await collectDiagnostics()
    await navigator.clipboard.writeText(formatDiagnostics(report))
    copied.value = t('settings.diagnosticsCopied')
    window.setTimeout(() => {
      copied.value = ''
    }, 2500)
  } catch (reason) {
    copyError.value = reason instanceof Error ? reason.message : String(reason)
  }
}

/** 清空错误清单 */
function clearAll() {
  clearErrors()
  errors.value = listErrors()
}
</script>

<template>
  <section class="rounded-lg border border-border p-[16px] dark:border-border-dark">
    <h3 class="text-h2 font-bold dark:text-primary-dark">{{ t('settings.diagnostics') }}</h3>
    <div class="mt-sm space-y-xs text-body-sm text-text-muted dark:text-text-muted-dark">
      <p>{{ t('settings.diagnosticsActiveTasks', { count: tasks.activeCount }) }}</p>
      <p v-if="lastFinished">
        {{
          t('settings.diagnosticsLastTask', {
            kind: lastFinished.kind,
            state: lastFinished.state,
            code: lastFinished.error ? lastFinished.error.code : '',
          })
        }}
      </p>
      <p v-if="tasks.syncError">
        {{ t('settings.diagnosticsSyncFailed', { message: tasks.syncError }) }}
      </p>
      <p v-if="visibleErrors.length === 0">{{ t('settings.diagnosticsNoErrors') }}</p>
      <ul v-else class="space-y-[2px]">
        <li v-for="entry in visibleErrors" :key="`${entry.code}-${entry.firstAt}`">
          [{{ entry.code }} × {{ entry.count }}] {{ entry.message }}
        </li>
      </ul>
      <p v-if="hiddenCount > 0">
        {{ t('settings.diagnosticsMoreErrors', { count: hiddenCount }) }}
      </p>
    </div>
    <div class="mt-sm flex items-center gap-sm">
      <UiButton @click="copyDiagnostics">{{ t('settings.copyDiagnostics') }}</UiButton>
      <UiButton v-if="errors.length > 0" variant="ghost" @click="clearAll">
        {{ t('settings.clearDiagnostics') }}
      </UiButton>
      <span v-if="copied" class="text-body-sm text-text-muted dark:text-text-muted-dark">{{
        copied
      }}</span>
      <span v-if="copyError" class="text-body-sm text-danger-strong dark:text-danger-dark">{{
        copyError
      }}</span>
    </div>
  </section>
</template>
