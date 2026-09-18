import { expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import { UiBadge } from '@/core/ui'
import RuntimeStatus from './RuntimeStatus.vue'
import { runtimeStatus } from './runtimeStatus'
import { composeStatus } from './compose/composeTemplates'

it('服务、容器、编排的同类状态使用相同文案和色调', () => {
  for (const status of ['active', 'running', composeStatus('running(2)')])
    expect(runtimeStatus(status)).toEqual({ label: '运行中', tone: 'success' })
  for (const status of ['inactive', 'exited', composeStatus('exited(2)')])
    expect(runtimeStatus(status)).toEqual({ label: '已停止', tone: 'neutral' })
  expect(runtimeStatus('activating')).toEqual({ label: '启动中', tone: 'warning' })
  expect(runtimeStatus('failed').tone).toBe('danger')
  expect(runtimeStatus(composeStatus('dead(1)')).tone).toBe('danger')
  expect(runtimeStatus(composeStatus('running(1), exited(1)'))).toEqual({
    label: '部分运行',
    tone: 'warning',
  })
  expect(runtimeStatus(composeStatus('paused(1), exited(1)')).tone).toBe('warning')
  expect(runtimeStatus('unexpected')).toEqual({ label: 'unexpected', tone: 'neutral' })
  expect(runtimeStatus('').label).toBe('未知')
})

it('执行中暂时覆盖展示，操作结束恢复真实状态', async () => {
  const wrapper = mount(RuntimeStatus, { props: { status: 'running', busy: true } })
  expect(wrapper.text()).toBe('执行中')
  expect(wrapper.getComponent(UiBadge).props('tone')).toBe('warning')
  await wrapper.setProps({ busy: false })
  expect(wrapper.text()).toBe('运行中')
  expect(wrapper.getComponent(UiBadge).props('tone')).toBe('success')
  wrapper.unmount()
})
