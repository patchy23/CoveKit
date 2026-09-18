/**
 * SSH 根工作区行为网（AR04 · P3a）
 *
 * 用途：在拆分状态所有权之前，把 `useSshWorkspace`（当前活跃实现）的**用户可感知行为**固化成可跑用例；
 * P3b/P3c 拆分的每一步都必须复用同一组断言，避免用过时实现（拆分前零消费方的
 * `profiles/useSshProfiles.ts` 与 `workspace/useSshIdleWatch.ts`，已于 P3b/P3c 删除）的行为覆盖活跃实现里的较新修复。
 *
 * 覆盖（对应决策书 §2.2 必需场景）：
 * 2 → 保存表单生命周期、凭证前置校验、profiles/groups 单一状态源
 * 3 → 删除服务器连带关闭工作区、失败不伪装成功
 * 4 → 并发连接归属、占位绑定、取消/卸载后成功结果不重开页签
 * 5 → 主机密钥队列并发确认、空队列、卸载统一取消
 * 6 → 卸载退订一次、卸载先于订阅完成时晚到句柄仍被调用
 * 7 → 旧会话断开不覆盖重连态、退避重连、关闭自动重连、手动关闭后计时器不复活、句柄回收后整条重建
 * 8 → 空闲断开、终端输出视为活跃、卸载清理连接与定时器
 *
 * 手法（决策书 §2.1）：真实挂载最小宿主组件触发 onMounted/onUnmounted；mock 本插件 IPC 门面
 * （命令可编程返回、事件可手动触发、退订句柄可检查、订阅可挂起制造竞态）；Pinia 每例独立实例，
 * localStorage 每例清空；断言用户可感知状态与资源责任，不固定无关内部调用顺序。
 */
import { mount } from '@vue/test-utils'
import { createPinia, setActivePinia, type Pinia } from 'pinia'
import { defineComponent, h, nextTick } from 'vue'
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest'
import { useSettingsStore } from '@/stores/settings'
import { useUiStore } from '@/stores/ui'
import type {
  ConnectStage,
  ConnectionStatus,
  FileTransferProgress,
  HostKeyVerifyRequest,
  ServerConnection,
  ServerProfile,
  SshConnectOutcome,
  SshGroup,
  TerminalClosed,
  TerminalData,
} from './contracts'
import { useSshWorkspace } from './useSshWorkspace'

/* ── IPC 门面假实现（vi.hoisted：mock 工厂先于被 mock 模块的导入执行） ── */

const env = vi.hoisted(() => {
  type Handler = (payload: unknown) => void

  /** 一路事件订阅：settle 前不登记处理器，用于制造「卸载先于订阅完成」的竞态 */
  interface Subscription {
    /** 事件名（与 ipc.ts 的订阅函数一一对应） */
    name: string
    /** 后端推送时的处理器 */
    handler: Handler
    /** 退订句柄（spy：断言恰好释放一次） */
    unsubscribe: Mock<() => void>
    /** 让订阅 Promise 完成并把处理器登记进分发表 */
    settle: () => void
  }

  const subscriptions: Subscription[] = []
  /** 事件名 → 已登记处理器 */
  const handlers = new Map<string, Set<Handler>>()
  /** true = 订阅 Promise 保持挂起，等 releasePending() 才完成 */
  let hold = false

  function subscribe(name: string) {
    return (handler: Handler): Promise<() => void> => {
      let release: (stop: () => void) => void = () => undefined
      const pending = new Promise<() => void>((resolve) => {
        release = resolve
      })
      let settled = false
      const entry: Subscription = {
        name,
        handler,
        unsubscribe: vi.fn<() => void>(() => {
          handlers.get(name)?.delete(handler)
        }),
        settle: () => {
          if (settled) return
          settled = true
          const registered = handlers.get(name) ?? new Set<Handler>()
          handlers.set(name, registered)
          registered.add(handler)
          release(entry.unsubscribe)
        },
      }
      subscriptions.push(entry)
      if (!hold) entry.settle()
      return pending
    }
  }

  /** 模拟后端推送（同步调用已登记处理器） */
  function emit(name: string, payload: unknown): void {
    for (const handler of [...(handlers.get(name) ?? [])]) handler(payload)
  }

  /** 放行全部挂起的订阅 */
  function releasePending(): void {
    for (const sub of subscriptions) sub.settle()
  }

  const commands = {
    sshProfileList: vi.fn(),
    sshProfileSave: vi.fn(),
    sshProfileDelete: vi.fn(),
    sshConnect: vi.fn(),
    sshDisconnect: vi.fn(),
    sshReconnect: vi.fn(),
    sshConnections: vi.fn(),
    sshHostKeyRespond: vi.fn(),
    sshGroupList: vi.fn(),
    sshGroupSave: vi.fn(),
    sshGroupDelete: vi.fn(),
  }

  return {
    subscriptions,
    commands,
    subscribe,
    emit,
    releasePending,
    setHold(value: boolean) {
      hold = value
    },
    reset() {
      subscriptions.length = 0
      handlers.clear()
      hold = false
    },
  }
})

