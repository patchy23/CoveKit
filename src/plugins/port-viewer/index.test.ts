import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { publishToolVisibility, resetToolVisibilityForTest } from '@/core/lifecycle'
import { scopeStats } from '@/core/lifecycle/scope'
import { UiButton, UiConfirmDialog, UiPagination, UiSelect, UiSwitch, UiTooltip } from '@/core/ui'
import Page from './index.vue'
import type { PortSnapshot } from './contracts'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  desktop: vi.fn(),
  reveal: vi.fn(),
  copy: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', async (original) => ({
  ...(await original<typeof import('@tauri-apps/api/core')>()),
  invoke: mocks.invoke,
}))
vi.mock('@/core/platform/window', () => ({ isDesktopRuntime: mocks.desktop }))
vi.mock('@tauri-apps/plugin-opener', () => ({ revealItemInDir: mocks.reveal }))
vi.mock('@/core/platform/clipboard', () => ({ writeClipboardText: mocks.copy }))
enableAutoUnmount(afterEach)
afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

const fixture = (): PortSnapshot => ({
  entries: [
    {
      protocol: 'TCP',
      family: 'IPv4',
      localAddress: '127.0.0.1',
      localPort: 8080,
      remoteAddress: null,
      remotePort: null,
      pid: 420,
      state: 'LISTEN',
      startedAt: '134029000000000000',
      processName: 'node.exe',
      executablePath: 'C:\\应用 程序\\node.exe',
      detailError: null,
    },
  ],
  warnings: [],
})
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (cause: unknown) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes
    reject = no
  })
  return { promise, resolve, reject }
}
function createPage() {
  return mount(Page, { global: { plugins: [createPinia()] } })
}
function button(page: ReturnType<typeof createPage>, label: string) {
  const match = page.findAllComponents(UiButton).find((item) => item.text() === label)
  if (!match) throw new Error(`缺少按钮：${label}`)
  return match.get('button')
}
function queryCount() {
  return mocks.invoke.mock.calls.filter(([command]) => command === 'port_viewer_query').length
}
async function autoRefresh(page: ReturnType<typeof createPage>, value: boolean) {
  page.getComponent(UiSwitch).vm.$emit('update:modelValue', value)
  await flushPromises()
}

beforeEach(() => {
  vi.resetAllMocks()
  resetToolVisibilityForTest()
  publishToolVisibility('port-viewer', { active: true })
  mocks.desktop.mockReturnValue(true)
  mocks.invoke.mockImplementation(async (command: string) =>
    command === 'port_viewer_supported' ? true : fixture()
  )
  mocks.copy.mockResolvedValue({ ok: true })
  mocks.reveal.mockResolvedValue(undefined)
})

it('通过真实 IPC 封装加载，单行路径保留完整提示，支持复制与定位', async () => {
  const page = createPage()
  await flushPromises()
  expect(mocks.invoke).toHaveBeenCalledWith('port_viewer_supported', {})
  expect(mocks.invoke).toHaveBeenCalledWith('port_viewer_query', {})
  expect(page.text()).toContain('127.0.0.1:8080')
  expect(page.text()).toContain('PID 420')
  const path = page
    .findAllComponents(UiTooltip)
    .find((item) => item.props('content') === 'C:\\应用 程序\\node.exe')
  expect(path?.get('p').classes()).toContain('truncate')
  expect(page.find('p[title]').exists()).toBe(false)
  await button(page, '复制信息').trigger('click')
  await flushPromises()
  expect(mocks.copy).toHaveBeenCalledWith(expect.stringContaining('TCP IPv4 127.0.0.1:8080'))
  await button(page, '打开所在目录').trigger('click')
  expect(mocks.reveal).toHaveBeenCalledWith('C:\\应用 程序\\node.exe')
})

it('浏览器和不支持的平台不读取端口', async () => {
  mocks.desktop.mockReturnValue(false)
  const browser = createPage()
  await flushPromises()
  expect(browser.text()).toContain('浏览器预览无法访问')
  expect(mocks.invoke).not.toHaveBeenCalled()
  browser.unmount()
  mocks.desktop.mockReturnValue(true)
  mocks.invoke.mockResolvedValueOnce(false)
  const unsupported = createPage()
  await flushPromises()
  expect(unsupported.text()).toContain('目前仅支持 Windows')
  expect(queryCount()).toBe(0)
})

