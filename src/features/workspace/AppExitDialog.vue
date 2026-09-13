<script setup lang="ts">
/**
 * AppExitDialog · 退出被拒绝提示（可靠性 T10-3）
 *
 * 托盘退出时窗口通常处于隐藏状态：后端把窗口唤到前台并把拒绝原因发过来，
 * 界面必须给出明确出路（重试退出 / 强制退出 / 取消），否则用户只会看到「点了没反应」。
 * 强制退出是显式动作，同时说明清理可能不完整。
 */
import { onMounted, onUnmounted, ref } from 'vue'
import UiButton from '@/core/ui/UiButton.vue'
import UiModal from '@/core/ui/UiModal.vue'
import { forceAppExit, requestAppExit, watchExitVeto, type ExitDecision } from '@/core/lifecycle'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()

/** 后端上报的拒绝原因（null = 无待处理退出） */
const decision = ref<ExitDecision | null>(null)
/** 处理中（防连点；按钮禁用并显示等待态） */
const busy = ref(false)
let stopWatch: (() => void) | null = null

onMounted(async () => {
  stopWatch = await watchExitVeto((payload) => {
    decision.value = payload
  })
})

onUnmounted(() => {
  stopWatch?.()
  stopWatch = null
})

/** 重试退出：先让用户在插件里处理完，再重新走一次退出流程 */
async function retryExit() {
  busy.value = true
  try {
    const result = await requestAppExit('exit')
    decision.value = result.started ? null : result
  } catch (error) {
    ui.toast(`退出请求失败：${error instanceof Error ? error.message : String(error)}`)
  } finally {
    busy.value = false
  }
}

/** 强制退出：跳过业务拦截（清理仍有总超时，可能来不及清完） */
async function forceExit() {
  busy.value = true
  try {
    await forceAppExit()
  } catch (error) {
    ui.toast(`强制退出失败：${error instanceof Error ? error.message : String(error)}`)
    busy.value = false
  }
}

/** 取消：留在应用里继续处理 */
function cancel() {
  decision.value = null
}
</script>

<template>
  <UiModal :open="decision !== null" width="min(460px, 92vw)" @close="cancel">
    <h3 class="mb-[8px] text-card-title font-medium text-primary dark:text-primary-dark">
      还有一些任务没有结束
    </h3>
    <p class="mb-[12px] text-body leading-relaxed text-secondary dark:text-secondary-dark">
      以下模块不同意现在退出，退出后它们的会话或进程会被中断：
    </p>
    <ul class="mb-[16px] flex flex-col gap-[4px]">
      <li
        v-for="item in decision?.blockers ?? []"
        :key="item"
        class="text-body-sm text-secondary dark:text-secondary-dark"
      >
        · {{ item }}
      </li>
    </ul>
    <p class="mb-[16px] text-caption text-text-muted dark:text-text-muted-dark">
      强制退出会跳过拦截，清理可能不完整（例如隧道或代理进程需要手动确认）。
    </p>
    <div class="flex justify-end gap-[8px]">
      <UiButton variant="ghost" :disabled="busy" @click="cancel">取消</UiButton>
      <UiButton variant="ghost" :disabled="busy" @click="forceExit">强制退出</UiButton>
      <UiButton :disabled="busy" @click="retryExit">{{ busy ? '处理中…' : '重试退出' }}</UiButton>
    </div>
  </UiModal>
</template>
