/**
 * ProcessTab：进程详情弹窗的行为契约
 *
 * 覆盖用户可感知行为与结果归属责任（并发、连接切换、失败提示）：
 * 1 → 打开详情展示 ps 字段与完整命令行；远端已无该进程时给「已退出」提示并禁用结束
 * 2 → 并发打开两个进程时，晚到的旧结果不得覆盖当前详情
 * 3 → 连接切换后晚到的结果被丢弃，弹窗不残留上一个会话的数据
 * 4 → 详情加载失败：关闭弹窗并提示，不静默吞错
 */
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia, type Pinia } from 'pinia'
import { nextTick } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { i18n } from '@/i18n'
import { useUiStore } from '@/stores/ui'
import type { ProcessDetail, ServerConnection } from '../contracts'
import ProcessTab from './ProcessTab.vue'
import { UiSwitch, UiSearchInput, UiButton } from '@/core/ui'

/* ── 本插件 IPC 门面假实现（返回值由各用例设定） ── */

const ipcMock = vi.hoisted(() => ({
  sshProcessList: vi.fn(),
  sshProcessDetail: vi.fn(),
  sshProcessKill: vi.fn(),
}))

vi.mock('../ipc', () => ({ ipc: ipcMock }))

/** 等异步加载与随后的一轮渲染落定 */
async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 0))
  await nextTick()
}

/** 可手动完成/拒绝的挂起 Promise，用于制造并发归属竞态 */
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

/** 断言用的最小连接对象（组件只读 sessionId / status） */
function connection(sessionId: string): ServerConnection {
  return { sessionId, status: 'connected' } as unknown as ServerConnection
}

/** 进程列表条目 */
function processRow(pid: number) {
  return {
    pid,
    user: 'root',
    cpuPercent: 1.5,
    memoryPercent: 0.2,
    memoryBytes: 65536,
    startedAt: 0,
    command: `proc-${pid}`,
  }
}

/** 进程详情返回值 */
function processDetail(pid: number, command: string): ProcessDetail {
  return {
    pid,
    found: true,
    user: 'root',
    ppid: 1,
    tty: '?',
    started: '09:12',
    cpuTime: '00:01:23',
    command,
    raw: `${pid} root ${command}`,
  }
}

/** 弹窗内容经 DialogPortal 渲染到 body，断言取 body 文本 */
function bodyText() {
  return document.body.textContent ?? ''
}

/**
 * 已挂载的包装器：用例结束必须先 unmount 再清空 body。
 *
 * 为什么不能只在 beforeEach 里清空 body：弹窗经 DialogPortal 把节点挂在 body 上，
 * 直接清空会让仍存活的组件实例失去 DOM 锚点，Vue 后续 patch 找不到 nextSibling，
 * 更新静默失效——表现为「组件状态已变（可探查到 detail/loading 都正确）但界面仍停在旧内容」。
 */
const mountedWrappers: ReturnType<typeof mount>[] = []

afterEach(() => {
  while (mountedWrappers.length) mountedWrappers.pop()?.unmount()
  // unmount 完成后再清空残留节点（顺序颠倒会复现上面描述的锚点失效）
  document.body.innerHTML = ''
})

/** 挂载进程页签（真实 pinia / i18n，IPC 走假实现） */
async function mountTab(pinia: Pinia = createPinia(), sessionId = 's1') {
  setActivePinia(pinia)
  const wrapper = mount(ProcessTab, {
    props: { connection: connection(sessionId) },
    global: { plugins: [pinia, i18n] },
  })
  await settle()
  mountedWrappers.push(wrapper)
  return wrapper
}

/** 点第 index 行（0 起）的「详情」按钮 */
async function clickDetail(wrapper: ReturnType<typeof mount>, index: number) {
  const buttons = wrapper.findAll('button').filter((button) => button.text() === '详情')
  await buttons[index]?.trigger('click')
  await settle()
}

