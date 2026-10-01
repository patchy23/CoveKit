/* eslint-disable vue/one-component-per-file, vue/require-default-prop */
import type { Channel } from '@tauri-apps/api/core'
import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import type { ConnConfig, DriverInstallProgress, DriverStatus } from './contracts'

const env = vi.hoisted(() => ({
  connectionIpc: {
    driverStatus: vi.fn(),
    installDriver: vi.fn(),
    cancelDriverInstall: vi.fn(),
    test: vi.fn(),
    save: vi.fn(),
  },
}))

vi.mock('./ipc', () => ({ connectionIpc: env.connectionIpc }))
vi.mock('./useDatabase', () => ({ withTimeout: (promise: Promise<unknown>) => promise }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }))
vi.mock('@/core/vault', () => ({ CredentialPicker: { template: '<div />' } }))
vi.mock('./ConnectionBasicsFields.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      emits: ['update:label', 'update:db-type'],
      setup(_props, { emit }) {
        return () =>
          h('div', [
            h(
              'button',
              { type: 'button', onClick: () => emit('update:db-type', 'oracle') },
              'Oracle'
            ),
            h(
              'button',
              { type: 'button', onClick: () => emit('update:db-type', 'postgresql') },
              '切换 PostgreSQL'
            ),
          ])
      },
    }),
  }
})
vi.mock('@/core/ui', async () => {
  const { defineComponent, h } = await import('vue')
  const UiButton = defineComponent({
    props: { disabled: Boolean },
    setup(props, { attrs, slots }) {
      return () =>
        h('button', { ...attrs, type: 'button', disabled: props.disabled }, slots.default?.())
    },
  })
  const UiInput = defineComponent({
    props: { modelValue: [String, Number], type: String, disabled: Boolean },
    emits: ['update:modelValue'],
    setup(props, { attrs, emit }) {
      return () =>
        h('input', {
          ...attrs,
          type: props.type ?? 'text',
          value: props.modelValue ?? '',
          disabled: props.disabled,
          onInput: (event: Event) =>
            emit('update:modelValue', (event.target as HTMLInputElement).value),
        })
    },
  })
  const UiCheckbox = defineComponent({
    props: { modelValue: Boolean, label: String },
    emits: ['update:modelValue'],
    setup(props, { emit }) {
      return () =>
        h('label', [
          h('input', {
            type: 'checkbox',
            checked: props.modelValue,
            onChange: (event: Event) =>
              emit('update:modelValue', (event.target as HTMLInputElement).checked),
          }),
          props.label,
        ])
    },
  })
  const UiModal = defineComponent({
    props: { open: Boolean },
    emits: ['close'],
    setup(props, { emit, slots }) {
      return () =>
        props.open
          ? h('div', { 'data-testid': 'modal' }, [
              h('button', { type: 'button', onClick: () => emit('close') }, '关闭对话框'),
              slots.default?.(),
              slots.footer?.(),
            ])
          : null
    },
  })
  const UiField = defineComponent({
    props: { label: String },
    setup(props, { slots }) {
      return () => h('label', [props.label, slots.default?.()])
    },
  })
  const UiPanel = defineComponent({
    setup(_props, { slots }) {
      return () => h('section', slots.default?.())
    },
  })
  const UiAlert = defineComponent({
    props: { title: String },
    setup(props, { slots }) {
      return () => h('div', [props.title, slots.default?.()])
    },
  })
  const UiSpinner = defineComponent({
    setup() {
      return () => h('span', '处理中')
    },
  })
  const UiSwitch = defineComponent({
    props: { modelValue: Boolean },
    emits: ['update:modelValue'],
    setup(props, { emit }) {
      return () =>
        h('input', {
          type: 'checkbox',
          checked: props.modelValue,
          onChange: (event: Event) =>
            emit('update:modelValue', (event.target as HTMLInputElement).checked),
        })
    },
  })
  return { UiAlert, UiCheckbox, UiButton, UiInput, UiModal, UiSpinner, UiSwitch, UiField, UiPanel }
})

