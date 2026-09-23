import { describe, expect, it, vi } from 'vitest'
import { createDownloadTargets } from './downloadTargets'
import type { RemoteFile } from '../contracts'
const file = { path: '/srv/a', name: 'a' } as RemoteFile
function setup() {
  let session = 'one'
  const submit = vi.fn().mockResolvedValue(undefined)
  const chooseDirectory = vi.fn().mockResolvedValue('/downloads')
  const directory = vi.fn().mockResolvedValue('/default')
  const deps = {
    session: () => session,
    defaultDirectory: directory,
    chooseDirectory,
    join: async (parent: string, name: string) => parent + '/' + name,
    submit,
    notify: vi.fn(),
  }
  return {
    deps,
    targets: createDownloadTargets(deps),
    change: () => {
      session = 'two'
    },
  }
}
describe('下载目标', () => {
  it('批量只选择一次目录，取消不创建任务', async () => {
    const { deps, targets } = setup()
    await targets.choose([file, { ...file, name: 'b', path: '/srv/b' }])
    expect(deps.chooseDirectory).toHaveBeenCalledTimes(1)
    expect(deps.submit).toHaveBeenNthCalledWith(2, '/downloads/b', '/srv/b')
    deps.chooseDirectory.mockResolvedValueOnce(null)
    await targets.choose([file])
    expect(deps.submit).toHaveBeenCalledTimes(2)
  })
  it('选择期间重连，旧选择不得提交到新会话', async () => {
    const { deps, targets, change } = setup()
    deps.chooseDirectory.mockImplementationOnce(async () => {
      change()
      return '/downloads'
    })
    await targets.choose([file])
    expect(deps.submit).not.toHaveBeenCalled()
  })
  it('拖拽直接使用本地栏，不打开选择器或解析默认目录', async () => {
    const { deps, targets } = setup()
    await targets.direct([file], '/target')
    expect(deps.submit).toHaveBeenCalledWith('/target/a', '/srv/a')
    expect(deps.chooseDirectory).not.toHaveBeenCalled()
    expect(deps.defaultDirectory).not.toHaveBeenCalled()
  })
})