describe('ProcessTab 进程详情', () => {
  beforeEach(() => {
    localStorage.clear()
    ipcMock.sshProcessList.mockReset().mockResolvedValue([processRow(100), processRow(200)])
    ipcMock.sshProcessDetail.mockReset()
    ipcMock.sshProcessKill.mockReset()
  })

  it('展示 ps 字段与完整命令行', async () => {
    ipcMock.sshProcessDetail.mockResolvedValue(processDetail(100, '/usr/bin/nginx -g daemon off'))

    const wrapper = await mountTab()
    await clickDetail(wrapper, 0)

    expect(bodyText()).toContain('进程详情 · PID 100')
    expect(bodyText()).toContain('父进程')
    expect(bodyText()).toContain('00:01:23')
    expect(bodyText()).toContain('完整命令行')
    expect(bodyText()).toContain('/usr/bin/nginx -g daemon off')
    expect(bodyText()).toContain('ps 原始输出')
  })

  it('默认隐藏内核线程，切换恢复且带关键词刷新不截断完整列表', async () => {
    ipcMock.sshProcessList.mockResolvedValue([
      processRow(100),
      { ...processRow(2), command: '[kthreadd]', memoryBytes: 0 },
    ])
    const wrapper = await mountTab()
    expect(wrapper.findAll('tbody tr')).toHaveLength(1)
    expect(wrapper.text()).toContain('已隐藏 1 个系统项')
    wrapper.getComponent(UiSwitch).vm.$emit('update:modelValue', false)
    await settle()
    expect(wrapper.findAll('tbody tr')).toHaveLength(2)
    wrapper.getComponent(UiSearchInput).vm.$emit('update:modelValue', '100')
    await settle()
    wrapper
      .findAllComponents(UiButton)
      .find((button) => button.props('title') === '刷新进程列表')!
      .vm.$emit('click')
    await settle()
    expect(ipcMock.sshProcessList).toHaveBeenLastCalledWith({ connectionId: 's1', sortBy: 'cpu' })
    wrapper.getComponent(UiSearchInput).vm.$emit('update:modelValue', '')
    await settle()
    expect(wrapper.findAll('tbody tr')).toHaveLength(2)
  })

  it('远端已无该进程时提示已退出且不允许再结束', async () => {
    ipcMock.sshProcessDetail.mockResolvedValue({
      pid: 100,
      found: false,
      raw: 'ps: 100: No such process',
    })

    const wrapper = await mountTab()
    await clickDetail(wrapper, 0)

    expect(bodyText()).toContain('远端已无该进程')
    expect(bodyText()).toContain('ps: 100: No such process')
    // 弹窗内容在 DialogPortal（body）里，用真实 DOM 查按钮
    const killButton = [...document.querySelectorAll('button')].find(
      (button) => button.textContent?.trim() === '结束进程'
    )
    expect(killButton, '进程不存在时应保留「结束进程」按钮').toBeTruthy()
    expect(killButton?.disabled, '进程不存在时「结束进程」应禁用').toBe(true)
  })

  it('并发打开时晚到的旧结果不覆盖当前详情', async () => {
    const slow = deferred<ProcessDetail>()
    const fast = deferred<ProcessDetail>()
    ipcMock.sshProcessDetail
      .mockImplementationOnce(() => slow.promise)
      .mockImplementationOnce(() => fast.promise)

    const wrapper = await mountTab()
    await clickDetail(wrapper, 0)
    await clickDetail(wrapper, 1)
    fast.resolve(processDetail(200, '/usr/bin/second'))
    await settle()
    slow.resolve(processDetail(100, '/usr/bin/first'))
    await settle()

    expect(bodyText()).toContain('/usr/bin/second')
    expect(bodyText()).not.toContain('/usr/bin/first')
  })

  it('连接切换后晚到的结果被丢弃且弹窗关闭', async () => {
    const pending = deferred<ProcessDetail>()
    ipcMock.sshProcessDetail.mockImplementation(() => pending.promise)

    const wrapper = await mountTab()
    await clickDetail(wrapper, 0)
    await wrapper.setProps({ connection: connection('s2') })
    pending.resolve(processDetail(100, '/usr/bin/first'))
    await settle()

    expect(bodyText()).not.toContain('/usr/bin/first')
    expect(bodyText()).not.toContain('完整命令行')
  })

  it('加载失败时关闭弹窗并提示', async () => {
    ipcMock.sshProcessDetail.mockRejectedValue(new Error('连接已断开'))
    const pinia = createPinia()

    const wrapper = await mountTab(pinia)
    const ui = useUiStore()
    const toast = vi.spyOn(ui, 'toast')
    await clickDetail(wrapper, 0)

    expect(bodyText()).not.toContain('完整命令行')
    expect(toast).toHaveBeenCalledWith(expect.stringContaining('进程详情加载失败'))
  })
})
