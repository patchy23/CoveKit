/**
 * 关闭协商弹窗的文案组装
 *
 * 单列成纯函数而不是写在 `CloseConfirmDialog.vue` 里：正文是用户可见产物，
 * 拼接符号（分号/冒号）写错类型检查发现不了，独立出来才能直接断言。
 */
import type { CloseIssue } from './types'

/** 弹窗正文前缀：说明「为什么被拦下」 */
export const CLOSE_PROMPT_PREFIX = '关闭后以下状态会丢失或中断'

/**
 * 把各 owner 上报的拒绝理由拼成弹窗正文
 *
 * @param blockers owner 上报的拒绝原因，界面按行列出；为空时给出兜底说明而不是空冒号
 * @returns 形如 `关闭后以下状态会丢失或中断：· frp：有任务正在运行`
 */
export function formatClosePromptMessage(blockers: CloseIssue[]): string {
  if (blockers.length === 0) return `${CLOSE_PROMPT_PREFIX}：未报告具体原因`
  const lines = blockers.map((item) => `· ${item.owner}：${item.message}`)
  return `${CLOSE_PROMPT_PREFIX}：${lines.join('；')}`
}
