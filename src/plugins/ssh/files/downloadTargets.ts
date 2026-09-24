import type { RemoteFile } from '../contracts'

/** 目录选择与直接拖拽下载共享提交逻辑；会话变化时不能向新会话提交旧选中项。 */
export function createDownloadTargets(deps: {
  session: () => string | undefined
  defaultDirectory: () => Promise<string>
  chooseFile?: (path: string) => Promise<string | null>
  parent?: (path: string) => Promise<string>
  chooseDirectory: (path: string) => Promise<string | string[] | null>
  join: (directory: string, name: string) => Promise<string>
  submit: (local: string, remote: string) => Promise<unknown>
  notify: (message: string) => void
}) {
  let choosing = false
  let lastDirectory = ''
  async function submit(items: RemoteFile[], directory: string, session: string) {
    const paths = await Promise.all(items.map((item) => deps.join(directory, item.name)))
    if (deps.session() !== session) return
    let count = 0
    for (const [index, item] of items.entries()) {
      if (deps.session() !== session) break
      try {
        const path = paths[index]
        if (deps.session() !== session) break
        void deps.submit(path, item.path).catch((error) => deps.notify(String(error)))
        count++
      } catch (error) {
        deps.notify('下载失败：' + item.name + '：' + String(error))
      }
    }
    if (count) deps.notify('已开始下载 ' + count + ' 项')
  }
  async function choose(items: RemoteFile[]) {
    const session = deps.session(),
      snapshot = items.map((item) => ({ ...item }))
    if (!session || choosing) return
    if (!snapshot.length) {
      deps.notify('请先选择文件')
      return
    }
    choosing = true
    try {
      let initial = lastDirectory
      if (!initial) {
        try {
          initial = await deps.defaultDirectory()
        } catch (error) {
          deps.notify(String(error))
        }
      }
      if (deps.session() !== session) return
      if (snapshot.length === 1 && !snapshot[0].isDir && deps.chooseFile) {
        const path = await deps.chooseFile(await deps.join(initial, snapshot[0].name))
        if (!path || deps.session() !== session) return
        if (deps.parent) lastDirectory = await deps.parent(path)
        if (deps.session() !== session) return
        await deps.submit(path, snapshot[0].path)
        return
      }
      const directory = await deps.chooseDirectory(initial)
      if (typeof directory !== 'string' || deps.session() !== session) return
      lastDirectory = directory
      await submit(snapshot, directory, session)
    } catch (error) {
      deps.notify('选择下载目录失败：' + String(error))
    } finally {
      choosing = false
    }
  }
  async function direct(items: RemoteFile[], directory: string) {
    const session = deps.session()
    if (!session) return
    if (!directory) {
      deps.notify('请先打开本地目标目录')
      return
    }
    await submit(
      items.map((item) => ({ ...item })),
      directory,
      session
    )
  }
  return { choose, direct }
}
