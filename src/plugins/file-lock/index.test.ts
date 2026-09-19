import { enableAutoUnmount, flushPromises, mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { PhysicalPosition } from '@tauri-apps/api/dpi'
import type { Event } from '@tauri-apps/api/event'
import { publishToolVisibility, resetToolVisibilityForTest } from '@/core/lifecycle'
import { UiButton, UiConfirmDialog } from '@/core/ui'
import { useUiStore } from '@/stores/ui'
import Page from './index.vue'
import type { FileLockResult } from './contracts'

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  desktop: vi.fn(),
  listen: vi.fn(),
  unlisten: vi.fn(),
  open: vi.fn(),
  reveal: vi.fn(),
  copy: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@tauri-apps/api/core')>()),
  invoke: mocks.invoke,
}))
vi.mock('@/core/platform/window', () => ({ isDesktopRuntime: mocks.desktop }))
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ onDragDropEvent: mocks.listen }),
}))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: mocks.open }))
vi.mock('@tauri-apps/plugin-opener', () => ({ revealItemInDir: mocks.reveal }))
vi.mock('@/core/platform/clipboard', () => ({ writeClipboardText: mocks.copy }))
enableAutoUnmount(afterEach)
afterEach(() => vi.restoreAllMocks())

let onDrop: (event: Event<DragDropEvent>) => void
const target = 'C:\\测试 文件\\占用.txt'
const fixture = (): FileLockResult => ({
  path: target,
  processes: [
    {
      pid: 420,
      startedAt: '134029000000000000',
      appName: '测试应用',
      serviceName: null,
      processName: 'editor.exe',
      executablePath: 'C:\\应用 程序\\editor.exe',
      detailError: null,
    },
  ],
})
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason: unknown) => void
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
async function search(page: ReturnType<typeof createPage>, path = target) {
  await page.get('input').setValue(path)
  await page.get('form').trigger('submit')
  await flushPromises()
}
function drop(paths: string[], x = 100, y = 100) {
  onDrop({
    event: 'tauri://drag-drop',
    id: 1,
    payload: { type: 'drop', paths, position: new PhysicalPosition(x, y) },
  })
}

beforeEach(() => {
  vi.resetAllMocks()
  vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible')
  resetToolVisibilityForTest()
  publishToolVisibility('file-lock', { active: true })
  mocks.desktop.mockReturnValue(true)
  mocks.invoke.mockImplementation(async (command: string) =>
    command === 'file_lock_supported' ? true : fixture()
  )
  mocks.listen.mockImplementation(async (handler) => {
    onDrop = handler
    return mocks.unlisten
  })
  mocks.open.mockResolvedValue(null)
  mocks.copy.mockResolvedValue({ ok: true })
  mocks.reveal.mockResolvedValue(undefined)
})

it('真实 IPC 封装传递中文空格路径，展示进程并支持复制和定位', async () => {
  const page = createPage()
  await flushPromises()
  await search(page, `"${target}"`)
  expect(mocks.invoke).toHaveBeenCalledWith('file_lock_supported', {})
  expect(mocks.invoke).toHaveBeenCalledWith('file_lock_query', { path: target })
  expect(page.text()).toContain('editor.exe')
  expect(page.text()).toContain('PID 420')
  expect(page.text()).toContain('测试应用')
  await button(page, '复制信息').trigger('click')
  await flushPromises()
  expect(mocks.copy).toHaveBeenCalledWith(expect.stringContaining(`查询文件：${target}`))
  expect(mocks.copy).toHaveBeenCalledWith(expect.stringContaining('PID：420'))
  await button(page, '打开所在目录').trigger('click')
  expect(mocks.reveal).toHaveBeenCalledWith('C:\\应用 程序\\editor.exe')
})

it('刷新无结果与查询失败不同，失败不保留旧快照', async () => {
  const page = createPage()
  await flushPromises()
  await search(page)
  mocks.invoke.mockResolvedValueOnce({ path: target, processes: [] })
  await page.get('form').trigger('submit')
  await flushPromises()
  expect(page.text()).toContain('未发现占用进程')
  expect(page.text()).toContain('不保证文件没有占用')
  mocks.invoke.mockRejectedValueOnce('访问被拒绝')
  await page.get('form').trigger('submit')
  await flushPromises()
  expect(page.get('[role="alert"]').text()).toContain('访问被拒绝')
  expect(page.text()).not.toContain('未发现占用进程')
  expect(page.text()).not.toContain('使用该文件的进程 ·')
})

it('详情权限不足仍保留 PID 与服务，禁用目录动作', async () => {
  const partial = fixture()
  Object.assign(partial.processes[0]!, {
    processName: null,
    executablePath: null,
    serviceName: 'ExampleService',
    detailError: '权限不足，无法读取程序路径',
  })
  const page = createPage()
  await flushPromises()
  mocks.invoke.mockResolvedValueOnce(partial)
  await search(page)
  expect(page.text()).toContain('PID 420')
  expect(page.text()).toContain('服务：ExampleService')
  expect(page.text()).toContain('权限不足')
  expect(button(page, '打开所在目录').attributes('disabled')).toBeDefined()
})

