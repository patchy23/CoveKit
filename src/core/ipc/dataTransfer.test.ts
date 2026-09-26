import { expect, it, vi } from 'vitest'
import { ipc } from './ipc'
const invoke = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@/core/resourceMonitor/metrics', () => ({ beginMeasuredCommand: () => undefined }))

it('数据包的登记、导出、预览、提交与取消传递同一请求标识', async () => {
  const selection = { entries: [], datasets: [] }
  await ipc.dataTransferPrepare()
  await ipc.dataExportStart(selection, 'test-only-password', 'test.pbdata', 'request')
  await ipc.dataImportInspect('test.pbdata', 'test-only-password', 'request')
  await ipc.dataImportCommit('plan', 'test-only-password', 'request')
  await ipc.dataTransferCancel('request')
  expect(invoke.mock.calls).toEqual([
    ['data_transfer_prepare', {}],
    [
      'data_export_start',
      { selection, password: 'test-only-password', path: 'test.pbdata', requestId: 'request' },
    ],
    [
      'data_import_inspect',
      { password: 'test-only-password', path: 'test.pbdata', requestId: 'request' },
    ],
    [
      'data_import_commit',
      { planId: 'plan', password: 'test-only-password', requestId: 'request' },
    ],
    ['data_transfer_cancel', { requestId: 'request' }],
  ])
})
