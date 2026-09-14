/**
 * frpc 下载结果提示的行为测试
 *
 * 关键口径：后端在「装好了但没通过 SHA256 强校验」时用 `error` 字段带回原因，
 * 前端必须接住并展示——它曾经被静默丢掉，用户无从判断这次安装是否可信。
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'

const binaryDownload = vi.fn()
const listen = vi.fn()

vi.mock('../ipc', () => ({
  ipc: {
    binaryDownload: (...args: unknown[]) => binaryDownload(...args),
    binaryDetect: vi.fn(),
    binaryVersions: vi.fn(),
  },
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listen(...args),
}))

const { useFrpBinary } = await import('./useFrpBinary')

describe('frpc 下载结果提示', () => {
  beforeEach(() => {
    binaryDownload.mockReset()
    listen.mockReset()
    listen.mockResolvedValue(() => {})
  })

  it('未强校验时把后端说明带到 downloadWarning', async () => {
    const warning =
      '上游未提供 frp_sha256_checksums.txt（版本 0.40.0 可能较旧），未强校验；仅校验了解压完整性。压缩包 SHA256：abc'
    binaryDownload.mockResolvedValue({ ok: true, path: 'C:/x/frpc.exe', error: warning })
    const binary = useFrpBinary()

    await binary.download('0.40.0')

    expect(binary.downloadWarning.value).toBe(warning)
    expect(binary.downloadError.value).toBe('')
  })

  it('强校验通过时不出现提示', async () => {
    binaryDownload.mockResolvedValue({ ok: true, path: 'C:/x/frpc.exe' })
    const binary = useFrpBinary()

    await binary.download('0.70.1')

    expect(binary.downloadWarning.value).toBe('')
  })

  it('下载失败走 downloadError，不残留上一次的警告', async () => {
    const warning = '上游未提供 frp_sha256_checksums.txt，未强校验'
    binaryDownload.mockResolvedValueOnce({ ok: true, path: 'C:/x/frpc.exe', error: warning })
    const binary = useFrpBinary()
    await binary.download('0.40.0')
    expect(binary.downloadWarning.value).toBe(warning)

    binaryDownload.mockResolvedValueOnce({ ok: false, error: 'SHA256 校验不通过' })
    await binary.download('0.70.1')

    expect(binary.downloadError.value).toBe('SHA256 校验不通过')
    expect(binary.downloadWarning.value).toBe('')
  })
})