vi.mock('./ipc', () => ({
  ipc: env.commands,
  onConnectionStatus: env.subscribe('connectionStatus'),
  onConnectStage: env.subscribe('connectStage'),
  onHostKeyVerify: env.subscribe('hostKeyVerify'),
  onTerminalData: env.subscribe('terminalData'),
  onTerminalClosed: env.subscribe('terminalClosed'),
  onTransferProgress: env.subscribe('transferProgress'),
}))

/** 框架设置读写走假实现，避免设置持久化失败降级到 storage 干扰断言 */
vi.mock('@/core/ipc/ipc', () => ({
  IpcError: class IpcError extends Error {},
  invokeCommand: vi.fn(async () => undefined),
  ipc: {
    settingsGet: vi.fn(async () => ({})),
    settingsSet: vi.fn(async () => undefined),
  },
}))

/* ── 夹具 ── */

function profile(id: string, name: string, groupId?: string): ServerProfile {
  return {
    id,
    name,
    host: '10.0.0.1',
    port: 22,
    username: 'root',
    authMethod: 'password',
    credentialRef: `cred-${id}`,
    groupId,
  }
}

function connection(
  profileId: string,
  sessionId: string,
  status: ConnectionStatus = 'connected'
): ServerConnection {
  return { profileId, sessionId, status }
}

function connectOk(requestId: string, profileId: string, sessionId: string): SshConnectOutcome {
  return { ok: true, requestId, connection: connection(profileId, sessionId) }
}

function group(id: string, name: string): SshGroup {
  return { id, name, sortOrder: 1 }
}

function hostKeyRequest(requestId: string): HostKeyVerifyRequest {
  return {
    requestId,
    kind: 'unknown',
    host: '10.0.0.1',
    port: 22,
    algorithm: 'ssh-ed25519',
    fingerprint: 'SHA256:probe',
    savedFingerprints: [],
  }
}

function stage(requestId: string, s: string): ConnectStage {
  return { requestId, profileId: 'profile-a', stage: s, status: 'start' }
}

/** 类型化的事件推送入口（事件名与 ipc.ts 的订阅一一对应） */
const push = {
  connectionStatus: (payload: ServerConnection) => env.emit('connectionStatus', payload),
  connectStage: (payload: ConnectStage) => env.emit('connectStage', payload),
  hostKeyVerify: (payload: HostKeyVerifyRequest) => env.emit('hostKeyVerify', payload),
  terminalData: (payload: TerminalData) => env.emit('terminalData', payload),
  terminalClosed: (payload: TerminalClosed) => env.emit('terminalClosed', payload),
  transferProgress: (payload: FileTransferProgress) => env.emit('transferProgress', payload),
}

/** 可手动完成的 Promise（制造乱序完成与取消竞态） */
function deferred<T>() {
  let resolve: (value: T) => void = () => undefined
  const promise = new Promise<T>((res) => {
    resolve = res
  })
  return { promise, resolve }
}

/* ── 宿主挂载与时间控制 ── */

type WorkspaceApi = ReturnType<typeof useSshWorkspace>

/** 假计时器是否启用（决定 settle 如何推进异步链） */
let fakeTimers = false
let pinia: Pinia
const mountedHosts: Array<() => void> = []

/** 启用假计时器（只接管计时器与 Date，微任务保持真实） */
function useFakeTimers(): void {
  fakeTimers = true
  vi.useFakeTimers({
    toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval', 'Date'],
  })
}

/** 推进 onMounted 的多段 await 链 */
async function settle(rounds = 10): Promise<void> {
  for (let index = 0; index < rounds; index += 1) {
    await nextTick()
    if (fakeTimers) await vi.advanceTimersByTimeAsync(0)
    else await new Promise((resolve) => setTimeout(resolve, 0))
  }
}

/** 推进假时钟并冲刷随之产生的异步回调 */
async function advance(ms: number): Promise<void> {
  await vi.advanceTimersByTimeAsync(ms)
  await settle(4)
}

/**
 * 挂载最小宿主组件：composable 必须在真实的挂载/卸载钩子里执行，
 * 才能覆盖 onMounted 的订阅与 onUnmounted 的清理责任。
 */
