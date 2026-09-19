<script setup lang="ts">
/**
 * CloseConfirmDialog · 关闭页签确认（可靠性 T10-2）
 *
 * 插件在关闭前拒绝了（未保存内容 / 任务运行中）时弹这个：把每个 owner 的理由逐条列出，
 * 用户可以选择「取消」或「放弃并关闭」——而不是让关闭静默丢东西。
 */
import { computed } from 'vue'
import { UiConfirmDialog } from '@/core/ui'
import { formatClosePromptMessage } from '@/core/lifecycle/closePrompt'
import { getTool } from '@/core/registry/toolRegistry'
import { useUiStore } from '@/stores/ui'

const ui = useUiStore()

/** 当前被拒绝关闭的工具名（取注册表显示名，取不到时退回 id） */
const toolName = computed(() => {
  const id = ui.closePrompt?.toolId
  if (!id) return ''
  return getTool(id)?.name ?? id
})

/** 弹窗标题 */
const title = computed(() => `${toolName.value} 还没准备好关闭`)

/** 拒绝原因逐条列出（owner：原因）；无待确认请求时为空串（此时弹窗本身不渲染） */
const message = computed(() =>
  ui.closePrompt ? formatClosePromptMessage(ui.closePrompt.blockers) : ''
)

/** 用户确认放弃并关闭 */
async function confirm() {
  await ui.confirmClose()
}

/** 用户取消关闭（保留页签与内容） */
function cancel() {
  ui.cancelClose()
}
</script>

<template>
  <UiConfirmDialog
    :open="ui.closePrompt !== null"
    :title="title"
    :message="message"
    confirm-label="放弃并关闭"
    danger
    @confirm="confirm"
    @close="cancel"
  />
</template>