it('查询期间重复提交不创建请求，更换路径丢弃迟到结果', async () => {
  const page = createPage()
  await flushPromises()
  const pending = deferred<FileLockResult>()
  mocks.invoke.mockReturnValueOnce(pending.promise)
  await search(page)
  await page.get('form').trigger('submit')
  expect(mocks.invoke.mock.calls.filter(([name]) => name === 'file_lock_query')).toHaveLength(1)
  await page.get('input').setValue('C:\\other.txt')
  pending.resolve(fixture())
  await flushPromises()
  expect(page.text()).not.toContain('editor.exe')
  expect(page.text()).not.toContain('未发现占用进程')
  await search(page, 'C:\\other.txt')
  expect(mocks.invoke).toHaveBeenLastCalledWith('file_lock_query', { path: 'C:\\other.txt' })
})

it('取消文件选择不查询，选择文件后自动查询，选择失败可见', async () => {
  const page = createPage()
  await flushPromises()
  await button(page, '选择文件').trigger('click')
  await flushPromises()
  expect(mocks.invoke).toHaveBeenCalledTimes(1)
  mocks.open.mockResolvedValueOnce(target)
  await button(page, '选择文件').trigger('click')
  await flushPromises()
  expect(mocks.invoke).toHaveBeenLastCalledWith('file_lock_query', { path: target })
  mocks.open.mockRejectedValueOnce(new Error('对话框失败'))
  await button(page, '选择文件').trigger('click')
  await flushPromises()
  expect(page.get('[role="alert"]').text()).toContain('选择文件失败')
})

it('非 Windows 与浏览器预览均明确禁用，不创建拖放订阅', async () => {
  mocks.invoke.mockResolvedValueOnce(false)
  const page = createPage()
  await flushPromises()
  expect(page.text()).toContain('目前仅支持 Windows')
  expect(button(page, '选择文件').attributes('disabled')).toBeDefined()
  expect(mocks.listen).not.toHaveBeenCalled()
  page.unmount()
  mocks.desktop.mockReturnValue(false)
  mocks.invoke.mockClear()
  const preview = createPage()
  await flushPromises()
  expect(preview.text()).toContain('请在 CoveKit 桌面应用中使用')
  expect(mocks.invoke).not.toHaveBeenCalled()
})

it('平台检测失败允许重试；拖放注册失败不阻断手动查询', async () => {
  mocks.invoke.mockRejectedValueOnce('检测失败')
  const page = createPage()
  await flushPromises()
  expect(page.text()).toContain('读取平台支持状态失败')
  mocks.listen.mockRejectedValueOnce('订阅失败')
  await button(page, '重试').trigger('click')
  await flushPromises()
  expect(page.text()).toContain('文件拖放不可用')
  await search(page)
  expect(page.text()).toContain('editor.exe')
})

it('仅可见页签的内容区域接收单文件，拒绝多文件拖放', async () => {
  const page = createPage()
  await flushPromises()
  vi.spyOn(page.element, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    top: 0,
    right: 500,
    bottom: 500,
    width: 500,
    height: 500,
    x: 0,
    y: 0,
    toJSON() {},
  })
  drop([target], 10000, 10000)
  publishToolVisibility('file-lock', { active: false })
  drop([target])
  publishToolVisibility('file-lock', { active: true, covered: true })
  drop([target])
  publishToolVisibility('file-lock', { covered: false, hidden: true })
  vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden')
  drop([target])
  expect(mocks.invoke).toHaveBeenCalledTimes(1)
  publishToolVisibility('file-lock', { hidden: false })
  vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible')
  drop([target, 'C:\\other.txt'])
  await flushPromises()
  expect(page.text()).toContain('请一次拖入一个文件')
  expect(mocks.invoke).toHaveBeenCalledTimes(1)
  drop([target])
  await flushPromises()
  expect(mocks.invoke).toHaveBeenLastCalledWith('file_lock_query', { path: target })
  page.unmount()
  await flushPromises()
  expect(mocks.unlisten).toHaveBeenCalledTimes(1)
})

it('从资源管理器拖入时窗口失焦仍接收文件，不把失焦当作页面隐藏', async () => {
  const page = createPage()
  await flushPromises()
  vi.spyOn(page.element, 'getBoundingClientRect').mockReturnValue({
    left: 0,
    top: 0,
    right: 500,
    bottom: 500,
    width: 500,
    height: 500,
    x: 0,
    y: 0,
    toJSON() {},
  })
  // App.vue 的 onFocusChanged 会把失焦发布为 hidden=true，拖入时焦点可仍在资源管理器。
  publishToolVisibility('file-lock', { active: true, covered: false, hidden: true })
  onDrop({
    event: 'tauri://drag-enter',
    id: 1,
    payload: {
      type: 'enter',
      paths: [target],
      position: new PhysicalPosition(100, 100),
    },
  })
  await flushPromises()
  expect(page.text()).toContain('松开以查询这个文件')
  drop([target])
  await flushPromises()
  expect(mocks.invoke).toHaveBeenLastCalledWith('file_lock_query', { path: target })
  expect((page.get('input').element as HTMLInputElement).value).toBe(target)
  expect(page.text()).toContain('PID 420')
  expect(page.text()).not.toContain('松开以查询这个文件')
})