function mountWorkspace(): { api: WorkspaceApi; unmount: () => void } {
  let api: WorkspaceApi | undefined
  const Host = defineComponent({
    setup() {
      api = useSshWorkspace()
      return () => h('div')
    },
  })
  const wrapper = mount(Host, { global: { plugins: [pinia] } })
  if (!api) throw new Error('宿主组件未初始化 SSH 工作区')
  let alive = true
  const unmount = () => {
    if (!alive) return
    alive = false
    wrapper.unmount()
  }
  mountedHosts.push(unmount)
  return { api, unmount }
}

function toastText(): string {
  return useUiStore().toastMessage
}

/** 每例复位：命令假实现回到默认行为（连接成功、列表为空、删除成功） */
function resetIpc(): void {
  vi.clearAllMocks()
  env.reset()
  const c = env.commands
  c.sshProfileList.mockResolvedValue([])
  c.sshProfileSave.mockImplementation(async (arg: { profile: ServerProfile }) => arg.profile)
  c.sshProfileDelete.mockResolvedValue({ ok: true })
  c.sshConnect.mockImplementation(async (arg: { profileId: string }) =>
    connectOk(`req-${arg.profileId}`, arg.profileId, `conn-${arg.profileId}`)
  )
  c.sshDisconnect.mockResolvedValue({ ok: true })
  c.sshReconnect.mockResolvedValue({
    ok: false,
    requestId: 'req-reconnect',
    error: { code: 'SESSION_NOT_FOUND', message: '会话不存在' },
  })
  c.sshConnections.mockResolvedValue([])
  c.sshHostKeyRespond.mockResolvedValue({ ok: true })
  c.sshGroupList.mockResolvedValue([])
  c.sshGroupSave.mockResolvedValue({ ok: true })
  c.sshGroupDelete.mockResolvedValue({ ok: true })
}

beforeEach(() => {
  localStorage.clear()
  pinia = createPinia()
  setActivePinia(pinia)
  resetIpc()
  // 失败分支会打印错误；用例只断言用户可见提示，不污染测试输出
  vi.spyOn(console, 'error').mockImplementation(() => undefined)
})

afterEach(() => {
  for (const unmount of mountedHosts.splice(0)) unmount()
  vi.useRealTimers()
  fakeTimers = false
  vi.restoreAllMocks()
  localStorage.clear()
})

/* ── 2. 服务器配置、凭证前置校验与单一状态源 ── */

