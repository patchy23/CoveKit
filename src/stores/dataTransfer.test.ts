/**
 * 数据管理状态的行为测试（sync L2）
 *
 * 关键口径：命令失败必须可见（toast + 诊断）且返回 false，调用方留在原步；
 * 密码绝不进 store（调用结束即丢）；导入成功后刷新空间列表，导出成功后留下报告。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

// vi.mock 会被提升到 import 之前，替身必须用 vi.hoisted 一起提上去
const { recordError, toast, ipcMock } = vi.hoisted(() => ({
  recordError: vi.fn(),
  toast: vi.fn(),
  /** 可编排的 IPC 替身：每个命令一个 vi.fn，用例内改返回值 */
  ipcMock: {
    dataExportCatalog: vi.fn(),
    dataSpacesList: vi.fn(async () => [] as unknown[]),
    dataExportStart: vi.fn(),
    dataImportInspect: vi.fn(),
    dataImportPlan: vi.fn(),
    dataImportCommit: vi.fn(),
    dataTransferCancel: vi.fn(async () => ({ cancelled: false })),
    dataSpaceSwitch: vi.fn(),
  },
}))

vi.mock('@/core/ipc/ipc', () => ({ ipc: ipcMock }))
vi.mock('@/core/diagnostics/errors', () => ({
  recordError: (...args: unknown[]) => recordError(...args),
}))
vi.mock('@/stores/ui', () => ({
  useUiStore: () => ({ toast: (...args: unknown[]) => toast(...args) }),
}))

import { useDataTransferStore } from '@/stores/dataTransfer'

function catalogFixture() {
  return {
    sourceSpaceId: 'default',
    sourceSpaceName: '默认空间',
    datasets: [],
    entries: [],
    defaults: { entries: [], datasets: ['core.favorites'], includeCredentials: true },
    warnings: [],
  }
}

function reportFixture() {
  return {
    path: 'D:/导出/patchybox-data.pbdata',
    bytes: 1024,
    packageId: 'pkg-1',
    sourceSpaceName: '默认空间',
    counts: { 'ssh.profiles': 2 },
    excluded: [],
    secretIncluded: true,
  }
}

describe('dataTransfer store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    recordError.mockClear()
    toast.mockClear()
    for (const fn of Object.values(ipcMock)) fn.mockClear()
    ipcMock.dataSpacesList.mockResolvedValue([])
    ipcMock.dataTransferCancel.mockResolvedValue({ cancelled: false })
  })

  it('读目录成功：填目录并按默认值重置勾选', async () => {
    ipcMock.dataExportCatalog.mockResolvedValue(catalogFixture())
    const store = useDataTransferStore()
    await expect(store.loadCatalog()).resolves.toBe(true)
    expect(store.catalog?.sourceSpaceName).toBe('默认空间')
    expect(store.choice.includeFavorites).toBe(true)
  })

  it('读目录失败：记诊断 + toast，返回 false 且不留半份目录', async () => {
    ipcMock.dataExportCatalog.mockRejectedValue(new Error('目录读取失败'))
    const store = useDataTransferStore()
    await expect(store.loadCatalog()).resolves.toBe(false)
    expect(store.catalog).toBeNull()
    expect(toast).toHaveBeenCalledWith('目录读取失败')
    expect(recordError).toHaveBeenCalledTimes(1)
  })

  it('导出成功留下报告；密码不进 store', async () => {
    ipcMock.dataExportCatalog.mockResolvedValue(catalogFixture())
    ipcMock.dataExportStart.mockResolvedValue({ taskId: 'task-1', report: reportFixture() })
    const store = useDataTransferStore()
    await store.loadCatalog()
    await expect(store.exportPack('s3cret-pass', 'D:/导出/patchybox-data.pbdata')).resolves.toBe(
      true
    )
    expect(store.exportReport?.packageId).toBe('pkg-1')
    expect(store.taskId).toBe('task-1')
    expect(JSON.stringify(store.$state)).not.toContain('s3cret-pass')
  })

  it('未加载目录时导出直接失败并提示（不静默）', async () => {
    const store = useDataTransferStore()
    await expect(store.exportPack('s3cret-pass', 'x.pbdata')).resolves.toBe(false)
    expect(toast).toHaveBeenCalledTimes(1)
    expect(ipcMock.dataExportStart).not.toHaveBeenCalled()
  })

  it('提交导入成功后刷新空间列表并留下报告', async () => {
    ipcMock.dataImportInspect.mockResolvedValue({
      inspectId: 'insp-1',
      summary: {
        packageId: 'pkg-1',
        sourceSpaceId: 'default',
        sourceSpaceName: '默认空间',
        createdAt: '',
        appVersion: '0.1.0',
        platform: 'windows',
        datasets: [],
      },
      defaults: { datasets: ['ssh.profiles'] },
      preview: [],
      pending: [],
      excluded: [],
      duplicate: null,
    })
    ipcMock.dataImportPlan.mockResolvedValue({
      planId: 'imp-1',
      spaceId: '11111111-1111-4111-8111-111111111111',
      spaceName: '工作机副本',
      targetDir: 'D:/spaces/1111',
      preview: [{ dataset: 'ssh.profiles', id: 'p1', label: 'A', outcome: 'added', note: null }],
      pending: [],
      excluded: [],
      counts: { 'ssh.profiles': 1 },
    })
    ipcMock.dataImportCommit.mockResolvedValue({
      taskId: 'task-2',
      report: {
        spaceId: '11111111-1111-4111-8111-111111111111',
        spaceName: '工作机副本',
        packageId: 'pkg-1',
        sourceSpaceId: 'default',
        sourceSpaceName: '默认空间',
        importedAt: '2026-09-16T12:00:00+08:00',
        counts: { 'ssh.profiles': 1 },
        declaredCounts: {},
        pending: [],
        excluded: [],
      },
    })
    ipcMock.dataSpacesList.mockResolvedValue([
      {
        spaceId: '11111111-1111-4111-8111-111111111111',
        name: '工作机副本',
        createdAt: '2026-09-16T12:00:00+08:00',
        active: false,
        imported: true,
        sourceSpaceName: '默认空间',
        importedAt: '2026-09-16T12:00:00+08:00',
        counts: { 'ssh.profiles': 1 },
      },
    ])

    const store = useDataTransferStore()
    await store.inspectPack('D:/包.pbdata', 's3cret-pass')
    expect(store.importDatasets).toEqual(['ssh.profiles'])
    await expect(store.planImport('工作机副本', false)).resolves.toBe(true)
    await expect(store.commitImport('s3cret-pass')).resolves.toBe(true)
    expect(store.importReport?.spaceName).toBe('工作机副本')
    expect(store.spaces).toHaveLength(1)
    expect(JSON.stringify(store.$state)).not.toContain('s3cret-pass')
  })

  it('切换空间失败：返回 false 并提示原因', async () => {
    ipcMock.dataSpaceSwitch.mockRejectedValue(new Error('空间不存在或尚未登记'))
    const store = useDataTransferStore()
    await expect(store.switchSpace('22222222-2222-4222-8222-222222222222')).resolves.toBe(false)
    expect(toast).toHaveBeenCalledWith('空间不存在或尚未登记')
  })
})
