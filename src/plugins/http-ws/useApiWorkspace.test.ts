import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { ApiRecord } from './contracts'
const mock = vi.hoisted(() => ({
  apiTreeMove: vi.fn(),
  apiList: vi.fn(),
  apiGroupList: vi.fn(),
  apiGroupCreate: vi.fn(),
  apiGroupMove: vi.fn(),
  apiMoveGroup: vi.fn(),
  apiSave: vi.fn(),
  apiRename: vi.fn(),
  apiDelete: vi.fn(),
  wsClose: vi.fn(),
  sseStop: vi.fn(),
  wsConnect: vi.fn(),
  wsRecv: vi.fn(),
}))
vi.mock('./ipc', () => ({ ipc: mock }))
import { useApiWorkspace } from './useApiWorkspace'
const record = (id: number): ApiRecord => ({
  id,
  type: 'http',
  name: `接口 ${id}`,
  method: 'POST',
  url: `https://example.invalid/${id}`,
  params: '[]',
  headers: '[]',
  bodyMode: 'text',
  body: `body ${id}`,
  groupName: '测试',
  options: '{}',
  updatedAt: '',
})

describe('接口库与多页签', () => {
  beforeEach(() => {
    vi.resetAllMocks()
    mock.apiList.mockResolvedValue([record(1), record(2)])
    mock.apiGroupList.mockResolvedValue(['测试'])
    mock.apiSave.mockResolvedValue(1)
    mock.apiDelete.mockResolvedValue(undefined)
  })
  it('新建立即标记未保存，保存后清除，后续编辑重新标记', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    const tab = workspace.create('http', '开发/用户')
    expect(tab.groupName).toBe('开发/用户')
    expect(workspace.dirty(tab)).toBe(true)
    await workspace.save(tab, '请求', tab.groupName)
    expect(mock.apiSave).toHaveBeenCalledWith(expect.objectContaining({ groupName: '开发/用户' }))
    expect(workspace.dirty(tab)).toBe(false)
    tab.draft.url = 'https://example.invalid/changed'
    expect(workspace.dirty(tab)).toBe(true)
    await workspace.dispose()
  })
  it('移动只更新分组，保留打开接口的未保存内容和脏标记', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    await workspace.load()
    const tab = workspace.open(workspace.apis.value[0])
    tab.draft.body = '未保存内容'
    mock.apiMoveGroup.mockResolvedValue(undefined)
    await workspace.move(tab.recordId!, '开发/用户')
    expect(mock.apiMoveGroup).toHaveBeenCalledWith(1, '开发/用户')
    expect(mock.apiSave).not.toHaveBeenCalled()
    expect(tab.groupName).toBe('开发/用户')
    expect(tab.draft.body).toBe('未保存内容')
    expect(workspace.dirty(tab)).toBe(true)
    await workspace.dispose()
  })
  it('移动失败不改变归属，移动中不允许保存旧分组快照', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    await workspace.load()
    const tab = workspace.open(workspace.apis.value[0])
    let reject!: (error: Error) => void
    mock.apiMoveGroup.mockReturnValue(
      new Promise((_, no) => {
        reject = no
      })
    )
    const moving = workspace.move(1, '目标')
    await expect(workspace.save(tab, tab.name, tab.groupName)).rejects.toThrow('正在移动')
    const failure = expect(moving).rejects.toThrow('写入失败')
    reject(new Error('写入失败'))
    await failure
    expect(tab.groupName).toBe('测试')
    expect(workspace.apis.value[0].groupName).toBe('测试')
    expect(workspace.moving.value.size).toBe(0)
    await workspace.dispose()
  })
  it('空分组可读取，创建子分组携带上级路径且失败不冒充成功', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    mock.apiGroupList.mockResolvedValue(['空分组', '开发/用户'])
    mock.apiGroupCreate.mockResolvedValue('开发/用户')
    await workspace.createGroup('用户', '开发')
    expect(mock.apiGroupCreate).toHaveBeenCalledWith('用户', '开发')
    expect(workspace.groups.value).toContain('空分组')
    mock.apiGroupCreate.mockRejectedValue(new Error('该分组已存在'))
    await expect(workspace.createGroup('用户', '开发')).rejects.toThrow('该分组已存在')
  })
  it('分组移动同步子树及新建草稿，保留请求内容、连接和相似前缀', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    const source = { ...record(1), groupName: '开发/用户/子组' }
    const tab = workspace.open(source)
    tab.draft.body = '未保存'
    const session = tab.session
    const fresh = workspace.create('ws', '开发/用户')
    const sibling = workspace.create('http', '开发/用户2')
    mock.apiGroupMove.mockResolvedValue('目标/用户')
    await workspace.moveGroup('开发/用户', '目标')
    expect(mock.apiGroupMove).toHaveBeenCalledWith('开发/用户', '目标')
    expect(tab.groupName).toBe('目标/用户/子组')
    expect(fresh.groupName).toBe('目标/用户')
    expect(sibling.groupName).toBe('开发/用户2')
    expect(tab.session).toBe(session)
    expect(tab.draft.body).toBe('未保存')
    expect(workspace.dirty(tab)).toBe(true)
    expect(mock.apiSave).not.toHaveBeenCalled()
    await workspace.dispose()
  })
  it('分组移动失败保留原路径，移动中阻止冲突写入并在失败后释放', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    await workspace.load()
    const tab = workspace.open(workspace.apis.value[0])
    let reject!: (error: Error) => void
    mock.apiGroupMove.mockReturnValue(
      new Promise((_, no) => {
        reject = no
      })
    )
    const moving = workspace.moveGroup('测试', '目标')
    await expect(workspace.save(tab, tab.name, tab.groupName)).rejects.toThrow('正在移动')
    await expect(workspace.rename(record(1), '改名', '测试')).rejects.toThrow('正在移动')
    await expect(workspace.remove(record(1))).rejects.toThrow('正在移动')
    await expect(workspace.createGroup('子组', '测试')).rejects.toThrow('正在移动')
    await expect(workspace.move(1, '目标')).rejects.toThrow('正在移动')
    const failure = expect(moving).rejects.toThrow('同名分组')
    reject(new Error('同名分组'))
    await failure
    expect(tab.groupName).toBe('测试')
    expect(workspace.groupMoving.value).toBe(false)
    await workspace.dispose()
  })
  it('库写入期间不能移动分组，避免旧路径被异步保存恢复', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    let resolve!: (id: number) => void
    mock.apiRename.mockReturnValue(
      new Promise<number>((yes) => {
        resolve = yes
      })
    )
    const saving = workspace.rename(record(1), '改名', '测试')
    await expect(workspace.moveGroup('测试', '目标')).rejects.toThrow('正在保存或移动')
    expect(mock.apiGroupMove).not.toHaveBeenCalled()
    resolve(1)
    await saving
    await workspace.dispose()
  })
  it('重复打开仅激活，切换和重命名另一个接口不覆盖草稿', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    const a = workspace.open(record(1)),
      b = workspace.open(record(2))
    a.draft.body = '未保存'
    expect(workspace.open(record(1))).toBe(a)
    expect(workspace.tabs).toHaveLength(2)
    await workspace.rename(record(2), '改名', '新分组')
    expect(mock.apiRename).toHaveBeenCalledWith(2, '改名', '新分组')
    expect(mock.apiSave).not.toHaveBeenCalled()
    expect(a.draft.body).toBe('未保存')
    expect(workspace.dirty(a)).toBe(true)
    expect(b.name).toBe('改名')
    await workspace.dispose()
  })
  it('保存异步完成后保留后续编辑，并把保存结果归还原页签', async () => {
    let resolve!: (id: number) => void
    mock.apiSave.mockReturnValue(new Promise<number>((yes) => (resolve = yes)))
    const workspace = useApiWorkspace(vi.fn(), () => true),
      a = workspace.open(record(1))
    a.draft.body = '提交的内容'
    const saving = workspace.save(a, '保存名', '分组')
    const b = workspace.open(record(2))
    a.draft.body = '后续编辑'
    resolve(1)
    await saving
    expect(a.draft.body).toBe('后续编辑')
    expect(workspace.dirty(a)).toBe(true)
    expect(workspace.current.value).toBe(b)
    await workspace.dispose()
  })
  it('保存期间拒绝重命名和删除，重命名失败后释放写入锁', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    const tab = workspace.open(record(1))
    let resolve!: (id: number) => void
    mock.apiSave.mockReturnValue(
      new Promise<number>((yes) => {
        resolve = yes
      })
    )
    const saving = workspace.save(tab, tab.name, tab.groupName)
    await expect(workspace.rename(record(1), '改名', '测试')).rejects.toThrow('正在保存')
    await expect(workspace.remove(record(1))).rejects.toThrow('正在保存')
    expect(mock.apiRename).not.toHaveBeenCalled()
    expect(mock.apiDelete).not.toHaveBeenCalled()
    resolve(1)
    await saving
    let reject!: (error: Error) => void
    mock.apiRename.mockReturnValue(
      new Promise((_, no) => {
        reject = no
      })
    )
    const renaming = workspace.rename(record(1), '改名', '新组')
    await expect(workspace.save(tab, tab.name, tab.groupName)).rejects.toThrow('正在移动')
    await expect(workspace.remove(record(1))).rejects.toThrow('正在移动')
    const failure = expect(renaming).rejects.toThrow('写入失败')
    reject(new Error('写入失败'))
    await failure
    expect(tab.name).toBe('接口 1')
    expect(tab.groupName).toBe('测试')
    expect(workspace.moving.value.size).toBe(0)
    await workspace.dispose()
  })
  it('关闭非活动页签不跳页，删除保留打开草稿供另存', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true),
      a = workspace.open(record(1)),
      b = workspace.open(record(2))
    await workspace.close(a)
    expect(workspace.current.value).toBe(b)
    await workspace.remove(record(2))
    expect(b.recordId).toBeNull()
    expect(b.draft.body).toBe('body 2')
    expect(workspace.dirty(b)).toBe(true)
    await workspace.close(b)
    expect(workspace.tabs).toHaveLength(0)
    expect(workspace.activeKey.value).toBe('')
  })
  it('接口库读取失败可感知，不能假装空库', async () => {
    const workspace = useApiWorkspace(vi.fn(), () => true)
    await workspace.load()
    mock.apiList.mockRejectedValue(new Error('数据库不可读'))
    await workspace.load()
    expect(workspace.apis.value).toHaveLength(2)
    expect(workspace.loadError.value).toContain('数据库不可读')
  })
})

it('树排序失败保留顺序和草稿，进行中拒绝重复写入', async () => {
  const workspace = useApiWorkspace(vi.fn(), () => true)
  mock.apiList.mockResolvedValue([record(1), record(2)])
  mock.apiGroupList.mockResolvedValue(['测试'])
  await workspace.load()
  const tab = workspace.open(workspace.apis.value[0])
  tab.draft.body = '未保存正文'
  let reject!: (error: Error) => void
  mock.apiTreeMove.mockReturnValue(
    new Promise((_, no) => {
      reject = no
    })
  )
  const moving = workspace.moveTree({ id: 'api:1', targetId: 'api:2', position: 'after' })
  await expect(
    workspace.moveTree({ id: 'api:2', targetId: 'api:1', position: 'before' })
  ).rejects.toThrow('正在保存或移动')
  await expect(workspace.save(tab, tab.name, tab.groupName)).rejects.toThrow('正在移动')
  const failure = expect(moving).rejects.toThrow('写入失败')
  reject(new Error('写入失败'))
  await failure
  expect(workspace.apis.value.map((api) => api.id)).toEqual([1, 2])
  expect(tab.draft.body).toBe('未保存正文')
  expect(workspace.groupMoving.value).toBe(false)
  await workspace.dispose()
})