describe('SSH 工作区 · 服务器配置与分组', () => {
  it('保存成功：关闭表单并入列', async () => {
    const { api } = mountWorkspace()
    await settle()
    api.openAddForm()
    expect(api.formOpen.value).toBe(true)

    await api.saveProfile(profile('profile-a', '生产服务器'), { password: 'secret' }, true)

    expect(api.formOpen.value).toBe(false)
    expect(api.profiles.value.map((item) => item.id)).toEqual(['profile-a'])
    expect(toastText()).toContain('已添加服务器')
  })

  it('保存失败：提示可见、表单与编辑数据保留、列表不变', async () => {
    const existing = profile('profile-a', '生产服务器')
    env.commands.sshProfileList.mockResolvedValue([existing])
    env.commands.sshProfileSave.mockRejectedValue(new Error('磁盘只读'))
    const { api } = mountWorkspace()
    await settle()
    api.openEditForm(existing)

    await api.saveProfile({ ...existing, name: '改名后' }, {}, false)

    expect(api.formOpen.value).toBe(true)
    expect(api.editingProfile.value?.id).toBe('profile-a')
    expect(api.profiles.value.map((item) => item.name)).toEqual(['生产服务器'])
    expect(toastText()).toContain('保存失败')
  })

  it('手工密码不入凭证库也可连接，重连继续传递内存认证信息', async () => {
    const { api } = mountWorkspace()
    await settle()
    const manual = { ...profile('manual', '手工服务器'), credentialRef: undefined }
    const credentials = { password: ' fixture password ' }
    await api.saveProfile(manual, credentials, false)
    expect(env.commands.sshProfileSave).toHaveBeenCalledWith(
      expect.objectContaining({ saveCredential: false })
    )
    const connected = await api.openConnection(manual.id)
    expect(env.commands.sshConnect).toHaveBeenCalledWith({
      profileId: manual.id,
      overrides: credentials,
    })
    expect(api.credentialRequestProfile.value).toBeNull()
    connected!.connection.status = 'disconnected'
    await api.reconnectWorkspace(connected!.id)
    expect(env.commands.sshReconnect).toHaveBeenCalledWith('conn-manual', credentials)
    expect(api.profiles.value[0]).not.toHaveProperty('password')
    expect(api.profiles.value[0].credentialRef).toBeUndefined()
  })

  it('无凭证服务器先请求输入，取消不连接，重新打开工具后不复用密码', async () => {
    const manual = { ...profile('manual', '手工服务器'), credentialRef: undefined }
    env.commands.sshProfileList.mockResolvedValue([manual])
    const first = mountWorkspace()
    await settle()
    const canceled = first.api.openConnection(manual.id)
    expect(first.api.credentialRequestProfile.value?.id).toBe(manual.id)
    first.api.respondCredentials()
    expect(await canceled).toBeUndefined()
    expect(env.commands.sshConnect).not.toHaveBeenCalled()
    const connecting = first.api.openConnection(manual.id)
    first.api.respondCredentials({ password: 'fixture' })
    await connecting
    first.unmount()
    const second = mountWorkspace()
    await settle()
    const pending = second.api.openConnection(manual.id)
    expect(second.api.credentialRequestProfile.value?.id).toBe(manual.id)
    second.unmount()
    expect(await pending).toBeUndefined()
  })

  it('本地保存后连接不弹输入框，认证失败可重输并用于重连', async () => {
    const manual = {
      ...profile('local', '本地服务器'),
      credentialRef: undefined,
      hasLocalAuth: true,
    }
    env.commands.sshProfileList.mockResolvedValue([manual])
    env.commands.sshConnect.mockResolvedValueOnce({
      ok: false,
      requestId: 'failed',
      error: { code: 'AUTH_FAILED', message: '认证失败' },
    })
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection(manual.id)
    expect(api.credentialRequestProfile.value).toBeNull()
    expect(env.commands.sshConnect).toHaveBeenCalledWith({
      profileId: manual.id,
      overrides: undefined,
    })
    const second = api.openConnection(manual.id)
    expect(api.credentialRequestProfile.value?.id).toBe(manual.id)
    api.respondCredentials({ password: 'corrected-fixture' })
    const connected = await second
    connected!.connection.status = 'disconnected'
    await api.reconnectWorkspace(connected!.id)
    expect(env.commands.sshReconnect).toHaveBeenCalledWith('conn-local', {
      password: 'corrected-fixture',
    })
  })

  it('本地保存将选项传给后端，编辑时留空保留已有认证', async () => {
    const manual = {
      ...profile('local', '本地服务器'),
      credentialRef: undefined,
      hasLocalAuth: true,
    }
    env.commands.sshProfileList.mockResolvedValue([manual])
    const { api } = mountWorkspace()
    await settle()
    await api.saveProfile(manual, {}, false, true)
    expect(env.commands.sshProfileSave).toHaveBeenCalledWith({
      profile: manual,
      saveCredential: false,
      saveLocal: true,
    })
    await api.openConnection(manual.id)
    expect(api.credentialRequestProfile.value).toBeNull()
    await api.saveProfile(manual, {}, true, false)
    expect(env.commands.sshProfileSave).toHaveBeenCalledTimes(1)
  })

  it('手工认证失败后再次连接允许重新输入', async () => {
    const manual = { ...profile('manual', '手工服务器'), credentialRef: undefined }
    env.commands.sshProfileList.mockResolvedValue([manual])
    env.commands.sshConnect.mockResolvedValue({
      ok: false,
      requestId: 'failed',
      error: { code: 'AUTH_FAILED', message: '认证失败' },
    })
    const { api } = mountWorkspace()
    await settle()
    const first = api.openConnection(manual.id)
    api.respondCredentials({ password: 'wrong-fixture' })
    await first
    const second = api.openConnection(manual.id)
    expect(api.credentialRequestProfile.value?.id).toBe(manual.id)
    api.respondCredentials()
    await second
  })

  it('新增服务器未提供完整凭证：拒绝提交且不发 IPC', async () => {
    const { api } = mountWorkspace()
    await settle()
    api.openAddForm()

    await api.saveProfile(
      { ...profile('profile-new', '新服务器'), credentialRef: undefined },
      {},
      false
    )

    expect(env.commands.sshProfileSave).not.toHaveBeenCalled()
    expect(toastText()).toContain('必须填写完整凭证')
    expect(api.profiles.value).toHaveLength(0)
  })

  it('删除服务器：连带关闭其工作区并断开会话，不误关其他服务器', async () => {
    env.commands.sshProfileList.mockResolvedValue([
      profile('profile-a', '生产服务器'),
      profile('profile-b', '测试服务器'),
    ])
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')
    await api.openConnection('profile-b')
    expect(api.connectionWorkspaces.value).toHaveLength(2)

    api.requestDelete(api.profiles.value[0])
    await api.confirmDelete()

    expect(api.connectionWorkspaces.value.map((item) => item.profileId)).toEqual(['profile-b'])
    expect(api.connectionWorkspaces.value[0].connection.sessionId).toBe('conn-profile-b')
    expect(env.commands.sshDisconnect.mock.calls.map((call) => call[0])).toEqual(['conn-profile-a'])
    expect(api.profiles.value.map((item) => item.id)).toEqual(['profile-b'])
    expect(api.activeProfileId.value).toBe('profile-b')
    expect(toastText()).toContain('已删除服务器')
  })

  it('删除失败：提示可见，配置仍在，不伪装成功', async () => {
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    env.commands.sshProfileDelete.mockRejectedValue(new Error('数据库被占用'))
    const { api } = mountWorkspace()
    await settle()

    api.requestDelete(api.profiles.value[0])
    await api.confirmDelete()

    expect(api.profiles.value.map((item) => item.id)).toEqual(['profile-a'])
    expect(toastText()).toContain('删除服务器失败')
  })

  it('删除必须经确认：请求删除只记录目标，确认后才落库', async () => {
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()

    api.requestDelete(api.profiles.value[0])
    expect(api.deleteTarget.value?.id).toBe('profile-a')
    expect(env.commands.sshProfileDelete).not.toHaveBeenCalled()

    await api.confirmDelete()
    expect(api.deleteTarget.value).toBeNull()
    expect(env.commands.sshProfileDelete).toHaveBeenCalledWith('profile-a')
    expect(api.profiles.value).toHaveLength(0)
  })

  it('分组只有一份状态：删除分组后组内连接回到未分组', async () => {
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器', 'group-1')])
    env.commands.sshGroupList.mockResolvedValue([group('group-1', '生产')])
    const { api } = mountWorkspace()
    await settle()

    expect(api.groups.value.map((item) => item.name)).toEqual(['生产'])
    await api.deleteGroup('group-1')

    expect(env.commands.sshGroupDelete).toHaveBeenCalledWith('group-1')
    expect(api.groups.value).toHaveLength(0)
    expect(api.profiles.value[0].groupId).toBeUndefined()
    expect(toastText()).toContain('已删除分组')
  })

  it('新建分组只写后端一次，列表随之更新', async () => {
    const { api } = mountWorkspace()
    await settle()

    await api.createGroup('测试组')

    expect(env.commands.sshGroupSave).toHaveBeenCalledTimes(1)
    expect(api.groups.value.map((item) => item.name)).toEqual(['测试组'])
    expect(toastText()).toContain('已创建分组')
  })

  it('服务器列表以后端为唯一来源，关键词过滤生效', async () => {
    env.commands.sshProfileList.mockResolvedValue([
      profile('profile-a', '生产服务器'),
      profile('profile-b', '测试环境'),
    ])
    const { api } = mountWorkspace()
    await settle()

    expect(api.profiles.value.map((item) => item.id)).toEqual(['profile-a', 'profile-b'])
    expect(api.filteredProfiles.value).toHaveLength(2)
    api.searchKeyword.value = '测试'
    expect(api.filteredProfiles.value.map((item) => item.id)).toEqual(['profile-b'])
  })
})

