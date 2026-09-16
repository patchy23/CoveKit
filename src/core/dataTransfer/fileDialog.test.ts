/**
 * 数据包文件对话框用例（sync L2）
 *
 * 关键口径：过滤器统一 `.pbdata`；取消不是错误（不提示）；失败要提示且返回 null；
 * 手输文件名缺扩展名时补上，已有其它扩展名不擅自改写。
 */
import { describe, expect, it, vi } from 'vitest'
import {
  createFileDialog,
  ensurePackExtension,
  PB_DATA_EXTENSION,
} from '@/core/dataTransfer/fileDialog'

describe('ensurePackExtension', () => {
  it('缺扩展名时补 .pbdata', () => {
    expect(ensurePackExtension('D:/out/patchybox')).toBe('D:/out/patchybox.pbdata')
    expect(ensurePackExtension('  D:/out/x  ')).toBe('D:/out/x.pbdata')
  })

  it('已有扩展名（含不带路径的同名文件）时不改写', () => {
    expect(ensurePackExtension('D:/out/x.pbdata')).toBe('D:/out/x.pbdata')
    expect(ensurePackExtension('D:/out/2026.09.pack')).toBe('D:/out/2026.09.pack')
  })

  it('空路径原样返回，不造出「.pbdata」这种文件名', () => {
    expect(ensurePackExtension('   ')).toBe('')
  })
})

describe('fileDialog', () => {
  it('保存：带 .pbdata 过滤器与建议名，取消返回 null 且不提示', async () => {
    const save = vi.fn().mockResolvedValue(null)
    const notify = vi.fn()
    const api = createFileDialog({ save, notify })

    await expect(api.pickSavePath({ defaultPath: 'D:/out' })).resolves.toBeNull()

    const options = save.mock.calls[0]?.[0] as {
      filters: { extensions: string[] }[]
      defaultPath?: string
    }
    expect(options.filters[0]?.extensions).toEqual([PB_DATA_EXTENSION])
    expect(options.defaultPath).toBe('D:/out')
    expect(notify).not.toHaveBeenCalled()
  })

  it('保存：用户手输不带扩展名时补上', async () => {
    const api = createFileDialog({
      save: vi.fn().mockResolvedValue('D:/out/pack'),
      notify: vi.fn(),
    })
    await expect(api.pickSavePath()).resolves.toBe('D:/out/pack.pbdata')
  })

  it('保存：失败提示用户并返回 null，不静默吞错误', async () => {
    const notify = vi.fn()
    const api = createFileDialog({
      save: vi.fn().mockRejectedValue(new Error('磁盘只读')),
      notify,
    })

    await expect(api.pickSavePath()).resolves.toBeNull()
    expect(notify).toHaveBeenCalledTimes(1)
    expect(String(notify.mock.calls[0]?.[0])).toContain('磁盘只读')
  })

  it('打开：单选文件 + .pbdata 过滤器；取消不提示', async () => {
    const open = vi.fn().mockResolvedValue(null)
    const notify = vi.fn()
    const api = createFileDialog({ open, notify })

    await expect(api.pickOpenPath()).resolves.toBeNull()

    const options = open.mock.calls[0]?.[0] as {
      multiple: boolean
      directory: boolean
      filters: { extensions: string[] }[]
    }
    expect(options.multiple).toBe(false)
    expect(options.directory).toBe(false)
    expect(options.filters[0]?.extensions).toEqual([PB_DATA_EXTENSION])
    expect(notify).not.toHaveBeenCalled()
  })

  it('打开：返回所选路径，不追加扩展名（文件已存在）', async () => {
    const api = createFileDialog({
      open: vi.fn().mockResolvedValue('D:/in/a.pbdata'),
      notify: vi.fn(),
    })
    await expect(api.pickOpenPath()).resolves.toBe('D:/in/a.pbdata')
  })

  it('打开：失败提示用户并返回 null', async () => {
    const notify = vi.fn()
    const api = createFileDialog({
      open: vi.fn().mockRejectedValue('对话框不可用'),
      notify,
    })

    await expect(api.pickOpenPath()).resolves.toBeNull()
    expect(notify).toHaveBeenCalledTimes(1)
    expect(String(notify.mock.calls[0]?.[0])).toContain('对话框不可用')
  })
})