const { default: ConnectionDialog } = await import('./ConnectionDialog.vue')

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined
  let reject: (error?: unknown) => void = () => undefined
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

function oracleConfig(): ConnConfig {
  return {
    id: 'oracle-config',
    label: 'Oracle 配置',
    dbType: 'oracle',
    host: 'oracle.local',
    port: 1521,
    username: 'app_user',
    database: 'APP',
    env: '开发',
    readonly: false,
    ssl: false,
    connectTimeoutMs: 12000,
  }
}

let wrapper: ReturnType<typeof mount> | undefined

function mountDialog() {
  wrapper = mount(ConnectionDialog, { props: { open: true, editing: oracleConfig() } })
  return wrapper
}

function button(text: string) {
  const match = wrapper?.findAll('button').find((candidate) => candidate.text().trim() === text)
  if (!match) throw new Error(`按钮不存在：${text}`)
  return match
}

function progressChannel() {
  return env.connectionIpc.installDriver.mock.calls.at(-1)?.[2] as
    Channel<DriverInstallProgress> | undefined
}

beforeEach(() => {
  let callbackId = 0
  vi.stubGlobal('__TAURI_INTERNALS__', {
    transformCallback: () => ++callbackId,
    unregisterCallback: vi.fn(),
  })
  env.connectionIpc.driverStatus.mockReset().mockResolvedValue({
    ready: false,
    kind: 'agent',
    autoInstall: true,
  })
  env.connectionIpc.installDriver.mockReset()
  env.connectionIpc.cancelDriverInstall.mockReset().mockResolvedValue(undefined)
  env.connectionIpc.test.mockReset().mockResolvedValue('19c')
  env.connectionIpc.save.mockReset().mockResolvedValue(undefined)
})