/* ── 3. 并发连接归属与取消竞态 ── */

describe('SSH 工作区 · 并发连接归属', () => {
  it('同 profile 两条连接乱序完成：结果各归原页签，进度不串台', async () => {
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const first = deferred<SshConnectOutcome>()
    const second = deferred<SshConnectOutcome>()
    env.commands.sshConnect.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const { api } = mountWorkspace()
    await settle()

    const openingFirst = api.openConnection('profile-a')
    const openingSecond = api.openConnection('profile-a')
    await settle(2)
    const [workspaceFirst, workspaceSecond] = api.connectionWorkspaces.value
    expect(api.connectionWorkspaces.value).toHaveLength(2)

    // 后发起的连接先返回
    second.resolve(connectOk('req-2', 'profile-a', 'conn-2'))
    await settle(4)
    first.resolve(connectOk('req-1', 'profile-a', 'conn-1'))
    await settle(4)

    expect(await openingFirst).toBe(workspaceFirst)
    expect(await openingSecond).toBe(workspaceSecond)
    expect(workspaceFirst.connection.sessionId).toBe('conn-1')
    expect(workspaceFirst.connectRequestId).toBe('req-1')
    expect(workspaceSecond.connection.sessionId).toBe('conn-2')
    expect(workspaceSecond.connectRequestId).toBe('req-2')

    // 已绑定 requestId 后按 requestId 精确匹配进度
    push.connectStage(stage('req-2', 'session'))
    expect(workspaceSecond.stageText).toBe('建立会话…')
    expect(workspaceFirst.stageText).toBe('')
  })

  it('占位工作区由首个 connect-stage 事件完成绑定，同 profile 两个占位各绑一个', async () => {
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const first = deferred<SshConnectOutcome>()
    const second = deferred<SshConnectOutcome>()
    env.commands.sshConnect.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
    const { api } = mountWorkspace()
    await settle()

    void api.openConnection('profile-a')
    void api.openConnection('profile-a')
    await settle(2)
    const [workspaceFirst, workspaceSecond] = api.connectionWorkspaces.value

    push.connectStage(stage('req-1', 'tcp'))
    expect(workspaceFirst.connectRequestId).toBe('req-1')
    expect(workspaceFirst.stageText).toBe('建立连接…')

    push.connectStage(stage('req-2', 'auth'))
    expect(workspaceSecond.connectRequestId).toBe('req-2')
    expect(workspaceSecond.stageText).toBe('认证中…')

    push.connectStage(stage('req-1', 'session'))
    expect(workspaceFirst.stageText).toBe('建立会话…')
    expect(workspaceSecond.stageText).toBe('认证中…')

    first.resolve(connectOk('req-1', 'profile-a', 'conn-1'))
    second.resolve(connectOk('req-2', 'profile-a', 'conn-2'))
    await settle(4)
  })

  it('连接期间关闭占位页签：成功后不重开页签，新会话被断开', async () => {
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const gate = deferred<SshConnectOutcome>()
    env.commands.sshConnect.mockReturnValueOnce(gate.promise)
    const { api } = mountWorkspace()
    await settle()

    const opening = api.openConnection('profile-a')
    await settle(2)
    await api.closeConnectionWorkspace(api.connectionWorkspaces.value[0].id)
    expect(api.connectionWorkspaces.value).toHaveLength(0)

    gate.resolve(connectOk('req-1', 'profile-a', 'conn-1'))
    expect(await opening).toBeUndefined()
    await settle(4)

    expect(api.connectionWorkspaces.value).toHaveLength(0)
    expect(env.commands.sshDisconnect).toHaveBeenCalledWith('conn-1')
  })

  it('卸载后晚到的连接成功结果不重开页签，新会话被断开', async () => {
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const gate = deferred<SshConnectOutcome>()
    env.commands.sshConnect.mockReturnValueOnce(gate.promise)
    const { api, unmount } = mountWorkspace()
    await settle()

    const opening = api.openConnection('profile-a')
    await settle(2)
    unmount()

    gate.resolve(connectOk('req-1', 'profile-a', 'conn-1'))
    expect(await opening).toBeUndefined()
    await settle(4)

    expect(api.connectionWorkspaces.value).toHaveLength(0)
    expect(env.commands.sshDisconnect).toHaveBeenCalledWith('conn-1')
  })

  it('连接成功刷新该服务器的最后连接时间', async () => {
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()
    const before = api.profiles.value[0].lastConnectedAt ?? 0

    await api.openConnection('profile-a')

    expect(api.profiles.value[0].lastConnectedAt).toBeGreaterThan(before)
    expect(api.connectionWorkspaces.value[0].connectRequest).toBe(1)
  })
})

