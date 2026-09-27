import { expect, it, vi } from 'vitest'
import { ipc } from './ipc'
import type { Channel } from '@tauri-apps/api/core'
import type { TransferProgress } from './contracts'
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

it('数据包和凭证备份传递同一请求归属及资源进展通道', async () => {
  invoke.mockClear()
  const onProgress = { onmessage: vi.fn() } as unknown as Channel<TransferProgress>
  await ipc.dataImportInspect('test.pbdata', 'test-only-password', 'memory-request', onProgress)
  await ipc.vaultExport('test.pbvault', 'test-only-password', 'memory-request', onProgress)
  await ipc.vaultImport('test.pbvault', 'test-only-password', false, 'memory-request', onProgress)
  for (const [, payload] of invoke.mock.calls) {
    expect(payload.requestId).toBe('memory-request')
    expect(payload.onProgress).toBe(onProgress)
  }
})