afterEach(() => {
  wrapper?.unmount()
  wrapper = undefined
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

describe('Oracle 连接对话框驱动准备', () => {
  it('安装成功后才测试，并把点击时的连接配置、密码与 clearPassword 快照传给测试', async () => {
    const installing = deferred<DriverStatus>()
    env.connectionIpc.installDriver.mockReturnValue(installing.promise)
    mountDialog()
    await flushPromises()
    await wrapper!.get('input[type="password"]').setValue('first-secret')
    await button('测试连接').trigger('click')
    await flushPromises()

    expect(env.connectionIpc.installDriver).toHaveBeenCalledTimes(1)
    expect(env.connectionIpc.test).not.toHaveBeenCalled()
    const channel = progressChannel()
    const requestId = env.connectionIpc.installDriver.mock.calls[0][1] as string
    channel?.onmessage({
      requestId,
      dbType: 'oracle',
      phase: 'downloading',
      downloadedBytes: 50,
      totalBytes: 100,
    })
    installing.resolve({ ready: true, kind: 'agent', version: '2.0' })
    await flushPromises()

    expect(env.connectionIpc.test).toHaveBeenCalledWith(
      expect.objectContaining({
        id: 'oracle-config',
        dbType: 'oracle',
        host: 'oracle.local',
        port: 1521,
        username: 'app_user',
        database: 'APP',
        connectTimeoutMs: 12000,
      }),
      'first-secret',
      false
    )
    expect(wrapper!.text()).toContain('连接成功')
  })

  it('下载后以测试开始时的 clearPassword 状态测试', async () => {
    const installing = deferred<DriverStatus>()
    env.connectionIpc.installDriver.mockReturnValue(installing.promise)
    mountDialog()
    await flushPromises()
    await wrapper!.get('input[type="checkbox"]').setValue(true)
    await flushPromises()
    await button('测试连接').trigger('click')
    await flushPromises()
    expect(env.connectionIpc.test).not.toHaveBeenCalled()

    installing.resolve({ ready: true, kind: 'agent' })
    await flushPromises()
    expect(env.connectionIpc.test).toHaveBeenCalledWith(
      expect.objectContaining({ id: 'oracle-config', dbType: 'oracle' }),
      '',
      true
    )
  })

  it.each([
    ['迟到就绪状态', { ready: true, kind: 'agent' }],
    ['迟到缺失状态', { ready: false, kind: 'agent', autoInstall: true }],
  ] as const)('卸载期间状态检查返回%s时不安装也不测试', async (_label, lateStatus) => {
    const checking = deferred<DriverStatus>()
    env.connectionIpc.driverStatus.mockReturnValueOnce(
      Promise.resolve({ ready: false, kind: 'agent', autoInstall: true })
    )
    mountDialog()
    await flushPromises()
    env.connectionIpc.driverStatus.mockReturnValueOnce(checking.promise)

    await button('测试连接').trigger('click')
    await flushPromises()
    expect(env.connectionIpc.driverStatus).toHaveBeenCalledTimes(2)

    wrapper!.unmount()
    wrapper = undefined
    checking.resolve(lateStatus)
    await flushPromises()

    expect(env.connectionIpc.installDriver).not.toHaveBeenCalled()
    expect(env.connectionIpc.test).not.toHaveBeenCalled()
  })

  it('新安装开始后旧安装取消失败仍发出操作错误', async () => {
    const oldInstall = deferred<DriverStatus>()
    const newInstall = deferred<DriverStatus>()
    const cancellation = deferred<void>()
    env.connectionIpc.installDriver.mockReturnValueOnce(oldInstall.promise)
    env.connectionIpc.installDriver.mockReturnValueOnce(newInstall.promise)
    env.connectionIpc.cancelDriverInstall.mockReturnValueOnce(cancellation.promise)
    mountDialog()
    await flushPromises()

    await button('测试连接').trigger('click')
    await flushPromises()
    const oldRequestId = env.connectionIpc.installDriver.mock.calls[0][1] as string
    progressChannel()!.onmessage({
      requestId: oldRequestId,
      dbType: 'oracle',
      phase: 'waiting',
      downloadedBytes: 0,
    })
    await wrapper!.get('input[placeholder="127.0.0.1"]').setValue('changed.host')
    await flushPromises()
    expect(env.connectionIpc.cancelDriverInstall).toHaveBeenCalledWith(oldRequestId)

    await button('下载').trigger('click')
    await flushPromises()
    expect(env.connectionIpc.installDriver).toHaveBeenCalledTimes(2)

    cancellation.reject(new Error('cancel transport failed'))
    await flushPromises()
    expect(wrapper!.emitted('operationError')).toEqual([
      ['Oracle 驱动操作失败：cancel transport failed'],
    ])
  })

  it.each(['修改表单', '修改 clearPassword', '切换数据库类型', '关闭弹窗', '卸载组件'] as const)(
    '%s 时取消准备且迟到的成功不会启动测试',
    async (action) => {
      const installing = deferred<DriverStatus>()
      env.connectionIpc.installDriver.mockReturnValue(installing.promise)
      mountDialog()
      await flushPromises()
      await button('测试连接').trigger('click')
      await flushPromises()
      const requestId = env.connectionIpc.installDriver.mock.calls[0][1] as string
      const channel = progressChannel()!
      expect(env.connectionIpc.test).not.toHaveBeenCalled()

      if (action === '修改表单')
        await wrapper!.get('input[placeholder="127.0.0.1"]').setValue('new.host')
      else if (action === '修改 clearPassword')
        await wrapper!.get('input[type="checkbox"]').setValue(true)
      else if (action === '切换数据库类型') await button('切换 PostgreSQL').trigger('click')
      else if (action === '关闭弹窗') await button('关闭对话框').trigger('click')
      else {
        wrapper!.unmount()
        wrapper = undefined
      }
      await flushPromises()

      channel.onmessage({
        requestId,
        dbType: 'oracle',
        phase: 'waiting',
        downloadedBytes: 0,
      })
      expect(env.connectionIpc.cancelDriverInstall).toHaveBeenCalledWith(requestId)
      installing.resolve({ ready: true, kind: 'agent' })
      await flushPromises()
      expect(env.connectionIpc.test).not.toHaveBeenCalled()
    }
  )
})