/* ── 4. 主机密钥确认队列 ── */

describe('SSH 工作区 · 主机密钥确认队列', () => {
  it('并发确认请求排队：只应答队首，下一个升为队首', async () => {
    const { api } = mountWorkspace()
    await settle()

    push.hostKeyVerify(hostKeyRequest('hk-1'))
    push.hostKeyVerify(hostKeyRequest('hk-2'))
    expect(api.hostKeyRequest.value?.requestId).toBe('hk-1')

    await api.respondHostKey('trustOnce')

    expect(env.commands.sshHostKeyRespond.mock.calls).toEqual([
      [{ requestId: 'hk-1', decision: 'trustOnce' }],
    ])
    expect(api.hostKeyRequest.value?.requestId).toBe('hk-2')
  })

  it('取消只作用于队首，不误答下一项；空队列时是空操作', async () => {
    const { api } = mountWorkspace()
    await settle()

    push.hostKeyVerify(hostKeyRequest('hk-1'))
    push.hostKeyVerify(hostKeyRequest('hk-2'))
    await api.respondHostKey('cancel')

    expect(env.commands.sshHostKeyRespond.mock.calls).toEqual([
      [{ requestId: 'hk-1', decision: 'cancel' }],
    ])
    expect(api.hostKeyRequest.value?.requestId).toBe('hk-2')

    await api.respondHostKey('trustSave')
    await api.respondHostKey('trustSave')

    expect(env.commands.sshHostKeyRespond.mock.calls).toEqual([
      [{ requestId: 'hk-1', decision: 'cancel' }],
      [{ requestId: 'hk-2', decision: 'trustSave' }],
    ])
    expect(api.hostKeyRequest.value).toBeNull()
  })

  it('卸载时积压请求统一取消，队列清空', async () => {
    const { api, unmount } = mountWorkspace()
    await settle()
    push.hostKeyVerify(hostKeyRequest('hk-1'))
    push.hostKeyVerify(hostKeyRequest('hk-2'))
    push.hostKeyVerify(hostKeyRequest('hk-3'))

    unmount()
    await settle(4)

    expect(env.commands.sshHostKeyRespond.mock.calls).toEqual([
      [{ requestId: 'hk-1', decision: 'cancel' }],
      [{ requestId: 'hk-2', decision: 'cancel' }],
      [{ requestId: 'hk-3', decision: 'cancel' }],
    ])
    expect(api.hostKeyRequest.value).toBeNull()
  })
})