it('卸载后才完成的订阅立即解绑；迟到文件选择不查询', async () => {
  const pending = deferred<() => void>()
  mocks.listen.mockReturnValueOnce(pending.promise)
  const page = createPage()
  await flushPromises()
  const selected = deferred<string>()
  mocks.open.mockReturnValueOnce(selected.promise)
  await button(page, '选择文件').trigger('click')
  page.unmount()
  pending.resolve(mocks.unlisten)
  selected.resolve(target)
  await flushPromises()
  expect(mocks.unlisten).toHaveBeenCalledTimes(1)
  expect(mocks.invoke).toHaveBeenCalledTimes(1)
})

it('打开目录失败给出操作反馈', async () => {
  const page = createPage()
  await flushPromises()
  await search(page)
  const toast = vi.spyOn(useUiStore(), 'toast')
  mocks.reveal.mockRejectedValueOnce('文件已移走')
  await button(page, '打开所在目录').trigger('click')
  await flushPromises()
  expect(toast).toHaveBeenCalledWith(expect.stringContaining('打开程序所在目录失败'))
})

it('关闭前展示目标和丢失提示，取消不发送命令', async () => {
  const page = createPage()
  await flushPromises()
  await search(page)
  expect(page.text()).not.toContain('进程信息已读取')
  expect(page.text()).not.toContain(target)
  expect((page.get('input').element as HTMLInputElement).value).toBe(target)
  await button(page, '关闭进程').trigger('click')
  await flushPromises()
  const dialog = page.getComponent(UiConfirmDialog)
  expect(dialog.props('open')).toBe(true)
  expect(dialog.props('message')).toContain('editor.exe（PID 420）')
  expect(dialog.props('message')).toContain('未保存的内容可能丢失')
  await button(page, '取消').trigger('click')
  expect(dialog.props('open')).toBe(false)
  expect(mocks.invoke.mock.calls.some(([name]) => name === 'file_lock_terminate')).toBe(false)
})

it('关闭传递查询文件与精确进程身份，防止重复执行，成功后刷新', async () => {
  const page = createPage()
  await flushPromises()
  await search(page)
  await button(page, '关闭进程').trigger('click')
  await flushPromises()
  const pending = deferred<void>()
  mocks.invoke.mockImplementation((command: string) =>
    command === 'file_lock_terminate'
      ? pending.promise
      : Promise.resolve({ path: target, processes: [] })
  )
  const dialog = page.getComponent(UiConfirmDialog)
  await button(page, '确认关闭').trigger('click')
  dialog.vm.$emit('confirm')
  dialog.vm.$emit('close')
  await flushPromises()
  expect(dialog.props('open')).toBe(true)
  expect(dialog.props('loading')).toBe(true)
  expect(mocks.invoke.mock.calls.filter(([name]) => name === 'file_lock_terminate')).toHaveLength(1)
  expect(mocks.invoke).toHaveBeenLastCalledWith('file_lock_terminate', {
    path: target,
    pid: 420,
    startedAt: '134029000000000000',
  })
  pending.resolve()
  await flushPromises()
  expect(dialog.props('open')).toBe(false)
  expect(page.text()).toContain('进程 editor.exe 已关闭')
  expect(page.text()).toContain('未发现占用进程')
  expect(mocks.invoke).toHaveBeenLastCalledWith('file_lock_query', { path: target })
})

it('关闭失败保留结果与弹窗错误，不伪装成功；卸载后的成功不再刷新', async () => {
  const page = createPage()
  await flushPromises()
  await search(page)
  await button(page, '关闭进程').trigger('click')
  await flushPromises()
  mocks.invoke.mockRejectedValueOnce('权限不足，拒绝关闭')
  await button(page, '确认关闭').trigger('click')
  await flushPromises()
  const dialog = page.getComponent(UiConfirmDialog)
  expect(dialog.props('error')).toContain('权限不足')
  expect(dialog.props('open')).toBe(true)
  expect(page.text()).not.toContain('已关闭')
  expect(page.text()).toContain('PID 420')
  const pending = deferred<void>()
  mocks.invoke.mockReturnValueOnce(pending.promise)
  await button(page, '确认关闭').trigger('click')
  page.unmount()
  const calls = mocks.invoke.mock.calls.length
  pending.resolve()
  await flushPromises()
  expect(mocks.invoke).toHaveBeenCalledTimes(calls)
})