it('平台检查失败可重试，空快照不会声称端口可绑定', async () => {
  mocks.invoke.mockRejectedValueOnce('服务不可用')
  const page = createPage()
  await flushPromises()
  expect(page.text()).toContain('读取平台支持状态失败')
  mocks.invoke.mockResolvedValueOnce(true).mockResolvedValueOnce({ entries: [], warnings: [] })
  await button(page, '重试').trigger('click')
  await flushPromises()
  expect(page.text()).toContain('未发现匹配的端口记录')
  expect(page.text()).toContain('不代表端口一定可以绑定')
})

it('部分协议和进程详情失败可见，保留端点但禁止不可验证的进程操作', async () => {
  const data = fixture()
  Object.assign(data.entries[0]!, {
    processName: null,
    executablePath: null,
    startedAt: null,
    detailError: '权限不足',
  })
  data.warnings.push('UDP IPv6：读取失败')
  mocks.invoke.mockResolvedValueOnce(true).mockResolvedValueOnce(data)
  const page = createPage()
  await flushPromises()
  expect(page.text()).toContain('UDP IPv6：读取失败')
  expect(page.text()).toContain('127.0.0.1:8080')
  expect(page.text()).toContain('权限不足')
  expect(button(page, '关闭进程').attributes('disabled')).toBeDefined()
  expect(button(page, '打开所在目录').attributes('disabled')).toBeDefined()
})

it('分页保持百条上限，筛选切换回第一页，全部连接展示远端', async () => {
  const row = fixture().entries[0]!
  const entries = Array.from({ length: 101 }, (_, index) => ({ ...row, localPort: 8000 + index }))
  entries.push({
    ...row,
    localPort: 9000,
    state: 'ESTABLISHED',
    remoteAddress: '10.0.0.1',
    remotePort: 443,
  })
  mocks.invoke.mockResolvedValueOnce(true).mockResolvedValueOnce({ entries, warnings: [] })
  const page = createPage()
  await flushPromises()
  expect(page.findAll('tbody tr')).toHaveLength(100)
  expect(page.text()).not.toContain('远端地址')
  page.getComponent(UiPagination).vm.$emit('update:modelValue', 2)
  await flushPromises()
  expect(page.findAll('tbody tr')).toHaveLength(1)
  await page.get('input').setValue('8000')
  expect(page.text()).toContain('127.0.0.1:8000')
  expect(page.findComponent(UiPagination).exists()).toBe(false)
  page.findAllComponents(UiSelect)[0]!.vm.$emit('update:modelValue', 'all')
  await page.get('input').setValue('9000')
  expect(page.text()).toContain('远端地址')
  expect(page.text()).toContain('10.0.0.1:443')
  await page.get('input').setValue('65536')
  expect(page.text()).toContain('请输入 1–65535')
  expect(page.findAll('tbody tr')).toHaveLength(0)
})

it('查询不重入，失败保留旧快照并禁用关闭，重试成功后恢复', async () => {
  const page = createPage()
  await flushPromises()
  const pending = deferred<PortSnapshot>()
  mocks.invoke.mockReturnValueOnce(pending.promise)
  await page.get('form').trigger('submit')
  await page.get('form').trigger('submit')
  expect(queryCount()).toBe(2)
  expect(button(page, '关闭进程').attributes('disabled')).toBeDefined()
  pending.reject('查询被拒绝')
  await flushPromises()
  expect(page.text()).toContain('下方保留上次快照')
  expect(page.text()).toContain('node.exe')
  expect(button(page, '关闭进程').attributes('disabled')).toBeDefined()
  await page.get('form').trigger('submit')
  await flushPromises()
  expect(button(page, '关闭进程').attributes('disabled')).toBeUndefined()
})

it('关闭必须确认，取消不发送命令，确认携带端点和启动时间并刷新', async () => {
  const page = createPage()
  await flushPromises()
  await button(page, '关闭进程').trigger('click')
  const dialog = page.getComponent(UiConfirmDialog)
  expect(dialog.props('message')).toContain('所有窗口、连接和任务')
  dialog.vm.$emit('close')
  await flushPromises()
  expect(mocks.invoke.mock.calls.some(([command]) => command === 'port_viewer_terminate')).toBe(
    false
  )
  await button(page, '关闭进程').trigger('click')
  const pending = deferred<void>()
  mocks.invoke.mockReturnValueOnce(pending.promise)
  dialog.vm.$emit('confirm')
  dialog.vm.$emit('confirm')
  await flushPromises()
  expect(
    mocks.invoke.mock.calls.filter(([command]) => command === 'port_viewer_terminate')
  ).toHaveLength(1)
  expect(mocks.invoke).toHaveBeenLastCalledWith('port_viewer_terminate', {
    endpoint: {
      protocol: 'TCP',
      family: 'IPv4',
      localAddress: '127.0.0.1',
      localPort: 8080,
      remoteAddress: null,
      remotePort: null,
      pid: 420,
    },
    startedAt: '134029000000000000',
  })
  pending.resolve()
  await flushPromises()
  expect(queryCount()).toBe(2)
  expect(page.text()).toContain('进程 node.exe 已关闭')
  expect(dialog.props('open')).toBe(false)
})

