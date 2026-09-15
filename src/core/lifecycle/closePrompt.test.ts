/**
 * 关闭协商弹窗正文的文案用例
 *
 * 锁定的是拼接格式：真机走查里出现过 `关闭后以下状态会丢失或中断：；· frp：…` 这种
 * 连续标点，类型检查看不见字符串拼接，只能靠断言兜住。
 */
import { describe, expect, it } from 'vitest'
import { formatClosePromptMessage } from './closePrompt'

describe('formatClosePromptMessage', () => {
  it('单条拒绝理由：前缀与理由之间只出现一个冒号', () => {
    const text = formatClosePromptMessage([{ owner: 'frp', message: '有任务正在运行' }])

    expect(text).toBe('关闭后以下状态会丢失或中断：· frp：有任务正在运行')
  })

  it('多条理由按 owner 逐条列出，且不出现连续标点', () => {
    const text = formatClosePromptMessage([
      { owner: 'frp', message: '有任务正在运行' },
      { owner: 'ssh', message: '2 个会话仍在连接' },
    ])

    expect(text).toBe('关闭后以下状态会丢失或中断：· frp：有任务正在运行；· ssh：2 个会话仍在连接')
    expect(text).not.toMatch(/[：；]{2}/)
  })

  it('owner 没给出理由时不留悬空冒号', () => {
    expect(formatClosePromptMessage([])).toBe('关闭后以下状态会丢失或中断：未报告具体原因')
  })
})