/* ── 5. 订阅生命周期与卸载清理 ── */

describe('SSH 工作区 · 订阅与清理', () => {
  it('卸载时逐路退订，每个退订句柄只释放一次', async () => {
    const { unmount } = mountWorkspace()
    await settle()
    expect(env.subscriptions).toHaveLength(5)

    unmount()
    await settle(4)

    for (const sub of env.subscriptions) expect(sub.unsubscribe).toHaveBeenCalledTimes(1)
  })

  it('卸载先于订阅完成：晚到的退订句柄仍被调用，且不产生多订阅', async () => {
    env.setHold(true)
    const { unmount } = mountWorkspace()
    await settle()
    // 订阅被挂起，初始化链停在第一路
    expect(env.subscriptions).toHaveLength(1)
    for (const sub of env.subscriptions) expect(sub.unsubscribe).not.toHaveBeenCalled()

    unmount()
    env.setHold(false)
    env.releasePending()
    await settle(14)

    expect(env.subscriptions).toHaveLength(5)
    for (const sub of env.subscriptions) expect(sub.unsubscribe).toHaveBeenCalledTimes(1)
  })

  it('卸载清理连接与空闲定时器：不再产生后续 IPC', async () => {
    useFakeTimers()
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api, unmount } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')

    unmount()
    await settle(4)
    expect(env.commands.sshDisconnect).toHaveBeenCalledWith('conn-profile-a')
    const callsAtUnmount = env.commands.sshDisconnect.mock.calls.length

    await advance(20 * 60_000)

    expect(env.commands.sshDisconnect.mock.calls).toHaveLength(callsAtUnmount)
    for (const sub of env.subscriptions) expect(sub.unsubscribe).toHaveBeenCalledTimes(1)
  })
})

/* ── 6. 断线与重连 ── */