it('关闭复核失败保留确认目标并展示原因，不伪装成功', async () => {
  const page = createPage()
  await flushPromises()
  await button(page, '关闭进程').trigger('click')
  mocks.invoke.mockRejectedValueOnce('该进程已不再使用指定端口，请刷新查询')
  page.getComponent(UiConfirmDialog).vm.$emit('confirm')
  await flushPromises()
  expect(page.getComponent(UiConfirmDialog).props('error')).toContain('不再使用指定端口')
  expect(page.getComponent(UiConfirmDialog).props('open')).toBe(true)
  expect(page.text()).not.toContain('已关闭')
  expect(queryCount()).toBe(1)
})

it('自动刷新遇到切换、覆盖或隐藏暂停，返回立即补查，关闭确认暂停且卸载释放', async () => {
  vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
  const baseline = scopeStats()
  const page = createPage()
  await flushPromises()
  const refresh = button(page, '刷新')
  expect(refresh.classes()).toEqual(expect.arrayContaining(['w-24', 'shrink-0']))
  const pending = deferred<PortSnapshot>()
  mocks.invoke.mockReturnValueOnce(pending.promise)
  await autoRefresh(page, true)
  expect(queryCount()).toBe(2)
  expect(refresh.attributes('aria-busy')).toBe('true')
  expect(refresh.attributes('disabled')).toBeDefined()
  expect(refresh.classes()).toEqual(expect.arrayContaining(['w-24', 'shrink-0']))
  pending.resolve(fixture())
  await flushPromises()
  expect(refresh.attributes('aria-busy')).toBeUndefined()
  expect(refresh.classes()).toEqual(expect.arrayContaining(['w-24', 'shrink-0']))
  await vi.advanceTimersByTimeAsync(3000)
  expect(queryCount()).toBe(3)
  for (const field of ['active', 'covered', 'hidden'] as const) {
    publishToolVisibility('port-viewer', { [field]: field !== 'active' })
    await flushPromises()
    const count = queryCount()
    await vi.advanceTimersByTimeAsync(9000)
    expect(queryCount()).toBe(count)
    publishToolVisibility('port-viewer', { [field]: field === 'active' })
    await flushPromises()
    expect(queryCount()).toBe(count + 1)
  }
  await button(page, '关闭进程').trigger('click')
  const count = queryCount()
  await vi.advanceTimersByTimeAsync(6000)
  expect(queryCount()).toBe(count)
  page.getComponent(UiConfirmDialog).vm.$emit('close')
  await flushPromises()
  expect(queryCount()).toBe(count + 1)
  page.unmount()
  await flushPromises()
  await vi.advanceTimersByTimeAsync(9000)
  expect(queryCount()).toBe(count + 1)
  expect(scopeStats()).toMatchObject({
    live: baseline.live,
    timers: baseline.timers,
    listeners: baseline.listeners,
  })
})

it('自动查询失败后停止定时器，迟到关闭结果不会在卸载后发起查询', async () => {
  vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
  const page = createPage()
  await flushPromises()
  mocks.invoke.mockRejectedValueOnce('读取失败')
  await autoRefresh(page, true)
  expect(page.text()).toContain('自动刷新已暂停')
  const count = queryCount()
  await vi.advanceTimersByTimeAsync(6000)
  expect(queryCount()).toBe(count)
  expect(page.getComponent(UiSwitch).props('modelValue')).toBe(false)
  await page.get('form').trigger('submit')
  await flushPromises()
  await button(page, '关闭进程').trigger('click')
  const pending = deferred<void>()
  mocks.invoke.mockReturnValueOnce(pending.promise)
  page.getComponent(UiConfirmDialog).vm.$emit('confirm')
  await flushPromises()
  page.unmount()
  pending.resolve()
  await flushPromises()
  expect(queryCount()).toBe(count + 1)
})