describe('SSH 工作区 · 断线与自动重连', () => {
  it('容器或主动关闭的终端通知不重置工作区、不触发连接恢复提示', async () => {
    useFakeTimers()
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')
    const workspace = api.connectionWorkspaces.value[0]
    const connected = workspace.connection

    push.terminalClosed({ terminalId: 'docker-term-1', connectionId: connected.sessionId })
    push.terminalClosed({ terminalId: 'closed-local-term', connectionId: connected.sessionId })
    await advance(30_000)

    expect(workspace.connection).toBe(connected)
    expect(workspace.connection.status).toBe('connected')
    expect(workspace.reconnectTick).toBe(0)
    expect(env.commands.sshReconnect).not.toHaveBeenCalled()
    expect(toastText()).not.toContain('已恢复')
  })

  it('重连等待期忽略旧会话的断开事件，不覆盖重连态', async () => {
    useFakeTimers()
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')
    const workspace = api.connectionWorkspaces.value[0]

    api.handleLinkDead(workspace.id)
    await settle(2)
    expect(workspace.connection.status).toBe('reconnecting')

    push.connectionStatus(connection('profile-a', 'conn-profile-a', 'disconnected'))
    await settle(2)

    expect(workspace.connection.status).toBe('reconnecting')
    expect(workspace.stageText).toContain('自动重连')
  })

  it('意外断线按退避自动重连，成功后递增 tick 并提示恢复', async () => {
    useFakeTimers()
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')
    const workspace = api.connectionWorkspaces.value[0]
    env.commands.sshReconnect.mockResolvedValue(connectOk('req-r', 'profile-a', 'conn-2'))

    api.handleLinkDead(workspace.id)
    await settle(2)
    expect(workspace.connection.status).toBe('reconnecting')
    expect(workspace.stageText).toContain('1 秒后自动重连（第 1 次）')

    await advance(1_000)

    expect(env.commands.sshReconnect).toHaveBeenCalledWith('conn-profile-a')
    expect(workspace.connection.status).toBe('connected')
    expect(workspace.connection.sessionId).toBe('conn-2')
    expect(workspace.reconnectTick).toBe(1)
    expect(workspace.reconnectAttempt).toBe(0)
    expect(toastText()).toContain('已恢复')
  })

  it('手动关闭页签后，已排定的重连计时器不复活连接', async () => {
    useFakeTimers()
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')
    const workspace = api.connectionWorkspaces.value[0]
    env.commands.sshReconnect.mockResolvedValue(connectOk('req-r', 'profile-a', 'conn-2'))

    api.handleLinkDead(workspace.id)
    await settle(2)
    expect(workspace.connection.status).toBe('reconnecting')

    const disconnectsBeforeClose = env.commands.sshDisconnect.mock.calls.length
    await api.closeConnectionWorkspace(workspace.id)
    await advance(30_000)

    expect(env.commands.sshReconnect).not.toHaveBeenCalled()
    expect(api.connectionWorkspaces.value).toHaveLength(0)
    expect(env.commands.sshDisconnect.mock.calls.length).toBe(disconnectsBeforeClose + 1)
  })

  it('旧句柄被后端回收（SESSION_NOT_FOUND）时按 profile 整条重建', async () => {
    useFakeTimers()
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')
    const workspace = api.connectionWorkspaces.value[0]
    env.commands.sshConnect.mockResolvedValue(connectOk('req-rebuild', 'profile-a', 'conn-3'))

    api.handleLinkDead(workspace.id)
    await settle(2)
    await advance(1_000)

    expect(env.commands.sshReconnect).toHaveBeenCalledWith('conn-profile-a')
    expect(env.commands.sshConnect).toHaveBeenCalledTimes(2)
    expect(workspace.connection.status).toBe('connected')
    expect(workspace.connection.sessionId).toBe('conn-3')
    expect(workspace.connectRequestId).toBe('req-rebuild')
    expect(workspace.reconnectTick).toBe(1)
  })
})

/* ── 7. 空闲断开与会话活跃 ── */

describe('SSH 工作区 · 空闲断开', () => {
  beforeEach(() => {
    useSettingsStore().settings.tools = {
      ssh: { idleDisconnectMinutes: '10', idleDisconnectV2: true },
    }
  })

  it('空闲超过设定时长：自动断开并提示', async () => {
    useFakeTimers()
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')
    const workspace = api.connectionWorkspaces.value[0]
    expect(workspace.connection.status).toBe('connected')

    await advance(10 * 60_000 + 30_000)

    expect(workspace.connection.status).toBe('disconnected')
    expect(env.commands.sshDisconnect).toHaveBeenCalledWith('conn-profile-a')
    expect(toastText()).toContain('空闲超过 10 分钟')
  })

  it('终端输出与传输进度刷新活跃时间，长任务不被误断', async () => {
    useFakeTimers()
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')
    const workspace = api.connectionWorkspaces.value[0]

    await advance(9 * 60_000)
    push.terminalData({
      terminalId: 'term-1',
      connectionId: 'conn-profile-a',
      data: 'y',
      time: Date.now(),
    })
    await settle(2)
    await advance(9 * 60_000 + 30_000)
    expect(workspace.connection.status).toBe('connected')

    // 传输进度同样视为活跃（同一节流通道）；间隔刻意都短于 10 分钟阈值，只有停止活动才会被断开
    await advance(30_000)
    push.transferProgress({
      transferId: 'transfer-1',
      connectionId: 'conn-profile-a',
      localPath: 'C:/tmp/a.txt',
      remotePath: '/root/a.txt',
      transferred: 1,
      total: 2,
      done: false,
    })
    await settle(2)
    await advance(9 * 60_000 + 30_000)
    expect(workspace.connection.status).toBe('connected')

    await advance(10 * 60_000 + 30_000)
    expect(workspace.connection.status).toBe('disconnected')
    expect(toastText()).toContain('空闲超过 10 分钟')
  })

  it('空闲断开阈值非正数时不生效', async () => {
    useFakeTimers()
    useSettingsStore().settings.tools = {
      ssh: { idleDisconnectMinutes: '0', idleDisconnectV2: true },
    }
    env.commands.sshProfileList.mockResolvedValue([profile('profile-a', '生产服务器')])
    const { api } = mountWorkspace()
    await settle()
    await api.openConnection('profile-a')

    await advance(60 * 60_000)

    expect(api.connectionWorkspaces.value[0].connection.status).toBe('connected')
    expect(env.commands.sshDisconnect).not.toHaveBeenCalled()
  })
})
